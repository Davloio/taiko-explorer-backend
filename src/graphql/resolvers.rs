use async_graphql::*;
use crate::db::operations::BlockRepository;
use crate::db::address_analytics::AddressAnalyticsRepository;
use crate::models::block::Block;
use crate::models::transaction::Transaction;
use crate::graphql::address_resolvers::{AddressStatsGQL, AddressLabelGQL, AddressBalanceGQL, BridgeTransactionGQL, BridgeStatsGQL, AddressAnalytics, BridgeAnalytics};
use std::sync::Arc;

#[derive(SimpleObject)]
#[graphql(complex)]
pub struct BlockGQL {
    pub id: i32,
    pub number: i64,
    pub hash: String,
    pub parent_hash: String,
    pub timestamp: i64,
    pub gas_limit: i64,
    pub gas_used: i64,
    pub miner: String,
    pub transaction_count: i32,
    pub size: Option<i64>,
    pub base_fee_per_gas: Option<i64>,
}

#[ComplexObject]
impl BlockGQL {
    async fn gas_used_percentage(&self) -> f64 {
        if self.gas_limit > 0 {
            (self.gas_used as f64 / self.gas_limit as f64) * 100.0
        } else {
            0.0
        }
    }
    
    async fn timestamp_iso(&self) -> String {
        chrono::DateTime::from_timestamp(self.timestamp, 0)
            .unwrap_or_default()
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string()
    }
}

impl From<Block> for BlockGQL {
    fn from(block: Block) -> Self {
        Self {
            id: block.id,
            number: block.number,
            hash: block.hash,
            parent_hash: block.parent_hash,
            timestamp: block.timestamp,
            gas_limit: block.gas_limit,
            gas_used: block.gas_used,
            miner: block.miner,
            transaction_count: block.transaction_count,
            size: block.size,
            base_fee_per_gas: block.base_fee_per_gas,
        }
    }
}

#[derive(SimpleObject)]
pub struct BlocksConnection {
    pub blocks: Vec<BlockGQL>,
    pub total_count: i64,
    pub has_next_page: bool,
    pub has_previous_page: bool,
}

#[derive(SimpleObject)]
#[graphql(complex)]
pub struct TransactionGQL {
    pub id: i32,
    pub hash: String,
    pub block_number: i64,
    pub block_hash: String,
    pub transaction_index: i32,
    pub from_address: String,
    pub to_address: Option<String>,
    pub value: String,
    pub gas_limit: i64,
    pub gas_used: Option<i64>,
    pub gas_price: Option<String>,
    pub max_fee_per_gas: Option<String>,
    pub max_priority_fee_per_gas: Option<String>,
    pub nonce: i64,
    pub input_data: Option<String>,
    pub status: Option<i32>,
    pub contract_address: Option<String>,
    pub logs_count: Option<i32>,
    pub cumulative_gas_used: Option<i64>,
    pub effective_gas_price: Option<String>,
    pub transaction_type: Option<i32>,
    #[graphql(skip)]
    pub block_repo: Option<Arc<BlockRepository>>,
}

#[ComplexObject]
impl TransactionGQL {
    async fn value_in_eth(&self) -> f64 {
        self.value.parse::<f64>().unwrap_or(0.0) / 1e18
    }
    
    async fn gas_price_in_gwei(&self) -> Option<f64> {
        self.gas_price.as_ref().map(|price| {
            price.parse::<f64>().unwrap_or(0.0) / 1e9
        })
    }
    
    async fn transaction_fee_in_eth(&self) -> Option<f64> {
        match (self.gas_used, &self.effective_gas_price) {
            (Some(gas_used), Some(gas_price)) => {
                let gas_price_val = gas_price.parse::<f64>().unwrap_or(0.0);
                let fee_wei = gas_used as f64 * gas_price_val;
                Some(fee_wei / 1e18)
            },
            _ => None,
        }
    }
    
    async fn is_successful(&self) -> bool {
        self.status == Some(1)
    }
    
    async fn is_failed(&self) -> bool {
        self.status == Some(0)
    }
    
    async fn is_contract_creation(&self) -> bool {
        self.to_address.is_none() && self.contract_address.is_some()
    }
    
    async fn timestamp(&self) -> Option<i64> {
        if let Some(repo) = &self.block_repo {
            repo.get_block_by_number(self.block_number)
                .ok()
                .flatten()
                .map(|block| block.timestamp)
        } else {
            None
        }
    }
    
    async fn timestamp_iso(&self) -> Option<String> {
        if let Some(repo) = &self.block_repo {
            repo.get_block_by_number(self.block_number)
                .ok()
                .flatten()
                .map(|block| {
                    chrono::DateTime::from_timestamp(block.timestamp, 0)
                        .unwrap_or_default()
                        .format("%Y-%m-%dT%H:%M:%SZ")
                        .to_string()
                })
        } else {
            None
        }
    }
}

impl Transaction {
    fn to_graphql(self, block_repo: Arc<BlockRepository>) -> TransactionGQL {
        TransactionGQL {
            id: self.id,
            hash: self.hash,
            block_number: self.block_number,
            block_hash: self.block_hash,
            transaction_index: self.transaction_index,
            from_address: self.from_address,
            to_address: self.to_address,
            value: self.value.to_string(),
            gas_limit: self.gas_limit,
            gas_used: self.gas_used,
            gas_price: self.gas_price.map(|gp| gp.to_string()),
            max_fee_per_gas: self.max_fee_per_gas.map(|mf| mf.to_string()),
            max_priority_fee_per_gas: self.max_priority_fee_per_gas.map(|mp| mp.to_string()),
            nonce: self.nonce,
            input_data: self.input_data,
            status: self.status,
            contract_address: self.contract_address,
            logs_count: self.logs_count,
            cumulative_gas_used: self.cumulative_gas_used,
            effective_gas_price: self.effective_gas_price.map(|egp| egp.to_string()),
            transaction_type: self.transaction_type,
            block_repo: Some(block_repo),
        }
    }
}

#[derive(SimpleObject)]
pub struct TransactionsConnection {
    pub transactions: Vec<TransactionGQL>,
    pub total_count: i64,
    pub has_next_page: bool,
    pub has_previous_page: bool,
}

#[derive(SimpleObject)]
pub struct ExplorerStats {
    pub total_blocks: i64,
    pub latest_block_number: i64,
    pub total_transactions: i64,
    pub avg_block_time: f64,
}

pub struct QueryResolver {
    block_repo: Arc<BlockRepository>,
    analytics_repo: Arc<AddressAnalyticsRepository>,
}

impl QueryResolver {
    pub fn new(block_repo: BlockRepository, analytics_repo: AddressAnalyticsRepository) -> Self {
        Self {
            block_repo: Arc::new(block_repo),
            analytics_repo: Arc::new(analytics_repo),
        }
    }
}

#[Object]
impl QueryResolver {
    /// Get a block by its number
    async fn block(&self, number: i64) -> Result<Option<BlockGQL>> {
        match self.block_repo.get_block_by_number(number) {
            Ok(Some(block)) => Ok(Some(block.into())),
            Ok(None) => Ok(None),
            Err(e) => Err(Error::new(format!("Database error: {}", e))),
        }
    }

    /// Get a block by its hash
    async fn block_by_hash(&self, hash: String) -> Result<Option<BlockGQL>> {
        match self.block_repo.get_block_by_hash(&hash) {
            Ok(Some(block)) => Ok(Some(block.into())),
            Ok(None) => Ok(None),
            Err(e) => Err(Error::new(format!("Database error: {}", e))),
        }
    }

    /// Get the latest block
    async fn latest_block(&self) -> Result<Option<BlockGQL>> {
        match self.block_repo.get_latest_block_number() {
            Ok(Some(latest_number)) => {
                match self.block_repo.get_block_by_number(latest_number) {
                    Ok(Some(block)) => Ok(Some(block.into())),
                    Ok(None) => Ok(None),
                    Err(e) => Err(Error::new(format!("Database error: {}", e))),
                }
            },
            Ok(None) => Ok(None),
            Err(e) => Err(Error::new(format!("Database error: {}", e))),
        }
    }

    /// Get multiple blocks with pagination
    async fn blocks(
        &self, 
        #[graphql(desc = "Starting block number")] from: Option<i64>,
        #[graphql(desc = "Number of blocks to fetch", default = 20)] limit: i32,
        #[graphql(desc = "Sort order: 'asc' or 'desc'", default = "desc")] order: String,
    ) -> Result<BlocksConnection> {
        let limit = std::cmp::min(limit, 100) as usize; // Max 100 blocks per query
        
        // Get total count
        let total_count = match self.block_repo.get_block_count() {
            Ok(count) => count,
            Err(e) => return Err(Error::new(format!("Database error: {}", e))),
        };

        // Determine starting point
        let start_block = from.unwrap_or_else(|| {
            if order == "desc" {
                self.block_repo.get_latest_block_number().unwrap_or(Some(0)).unwrap_or(0)
            } else {
                1
            }
        });

        // Generate block numbers to fetch
        let block_numbers: Vec<i64> = if order == "desc" {
            (start_block.saturating_sub(limit as i64 - 1)..=start_block)
                .rev()
                .collect()
        } else {
            (start_block..=start_block + limit as i64 - 1)
                .collect()
        };

        // Fetch blocks
        let mut blocks = Vec::new();
        for block_num in block_numbers {
            if let Ok(Some(block)) = self.block_repo.get_block_by_number(block_num) {
                blocks.push(block.into());
            }
        }

        let has_next_page = if order == "desc" {
            start_block > limit as i64
        } else {
            start_block + limit as i64 <= total_count
        };

        let has_previous_page = if order == "desc" {
            start_block < total_count
        } else {
            start_block > 1
        };

        Ok(BlocksConnection {
            blocks,
            total_count,
            has_next_page,
            has_previous_page,
        })
    }

    /// Get explorer statistics
    async fn stats(&self) -> Result<ExplorerStats> {
        let total_blocks = match self.block_repo.get_block_count() {
            Ok(count) => count,
            Err(e) => return Err(Error::new(format!("Database error: {}", e))),
        };

        let latest_block_number = match self.block_repo.get_latest_block_number() {
            Ok(Some(number)) => number,
            Ok(None) => 0,
            Err(_) => 0,
        };

        // Get actual transaction count
        let total_transactions = match self.block_repo.get_transaction_count() {
            Ok(count) => count,
            Err(_) => 0,
        };
        let avg_block_time = 12.0; // Taiko block time

        Ok(ExplorerStats {
            total_blocks,
            latest_block_number,
            total_transactions,
            avg_block_time,
        })
    }

    /// Get a transaction by its hash
    async fn transaction(&self, hash: String) -> Result<Option<TransactionGQL>> {
        match self.block_repo.get_transaction_by_hash(&hash) {
            Ok(Some(tx)) => Ok(Some(tx.to_graphql(Arc::clone(&self.block_repo)))),
            Ok(None) => Ok(None),
            Err(e) => Err(Error::new(format!("Database error: {}", e))),
        }
    }

    /// Get transactions for a specific block
    async fn transactions_by_block(&self, block_number: i64) -> Result<Vec<TransactionGQL>> {
        match self.block_repo.get_transactions_by_block(block_number) {
            Ok(txs) => Ok(txs.into_iter().map(|tx| tx.to_graphql(Arc::clone(&self.block_repo))).collect()),
            Err(e) => Err(Error::new(format!("Database error: {}", e))),
        }
    }

    /// Get transactions with pagination
    async fn transactions(
        &self,
        #[graphql(desc = "Number of transactions to fetch", default = 20)] limit: i32,
        #[graphql(desc = "Number of transactions to skip", default = 0)] offset: i32,
        #[graphql(desc = "Sort order: true for desc, false for asc", default = true)] order_desc: bool,
    ) -> Result<TransactionsConnection> {
        let limit = std::cmp::min(limit, 100) as i64; // Max 100 transactions per query
        let offset = offset as i64;

        // Get total count
        let total_count = match self.block_repo.get_transaction_count() {
            Ok(count) => count,
            Err(e) => return Err(Error::new(format!("Database error: {}", e))),
        };

        // Fetch transactions
        let transactions = match self.block_repo.get_transactions_paginated(limit, offset, order_desc) {
            Ok(txs) => txs.into_iter().map(|tx| tx.to_graphql(Arc::clone(&self.block_repo))).collect(),
            Err(e) => return Err(Error::new(format!("Database error: {}", e))),
        };

        let has_next_page = offset + limit < total_count;
        let has_previous_page = offset > 0;

        Ok(TransactionsConnection {
            transactions,
            total_count,
            has_next_page,
            has_previous_page,
        })
    }

    /// Get transactions for a specific address (sent from or received to)
    async fn transactions_by_address(
        &self,
        address: String,
        #[graphql(desc = "Number of transactions to fetch", default = 20)] limit: i32,
        #[graphql(desc = "Number of transactions to skip", default = 0)] offset: i32,
    ) -> Result<TransactionsConnection> {
        let limit = std::cmp::min(limit, 100) as i64;
        let offset = offset as i64;

        // Get total count for address
        let total_count = match self.block_repo.get_transactions_count_by_address(&address) {
            Ok(count) => count,
            Err(e) => return Err(Error::new(format!("Database error: {}", e))),
        };

        // Fetch transactions
        let transactions = match self.block_repo.get_transactions_by_address(&address, limit, offset) {
            Ok(txs) => txs.into_iter().map(|tx| tx.to_graphql(Arc::clone(&self.block_repo))).collect(),
            Err(e) => return Err(Error::new(format!("Database error: {}", e))),
        };

        let has_next_page = offset + limit < total_count;
        let has_previous_page = offset > 0;

        Ok(TransactionsConnection {
            transactions,
            total_count,
            has_next_page,
            has_previous_page,
        })
    }

    /// Get failed transactions
    async fn failed_transactions(
        &self,
        #[graphql(desc = "Number of transactions to fetch", default = 20)] limit: i32,
    ) -> Result<Vec<TransactionGQL>> {
        let limit = std::cmp::min(limit, 100) as i64;

        match self.block_repo.get_failed_transactions(limit) {
            Ok(txs) => Ok(txs.into_iter().map(|tx| tx.to_graphql(Arc::clone(&self.block_repo))).collect()),
            Err(e) => Err(Error::new(format!("Database error: {}", e))),
        }
    }

    /// Get contract creation transactions
    async fn contract_creations(
        &self,
        #[graphql(desc = "Number of transactions to fetch", default = 20)] limit: i32,
    ) -> Result<Vec<TransactionGQL>> {
        let limit = std::cmp::min(limit, 100) as i64;

        match self.block_repo.get_contract_creation_transactions(limit) {
            Ok(txs) => Ok(txs.into_iter().map(|tx| tx.to_graphql(Arc::clone(&self.block_repo))).collect()),
            Err(e) => Err(Error::new(format!("Database error: {}", e))),
        }
    }

    // Address Analytics Queries

    /// Get address statistics
    async fn address_stats(&self, address: String) -> Result<Option<AddressStatsGQL>> {
        match self.analytics_repo.get_address_stats(&address) {
            Ok(Some(stats)) => Ok(Some(stats.into())),
            Ok(None) => Ok(None),
            Err(e) => Err(Error::new(format!("Database error: {}", e))),
        }
    }

    /// Get top addresses by transaction volume
    async fn top_addresses_by_volume(
        &self,
        #[graphql(desc = "Number of addresses to fetch", default = 10)] limit: i32,
    ) -> Result<Vec<AddressStatsGQL>> {
        let limit = std::cmp::min(limit, 100) as i64;

        match self.analytics_repo.get_top_addresses_by_volume(limit) {
            Ok(addresses) => Ok(addresses.into_iter().map(|addr| addr.into()).collect()),
            Err(e) => Err(Error::new(format!("Database error: {}", e))),
        }
    }

    /// Get top addresses by transaction count
    async fn top_addresses_by_activity(
        &self,
        #[graphql(desc = "Number of addresses to fetch", default = 10)] limit: i32,
    ) -> Result<Vec<AddressStatsGQL>> {
        let limit = std::cmp::min(limit, 100) as i64;

        match self.analytics_repo.get_top_addresses_by_transactions(limit) {
            Ok(addresses) => Ok(addresses.into_iter().map(|addr| addr.into()).collect()),
            Err(e) => Err(Error::new(format!("Database error: {}", e))),
        }
    }

    /// Get address labels by category
    async fn address_labels(
        &self,
        #[graphql(desc = "Filter by category (DEX, Bridge, etc.)")] category: Option<String>,
    ) -> Result<Vec<AddressLabelGQL>> {
        match self.analytics_repo.get_address_labels(category) {
            Ok(labels) => Ok(labels.into_iter().map(|label| label.into()).collect()),
            Err(e) => Err(Error::new(format!("Database error: {}", e))),
        }
    }

    /// Get address balance history
    async fn address_balance_history(
        &self,
        address: String,
        #[graphql(desc = "Number of balance snapshots to fetch", default = 30)] limit: i32,
    ) -> Result<Vec<AddressBalanceGQL>> {
        let limit = std::cmp::min(limit, 1000) as i64;

        match self.analytics_repo.get_address_balance_history(&address, Some(limit)) {
            Ok(balances) => Ok(balances.into_iter().map(|balance| balance.into()).collect()),
            Err(e) => Err(Error::new(format!("Database error: {}", e))),
        }
    }

    // Bridge Monitoring Queries

    /// Get recent bridge transactions
    async fn bridge_transactions(
        &self,
        #[graphql(desc = "Number of transactions to fetch", default = 20)] limit: i32,
        #[graphql(desc = "Number of transactions to skip", default = 0)] offset: i32,
    ) -> Result<Vec<BridgeTransactionGQL>> {
        let limit = std::cmp::min(limit, 100) as i64;
        let offset = offset as i64;

        match self.analytics_repo.get_bridge_transactions(limit, offset) {
            Ok(bridge_txs) => Ok(bridge_txs.into_iter().map(|tx| tx.into()).collect()),
            Err(e) => Err(Error::new(format!("Database error: {}", e))),
        }
    }

    /// Get bridge transactions for a specific address
    async fn bridge_transactions_by_address(
        &self,
        address: String,
        #[graphql(desc = "Number of transactions to fetch", default = 20)] limit: i32,
    ) -> Result<Vec<BridgeTransactionGQL>> {
        let limit = std::cmp::min(limit, 100) as i64;

        match self.analytics_repo.get_bridge_transactions_by_address(&address, limit) {
            Ok(bridge_txs) => Ok(bridge_txs.into_iter().map(|tx| tx.into()).collect()),
            Err(e) => Err(Error::new(format!("Database error: {}", e))),
        }
    }

    /// Get bridge volume statistics over time
    async fn bridge_volume_stats(
        &self,
        #[graphql(desc = "Number of days to fetch", default = 30)] days: i32,
    ) -> Result<Vec<BridgeStatsGQL>> {
        let days = std::cmp::min(days, 365);

        match self.analytics_repo.get_bridge_volume_by_date(days) {
            Ok(stats) => Ok(stats.into_iter().map(|stat| stat.into()).collect()),
            Err(e) => Err(Error::new(format!("Database error: {}", e))),
        }
    }

    /// Get comprehensive address analytics
    async fn address_analytics(&self) -> Result<AddressAnalytics> {
        // Get top addresses by volume (limit 10)
        let top_volume = match self.analytics_repo.get_top_addresses_by_volume(10) {
            Ok(addresses) => addresses.into_iter().map(|addr| addr.into()).collect(),
            Err(_) => vec![],
        };

        // Get top addresses by activity (limit 10)
        let top_activity = match self.analytics_repo.get_top_addresses_by_transactions(10) {
            Ok(addresses) => addresses.into_iter().map(|addr| addr.into()).collect(),
            Err(_) => vec![],
        };

        // Get DEX addresses
        let dex_addresses = match self.analytics_repo.get_address_labels(Some("DEX".to_string())) {
            Ok(labels) => labels.into_iter().map(|label| label.into()).collect(),
            Err(_) => vec![],
        };

        // Get Bridge addresses
        let bridge_addresses = match self.analytics_repo.get_address_labels(Some("Bridge".to_string())) {
            Ok(labels) => labels.into_iter().map(|label| label.into()).collect(),
            Err(_) => vec![],
        };

        Ok(AddressAnalytics {
            top_addresses_by_volume: top_volume,
            top_addresses_by_activity: top_activity,
            dex_addresses,
            bridge_addresses,
        })
    }

    /// Get comprehensive bridge analytics
    async fn bridge_analytics(&self) -> Result<BridgeAnalytics> {
        // Get recent bridge transactions
        let recent_transactions = match self.analytics_repo.get_bridge_transactions(20, 0) {
            Ok(txs) => txs.into_iter().map(|tx| tx.into()).collect(),
            Err(_) => vec![],
        };

        // Get daily bridge stats for last 30 days
        let daily_stats = match self.analytics_repo.get_bridge_volume_by_date(30) {
            Ok(stats) => stats.into_iter().map(|stat| stat.into()).collect(),
            Err(_) => vec![],
        };

        // Calculate aggregated metrics (placeholder values)
        let total_tvl_eth = 0.0; // Would calculate from latest stats
        let bridge_volume_24h_eth = 0.0; // Would calculate from last 24h

        Ok(BridgeAnalytics {
            recent_bridge_transactions: recent_transactions,
            daily_bridge_stats: daily_stats,
            total_tvl_eth,
            bridge_volume_24h_eth,
        })
    }
}