-- Add project_id to execution_requests table
-- This allows project_id to be preserved when creating execute tasks after compilation
-- project_id is used to determine secret accessor type (Project vs Repo vs WasmHash)

ALTER TABLE execution_requests ADD COLUMN IF NOT EXISTS project_id TEXT;

-- Index for querying by project_id
CREATE INDEX IF NOT EXISTS idx_execution_requests_project_id ON execution_requests(project_id);
