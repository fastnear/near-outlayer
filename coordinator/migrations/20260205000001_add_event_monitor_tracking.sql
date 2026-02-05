-- Add event monitor block height tracking to worker_status table
-- Workers report their event monitor's current block height in heartbeats
-- Used by /health/detailed to detect stale event monitors and block lag

ALTER TABLE worker_status
ADD COLUMN event_monitor_block_height BIGINT NULL,
ADD COLUMN event_monitor_updated_at TIMESTAMPTZ NULL;
