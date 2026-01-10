-- Payment Keys metadata table
-- Stores key_hash, initial_balance, project_ids, max_per_call
-- Synced from contract via TopUp/Delete events processed by worker

CREATE TABLE IF NOT EXISTS payment_keys (
    owner TEXT NOT NULL,
    nonce INTEGER NOT NULL,
    -- SHA256 hash of the key (hex encoded, 64 chars)
    key_hash TEXT NOT NULL,
    -- Initial balance from the encrypted secret (in minimal token units)
    initial_balance TEXT NOT NULL,
    -- Allowed project IDs (empty array = all projects allowed)
    project_ids TEXT[] NOT NULL DEFAULT '{}',
    -- Max amount per API call (NULL = no limit)
    max_per_call TEXT,
    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- Soft delete timestamp (NULL = active, set = deleted)
    deleted_at TIMESTAMPTZ,
    PRIMARY KEY (owner, nonce)
);

-- Index for active keys lookup (most common query)
CREATE INDEX IF NOT EXISTS idx_payment_keys_active
    ON payment_keys(owner, nonce)
    WHERE deleted_at IS NULL;

-- Index for key_hash lookup (validation)
CREATE INDEX IF NOT EXISTS idx_payment_keys_hash
    ON payment_keys(key_hash);
