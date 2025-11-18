-- Store original execution requests for retrieval after compilation
-- This allows complete_job to create execute tasks with full original data

CREATE TABLE IF NOT EXISTS execution_requests (
    request_id BIGINT PRIMARY KEY,
    data_id TEXT NOT NULL,

    -- Full request data
    input_data TEXT,
    max_instructions BIGINT,
    max_memory_mb INTEGER,
    max_execution_seconds BIGINT,

    -- Secrets reference
    secrets_profile TEXT,
    secrets_account_id TEXT,

    -- Response format
    response_format TEXT,

    -- Execution context
    context_sender_id TEXT,
    context_block_height BIGINT,
    context_block_timestamp BIGINT,
    context_contract_id TEXT,
    context_transaction_hash TEXT,
    context_receipt_id TEXT,
    context_predecessor_id TEXT,
    context_signer_public_key TEXT,
    context_gas_burnt BIGINT,

    -- Metadata
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

-- Index for cleanup of old requests
CREATE INDEX IF NOT EXISTS idx_execution_requests_created_at ON execution_requests(created_at);
