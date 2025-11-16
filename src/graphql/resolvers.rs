use async_graphql::*;
use crate::db::operations::BlockRepository;
use crate::db::address_analytics::AddressAnalyticsRepository;
use crate::models::block::Block;
use crate::models::transaction::Transaction;
use crate::graphql::address_resolvers::{AddressStatsGQL, BridgeTransactionGQL, BridgeStatsGQL, AddressAnalytics, BridgeAnalytics, AddressProfileGQL};
use std::sync::Arc;

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum TransactionDirection {
    In,
    Out,
    Inside,
}

impl TransactionDirection {
    fn to_db_value(&self) -> &'static str {
        match self {
            TransactionDirection::In => "in",
            TransactionDirection::Out => "out",
            TransactionDirection::Inside => "inside",
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum TransactionStatus {
    Success,
    Failed,
    Pending,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum TimeRange {
    Last7Days,
    Last30Days,
    Last3Months,
    AllTime,
}

impl TimeRange {
    fn to_days(&self) -> Option<i32> {
        match self {
            TimeRange::Last7Days => Some(7),
            TimeRange::Last30Days => Some(30),
            TimeRange::Last3Months => Some(90),
            TimeRange::AllTime => None,
        }
    }

    fn to_string(&self) -> &'static str {
        match self {
            TimeRange::Last7Days => "7D",
            TimeRange::Last30Days => "30D",
            TimeRange::Last3Months => "3M",
            TimeRange::AllTime => "ALL",
        }
    }
}

#[derive(SimpleObject)]
pub struct AddressChartPoint {
    pub timestamp: String,
    pub total_addresses: i32,
    pub new_addresses: i32,
}

#[derive(SimpleObject)]
pub struct AddressGrowthChart {
    pub data: Vec<AddressChartPoint>,
    pub total_addresses: i32,
    pub growth_rate: f64,
    pub time_range: String,
    pub data_points: i32,
}

impl TransactionStatus {
    fn to_db_value(&self) -> Option<i32> {
        match self {
            TransactionStatus::Success => Some(1),
            TransactionStatus::Failed => Some(0),
            TransactionStatus::Pending => None,
        }
    }
}

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
    pub direction: String,
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
            direction: self.direction,
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
pub struct TransactionStatusCounts {
    pub success_count: i64,
    pub failed_count: i64,
    pub pending_count: i64,
    pub total_count: i64,
}

#[derive(SimpleObject)]
pub struct TransactionDirectionCounts {
    pub in_count: i64,
    pub out_count: i64,
    pub inside_count: i64,
    pub total_count: i64,
}

#[derive(SimpleObject)]
pub struct ExplorerStats {
    pub total_blocks: i64,
    pub latest_block_number: i64,
    pub total_transactions: i64,
    pub total_addresses: i64,
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
    async fn block(&self, number: i64) -> Result<Option<BlockGQL>> {
        match self.block_repo.get_block_by_number(number) {
            Ok(Some(block)) => Ok(Some(block.into())),
            Ok(None) => Ok(None),
            Err(e) => Err(Error::new(format!("Database error: {}", e))),
        }
    }

     async fn block_by_hash(&self, hash: String) -> Result<Option<BlockGQL>> {
        match self.block_repo.get_block_by_hash(&hash) {
            Ok(Some(block)) => Ok(Some(block.into())),
            Ok(None) => Ok(None),
            Err(e) => Err(Error::new(format!("Database error: {}", e))),
        }
    }

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

     async fn blocks(
        &self, 
        #[graphql(desc = "Starting block number")] from: Option<i64>,
        #[graphql(desc = "Number of blocks to fetch", default = 20)] limit: i32,
        #[graphql(desc = "Sort order: 'asc' or 'desc'", default = "desc")] order: String,
    ) -> Result<BlocksConnection> {
        let limit = std::cmp::min(limit, 100) as usize;
        let total_count = match self.block_repo.get_block_count() {
            Ok(count) => count,
            Err(e) => return Err(Error::new(format!("Database error: {}", e))),
        };
        let start_block = from.unwrap_or_else(|| {
            if order == "desc" {
                self.block_repo.get_latest_block_number().unwrap_or(Some(0)).unwrap_or(0)
            } else {
                1
            }
        });
        let block_numbers: Vec<i64> = if order == "desc" {
            (start_block.saturating_sub(limit as i64 - 1)..=start_block)
                .rev()
                .collect()
        } else {
            (start_block..=start_block + limit as i64 - 1)
                .collect()
        };
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
        let total_transactions = match self.block_repo.get_transaction_count() {
            Ok(count) => count,
            Err(_) => 0,
        };
        let total_addresses = match self.block_repo.get_unique_address_count() {
            Ok(count) => count,
            Err(_) => 0,
        };

        let avg_block_time = 12.0;

        Ok(ExplorerStats {
            total_blocks,
            latest_block_number,
            total_transactions,
            total_addresses,
            avg_block_time,
        })
    }

     async fn transaction(&self, hash: String) -> Result<Option<TransactionGQL>> {
        match self.block_repo.get_transaction_by_hash(&hash) {
            Ok(Some(tx)) => Ok(Some(tx.to_graphql(Arc::clone(&self.block_repo)))),
            Ok(None) => Ok(None),
            Err(e) => Err(Error::new(format!("Database error: {}", e))),
        }
    }

     async fn transactions_by_block(&self, block_number: i64) -> Result<Vec<TransactionGQL>> {
        match self.block_repo.get_transactions_by_block(block_number) {
            Ok(txs) => Ok(txs.into_iter().map(|tx| tx.to_graphql(Arc::clone(&self.block_repo))).collect()),
            Err(e) => Err(Error::new(format!("Database error: {}", e))),
        }
    }

     async fn transactions(
        &self,
        #[graphql(desc = "Number of transactions to fetch", default = 20)] limit: i32,
        #[graphql(desc = "Number of transactions to skip", default = 0)] offset: i32,
        #[graphql(desc = "Sort order: true for desc, false for asc", default = true)] order_desc: bool,
        #[graphql(desc = "Filter by transaction status")] status_filter: Option<TransactionStatus>,
        #[graphql(desc = "Filter by transaction direction")] direction_filter: Option<TransactionDirection>,
    ) -> Result<TransactionsConnection> {
        let limit = std::cmp::min(limit, 100) as i64;
        let offset = offset as i64;
        let status_filter_db = status_filter.and_then(|s| s.to_db_value());
        let direction_filter_db = direction_filter.map(|d| d.to_db_value());
        let total_count = match self.block_repo.get_transaction_count_filtered(status_filter_db, direction_filter_db) {
            Ok(count) => count,
            Err(e) => return Err(Error::new(format!("Database error: {}", e))),
        };
        let transactions = match self.block_repo.get_transactions_paginated_filtered(limit, offset, order_desc, status_filter_db, direction_filter_db) {
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

    async fn transactions_by_address(
        &self,
        address: String,
        #[graphql(desc = "Number of transactions to fetch", default = 20)] limit: i32,
        #[graphql(desc = "Number of transactions to skip", default = 0)] offset: i32,
        #[graphql(desc = "Filter by transaction status")] status_filter: Option<TransactionStatus>,
        #[graphql(desc = "Filter by transaction direction")] direction_filter: Option<TransactionDirection>,
    ) -> Result<TransactionsConnection> {
        let limit = std::cmp::min(limit, 100) as i64;
        let offset = offset as i64;

        let status_filter_db = status_filter.and_then(|s| s.to_db_value());
        let direction_filter_db = direction_filter.map(|d| d.to_db_value());

        let total_count = match self.block_repo.get_transactions_count_by_address_filtered(&address, status_filter_db, direction_filter_db) {
            Ok(count) => count,
            Err(e) => return Err(Error::new(format!("Database error: {}", e))),
        };
        let transactions = match self.block_repo.get_transactions_by_address_filtered(&address, limit, offset, status_filter_db, direction_filter_db) {
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


     async fn address_stats(&self, address: String) -> Result<Option<AddressStatsGQL>> {
        match self.analytics_repo.get_address_stats(&address) {
            Ok(Some(stats)) => Ok(Some(stats.into())),
            Ok(None) => Ok(None),
            Err(e) => Err(Error::new(format!("Database error: {}", e))),
        }
    }

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

    async fn address_analytics(&self) -> Result<AddressAnalytics> {
        let top_volume = match self.analytics_repo.get_top_addresses_by_volume(10) {
            Ok(addresses) => addresses.into_iter().map(|addr| addr.into()).collect(),
            Err(_) => vec![],
        };

        let top_activity = match self.analytics_repo.get_top_addresses_by_transactions(10) {
            Ok(addresses) => addresses.into_iter().map(|addr| addr.into()).collect(),
            Err(_) => vec![],
        };

        Ok(AddressAnalytics {
            top_addresses_by_volume: top_volume,
            top_addresses_by_activity: top_activity,
        })
    }

     async fn bridge_analytics(&self) -> Result<BridgeAnalytics> {
        let recent_transactions = match self.analytics_repo.get_bridge_transactions(20, 0) {
            Ok(txs) => txs.into_iter().map(|tx| tx.into()).collect(),
            Err(_) => vec![],
        };

        let daily_stats = match self.analytics_repo.get_bridge_volume_by_date(30) {
            Ok(stats) => stats.into_iter().map(|stat| stat.into()).collect(),
            Err(_) => vec![],
        };

        let total_tvl_eth = 0.0;
        let bridge_volume_24h_eth = 0.0;

        Ok(BridgeAnalytics {
            recent_bridge_transactions: recent_transactions,
            daily_bridge_stats: daily_stats,
            total_tvl_eth,
            bridge_volume_24h_eth,
        })
    }

     async fn address_profile(&self, address: String) -> Result<AddressProfileGQL> {
        match self.block_repo.get_address_profile(&address) {
            Ok(profile) => Ok(profile.into()),
            Err(e) => Err(Error::new(format!("Database error: {}", e))),
        }
    }
    
     async fn transaction_counts_by_status(&self) -> Result<TransactionStatusCounts> {
        let success_count = match self.block_repo.get_transaction_count_filtered(Some(1), None) {
            Ok(count) => count,
            Err(e) => return Err(Error::new(format!("Database error: {}", e))),
        };
        
        let failed_count = match self.block_repo.get_transaction_count_filtered(Some(0), None) {
            Ok(count) => count,
            Err(e) => return Err(Error::new(format!("Database error: {}", e))),
        };
        
        let pending_count = match self.block_repo.get_transaction_count_filtered(None, None) {
            Ok(total) => total - success_count - failed_count,
            Err(e) => return Err(Error::new(format!("Database error: {}", e))),
        };

        Ok(TransactionStatusCounts {
            success_count,
            failed_count,
            pending_count,
            total_count: success_count + failed_count + pending_count,
        })
    }
    
     async fn transaction_counts_by_direction(&self) -> Result<TransactionDirectionCounts> {
        let in_count = match self.block_repo.get_transaction_count_filtered(None, Some("in")) {
            Ok(count) => count,
            Err(e) => return Err(Error::new(format!("Database error: {}", e))),
        };
        
        let out_count = match self.block_repo.get_transaction_count_filtered(None, Some("out")) {
            Ok(count) => count,
            Err(e) => return Err(Error::new(format!("Database error: {}", e))),
        };
        
        let inside_count = match self.block_repo.get_transaction_count_filtered(None, Some("inside")) {
            Ok(count) => count,
            Err(e) => return Err(Error::new(format!("Database error: {}", e))),
        };

        Ok(TransactionDirectionCounts {
            in_count,
            out_count,
            inside_count,
            total_count: in_count + out_count + inside_count,
        })
    }

     async fn address_growth_chart(
        &self,
        #[graphql(desc = "Time range for the chart")] time_range: TimeRange,
    ) -> Result<AddressGrowthChart> {
        let chart_data = match self.block_repo.get_address_growth_chart(None) {
            Ok(data) => data,
            Err(e) => return Err(Error::new(format!("Database error: {}", e))),
        };

        let data_points: Vec<AddressChartPoint> = chart_data
            .iter()
            .map(|(timestamp, total, new)| AddressChartPoint {
                timestamp: timestamp.clone(),
                total_addresses: *total,
                new_addresses: *new,
            })
            .collect();

        let total_addresses = match self.block_repo.get_unique_address_count() {
            Ok(count) => count as i32,
            Err(_) => data_points.last().map(|p| p.total_addresses).unwrap_or(0),
        };
        let first_total = data_points.first().map(|p| p.total_addresses).unwrap_or(0);
        
        let growth_rate = if first_total > 0 {
            ((total_addresses - first_total) as f64 / first_total as f64) * 100.0
        } else {
            0.0
        };

        Ok(AddressGrowthChart {
            data: data_points,
            total_addresses,
            growth_rate,
            time_range: time_range.to_string().to_string(),
            data_points: chart_data.len() as i32,
        })
    }
}