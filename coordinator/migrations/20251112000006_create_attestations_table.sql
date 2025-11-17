-- Create task_attestations table for storing TDX attestation quotes
-- Migration: 20251112000006
-- Updated: Unified schema for both Compile and Execute tasks

CREATE TABLE IF NOT EXISTS task_attestations (
    id BIGSERIAL PRIMARY KEY,

    -- Task identification
    task_id BIGINT NOT NULL,                   -- Internal coordinator task ID
    task_type VARCHAR(20) NOT NULL,            -- 'compile' or 'execute'

    -- TDX attestation data
    tdx_quote BYTEA NOT NULL,                  -- Raw Intel TDX quote (binary, 5-8KB)
    worker_measurement VARCHAR(96) NOT NULL,   -- RTMR3 from TDX quote (48 bytes hex = 96 chars)

    -- NEAR context (from contract event)
    request_id BIGINT,                          -- Request ID from NEAR contract
    caller_account_id VARCHAR(64),              -- NEAR account that called contract
    transaction_hash VARCHAR(64),               -- NEAR transaction hash
    block_height BIGINT,                        -- NEAR block height

    -- Code source (present in both Compile and Execute tasks)
    repo_url VARCHAR(512),                      -- GitHub repository URL
    commit_hash VARCHAR(64),                    -- Git commit hash
    build_target VARCHAR(64),                   -- e.g., "wasm32-wasip1"

    -- Task data hashes
    wasm_hash VARCHAR(64),                      -- SHA256 of WASM bytes (output for Compile, input for Execute)
    input_hash VARCHAR(64),                     -- SHA256 of input_data (Execute only, NULL for Compile)
    output_hash VARCHAR(64) NOT NULL,           -- SHA256 of output (always present)

    -- Metadata
    created_at TIMESTAMP DEFAULT NOW(),

    -- Constraints
    CONSTRAINT valid_task_type CHECK (task_type IN ('compile', 'execute')),
    CONSTRAINT valid_hashes CHECK (
        (commit_hash IS NULL OR length(commit_hash) = 64) AND
        (wasm_hash IS NULL OR length(wasm_hash) = 64) AND
        (input_hash IS NULL OR length(input_hash) = 64) AND
        length(output_hash) = 64 AND
        length(worker_measurement) = 96
    ),
    CONSTRAINT valid_near_data CHECK (
        (request_id IS NULL OR request_id >= 0) AND
        (block_height IS NULL OR block_height >= 0)
    ),
    -- Execute tasks must have input_hash
    CONSTRAINT execute_task_fields CHECK (
        task_type != 'execute' OR input_hash IS NOT NULL
    )
);

-- Indexes for efficient queries
CREATE INDEX IF NOT EXISTS idx_attestations_task_id ON task_attestations(task_id);
CREATE INDEX IF NOT EXISTS idx_attestations_request_id ON task_attestations(request_id) WHERE request_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_attestations_task_type ON task_attestations(task_type);
CREATE INDEX IF NOT EXISTS idx_attestations_caller ON task_attestations(caller_account_id) WHERE caller_account_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_attestations_created_at ON task_attestations(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_attestations_worker_measurement ON task_attestations(worker_measurement);
CREATE INDEX IF NOT EXISTS idx_attestations_repo_url ON task_attestations(repo_url) WHERE repo_url IS NOT NULL;

-- Comments for documentation
COMMENT ON TABLE task_attestations IS 'TDX attestation quotes for both compilation and execution tasks - unified format';
COMMENT ON COLUMN task_attestations.task_id IS 'References task ID from coordinator jobs table';
COMMENT ON COLUMN task_attestations.task_type IS 'Type: compile (GitHub→WASM) or execute (WASM+input→output)';
COMMENT ON COLUMN task_attestations.tdx_quote IS 'Raw Intel TDX quote (binary, 5-8KB) - cryptographic proof from TEE';
COMMENT ON COLUMN task_attestations.worker_measurement IS 'RTMR3 value from TDX quote (worker code measurement)';
COMMENT ON COLUMN task_attestations.request_id IS 'Request ID from NEAR contract';
COMMENT ON COLUMN task_attestations.caller_account_id IS 'NEAR account that initiated the request';
COMMENT ON COLUMN task_attestations.transaction_hash IS 'NEAR transaction hash';
COMMENT ON COLUMN task_attestations.block_height IS 'NEAR block height when request was created';
COMMENT ON COLUMN task_attestations.repo_url IS 'GitHub repository URL (both task types)';
COMMENT ON COLUMN task_attestations.commit_hash IS 'Git commit hash (both task types)';
COMMENT ON COLUMN task_attestations.build_target IS 'Build target like wasm32-wasip1 (both task types)';
COMMENT ON COLUMN task_attestations.wasm_hash IS 'SHA256 of WASM: output for Compile, input for Execute';
COMMENT ON COLUMN task_attestations.input_hash IS 'SHA256 of input data (Execute only, NULL for Compile)';
COMMENT ON COLUMN task_attestations.output_hash IS 'SHA256 of output (both task types, always present)';
