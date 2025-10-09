-- Initial database schema for NEAR Offshore Coordinator

-- Execution requests tracking
CREATE TABLE execution_requests (
    request_id BIGINT PRIMARY KEY,
    data_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_requests_status ON execution_requests(status);
CREATE INDEX idx_requests_created ON execution_requests(created_at);

-- WASM cache metadata (local filesystem with LRU)
CREATE TABLE wasm_cache (
    checksum TEXT PRIMARY KEY,
    repo_url TEXT NOT NULL,
    commit_hash TEXT NOT NULL,
    file_size BIGINT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    last_accessed_at TIMESTAMP NOT NULL DEFAULT NOW(),
    access_count BIGINT NOT NULL DEFAULT 0
);

CREATE INDEX idx_cache_repo_commit ON wasm_cache(repo_url, commit_hash);
CREATE INDEX idx_cache_last_accessed ON wasm_cache(last_accessed_at);

-- Worker authentication tokens
CREATE TABLE worker_auth_tokens (
    token_hash TEXT PRIMARY KEY,
    worker_name TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMP NOT NULL DEFAULT NOW(),
    is_active BOOLEAN NOT NULL DEFAULT true
);

CREATE INDEX idx_tokens_active ON worker_auth_tokens(is_active);

-- Execution history (analytics)
CREATE TABLE execution_history (
    id BIGSERIAL PRIMARY KEY,
    request_id BIGINT REFERENCES execution_requests(request_id),
    worker_id TEXT NOT NULL,
    success BOOLEAN NOT NULL,
    execution_time_ms BIGINT NOT NULL,
    instructions_used BIGINT,
    memory_used_bytes BIGINT,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_history_request ON execution_history(request_id);
CREATE INDEX idx_history_worker ON execution_history(worker_id);
CREATE INDEX idx_history_created ON execution_history(created_at);
