-- Address Analytics Tables

-- Address labels/tags for categorization (DEX, Bridge, etc.)
CREATE TABLE address_labels (
    id SERIAL PRIMARY KEY,
    address VARCHAR(42) NOT NULL UNIQUE,
    label VARCHAR(100) NOT NULL,
    category VARCHAR(50) NOT NULL, -- 'DEX', 'Bridge', 'Exchange', 'Contract', 'EOA', etc.
    description TEXT,
    verified BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Address balance history snapshots (daily/hourly snapshots)
CREATE TABLE address_balances (
    id SERIAL PRIMARY KEY,
    address VARCHAR(42) NOT NULL,
    balance NUMERIC(78, 0) NOT NULL, -- ETH balance in wei
    block_number BIGINT NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Address activity statistics (cached/computed data)
CREATE TABLE address_stats (
    id SERIAL PRIMARY KEY,
    address VARCHAR(42) NOT NULL UNIQUE,
    first_seen_block BIGINT NOT NULL,
    last_seen_block BIGINT NOT NULL,
    total_transactions BIGINT DEFAULT 0,
    total_sent_transactions BIGINT DEFAULT 0,
    total_received_transactions BIGINT DEFAULT 0,
    total_volume_sent NUMERIC(78, 0) DEFAULT 0, -- Total ETH sent in wei
    total_volume_received NUMERIC(78, 0) DEFAULT 0, -- Total ETH received in wei
    gas_used BIGINT DEFAULT 0, -- Total gas used
    gas_fees_paid NUMERIC(78, 0) DEFAULT 0, -- Total gas fees paid in wei
    unique_counterparties INTEGER DEFAULT 0, -- Number of unique addresses interacted with
    contract_deployments INTEGER DEFAULT 0, -- Number of contracts deployed
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Bridge transactions detection table
CREATE TABLE bridge_transactions (
    id SERIAL PRIMARY KEY,
    transaction_hash VARCHAR(66) NOT NULL UNIQUE,
    block_number BIGINT NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    bridge_type VARCHAR(50) NOT NULL, -- 'deposit', 'withdrawal', 'claim'
    from_chain VARCHAR(20) NOT NULL, -- 'ethereum', 'taiko'
    to_chain VARCHAR(20) NOT NULL, -- 'ethereum', 'taiko'
    from_address VARCHAR(42) NOT NULL,
    to_address VARCHAR(42),
    token_address VARCHAR(42), -- NULL for ETH, contract address for tokens
    amount NUMERIC(78, 0) NOT NULL, -- Amount in wei/smallest unit
    l1_transaction_hash VARCHAR(66), -- Related L1 tx hash
    l2_transaction_hash VARCHAR(66), -- Related L2 tx hash
    bridge_contract VARCHAR(42) NOT NULL, -- Bridge contract address
    status VARCHAR(20) DEFAULT 'pending', -- 'pending', 'success', 'failed', 'proven', 'finalized'
    proof_submitted BOOLEAN DEFAULT FALSE,
    finalized BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Bridge statistics (TVL, volume, etc.)
CREATE TABLE bridge_stats (
    id SERIAL PRIMARY KEY,
    date DATE NOT NULL UNIQUE,
    total_deposits_count BIGINT DEFAULT 0,
    total_withdrawals_count BIGINT DEFAULT 0,
    total_deposit_volume NUMERIC(78, 0) DEFAULT 0, -- ETH deposited in wei
    total_withdrawal_volume NUMERIC(78, 0) DEFAULT 0, -- ETH withdrawn in wei
    tvl_eth NUMERIC(78, 0) DEFAULT 0, -- Total Value Locked in ETH (wei)
    unique_depositors INTEGER DEFAULT 0,
    unique_withdrawers INTEGER DEFAULT 0,
    avg_deposit_size NUMERIC(78, 0) DEFAULT 0,
    avg_withdrawal_size NUMERIC(78, 0) DEFAULT 0,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Indexes for performance
CREATE INDEX idx_address_labels_address ON address_labels(address);
CREATE INDEX idx_address_labels_category ON address_labels(category);

CREATE INDEX idx_address_balances_address ON address_balances(address);
CREATE INDEX idx_address_balances_block_number ON address_balances(block_number);
CREATE INDEX idx_address_balances_timestamp ON address_balances(timestamp);
CREATE INDEX idx_address_balances_address_timestamp ON address_balances(address, timestamp);

CREATE INDEX idx_address_stats_address ON address_stats(address);
CREATE INDEX idx_address_stats_total_volume_sent ON address_stats(total_volume_sent);
CREATE INDEX idx_address_stats_total_transactions ON address_stats(total_transactions);
CREATE INDEX idx_address_stats_last_seen_block ON address_stats(last_seen_block);

CREATE INDEX idx_bridge_transactions_hash ON bridge_transactions(transaction_hash);
CREATE INDEX idx_bridge_transactions_block_number ON bridge_transactions(block_number);
CREATE INDEX idx_bridge_transactions_timestamp ON bridge_transactions(timestamp);
CREATE INDEX idx_bridge_transactions_bridge_type ON bridge_transactions(bridge_type);
CREATE INDEX idx_bridge_transactions_from_address ON bridge_transactions(from_address);
CREATE INDEX idx_bridge_transactions_to_address ON bridge_transactions(to_address);
CREATE INDEX idx_bridge_transactions_status ON bridge_transactions(status);

CREATE INDEX idx_bridge_stats_date ON bridge_stats(date);
