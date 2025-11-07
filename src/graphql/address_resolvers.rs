use async_graphql::*;
use std::sync::Arc;

use crate::db::address_analytics::AddressAnalyticsRepository;
use crate::models::address_analytics::{AddressLabel, AddressStats, AddressBalance};
use crate::models::bridge::{BridgeTransaction, BridgeStats};

#[derive(SimpleObject)]
#[graphql(complex)]
pub struct AddressLabelGQL {
    pub id: i32,
    pub address: String,
    pub label: String,
    pub category: String,
    pub description: Option<String>,
    pub verified: Option<bool>,
}

#[ComplexObject]
impl AddressLabelGQL {
    async fn created_at_iso(&self) -> Option<String> {
        // Would need to add created_at field to the struct
        None
    }
}

impl From<AddressLabel> for AddressLabelGQL {
    fn from(label: AddressLabel) -> Self {
        Self {
            id: label.id,
            address: label.address,
            label: label.label,
            category: label.category,
            description: label.description,
            verified: label.verified,
        }
    }
}

#[derive(SimpleObject)]
#[graphql(complex)]
pub struct AddressStatsGQL {
    pub id: i32,
    pub address: String,
    pub first_seen_block: i64,
    pub last_seen_block: i64,
    pub total_transactions: Option<i64>,
    pub total_sent_transactions: Option<i64>,
    pub total_received_transactions: Option<i64>,
    pub gas_used: Option<i64>,
    pub unique_counterparties: Option<i32>,
    pub contract_deployments: Option<i32>,
}

#[ComplexObject]
impl AddressStatsGQL {
    async fn total_volume_in_eth(&self, ctx: &Context<'_>) -> Result<f64> {
        // This would require access to the actual AddressStats model
        // For now, return 0.0 as placeholder
        Ok(0.0)
    }

    async fn gas_fees_in_eth(&self) -> f64 {
        // Placeholder - would need gas_fees_paid field
        0.0
    }

    async fn activity_score(&self) -> f64 {
        let txs = self.total_transactions.unwrap_or(0) as f64;
        let counterparties = self.unique_counterparties.unwrap_or(0) as f64;
        let deployments = self.contract_deployments.unwrap_or(0) as f64 * 10.0;
        
        txs + counterparties + deployments
    }

    async fn label(&self, ctx: &Context<'_>) -> Result<Option<AddressLabelGQL>> {
        let repo = ctx.data::<Arc<AddressAnalyticsRepository>>()?;
        
        match repo.get_address_label(&self.address) {
            Ok(Some(label)) => Ok(Some(label.into())),
            Ok(None) => Ok(None),
            Err(e) => Err(Error::new(format!("Database error: {}", e))),
        }
    }
}

impl From<AddressStats> for AddressStatsGQL {
    fn from(stats: AddressStats) -> Self {
        Self {
            id: stats.id,
            address: stats.address,
            first_seen_block: stats.first_seen_block,
            last_seen_block: stats.last_seen_block,
            total_transactions: stats.total_transactions,
            total_sent_transactions: stats.total_sent_transactions,
            total_received_transactions: stats.total_received_transactions,
            gas_used: stats.gas_used,
            unique_counterparties: stats.unique_counterparties,
            contract_deployments: stats.contract_deployments,
        }
    }
}

#[derive(SimpleObject)]
pub struct AddressBalanceGQL {
    pub id: i32,
    pub address: String,
    pub balance_wei: String,
    pub balance_eth: f64,
    pub block_number: i64,
    pub timestamp: String,
}

impl From<AddressBalance> for AddressBalanceGQL {
    fn from(balance: AddressBalance) -> Self {
        let balance_eth = balance.balance.to_string().parse::<f64>().unwrap_or(0.0) / 1e18;
        
        Self {
            id: balance.id,
            address: balance.address,
            balance_wei: balance.balance.to_string(),
            balance_eth,
            block_number: balance.block_number,
            timestamp: balance.timestamp.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        }
    }
}

#[derive(SimpleObject)]
#[graphql(complex)]
pub struct BridgeTransactionGQL {
    pub id: i32,
    pub transaction_hash: String,
    pub block_number: i64,
    pub bridge_type: String,
    pub from_chain: String,
    pub to_chain: String,
    pub from_address: String,
    pub to_address: Option<String>,
    pub token_address: Option<String>,
    pub amount_wei: String,
    pub bridge_contract: String,
    pub status: Option<String>,
    pub proof_submitted: Option<bool>,
    pub finalized: Option<bool>,
}

#[ComplexObject]
impl BridgeTransactionGQL {
    async fn amount_in_eth(&self) -> f64 {
        self.amount_wei.parse::<f64>().unwrap_or(0.0) / 1e18
    }

    async fn timestamp_iso(&self) -> String {
        // Would need to add timestamp field
        "".to_string()
    }

    async fn is_deposit(&self) -> bool {
        self.bridge_type == "deposit"
    }

    async fn is_withdrawal(&self) -> bool {
        self.bridge_type == "withdrawal"
    }

    async fn is_pending(&self) -> bool {
        self.status.as_ref().map_or(true, |s| s == "pending")
    }
}

impl From<BridgeTransaction> for BridgeTransactionGQL {
    fn from(bridge_tx: BridgeTransaction) -> Self {
        Self {
            id: bridge_tx.id,
            transaction_hash: bridge_tx.transaction_hash,
            block_number: bridge_tx.block_number,
            bridge_type: bridge_tx.bridge_type,
            from_chain: bridge_tx.from_chain,
            to_chain: bridge_tx.to_chain,
            from_address: bridge_tx.from_address,
            to_address: bridge_tx.to_address,
            token_address: bridge_tx.token_address,
            amount_wei: bridge_tx.amount.to_string(),
            bridge_contract: bridge_tx.bridge_contract,
            status: bridge_tx.status,
            proof_submitted: bridge_tx.proof_submitted,
            finalized: bridge_tx.finalized,
        }
    }
}

#[derive(SimpleObject)]
pub struct BridgeStatsGQL {
    pub date: String,
    pub total_deposits_count: Option<i64>,
    pub total_withdrawals_count: Option<i64>,
    pub total_deposit_volume_eth: f64,
    pub total_withdrawal_volume_eth: f64,
    pub tvl_eth: f64,
    pub unique_depositors: Option<i32>,
    pub unique_withdrawers: Option<i32>,
    pub net_flow_eth: f64,
}

impl From<BridgeStats> for BridgeStatsGQL {
    fn from(stats: BridgeStats) -> Self {
        let deposit_volume_eth = stats.total_deposit_volume.as_ref()
            .map(|v| v.to_string().parse::<f64>().unwrap_or(0.0) / 1e18)
            .unwrap_or(0.0);
        
        let withdrawal_volume_eth = stats.total_withdrawal_volume.as_ref()
            .map(|v| v.to_string().parse::<f64>().unwrap_or(0.0) / 1e18)
            .unwrap_or(0.0);
        
        let tvl_eth = stats.tvl_eth.as_ref()
            .map(|v| v.to_string().parse::<f64>().unwrap_or(0.0) / 1e18)
            .unwrap_or(0.0);

        Self {
            date: stats.date.format("%Y-%m-%d").to_string(),
            total_deposits_count: stats.total_deposits_count,
            total_withdrawals_count: stats.total_withdrawals_count,
            total_deposit_volume_eth: deposit_volume_eth,
            total_withdrawal_volume_eth: withdrawal_volume_eth,
            tvl_eth,
            unique_depositors: stats.unique_depositors,
            unique_withdrawers: stats.unique_withdrawers,
            net_flow_eth: deposit_volume_eth - withdrawal_volume_eth,
        }
    }
}

#[derive(SimpleObject)]
pub struct AddressAnalytics {
    pub top_addresses_by_volume: Vec<AddressStatsGQL>,
    pub top_addresses_by_activity: Vec<AddressStatsGQL>,
    pub dex_addresses: Vec<AddressLabelGQL>,
    pub bridge_addresses: Vec<AddressLabelGQL>,
}

#[derive(SimpleObject)]
pub struct BridgeAnalytics {
    pub recent_bridge_transactions: Vec<BridgeTransactionGQL>,
    pub daily_bridge_stats: Vec<BridgeStatsGQL>,
    pub total_tvl_eth: f64,
    pub bridge_volume_24h_eth: f64,
}