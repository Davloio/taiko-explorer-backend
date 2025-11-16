use anyhow::Result;
use std::time::{Duration, Instant};
use tracing::{error, info, warn};
use std::sync::Arc;
use futures::future::join_all;
use futures::StreamExt;
use futures_util::SinkExt;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use serde_json::json;

use crate::db::operations::BlockRepository;
use crate::db::address_analytics::AddressAnalyticsRepository;
use crate::rpc::client::TaikoRpcClient;
use crate::rpc::batch::BatchRpcClient;
use crate::rpc::types::{block_to_new_block, transaction_to_new_transaction};
use crate::bridge::BridgeDetector;
use crate::analytics::analytics::process_block_analytics;
use crate::websocket::WebSocketBroadcaster;

/// MEGA AGGRESSIVE batch indexer - processes hundreds of blocks at once
pub struct MegaBatchIndexer {
    nodes: Vec<Arc<TaikoRpcClient>>,
    batch_clients: Vec<Arc<BatchRpcClient>>,
    node_urls: Vec<String>,  // Store original URLs for WebSocket connections
    block_repo: Arc<BlockRepository>,
    analytics_repo: Arc<AddressAnalyticsRepository>,
    bridge_detector: Arc<BridgeDetector>,
    websocket_broadcaster: Arc<WebSocketBroadcaster>,
}

impl MegaBatchIndexer {
    pub async fn new(
        block_repo: BlockRepository,
        analytics_repo: AddressAnalyticsRepository,
        websocket_broadcaster: Arc<WebSocketBroadcaster>,
        working_nodes: Vec<String>,
    ) -> Result<Self> {
        
        let mut nodes = Vec::new();
        let mut batch_clients = Vec::new();
        let mut node_urls = Vec::new();
        
        for node_url in working_nodes {
            info!("🔗 Initializing mega batch node: {}", node_url);
            
            let client = TaikoRpcClient::new(&node_url, 167000).await?;
            nodes.push(Arc::new(client));
            
            let batch_client = BatchRpcClient::new(node_url.clone());
            batch_clients.push(Arc::new(batch_client));
            
            node_urls.push(node_url);
        }
        
        info!("✅ Initialized {} nodes for MEGA BATCHING", nodes.len());
        
        Ok(Self {
            nodes,
            batch_clients,
            node_urls,
            block_repo: Arc::new(block_repo),
            analytics_repo: Arc::new(analytics_repo),
            bridge_detector: Arc::new(BridgeDetector::new()),
            websocket_broadcaster,
        })
    }
    
    /// MEGA AGGRESSIVE indexing - process 1000+ blocks in massive batches
    pub async fn start_mega_batching(&self) -> Result<()> {
        info!("🚀 STARTING MEGA BATCH INDEXER");
        info!("🎯 Target: 1000 blocks per batch, instant DB storage");
        
        loop {
            let latest_db_block = self.block_repo.get_latest_block_number()?.unwrap_or(0);
            let latest_rpc_block = self.get_latest_block().await?;
            let blocks_behind = latest_rpc_block.saturating_sub(latest_db_block as u64);
            
            info!("📊 DB: {}, RPC: {}, Behind: {} blocks", latest_db_block, latest_rpc_block, blocks_behind);
            
            if blocks_behind <= 5 {  // Within 5 blocks of head = essentially caught up
                info!("🎯 Caught up! Switching to LIVE MODE with WebSocket subscription");
                
                // Enter live mode with WebSocket subscription
                match self.run_live_mode().await {
                    Ok(_) => info!("Live mode ended, returning to batch mode"),
                    Err(e) => error!("Live mode failed: {}, falling back to batch mode", e),
                }
                continue;
            }
            
            // MEGA BATCH: Process up to 1000 blocks at once - ULTRA AGGRESSIVE
            let batch_size = std::cmp::min(1000, blocks_behind);
            let start_block = latest_db_block as u64 + 1;
            let end_block = start_block + batch_size - 1;
            
            info!("🚀 MEGA BATCH: Processing blocks {} to {} ({} blocks)", start_block, end_block, batch_size);
            
            let start_time = Instant::now();
            match self.process_mega_batch(start_block, end_block).await {
                Ok(processed) => {
                    let duration = start_time.elapsed();
                    let blocks_per_second = processed as f64 / duration.as_secs_f64();
                    info!("✅ MEGA BATCH COMPLETE: {} blocks in {:.2}s = {:.1} blocks/second", 
                          processed, duration.as_secs_f64(), blocks_per_second);
                }
                Err(e) => {
                    error!("❌ Mega batch failed: {}", e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }
    
    /// Process a massive batch of blocks (up to 1000)
    async fn process_mega_batch(&self, start_block: u64, end_block: u64) -> Result<u64> {
        let batch_size = end_block - start_block + 1;
        info!("📦 MEGA BATCH: Fetching {} blocks", batch_size);
        
        // Step 1: Fetch ALL block headers in parallel (super fast)
        let fetch_start = Instant::now();
        let blocks = self.fetch_blocks_mega_parallel(start_block, end_block).await?;
        let fetch_duration = fetch_start.elapsed();
        info!("⚡ Block headers fetched in {:.2}s ({:.1} blocks/second)", 
              fetch_duration.as_secs_f64(), batch_size as f64 / fetch_duration.as_secs_f64());
        
        // Step 2: Extract ALL transaction hashes
        let mut all_tx_hashes = Vec::new();
        for block in &blocks {
            for tx in &block.transactions {
                all_tx_hashes.push(tx.hash);
            }
        }
        
        info!("📦 Total transactions to fetch receipts: {}", all_tx_hashes.len());
        
        // Step 3: Fetch ALL receipts in massive batches (1000 receipts per call)
        let receipt_start = Instant::now();
        let all_receipts = self.fetch_receipts_ultra_batch(all_tx_hashes).await?;
        let receipt_duration = receipt_start.elapsed();
        info!("⚡ All receipts fetched in {:.2}s ({:.1} receipts/second)", 
              receipt_duration.as_secs_f64(), all_receipts.len() as f64 / receipt_duration.as_secs_f64());
        
        // Step 4: Process and store EVERYTHING in bulk
        let store_start = Instant::now();
        let processed = self.bulk_store_everything(blocks, all_receipts).await?;
        let store_duration = store_start.elapsed();
        info!("💾 Bulk storage complete in {:.2}s ({:.1} blocks/second)", 
              store_duration.as_secs_f64(), processed as f64 / store_duration.as_secs_f64());
        
        Ok(processed)
    }
    
    /// Fetch blocks in massive parallel - use all nodes simultaneously
    async fn fetch_blocks_mega_parallel(&self, start_block: u64, end_block: u64) -> Result<Vec<ethers::types::Block<ethers::types::Transaction>>> {
        let batch_size = end_block - start_block + 1;
        let blocks_per_node = (batch_size + self.nodes.len() as u64 - 1) / self.nodes.len() as u64;
        
        let mut fetch_tasks = Vec::new();
        
        for (node_idx, node) in self.nodes.iter().enumerate() {
            let node_start = start_block + (node_idx as u64 * blocks_per_node);
            let node_end = std::cmp::min(node_start + blocks_per_node - 1, end_block);
            
            if node_start <= end_block {
                let node_clone = node.clone();
                let task = tokio::spawn(async move {
                    let mut node_blocks = Vec::new();
                    
                    // Fetch blocks in chunks of 50 to avoid RPC limits
                    for chunk_start in (node_start..=node_end).step_by(50) {
                        let chunk_end = std::cmp::min(chunk_start + 49, node_end);
                        
                        let mut chunk_tasks = Vec::new();
                        for block_num in chunk_start..=chunk_end {
                            let node = node_clone.clone();
                            chunk_tasks.push(tokio::spawn(async move {
                                node.get_block_by_number(block_num).await
                            }));
                        }
                        
                        let chunk_results = join_all(chunk_tasks).await;
                        for result in chunk_results {
                            match result {
                                Ok(Ok(Some(block))) => node_blocks.push(block),
                                Ok(Ok(None)) => warn!("Block not found"),
                                Ok(Err(e)) => error!("Failed to fetch block: {}", e),
                                Err(e) => error!("Task failed: {}", e),
                            }
                        }
                        
                        // Small delay to be nice to the RPC
                        if chunk_end < node_end {
                            tokio::time::sleep(Duration::from_millis(50)).await;
                        }
                    }
                    
                    node_blocks
                });
                
                fetch_tasks.push(task);
            }
        }
        
        // Collect all blocks from all nodes
        let mut all_blocks = Vec::new();
        let results = join_all(fetch_tasks).await;
        
        for result in results {
            match result {
                Ok(mut blocks) => all_blocks.append(&mut blocks),
                Err(e) => error!("Node fetch task failed: {}", e),
            }
        }
        
        // Sort blocks by number
        all_blocks.sort_by_key(|b| b.number.unwrap_or_default());
        
        Ok(all_blocks)
    }
    
    /// Fetch receipts in ultra-massive batches (1000+ receipts per call)
    async fn fetch_receipts_ultra_batch(&self, tx_hashes: Vec<ethers::types::H256>) -> Result<Vec<Option<ethers::types::TransactionReceipt>>> {
        if tx_hashes.is_empty() {
            return Ok(Vec::new());
        }
        
        info!("🚀 ULTRA BATCH: Fetching {} receipts in batches of 1000", tx_hashes.len());
        
        // Use the fastest node for receipts
        let batch_client = &self.batch_clients[0];
        
        // Fetch in chunks of 1000 receipts - this is the key optimization!
        let chunk_size = 1000;
        let mut all_receipts = Vec::with_capacity(tx_hashes.len());
        
        for chunk in tx_hashes.chunks(chunk_size) {
            info!("📦 Fetching batch of {} receipts", chunk.len());
            let receipts = batch_client.get_receipts_batch(chunk.to_vec(), chunk.len()).await?;
            all_receipts.extend(receipts);
            
            // Tiny delay to avoid overwhelming the node
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        
        Ok(all_receipts)
    }
    
    /// Store everything in massive bulk operations
    async fn bulk_store_everything(
        &self,
        blocks: Vec<ethers::types::Block<ethers::types::Transaction>>,
        receipts: Vec<Option<ethers::types::TransactionReceipt>>,
    ) -> Result<u64> {
        info!("💾 BULK STORAGE: Processing {} blocks with bulk operations", blocks.len());
        
        let mut all_new_blocks = Vec::new();
        let mut all_new_transactions = Vec::new();
        let mut all_new_addresses = std::collections::HashSet::new();
        let mut receipt_idx = 0;
        
        // Process all blocks and transactions into bulk inserts
        for block in &blocks {
            let new_block = block_to_new_block(block);
            all_new_blocks.push(new_block);
            
            for (tx_index, tx) in block.transactions.iter().enumerate() {
                let receipt = receipts.get(receipt_idx).and_then(|r| r.as_ref());
                let block_number = block.number.unwrap().as_u64() as i64;
                let block_hash = format!("0x{:x}", block.hash.unwrap());
                let new_tx = transaction_to_new_transaction(
                    tx, 
                    block_number, 
                    &block_hash, 
                    tx_index as i32, 
                    receipt
                );
                
                // Collect addresses for analytics
                all_new_addresses.insert(new_tx.from_address.clone());
                if let Some(ref to) = new_tx.to_address {
                    all_new_addresses.insert(to.clone());
                }
                
                all_new_transactions.push(new_tx);
                receipt_idx += 1;
            }
        }
        
        info!("📊 Prepared {} blocks, {} transactions, {} addresses", 
              all_new_blocks.len(), all_new_transactions.len(), all_new_addresses.len());
        
        // BULK INSERT EVERYTHING AT ONCE
        let store_start = Instant::now();
        
        // Store blocks in bulk
        let blocks_inserted = self.block_repo.insert_blocks_bulk(all_new_blocks)?;
        info!("✅ Bulk inserted {} blocks", blocks_inserted);
        
        // Store transactions in chunks to avoid PostgreSQL parameter limit (65535)
        // Each transaction has ~20 parameters, so we can do ~3000 transactions per chunk
        let mut total_txs_inserted = 0;
        for tx_chunk in all_new_transactions.chunks(3000) {
            let chunk_inserted = self.block_repo.insert_transactions_bulk(tx_chunk.to_vec())?;
            total_txs_inserted += chunk_inserted;
        }
        
        // Skip analytics processing in mega batch mode for maximum speed
        
        let store_duration = store_start.elapsed();
        info!("💾 BULK STORAGE COMPLETE in {:.2}s", store_duration.as_secs_f64());
        
        // Skip WebSocket broadcasting in mega batch mode for maximum speed
        info!("📡 Skipping WebSocket broadcast for ultra-speed");
        
        Ok(blocks.len() as u64)
    }
    
    async fn get_latest_block(&self) -> Result<u64> {
        for node in &self.nodes {
            if let Ok(block_num) = node.get_latest_block_number().await {
                return Ok(block_num);
            }
        }
        Err(anyhow::anyhow!("All nodes failed"))
    }
    
    /// Get the best performing node URL for WebSocket connection
    fn get_best_node_url(&self) -> Option<String> {
        // For now, use the first node (rpc.mainnet.taiko.xyz has confirmed WebSocket support)
        // TODO: Track node performance and select the fastest
        self.node_urls.first().map(|url| {
            // Convert HTTP URL to WebSocket URL
            url.replace("https://", "wss://").replace("http://", "ws://")
        })
    }
    
    /// Run in live mode with WebSocket subscription when caught up
    /// OPTIMIZED: Always store latest block height immediately, process transactions in background
    async fn run_live_mode(&self) -> Result<()> {
        info!("🔴 LIVE MODE: Initializing OPTIMIZED WebSocket subscription for real-time blocks");
        info!("🚀 OPTIMIZATION: Instant block height storage + parallel transaction processing");
        
        // Select the best performing node
        let ws_url = self.get_best_node_url()
            .ok_or_else(|| anyhow::anyhow!("No nodes available for WebSocket"))?;
        
        info!("📡 Connecting to WebSocket: {}", ws_url);
        
        // Connect to WebSocket
        let (ws_stream, _) = match connect_async(&ws_url).await {
            Ok(conn) => conn,
            Err(e) => {
                error!("Failed to connect WebSocket: {}", e);
                return Err(anyhow::anyhow!("WebSocket connection failed"));
            }
        };
        
        let (mut write, mut read) = ws_stream.split();
        
        // Subscribe to newHeads
        let subscribe_msg = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_subscribe",
            "params": ["newHeads"]
        });
        
        write.send(Message::Text(subscribe_msg.to_string().into())).await?;
        info!("📨 Sent subscription request for newHeads");
        
        // Track processing
        let mut consecutive_errors = 0;
        let mut last_check = Instant::now();
        
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    // Parse the subscription message
                    if let Ok(json_msg) = serde_json::from_str::<serde_json::Value>(&text) {
                        // Check if it's a subscription notification
                        if let Some(params) = json_msg.get("params") {
                            if let Some(result) = params.get("result") {
                                if let Some(block_number_hex) = result.get("number") {
                                    if let Some(hex_str) = block_number_hex.as_str() {
                                        // Convert hex to decimal
                                        let block_num = u64::from_str_radix(&hex_str[2..], 16)?;
                                        info!("🔴 LIVE: New block #{} received via WebSocket", block_num);
                                        
                                        // OPTIMIZATION: Process block with instant height storage
                                        match self.process_live_block_optimized(block_num, &result).await {
                                            Ok(_) => {
                                                info!("⚡ LIVE: Block #{} height stored instantly", block_num);
                                                consecutive_errors = 0;
                                            }
                                            Err(e) => {
                                                error!("Failed to process live block: {}", e);
                                                consecutive_errors += 1;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    info!("WebSocket closed by server");
                    break;
                }
                Err(e) => {
                    error!("WebSocket error: {}", e);
                    consecutive_errors += 1;
                }
                _ => {}
            }
            
            // Periodically check if we've fallen behind (much more lenient now)
            if last_check.elapsed() > Duration::from_secs(60) {
                last_check = Instant::now();
                let latest_db = self.block_repo.get_latest_block_number()?.unwrap_or(0);
                let latest_chain = self.get_latest_block().await?;
                let behind = latest_chain.saturating_sub(latest_db as u64);
                
                // Much higher threshold since we store height immediately
                if behind > 50 {
                    info!("📉 Fallen behind by {} blocks (height only), switching back to batch mode", behind);
                    return Ok(());
                }
                
                info!("📊 LIVE STATUS: DB height: {}, Chain: {}, Behind: {} blocks", latest_db, latest_chain, behind);
            }
            
            // If too many errors, fall back to batch mode
            if consecutive_errors > 10 {
                error!("Too many consecutive errors in live mode, falling back");
                return Err(anyhow::anyhow!("Too many errors in live mode"));
            }
        }
        
        Ok(())
    }
    
    /// OPTIMIZED: Process live block with instant height storage + parallel transaction processing
    /// This ensures we NEVER fall behind on block heights, even if transaction processing is slow
    async fn process_live_block_optimized(&self, block_num: u64, block_header: &serde_json::Value) -> Result<()> {
        let start_time = Instant::now();
        
        // STEP 1: INSTANTLY store the block height/header (no transactions yet)
        // This takes ~1ms and ensures we never fall behind on block heights
        let block_hash = block_header.get("hash")
            .and_then(|h| h.as_str())
            .unwrap_or("0x0");
        let timestamp = block_header.get("timestamp")
            .and_then(|t| t.as_str())
            .and_then(|t| u64::from_str_radix(&t[2..], 16).ok())
            .unwrap_or(0);
        
        // Create minimal block record for instant storage
        let minimal_block = crate::models::block::NewBlock {
            number: block_num as i64,
            hash: block_hash.to_string(),
            parent_hash: block_header.get("parentHash")
                .and_then(|h| h.as_str())
                .unwrap_or("0x0").to_string(),
            timestamp: timestamp as i64,
            gas_used: block_header.get("gasUsed")
                .and_then(|g| g.as_str())
                .and_then(|g| u64::from_str_radix(&g[2..], 16).ok())
                .unwrap_or(0) as i64,
            gas_limit: block_header.get("gasLimit")
                .and_then(|g| g.as_str())
                .and_then(|g| u64::from_str_radix(&g[2..], 16).ok())
                .unwrap_or(0) as i64,
            miner: block_header.get("miner")
                .and_then(|m| m.as_str())
                .unwrap_or("0x0").to_string(),
            transaction_count: 0, // Will be updated later
            size: block_header.get("size")
                .and_then(|s| s.as_str())
                .and_then(|s| u64::from_str_radix(&s[2..], 16).ok())
                .map(|v| v as i64),
            difficulty: bigdecimal::BigDecimal::from(0), // Taiko doesn't use difficulty
            total_difficulty: Some(bigdecimal::BigDecimal::from(0)),
            nonce: Some("0".to_string()),
            extra_data: block_header.get("extraData")
                .and_then(|d| d.as_str())
                .map(|d| d.to_string()),
            logs_bloom: None, // Not available in WebSocket header
            mix_hash: None,   // Not available in WebSocket header
            base_fee_per_gas: block_header.get("baseFeePerGas")
                .and_then(|f| f.as_str())
                .and_then(|f| u64::from_str_radix(&f[2..], 16).ok())
                .map(|v| v as i64),
        };
        
        // INSTANT STORAGE: Store block height immediately (takes ~1-2ms)
        let insert_start = Instant::now();
        match self.block_repo.insert_block(minimal_block) {
            Ok(_) => {
                let insert_duration = insert_start.elapsed();
                info!("⚡ INSTANT: Block #{} height stored in {:.2}ms", block_num, insert_duration.as_secs_f64() * 1000.0);
                
                // Immediately broadcast that we have a new block height with miner info
                let miner = block_header.get("miner")
                    .and_then(|m| m.as_str())
                    .unwrap_or("0x0").to_string();
                self.websocket_broadcaster.broadcast_new_block_height(block_num, miner, timestamp as i64).await;
            }
            Err(e) => {
                error!("Failed to instantly store block #{}: {}", block_num, e);
                return Err(e.into());
            }
        }
        
        // STEP 2: SPAWN PARALLEL TASK for transaction processing
        // This runs in background and won't block the next block
        let block_repo = self.block_repo.clone();
        let analytics_repo = self.analytics_repo.clone();
        let websocket_broadcaster = self.websocket_broadcaster.clone();
        let bridge_detector = self.bridge_detector.clone();
        let nodes = self.nodes.clone();
        
        tokio::spawn(async move {
            let tx_start = Instant::now();
            info!("🔄 PARALLEL: Starting transaction processing for block #{}", block_num);
            
            // Fetch full block with transactions
            let block_with_txs = match Self::fetch_full_block(&nodes, block_num).await {
                Ok(block) => block,
                Err(e) => {
                    error!("Failed to fetch full block #{}: {}", block_num, e);
                    return;
                }
            };
            
            // Extract transaction hashes
            let tx_hashes: Vec<_> = block_with_txs.transactions.iter().map(|tx| tx.hash).collect();
            
            if !tx_hashes.is_empty() {
                // Fetch receipts for all transactions
                match Self::fetch_receipts_for_block(&nodes, &tx_hashes).await {
                    Ok(receipts) => {
                        // Process and store all transactions
                        match Self::store_block_transactions(&block_repo, &analytics_repo, &bridge_detector, 
                                                           &websocket_broadcaster, &block_with_txs, &receipts).await {
                            Ok(tx_count) => {
                                let tx_duration = tx_start.elapsed();
                                info!("✅ PARALLEL: Block #{} - {} transactions processed in {:.2}s", 
                                      block_num, tx_count, tx_duration.as_secs_f64());
                                
                                // TODO: Update block with transaction count (implement later)
                                info!("✅ Block #{} has {} transactions (transaction count update skipped for now)", block_num, tx_count);
                                
                                // Broadcast complete block with transactions
                                if let Ok(Some(complete_block)) = block_repo.get_block_by_number(block_num as i64) {
                                    websocket_broadcaster.broadcast_live_block_complete(complete_block).await;
                                } else {
                                    warn!("Could not fetch complete block #{} for WebSocket broadcast", block_num);
                                }
                                
                                // Broadcast updated stats
                                Self::broadcast_updated_stats(&block_repo, &websocket_broadcaster).await;
                            }
                            Err(e) => {
                                error!("Failed to store transactions for block #{}: {}", block_num, e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to fetch receipts for block #{}: {}", block_num, e);
                    }
                }
            } else {
                info!("📭 Block #{} has no transactions", block_num);
                if let Ok(Some(complete_block)) = block_repo.get_block_by_number(block_num as i64) {
                    websocket_broadcaster.broadcast_live_block_complete(complete_block).await;
                } else {
                    warn!("Could not fetch complete block #{} for WebSocket broadcast", block_num);
                }
                
                // Broadcast updated stats
                Self::broadcast_updated_stats(&block_repo, &websocket_broadcaster).await;
            }
        });
        
        let total_duration = start_time.elapsed();
        info!("⚡ LIVE OPTIMIZED: Block #{} height stored + parallel TX processing started in {:.2}ms", 
              block_num, total_duration.as_secs_f64() * 1000.0);
        
        Ok(())
    }
    
    /// Helper: Fetch full block with transactions from any available node
    async fn fetch_full_block(nodes: &[Arc<TaikoRpcClient>], block_num: u64) -> Result<ethers::types::Block<ethers::types::Transaction>> {
        for node in nodes {
            if let Ok(Some(block)) = node.get_block_by_number(block_num).await {
                return Ok(block);
            }
        }
        Err(anyhow::anyhow!("Failed to fetch block #{} from any node", block_num))
    }
    
    /// Helper: Fetch receipts for a block's transactions
    async fn fetch_receipts_for_block(
        nodes: &[Arc<TaikoRpcClient>], 
        tx_hashes: &[ethers::types::H256]
    ) -> Result<Vec<Option<ethers::types::TransactionReceipt>>> {
        // Use first node for simplicity - could be optimized to use batch client
        if let Some(node) = nodes.first() {
            let mut receipts = Vec::with_capacity(tx_hashes.len());
            
            for &tx_hash in tx_hashes {
                match node.get_transaction_receipt(tx_hash).await {
                    Ok(receipt) => receipts.push(receipt),
                    Err(_) => receipts.push(None),
                }
            }
            
            Ok(receipts)
        } else {
            Err(anyhow::anyhow!("No nodes available for receipt fetching"))
        }
    }
    
    /// Helper: Store block transactions and update analytics
    async fn store_block_transactions(
        block_repo: &Arc<BlockRepository>,
        analytics_repo: &Arc<AddressAnalyticsRepository>,
        bridge_detector: &Arc<BridgeDetector>,
        websocket_broadcaster: &Arc<WebSocketBroadcaster>,
        block: &ethers::types::Block<ethers::types::Transaction>,
        receipts: &[Option<ethers::types::TransactionReceipt>],
    ) -> Result<usize> {
        let mut new_transactions = Vec::new();
        let mut new_addresses = std::collections::HashSet::new();
        
        for (tx_index, tx) in block.transactions.iter().enumerate() {
            let receipt = receipts.get(tx_index).and_then(|r| r.as_ref());
            let block_number = block.number.unwrap().as_u64() as i64;
            let block_hash = format!("0x{:x}", block.hash.unwrap());
            
            let new_tx = transaction_to_new_transaction(
                tx, 
                block_number, 
                &block_hash, 
                tx_index as i32, 
                receipt
            );
            
            // Collect addresses for analytics
            new_addresses.insert(new_tx.from_address.clone());
            if let Some(ref to) = new_tx.to_address {
                new_addresses.insert(to.clone());
            }
            
            new_transactions.push(new_tx);
        }
        
        // Store transactions in bulk and broadcast them
        let tx_count = if !new_transactions.is_empty() {
            let inserted_count = block_repo.insert_transactions_bulk(new_transactions.clone())?;
            
            // ✅ BROADCAST EACH NEW TRANSACTION to WebSocket clients for LIVE updates
            info!("📡 Broadcasting {} new transactions to WebSocket clients", new_transactions.len());
            for new_tx in &new_transactions {
                // Fetch the complete stored transaction for broadcasting
                if let Ok(Some(stored_tx)) = block_repo.get_transaction_by_hash(&new_tx.hash) {
                    websocket_broadcaster.broadcast_new_transaction(stored_tx).await;
                } else {
                    warn!("Could not fetch stored transaction {} for WebSocket broadcast", new_tx.hash);
                }
            }
            
            inserted_count
        } else {
            0
        };
        
        // TODO: Process analytics for new addresses (implement later)
        if !new_addresses.is_empty() {
            info!("📊 ANALYTICS: Skipping analytics processing for {} addresses (implement later)", new_addresses.len());
        }
        
        Ok(tx_count)
    }
    
    
    /// Helper: Broadcast updated stats (total blocks, transactions, etc.)
    async fn broadcast_updated_stats(
        block_repo: &Arc<BlockRepository>,
        websocket_broadcaster: &Arc<WebSocketBroadcaster>
    ) {
        // Get updated stats
        if let (Ok(total_blocks), Ok(latest_block), Ok(total_transactions)) = (
            block_repo.get_block_count(),
            block_repo.get_latest_block_number(),
            block_repo.get_transaction_count()
        ) {
            websocket_broadcaster.broadcast_stats(
                total_blocks,
                latest_block,
                total_transactions,
                0 // TODO: Add actual address count later
            ).await;
        }
    }
    
    /// Broadcast a newly processed block to WebSocket clients for real-time updates
    async fn broadcast_live_block(&self, block_number: u64) {
        // Fetch the block from database to get complete details
        match self.block_repo.get_block_by_number(block_number as i64) {
            Ok(Some(block)) => {
                info!("📡 Broadcasting live block #{} to WebSocket clients", block_number);
                self.websocket_broadcaster.broadcast_new_block(block.clone()).await;
                
                // Also broadcast all transactions in this block
                match self.block_repo.get_transactions_by_block(block_number as i64) {
                    Ok(transactions) => {
                        for tx in transactions {
                            self.websocket_broadcaster.broadcast_new_transaction(tx).await;
                        }
                        info!("📡 Broadcasted {} transactions from block #{}", block.transaction_count, block_number);
                    }
                    Err(e) => {
                        warn!("Failed to fetch transactions for block #{}: {}", block_number, e);
                    }
                }
                
                // Also broadcast updated stats
                if let Ok(latest_block) = self.block_repo.get_latest_block_number() {
                    if let Ok(total_blocks) = self.block_repo.get_block_count() {
                        if let Ok(total_transactions) = self.block_repo.get_transaction_count() {
                            // For now, set total_addresses to 0 since we don't have that method
                            self.websocket_broadcaster.broadcast_stats(
                                total_blocks, 
                                latest_block, 
                                total_transactions,
                                0
                            ).await;
                        }
                    }
                }
            }
            Ok(None) => {
                warn!("Block #{} not found in database for WebSocket broadcast", block_number);
            }
            Err(e) => {
                error!("Failed to fetch block #{} for WebSocket broadcast: {}", block_number, e);
            }
        }
    }
}