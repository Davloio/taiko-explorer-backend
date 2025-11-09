ALTER TABLE transactions 
ADD COLUMN direction VARCHAR(10) DEFAULT 'inside' NOT NULL;

CREATE INDEX idx_transactions_direction ON transactions(direction);
CREATE INDEX idx_transactions_status_direction ON transactions(status, direction);