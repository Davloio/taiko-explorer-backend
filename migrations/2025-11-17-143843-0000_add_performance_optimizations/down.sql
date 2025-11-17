-- Rollback Performance Optimizations Migration

-- Drop materialized view and views
DROP MATERIALIZED VIEW IF EXISTS mv_explorer_stats;
DROP VIEW IF EXISTS v_quick_address_count;

-- Drop performance indexes (in reverse order)
DROP INDEX IF EXISTS idx_transactions_recent_activity;
DROP INDEX IF EXISTS idx_transactions_to_addresses_composite;
DROP INDEX IF EXISTS idx_transactions_contract_creation;
DROP INDEX IF EXISTS idx_transactions_failed;
DROP INDEX IF EXISTS idx_transactions_block_composite;
DROP INDEX IF EXISTS idx_unique_addresses_to;
DROP INDEX IF EXISTS idx_transactions_addresses_composite;
DROP INDEX IF EXISTS idx_transactions_id_desc;
DROP INDEX IF EXISTS idx_blocks_number_desc;
DROP INDEX IF EXISTS idx_transactions_status_only;