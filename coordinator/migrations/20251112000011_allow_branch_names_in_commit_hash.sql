-- Allow branch names in commit_hash field (not just 64-char SHA256)
--
-- Background: commit_hash can be:
-- - Git SHA1 (40 hex chars)
-- - Branch name ("main", "dev", etc - any length)
-- - SHA256 (64 hex chars)
--
-- Previous constraint required 64 chars for all normal tasks.
-- This migration removes the commit_hash length requirement entirely.

-- Drop old valid_hashes constraint
ALTER TABLE task_attestations DROP CONSTRAINT IF EXISTS valid_hashes;

-- Add new valid_hashes constraint without commit_hash length requirement
ALTER TABLE task_attestations ADD CONSTRAINT valid_hashes CHECK (
    -- worker_measurement must always be 96 chars (RTMR3 from real TDX quote)
    length(worker_measurement) = 96 AND
    (
        (task_id < 0) OR  -- Special tasks: skip all hash length validation
        (
            -- Normal tasks: enforce strict hash lengths EXCEPT for commit_hash
            (wasm_hash IS NULL OR length(wasm_hash) = 64) AND
            (input_hash IS NULL OR length(input_hash) = 64) AND
            length(output_hash) = 64
            -- commit_hash: any length allowed (branch names, Git SHA1, SHA256)
        )
    )
);
