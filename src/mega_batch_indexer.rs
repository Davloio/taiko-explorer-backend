use anyhow::Result;
use std::time::{Duration, Instant};
use tracing::{error, info, warn};
use std::sync::Arc;
use futures::future::join_all;

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
        
        for node_url in working_nodes {
            info!("🔗 Initializing mega batch node: {}", node_url);
            
            let client = TaikoRpcClient::new(&node_url, 167000).await?;
            nodes.push(Arc::new(client));
            
            let batch_client = BatchRpcClient::new(node_url.clone());
            batch_clients.push(Arc::new(batch_client));
        }
        
        info!("✅ Initialized {} nodes for MEGA BATCHING", nodes.len());
        
        Ok(Self {
            nodes,
            batch_clients,
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
            
            if blocks_behind == 0 {
                info!("✅ Caught up! Waiting for new blocks...");
                tokio::time::sleep(Duration::from_secs(2)).await;
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
}