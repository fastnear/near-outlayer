-- Payment Key tables for HTTPS API

-- Track payment key balances (spent and reserved)
CREATE TABLE IF NOT EXISTS payment_key_balances (
    owner TEXT NOT NULL,
    nonce INTEGER NOT NULL,
    -- spent: total amount spent (in minimal token units, e.g., 1 = 0.000001 USDT)
    spent NUMERIC(38, 0) NOT NULL DEFAULT 0,
    -- reserved: amount currently reserved for in-flight calls
    reserved NUMERIC(38, 0) NOT NULL DEFAULT 0,
    last_used_at TIMESTAMPTZ,
    last_reserved_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (owner, nonce)
);

-- Track payment key usage history (for audit and billing)
CREATE TABLE IF NOT EXISTS payment_key_usage (
    id BIGSERIAL PRIMARY KEY,
    owner TEXT NOT NULL,
    nonce INTEGER NOT NULL,
    call_id UUID NOT NULL,
    project_id TEXT, -- "owner.near/project-name"
    -- compute_cost: amount charged for compute (goes to OutLayer)
    compute_cost NUMERIC(38, 0) NOT NULL,
    -- attached_deposit: amount sent to project owner
    attached_deposit NUMERIC(38, 0) NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'completed', -- 'completed', 'failed'
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Track project owner earnings (from attached_deposit)
CREATE TABLE IF NOT EXISTS project_owner_earnings (
    project_owner TEXT PRIMARY KEY,
    -- balance: current available balance (can be withdrawn)
    balance NUMERIC(38, 0) NOT NULL DEFAULT 0,
    -- total_earned: total earnings ever (for stats)
    total_earned NUMERIC(38, 0) NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Track project owner withdrawals (for audit)
CREATE TABLE IF NOT EXISTS project_owner_withdrawals (
    id BIGSERIAL PRIMARY KEY,
    project_owner TEXT NOT NULL,
    amount NUMERIC(38, 0) NOT NULL,
    tx_hash TEXT, -- NEAR transaction hash
    status TEXT NOT NULL DEFAULT 'pending', -- 'pending', 'completed', 'failed'
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

-- HTTPS API calls tracking (similar to jobs but for HTTPS)
CREATE TABLE IF NOT EXISTS https_calls (
    call_id UUID PRIMARY KEY,
    owner TEXT NOT NULL, -- Payment key owner
    nonce INTEGER NOT NULL, -- Payment key nonce
    project_id TEXT NOT NULL, -- "owner.near/project-name"
    status TEXT NOT NULL DEFAULT 'pending', -- 'pending', 'running', 'completed', 'failed'
    input_data TEXT,
    output_data TEXT,
    error_message TEXT,
    -- resource usage
    instructions BIGINT,
    time_ms BIGINT,
    compile_time_ms BIGINT,
    -- costs (in minimal token units)
    compute_cost NUMERIC(38, 0),
    attached_deposit NUMERIC(38, 0) NOT NULL DEFAULT 0,
    -- timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ
);

-- Indices for performance
CREATE INDEX IF NOT EXISTS idx_payment_key_usage_owner_nonce ON payment_key_usage(owner, nonce);
CREATE INDEX IF NOT EXISTS idx_payment_key_usage_call_id ON payment_key_usage(call_id);
CREATE INDEX IF NOT EXISTS idx_payment_key_usage_created_at ON payment_key_usage(created_at);
CREATE INDEX IF NOT EXISTS idx_project_owner_withdrawals_owner ON project_owner_withdrawals(project_owner);
CREATE INDEX IF NOT EXISTS idx_https_calls_owner_nonce ON https_calls(owner, nonce);
CREATE INDEX IF NOT EXISTS idx_https_calls_status ON https_calls(status);
CREATE INDEX IF NOT EXISTS idx_https_calls_created_at ON https_calls(created_at);
