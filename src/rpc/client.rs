use anyhow::Result;
use ethers::prelude::*;
use std::sync::Arc;
use tracing::{info, warn};

#[derive(Clone)]
pub struct TaikoRpcClient {
    provider: Arc<Provider<Http>>,
    chain_id: u64,
}

impl TaikoRpcClient {
    pub async fn new(rpc_url: &str, chain_id: u64) -> Result<Self> {
        let provider = Provider::<Http>::try_from(rpc_url)?;
        let provider = Arc::new(provider);
        
        let client = Self {
            provider,
            chain_id,
        };
        
        client.validate_connection().await?;
        Ok(client)
    }

    async fn validate_connection(&self) -> Result<()> {
        let network_chain_id = self.provider.get_chainid().await?;
        
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

    pub async fn get_latest_block_number(&self) -> Result<u64> {
        let block_number = self.provider.get_block_number().await?;
        Ok(block_number.as_u64())
    }

    pub async fn get_block_by_number(&self, block_number: u64) -> Result<Option<Block<Transaction>>> {
        let block = self
            .provider
            .get_block_with_txs(BlockNumber::Number(block_number.into()))
            .await?;
        Ok(block)
    }

    pub async fn get_block_by_hash(&self, block_hash: H256) -> Result<Option<Block<Transaction>>> {
        let block = self.provider.get_block_with_txs(block_hash).await?;
        Ok(block)
    }
}