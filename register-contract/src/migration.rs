use crate::*;
use near_sdk::borsh::BorshDeserialize;
use near_sdk::collections::UnorderedSet;

#[derive(BorshDeserialize)]
#[borsh(crate = "near_sdk::borsh")]
#[allow(dead_code)] // Fields needed for Borsh deserialization during migration
pub struct RegisterContractV1 {
    pub owner_id: AccountId,
    pub init_worker_account: AccountId,
    pub approved_rtmr3: UnorderedSet<String>,
    pub quote_collateral: Option<String>,
    pub outlayer_contract_id: AccountId,
}

#[near_bindgen]
impl RegisterContract {
    #[private]
    #[init(ignore_state)]
    pub fn migrate() -> Self {
        let old_state: RegisterContractV1 = env::state_read().expect("Failed to read old state");

        // Drop approved_rtmr3 data — admin will add full measurements after migration
        Self {
            owner_id: old_state.owner_id,
            init_worker_account: old_state.init_worker_account,
            approved_measurements: Vec::new(),
            quote_collateral: old_state.quote_collateral,
            outlayer_contract_id: old_state.outlayer_contract_id,
        }
    }
}
