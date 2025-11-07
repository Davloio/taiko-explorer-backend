use bigdecimal::BigDecimal;
use chrono::{DateTime, NaiveDate, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::schema::{address_balances, address_labels, address_stats};

#[derive(Debug, Clone, Queryable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = address_labels)]
pub struct AddressLabel {
    pub id: i32,
    pub address: String,
    pub label: String,
    pub category: String,
    pub description: Option<String>,
    pub verified: Option<bool>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = address_labels)]
pub struct NewAddressLabel {
    pub address: String,
    pub label: String,
    pub category: String,
    pub description: Option<String>,
    pub verified: Option<bool>,
}

#[derive(Debug, Clone, Queryable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = address_balances)]
pub struct AddressBalance {
    pub id: i32,
    pub address: String,
    pub balance: BigDecimal,
    pub block_number: i64,
    pub timestamp: DateTime<Utc>,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = address_balances)]
pub struct NewAddressBalance {
    pub address: String,
    pub balance: BigDecimal,
    pub block_number: i64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Queryable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = address_stats)]
pub struct AddressStats {
    pub id: i32,
    pub address: String,
    pub first_seen_block: i64,
    pub last_seen_block: i64,
    pub total_transactions: Option<i64>,
    pub total_sent_transactions: Option<i64>,
    pub total_received_transactions: Option<i64>,
    pub total_volume_sent: Option<BigDecimal>,
    pub total_volume_received: Option<BigDecimal>,
    pub gas_used: Option<i64>,
    pub gas_fees_paid: Option<BigDecimal>,
    pub unique_counterparties: Option<i32>,
    pub contract_deployments: Option<i32>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = address_stats)]
pub struct NewAddressStats {
    pub address: String,
    pub first_seen_block: i64,
    pub last_seen_block: i64,
    pub total_transactions: Option<i64>,
    pub total_sent_transactions: Option<i64>,
    pub total_received_transactions: Option<i64>,
    pub total_volume_sent: Option<BigDecimal>,
    pub total_volume_received: Option<BigDecimal>,
    pub gas_used: Option<i64>,
    pub gas_fees_paid: Option<BigDecimal>,
    pub unique_counterparties: Option<i32>,
    pub contract_deployments: Option<i32>,
}

impl AddressStats {
    pub fn total_volume(&self) -> BigDecimal {
        let sent = self.total_volume_sent.clone().unwrap_or_default();
        let received = self.total_volume_received.clone().unwrap_or_default();
        &sent + &received
    }

    pub fn total_volume_in_eth(&self) -> f64 {
        let total_wei = self.total_volume();
        total_wei.to_string().parse::<f64>().unwrap_or(0.0) / 1e18
    }

    pub fn gas_fees_in_eth(&self) -> f64 {
        self.gas_fees_paid.as_ref()
            .map(|fees| fees.to_string().parse::<f64>().unwrap_or(0.0) / 1e18)
            .unwrap_or(0.0)
    }
}