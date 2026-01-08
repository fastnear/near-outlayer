-- Add project fields to jobs table for project-based execution
ALTER TABLE jobs ADD COLUMN IF NOT EXISTS project_uuid VARCHAR(64);
ALTER TABLE jobs ADD COLUMN IF NOT EXISTS project_id VARCHAR(128);

-- Index for querying jobs by project
CREATE INDEX IF NOT EXISTS idx_jobs_project_uuid ON jobs(project_uuid);
CREATE INDEX IF NOT EXISTS idx_jobs_project_id ON jobs(project_id);

COMMENT ON COLUMN jobs.project_uuid IS 'Project UUID for storage encryption key derivation';
COMMENT ON COLUMN jobs.project_id IS 'Project ID (format: owner.near/name) for display';
