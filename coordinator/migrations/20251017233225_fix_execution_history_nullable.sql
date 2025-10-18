-- Make execution_time_ms nullable since compile jobs don't have execution time
ALTER TABLE execution_history 
ALTER COLUMN execution_time_ms DROP NOT NULL;

-- Add default value for existing rows
UPDATE execution_history 
SET execution_time_ms = 0 
WHERE execution_time_ms IS NULL;
