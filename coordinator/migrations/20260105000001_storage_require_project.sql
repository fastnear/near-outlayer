-- Remove standalone storage support - project_uuid is now required
-- Storage only works for projects (allows proper cleanup by project owner)

-- First, delete any orphan storage data without project_uuid
DELETE FROM storage_data WHERE project_uuid IS NULL;

-- Make project_uuid NOT NULL
ALTER TABLE storage_data ALTER COLUMN project_uuid SET NOT NULL;

-- Update comments
COMMENT ON COLUMN storage_data.project_uuid IS 'Project UUID - required for all storage operations';

-- Update storage_usage table - remove wasm_hash-only rows and constraint
DELETE FROM storage_usage WHERE project_uuid IS NULL;

-- Drop the old constraint that allows either project_uuid OR wasm_hash
ALTER TABLE storage_usage DROP CONSTRAINT IF EXISTS storage_usage_check_one_id;

-- Make project_uuid NOT NULL in storage_usage
ALTER TABLE storage_usage ALTER COLUMN project_uuid SET NOT NULL;

-- Drop the wasm_hash unique constraint (no longer needed as primary identifier)
ALTER TABLE storage_usage DROP CONSTRAINT IF EXISTS storage_usage_wasm_unique;

-- Update comment
COMMENT ON TABLE storage_usage IS 'Storage usage tracking per project/account (standalone storage removed)';
COMMENT ON COLUMN storage_usage.project_uuid IS 'Project UUID - required for all storage operations';
