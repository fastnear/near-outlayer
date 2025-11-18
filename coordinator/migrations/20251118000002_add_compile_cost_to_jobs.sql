-- Add compile_cost and compile_error columns to jobs table
-- These are used to pass compilation results from compile jobs to execute jobs

-- Add compile_cost_yocto to store the cost of compilation
ALTER TABLE jobs ADD COLUMN IF NOT EXISTS compile_cost_yocto TEXT;

-- Add compile_error to store error message if compilation failed
ALTER TABLE jobs ADD COLUMN IF NOT EXISTS compile_error TEXT;

-- Add compile_time_ms to store how long compilation took
ALTER TABLE jobs ADD COLUMN IF NOT EXISTS compile_time_ms BIGINT;

-- Comment explaining the flow:
-- 1. Compile job completes -> updates jobs.compile_cost_yocto, compile_time_ms (or compile_error if failed)
-- 2. Execute job claims work -> reads compile_cost_yocto from compile job to include in total cost
-- 3. If compile_error is set, executor sends resolve_execution with that error
