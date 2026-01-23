-- Add is_encrypted flag for public storage support
-- When is_encrypted=false, data is stored in plaintext and can be read by other projects

ALTER TABLE storage_data ADD COLUMN is_encrypted BOOLEAN NOT NULL DEFAULT TRUE;

-- Index for finding public storage entries (for cross-project reads)
CREATE INDEX idx_storage_data_public ON storage_data(project_uuid, key_hash)
    WHERE is_encrypted = FALSE;

-- For public storage, we store:
-- - key_hash: SHA256 of plaintext key (same as encrypted)
-- - encrypted_key: plaintext key (not actually encrypted)
-- - encrypted_value: plaintext value (not actually encrypted)
-- This reuses existing columns to minimize schema changes

COMMENT ON COLUMN storage_data.is_encrypted IS 'If FALSE, key and value are stored in plaintext and readable by other projects';
