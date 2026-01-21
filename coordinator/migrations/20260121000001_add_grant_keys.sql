-- Add is_grant flag to payment_keys for grant (non-withdrawable) keys
-- Grant keys:
-- - Cannot use X-Attached-Deposit (no earnings transfer)
-- - Compute usage is charged normally
-- - Created by admin, not synced from contract
-- - Cannot be withdrawn

ALTER TABLE payment_keys ADD COLUMN is_grant BOOLEAN NOT NULL DEFAULT FALSE;

-- Index for admin queries on grant keys
CREATE INDEX IF NOT EXISTS idx_payment_keys_grants
    ON payment_keys(is_grant)
    WHERE is_grant = TRUE AND deleted_at IS NULL;
