use crate::*;

#[near_bindgen]
impl Contract {
    /// Get execution request by ID
    pub fn get_request(&self, request_id: u64) -> Option<ExecutionRequest> {
        self.pending_requests.get(&request_id)
    }

    /// Get contract statistics
    pub fn get_stats(&self) -> (u64, U128) {
        (self.total_executions, U128(self.total_fees_collected))
    }

    /// Get current pricing
    pub fn get_pricing(&self) -> (U128, U128, U128, U128) {
        (
            U128(self.base_fee),
            U128(self.per_instruction_fee),
            U128(self.per_mb_fee),
            U128(self.per_second_fee),
        )
    }

    /// Check if contract is paused
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Get owner and operator
    pub fn get_config(&self) -> (AccountId, AccountId) {
        (self.owner_id.clone(), self.operator_id.clone())
    }
}
