use anyhow::Result;
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use ethers::types::{Transaction, TransactionReceipt, Log};
use tracing::{info, warn};

use crate::models::bridge::NewBridgeTransaction;

// Taiko Bridge Contract Addresses
pub const TAIKO_L2_BRIDGE_ADDRESS: &str = "0x1670000000000000000000000000000000000001";
pub const TAIKO_L1_BRIDGE_ADDRESS: &str = "0xd60247c6848b7ca29eddf63aa924e53db6ddd8ec"; // Main L1 bridge

// Bridge event signatures (keccak256 hashes)
pub const MESSAGE_SENT_SIGNATURE: &str = "0x8efab4ba78cc81b7ae98c94e4b33c39e4f06a2c7dd5f71e564ceebf6ed60d989"; // MessageSent event
pub const MESSAGE_RECEIVED_SIGNATURE: &str = "0xe9bded5f24a4168e4f3bf44e00298c993b22376aad8c58c7dda9718a54cbea82"; // MessageReceived event
pub const ETHER_SENT_SIGNATURE: &str = "0x36e8cefb6e7a7b9d5b5f64c0d5d4c7b0a5b9c4d1a7b2e3f4c5d6e7f8a9b0c1d2"; // EtherSent event

#[derive(Debug, Clone)]
pub struct BridgeDetector;

impl BridgeDetector {
    pub fn new() -> Self {
        Self
    }

    /// Detects if a transaction is a bridge transaction and extracts bridge details
    pub fn detect_bridge_transaction(
        &self,
        tx: &Transaction,
        receipt: Option<&TransactionReceipt>,
        block_number: i64,
        timestamp: DateTime<Utc>,
    ) -> Result<Option<NewBridgeTransaction>> {
        // Check if transaction is to a known bridge contract
        let is_bridge_tx = self.is_bridge_contract(&tx.to);
        
        if !is_bridge_tx {
            return Ok(None);
        }

        // Analyze transaction input data and logs to determine bridge type
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

    /// Checks if an address is a known bridge contract
    fn is_bridge_contract(&self, to_address: &Option<ethers::types::Address>) -> bool {
        if let Some(addr) = to_address {
            let addr_str = format!("0x{:x}", addr).to_lowercase();
            
            // Check against known Taiko bridge addresses
            addr_str == TAIKO_L2_BRIDGE_ADDRESS.to_lowercase() ||
            addr_str == TAIKO_L1_BRIDGE_ADDRESS.to_lowercase()
        } else {
            false
        }
    }

    /// Analyzes transaction data to extract bridge details
    fn analyze_bridge_transaction(
        &self,
        tx: &Transaction,
        receipt: Option<&TransactionReceipt>,
    ) -> Result<Option<BridgeAnalysis>> {
        // Check transaction input data for bridge function signatures
        let input_hex = hex::encode(&tx.input);
        
        // Common bridge function signatures
        let bridge_type = if input_hex.starts_with("e3ed56cc") {
            // sendMessage(uint64,address,bytes)
            "deposit"
        } else if input_hex.starts_with("40cdebb4") {
            // processMessage(Message,bytes)
            "withdrawal"
        } else if input_hex.starts_with("1e8e1e13") {
            // proveMessageReceived(Message,bytes)
            "claim"
        } else {
            // Fallback: analyze by value and recipient
            if tx.value > ethers::types::U256::zero() {
                "deposit"
            } else {
                "unknown"
            }
        };

        // Determine chains based on contract address
        let (from_chain, to_chain) = if let Some(to_addr) = tx.to {
            let addr_str = format!("0x{:x}", to_addr).to_lowercase();
            if addr_str == TAIKO_L2_BRIDGE_ADDRESS.to_lowercase() {
                ("taiko", "ethereum")  // L2 -> L1 withdrawal
            } else {
                ("ethereum", "taiko")  // L1 -> L2 deposit
            }
        } else {
            ("unknown", "unknown")
        };

        // Extract amount (ETH value for ETH transfers)
        let amount = BigDecimal::from(tx.value.as_u128());

        // Analyze logs for more detailed information
        let mut status = Some("pending".to_string());
        let mut proof_submitted = Some(false);
        let mut finalized = Some(false);

        if let Some(receipt) = receipt {
            // Check if transaction was successful
            if receipt.status == Some(ethers::types::U64::from(1)) {
                status = Some("success".to_string());
                
                // Analyze logs for bridge events
                for log in &receipt.logs {
                    self.analyze_bridge_log(log, &mut proof_submitted, &mut finalized);
                }
            } else {
                status = Some("failed".to_string());
            }
        }

        // Skip if amount is zero (not a real bridge transaction)
        if amount == BigDecimal::from(0) && bridge_type == "unknown" {
            return Ok(None);
        }

        Ok(Some(BridgeAnalysis {
            bridge_type: bridge_type.to_string(),
            from_chain: from_chain.to_string(),
            to_chain: to_chain.to_string(),
            token_address: None, // ETH transfers don't have token address
            amount,
            l1_transaction_hash: None, // Would need to track cross-chain
            status,
            proof_submitted,
            finalized,
        }))
    }

    /// Analyzes bridge-related logs to extract status information
    fn analyze_bridge_log(
        &self,
        log: &Log,
        proof_submitted: &mut Option<bool>,
        finalized: &mut Option<bool>,
    ) {
        let topic0 = log.topics.get(0).map(|t| format!("0x{:x}", t));
        
        match topic0.as_deref() {
            Some(MESSAGE_SENT_SIGNATURE) => {
                // Message sent successfully
            },
            Some(MESSAGE_RECEIVED_SIGNATURE) => {
                // Message received on destination
                *finalized = Some(true);
            },
            Some(ETHER_SENT_SIGNATURE) => {
                // Ether sent in bridge transaction
            },
            _ => {
                // Check for proof-related events
                if let Some(topic) = &topic0 {
                    if topic.contains("proof") || topic.contains("Proof") {
                        *proof_submitted = Some(true);
                    }
                }
            }
        }
    }

    /// Gets bridge statistics for analytics
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