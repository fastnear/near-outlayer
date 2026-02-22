-- Wallet module: Fireblocks-style custody for AI agents
-- wallet_id = UUID string (no prefix), near_pubkey = ed25519:hex (on-chain identity)
-- Drop old schema (testnet had account_id-based tables from pre-commit migrations)

DROP TABLE IF EXISTS wallet_webhook_deliveries CASCADE;
DROP TABLE IF EXISTS wallet_audit_log CASCADE;
DROP TABLE IF EXISTS wallet_usage CASCADE;
DROP TABLE IF EXISTS wallet_approval_signatures CASCADE;
DROP TABLE IF EXISTS wallet_pending_approvals CASCADE;
DROP TABLE IF EXISTS wallet_requests CASCADE;
DROP TABLE IF EXISTS wallet_api_keys CASCADE;
DROP TABLE IF EXISTS wallet_accounts CASCADE;

CREATE TABLE wallet_accounts (
    wallet_id TEXT PRIMARY KEY,             -- UUID string
    near_pubkey TEXT UNIQUE,                -- "ed25519:<hex>" for contract/dashboard lookup
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE wallet_api_keys (
    key_hash TEXT PRIMARY KEY,              -- SHA256(api_key)
    wallet_id TEXT NOT NULL REFERENCES wallet_accounts(wallet_id),
    label TEXT NOT NULL DEFAULT 'primary',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    revoked_at TIMESTAMPTZ
);
CREATE INDEX idx_wallet_api_keys_wallet ON wallet_api_keys(wallet_id);

-- Async operation tracking (withdraw, deposit, call)
CREATE TABLE wallet_requests (
    request_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    wallet_id TEXT NOT NULL,
    request_type TEXT NOT NULL,
    chain TEXT NOT NULL,
    request_data JSONB NOT NULL,
    intents_ref TEXT,
    approval_id UUID,
    status TEXT NOT NULL DEFAULT 'processing',
    result_data JSONB,
    idempotency_key TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_wallet_requests_wallet ON wallet_requests(wallet_id, created_at DESC);
CREATE UNIQUE INDEX idx_wallet_requests_idempotency ON wallet_requests(wallet_id, idempotency_key) WHERE idempotency_key IS NOT NULL;

-- Pending approvals (multisig)
CREATE TABLE wallet_pending_approvals (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    wallet_id TEXT NOT NULL,
    request_type TEXT NOT NULL,
    request_data JSONB NOT NULL,
    request_hash TEXT NOT NULL,
    required_approvals INT NOT NULL,
    status TEXT DEFAULT 'pending',
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_wallet_approvals_wallet ON wallet_pending_approvals(wallet_id, status);

ALTER TABLE wallet_requests
    ADD CONSTRAINT fk_wallet_requests_approval
    FOREIGN KEY (approval_id) REFERENCES wallet_pending_approvals(id);

-- Approval signatures
CREATE TABLE wallet_approval_signatures (
    approval_id UUID REFERENCES wallet_pending_approvals(id) ON DELETE CASCADE,
    approver_id TEXT NOT NULL,
    approver_role TEXT NOT NULL,
    signature TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (approval_id, approver_id)
);

-- Usage tracking
CREATE TABLE wallet_usage (
    wallet_id TEXT NOT NULL,
    token TEXT NOT NULL,
    period TEXT NOT NULL,
    total_amount TEXT NOT NULL DEFAULT '0',
    tx_count INT NOT NULL DEFAULT 0,
    PRIMARY KEY (wallet_id, token, period)
);

-- Audit log
CREATE TABLE wallet_audit_log (
    id BIGSERIAL PRIMARY KEY,
    wallet_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    actor TEXT NOT NULL,
    details JSONB NOT NULL,
    request_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_wallet_audit_wallet ON wallet_audit_log(wallet_id, created_at DESC);

-- Webhook delivery queue
CREATE TABLE wallet_webhook_deliveries (
    id BIGSERIAL PRIMARY KEY,
    wallet_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL,
    webhook_url TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    attempts INT NOT NULL DEFAULT 0,
    next_retry_at TIMESTAMPTZ,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_wallet_webhook_pending ON wallet_webhook_deliveries(status, next_retry_at)
    WHERE status = 'pending';

-- wallet_id on execution_requests for linking
ALTER TABLE execution_requests ADD COLUMN IF NOT EXISTS wallet_id TEXT;
