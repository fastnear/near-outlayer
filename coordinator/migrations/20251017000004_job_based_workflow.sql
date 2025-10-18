-- Job-based workflow migration
-- Each request from contract can spawn multiple jobs (compile + execute)

-- Drop old table
DROP TABLE IF EXISTS execution_requests CASCADE;

-- Jobs table - tracks individual units of work
CREATE TABLE jobs (
    job_id BIGSERIAL PRIMARY KEY,
    request_id BIGINT NOT NULL,
    data_id TEXT NOT NULL,
    job_type TEXT NOT NULL CHECK (job_type IN ('compile', 'execute')),
    worker_id TEXT,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'in_progress', 'completed', 'failed')),
    wasm_checksum TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMP,

    -- One job of each type per (request_id, data_id)
    UNIQUE (request_id, data_id, job_type)
);

CREATE INDEX idx_jobs_status ON jobs(status, created_at);
CREATE INDEX idx_jobs_request_id ON jobs(request_id);
CREATE INDEX idx_jobs_worker ON jobs(worker_id, status);
CREATE INDEX idx_jobs_data_id ON jobs(data_id);

-- Execution history - keep detailed logs
-- Foreign key references job_id instead of request_id
ALTER TABLE execution_history
    DROP CONSTRAINT IF EXISTS execution_history_request_id_fkey;

-- Add job_id column to execution_history
ALTER TABLE execution_history
    ADD COLUMN job_id BIGINT REFERENCES jobs(job_id);

-- Update execution_history structure for job tracking
ALTER TABLE execution_history
    ADD COLUMN job_type TEXT CHECK (job_type IN ('compile', 'execute'));

-- Add compile metrics
ALTER TABLE execution_history
    ADD COLUMN compile_time_ms BIGINT;

CREATE INDEX idx_history_job ON execution_history(job_id);
