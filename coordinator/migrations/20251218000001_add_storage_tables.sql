-- Storage data table for persistent project storage
CREATE TABLE storage_data (
    id BIGSERIAL PRIMARY KEY,
    project_uuid VARCHAR(64),           -- NULL for standalone WASM (no project)
    wasm_hash VARCHAR(64) NOT NULL,     -- Which version wrote this data
    account_id VARCHAR(64) NOT NULL,    -- NEAR account or "@worker" for WASM-private
    key_hash VARCHAR(64) NOT NULL,      -- SHA256 of plaintext key (for lookups)
    encrypted_key BYTEA NOT NULL,       -- Encrypted storage key
    encrypted_value BYTEA NOT NULL,     -- Encrypted storage value
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- Unique constraint: one key per project/account combination
    CONSTRAINT storage_data_unique_key UNIQUE (project_uuid, account_id, key_hash)
);

-- Index for querying by wasm_hash (for storage_get_by_version)
CREATE INDEX idx_storage_data_wasm_hash ON storage_data(wasm_hash);

-- Index for listing keys by project and account
CREATE INDEX idx_storage_data_project_account ON storage_data(project_uuid, account_id);

COMMENT ON TABLE storage_data IS 'Persistent encrypted storage for OutLayer projects';
COMMENT ON COLUMN storage_data.project_uuid IS 'Project UUID (NULL for standalone WASM without project)';
COMMENT ON COLUMN storage_data.wasm_hash IS 'SHA256 hash of WASM that wrote this data (for version migration)';
COMMENT ON COLUMN storage_data.account_id IS 'NEAR account ID or "@worker" for WASM-private storage';
COMMENT ON COLUMN storage_data.key_hash IS 'SHA256 hash of plaintext key for unique constraint';
COMMENT ON COLUMN storage_data.encrypted_key IS 'AES-encrypted storage key';
COMMENT ON COLUMN storage_data.encrypted_value IS 'AES-encrypted storage value';


-- Storage usage tracking per project/account
CREATE TABLE storage_usage (
    id BIGSERIAL PRIMARY KEY,
    project_uuid VARCHAR(64),           -- NULL for standalone WASM
    wasm_hash VARCHAR(64),              -- NULL if project_uuid is set
    account_id VARCHAR(64) NOT NULL,    -- NEAR account or "@worker"
    total_bytes BIGINT DEFAULT 0,       -- Total storage bytes used
    key_count INT DEFAULT 0,            -- Number of keys stored
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- Only one of project_uuid or wasm_hash should be set
    CONSTRAINT storage_usage_project_unique UNIQUE (project_uuid, account_id),
    CONSTRAINT storage_usage_wasm_unique UNIQUE (wasm_hash, account_id),
    CONSTRAINT storage_usage_check_one_id CHECK (
        (project_uuid IS NOT NULL AND wasm_hash IS NULL) OR
        (project_uuid IS NULL AND wasm_hash IS NOT NULL)
    )
);

COMMENT ON TABLE storage_usage IS 'Storage usage tracking for billing (MVP: tracking only, no limits)';
COMMENT ON COLUMN storage_usage.total_bytes IS 'Total bytes used (encrypted_key + encrypted_value)';
COMMENT ON COLUMN storage_usage.key_count IS 'Number of storage keys';
