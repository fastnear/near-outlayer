-- Add user and payment information to jobs table
-- This information is needed for execution_history and dashboard

ALTER TABLE jobs
ADD COLUMN user_account_id TEXT,
ADD COLUMN near_payment_yocto TEXT,
ADD COLUMN github_repo TEXT,
ADD COLUMN github_commit TEXT,
ADD COLUMN transaction_hash TEXT;

-- Create indexes for faster queries
CREATE INDEX idx_jobs_user ON jobs(user_account_id);
CREATE INDEX idx_jobs_github_repo ON jobs(github_repo);
