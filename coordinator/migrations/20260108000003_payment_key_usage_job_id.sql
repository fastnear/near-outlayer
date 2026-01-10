-- Add job_id to payment_key_usage for attestation lookups
-- Each HTTPS call creates 1-2 jobs (compile + execute), we store the execute job's id

ALTER TABLE payment_key_usage ADD COLUMN IF NOT EXISTS job_id BIGINT;
CREATE INDEX IF NOT EXISTS idx_payment_key_usage_job_id ON payment_key_usage(job_id);
