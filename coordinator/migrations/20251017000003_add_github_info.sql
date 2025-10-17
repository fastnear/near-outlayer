-- Add GitHub repo and commit info to execution history

ALTER TABLE execution_history
ADD COLUMN IF NOT EXISTS github_repo TEXT,
ADD COLUMN IF NOT EXISTS github_commit TEXT;

CREATE INDEX IF NOT EXISTS idx_history_github_repo ON execution_history(github_repo);
