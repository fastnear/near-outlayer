use crate::*;

impl Contract {
    pub(crate) fn assert_owner(&self) {
        assert_eq!(
            env::predecessor_account_id(),
            self.owner_id,
            "Only owner can call this method"
        );
    }
}

#[near_bindgen]
impl Contract {
    /// Set new owner (only current owner can call)
    pub fn set_owner(&mut self, new_owner_id: AccountId) {
        self.assert_owner();
        let old_owner = self.owner_id.clone();
        self.owner_id = new_owner_id.clone();

        log!("Owner changed from {} to {}", old_owner, new_owner_id);
    }

    /// Set new operator (only owner can call)
    pub fn set_operator(&mut self, new_operator_id: AccountId) {
        self.assert_owner();
        let old_operator = self.operator_id.clone();
        self.operator_id = new_operator_id.clone();

        log!(
            "Operator changed from {} to {}",
            old_operator,
            new_operator_id
        );
    }

    /// Pause/unpause contract (only owner can call)
    pub fn set_paused(&mut self, paused: bool) {
        self.assert_owner();
        self.paused = paused;

        log!("Contract {}", if paused { "paused" } else { "unpaused" });
    }

    /// Update pricing (only owner can call)
    /// Supports both NEAR pricing (for blockchain transactions) and USD pricing (for HTTPS API)
    pub fn set_pricing(
        &mut self,
        // NEAR pricing
        base_fee: Option<U128>,
        per_million_instructions_fee: Option<U128>,
        per_ms_fee: Option<U128>,
        per_compile_ms_fee: Option<U128>,
        // USD pricing (for HTTPS API)
        base_fee_usd: Option<U128>,
        per_million_instructions_fee_usd: Option<U128>,
        per_sec_fee_usd: Option<U128>,
        per_compile_ms_fee_usd: Option<U128>,
    ) {
        self.assert_owner();

        // NEAR pricing
        if let Some(fee) = base_fee {
            self.base_fee = fee.0;
            log!("Base fee (NEAR) updated to {}", fee.0);
        }
        if let Some(fee) = per_million_instructions_fee {
            self.per_million_instructions_fee = fee.0;
            log!("Per million instructions fee (NEAR) updated to {}", fee.0);
        }
        if let Some(fee) = per_ms_fee {
            self.per_ms_fee = fee.0;
            log!("Per millisecond fee (NEAR, execution) updated to {}", fee.0);
        }
        if let Some(fee) = per_compile_ms_fee {
            self.per_compile_ms_fee = fee.0;
            log!("Per millisecond fee (NEAR, compilation) updated to {}", fee.0);
        }

        // USD pricing
        if let Some(fee) = base_fee_usd {
            self.base_fee_usd = fee.0;
            log!("Base fee (USD) updated to {}", fee.0);
        }
        if let Some(fee) = per_million_instructions_fee_usd {
            self.per_million_instructions_fee_usd = fee.0;
            log!("Per million instructions fee (USD) updated to {}", fee.0);
        }
        if let Some(fee) = per_sec_fee_usd {
            self.per_sec_fee_usd = fee.0;
            log!("Per second fee (USD, execution) updated to {}", fee.0);
        }
        if let Some(fee) = per_compile_ms_fee_usd {
            self.per_compile_ms_fee_usd = fee.0;
            log!("Per millisecond fee (USD, compilation) updated to {}", fee.0);
        }
    }

    /// Set payment token contract for HTTPS API (only owner can call)
    /// This is the stablecoin contract used for Payment Keys (e.g., "usdt.tether-token.near")
    pub fn set_payment_token_contract(&mut self, token_contract: Option<AccountId>) {
        self.assert_owner();
        self.payment_token_contract = token_contract.clone();
        match token_contract {
            Some(contract) => log!("Payment token contract set to {}", contract),
            None => log!("Payment token contract cleared"),
        }
    }

    /// Emergency function to cancel pending execution and refund payer (only owner can call)
    pub fn emergency_cancel_execution(&mut self, request_id: u64) {
        self.assert_owner();

        if let Some(request) = self.pending_requests.remove(&request_id) {
            // Refund payment to payer
            near_sdk::Promise::new(request.payer_account_id.clone())
                .transfer(NearToken::from_yoctonear(request.payment));

            log!(
                "Emergency cancelled execution {} and refunded {} yoctoNEAR to {}",
                request_id,
                request.payment,
                request.payer_account_id
            );
        } else {
            env::panic_str("Execution request not found");
        }
    }

    /// Set event metadata (only owner can call)
    /// Used to customize event standard name and version for different deployments
    pub fn set_event_metadata(&mut self, standard: Option<String>, version: Option<String>) {
        self.assert_owner();

        if let Some(s) = standard {
            self.event_standard = s.clone();
            log!("Event standard updated to {}", s);
        }
        if let Some(v) = version {
            self.event_version = v.clone();
            log!("Event version updated to {}", v);
        }
    }

    /// Admin method to clear all pending requests (only owner can call)
    /// Used for emergency cleanup or testing
    ///
    /// # Arguments
    /// * `limit` - Maximum number of requests to clear in this call (to avoid gas limits)
    ///
    /// # Returns
    /// Number of requests cleared
    pub fn clear_all_pending_requests(&mut self, limit: Option<u64>) -> u64 {
        self.assert_owner();

        let max_limit = limit.unwrap_or(100); // Default to 100 to avoid gas issues
        let mut cleared = 0;

        for request_id in 0..self.next_request_id {
            if cleared >= max_limit {
                break;
            }

            if let Some(request) = self.pending_requests.remove(&request_id) {
                // Refund payment to payer
                near_sdk::Promise::new(request.payer_account_id.clone())
                    .transfer(NearToken::from_yoctonear(request.payment));

                log!(
                    "Cleared request {} and refunded {} yoctoNEAR to {}",
                    request_id,
                    request.payment,
                    request.payer_account_id
                );

                cleared += 1;
            }
        }

        log!("Cleared {} pending requests", cleared);
        cleared
    }
}
