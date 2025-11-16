use anyhow::Result;
use serde::Deserialize;
use std::env;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub database_url: String,
    pub taiko_rpc_nodes: Vec<String>,
    pub taiko_chain_id: u64,
    pub log_level: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenv::dotenv().ok();

        let rpc_nodes_string = env::var("TAIKO_RPC_NODES")
            .unwrap_or_else(|_| "https://taiko-rpc.publicnode.com,https://taiko-json-rpc.stakely.io,https://rpc.mainnet.taiko.xyz".to_string());
        
        let rpc_nodes: Vec<String> = rpc_nodes_string
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let config = Self {
            database_url: env::var("DATABASE_URL")
                .expect("DATABASE_URL must be set"),
            taiko_rpc_nodes: rpc_nodes,
            taiko_chain_id: env::var("TAIKO_CHAIN_ID")
                .unwrap_or_else(|_| "167000".to_string())
                .parse()
                .expect("TAIKO_CHAIN_ID must be a valid number"),
            log_level: env::var("LOG_LEVEL")
                .unwrap_or_else(|_| "info".to_string()),
        };

        Ok(config)
    }
}