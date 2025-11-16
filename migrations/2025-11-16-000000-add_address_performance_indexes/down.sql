-- Rollback: Remove performance indexes
DROP INDEX IF EXISTS idx_address_stats_volume_composite;
DROP INDEX IF EXISTS idx_address_stats_activity_composite;
DROP INDEX IF EXISTS idx_address_stats_first_seen;
DROP INDEX IF EXISTS idx_address_stats_block_range;
DROP INDEX IF EXISTS idx_address_stats_counterparties;