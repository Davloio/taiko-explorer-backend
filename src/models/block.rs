use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use bigdecimal::BigDecimal;
use diesel::sql_types::Numeric;

#[derive(Queryable, Selectable, Serialize, Deserialize, Debug, Clone)]
#[diesel(table_name = crate::schema::blocks)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Block {
    pub id: i32,
    pub number: i64,
    pub hash: String,
    pub parent_hash: String,
    pub timestamp: i64,
    pub gas_limit: i64,
    pub gas_used: i64,
    pub miner: String,
    #[diesel(sql_type = Numeric)]
    pub difficulty: BigDecimal,
    #[diesel(sql_type = diesel::sql_types::Nullable<Numeric>)]
    pub total_difficulty: Option<BigDecimal>,
    pub size: Option<i64>,
    pub transaction_count: i32,
    pub extra_data: Option<String>,
    pub logs_bloom: Option<String>,
    pub mix_hash: Option<String>,
    pub nonce: Option<String>,
    pub base_fee_per_gas: Option<i64>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = crate::schema::blocks)]
pub struct NewBlock {
    pub number: i64,
    pub hash: String,
    pub parent_hash: String,
    pub timestamp: i64,
    pub gas_limit: i64,
    pub gas_used: i64,
    pub miner: String,
    pub difficulty: BigDecimal,
    pub total_difficulty: Option<BigDecimal>,
    pub size: Option<i64>,
    pub transaction_count: i32,
    pub extra_data: Option<String>,
    pub logs_bloom: Option<String>,
    pub mix_hash: Option<String>,
    pub nonce: Option<String>,
    pub base_fee_per_gas: Option<i64>,
}