-- Recreate address tables (for rollback)
CREATE TABLE address_labels (
    id SERIAL PRIMARY KEY,
    address VARCHAR(42) NOT NULL,
    label VARCHAR(100) NOT NULL,
    category VARCHAR(50) NOT NULL,
    description TEXT,
    verified BOOLEAN,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE address_balances (
    id SERIAL PRIMARY KEY,
    address VARCHAR(42) NOT NULL,
    balance NUMERIC NOT NULL,
    block_number BIGINT NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);