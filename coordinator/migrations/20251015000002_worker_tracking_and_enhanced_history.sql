-- Enhanced execution history and worker tracking

-- Add missing columns to execution_history
ALTER TABLE execution_history
ADD COLUMN data_id TEXT,
ADD COLUMN resolve_tx_id TEXT,
ADD COLUMN user_account_id TEXT,
ADD COLUMN near_payment_yocto TEXT; -- Store as text to avoid overflow (u128)

-- Add indexes for new columns
CREATE INDEX idx_history_data_id ON execution_history(data_id);
CREATE INDEX idx_history_user ON execution_history(user_account_id);

-- Worker status tracking
CREATE TABLE worker_status (
    worker_id TEXT PRIMARY KEY,
    worker_name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'offline', -- 'online', 'busy', 'offline'
    current_task_id BIGINT,
    last_heartbeat_at TIMESTAMP NOT NULL DEFAULT NOW(),
    last_task_completed_at TIMESTAMP,
    total_tasks_completed BIGINT NOT NULL DEFAULT 0,
    total_tasks_failed BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_worker_status ON worker_status(status);
CREATE INDEX idx_worker_heartbeat ON worker_status(last_heartbeat_at);

-- Enhanced WASM cache metadata with build_target
ALTER TABLE wasm_cache
ADD COLUMN build_target TEXT DEFAULT 'wasm32-wasip1';

-- Add composite index for repo + commit + target
CREATE INDEX idx_cache_repo_commit_target ON wasm_cache(repo_url, commit_hash, build_target);
