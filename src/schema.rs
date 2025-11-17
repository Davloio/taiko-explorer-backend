// @generated automatically by Diesel CLI.

diesel::table! {
    address_stats (id) {
        id -> Int4,
        #[max_length = 42]
        address -> Varchar,
        first_seen_block -> Int8,
        last_seen_block -> Int8,
        total_transactions -> Nullable<Int8>,
        total_sent_transactions -> Nullable<Int8>,
        total_received_transactions -> Nullable<Int8>,
        total_volume_sent -> Nullable<Numeric>,
        total_volume_received -> Nullable<Numeric>,
        gas_used -> Nullable<Int8>,
        gas_fees_paid -> Nullable<Numeric>,
        unique_counterparties -> Nullable<Int4>,
        contract_deployments -> Nullable<Int4>,
        created_at -> Nullable<Timestamptz>,
        updated_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    blocks (id) {
        id -> Int4,
        number -> Int8,
        #[max_length = 66]
        hash -> Varchar,
        #[max_length = 66]
        parent_hash -> Varchar,
        timestamp -> Int8,
        gas_limit -> Int8,
        gas_used -> Int8,
        #[max_length = 42]
        miner -> Varchar,
        difficulty -> Numeric,
        total_difficulty -> Nullable<Numeric>,
        size -> Nullable<Int8>,
        transaction_count -> Int4,
        extra_data -> Nullable<Text>,
        logs_bloom -> Nullable<Text>,
        #[max_length = 66]
        mix_hash -> Nullable<Varchar>,
        #[max_length = 18]
        nonce -> Nullable<Varchar>,
        base_fee_per_gas -> Nullable<Int8>,
        created_at -> Nullable<Timestamptz>,
        updated_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    bridge_stats (id) {
        id -> Int4,
        date -> Date,
        total_deposits_count -> Nullable<Int8>,
        total_withdrawals_count -> Nullable<Int8>,
        total_deposit_volume -> Nullable<Numeric>,
        total_withdrawal_volume -> Nullable<Numeric>,
        tvl_eth -> Nullable<Numeric>,
        unique_depositors -> Nullable<Int4>,
        unique_withdrawers -> Nullable<Int4>,
        avg_deposit_size -> Nullable<Numeric>,
        avg_withdrawal_size -> Nullable<Numeric>,
        created_at -> Nullable<Timestamptz>,
        updated_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    bridge_transactions (id) {
        id -> Int4,
        #[max_length = 66]
        transaction_hash -> Varchar,
        block_number -> Int8,
        timestamp -> Timestamptz,
        #[max_length = 50]
        bridge_type -> Varchar,
        #[max_length = 20]
        from_chain -> Varchar,
        #[max_length = 20]
        to_chain -> Varchar,
        #[max_length = 42]
        from_address -> Varchar,
        #[max_length = 42]
        to_address -> Nullable<Varchar>,
        #[max_length = 42]
        token_address -> Nullable<Varchar>,
        amount -> Numeric,
        #[max_length = 66]
        l1_transaction_hash -> Nullable<Varchar>,
        #[max_length = 66]
        l2_transaction_hash -> Nullable<Varchar>,
        #[max_length = 42]
        bridge_contract -> Varchar,
        #[max_length = 20]
        status -> Nullable<Varchar>,
        proof_submitted -> Nullable<Bool>,
        finalized -> Nullable<Bool>,
        created_at -> Nullable<Timestamptz>,
        updated_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    transactions (id) {
        id -> Int4,
        #[max_length = 66]
        hash -> Varchar,
        block_number -> Int8,
        #[max_length = 66]
        block_hash -> Varchar,
        transaction_index -> Int4,
        #[max_length = 42]
        from_address -> Varchar,
        #[max_length = 42]
        to_address -> Nullable<Varchar>,
        value -> Numeric,
        gas_limit -> Int8,
        gas_used -> Nullable<Int8>,
        gas_price -> Nullable<Numeric>,
        max_fee_per_gas -> Nullable<Numeric>,
        max_priority_fee_per_gas -> Nullable<Numeric>,
        nonce -> Int8,
        input_data -> Nullable<Text>,
        status -> Nullable<Int4>,
        #[max_length = 42]
        contract_address -> Nullable<Varchar>,
        logs_count -> Nullable<Int4>,
        cumulative_gas_used -> Nullable<Int8>,
        effective_gas_price -> Nullable<Numeric>,
        transaction_type -> Nullable<Int4>,
        access_list -> Nullable<Text>,
        created_at -> Nullable<Timestamptz>,
        updated_at -> Nullable<Timestamptz>,
        #[max_length = 10]
        direction -> Varchar,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    address_stats,
    blocks,
    bridge_stats,
    bridge_transactions,
    transactions,
);
