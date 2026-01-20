-- Rename usdc columns to usd (generic stablecoin support)
-- This is a follow-up migration after the initial columns were created with usdc names

-- Rename in execution_requests
ALTER TABLE execution_requests RENAME COLUMN attached_usdc TO attached_usd;

-- Rename in earnings_history
ALTER TABLE earnings_history RENAME COLUMN attached_usdc TO attached_usd;
ALTER TABLE earnings_history RENAME COLUMN refund_usdc TO refund_usd;
