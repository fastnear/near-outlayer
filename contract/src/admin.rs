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
    pub fn set_pricing(
        &mut self,
        base_fee: Option<U128>,
        per_instruction_fee: Option<U128>,
        per_ms_fee: Option<U128>,
        per_compile_ms_fee: Option<U128>,
    ) {
        self.assert_owner();

        if let Some(fee) = base_fee {
            self.base_fee = fee.0;
            log!("Base fee updated to {}", fee.0);
        }
        if let Some(fee) = per_instruction_fee {
            self.per_instruction_fee = fee.0;
            log!("Per instruction fee updated to {}", fee.0);
        }
        if let Some(fee) = per_ms_fee {
            self.per_ms_fee = fee.0;
            log!("Per millisecond fee (execution) updated to {}", fee.0);
        }
        if let Some(fee) = per_compile_ms_fee {
            self.per_compile_ms_fee = fee.0;
            log!("Per millisecond fee (compilation) updated to {}", fee.0);
        }
    }

    /// Emergency function to cancel pending execution and refund user (only owner can call)
    pub fn emergency_cancel_execution(&mut self, request_id: u64) {
        self.assert_owner();

        if let Some(request) = self.pending_requests.remove(&request_id) {
            // Refund payment to user
            near_sdk::Promise::new(request.sender_id.clone())
                .transfer(NearToken::from_yoctonear(request.payment));

            log!(
                "Emergency cancelled execution {} and refunded {} yoctoNEAR to {}",
                request_id,
                request.payment,
                request.sender_id
            );
        } else {
            env::panic_str("Execution request not found");
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
                // Refund payment to user
                near_sdk::Promise::new(request.sender_id.clone())
                    .transfer(NearToken::from_yoctonear(request.payment));

                log!(
                    "Cleared request {} and refunded {} yoctoNEAR to {}",
                    request_id,
                    request.payment,
                    request.sender_id
                );

                cleared += 1;
            }
        }

        log!("Cleared {} pending requests", cleared);
        cleared
    }
}
