CREATE TABLE transactions (
    id SERIAL PRIMARY KEY,
    hash VARCHAR(66) NOT NULL UNIQUE,
    block_number BIGINT NOT NULL,
    block_hash VARCHAR(66) NOT NULL,
    transaction_index INTEGER NOT NULL,
    from_address VARCHAR(42) NOT NULL,
    to_address VARCHAR(42),
    value NUMERIC(78, 0) NOT NULL DEFAULT 0,
    gas_limit BIGINT NOT NULL,
    gas_used BIGINT,
    gas_price NUMERIC(78, 0),
    max_fee_per_gas NUMERIC(78, 0),
    max_priority_fee_per_gas NUMERIC(78, 0),
    nonce BIGINT NOT NULL,
    input_data TEXT,
    status INTEGER,
    contract_address VARCHAR(42),
    logs_count INTEGER DEFAULT 0,
    cumulative_gas_used BIGINT,
    effective_gas_price NUMERIC(78, 0),
    transaction_type INTEGER DEFAULT 0,
    access_list TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    
    FOREIGN KEY (block_number) REFERENCES blocks(number) ON DELETE CASCADE
);

CREATE INDEX idx_transactions_hash ON transactions(hash);
CREATE INDEX idx_transactions_block_number ON transactions(block_number);
CREATE INDEX idx_transactions_block_hash ON transactions(block_hash);
CREATE INDEX idx_transactions_from_address ON transactions(from_address);
CREATE INDEX idx_transactions_to_address ON transactions(to_address);
CREATE INDEX idx_transactions_status ON transactions(status);
CREATE INDEX idx_transactions_created_at ON transactions(created_at);
CREATE INDEX idx_transactions_value ON transactions(value);
CREATE INDEX idx_transactions_gas_used ON transactions(gas_used);

CREATE INDEX idx_transactions_block_tx_index ON transactions(block_number, transaction_index);
CREATE INDEX idx_transactions_from_to ON transactions(from_address, to_address);