-- Fix execute_task_fields constraint to allow startup attestations (task_id = -1)
--
-- Background: Startup attestation tasks use task_id = -1 and task_type = 'execute',
-- but have input_hash = NULL. The original constraint required input_hash for all
-- execute tasks, which breaks startup attestations.
--
-- This mirrors the fix in migration 20251112000010 for valid_hashes constraint.

-- Drop old execute_task_fields constraint
ALTER TABLE task_attestations DROP CONSTRAINT IF EXISTS execute_task_fields;

-- Add new execute_task_fields constraint that skips validation for task_id < 0
ALTER TABLE task_attestations ADD CONSTRAINT execute_task_fields CHECK (
    task_id < 0 OR  -- Special tasks (startup): skip input_hash requirement
    task_type != 'execute' OR input_hash IS NOT NULL
);
