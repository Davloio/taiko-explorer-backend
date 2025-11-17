-- Performance Optimization Migration
-- Adds materialized view for fast stats and critical missing indexes

-- 1. CREATE MISSING CRITICAL INDEXES (IF NOT EXISTS prevents errors on existing indexes)

-- Transaction status filtering (homepage stats)
CREATE INDEX IF NOT EXISTS idx_transactions_status_only ON transactions (status) WHERE status IS NOT NULL;

-- Block queries (homepage recent blocks)
CREATE INDEX IF NOT EXISTS idx_blocks_number_desc ON blocks (number DESC);

-- Transaction pagination (homepage recent transactions)  
CREATE INDEX IF NOT EXISTS idx_transactions_id_desc ON transactions (id DESC);

-- Address transaction lookups (address pages)
CREATE INDEX IF NOT EXISTS idx_transactions_addresses_composite ON transactions (from_address, block_number DESC, id DESC);

-- Unique address counting optimization
CREATE INDEX IF NOT EXISTS idx_unique_addresses_to ON transactions (to_address) WHERE to_address IS NOT NULL;

-- Block/transaction composite queries
CREATE INDEX IF NOT EXISTS idx_transactions_block_composite ON transactions (block_number DESC, id DESC);

-- Failed transactions filtering
CREATE INDEX IF NOT EXISTS idx_transactions_failed ON transactions (status, block_number DESC) WHERE status = 0;

-- Contract creation queries
CREATE INDEX IF NOT EXISTS idx_transactions_contract_creation ON transactions (contract_address, block_number DESC) WHERE contract_address IS NOT NULL;

-- Address composite index for "to" field
CREATE INDEX IF NOT EXISTS idx_transactions_to_addresses_composite ON transactions (to_address, block_number DESC, id DESC) WHERE to_address IS NOT NULL;

-- Recent activity optimization
CREATE INDEX IF NOT EXISTS idx_transactions_recent_activity ON transactions (created_at DESC, id DESC) WHERE created_at IS NOT NULL;

-- 2. CREATE MATERIALIZED VIEW FOR FAST STATS

-- Drop existing view if it exists
DROP MATERIALIZED VIEW IF EXISTS mv_explorer_stats;

-- Create comprehensive stats view
CREATE MATERIALIZED VIEW mv_explorer_stats AS
SELECT 
    (SELECT COUNT(*) FROM blocks) as total_blocks,
    (SELECT MAX(number) FROM blocks) as latest_block_number,
    (SELECT COUNT(*) FROM transactions) as total_transactions,
    -- Fast unique address count using UNION of from and to addresses
    (SELECT COUNT(DISTINCT address) FROM (
        SELECT from_address as address FROM transactions
        UNION
        SELECT to_address as address FROM transactions WHERE to_address IS NOT NULL
    ) all_addresses) as total_unique_addresses,
    -- Transaction status counts
    (SELECT COUNT(*) FROM transactions WHERE status = 1) as successful_transactions,
    (SELECT COUNT(*) FROM transactions WHERE status = 0) as failed_transactions,
    -- Transaction direction counts  
    (SELECT COUNT(*) FROM transactions WHERE direction = 'in') as in_transactions,
    (SELECT COUNT(*) FROM transactions WHERE direction = 'out') as out_transactions,
    (SELECT COUNT(*) FROM transactions WHERE direction = 'inside') as inside_transactions,
    -- Timestamp for cache invalidation
    NOW() as last_updated;

-- Create unique index on materialized view for fast access
CREATE UNIQUE INDEX idx_mv_explorer_stats_updated ON mv_explorer_stats (last_updated);

-- 3. CREATE VIEW FOR FAST ADDRESS STATS
CREATE OR REPLACE VIEW v_quick_address_count AS
SELECT COUNT(*) as total_addresses FROM address_stats;

COMMENT ON MATERIALIZED VIEW mv_explorer_stats IS 'Pre-computed explorer statistics for fast homepage loading. Refresh periodically.';
COMMENT ON VIEW v_quick_address_count IS 'Fast address count using address_stats table instead of transaction DISTINCT queries.';