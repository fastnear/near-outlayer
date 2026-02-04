-- HTTPS calls: store execution data in execution_requests instead of Redis-only.
--
-- Previously HTTPS calls used request_id=0 as a sentinel and stored data only in Redis.
-- This was fragile: any code path querying execution_requests by request_id would miss HTTPS data.
--
-- Now HTTPS calls get a generated request_id from a sequence starting at 100B,
-- well above blockchain request_ids which start from 0 and grow slowly
-- (reaching 100B would take ~3170 years at 1000 req/sec).

-- Sequence for generating HTTPS request_ids (starts at 100B to avoid collision with blockchain IDs)
CREATE SEQUENCE IF NOT EXISTS https_request_id_seq START WITH 100000000000;

-- HTTPS-specific columns on execution_requests
ALTER TABLE execution_requests ADD COLUMN IF NOT EXISTS is_https_call BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE execution_requests ADD COLUMN IF NOT EXISTS call_id UUID;
ALTER TABLE execution_requests ADD COLUMN IF NOT EXISTS payment_key_owner TEXT;
ALTER TABLE execution_requests ADD COLUMN IF NOT EXISTS payment_key_nonce INTEGER;
ALTER TABLE execution_requests ADD COLUMN IF NOT EXISTS usd_payment TEXT;
ALTER TABLE execution_requests ADD COLUMN IF NOT EXISTS compute_limit_usd TEXT;
ALTER TABLE execution_requests ADD COLUMN IF NOT EXISTS attached_deposit_usd TEXT;
ALTER TABLE execution_requests ADD COLUMN IF NOT EXISTS version_key TEXT;
ALTER TABLE execution_requests ADD COLUMN IF NOT EXISTS user_account_id TEXT;

-- Index for looking up HTTPS calls by call_id (used by /calls/{call_id} endpoint)
CREATE INDEX IF NOT EXISTS idx_execution_requests_call_id
    ON execution_requests(call_id) WHERE call_id IS NOT NULL;
