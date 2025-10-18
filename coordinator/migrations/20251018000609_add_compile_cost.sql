-- Add compile_cost_yocto field to track compilation cost separately
ALTER TABLE execution_history 
ADD COLUMN compile_cost_yocto TEXT;

-- Note: This is different from actual_cost_yocto which includes both compile + execute costs
-- compile_cost_yocto = compile_time_ms * per_compile_ms_fee (calculated by worker)
