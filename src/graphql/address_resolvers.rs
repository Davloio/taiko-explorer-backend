use async_graphql::*;
use std::sync::Arc;

use crate::db::address_analytics::AddressAnalyticsRepository;
use crate::db::operations::{AddressProfile, ActivityInfo};
use crate::models::address_analytics::AddressStats;
use crate::models::bridge::{BridgeTransaction, BridgeStats};

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
    pub total_volume_sent: Option<String>,
    pub total_volume_received: Option<String>,
}

#[ComplexObject]
impl AddressStatsGQL {
    async fn total_volume_in_eth(&self) -> Result<f64> {
        let sent_vol = self.total_volume_sent.as_ref()
            .map(|v| v.parse::<f64>().unwrap_or(0.0))
            .unwrap_or(0.0);
        
        let received_vol = self.total_volume_received.as_ref()
            .map(|v| v.parse::<f64>().unwrap_or(0.0))
            .unwrap_or(0.0);
        
        Ok((sent_vol + received_vol) / 1e18)
    }

    async fn gas_fees_in_eth(&self) -> f64 {
        0.0
    }

    async fn activity_score(&self) -> f64 {
        let txs = self.total_transactions.unwrap_or(0) as f64;
        let counterparties = self.unique_counterparties.unwrap_or(0) as f64;
        let deployments = self.contract_deployments.unwrap_or(0) as f64 * 10.0;
        
        txs + counterparties + deployments
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
            total_volume_sent: stats.total_volume_sent.map(|v| v.to_string()),
            total_volume_received: stats.total_volume_received.map(|v| v.to_string()),
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
}

#[derive(SimpleObject)]
pub struct BridgeAnalytics {
    pub recent_bridge_transactions: Vec<BridgeTransactionGQL>,
    pub daily_bridge_stats: Vec<BridgeStatsGQL>,
    pub total_tvl_eth: f64,
    pub bridge_volume_24h_eth: f64,
}

#[derive(SimpleObject)]
pub struct ActivityInfoGQL {
    pub block_number: i64,
    pub transaction_hash: String,
}

impl From<ActivityInfo> for ActivityInfoGQL {
    fn from(activity: ActivityInfo) -> Self {
        Self {
            block_number: activity.block_number,
            transaction_hash: activity.transaction_hash,
        }
    }
}

#[derive(SimpleObject)]
pub struct AddressProfileGQL {
    pub address: String,
    pub total_transactions: i64,
    pub first_activity: Option<ActivityInfoGQL>,
    pub last_activity: Option<ActivityInfoGQL>,
    pub total_sent: String,
    pub total_received: String,
    pub total_gas_fees: String,
    pub total_volume: String,
    pub net_balance: String,
}

impl From<AddressProfile> for AddressProfileGQL {
    fn from(profile: AddressProfile) -> Self {
        let total_volume = &profile.total_sent + &profile.total_received;
        let net_balance = &profile.total_received - &profile.total_sent;
        
        Self {
            address: profile.address,
            total_transactions: profile.total_transactions,
            first_activity: profile.first_activity.map(|a| a.into()),
            last_activity: profile.last_activity.map(|a| a.into()),
            total_sent: profile.total_sent.to_string(),
            total_received: profile.total_received.to_string(),
            total_gas_fees: profile.total_gas_fees.to_string(),
            total_volume: total_volume.to_string(),
            net_balance: net_balance.to_string(),
        }
    }
}