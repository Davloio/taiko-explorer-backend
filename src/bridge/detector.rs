use anyhow::Result;
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use ethers::types::{Transaction, TransactionReceipt, Log};
use tracing::{info, warn};

use crate::models::bridge::NewBridgeTransaction;

pub const TAIKO_L2_BRIDGE_ADDRESS: &str = "0x1670000000000000000000000000000000000001";
pub const TAIKO_L1_BRIDGE_ADDRESS: &str = "0xd60247c6848b7ca29eddf63aa924e53db6ddd8ec";

pub const MESSAGE_SENT_SIGNATURE: &str = "0x8efab4ba78cc81b7ae98c94e4b33c39e4f06a2c7dd5f71e564ceebf6ed60d989";
pub const MESSAGE_RECEIVED_SIGNATURE: &str = "0xe9bded5f24a4168e4f3bf44e00298c993b22376aad8c58c7dda9718a54cbea82";
pub const ETHER_SENT_SIGNATURE: &str = "0x36e8cefb6e7a7b9d5b5f64c0d5d4c7b0a5b9c4d1a7b2e3f4c5d6e7f8a9b0c1d2";

#[derive(Debug, Clone)]
pub struct BridgeDetector;

impl BridgeDetector {
    pub fn new() -> Self {
        Self
    }

    pub fn detect_bridge_transaction(
        &self,
        tx: &Transaction,
        receipt: Option<&TransactionReceipt>,
        block_number: i64,
        timestamp: DateTime<Utc>,
    ) -> Result<Option<NewBridgeTransaction>> {
        let is_bridge_tx = self.is_bridge_contract(&tx.to);
        
        if !is_bridge_tx {
            return Ok(None);
        }

        let bridge_analysis = self.analyze_bridge_transaction(tx, receipt)?;
        
        if let Some(analysis) = bridge_analysis {
            let bridge_tx = NewBridgeTransaction {
                transaction_hash: format!("0x{:x}", tx.hash),
                block_number,
                timestamp,
                bridge_type: analysis.bridge_type,
                from_chain: analysis.from_chain,
                to_chain: analysis.to_chain,
                from_address: format!("0x{:x}", tx.from),
                to_address: tx.to.map(|addr| format!("0x{:x}", addr)),
                token_address: analysis.token_address,
                amount: analysis.amount,
                l1_transaction_hash: analysis.l1_transaction_hash,
                l2_transaction_hash: Some(format!("0x{:x}", tx.hash)),
                bridge_contract: tx.to.map(|addr| format!("0x{:x}", addr)).unwrap_or_default(),
                status: analysis.status,
                proof_submitted: analysis.proof_submitted,
                finalized: analysis.finalized,
            };
            
            info!("Detected bridge transaction: {} type: {}", 
                  bridge_tx.transaction_hash, bridge_tx.bridge_type);
            
            return Ok(Some(bridge_tx));
        }
        
        Ok(None)
    }

    fn is_bridge_contract(&self, to_address: &Option<ethers::types::Address>) -> bool {
        if let Some(addr) = to_address {
            let addr_str = format!("0x{:x}", addr).to_lowercase();
            
            addr_str == TAIKO_L2_BRIDGE_ADDRESS.to_lowercase() ||
            addr_str == TAIKO_L1_BRIDGE_ADDRESS.to_lowercase()
        } else {
            false
        }
    }

    fn analyze_bridge_transaction(
        &self,
        tx: &Transaction,
        receipt: Option<&TransactionReceipt>,
    ) -> Result<Option<BridgeAnalysis>> {
        let input_hex = hex::encode(&tx.input);
        
        let bridge_type = if input_hex.starts_with("e3ed56cc") {
            "deposit"
        } else if input_hex.starts_with("40cdebb4") {
            "withdrawal"
        } else if input_hex.starts_with("1e8e1e13") {
            "claim"
        } else {
            if tx.value > ethers::types::U256::zero() {
                "deposit"
            } else {
                "unknown"
            }
        };

        let (from_chain, to_chain) = if let Some(to_addr) = tx.to {
            let addr_str = format!("0x{:x}", to_addr).to_lowercase();
            if addr_str == TAIKO_L2_BRIDGE_ADDRESS.to_lowercase() {
                ("taiko", "ethereum")
            } else {
                ("ethereum", "taiko")
            }
        } else {
            ("unknown", "unknown")
        };

        let amount = BigDecimal::from(tx.value.as_u128());

        let mut status = Some("pending".to_string());
        let mut proof_submitted = Some(false);
        let mut finalized = Some(false);

        if let Some(receipt) = receipt {
            if receipt.status == Some(ethers::types::U64::from(1)) {
                status = Some("success".to_string());
                
                for log in &receipt.logs {
                    self.analyze_bridge_log(log, &mut proof_submitted, &mut finalized);
                }
            } else {
                status = Some("failed".to_string());
            }
        }

        if amount == BigDecimal::from(0) && bridge_type == "unknown" {
            return Ok(None);
        }

        Ok(Some(BridgeAnalysis {
            bridge_type: bridge_type.to_string(),
            from_chain: from_chain.to_string(),
            to_chain: to_chain.to_string(),
            token_address: None,
            amount,
            l1_transaction_hash: None,
            status,
            proof_submitted,
            finalized,
        }))
    }

    fn analyze_bridge_log(
        &self,
        log: &Log,
        proof_submitted: &mut Option<bool>,
        finalized: &mut Option<bool>,
    ) {
        let topic0 = log.topics.get(0).map(|t| format!("0x{:x}", t));
        
        match topic0.as_deref() {
            Some(MESSAGE_SENT_SIGNATURE) => {
            },
            Some(MESSAGE_RECEIVED_SIGNATURE) => {
                *finalized = Some(true);
            },
            Some(ETHER_SENT_SIGNATURE) => {
            },
            _ => {
                if let Some(topic) = &topic0 {
                    if topic.contains("proof") || topic.contains("Proof") {
                        *proof_submitted = Some(true);
                    }
                }
            }
        }
    }

    pub fn get_bridge_stats(&self, transactions: &[NewBridgeTransaction]) -> BridgeStatsSummary {
        let mut deposits = 0u64;
        let mut withdrawals = 0u64;
        let mut total_deposit_volume = BigDecimal::from(0);
        let mut total_withdrawal_volume = BigDecimal::from(0);

        for tx in transactions {
            match tx.bridge_type.as_str() {
                "deposit" => {
                    deposits += 1;
                    total_deposit_volume += &tx.amount;
                },
                "withdrawal" => {
                    withdrawals += 1;
                    total_withdrawal_volume += &tx.amount;
                },
                _ => {}
            }
        }

        let net_flow = &total_deposit_volume - &total_withdrawal_volume;
        
        BridgeStatsSummary {
            total_deposits: deposits,
            total_withdrawals: withdrawals,
            total_deposit_volume,
            total_withdrawal_volume,
            net_flow,
        }
    }
}

#[derive(Debug)]
struct BridgeAnalysis {
    bridge_type: String,
    from_chain: String,
    to_chain: String,
    token_address: Option<String>,
    amount: BigDecimal,
    l1_transaction_hash: Option<String>,
    status: Option<String>,
    proof_submitted: Option<bool>,
    finalized: Option<bool>,
}

#[derive(Debug)]
pub struct BridgeStatsSummary {
    pub total_deposits: u64,
    pub total_withdrawals: u64,
    pub total_deposit_volume: BigDecimal,
    pub total_withdrawal_volume: BigDecimal,
    pub net_flow: BigDecimal,
}

impl Default for BridgeDetector {
    fn default() -> Self {
        Self::new()
    }
}