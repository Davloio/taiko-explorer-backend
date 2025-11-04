use anyhow::Result;
use serde::Deserialize;
use std::env;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub database_url: String,
    pub taiko_rpc_url: String,
    pub taiko_chain_id: u64,
    pub log_level: String,
    pub batch_size: u64,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenv::dotenv().ok();

        let config = Self {
            database_url: env::var("DATABASE_URL")
                .expect("DATABASE_URL must be set"),
            taiko_rpc_url: env::var("TAIKO_RPC_URL")
                .unwrap_or_else(|_| "https://rpc.mainnet.taiko.xyz".to_string()),
            taiko_chain_id: env::var("TAIKO_CHAIN_ID")
                .unwrap_or_else(|_| "167000".to_string())
                .parse()
                .expect("TAIKO_CHAIN_ID must be a valid number"),
            log_level: env::var("LOG_LEVEL")
                .unwrap_or_else(|_| "info".to_string()),
            batch_size: env::var("BATCH_SIZE")
                .unwrap_or_else(|_| "100".to_string())
                .parse()
                .expect("BATCH_SIZE must be a valid number"),
        };

        Ok(config)
    }
}