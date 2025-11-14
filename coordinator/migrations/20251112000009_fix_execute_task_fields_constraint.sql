-- Fix CHECK constraint to allow NULL input_hash for startup tasks (task_id = -1)
--
-- Background: Startup attestation tasks have task_id = -1 and legitimately have NULL values
-- for input_hash, commit_hash, wasm_hash, output_hash because they're not real execution tasks.
-- They just attest to worker's TEE capabilities at startup.
--
-- This migration modifies the execute_task_fields constraint to allow NULL input_hash
-- for special tasks (task_id < 0) while still requiring it for normal execution tasks.

-- Drop old constraint
ALTER TABLE task_attestations DROP CONSTRAINT IF EXISTS execute_task_fields;

-- Add new constraint that allows NULL input_hash for task_id < 0
ALTER TABLE task_attestations ADD CONSTRAINT execute_task_fields CHECK (
    task_type != 'execute' OR task_id < 0 OR input_hash IS NOT NULL
);
