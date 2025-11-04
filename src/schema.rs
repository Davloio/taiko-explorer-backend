// @generated automatically by Diesel CLI.

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
