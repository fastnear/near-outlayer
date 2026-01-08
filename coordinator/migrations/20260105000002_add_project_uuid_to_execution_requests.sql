-- Add project_uuid to execution_requests table
-- This allows project_uuid to be preserved when creating execute tasks after compilation

ALTER TABLE execution_requests ADD COLUMN IF NOT EXISTS project_uuid TEXT;

-- Index for querying by project
CREATE INDEX IF NOT EXISTS idx_execution_requests_project_uuid ON execution_requests(project_uuid);
