use anyhow::{anyhow, Result};
use ethers::prelude::*;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
struct NodeHealth {
    url: String,
    last_success: Option<Instant>,
    consecutive_failures: u32,
    avg_response_time: Option<Duration>,
    is_healthy: bool,
}

impl NodeHealth {
    fn new(url: String) -> Self {
        Self {
            url,
            last_success: None,
            consecutive_failures: 0,
            avg_response_time: None,
            is_healthy: true,
        }
    }
    
    fn record_success(&mut self, response_time: Duration) {
        self.last_success = Some(Instant::now());
        self.consecutive_failures = 0;
        self.is_healthy = true;
        
        self.avg_response_time = Some(match self.avg_response_time {
            Some(avg) => Duration::from_millis((avg.as_millis() as f64 * 0.7 + response_time.as_millis() as f64 * 0.3) as u64),
            None => response_time,
        });
    }
    
    fn record_failure(&mut self) {
        self.consecutive_failures += 1;
        
        if self.consecutive_failures >= 3 {
            self.is_healthy = false;
        }
    }
    
    fn priority_score(&self) -> f64 {
        if !self.is_healthy {
            return 0.0;
        }
        
        let base_score = 100.0;
        
        let failure_penalty = self.consecutive_failures as f64 * 20.0;
        
        let recency_bonus = match self.last_success {
            Some(last) => {
                let elapsed = last.elapsed().as_secs() as f64;
                if elapsed < 60.0 { 20.0 } else if elapsed < 300.0 { 10.0 } else { 0.0 }
            },
            None => -10.0,
        };
        
        let speed_bonus = match self.avg_response_time {
            Some(rt) if rt < Duration::from_millis(50) => 15.0,
            Some(rt) if rt < Duration::from_millis(100) => 10.0,
            Some(rt) if rt < Duration::from_millis(200) => 5.0,
            _ => 0.0,
        };
        
        (base_score - failure_penalty + recency_bonus + speed_bonus).max(0.0)
    }
}

#[derive(Clone)]
pub struct TaikoRpcClient {
    nodes: Arc<RwLock<Vec<NodeHealth>>>,
    current_provider: Arc<RwLock<Option<Arc<Provider<Http>>>>>,
    current_node_index: Arc<RwLock<usize>>,
    chain_id: u64,
}


impl TaikoRpcClient {
    pub async fn new_with_rotation(rpc_urls: Vec<String>, chain_id: u64) -> Result<Self> {
        let nodes: Vec<NodeHealth> = rpc_urls.iter()
            .map(|url| NodeHealth::new(url.clone()))
            .collect();
            
        let client = Self {
            nodes: Arc::new(RwLock::new(nodes)),
            current_provider: Arc::new(RwLock::new(None)),
            current_node_index: Arc::new(RwLock::new(0)),
            chain_id,
        };
        
        client.connect_to_best_node().await?;
        Ok(client)
    }
    
    pub async fn new(rpc_url: &str, chain_id: u64) -> Result<Self> {
        let provider = Provider::<Http>::try_from(rpc_url)?;
        let provider = Arc::new(provider);
        
        let nodes = vec![NodeHealth::new(rpc_url.to_string())];
        
        let client = Self {
            nodes: Arc::new(RwLock::new(nodes)),
            current_provider: Arc::new(RwLock::new(Some(provider))),
            current_node_index: Arc::new(RwLock::new(0)),
            chain_id,
        };
        
        client.validate_current_connection().await?;
        Ok(client)
    }
    
    async fn connect_to_best_node(&self) -> Result<()> {
        let nodes = self.nodes.read().await;
        
        let mut best_index = 0;
        let mut best_score = 0.0;
        
        for (i, node) in nodes.iter().enumerate() {
            let score = node.priority_score();
            if score > best_score {
                best_score = score;
                best_index = i;
            }
        }
        
        drop(nodes);
        
        if best_score == 0.0 {
            warn!("No healthy nodes found, testing all nodes...");
            return self.test_and_connect_first_available().await;
        }
        
        self.connect_to_node(best_index).await
    }
    
    async fn test_and_connect_first_available(&self) -> Result<()> {
        let nodes = self.nodes.read().await;
        let node_count = nodes.len();
        drop(nodes);
        
        for i in 0..node_count {
            match self.test_node_connection(i).await {
                Ok(_) => {
                    info!("Successfully connected to node {}", i);
                    return self.connect_to_node(i).await;
                },
                Err(e) => {
                    warn!("Node {} failed connection test: {}", i, e);
                }
            }
        }
        
        Err(anyhow!("No RPC nodes are available"))
    }
    
    async fn test_node_connection(&self, node_index: usize) -> Result<()> {
        let nodes = self.nodes.read().await;
        let node_url = nodes.get(node_index)
            .ok_or_else(|| anyhow!("Invalid node index: {}", node_index))?
            .url.clone();
        drop(nodes);
        
        let start_time = Instant::now();
        let provider = Provider::<Http>::try_from(node_url.as_str())?;
        
        let chain_id = provider.get_chainid().await?;
        let response_time = start_time.elapsed();
        
        if chain_id.as_u64() != self.chain_id {
            return Err(anyhow!(
                "Chain ID mismatch: expected {}, got {}",
                self.chain_id,
                chain_id.as_u64()
            ));
        }
        
        let mut nodes = self.nodes.write().await;
        if let Some(node) = nodes.get_mut(node_index) {
            node.record_success(response_time);
            info!("Node {} ({}) - Response time: {:?}", node_index, node.url, response_time);
        }
        
        Ok(())
    }
    
    async fn connect_to_node(&self, node_index: usize) -> Result<()> {
        let nodes = self.nodes.read().await;
        let node_url = nodes.get(node_index)
            .ok_or_else(|| anyhow!("Invalid node index: {}", node_index))?
            .url.clone();
        drop(nodes);
        
        let provider = Provider::<Http>::try_from(node_url.as_str())?;
        let provider = Arc::new(provider);
        
        *self.current_provider.write().await = Some(provider.clone());
        *self.current_node_index.write().await = node_index;
        
        info!("Connected to RPC node: {}", node_url);
        self.validate_current_connection().await?;
        Ok(())
    }

    async fn validate_current_connection(&self) -> Result<()> {
        let provider = self.current_provider.read().await;
        let provider = provider.as_ref()
            .ok_or_else(|| anyhow!("No active provider"))?;
        
        let network_chain_id = provider.get_chainid().await?;
        
        if network_chain_id.as_u64() != self.chain_id {
            warn!(
                "Chain ID mismatch: expected {}, got {}",
                self.chain_id,
                network_chain_id.as_u64()
            );
        }
        
        info!("Connected to Taiko network, chain ID: {}", network_chain_id);
        Ok(())
    }
    
    async fn rotate_to_next_node(&self) -> Result<()> {
        {
            let current_index = *self.current_node_index.read().await;
            let mut nodes = self.nodes.write().await;
            if let Some(node) = nodes.get_mut(current_index) {
                node.record_failure();
                error!("Node {} ({}) marked as failed", current_index, node.url);
            }
        }
        
        self.connect_to_best_node().await
    }

    async fn get_optimal_provider_for_block(&self, block_number: u64) -> Result<Arc<Provider<Http>>> {
        let nodes = self.nodes.read().await;
        
        let healthy_nodes: Vec<(usize, &NodeHealth)> = nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.is_healthy)
            .collect();
        
        if healthy_nodes.is_empty() {
            drop(nodes);
            let provider = self.current_provider.read().await;
            return provider.as_ref()
                .ok_or_else(|| anyhow!("No healthy RPC nodes available"))
                .map(|p| p.clone());
        }
        
        let node_index = (block_number / 10) % healthy_nodes.len() as u64;
        let selected_node = &healthy_nodes[node_index as usize];
        let node_url = selected_node.1.url.clone();
        
        drop(nodes);
        
        let provider = Provider::<Http>::try_from(node_url.as_str())?;
        Ok(Arc::new(provider))
    }

    pub async fn get_block_by_number_distributed(&self, block_number: u64) -> Result<Option<Block<Transaction>>> {
        let provider = self.get_optimal_provider_for_block(block_number).await?;
        
        let start_time = Instant::now();
        match provider.get_block_with_txs(BlockNumber::Number(block_number.into())).await {
            Ok(block) => {
                let response_time = start_time.elapsed();
                info!("✅ Block {} fetched via distributed RPC in {:?}", block_number, response_time);
                Ok(block)
            },
            Err(e) => {
                warn!("❌ Distributed RPC failed for block {}: {}. Falling back to standard rotation.", block_number, e);
                self.get_block_by_number(block_number).await
            }
        }
    }
    
    async fn execute_with_retry<F, T>(&self, operation: F) -> Result<T>
    where
        F: Fn(Arc<Provider<Http>>) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T>> + Send>> + Send + Sync,
        T: Send,
    {
        const MAX_RETRIES: u8 = 3;
        let mut retries = 0;
        
        loop {
            let start_time = Instant::now();
            
            let provider = {
                let current_provider = self.current_provider.read().await;
                match current_provider.as_ref() {
                    Some(p) => p.clone(),
                    None => {
                        self.connect_to_best_node().await?;
                        let current_provider = self.current_provider.read().await;
                        current_provider.as_ref().unwrap().clone()
                    }
                }
            };
            
            match operation(provider).await {
                Ok(result) => {
                    let response_time = start_time.elapsed();
                    let current_index = *self.current_node_index.read().await;
                    let mut nodes = self.nodes.write().await;
                    if let Some(node) = nodes.get_mut(current_index) {
                        node.record_success(response_time);
                    }
                    return Ok(result);
                },
                Err(e) => {
                    let error_msg = e.to_string().to_lowercase();
                    
                    if error_msg.contains("429") || 
                       error_msg.contains("too many requests") ||
                       error_msg.contains("rate limit") ||
                       error_msg.contains("connection") ||
                       error_msg.contains("timeout") {
                        
                        warn!("RPC error ({}): {}. Attempting node rotation...", retries + 1, e);
                        
                        if let Err(rotate_err) = self.rotate_to_next_node().await {
                            error!("Failed to rotate to next node: {}", rotate_err);
                        }
                        
                        retries += 1;
                        if retries >= MAX_RETRIES {
                            return Err(anyhow!("Max retries exceeded. Last error: {}", e));
                        }
                        
                        tokio::time::sleep(Duration::from_millis(1000 * retries as u64)).await;
                        continue;
                    } else {
                        return Err(e);
                    }
                }
            }
        }
    }

    pub async fn get_latest_block_number(&self) -> Result<u64> {
        self.execute_with_retry(|provider| {
            Box::pin(async move {
                let block_number = provider.get_block_number().await?;
                Ok(block_number.as_u64())
            })
        }).await
    }

    pub async fn get_block_by_number(&self, block_number: u64) -> Result<Option<Block<Transaction>>> {
        self.execute_with_retry(|provider| {
            Box::pin(async move {
                let block = provider
                    .get_block_with_txs(BlockNumber::Number(block_number.into()))
                    .await?;
                Ok(block)
            })
        }).await
    }

    pub async fn get_block_by_hash(&self, block_hash: H256) -> Result<Option<Block<Transaction>>> {
        self.execute_with_retry(|provider| {
            Box::pin(async move {
                let block = provider.get_block_with_txs(block_hash).await?;
                Ok(block)
            })
        }).await
    }

    pub async fn get_transaction_receipt(&self, tx_hash: H256) -> Result<Option<TransactionReceipt>> {
        self.execute_with_retry(|provider| {
            Box::pin(async move {
                let receipt = provider.get_transaction_receipt(tx_hash).await?;
                Ok(receipt)
            })
        }).await
    }

    pub async fn get_transaction_receipts_batch(&self, tx_hashes: Vec<H256>) -> Result<Vec<Option<TransactionReceipt>>> {
        let mut receipts = Vec::new();
        let mut consecutive_errors = 0;
        
        for (i, tx_hash) in tx_hashes.iter().enumerate() {
            match self.get_transaction_receipt(*tx_hash).await {
                Ok(receipt) => {
                    receipts.push(receipt);
                    consecutive_errors = 0;
                },
                Err(e) => {
                    let error_msg = e.to_string();
                    
                    if error_msg.contains("Too many requests") || error_msg.contains("rate limit") {
                        consecutive_errors += 1;
                        warn!("Rate limited on tx {} ({}/{}). Waiting longer...", tx_hash, i+1, tx_hashes.len());
                        
                        if consecutive_errors > 3 {
                            warn!("Too many rate limit errors. Skipping remaining receipts for this block.");
                            for _ in i..tx_hashes.len() {
                                receipts.push(None);
                            }
                            break;
                        }
                        
                        let wait_time = std::time::Duration::from_secs(2u64.pow(consecutive_errors));
                        tokio::time::sleep(wait_time).await;
                        receipts.push(None);
                    } else {
                        warn!("Failed to fetch receipt for tx {}: {}", tx_hash, e);
                        receipts.push(None);
                    }
                }
            }
            
            let delay_ms = if consecutive_errors > 0 {
                50 * (consecutive_errors + 1) as u64
            } else {
                10
            };
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        }
        
        Ok(receipts)
    }
}