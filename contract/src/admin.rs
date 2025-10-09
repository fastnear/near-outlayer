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
        per_mb_fee: Option<U128>,
        per_second_fee: Option<U128>,
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
        if let Some(fee) = per_mb_fee {
            self.per_mb_fee = fee.0;
            log!("Per MB fee updated to {}", fee.0);
        }
        if let Some(fee) = per_second_fee {
            self.per_second_fee = fee.0;
            log!("Per second fee updated to {}", fee.0);
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
}
