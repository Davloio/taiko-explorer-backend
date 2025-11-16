use anyhow::Result;
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use diesel::prelude::*;

use crate::db::connection::DbPool;
use crate::models::address_analytics::{AddressStats, NewAddressStats};
use crate::models::bridge::{BridgeStats, BridgeTransaction, NewBridgeTransaction};
use crate::schema::{address_stats, bridge_stats, bridge_transactions, transactions};

#[derive(Clone)]
pub struct AddressAnalyticsRepository {
    pool: DbPool,
}

impl AddressAnalyticsRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }


    pub fn get_address_stats(&self, address: &str) -> Result<Option<AddressStats>> {
        let mut conn = self.pool.get()?;
        
        let stats = address_stats::table
            .filter(address_stats::address.eq(address))
            .first::<AddressStats>(&mut conn)
            .optional()?;
        
        Ok(stats)
    }

    pub fn upsert_address_stats(&self, new_stats: NewAddressStats) -> Result<AddressStats> {
        let mut conn = self.pool.get()?;
        
        let stats = diesel::insert_into(address_stats::table)
            .values(&new_stats)
            .on_conflict(address_stats::address)
            .do_update()
            .set((
                address_stats::last_seen_block.eq(&new_stats.last_seen_block),
                address_stats::total_transactions.eq(&new_stats.total_transactions),
                address_stats::total_sent_transactions.eq(&new_stats.total_sent_transactions),
                address_stats::total_received_transactions.eq(&new_stats.total_received_transactions),
                address_stats::total_volume_sent.eq(&new_stats.total_volume_sent),
                address_stats::total_volume_received.eq(&new_stats.total_volume_received),
                address_stats::updated_at.eq(chrono::Utc::now()),
            ))
            .get_result::<AddressStats>(&mut conn)?;
        
        Ok(stats)
    }

    pub fn get_top_addresses_by_volume(&self, limit: i64) -> Result<Vec<AddressStats>> {
        let mut conn = self.pool.get()?;
        
        let addresses = address_stats::table
            .filter(address_stats::total_volume_sent.is_not_null())
            .filter(address_stats::total_volume_received.is_not_null())
            .order(
                (address_stats::total_volume_sent + address_stats::total_volume_received).desc()
            )
            .limit(limit)
            .load::<AddressStats>(&mut conn)?;
        
        Ok(addresses)
    }

    pub fn get_top_addresses_by_transactions(&self, limit: i64) -> Result<Vec<AddressStats>> {
        let mut conn = self.pool.get()?;
        
        let addresses = address_stats::table
            .filter(address_stats::total_transactions.is_not_null())
            .order(address_stats::total_transactions.desc())
            .limit(limit)
            .load::<AddressStats>(&mut conn)?;
        
        Ok(addresses)
    }

    pub fn compute_address_stats_from_transactions(&self, address: &str) -> Result<Option<NewAddressStats>> {
        let mut conn = self.pool.get()?;
        
        let sent_count = transactions::table
            .filter(transactions::from_address.eq(address))
            .count()
            .get_result::<i64>(&mut conn)?;
        
        let received_count = transactions::table
            .filter(transactions::to_address.eq(address))
            .count()
            .get_result::<i64>(&mut conn)?;
        
        let total_transactions = sent_count + received_count;
        
        if total_transactions == 0 {
            return Ok(None);
        }
        
        let sent_volume = transactions::table
            .filter(transactions::from_address.eq(address))
            .select(diesel::dsl::sum(transactions::value))
            .first::<Option<BigDecimal>>(&mut conn)?
            .unwrap_or_default();
        
        let received_volume = transactions::table
            .filter(transactions::to_address.eq(address))
            .select(diesel::dsl::sum(transactions::value))
            .first::<Option<BigDecimal>>(&mut conn)?
            .unwrap_or_default();
        
        let first_block = transactions::table
            .filter(
                transactions::from_address.eq(address)
                .or(transactions::to_address.eq(address))
            )
            .select(diesel::dsl::min(transactions::block_number))
            .first::<Option<i64>>(&mut conn)?
            .unwrap_or(0);
        
        let last_block = transactions::table
            .filter(
                transactions::from_address.eq(address)
                .or(transactions::to_address.eq(address))
            )
            .select(diesel::dsl::max(transactions::block_number))
            .first::<Option<i64>>(&mut conn)?
            .unwrap_or(0);
        
        Ok(Some(NewAddressStats {
            address: address.to_string(),
            first_seen_block: first_block,
            last_seen_block: last_block,
            total_transactions: Some(total_transactions),
            total_sent_transactions: Some(sent_count),
            total_received_transactions: Some(received_count),
            total_volume_sent: Some(sent_volume),
            total_volume_received: Some(received_volume),
            gas_used: None,
            gas_fees_paid: None,
            unique_counterparties: None,
            contract_deployments: None,
        }))
    }


    pub fn insert_bridge_transaction(&self, new_bridge_tx: NewBridgeTransaction) -> Result<BridgeTransaction> {
        let mut conn = self.pool.get()?;
        
        let bridge_tx = diesel::insert_into(bridge_transactions::table)
            .values(&new_bridge_tx)
            .on_conflict(bridge_transactions::transaction_hash)
            .do_nothing()
            .get_result::<BridgeTransaction>(&mut conn)
            .optional()?;
        
        match bridge_tx {
            Some(tx) => Ok(tx),
            None => {
                self.get_bridge_transaction_by_hash(&new_bridge_tx.transaction_hash)?
                    .ok_or_else(|| anyhow::anyhow!("Failed to insert or fetch bridge transaction"))
            }
        }
    }

    pub fn get_bridge_transaction_by_hash(&self, tx_hash: &str) -> Result<Option<BridgeTransaction>> {
        let mut conn = self.pool.get()?;
        
        let bridge_tx = bridge_transactions::table
            .filter(bridge_transactions::transaction_hash.eq(tx_hash))
            .first::<BridgeTransaction>(&mut conn)
            .optional()?;
        
        Ok(bridge_tx)
    }

    pub fn get_bridge_transactions(&self, limit: i64, offset: i64) -> Result<Vec<BridgeTransaction>> {
        let mut conn = self.pool.get()?;
        
        let transactions = bridge_transactions::table
            .order(bridge_transactions::block_number.desc())
            .limit(limit)
            .offset(offset)
            .load::<BridgeTransaction>(&mut conn)?;
        
        Ok(transactions)
    }

    pub fn get_bridge_transactions_by_address(&self, address: &str, limit: i64) -> Result<Vec<BridgeTransaction>> {
        let mut conn = self.pool.get()?;
        
        let transactions = bridge_transactions::table
            .filter(
                bridge_transactions::from_address.eq(address)
                .or(bridge_transactions::to_address.eq(address))
            )
            .order(bridge_transactions::block_number.desc())
            .limit(limit)
            .load::<BridgeTransaction>(&mut conn)?;
        
        Ok(transactions)
    }

    pub fn get_bridge_volume_by_date(&self, days: i32) -> Result<Vec<BridgeStats>> {
        let mut conn = self.pool.get()?;
        
        let start_date = chrono::Utc::now().date_naive() - chrono::Duration::days(days as i64);
        
        let stats = bridge_stats::table
            .filter(bridge_stats::date.ge(start_date))
            .order(bridge_stats::date.desc())
            .load::<BridgeStats>(&mut conn)?;
        
        Ok(stats)
    }
}