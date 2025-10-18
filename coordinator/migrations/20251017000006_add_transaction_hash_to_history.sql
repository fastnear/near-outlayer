-- Add transaction_hash to execution_history table
ALTER TABLE execution_history
ADD COLUMN transaction_hash TEXT;

-- Add index for transaction hash lookups
CREATE INDEX idx_history_transaction ON execution_history(transaction_hash);
