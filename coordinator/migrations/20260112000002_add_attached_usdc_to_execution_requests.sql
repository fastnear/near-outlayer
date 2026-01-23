-- Add attached_usdc to execution_requests table
-- This stores the stablecoin amount attached by user for project developer payment
-- Value is in minimal token units (e.g., 1000000 = 1 USDC with 6 decimals)

ALTER TABLE execution_requests ADD COLUMN IF NOT EXISTS attached_usdc TEXT;
