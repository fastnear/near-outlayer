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
    pub fn get_pricing(&self) -> (U128, U128, U128) {
        (
            U128(self.base_fee),
            U128(self.per_instruction_fee),
            U128(self.per_ms_fee),
        )
    }

    /// Estimate cost for given resource limits
    pub fn estimate_execution_cost(&self, resource_limits: Option<ResourceLimits>) -> U128 {
        let limits = resource_limits.unwrap_or_default();
        U128(self.estimate_cost(&limits))
    }

    /// Get maximum resource limits (hard caps)
    pub fn get_max_limits(&self) -> (u64, u64) {
        (MAX_INSTRUCTIONS, MAX_EXECUTION_SECONDS)
    }

    /// Check if contract is paused
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Get owner and operator
    pub fn get_config(&self) -> (AccountId, AccountId) {
        (self.owner_id.clone(), self.operator_id.clone())
    }

    /// Get keystore public key (for encrypting secrets)
    ///
    /// Returns None if keystore is not configured.
    /// Users should encrypt their secrets with this public key before calling request_execution.
    pub fn get_keystore_pubkey(&self) -> Option<String> {
        self.keystore_pubkey.clone()
    }

    /// Get keystore account ID
    pub fn get_keystore_account(&self) -> Option<AccountId> {
        self.keystore_account_id.clone()
    }
}
