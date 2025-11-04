use ethers::prelude::*;
use bigdecimal::BigDecimal;
use crate::models::block::NewBlock;
use std::str::FromStr;

pub fn block_to_new_block(block: &Block<Transaction>) -> NewBlock {
    let difficulty = BigDecimal::from_str(&block.difficulty.to_string()).unwrap_or_else(|_| BigDecimal::from(0));
    
    let total_difficulty = block.total_difficulty
        .map(|td| BigDecimal::from_str(&td.to_string()).unwrap_or_else(|_| BigDecimal::from(0)));

    NewBlock {
        number: block.number.unwrap().as_u64() as i64,
        hash: format!("0x{:x}", block.hash.unwrap()),
        parent_hash: format!("0x{:x}", block.parent_hash),
        timestamp: block.timestamp.as_u64() as i64,
        gas_limit: block.gas_limit.as_u64() as i64,
        gas_used: block.gas_used.as_u64() as i64,
        miner: format!("0x{:x}", block.author.unwrap_or_default()),
        difficulty,
        total_difficulty,
        size: block.size.map(|s| s.as_u64() as i64),
        transaction_count: block.transactions.len() as i32,
        extra_data: Some(hex::encode(&block.extra_data)),
        logs_bloom: block.logs_bloom.map(|lb| format!("0x{:x}", lb)),
        mix_hash: block.mix_hash.map(|mh| format!("0x{:x}", mh)),
        nonce: block.nonce.map(|n| format!("0x{:x}", n)),
        base_fee_per_gas: block.base_fee_per_gas.map(|bf| bf.as_u64() as i64),
    }
}