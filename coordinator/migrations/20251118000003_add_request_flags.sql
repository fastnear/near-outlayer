-- Add compile_only, force_rebuild, store_on_fastfs flags to execution_requests

ALTER TABLE execution_requests
ADD COLUMN IF NOT EXISTS compile_only BOOLEAN NOT NULL DEFAULT FALSE,
ADD COLUMN IF NOT EXISTS force_rebuild BOOLEAN NOT NULL DEFAULT FALSE,
ADD COLUMN IF NOT EXISTS store_on_fastfs BOOLEAN NOT NULL DEFAULT FALSE;
