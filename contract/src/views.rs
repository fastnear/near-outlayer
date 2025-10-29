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
    /// Returns: (base_fee, per_million_instructions_fee, per_ms_fee, per_compile_ms_fee)
    pub fn get_pricing(&self) -> (U128, U128, U128, U128) {
        (
            U128(self.base_fee),
            U128(self.per_million_instructions_fee),
            U128(self.per_ms_fee),
            U128(self.per_compile_ms_fee),
        )
    }

    /// Estimate cost for given resource limits
    pub fn estimate_execution_cost(&self, resource_limits: Option<ResourceLimits>) -> U128 {
        let limits = resource_limits.unwrap_or_default();
        U128(self.estimate_cost(&limits))
    }

    /// Get maximum resource limits (hard caps)
    /// Returns: (max_instructions, max_execution_seconds, max_compilation_seconds)
    pub fn get_max_limits(&self) -> (u64, u64, u64) {
        (MAX_INSTRUCTIONS, MAX_EXECUTION_SECONDS, MAX_COMPILATION_SECONDS)
    }

    /// Check if contract is paused
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Get owner and operator
    pub fn get_config(&self) -> (AccountId, AccountId) {
        (self.owner_id.clone(), self.operator_id.clone())
    }

    /// Get pending output data for a given request_id
    /// Used by coordinator to check if large output was submitted
    pub fn get_pending_output(&self, request_id: u64) -> Option<ExecutionOutput> {
        let request = self.pending_requests.get(&request_id)?;
        // Convert from internal storage format to ExecutionOutput
        request.pending_output.map(|stored| stored.into())
    }

    /// Check if pending output exists for a given request_id
    pub fn has_pending_output(&self, request_id: u64) -> bool {
        self.pending_requests
            .get(&request_id)
            .map(|req| req.output_submitted)
            .unwrap_or(false)
    }
}
