-- Add new job statuses for better error classification
-- This allows distinguishing between technical failures and business logic errors

-- 1. Add new status values
ALTER TABLE jobs DROP CONSTRAINT IF EXISTS jobs_status_check;

ALTER TABLE jobs ADD CONSTRAINT jobs_status_check CHECK (
    status IN (
        'pending',              -- Waiting to be picked up by worker
        'in_progress',          -- Currently being processed
        'completed',            -- ✅ Successfully completed
        'failed',               -- ❌ Technical/infrastructure error (NEAR RPC down, coordinator issue, etc.)
        'compilation_failed',   -- ❌ Compilation error (repo doesn't exist, syntax error, build failed)
        'execution_failed',     -- ❌ WASM execution error (panic, trap, timeout)
        'access_denied',        -- ❌ Access denied to secrets (Whitelist, AccountPattern, attestation)
        'insufficient_payment', -- ❌ Not enough payment for requested resources
        'custom'                -- ❌ Custom error (see error_details)
    )
);

-- 2. Add error_details column for detailed error messages
ALTER TABLE jobs ADD COLUMN IF NOT EXISTS error_details TEXT;

-- 3. Create index for filtering by error status
CREATE INDEX IF NOT EXISTS idx_jobs_error_status ON jobs(status)
    WHERE status IN ('failed', 'compilation_failed', 'execution_failed', 'access_denied', 'insufficient_payment', 'custom');

-- 4. Add comment for documentation
COMMENT ON COLUMN jobs.status IS 'Job status: pending/in_progress/completed/failed/compilation_failed/execution_failed/access_denied/insufficient_payment/custom';
COMMENT ON COLUMN jobs.error_details IS 'Detailed error message for failed jobs';
