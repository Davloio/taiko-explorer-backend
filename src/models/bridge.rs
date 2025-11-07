use bigdecimal::BigDecimal;
use chrono::{DateTime, NaiveDate, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::schema::{bridge_stats, bridge_transactions};

#[derive(Debug, Clone, Queryable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = bridge_transactions)]
pub struct BridgeTransaction {
    pub id: i32,
    pub transaction_hash: String,
    pub block_number: i64,
    pub timestamp: DateTime<Utc>,
    pub bridge_type: String,
    pub from_chain: String,
    pub to_chain: String,
    pub from_address: String,
    pub to_address: Option<String>,
    pub token_address: Option<String>,
    pub amount: BigDecimal,
    pub l1_transaction_hash: Option<String>,
    pub l2_transaction_hash: Option<String>,
    pub bridge_contract: String,
    pub status: Option<String>,
    pub proof_submitted: Option<bool>,
    pub finalized: Option<bool>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = bridge_transactions)]
pub struct NewBridgeTransaction {
    pub transaction_hash: String,
    pub block_number: i64,
    pub timestamp: DateTime<Utc>,
    pub bridge_type: String,
    pub from_chain: String,
    pub to_chain: String,
    pub from_address: String,
    pub to_address: Option<String>,
    pub token_address: Option<String>,
    pub amount: BigDecimal,
    pub l1_transaction_hash: Option<String>,
    pub l2_transaction_hash: Option<String>,
    pub bridge_contract: String,
    pub status: Option<String>,
    pub proof_submitted: Option<bool>,
    pub finalized: Option<bool>,
}

#[derive(Debug, Clone, Queryable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = bridge_stats)]
pub struct BridgeStats {
    pub id: i32,
    pub date: NaiveDate,
    pub total_deposits_count: Option<i64>,
    pub total_withdrawals_count: Option<i64>,
    pub total_deposit_volume: Option<BigDecimal>,
    pub total_withdrawal_volume: Option<BigDecimal>,
    pub tvl_eth: Option<BigDecimal>,
    pub unique_depositors: Option<i32>,
    pub unique_withdrawers: Option<i32>,
    pub avg_deposit_size: Option<BigDecimal>,
    pub avg_withdrawal_size: Option<BigDecimal>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = bridge_stats)]
pub struct NewBridgeStats {
    pub date: NaiveDate,
    pub total_deposits_count: Option<i64>,
    pub total_withdrawals_count: Option<i64>,
    pub total_deposit_volume: Option<BigDecimal>,
    pub total_withdrawal_volume: Option<BigDecimal>,
    pub tvl_eth: Option<BigDecimal>,
    pub unique_depositors: Option<i32>,
    pub unique_withdrawers: Option<i32>,
    pub avg_deposit_size: Option<BigDecimal>,
    pub avg_withdrawal_size: Option<BigDecimal>,
}

impl BridgeTransaction {
    pub fn amount_in_eth(&self) -> f64 {
        self.amount.to_string().parse::<f64>().unwrap_or(0.0) / 1e18
    }

    pub fn is_deposit(&self) -> bool {
        self.bridge_type == "deposit"
    }

    pub fn is_withdrawal(&self) -> bool {
        self.bridge_type == "withdrawal"
    }

    pub fn is_pending(&self) -> bool {
        self.status.as_ref().map_or(true, |s| s == "pending")
    }

    pub fn is_completed(&self) -> bool {
        self.status.as_ref().map_or(false, |s| s == "success" || s == "finalized")
    }
}

impl BridgeStats {
    pub fn total_volume_in_eth(&self) -> f64 {
        let deposits = self.total_deposit_volume.clone().unwrap_or_default();
        let withdrawals = self.total_withdrawal_volume.clone().unwrap_or_default();
        let total = &deposits + &withdrawals;
        total.to_string().parse::<f64>().unwrap_or(0.0) / 1e18
    }

    pub fn tvl_in_eth(&self) -> f64 {
        self.tvl_eth.as_ref()
            .map(|tvl| tvl.to_string().parse::<f64>().unwrap_or(0.0) / 1e18)
            .unwrap_or(0.0)
    }

    pub fn net_flow_in_eth(&self) -> f64 {
        let deposits = self.total_deposit_volume.clone().unwrap_or_default();
        let withdrawals = self.total_withdrawal_volume.clone().unwrap_or_default();
        let net = &deposits - &withdrawals;
        net.to_string().parse::<f64>().unwrap_or(0.0) / 1e18
    }
}