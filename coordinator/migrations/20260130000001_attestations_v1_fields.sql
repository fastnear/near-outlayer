-- Add V1 attestation fields for enhanced verification
-- These fields are included in the attestation hash starting from OUTLAYER_ATTESTATION_V1 timestamp

-- Project ID (e.g., "alice.near/my-app")
ALTER TABLE task_attestations
    ADD COLUMN IF NOT EXISTS project_id TEXT;

-- Secrets reference in format "{account_id}/{profile}"
ALTER TABLE task_attestations
    ADD COLUMN IF NOT EXISTS secrets_ref TEXT;

-- Payment amount in minimal token units (USD stablecoin)
ALTER TABLE task_attestations
    ADD COLUMN IF NOT EXISTS attached_usd TEXT;

-- Index for project lookups
CREATE INDEX IF NOT EXISTS idx_attestations_project_id
    ON task_attestations(project_id) WHERE project_id IS NOT NULL;

-- Comments
COMMENT ON COLUMN task_attestations.project_id IS 'Project ID for project-based executions (V1 attestation field)';
COMMENT ON COLUMN task_attestations.secrets_ref IS 'Secrets reference as {account_id}/{profile} (V1 attestation field)';
COMMENT ON COLUMN task_attestations.attached_usd IS 'USD payment amount in minimal token units (V1 attestation field)';
