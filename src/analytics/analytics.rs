use anyhow::Result;
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use ethers::types::{Transaction, TransactionReceipt, H256};
use tracing::{info, warn};

use crate::db::address_analytics::AddressAnalyticsRepository;
use crate::bridge::BridgeDetector;
use crate::models::address_analytics::NewAddressStats;
use crate::models::bridge::NewBridgeTransaction;

pub async fn process_block_analytics(
    analytics_repo: &AddressAnalyticsRepository,
    bridge_detector: &BridgeDetector,
    block_number: i64,
    block_timestamp: i64,
    transactions: &[Transaction],
    receipts: &[Option<TransactionReceipt>],
) -> Result<()> {
    let timestamp = DateTime::<Utc>::from_timestamp(block_timestamp, 0).unwrap_or_else(Utc::now);
    
    for (index, tx) in transactions.iter().enumerate() {
        let receipt = receipts.get(index).and_then(|r| r.as_ref());
        
        if let Some(bridge_tx) = detect_bridge_transaction(tx, receipt, block_number, timestamp)? {
            match analytics_repo.insert_bridge_transaction(bridge_tx) {
                Ok(_) => info!("🌉 Bridge transaction detected: {}", format!("0x{:x}", tx.hash)),
                Err(e) => warn!("Failed to store bridge transaction: {}", e),
            }
        }
        
        update_address_stats(
            analytics_repo,
            &format!("0x{:x}", tx.from),
            block_number,
            true, // is_sender
            &tx.value.to_string(),
            tx.gas_price.map(|p| p.to_string()),
            receipt.and_then(|r| r.gas_used).map(|g| g.as_u64()),
            tx.to.as_ref().map(|addr| format!("0x{:x}", addr)),
        );
        
        if let Some(to_addr) = &tx.to {
            update_address_stats(
                analytics_repo,
                &format!("0x{:x}", to_addr),
                block_number,
                false, // is_receiver
                &tx.value.to_string(),
                None,
                None,
                Some(format!("0x{:x}", tx.from)),
            );
        }
    }
    
    Ok(())
}

fn detect_bridge_transaction(
    tx: &Transaction,
    receipt: Option<&TransactionReceipt>,
    block_number: i64,
    timestamp: DateTime<Utc>,
) -> Result<Option<NewBridgeTransaction>> {
    const BRIDGE_CONTRACTS: &[&str] = &[
        "0x1670000000000000000000000000000000000001",  // Taiko Bridge
        "0x1670000000000000000000000000000000010001",  // Taiko Signal Service
    ];
    
    if let Some(to_addr) = &tx.to {
        let to_str = format!("0x{:x}", to_addr).to_lowercase();
        
        for bridge_addr in BRIDGE_CONTRACTS {
            if to_str == bridge_addr.to_lowercase() {
                let bridge_type = if tx.value > ethers::types::U256::zero() {
                    "deposit"
                } else if tx.input.len() > 4 {
                    "message"
                } else {
                    "unknown"
                };
                
                let status = receipt.map(|r| {
                    if r.status == Some(ethers::types::U64::from(1)) {
                        "success"
                    } else {
                        "failed"
                    }
                }).unwrap_or("pending");
                
                return Ok(Some(NewBridgeTransaction {
                    transaction_hash: format!("0x{:x}", tx.hash),
                    block_number,
                    timestamp,
                    bridge_type: bridge_type.to_string(),
                    from_chain: "taiko".to_string(),
                    to_chain: "ethereum".to_string(),
                    from_address: format!("0x{:x}", tx.from),
                    to_address: Some(to_str.clone()),
                    token_address: None, // ETH transfers
                    amount: BigDecimal::from(tx.value.as_u128()),
                    l1_transaction_hash: None,
                    l2_transaction_hash: Some(format!("0x{:x}", tx.hash)),
                    bridge_contract: to_str,
                    status: Some(status.to_string()),
                    proof_submitted: Some(false),
                    finalized: Some(false),
                }));
            }
        }
    }
    
    Ok(None)
}

fn update_address_stats(
    analytics_repo: &AddressAnalyticsRepository,
    address: &str,
    block_number: i64,
    is_sender: bool,
    value: &str,
    gas_price: Option<String>,
    gas_used: Option<u64>,
    counterparty: Option<String>,
) {
    let existing_stats = analytics_repo.get_address_stats(address).ok().flatten();
    
    let value_bigdec = value.parse::<BigDecimal>().unwrap_or_default();
    
    let new_stats = if let Some(mut stats) = existing_stats {
        stats.last_seen_block = block_number;
        stats.total_transactions = stats.total_transactions.map(|t| t + 1).or(Some(1));
        
        if is_sender {
            stats.total_sent_transactions = stats.total_sent_transactions.map(|t| t + 1).or(Some(1));
            stats.total_volume_sent = Some(
                stats.total_volume_sent.unwrap_or_default() + &value_bigdec
            );
            
            if let (Some(gas_used_val), Some(gas_price_str)) = (gas_used, gas_price) {
                if let Ok(gas_price_val) = gas_price_str.parse::<BigDecimal>() {
                    stats.gas_used = Some(stats.gas_used.unwrap_or(0) + gas_used_val as i64);
                    stats.gas_fees_paid = Some(
                        stats.gas_fees_paid.unwrap_or_default() + 
                        (gas_price_val * BigDecimal::from(gas_used_val))
                    );
                }
            }
        } else {
            stats.total_received_transactions = stats.total_received_transactions.map(|t| t + 1).or(Some(1));
            stats.total_volume_received = Some(
                stats.total_volume_received.unwrap_or_default() + &value_bigdec
            );
        }
        
        NewAddressStats {
            address: stats.address,
            first_seen_block: stats.first_seen_block,
            last_seen_block: stats.last_seen_block,
            total_transactions: stats.total_transactions,
            total_sent_transactions: stats.total_sent_transactions,
            total_received_transactions: stats.total_received_transactions,
            total_volume_sent: stats.total_volume_sent,
            total_volume_received: stats.total_volume_received,
            gas_used: stats.gas_used,
            gas_fees_paid: stats.gas_fees_paid,
            unique_counterparties: stats.unique_counterparties,
            contract_deployments: stats.contract_deployments,
        }
    } else {
        NewAddressStats {
            address: address.to_string(),
            first_seen_block: block_number,
            last_seen_block: block_number,
            total_transactions: Some(1),
            total_sent_transactions: if is_sender { Some(1) } else { Some(0) },
            total_received_transactions: if !is_sender { Some(1) } else { Some(0) },
            total_volume_sent: if is_sender { Some(value_bigdec.clone()) } else { Some(BigDecimal::from(0)) },
            total_volume_received: if !is_sender { Some(value_bigdec) } else { Some(BigDecimal::from(0)) },
            gas_used: if is_sender { gas_used.map(|g| g as i64) } else { Some(0) },
            gas_fees_paid: Some(BigDecimal::from(0)),
            unique_counterparties: Some(1),
            contract_deployments: Some(0),
        }
    };
    
    match analytics_repo.upsert_address_stats(new_stats) {
        Ok(_) => {},
        Err(e) => warn!("Failed to update address stats for {}: {}", address, e),
    }
}

