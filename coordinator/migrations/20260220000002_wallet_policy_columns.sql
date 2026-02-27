-- Add policy_json and frozen columns for on-chain policy sync
ALTER TABLE wallet_accounts ADD COLUMN IF NOT EXISTS policy_json JSONB;
ALTER TABLE wallet_accounts ADD COLUMN IF NOT EXISTS frozen BOOLEAN NOT NULL DEFAULT FALSE;

-- Add source column to wallet_api_keys for distinguishing bootstrap vs policy-sourced keys
ALTER TABLE wallet_api_keys ADD COLUMN IF NOT EXISTS source TEXT NOT NULL DEFAULT 'bootstrap';
