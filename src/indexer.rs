use anyhow::{anyhow, Result};
use ethers::types::H256;
use std::time::{Duration, Instant};
use tokio::time::{interval, sleep};
use tracing::{error, info, warn};
use futures::future::join_all;
use std::sync::Arc;
use std::collections::HashSet;
use chrono::Utc;

use crate::db::operations::BlockRepository;
use crate::db::address_analytics::AddressAnalyticsRepository;
use crate::rpc::client::TaikoRpcClient;
use crate::rpc::types::{block_to_new_block, transaction_to_new_transaction};
use crate::bridge::BridgeDetector;
use crate::analytics::analytics::process_block_analytics;
use crate::websocket::WebSocketBroadcaster;

pub struct BlockIndexer {
    rpc_client: Arc<TaikoRpcClient>,
    block_repo: Arc<BlockRepository>,
    analytics_repo: Arc<AddressAnalyticsRepository>,
    bridge_detector: Arc<BridgeDetector>,
    websocket_broadcaster: Arc<WebSocketBroadcaster>,
    batch_size: u64,
    concurrent_requests: usize,
}

impl BlockIndexer {
    pub fn new(
        rpc_client: TaikoRpcClient,
        block_repo: BlockRepository,
        analytics_repo: AddressAnalyticsRepository,
        websocket_broadcaster: Arc<WebSocketBroadcaster>,
        batch_size: u64,
    ) -> Self {
        Self {
            rpc_client: Arc::new(rpc_client),
            block_repo: Arc::new(block_repo),
            analytics_repo: Arc::new(analytics_repo),
            bridge_detector: Arc::new(BridgeDetector::new()),
            websocket_broadcaster,
            batch_size,
            concurrent_requests: 5, // Very conservative start
        }
    }

    pub async fn start_indexing(&self) -> Result<()> {
        info!("Starting block indexer...");
        
        self.catch_up_historical_blocks().await?;
        
        info!("Historical sync complete, starting real-time indexing...");
        self.start_real_time_indexing().await
    }

    async fn catch_up_historical_blocks(&self) -> Result<()> {
        // Start with a single request to test RPC health
        info!("🔍 Testing RPC connection...");
        match self.rpc_client.get_latest_block_number().await {
            Ok(latest_rpc_block) => {
                info!("✅ RPC connection successful. Latest block: {}", latest_rpc_block);
            },
            Err(e) => {
                error!("❌ Initial RPC test failed: {}", e);
                if e.to_string().contains("429") || e.to_string().contains("Too Many Requests") {
                    info!("⏳ Rate limited on first request. Waiting 10 seconds...");
                    sleep(Duration::from_secs(10)).await;
                } else {
                    return Err(e);
                }
            }
        }
        
        let latest_rpc_block = self.rpc_client.get_latest_block_number().await?;
        let latest_db_block = self.block_repo.get_latest_block_number()?;
        
        let start_block = match latest_db_block {
            Some(block_num) => (block_num + 1) as u64,
            None => 1, // Start from block 1 if database is empty
        };

        if start_block > latest_rpc_block {
            info!("Database is up to date");
            return Ok(());
        }

        let total_blocks = latest_rpc_block - start_block + 1;
        info!(
            "🚀 Fast syncing {} blocks from {} to {} with {} concurrent requests",
            total_blocks, start_block, latest_rpc_block, self.concurrent_requests
        );

        let start_time = Instant::now();
        let mut processed = 0u64;

        info!("🐌 Processing {} blocks sequentially (one by one) to avoid rate limits", total_blocks);
        
        // Process blocks one by one - slow but reliable
        for block_num in start_block..=latest_rpc_block {
            let block_start_time = Instant::now();
            
            match Self::fetch_and_store_block(
                Arc::clone(&self.rpc_client), 
                Arc::clone(&self.block_repo),
                Arc::clone(&self.analytics_repo),
                Arc::clone(&self.bridge_detector),
                Arc::clone(&self.websocket_broadcaster),
                block_num
            ).await {
                Ok(_) => {
                    processed += 1;
                    
                    // Progress report every 10 blocks
                    if processed % 10 == 0 {
                        let overall_blocks_per_sec = processed as f64 / start_time.elapsed().as_secs_f64();
                        let db_latest = self.block_repo.get_latest_block_number().unwrap_or(Some(-1)).unwrap_or(-1);
                        
                        info!(
                            "📊 Block #{} stored | Speed: {:.1} blocks/sec | DB Latest: {} | Progress: {:.2}%",
                            block_num,
                            overall_blocks_per_sec,
                            db_latest,
                            (processed as f64 / total_blocks as f64) * 100.0
                        );
                    }
                },
                Err(e) => {
                    error!("❌ Failed to process block {}: {}", block_num, e);
                    
                    // Wait longer on errors to avoid getting banned
                    sleep(Duration::from_secs(10)).await;
                }
            }
            
            // Small delay between requests to be respectful to RPC
            sleep(Duration::from_millis(100)).await; // ~10 blocks/sec max
        }

        let total_duration = start_time.elapsed();
        let avg_blocks_per_sec = processed as f64 / total_duration.as_secs_f64();
        
        info!(
            "✅ Sync complete! Processed {} blocks in {:.2}s ({:.0} blocks/sec average)",
            processed,
            total_duration.as_secs_f64(),
            avg_blocks_per_sec
        );

        Ok(())
    }

    async fn fetch_and_store_block(
        rpc_client: Arc<TaikoRpcClient>,
        block_repo: Arc<BlockRepository>,
        analytics_repo: Arc<AddressAnalyticsRepository>,
        bridge_detector: Arc<BridgeDetector>,
        websocket_broadcaster: Arc<WebSocketBroadcaster>,
        block_number: u64,
    ) -> Result<()> {
        // Check if block already exists
        if block_repo.block_exists(block_number as i64)? {
            return Ok(());
        }

        // Retry logic for rate limiting
        let mut retries = 0;
        const MAX_RETRIES: u32 = 3;
        
        loop {
            match rpc_client.get_block_by_number(block_number).await {
                Ok(Some(block)) => {
                    // Convert and store block
                    let new_block = block_to_new_block(&block);
                    let block_hash = format!("0x{:x}", block.hash.unwrap());
                    
                    match block_repo.insert_block(new_block) {
                        Ok(_) => {
                            let mut receipts_for_analytics = Vec::new();
                            
                            // Block stored successfully, now fetch receipts and store transactions
                            if !block.transactions.is_empty() {
                                info!("📄 Fetching {} transaction receipts for block {}", block.transactions.len(), block_number);
                                
                                let tx_hashes: Vec<H256> = block.transactions.iter().map(|tx| tx.hash).collect();
                                
                                match rpc_client.get_transaction_receipts_batch(tx_hashes).await {
                                    Ok(receipts) => {
                                        let mut new_transactions = Vec::new();
                                        
                                        for (index, tx) in block.transactions.iter().enumerate() {
                                            let receipt = receipts.get(index).and_then(|r| r.as_ref());
                                            let new_tx = transaction_to_new_transaction(
                                                tx, 
                                                block_number as i64, 
                                                &block_hash, 
                                                index as i32,
                                                receipt
                                            );
                                            new_transactions.push(new_tx);
                                        }
                                        
                                        // Store receipts for analytics
                                        receipts_for_analytics = receipts;
                                        
                                        match block_repo.insert_transactions_batch(new_transactions.clone()) {
                                            Ok(inserted_count) => {
                                                info!("✅ Block {} with {} transactions (with receipts) processed", block_number, inserted_count);
                                                
                                                // 🌐 Broadcast each new transaction via WebSocket
                                                for new_tx in &new_transactions {
                                                    if let Ok(Some(db_tx)) = block_repo.get_transaction_by_hash(&new_tx.hash) {
                                                        websocket_broadcaster.broadcast_new_transaction(db_tx).await;
                                                    }
                                                }
                                            },
                                            Err(e) => {
                                                warn!("⚠️ Block {} stored but transaction insertion failed: {}", block_number, e);
                                            }
                                        }
                                    },
                                    Err(e) => {
                                        warn!("🔄 Failed to fetch receipts for block {}, storing transactions without receipt data: {}", block_number, e);
                                        
                                        // Fallback: store transactions without receipt data
                                        let mut new_transactions = Vec::new();
                                        for (index, tx) in block.transactions.iter().enumerate() {
                                            let new_tx = transaction_to_new_transaction(
                                                tx, 
                                                block_number as i64, 
                                                &block_hash, 
                                                index as i32,
                                                None
                                            );
                                            new_transactions.push(new_tx);
                                        }
                                        
                                        // Use empty receipts for analytics
                                        receipts_for_analytics = vec![None; block.transactions.len()];
                                        
                                        if let Ok(_) = block_repo.insert_transactions_batch(new_transactions.clone()) {
                                            // 🌐 Broadcast each new transaction via WebSocket (fallback case)
                                            for new_tx in &new_transactions {
                                                if let Ok(Some(db_tx)) = block_repo.get_transaction_by_hash(&new_tx.hash) {
                                                    websocket_broadcaster.broadcast_new_transaction(db_tx).await;
                                                }
                                            }
                                        }
                                    }
                                }
                                
                                // Process analytics after storing transactions
                                let block_timestamp = block.timestamp.as_u64() as i64;
                                if let Err(e) = process_block_analytics(
                                    &analytics_repo,
                                    &bridge_detector,
                                    block_number as i64,
                                    block_timestamp,
                                    &block.transactions,
                                    &receipts_for_analytics,
                                ).await {
                                    warn!("Failed to process analytics for block {}: {}", block_number, e);
                                }
                            }
                            
                            // 🌐 Broadcast new block via WebSocket
                            if let Ok(Some(db_block)) = block_repo.get_block_by_number(block_number as i64) {
                                websocket_broadcaster.broadcast_new_block(db_block).await;
                            }
                            
                            // 🌐 Broadcast updated stats via WebSocket
                            if let Ok(total_blocks) = block_repo.get_block_count() {
                                if let Ok(latest_block) = block_repo.get_latest_block_number() {
                                    if let Ok(total_transactions) = block_repo.get_transaction_count() {
                                        if let Ok(total_addresses) = block_repo.get_unique_address_count() {
                                            websocket_broadcaster.broadcast_stats(total_blocks, latest_block, total_transactions, total_addresses).await;
                                        }
                                    }
                                }
                            }
                            
                            return Ok(());
                        },
                        Err(e) => {
                            let error_msg = e.to_string();
                            if error_msg.contains("duplicate key") {
                                return Ok(()); // Block already exists, that's fine
                            } else if error_msg.contains("connection") || error_msg.contains("timeout") {
                                error!("Database connection error for block {}: {}", block_number, e);
                                sleep(Duration::from_secs(1)).await;
                                // Don't retry here, let it fail and be retried at chunk level
                                return Err(e);
                            } else {
                                error!("Database insertion error for block {}: {}", block_number, e);
                                return Err(e);
                            }
                        }
                    }
                }
                Ok(None) => {
                    return Err(anyhow!("Block {} not found", block_number));
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    if error_msg.contains("429") || error_msg.contains("Too Many Requests") {
                        retries += 1;
                        if retries > MAX_RETRIES {
                            return Err(anyhow!("Max retries exceeded for block {}: {}", block_number, e));
                        }
                        
                        // Exponential backoff: 1s, 2s, 4s
                        let delay = Duration::from_secs(2_u64.pow(retries - 1));
                        sleep(delay).await;
                        continue;
                    } else {
                        return Err(e);
                    }
                }
            }
        }
    }

    async fn start_real_time_indexing(&self) -> Result<()> {
        let mut interval = interval(Duration::from_secs(12)); // Taiko block time

        loop {
            interval.tick().await;
            
            match self.index_latest_blocks().await {
                Ok(_) => {}
                Err(e) => {
                    error!("Real-time indexing error: {}", e);
                    sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }

    async fn index_latest_blocks(&self) -> Result<()> {
        let latest_rpc_block = self.rpc_client.get_latest_block_number().await?;
        let latest_db_block = self.block_repo.get_latest_block_number()?;
        
        let start_block = match latest_db_block {
            Some(block_num) => (block_num + 1) as u64,
            None => latest_rpc_block,
        };

        for block_num in start_block..=latest_rpc_block {
            if !self.block_repo.block_exists(block_num as i64)? {
                self.index_single_block(block_num).await?;
                info!("Indexed new block #{}", block_num);
            }
        }

        Ok(())
    }

    async fn index_single_block(&self, block_number: u64) -> Result<()> {
        let block = self
            .rpc_client
            .get_block_by_number(block_number)
            .await?
            .ok_or_else(|| anyhow!("Block {} not found", block_number))?;

        let new_block = block_to_new_block(&block);
        let block_hash = format!("0x{:x}", block.hash.unwrap());
        
        match self.block_repo.insert_block(new_block) {
            Ok(_) => {
                // Store transactions with receipts
                if !block.transactions.is_empty() {
                    let tx_hashes: Vec<H256> = block.transactions.iter().map(|tx| tx.hash).collect();
                    
                    match self.rpc_client.get_transaction_receipts_batch(tx_hashes).await {
                        Ok(receipts) => {
                            let mut new_transactions = Vec::new();
                            
                            for (index, tx) in block.transactions.iter().enumerate() {
                                let receipt = receipts.get(index).and_then(|r| r.as_ref());
                                let new_tx = transaction_to_new_transaction(
                                    tx, 
                                    block_number as i64, 
                                    &block_hash, 
                                    index as i32,
                                    receipt
                                );
                                new_transactions.push(new_tx);
                            }
                            
                            if let Ok(_) = self.block_repo.insert_transactions_batch(new_transactions.clone()) {
                                // 🌐 Broadcast each new transaction via WebSocket (real-time)
                                for new_tx in &new_transactions {
                                    if let Ok(Some(db_tx)) = self.block_repo.get_transaction_by_hash(&new_tx.hash) {
                                        self.websocket_broadcaster.broadcast_new_transaction(db_tx.clone()).await;
                                        
                                        // 🏠 Broadcast address activity
                                        self.websocket_broadcaster.broadcast_address_activity(
                                            db_tx.from_address.clone(),
                                            db_tx.hash.clone(),
                                            "sent".to_string()
                                        ).await;
                                        
                                        if let Some(to_addr) = &db_tx.to_address {
                                            self.websocket_broadcaster.broadcast_address_activity(
                                                to_addr.clone(),
                                                db_tx.hash.clone(),
                                                "received".to_string()
                                            ).await;
                                        }
                                    }
                                }
                                
                                // 🆕 Check for new addresses and broadcast
                                self.check_and_broadcast_new_addresses(&new_transactions).await;
                            }
                            
                            // Process analytics after storing transactions  
                            let block_timestamp = block.timestamp.as_u64() as i64;
                            if let Err(e) = process_block_analytics(
                                &self.analytics_repo,
                                &self.bridge_detector,
                                block_number as i64,
                                block_timestamp,
                                &block.transactions,
                                &receipts,
                            ).await {
                                warn!("Failed to process analytics for block {}: {}", block_number, e);
                            }
                        },
                        Err(_) => {
                            // Fallback without receipts
                            let mut new_transactions = Vec::new();
                            for (index, tx) in block.transactions.iter().enumerate() {
                                let new_tx = transaction_to_new_transaction(
                                    tx, 
                                    block_number as i64, 
                                    &block_hash, 
                                    index as i32,
                                    None
                                );
                                new_transactions.push(new_tx);
                            }
                            if let Ok(_) = self.block_repo.insert_transactions_batch(new_transactions.clone()) {
                                // 🌐 Broadcast each new transaction via WebSocket (real-time fallback)
                                for new_tx in &new_transactions {
                                    if let Ok(Some(db_tx)) = self.block_repo.get_transaction_by_hash(&new_tx.hash) {
                                        self.websocket_broadcaster.broadcast_new_transaction(db_tx.clone()).await;
                                        
                                        // 🏠 Broadcast address activity
                                        self.websocket_broadcaster.broadcast_address_activity(
                                            db_tx.from_address.clone(),
                                            db_tx.hash.clone(),
                                            "sent".to_string()
                                        ).await;
                                        
                                        if let Some(to_addr) = &db_tx.to_address {
                                            self.websocket_broadcaster.broadcast_address_activity(
                                                to_addr.clone(),
                                                db_tx.hash.clone(),
                                                "received".to_string()
                                            ).await;
                                        }
                                    }
                                }
                                
                                // 🆕 Check for new addresses and broadcast (fallback)
                                self.check_and_broadcast_new_addresses(&new_transactions).await;
                            }
                            
                            // Process analytics even without receipts
                            let block_timestamp = block.timestamp.as_u64() as i64;
                            let empty_receipts: Vec<Option<ethers::types::TransactionReceipt>> = vec![None; block.transactions.len()];
                            if let Err(e) = process_block_analytics(
                                &self.analytics_repo,
                                &self.bridge_detector,
                                block_number as i64,
                                block_timestamp,
                                &block.transactions,
                                &empty_receipts,
                            ).await {
                                warn!("Failed to process analytics for block {}: {}", block_number, e);
                            }
                        }
                    }
                }
                
                // 🌐 Broadcast new block via WebSocket (real-time indexing)
                if let Ok(Some(db_block)) = self.block_repo.get_block_by_number(block_number as i64) {
                    self.websocket_broadcaster.broadcast_new_block(db_block).await;
                }
                
                // 🌐 Broadcast updated stats via WebSocket (real-time indexing)
                if let Ok(total_blocks) = self.block_repo.get_block_count() {
                    if let Ok(latest_block) = self.block_repo.get_latest_block_number() {
                        if let Ok(total_transactions) = self.block_repo.get_transaction_count() {
                            if let Ok(total_addresses) = self.block_repo.get_unique_address_count() {
                                self.websocket_broadcaster.broadcast_stats(total_blocks, latest_block, total_transactions, total_addresses).await;
                            }
                        }
                    }
                }
                
                Ok(())
            },
            Err(e) => {
                if e.to_string().contains("duplicate key") {
                    warn!("Block #{} already exists, skipping", block_number);
                    Ok(())
                } else {
                    Err(e)
                }
            }
        }
    }

    pub async fn get_indexer_status(&self) -> Result<IndexerStatus> {
        let latest_rpc_block = self.rpc_client.get_latest_block_number().await?;
        let latest_db_block = self.block_repo.get_latest_block_number()?;
        let total_blocks = self.block_repo.get_block_count()?;

        Ok(IndexerStatus {
            latest_rpc_block,
            latest_db_block,
            total_blocks,
            is_synced: latest_db_block.map_or(false, |db| db as u64 >= latest_rpc_block),
        })
    }

    /// Check for new addresses in transactions and broadcast new address events
    async fn check_and_broadcast_new_addresses(&self, new_transactions: &[crate::models::transaction::NewTransaction]) {
        let mut addresses_in_block = HashSet::new();
        
        // Collect all unique addresses from this block's transactions
        for tx in new_transactions {
            addresses_in_block.insert(&tx.from_address);
            if let Some(to_addr) = &tx.to_address {
                addresses_in_block.insert(to_addr);
            }
        }
        
        // Check if each address is new (first time seen)
        for address in addresses_in_block {
            if let Ok(count) = self.block_repo.get_transactions_count_by_address(address) {
                // If this address has only 1 transaction, it's a new address
                if count == 1 {
                    self.websocket_broadcaster.broadcast_new_address(address.clone()).await;
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct IndexerStatus {
    pub latest_rpc_block: u64,
    pub latest_db_block: Option<i64>,
    pub total_blocks: i64,
    pub is_synced: bool,
}