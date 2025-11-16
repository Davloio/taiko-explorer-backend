use anyhow::{anyhow, Result};
use ethers::types::{H256, TransactionReceipt, Block, Transaction};
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::{info, warn, error};

pub struct BatchRpcClient {
    client: reqwest::Client,
    base_url: String,
}

impl BatchRpcClient {
    pub fn new(base_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url,
        }
    }

    pub async fn get_receipts_batch(&self, tx_hashes: Vec<H256>, batch_size: usize) -> Result<Vec<Option<TransactionReceipt>>> {
        if tx_hashes.is_empty() {
            return Ok(Vec::new());
        }

        let mut all_receipts = Vec::with_capacity(tx_hashes.len());
        
        for chunk in tx_hashes.chunks(batch_size) {
            info!("📦 Fetching batch of {} receipts (NO FALLBACK)", chunk.len());
            
            let mut receipts = self.fetch_receipt_batch(chunk).await?;
            all_receipts.append(&mut receipts);
        }

        info!("✅ Batch processing complete: {} receipts fetched in {} batch calls", 
              all_receipts.len(), (tx_hashes.len() + batch_size - 1) / batch_size);

        Ok(all_receipts)
    }

    async fn fetch_receipt_batch(&self, tx_hashes: &[H256]) -> Result<Vec<Option<TransactionReceipt>>> {
        let mut batch_request = Vec::new();
        
        for (id, tx_hash) in tx_hashes.iter().enumerate() {
            batch_request.push(json!({
                "jsonrpc": "2.0",
                "method": "eth_getTransactionReceipt",
                "params": [format!("0x{:x}", tx_hash)],
                "id": id
            }));
        }

        let start_time = std::time::Instant::now();

        let response = self.client
            .post(&self.base_url)
            .header("Content-Type", "application/json")
            .json(&batch_request)
            .send()
            .await?;

        let response_time = start_time.elapsed();
        info!("⚡ Batch request completed in {:?} ({} receipts = {:.1} receipts/ms)", 
              response_time, tx_hashes.len(), tx_hashes.len() as f64 / response_time.as_millis() as f64);

        if !response.status().is_success() {
            return Err(anyhow!("HTTP error: {}", response.status()));
        }

        let batch_response: Value = response.json().await?;
        
        self.parse_batch_response(batch_response, tx_hashes.len()).await
    }

    async fn parse_batch_response(&self, response: Value, expected_count: usize) -> Result<Vec<Option<TransactionReceipt>>> {
        let mut receipts = vec![None; expected_count];

        match response {
            Value::Array(responses) => {
                for response_item in responses {
                    if let Some(id) = response_item["id"].as_u64() {
                        let index = id as usize;
                        
                        if index < expected_count {
                            if let Some(result) = response_item["result"].as_object() {
                                if !result.is_empty() {
                                    match serde_json::from_value::<TransactionReceipt>(response_item["result"].clone()) {
                                        Ok(receipt) => {
                                            receipts[index] = Some(receipt);
                                        }
                                        Err(e) => {
                                            warn!("Failed to parse receipt at index {}: {}", index, e);
                                        }
                                    }
                                }
                            } else if response_item["error"].is_object() {
                                warn!("RPC error for receipt {}: {:?}", index, response_item["error"]);
                            }
                        }
                    }
                }
            }
            _ => {
                return Err(anyhow!("Invalid batch response format"));
            }
        }

        Ok(receipts)
    }

    async fn get_single_receipt(&self, tx_hash: H256) -> Result<Option<TransactionReceipt>> {
        let request = json!({
            "jsonrpc": "2.0",
            "method": "eth_getTransactionReceipt",
            "params": [format!("0x{:x}", tx_hash)],
            "id": 1
        });

        let response = self.client
            .post(&self.base_url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("HTTP error: {}", response.status()));
        }

        let response: Value = response.json().await?;
        
        if let Some(result) = response["result"].as_object() {
            if !result.is_empty() {
                match serde_json::from_value::<TransactionReceipt>(response["result"].clone()) {
                    Ok(receipt) => Ok(Some(receipt)),
                    Err(e) => Err(anyhow!("Failed to parse receipt: {}", e)),
                }
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    pub async fn test_batch_limits(&self) -> Result<BatchCapabilities> {
        let test_hash = H256::from_low_u64_be(1);
        let test_sizes = vec![1, 10, 50, 100, 500, 1000];
        
        let mut max_batch_size = 1;
        let mut optimal_batch_size = 10;
        let mut results = HashMap::new();

        for size in test_sizes {
            info!("🧪 Testing batch size: {}", size);
            
            let test_hashes = vec![test_hash; size];
            let start_time = std::time::Instant::now();
            
            match self.fetch_receipt_batch(&test_hashes).await {
                Ok(_) => {
                    let duration = start_time.elapsed();
                    let rps = size as f64 / duration.as_secs_f64();
                    
                    info!("✅ Batch size {} succeeded: {:.1} receipts/second", size, rps);
                    results.insert(size, rps);
                    max_batch_size = size;
                    
                    if let Some(&current_best) = results.get(&optimal_batch_size) {
                        if rps > current_best {
                            optimal_batch_size = size;
                        }
                    }
                }
                Err(e) => {
                    warn!("❌ Batch size {} failed: {}", size, e);
                    break;
                }
            }
            
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }

        Ok(BatchCapabilities {
            max_batch_size,
            optimal_batch_size,
            throughput_results: results,
        })
    }
}

#[derive(Debug)]
pub struct BatchCapabilities {
    pub max_batch_size: usize,
    pub optimal_batch_size: usize,
    pub throughput_results: HashMap<usize, f64>,
}

impl BatchCapabilities {
    pub fn print_results(&self) {
        info!("\n🎯 ===== BATCH CAPABILITIES =====");
        info!("📊 Maximum batch size: {}", self.max_batch_size);
        info!("⚡ Optimal batch size: {} (best throughput)", self.optimal_batch_size);
        
        info!("\n📈 Throughput by batch size:");
        let mut sorted_results: Vec<_> = self.throughput_results.iter().collect();
        sorted_results.sort_by_key(|(size, _)| *size);
        
        for (size, rps) in sorted_results {
            info!("   Batch {}: {:.1} receipts/second", size, rps);
        }

        if let Some(&optimal_rps) = self.throughput_results.get(&self.optimal_batch_size) {
            let blocks_per_second_10tx = optimal_rps / 10.0;
            let blocks_per_second_40tx = optimal_rps / 40.0;
            
            info!("\n💡 THEORETICAL PERFORMANCE:");
            info!("   With 10 tx/block: {:.1} blocks/second", blocks_per_second_10tx);
            info!("   With 40 tx/block: {:.1} blocks/second", blocks_per_second_40tx);
            
            if blocks_per_second_40tx >= 30.0 {
                info!("🎉 SUCCESS: Can achieve 30+ blocks/second even with 40 tx/block!");
            } else {
                info!("⚠️  LIMITED: Max ~{:.1} blocks/second with 40 tx/block", blocks_per_second_40tx);
            }
        }
    }
}

impl BatchRpcClient {
    pub async fn get_blocks_batch(&self, block_numbers: Vec<u64>) -> Result<Vec<Option<Block<Transaction>>>> {
        if block_numbers.is_empty() {
            return Ok(Vec::new());
        }

        let mut batch_request = Vec::new();
        
        for (id, block_num) in block_numbers.iter().enumerate() {
            batch_request.push(json!({
                "jsonrpc": "2.0",
                "method": "eth_getBlockByNumber",
                "params": [format!("0x{:x}", block_num), true],
                "id": id
            }));
        }

        let start_time = std::time::Instant::now();

        let response = self.client
            .post(&self.base_url)
            .header("Content-Type", "application/json")
            .json(&batch_request)
            .send()
            .await?;

        let response_time = start_time.elapsed();
        info!("⚡ Block batch request completed in {:?} ({} blocks = {:.1} blocks/ms)", 
              response_time, block_numbers.len(), block_numbers.len() as f64 / response_time.as_millis() as f64);

        if !response.status().is_success() {
            return Err(anyhow!("HTTP error: {}", response.status()));
        }

        let batch_response: Value = response.json().await?;
        let mut blocks = vec![None; block_numbers.len()];

        match batch_response {
            Value::Array(responses) => {
                for response in responses {
                    if let Some(id) = response["id"].as_u64() {
                        if let Some(index) = id.checked_sub(0).and_then(|i| (i as usize).checked_sub(0)) {
                            if index < blocks.len() {
                                if let Some(result) = response.get("result") {
                                    if !result.is_null() {
                                        match serde_json::from_value::<Block<Transaction>>(result.clone()) {
                                            Ok(block) => blocks[index] = Some(block),
                                            Err(e) => warn!("Failed to parse block {}: {}", block_numbers[index], e),
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {
                return Err(anyhow!("Invalid batch response format"));
            }
        }

        Ok(blocks)
    }
}