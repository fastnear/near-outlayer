-- Add RTMR3 tracking to worker_auth_tokens table
-- This allows monitoring which RTMR3 each worker is using
-- without enforcing validation (all workers with valid bearer token are trusted)

ALTER TABLE worker_auth_tokens
ADD COLUMN last_seen_rtmr3 TEXT,
ADD COLUMN last_attestation_at TIMESTAMPTZ;

-- Create index for monitoring queries
CREATE INDEX idx_worker_auth_tokens_last_attestation
ON worker_auth_tokens(last_attestation_at DESC);

-- Comment for documentation
COMMENT ON COLUMN worker_auth_tokens.last_seen_rtmr3 IS 'Last RTMR3 measurement received from this worker (96 hex chars)';
COMMENT ON COLUMN worker_auth_tokens.last_attestation_at IS 'Timestamp of last attestation from this worker';
