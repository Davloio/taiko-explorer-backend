-- Performance indexes for address analytics queries
-- These composite indexes will significantly speed up the address page queries

-- Composite index for top addresses by volume query
CREATE INDEX IF NOT EXISTS idx_address_stats_volume_composite 
ON address_stats(total_volume_sent DESC, total_volume_received DESC, address);

-- Composite index for top addresses by activity/transactions
CREATE INDEX IF NOT EXISTS idx_address_stats_activity_composite 
ON address_stats(total_transactions DESC, address);

-- Index for address growth chart queries (aggregates by first_seen_block)
CREATE INDEX IF NOT EXISTS idx_address_stats_first_seen 
ON address_stats(first_seen_block);

-- Composite index for filtering active addresses by block range
CREATE INDEX IF NOT EXISTS idx_address_stats_block_range 
ON address_stats(first_seen_block, last_seen_block);

-- Index for unique counterparties ranking
CREATE INDEX IF NOT EXISTS idx_address_stats_counterparties 
ON address_stats(unique_counterparties DESC, address);

-- IMPORTANT: For existing DB, run these same commands manually:
-- psql -U postgres -d taiko_explorer -c "CREATE INDEX IF NOT EXISTS idx_address_stats_volume_composite ON address_stats(total_volume_sent DESC, total_volume_received DESC, address);"
-- psql -U postgres -d taiko_explorer -c "CREATE INDEX IF NOT EXISTS idx_address_stats_activity_composite ON address_stats(total_transactions DESC, address);"
-- etc...