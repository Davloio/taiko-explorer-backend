use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::schema::transactions;

#[derive(Debug, Clone, Queryable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = transactions)]
pub struct Transaction {
    pub id: i32,
    pub hash: String,
    pub block_number: i64,
    pub block_hash: String,
    pub transaction_index: i32,
    pub from_address: String,
    pub to_address: Option<String>,
    pub value: BigDecimal,
    pub gas_limit: i64,
    pub gas_used: Option<i64>,
    pub gas_price: Option<BigDecimal>,
    pub max_fee_per_gas: Option<BigDecimal>,
    pub max_priority_fee_per_gas: Option<BigDecimal>,
    pub nonce: i64,
    pub input_data: Option<String>,
    pub status: Option<i32>,
    pub contract_address: Option<String>,
    pub logs_count: Option<i32>,
    pub cumulative_gas_used: Option<i64>,
    pub effective_gas_price: Option<BigDecimal>,
    pub transaction_type: Option<i32>,
    pub access_list: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub direction: String,
}

#[derive(Debug, Clone, Insertable, Serialize, Deserialize)]
#[diesel(table_name = transactions)]
pub struct NewTransaction {
    pub hash: String,
    pub block_number: i64,
    pub block_hash: String,
    pub transaction_index: i32,
    pub from_address: String,
    pub to_address: Option<String>,
    pub value: BigDecimal,
    pub gas_limit: i64,
    pub gas_used: Option<i64>,
    pub gas_price: Option<BigDecimal>,
    pub max_fee_per_gas: Option<BigDecimal>,
    pub max_priority_fee_per_gas: Option<BigDecimal>,
    pub nonce: i64,
    pub input_data: Option<String>,
    pub status: Option<i32>,
    pub contract_address: Option<String>,
    pub logs_count: Option<i32>,
    pub cumulative_gas_used: Option<i64>,
    pub effective_gas_price: Option<BigDecimal>,
    pub transaction_type: Option<i32>,
    pub access_list: Option<String>,
    pub direction: String,
}

impl Transaction {
    pub fn is_contract_creation(&self) -> bool {
        self.to_address.is_none() && self.contract_address.is_some()
    }
    
    pub fn is_outbound(&self) -> bool {
        self.direction == "out"
    }
    
    pub fn is_inbound(&self) -> bool {
        self.direction == "in"
    }
    
    pub fn is_internal(&self) -> bool {
        self.direction == "inside"
    }
    
    pub fn is_successful(&self) -> bool {
        self.status == Some(1)
    }
    
    pub fn is_failed(&self) -> bool {
        self.status == Some(0)
    }
    
    pub fn value_in_eth(&self) -> f64 {
        let wei_per_eth = BigDecimal::from(10_i64.pow(18));
        let value_f64 = self.value.to_string().parse::<f64>().unwrap_or(0.0);
        let eth_divisor = wei_per_eth.to_string().parse::<f64>().unwrap_or(1.0);
        value_f64 / eth_divisor
    }
    
    pub fn gas_price_in_gwei(&self) -> Option<f64> {
        self.gas_price.as_ref().map(|price| {
            let gwei_divisor = BigDecimal::from(10_i64.pow(9));
            let price_f64 = price.to_string().parse::<f64>().unwrap_or(0.0);
            let gwei_divisor_f64 = gwei_divisor.to_string().parse::<f64>().unwrap_or(1.0);
            price_f64 / gwei_divisor_f64
        })
    }
    
    pub fn transaction_fee(&self) -> Option<BigDecimal> {
        match (self.gas_used, &self.effective_gas_price) {
            (Some(gas_used), Some(gas_price)) => {
                Some(BigDecimal::from(gas_used) * gas_price)
            }
            _ => None,
        }
    }
    
    pub fn transaction_fee_in_eth(&self) -> Option<f64> {
        self.transaction_fee().map(|fee| {
            let wei_per_eth = BigDecimal::from(10_i64.pow(18));
            let fee_f64 = fee.to_string().parse::<f64>().unwrap_or(0.0);
            let eth_divisor = wei_per_eth.to_string().parse::<f64>().unwrap_or(1.0);
            fee_f64 / eth_divisor
        })
    }
}