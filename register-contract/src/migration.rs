use crate::*;
use near_sdk::borsh::BorshDeserialize;
use near_sdk::collections::{UnorderedSet};

#[derive(BorshDeserialize)]
#[borsh(crate = "near_sdk::borsh")]
#[allow(dead_code)] // Fields needed for Borsh deserialization during migration
pub struct RegisterContractV0 {    
    pub owner_id: AccountId,
    pub init_worker_account: AccountId,
    pub approved_rtmr3: UnorderedSet<String>,
    pub quote_collateral: Option<String>,
}

#[near_bindgen]
impl RegisterContract {
    #[private]
    #[init(ignore_state)]
    pub fn migrate(outlayer_contract_id: AccountId) -> Self {
        let old_state: RegisterContractV0 = env::state_read().expect("Failed to read old state");

        Self {
            owner_id: old_state.owner_id,
            init_worker_account: old_state.init_worker_account,
            approved_rtmr3: old_state.approved_rtmr3,
            quote_collateral: old_state.quote_collateral,
            outlayer_contract_id: outlayer_contract_id
        }
    }    
}
