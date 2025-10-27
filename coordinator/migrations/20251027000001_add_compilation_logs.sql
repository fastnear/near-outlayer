-- ⚠️ CRITICAL SECURITY WARNING ⚠️
-- This table contains RAW stderr/stdout from compilation/execution
-- NEVER expose this table via public API endpoints (/public/*)
-- Access ONLY via /admin/* routes on localhost/SSH
-- Potential exploit: malicious code could output system file contents to stderr
-- Example: `cat /etc/passwd` in build.rs could leak server secrets

-- Create system_hidden_logs table for storing raw logs (admin-only access)
-- This table stores raw stderr/stdout/execution details for debugging purposes
-- NOT exposed via public API for security reasons

CREATE TABLE IF NOT EXISTS system_hidden_logs (
    id BIGSERIAL PRIMARY KEY,
    request_id BIGINT NOT NULL,
    job_id BIGINT,
    log_type VARCHAR(50) NOT NULL, -- 'compilation' or 'execution'
    stderr TEXT,
    stdout TEXT,
    exit_code INTEGER,
    execution_error TEXT, -- For WASM execution errors
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

-- Index for fast lookups by request_id
CREATE INDEX IF NOT EXISTS idx_system_hidden_logs_request_id ON system_hidden_logs(request_id);

-- Index for log type filtering
CREATE INDEX IF NOT EXISTS idx_system_hidden_logs_log_type ON system_hidden_logs(log_type);

-- Index for chronological queries
CREATE INDEX IF NOT EXISTS idx_system_hidden_logs_created_at ON system_hidden_logs(created_at DESC);

COMMENT ON TABLE system_hidden_logs IS '⚠️ ADMIN ONLY - Raw logs with potential security risks - NEVER expose via /public/* API';
COMMENT ON COLUMN system_hidden_logs.log_type IS 'Type of log: compilation, execution';
COMMENT ON COLUMN system_hidden_logs.stderr IS '⚠️ SECURITY RISK - Raw stderr may contain system file contents from malicious code';
COMMENT ON COLUMN system_hidden_logs.stdout IS '⚠️ SECURITY RISK - Raw stdout may contain system file contents from malicious code';
COMMENT ON COLUMN system_hidden_logs.execution_error IS 'Raw WASM execution error message';
