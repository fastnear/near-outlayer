-- Add compile_result column to execution_requests table
-- This stores the result from compile job to pass to executor
-- (e.g., FastFS URL or compilation error message)

ALTER TABLE execution_requests
ADD COLUMN IF NOT EXISTS compile_result TEXT;
