-- Fix valid_hashes constraint to allow non-standard hash values for startup tasks (task_id = -1)
--
-- Background: Startup attestation tasks (task_id = -1) use placeholder values like:
-- - output_hash = "worker_startup" (14 chars, not 64)
-- - commit_hash = "startup" (7 chars, not 64)
-- These are legitimate for startup tasks but fail the 64-char validation.
--
-- This migration relaxes hash length validation for task_id < 0 while keeping
-- strict validation for normal tasks (task_id >= 0).

-- Drop old valid_hashes constraint
ALTER TABLE task_attestations DROP CONSTRAINT IF EXISTS valid_hashes;

-- Add new valid_hashes constraint that skips hash length checks for task_id < 0
ALTER TABLE task_attestations ADD CONSTRAINT valid_hashes CHECK (
    (task_id < 0) OR  -- Special tasks: skip hash length validation entirely
    (
        -- Normal tasks: enforce strict hash lengths
        (commit_hash IS NULL OR length(commit_hash) = 64) AND
        (wasm_hash IS NULL OR length(wasm_hash) = 64) AND
        (input_hash IS NULL OR length(input_hash) = 64) AND
        length(output_hash) = 64
    )
) AND
-- worker_measurement must always be 96 chars (RTMR3 from real TDX quote)
length(worker_measurement) = 96;
