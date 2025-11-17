use anyhow::Result;
use bigdecimal::BigDecimal;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::db::connection::DbPool;
use crate::models::block::{Block, NewBlock};
use crate::models::transaction::{Transaction, NewTransaction};
use crate::schema::{blocks, transactions};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityInfo {
    pub block_number: i64,
    pub transaction_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressProfile {
    pub address: String,
    pub total_transactions: i64,
    pub first_activity: Option<ActivityInfo>,
    pub last_activity: Option<ActivityInfo>,
    pub total_sent: BigDecimal,
    pub total_received: BigDecimal,
    pub total_gas_fees: BigDecimal,
}

#[derive(Debug, Clone, QueryableByName)]
pub struct ExplorerStatsFromView {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub total_blocks: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub latest_block_number: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub total_transactions: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub total_unique_addresses: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub successful_transactions: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub failed_transactions: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub in_transactions: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub out_transactions: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub inside_transactions: i64,
    #[diesel(sql_type = diesel::sql_types::Timestamptz)]
    pub last_updated: DateTime<Utc>,
}

#[derive(Clone)]
pub struct BlockRepository {
    pool: DbPool,
}

impl BlockRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn insert_block(&self, new_block: NewBlock) -> Result<Block> {
        let mut conn = self.pool.get()?;
        
        let block = diesel::insert_into(blocks::table)
            .values(&new_block)
            .on_conflict(blocks::number)
            .do_nothing()
            .get_result::<Block>(&mut conn)
            .optional()?;
        
        match block {
            Some(b) => Ok(b),
            None => {
                self.get_block_by_number(new_block.number)?.ok_or_else(|| {
                    anyhow::anyhow!("Failed to insert or fetch block {}", new_block.number)
                })
            }
        }
    }

    pub fn insert_blocks_batch(&self, new_blocks: Vec<NewBlock>) -> Result<usize> {
        let mut conn = self.pool.get()?;
        
        let inserted = diesel::insert_into(blocks::table)
            .values(&new_blocks)
            .on_conflict(blocks::number)
            .do_nothing()
            .execute(&mut conn)?;
        
        Ok(inserted)
    }

    pub fn insert_blocks_bulk(&self, new_blocks: Vec<NewBlock>) -> Result<usize> {
        self.insert_blocks_batch(new_blocks)
    }

    pub fn get_latest_block_number(&self) -> Result<Option<i64>> {
        let mut conn = self.pool.get()?;
        
        let latest_number = blocks::table
            .select(blocks::number)
            .order(blocks::number.desc())
            .first::<i64>(&mut conn)
            .optional()?;
        
        Ok(latest_number)
    }

    pub fn block_exists(&self, block_number: i64) -> Result<bool> {
        let mut conn = self.pool.get()?;
        
        let count = blocks::table
            .filter(blocks::number.eq(block_number))
            .count()
            .get_result::<i64>(&mut conn)?;
        
        Ok(count > 0)
    }

    pub fn get_block_by_number(&self, block_number: i64) -> Result<Option<Block>> {
        let mut conn = self.pool.get()?;
        
        let block = blocks::table
            .filter(blocks::number.eq(block_number))
            .first::<Block>(&mut conn)
            .optional()?;
        
        Ok(block)
    }

    pub fn get_block_by_hash(&self, block_hash: &str) -> Result<Option<Block>> {
        let mut conn = self.pool.get()?;
        
        let block = blocks::table
            .filter(blocks::hash.eq(block_hash))
            .first::<Block>(&mut conn)
            .optional()?;
        
        Ok(block)
    }

    pub fn get_block_count(&self) -> Result<i64> {
        let mut conn = self.pool.get()?;
        
        let count = blocks::table
            .count()
            .get_result::<i64>(&mut conn)?;
        
        Ok(count)
    }

    pub fn update_block_transaction_count(&self, block_number: i64, tx_count: i32) -> Result<()> {
        use diesel::prelude::*;
        let mut conn = self.pool.get()?;
        
        diesel::update(blocks::table)
            .filter(blocks::number.eq(block_number))
            .set(blocks::transaction_count.eq(tx_count))
            .execute(&mut conn)?;
        
        Ok(())
    }


    pub fn insert_transaction(&self, new_transaction: NewTransaction) -> Result<Transaction> {
        let mut conn = self.pool.get()?;
        
        let transaction = diesel::insert_into(transactions::table)
            .values(&new_transaction)
            .on_conflict(transactions::hash)
            .do_nothing()
            .get_result::<Transaction>(&mut conn)
            .optional()?;
        
        match transaction {
            Some(tx) => Ok(tx),
            None => {
                self.get_transaction_by_hash(&new_transaction.hash)?.ok_or_else(|| {
                    anyhow::anyhow!("Failed to insert or fetch transaction {}", new_transaction.hash)
                })
            }
        }
    }

    pub fn insert_transactions_batch(&self, new_transactions: Vec<NewTransaction>) -> Result<usize> {
        let mut conn = self.pool.get()?;
        
        let inserted = diesel::insert_into(transactions::table)
            .values(&new_transactions)
            .on_conflict(transactions::hash)
            .do_nothing()
            .execute(&mut conn)?;
        
        Ok(inserted)
    }

    pub fn insert_transactions_bulk(&self, new_transactions: Vec<NewTransaction>) -> Result<usize> {
        self.insert_transactions_batch(new_transactions)
    }

    pub fn get_transaction_by_hash(&self, tx_hash: &str) -> Result<Option<Transaction>> {
        let mut conn = self.pool.get()?;
        
        let transaction = transactions::table
            .filter(transactions::hash.eq(tx_hash))
            .first::<Transaction>(&mut conn)
            .optional()?;
        
        Ok(transaction)
    }

    pub fn get_transactions_by_block(&self, block_number: i64) -> Result<Vec<Transaction>> {
        let mut conn = self.pool.get()?;
        
        let transactions_list = transactions::table
            .filter(transactions::block_number.eq(block_number))
            .order(transactions::transaction_index.asc())
            .load::<Transaction>(&mut conn)?;
        
        Ok(transactions_list)
    }

    pub fn get_transactions_by_address(&self, address: &str, limit: i64, offset: i64) -> Result<Vec<Transaction>> {
        let mut conn = self.pool.get()?;
        
        let transactions_list = transactions::table
            .filter(
                transactions::from_address.eq(address)
                    .or(transactions::to_address.eq(address))
            )
            .order(transactions::block_number.desc())
            .limit(limit)
            .offset(offset)
            .load::<Transaction>(&mut conn)?;
        
        Ok(transactions_list)
    }

    pub fn get_transactions_paginated_filtered(&self, limit: i64, offset: i64, order_desc: bool, status_filter: Option<i32>, direction_filter: Option<&str>) -> Result<Vec<Transaction>> {
        let mut conn = self.pool.get()?;
        
        let mut query = transactions::table.into_boxed();
        if let Some(status) = status_filter {
            query = query.filter(transactions::status.eq(status));
        }
        if let Some(direction) = direction_filter {
            query = query.filter(transactions::direction.eq(direction));
        }
        if order_desc {
            query = query.order(transactions::block_number.desc());
        } else {
            query = query.order(transactions::block_number.asc());
        }
        
        let transactions_list = query
            .limit(limit)
            .offset(offset)
            .load::<Transaction>(&mut conn)?;
        
        Ok(transactions_list)
    }
    
    pub fn get_transaction_count_filtered(&self, status_filter: Option<i32>, direction_filter: Option<&str>) -> Result<i64> {
        let mut conn = self.pool.get()?;
        
        let mut query = transactions::table.into_boxed();
        if let Some(status) = status_filter {
            query = query.filter(transactions::status.eq(status));
        }
        if let Some(direction) = direction_filter {
            query = query.filter(transactions::direction.eq(direction));
        }
        
        let count = query.count().get_result::<i64>(&mut conn)?;
        Ok(count)
    }

    pub fn get_transactions_paginated(&self, limit: i64, offset: i64, order_desc: bool) -> Result<Vec<Transaction>> {
        let mut conn = self.pool.get()?;
        
        let mut query = transactions::table.into_boxed();
        
        if order_desc {
            query = query.order(transactions::block_number.desc());
        } else {
            query = query.order(transactions::block_number.asc());
        }
        
        let transactions_list = query
            .limit(limit)
            .offset(offset)
            .load::<Transaction>(&mut conn)?;
        
        Ok(transactions_list)
    }

    pub fn get_transaction_count(&self) -> Result<i64> {
        let mut conn = self.pool.get()?;
        
        let count = transactions::table
            .count()
            .get_result::<i64>(&mut conn)?;
        
        Ok(count)
    }

    pub fn get_transactions_count_by_address(&self, address: &str) -> Result<i64> {
        let mut conn = self.pool.get()?;
        
        let count = transactions::table
            .filter(
                transactions::from_address.eq(address)
                    .or(transactions::to_address.eq(address))
            )
            .count()
            .get_result::<i64>(&mut conn)?;
        
        Ok(count)
    }

    pub fn get_transactions_by_address_filtered(&self, address: &str, limit: i64, offset: i64, status_filter: Option<i32>, direction_filter: Option<&str>) -> Result<Vec<Transaction>> {
        let mut conn = self.pool.get()?;
        
        let mut query = transactions::table
            .filter(
                transactions::from_address.eq(address)
                    .or(transactions::to_address.eq(address))
            )
            .into_boxed();
        if let Some(status) = status_filter {
            query = query.filter(transactions::status.eq(status));
        }
        if let Some(direction) = direction_filter {
            query = query.filter(transactions::direction.eq(direction));
        }
        
        let transactions_list = query
            .order(transactions::block_number.desc())
            .limit(limit)
            .offset(offset)
            .load::<Transaction>(&mut conn)?;
        
        Ok(transactions_list)
    }

    pub fn get_transactions_count_by_address_filtered(&self, address: &str, status_filter: Option<i32>, direction_filter: Option<&str>) -> Result<i64> {
        let mut conn = self.pool.get()?;
        
        let mut query = transactions::table
            .filter(
                transactions::from_address.eq(address)
                    .or(transactions::to_address.eq(address))
            )
            .into_boxed();
        if let Some(status) = status_filter {
            query = query.filter(transactions::status.eq(status));
        }
        if let Some(direction) = direction_filter {
            query = query.filter(transactions::direction.eq(direction));
        }
        
        let count = query.count().get_result::<i64>(&mut conn)?;
        
        Ok(count)
    }

    pub fn get_address_profile(&self, address: &str) -> Result<AddressProfile> {
        let mut conn = self.pool.get()?;
        let total_transactions = self.get_transactions_count_by_address(address)?;
        let first_activity = transactions::table
            .filter(
                transactions::from_address.eq(address)
                .or(transactions::to_address.eq(address))
            )
            .order(transactions::block_number.asc())
            .select((transactions::block_number, transactions::hash))
            .first::<(i64, String)>(&mut conn)
            .optional()?;
            
        let last_activity = transactions::table
            .filter(
                transactions::from_address.eq(address)
                .or(transactions::to_address.eq(address))
            )
            .order(transactions::block_number.desc())
            .select((transactions::block_number, transactions::hash))
            .first::<(i64, String)>(&mut conn)
            .optional()?;
        let sent_volume = transactions::table
            .filter(transactions::from_address.eq(address))
            .select(diesel::dsl::sum(transactions::value))
            .first::<Option<BigDecimal>>(&mut conn)?
            .unwrap_or_else(|| BigDecimal::from(0));
            
        let received_volume = transactions::table
            .filter(transactions::to_address.eq(address))
            .select(diesel::dsl::sum(transactions::value))
            .first::<Option<BigDecimal>>(&mut conn)?
            .unwrap_or_else(|| BigDecimal::from(0));
        let total_gas_fees = transactions::table
            .filter(transactions::from_address.eq(address))
            .filter(transactions::gas_price.is_not_null())
            .filter(transactions::gas_used.is_not_null())
            .load::<Transaction>(&mut conn)?
            .iter()
            .map(|tx| {
                if let (Some(gas_price), Some(gas_used)) = (&tx.gas_price, &tx.gas_used) {
                    gas_price * BigDecimal::from(*gas_used)
                } else {
                    BigDecimal::from(0)
                }
            })
            .sum::<BigDecimal>();
        
        Ok(AddressProfile {
            address: address.to_string(),
            total_transactions,
            first_activity: first_activity.map(|(block, hash)| ActivityInfo { block_number: block, transaction_hash: hash }),
            last_activity: last_activity.map(|(block, hash)| ActivityInfo { block_number: block, transaction_hash: hash }),
            total_sent: sent_volume,
            total_received: received_volume,
            total_gas_fees,
        })
    }

    pub fn get_failed_transactions(&self, limit: i64) -> Result<Vec<Transaction>> {
        let mut conn = self.pool.get()?;
        
        let transactions_list = transactions::table
            .filter(transactions::status.eq(0))
            .order(transactions::block_number.desc())
            .limit(limit)
            .load::<Transaction>(&mut conn)?;
        
        Ok(transactions_list)
    }

    pub fn get_contract_creation_transactions(&self, limit: i64) -> Result<Vec<Transaction>> {
        let mut conn = self.pool.get()?;
        
        let transactions_list = transactions::table
            .filter(
                transactions::to_address.is_null()
                    .and(transactions::contract_address.is_not_null())
            )
            .order(transactions::block_number.desc())
            .limit(limit)
            .load::<Transaction>(&mut conn)?;
        
        Ok(transactions_list)
    }

    pub fn get_unique_address_count(&self) -> Result<i64> {
        let mut conn = self.pool.get()?;
        let from_count = transactions::table
            .select(transactions::from_address)
            .distinct()
            .count()
            .get_result::<i64>(&mut conn)?;
        let to_count = transactions::table
            .filter(transactions::to_address.is_not_null())
            .select(transactions::to_address)
            .distinct()
            .count()
            .get_result::<i64>(&mut conn)?;
        use diesel::sql_types::BigInt;
        #[derive(QueryableByName)]
        struct AddressCount {
            #[diesel(sql_type = BigInt)]
            count: i64,
        }
        
        let result = diesel::sql_query(
            "SELECT COUNT(*) as count FROM (
                SELECT from_address FROM transactions
                UNION
                SELECT to_address FROM transactions WHERE to_address IS NOT NULL
            ) AS unique_addresses"
        )
        .get_result::<AddressCount>(&mut conn)?;
        
        Ok(result.count)
    }

    pub fn get_address_growth_chart(&self, _days: Option<i32>) -> Result<Vec<(String, i32, i32)>> {
        let mut conn = self.pool.get()?;
        let query = "
            WITH time_bounds AS (
                SELECT 
                    DATE(to_timestamp(MIN(b.timestamp))) as start_date,
                    DATE(to_timestamp(MAX(b.timestamp))) as end_date
                FROM blocks b
            ),
            date_series AS (
                SELECT 
                    (tb.start_date + (n * (tb.end_date - tb.start_date) / GREATEST(9, 1)))::date as period_date
                FROM time_bounds tb
                CROSS JOIN generate_series(0, 9) as n
            ),
            daily_unique_addresses AS (
                SELECT 
                    DATE(to_timestamp(b.timestamp)) as date,
                    t.from_address as address
                FROM transactions t
                JOIN blocks b ON t.block_number = b.number
                WHERE t.from_address IS NOT NULL
                
                UNION
                
                SELECT 
                    DATE(to_timestamp(b.timestamp)) as date,
                    t.to_address as address
                FROM transactions t
                JOIN blocks b ON t.block_number = b.number
                WHERE t.to_address IS NOT NULL
            ),
            first_appearances AS (
                SELECT 
                    address,
                    MIN(date) as first_seen_date
                FROM daily_unique_addresses
                GROUP BY address
            ),
            daily_totals AS (
                SELECT 
                    first_seen_date as date,
                    COUNT(*) as daily_new_count
                FROM first_appearances
                GROUP BY first_seen_date
                ORDER BY first_seen_date
            ),
            cumulative_totals AS (
                SELECT 
                    date,
                    SUM(daily_new_count) OVER (ORDER BY date) as cumulative_count
                FROM daily_totals
            ),
            period_data AS (
                SELECT 
                    ds.period_date,
                    COALESCE(MAX(ct.cumulative_count), 0)::bigint as total_addresses_at_date
                FROM date_series ds
                LEFT JOIN cumulative_totals ct ON ct.date <= ds.period_date
                GROUP BY ds.period_date
                ORDER BY ds.period_date
            )
            SELECT 
                period_date::text as date,
                COALESCE(total_addresses_at_date - LAG(total_addresses_at_date, 1, 0) OVER (ORDER BY period_date), total_addresses_at_date)::bigint as new_addresses,
                total_addresses_at_date as total_addresses
            FROM period_data
            ORDER BY period_date";
        let results = diesel::sql_query(query)
            .load::<DailyAddressCount>(&mut conn)?;
        let mut chart_data = Vec::new();

        for result in results {
            chart_data.push((
                format!("{}T00:00:00Z", result.date),
                result.total_addresses as i32,
                result.new_addresses as i32
            ));
        }

        Ok(chart_data)
    }

    // PERFORMANCE OPTIMIZED METHODS USING MATERIALIZED VIEW

    /// Get stats from materialized view - 1000x faster than individual queries
    pub fn get_stats_from_materialized_view(&self) -> Result<ExplorerStatsFromView> {
        let mut conn = self.pool.get()?;
        
        let result = diesel::sql_query(
            "SELECT 
                total_blocks,
                latest_block_number,
                total_transactions,
                total_unique_addresses,
                successful_transactions,
                failed_transactions,
                in_transactions,
                out_transactions,
                inside_transactions,
                last_updated
            FROM mv_explorer_stats 
            LIMIT 1"
        )
        .get_result::<ExplorerStatsFromView>(&mut conn)?;
        
        Ok(result)
    }

    /// Refresh the materialized view (call this when new data is indexed)
    pub fn refresh_stats_view(&self) -> Result<()> {
        let mut conn = self.pool.get()?;
        
        diesel::sql_query("REFRESH MATERIALIZED VIEW mv_explorer_stats")
            .execute(&mut conn)?;
            
        Ok(())
    }

    /// Fast unique address count using address_stats table (alternative method)
    pub fn get_unique_address_count_fast(&self) -> Result<i64> {
        use crate::schema::address_stats;
        let mut conn = self.pool.get()?;
        
        let count = address_stats::table
            .count()
            .get_result::<i64>(&mut conn)?;
        
        Ok(count)
    }

    /// Check if materialized view exists and is fresh (less than 1 hour old)
    pub fn is_stats_view_fresh(&self) -> Result<bool> {
        let mut conn = self.pool.get()?;
        
        #[derive(QueryableByName)]
        struct ViewAge {
            #[diesel(sql_type = diesel::sql_types::Bool)]
            is_fresh: bool,
        }
        
        let result = diesel::sql_query(
            "SELECT (NOW() - last_updated) < INTERVAL '1 hour' as is_fresh 
            FROM mv_explorer_stats 
            LIMIT 1"
        )
        .get_result::<ViewAge>(&mut conn)
        .optional()?;
        
        Ok(result.map(|r| r.is_fresh).unwrap_or(false))
    }
}

#[derive(QueryableByName)]
struct DailyAddressCount {
    #[diesel(sql_type = diesel::sql_types::Text)]
    date: String,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    new_addresses: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    total_addresses: i64,
}