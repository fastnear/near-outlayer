-- Add HTTPS call support to task_attestations table
-- For HTTPS calls: call_id is set, transaction_hash/request_id are NULL
-- For NEAR calls: transaction_hash/request_id are set, call_id is NULL

-- Add call_id for HTTPS calls
ALTER TABLE task_attestations
    ADD COLUMN IF NOT EXISTS call_id UUID;

-- Add payment key info for HTTPS calls
ALTER TABLE task_attestations
    ADD COLUMN IF NOT EXISTS payment_key_owner TEXT;

ALTER TABLE task_attestations
    ADD COLUMN IF NOT EXISTS payment_key_nonce INTEGER;

-- Add index for call_id lookups
CREATE INDEX IF NOT EXISTS idx_attestations_call_id
    ON task_attestations(call_id) WHERE call_id IS NOT NULL;

-- Add index for payment key lookups
CREATE INDEX IF NOT EXISTS idx_attestations_payment_key
    ON task_attestations(payment_key_owner, payment_key_nonce)
    WHERE payment_key_owner IS NOT NULL;

-- Comments for documentation
COMMENT ON COLUMN task_attestations.call_id IS 'UUID for HTTPS API calls (NULL for NEAR blockchain calls)';
COMMENT ON COLUMN task_attestations.payment_key_owner IS 'Owner account of Payment Key (HTTPS calls only)';
COMMENT ON COLUMN task_attestations.payment_key_nonce IS 'Nonce of Payment Key (HTTPS calls only)';
