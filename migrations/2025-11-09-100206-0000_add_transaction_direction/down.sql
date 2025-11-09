DROP INDEX IF EXISTS idx_transactions_status_direction;
DROP INDEX IF EXISTS idx_transactions_direction;

ALTER TABLE transactions 
DROP COLUMN direction;