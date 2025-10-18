-- Add actual_cost_yocto field to track real charged amount (after refund)
ALTER TABLE execution_history 
ADD COLUMN actual_cost_yocto TEXT;

-- For old records, set actual_cost = near_payment (before we tracked refunds)
UPDATE execution_history 
SET actual_cost_yocto = near_payment_yocto 
WHERE actual_cost_yocto IS NULL AND near_payment_yocto IS NOT NULL;
