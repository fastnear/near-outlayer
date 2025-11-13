-- Create api_keys table for attestation API access control
-- Migration: 20251112000007

CREATE TABLE IF NOT EXISTS api_keys (
    id BIGSERIAL PRIMARY KEY,
    api_key VARCHAR(64) UNIQUE NOT NULL,   -- SHA256 hash of the API key (for secure lookup)
    near_account_id VARCHAR(64) NOT NULL,  -- NEAR account that owns this key
    key_name VARCHAR(255),                  -- Optional: user-friendly name for the key
    is_active BOOLEAN DEFAULT true,
    rate_limit_per_minute INTEGER DEFAULT 60,
    created_at TIMESTAMP DEFAULT NOW(),
    last_used_at TIMESTAMP,

    CONSTRAINT valid_near_account CHECK (
        near_account_id ~* '^[a-z0-9._-]+\.near$|^[a-z0-9._-]{2,64}$'
    ),
    CONSTRAINT valid_rate_limit CHECK (
        rate_limit_per_minute > 0 AND rate_limit_per_minute <= 600
    )
);

CREATE INDEX IF NOT EXISTS idx_api_keys_near_account ON api_keys(near_account_id);
CREATE INDEX IF NOT EXISTS idx_api_keys_active ON api_keys(is_active) WHERE is_active = true;
CREATE INDEX IF NOT EXISTS idx_api_keys_api_key ON api_keys(api_key) WHERE is_active = true;

-- Track API key usage statistics
CREATE TABLE IF NOT EXISTS api_key_usage (
    id BIGSERIAL PRIMARY KEY,
    api_key_id BIGINT REFERENCES api_keys(id) ON DELETE CASCADE,
    endpoint VARCHAR(255) NOT NULL,
    request_count INTEGER DEFAULT 1,
    date DATE DEFAULT CURRENT_DATE,

    UNIQUE(api_key_id, endpoint, date)
);

CREATE INDEX IF NOT EXISTS idx_api_key_usage_date ON api_key_usage(date DESC);
CREATE INDEX IF NOT EXISTS idx_api_key_usage_api_key_id ON api_key_usage(api_key_id);

COMMENT ON TABLE api_keys IS 'API keys for accessing attestation data - linked to NEAR accounts';
COMMENT ON COLUMN api_keys.api_key IS 'SHA256 hash of the plaintext API key (never store plaintext!)';
COMMENT ON COLUMN api_keys.near_account_id IS 'NEAR account that owns this API key';
COMMENT ON COLUMN api_keys.rate_limit_per_minute IS 'Maximum requests per minute allowed for this key';

COMMENT ON TABLE api_key_usage IS 'Daily usage statistics per API key and endpoint';
