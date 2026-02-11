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

    /// Get current NEAR pricing (for backward compatibility)
    /// Returns: (base_fee, per_million_instructions_fee, per_ms_fee, per_compile_ms_fee)
    pub fn get_pricing(&self) -> (U128, U128, U128, U128) {
        (
            U128(self.base_fee),
            U128(self.per_million_instructions_fee),
            U128(self.per_ms_fee),
            U128(self.per_compile_ms_fee),
        )
    }

    /// Get full pricing (NEAR and USD)
    /// USD pricing is for HTTPS API, values are in minimal token units (e.g., 1 = 0.000001 USDT)
    pub fn get_pricing_full(&self) -> PricingView {
        PricingView {
            // NEAR pricing
            base_fee: U128(self.base_fee),
            per_million_instructions_fee: U128(self.per_million_instructions_fee),
            per_ms_fee: U128(self.per_ms_fee),
            per_compile_ms_fee: U128(self.per_compile_ms_fee),
            // USD pricing
            base_fee_usd: U128(self.base_fee_usd),
            per_million_instructions_fee_usd: U128(self.per_million_instructions_fee_usd),
            per_sec_fee_usd: U128(self.per_sec_fee_usd),
            per_compile_ms_fee_usd: U128(self.per_compile_ms_fee_usd),
        }
    }

    /// Get payment token contract for HTTPS API
    pub fn get_payment_token_contract(&self) -> Option<AccountId> {
        self.payment_token_contract.clone()
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

    /// Get event metadata (standard name and version)
    /// Used by workers to filter events
    pub fn get_event_metadata(&self) -> (String, String) {
        (self.event_standard.clone(), self.event_version.clone())
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

    /// Get developer earnings balance for an account (stablecoin)
    ///
    /// Earnings are credited when users call request_execution with attached_usd
    /// for projects owned by this account.
    ///
    /// # Arguments
    /// * `account_id` - Developer account (project owner)
    ///
    /// # Returns
    /// Balance in minimal stablecoin units (e.g., 1000000 = 1 USD with 6 decimals)
    pub fn get_developer_earnings(&self, account_id: AccountId) -> U128 {
        U128(self.developer_earnings.get(&account_id).unwrap_or(0))
    }

    /// Get user's stablecoin balance for attached_usd payments
    ///
    /// Users deposit stablecoins via ft_transfer_call with action=deposit_balance.
    /// This balance is used when calling request_execution with attached_usd parameter.
    ///
    /// # Arguments
    /// * `account_id` - User account
    ///
    /// # Returns
    /// Balance in minimal stablecoin units (e.g., 1000000 = 1 USD with 6 decimals)
    pub fn get_user_stablecoin_balance(&self, account_id: AccountId) -> U128 {
        U128(self.user_stablecoin_balances.get(&account_id).unwrap_or(0))
    }

    /// Get IDs of all pending execution requests with pagination
    ///
    /// # Arguments
    /// * `from_index` - Starting index (default: 0)
    /// * `limit` - Maximum number of IDs to return (default: 100)
    ///
    /// # Returns
    /// Vector of pending request IDs
    pub fn get_pending_request_ids(&self, from_index: Option<u64>, limit: Option<u64>) -> Vec<u64> {
        let from = from_index.unwrap_or(0);
        let max_limit = limit.unwrap_or(100);

        let mut result = Vec::new();
        let mut skipped = 0u64;

        for request_id in 0..self.next_request_id {
            if self.pending_requests.contains_key(&request_id) {
                if skipped < from {
                    skipped += 1;
                    continue;
                }
                result.push(request_id);
                if result.len() as u64 >= max_limit {
                    break;
                }
            }
        }

        result
    }

    /// Get the next available payment key nonce for a user.
    ///
    /// Scans existing payment key secrets and returns max(nonce) + 1.
    /// Returns 1 if the user has no payment keys yet.
    pub fn get_next_payment_key_nonce(&self, account_id: AccountId) -> u32 {
        let user_secrets = self.user_secrets_index.get(&account_id);
        match user_secrets {
            Some(secrets_set) => {
                let max_nonce = secrets_set
                    .iter()
                    .filter(|key| {
                        matches!(
                            key.accessor,
                            SecretAccessor::System(SystemSecretType::PaymentKey)
                        )
                    })
                    .filter_map(|key| key.profile.parse::<u32>().ok())
                    .max()
                    .unwrap_or(0);
                max_nonce + 1
            }
            None => 1,
        }
    }
}
