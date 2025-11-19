-- Add compile_time_ms column to jobs table
-- This stores actual compilation time from compiler worker for executor to use

ALTER TABLE jobs ADD COLUMN compile_time_ms BIGINT;

-- Index for faster lookups (optional)
CREATE INDEX idx_jobs_compile_time ON jobs(compile_time_ms) WHERE compile_time_ms IS NOT NULL;
