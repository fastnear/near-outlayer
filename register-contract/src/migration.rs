use crate::*;
use near_sdk::borsh::BorshDeserialize;

/// Previous on-chain layout: a single `quote_collateral: Option<String>`.
/// The live `worker.outlayer.testnet` state matches this (owner + init account +
/// approved_measurements + one collateral). `migrate()` moves that single collateral into the
/// new multi-slot `collaterals` vec at slot 0 (e.g. the existing Phala 20a06f000000 collateral),
/// preserving approved_measurements. Field order MUST match the serialized layout.
#[derive(BorshDeserialize)]
#[borsh(crate = "near_sdk::borsh")]
#[allow(dead_code)] // fields needed for Borsh deserialization during migration
pub struct RegisterContractV2 {
    pub owner_id: AccountId,
    pub init_worker_account: AccountId,
    pub approved_measurements: Vec<ApprovedMeasurements>,
    pub quote_collateral: Option<String>,
    pub outlayer_contract_id: AccountId,
}

#[near_bindgen]
impl RegisterContract {
    #[private]
    #[init(ignore_state)]
    pub fn migrate() -> Self {
        let old: RegisterContractV2 = env::state_read().expect("Failed to read old state");

        Self {
            owner_id: old.owner_id,
            init_worker_account: old.init_worker_account,
            approved_measurements: old.approved_measurements,
            // Move the single cached collateral into slot 0; owner adds others (self-hosted
            // FMSPC) via `update_collateral(collateral, 1)`.
            collaterals: old.quote_collateral.map(|c| vec![c]).unwrap_or_default(),
            outlayer_contract_id: old.outlayer_contract_id,
        }
    }
}
