-- Unified earnings history for both blockchain and HTTPS calls
-- Note: project_owner_earnings table is only for HTTPS calls balance
-- Blockchain earnings are stored in the NEAR contract (developer_earnings)
CREATE TABLE IF NOT EXISTS earnings_history (
    id BIGSERIAL PRIMARY KEY,
    project_owner TEXT NOT NULL,
    project_id TEXT NOT NULL,
    attached_usdc NUMERIC(38, 0) NOT NULL,  -- amount user attached
    refund_usdc NUMERIC(38, 0) NOT NULL DEFAULT 0,  -- amount refunded to user
    amount NUMERIC(38, 0) NOT NULL,         -- actual amount to developer (attached - refund)
    source TEXT NOT NULL,                   -- 'blockchain' or 'https'
    -- Blockchain specific fields (NULL for HTTPS)
    tx_hash TEXT,                           -- NEAR transaction hash
    caller TEXT,                            -- NEAR account that called request_execution
    request_id BIGINT,                      -- contract request_id
    -- HTTPS specific fields (NULL for blockchain)
    call_id UUID,                           -- HTTPS call UUID
    payment_key_owner TEXT,                 -- payment key owner
    payment_key_nonce INTEGER,              -- payment key nonce
    -- Common
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_earnings_history_owner ON earnings_history(project_owner);
CREATE INDEX IF NOT EXISTS idx_earnings_history_created_at ON earnings_history(created_at);
CREATE INDEX IF NOT EXISTS idx_earnings_history_source ON earnings_history(source);
