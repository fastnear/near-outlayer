use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, near_bindgen, AccountId, Allowance, NearToken, Promise, PublicKey, BorshStorageKey};
use schemars::JsonSchema;

// Collateral wrapper with Borsh support (same approach as MPC Node)
mod collateral;
use collateral::Collateral;

mod migration;

// Custom getrandom implementation for WASM (same as MPC Node)
// We don't need actual randomness in this contract (only verification)
#[cfg(target_arch = "wasm32")]
use getrandom::{register_custom_getrandom, Error};
#[cfg(target_arch = "wasm32")]
fn randomness_unsupported(_: &mut [u8]) -> Result<(), Error> {
    Err(Error::UNSUPPORTED)
}
#[cfg(target_arch = "wasm32")]
register_custom_getrandom!(randomness_unsupported);

#[derive(BorshSerialize, BorshStorageKey)]
#[borsh(crate = "near_sdk::borsh")]
#[allow(dead_code)] // ApprovedRtmr3 used only in migration deserialization
enum StorageKey {
    ApprovedRtmr3,
}

/// Full TEE measurements for verifying the entire dstack environment.
/// All fields are 96 hex characters (48 bytes).
///
/// Without verifying MRTD + RTMR0-2, an attacker can run a dev dstack image
/// (with SSH enabled), connect to the container, and modify the running code
/// while RTMR3 still passes because the docker-compose is the same.
#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, JsonSchema, Clone, PartialEq, Eq, Debug)]
#[serde(crate = "near_sdk::serde")]
pub struct ApprovedMeasurements {
    /// MRTD - Virtual firmware (TDX module) identity
    pub mrtd: String,
    /// RTMR0 - Bootloader, firmware config
    pub rtmr0: String,
    /// RTMR1 - OS kernel, boot params, initrd
    pub rtmr1: String,
    /// RTMR2 - OS applications layer
    pub rtmr2: String,
    /// RTMR3 - Runtime events (compose-hash, key-provider)
    pub rtmr3: String,
}

/// Register Contract - TEE Worker Key Registration
///
/// This contract verifies TDX attestations and registers worker public keys
/// by adding them as access keys to its own account (the worker account).
///
/// # Security Model
/// 1. Worker generates keypair INSIDE TEE (private key never leaves TEE)
/// 2. Worker generates TDX quote with public key embedded in report_data
/// 3. This contract verifies TDX quote signature (Intel cryptographic proof)
/// 4. This contract extracts MRTD + RTMR0-3 and checks them against approved measurements list
/// 5. This contract adds public key to itself (current_account_id) with permissions for offchainvm_contract_id
/// 6. Result: Public key is cryptographically proven to be generated in approved TEE
///
/// # Usage
/// 1. Deploy contract: `near deploy worker.outlayer.testnet --wasmFile register.wasm`
/// 2. Initialize: `near call worker.outlayer.testnet new '{"owner_id":"outlayer.testnet","init_worker_account":"init-worker.outlayer.testnet"}' --accountId outlayer.testnet`
/// 3. Add approved measurements: `near call worker.outlayer.testnet add_approved_measurements '{"measurements":{...}}' --accountId outlayer.testnet`
/// 4. Init worker calls: `near call worker.outlayer.testnet register_worker_key '{"public_key":"...","tdx_quote_hex":"..."}' --accountId init-worker.outlayer.testnet`
/// 5. Contract verifies and adds key to worker.outlayer.testnet with permissions for outlayer.testnet
#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct RegisterContract {
    /// Owner who can manage approved RTMR3 list
    pub owner_id: AccountId,

    /// Init worker account that can call register_worker_key
    pub init_worker_account: AccountId,

    /// Full TEE measurements approved for worker registration.
    /// Each entry contains MRTD + RTMR0-3 (all must match for registration).
    pub approved_measurements: Vec<ApprovedMeasurements>,

    /// Quote collateral data (cached, updated periodically by owner)
    /// This is Intel's reference data needed for TDX quote verification
    pub quote_collateral: Option<String>,

    pub outlayer_contract_id: AccountId
}

impl Default for RegisterContract {
    fn default() -> Self {
        env::panic_str("RegisterContract must be initialized");
    }
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(crate = "near_sdk::serde")]
pub struct WorkerKeyInfo {
    #[schemars(with = "String")]
    pub public_key: PublicKey,
    pub measurements: ApprovedMeasurements,
    pub registered_at: u64,
}

#[near_bindgen]
impl RegisterContract {
    /// Initialize contract
    #[init]
    pub fn new(owner_id: AccountId, init_worker_account: AccountId, outlayer_contract_id: AccountId) -> Self {
        Self {
            owner_id,
            init_worker_account,
            approved_measurements: Vec::new(),
            quote_collateral: None,
            outlayer_contract_id,
        }
    }

    /// Register worker public key with TEE attestation
    ///
    /// This method:
    /// 1. Verifies TDX quote signature (Intel cryptographic proof)
    /// 2. Extracts RTMR3 from quote
    /// 3. Checks RTMR3 is in approved list
    /// 4. Extracts public key from quote report_data
    /// 5. Verifies public_key parameter matches embedded key
    /// 6. Adds access key to this contract's account (self)
    ///
    /// # Arguments
    /// * `public_key` - Worker's ed25519 public key (generated in TEE)
    /// * `tdx_quote_hex` - Hex-encoded TDX quote from Phala dstack
    ///
    /// # Returns
    /// Promise to add access key to this account
    ///
    /// # Panics
    /// - If TDX quote verification fails
    /// - If RTMR3 not in approved list
    /// - If public key doesn't match quote
    /// - If collateral not cached (use update_collateral first)
    pub fn register_worker_key(
        &mut self,
        public_key: PublicKey,
        tdx_quote_hex: String,
    ) -> Promise {
        // Check that caller is init_worker_account
        assert_eq!(
            env::predecessor_account_id(),
            self.init_worker_account,
            "Only {} can call register_worker_key",
            self.init_worker_account
        );

        // Use ONLY cached collateral (security: prevent custom collateral bypass)
        let collateral = self.quote_collateral.clone()
            .expect("Quote collateral required (cache via update_collateral)");

        // 1. Verify TDX quote and extract full measurements + embedded public key
        let (measurements, embedded_pubkey) =
            self.verify_worker_registration(&tdx_quote_hex, &collateral);

        // Log verification result for admin visibility
        env::log_str(&format!(
            "📋 Verified TDX quote. Worker's TEE-generated key (from quote report_data): {:?}",
            embedded_pubkey
        ));
        env::log_str(&format!(
            "📋 Measurements from TDX quote: mrtd={}, rtmr0={}, rtmr1={}, rtmr2={}, rtmr3={}",
            measurements.mrtd, measurements.rtmr0, measurements.rtmr1,
            measurements.rtmr2, measurements.rtmr3
        ));

        // 2. Check full measurements are approved
        assert!(
            self.approved_measurements.contains(&measurements),
            "Worker measurements not approved. MRTD={}, RTMR0={}, RTMR1={}, RTMR2={}, RTMR3={}. Contact admin to add via add_approved_measurements.",
            measurements.mrtd, measurements.rtmr0, measurements.rtmr1,
            measurements.rtmr2, measurements.rtmr3
        );

        // 3. Verify public_key matches the one embedded in TDX quote
        assert_eq!(
            embedded_pubkey, public_key,
            "Public key mismatch: provided key doesn't match TDX quote report_data"
        );

        env::log_str(&format!(
            "✅ TEE verification passed: all 5 measurements approved, pubkey={:?}",
            public_key
        ));

        // 4. Add access key to this contract's account (worker account)
        // Permission: Function call to offchainvm_contract_id::resolve_execution and submit_execution_output_and_resolve
        let allowance: Allowance = Allowance::limited(NearToken::from_near(10)).unwrap(); // 10 NEAR for gas
        let method_names = "resolve_execution,submit_execution_output_and_resolve,resume_topup,resume_delete_payment_key".to_string();
        let current_account = env::current_account_id();
        
        env::log_str(&format!(
            "Adding access key to {}: pubkey={:?}, allowance=10 NEAR, methods={}, receiver={}",
            current_account.clone(), public_key, method_names, self.outlayer_contract_id
        ));

        // Add key to this account (self) with permissions for offchainvm_contract_id
        Promise::new(current_account.clone()).add_access_key_allowance(
            public_key,
            allowance,
            self.outlayer_contract_id.clone(),
            method_names,
        )
    }

    /// Verify TDX quote and extract RTMR3 + embedded public key
    ///
    /// Uses dcap-qvl library to:
    /// - Parse TDX quote structure
    /// - Verify Intel's cryptographic signature
    /// - Extract RTMR3 (TEE measurement)
    /// - Extract public key from report_data (first 32 bytes)
    fn verify_worker_registration(
        &self,
        quote_hex: &str,
        collateral_str: &str,
    ) -> (ApprovedMeasurements, PublicKey) {
        use dcap_qvl::verify;
        use hex::decode;

        // Decode hex quote
        let quote_bytes = decode(quote_hex).expect("Invalid quote hex encoding");

        // Parse collateral JSON using MPC Node's Collateral wrapper (audited code)
        // Full structure with 9 fields including CRL
        let collateral_json: serde_json::Value =
            serde_json::from_str(collateral_str).expect("Invalid collateral JSON format");
        let collateral: Collateral = Collateral::try_from_json(collateral_json)
            .expect("Failed to parse collateral");

        // Verify quote with Intel's cryptographic signature
        // Uses dcap_qvl 0.3.2 with "contract" feature (NEAR-compatible, same as MPC Node)
        let now = env::block_timestamp() / 1_000_000_000; // Convert to seconds
        let result = verify::verify(&quote_bytes, collateral.inner(), now)
            .expect("TDX quote verification failed - invalid signature or expired TCB");

        // Extract all measurements from TDX report (MRTD + RTMR0-3)
        let td10 = result
            .report
            .as_td10()
            .expect("Quote is not TDX format (expected TD10)");

        let measurements = ApprovedMeasurements {
            mrtd: hex::encode(td10.mr_td.to_vec()),
            rtmr0: hex::encode(td10.rt_mr0.to_vec()),
            rtmr1: hex::encode(td10.rt_mr1.to_vec()),
            rtmr2: hex::encode(td10.rt_mr2.to_vec()),
            rtmr3: hex::encode(td10.rt_mr3.to_vec()),
        };

        // Extract public key from report_data (first 32 bytes)
        let pubkey_bytes = &td10.report_data[..32]; // ed25519 public key (32 bytes)

        // Convert bytes to NEAR PublicKey
        let pubkey_with_prefix = [&[0u8], pubkey_bytes].concat(); // Add ed25519 prefix
        let public_key = PublicKey::try_from(pubkey_with_prefix)
            .expect("Invalid ed25519 public key in quote report_data");

        (measurements, public_key)
    }

    /// Update cached quote collateral
    ///
    /// Collateral contains Intel's reference data (certificates, TCB info, CRL)
    /// Should be updated when Intel releases new TCB versions (~ monthly)
    ///
    /// Get collateral from: https://api.trustedservices.intel.com/sgx/certification/v4/
    /// or via Phala's dcap-qvl CLI tool
    pub fn update_collateral(&mut self, collateral: String) {
        self.assert_owner();
        self.quote_collateral = Some(collateral);
        env::log_str("Quote collateral updated");
    }

    /// Add approved TEE measurements (MRTD + RTMR0-3).
    ///
    /// All 5 measurements must match for a worker to register.
    /// Get measurements from Phala attestation:
    /// `phala cvms attestation <CVM_NAME> --json | jq '.tcb_info'`
    ///
    /// If `clear_others` is true, removes all existing entries before adding.
    pub fn add_approved_measurements(&mut self, measurements: ApprovedMeasurements, clear_others: Option<bool>) {
        self.assert_owner();
        Self::validate_measurements(&measurements);

        if clear_others.unwrap_or(false) {
            let count = self.approved_measurements.len();
            self.approved_measurements.clear();
            env::log_str(&format!("Cleared {} existing measurement entries", count));
        }

        if !self.approved_measurements.contains(&measurements) {
            self.approved_measurements.push(measurements.clone());
        }

        env::log_str(&format!(
            "Approved measurements added: mrtd={}, rtmr0={}, rtmr1={}, rtmr2={}, rtmr3={}",
            measurements.mrtd, measurements.rtmr0, measurements.rtmr1,
            measurements.rtmr2, measurements.rtmr3
        ));
        env::log_str(&format!("Total approved measurements: {}", self.approved_measurements.len()));
    }

    /// Remove approved measurements
    pub fn remove_approved_measurements(&mut self, measurements: ApprovedMeasurements) {
        self.assert_owner();
        self.approved_measurements.retain(|m| m != &measurements);
        env::log_str(&format!("Approved measurements removed. Remaining: {}", self.approved_measurements.len()));
    }

    /// Clear all approved measurements
    pub fn clear_all_approved_measurements(&mut self) {
        self.assert_owner();
        let count = self.approved_measurements.len();
        self.approved_measurements.clear();
        env::log_str(&format!("Cleared all {} measurement entries", count));
    }

    /// Remove old worker access keys (cleanup + security revocation)
    ///
    /// Call after worker restart when old keys are no longer needed.
    /// Also used to immediately revoke compromised keys.
    ///
    /// Each key is removed via an independent promise so that one
    /// missing key does not cause the entire batch to fail.
    ///
    /// # Security
    /// - Removing a key invalidates any TEE sessions that rely on
    ///   `view_access_key` checks against this contract
    /// - Frees ~0.042 NEAR storage per key
    pub fn remove_worker_keys(&mut self, public_keys: Vec<PublicKey>) {
        self.assert_owner();
        let account = env::current_account_id();
        for key in &public_keys {
            env::log_str(&format!("Removing worker key: {:?}", key));
            // Each delete_key is an independent promise — if one key
            // doesn't exist, the others still get removed.
            Promise::new(account.clone()).delete_key(key.clone());
        }
        env::log_str(&format!("Scheduled removal of {} worker key(s)", public_keys.len()));
    }

    /// Transfer ownership
    pub fn transfer_ownership(&mut self, new_owner: AccountId) {
        self.assert_owner();
        env::log_str(&format!(
            "Ownership transferred: {} -> {}",
            self.owner_id, new_owner
        ));
        self.owner_id = new_owner;
    }

    // ========== View methods ==========

    /// Get list of approved measurements
    pub fn get_approved_measurements(&self) -> Vec<ApprovedMeasurements> {
        self.approved_measurements.clone()
    }

    /// Check if measurements are approved
    pub fn is_measurements_approved(&self, measurements: ApprovedMeasurements) -> bool {
        self.approved_measurements.contains(&measurements)
    }

    /// Get cached collateral (if any)
    pub fn get_collateral(&self) -> Option<String> {
        self.quote_collateral.clone()
    }

    /// Get init worker account
    pub fn get_init_worker_account(&self) -> AccountId {
        self.init_worker_account.clone()
    }

    // ========== Internal ==========

    fn assert_owner(&self) {
        assert_eq!(
            env::predecessor_account_id(),
            self.owner_id,
            "Only owner can call this method"
        );
    }

    fn validate_measurement_field(name: &str, value: &str) {
        assert_eq!(
            value.len(),
            96,
            "Invalid {} format: expected 96 hex characters, got {}",
            name,
            value.len()
        );
        assert!(
            value.chars().all(|c| c.is_ascii_hexdigit()),
            "Invalid {} format: must be hex string",
            name
        );
    }

    fn validate_measurements(m: &ApprovedMeasurements) {
        Self::validate_measurement_field("mrtd", &m.mrtd);
        Self::validate_measurement_field("rtmr0", &m.rtmr0);
        Self::validate_measurement_field("rtmr1", &m.rtmr1);
        Self::validate_measurement_field("rtmr2", &m.rtmr2);
        Self::validate_measurement_field("rtmr3", &m.rtmr3);
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::testing_env;

    fn get_context() -> VMContextBuilder {
        let mut builder = VMContextBuilder::new();
        builder
            .current_account_id(accounts(0))
            .predecessor_account_id(accounts(1))
            .signer_account_id(accounts(1));
        builder
    }

    fn dummy_measurements() -> ApprovedMeasurements {
        ApprovedMeasurements {
            mrtd: "a".repeat(96),
            rtmr0: "b".repeat(96),
            rtmr1: "c".repeat(96),
            rtmr2: "d".repeat(96),
            rtmr3: "e".repeat(96),
        }
    }

    #[test]
    fn test_init() {
        let context = get_context();
        testing_env!(context.build());

        let contract = RegisterContract::new(accounts(1), accounts(2), accounts(3));
        assert_eq!(contract.owner_id, accounts(1));
        assert_eq!(contract.init_worker_account, accounts(2));
        assert_eq!(contract.get_approved_measurements().len(), 0);
    }

    #[test]
    fn test_add_approved_measurements() {
        let context = get_context();
        testing_env!(context.build());

        let mut contract = RegisterContract::new(accounts(1), accounts(2), accounts(3));
        let m = dummy_measurements();
        contract.add_approved_measurements(m.clone(), None);

        assert!(contract.is_measurements_approved(m));
        assert_eq!(contract.get_approved_measurements().len(), 1);
    }

    #[test]
    fn test_add_duplicate_measurements() {
        let context = get_context();
        testing_env!(context.build());

        let mut contract = RegisterContract::new(accounts(1), accounts(2), accounts(3));
        let m = dummy_measurements();
        contract.add_approved_measurements(m.clone(), None);
        contract.add_approved_measurements(m.clone(), None);

        assert_eq!(contract.get_approved_measurements().len(), 1);
    }

    #[test]
    fn test_clear_others() {
        let context = get_context();
        testing_env!(context.build());

        let mut contract = RegisterContract::new(accounts(1), accounts(2), accounts(3));
        let m1 = dummy_measurements();
        contract.add_approved_measurements(m1.clone(), None);

        let m2 = ApprovedMeasurements { rtmr3: "f".repeat(96), ..m1 };
        contract.add_approved_measurements(m2.clone(), Some(true));

        assert_eq!(contract.get_approved_measurements().len(), 1);
        assert!(contract.is_measurements_approved(m2));
    }

    #[test]
    #[should_panic(expected = "Invalid mrtd format")]
    fn test_invalid_measurement_length() {
        let context = get_context();
        testing_env!(context.build());

        let mut contract = RegisterContract::new(accounts(1), accounts(2), accounts(3));
        let m = ApprovedMeasurements {
            mrtd: "abc".to_string(),
            rtmr0: "b".repeat(96),
            rtmr1: "c".repeat(96),
            rtmr2: "d".repeat(96),
            rtmr3: "e".repeat(96),
        };
        contract.add_approved_measurements(m, None);
    }

    #[test]
    #[should_panic(expected = "Only owner")]
    fn test_non_owner_cannot_add_measurements() {
        let mut context = get_context();
        testing_env!(context.build());

        let mut contract = RegisterContract::new(accounts(1), accounts(2), accounts(3));

        // Change predecessor to non-owner
        testing_env!(context.predecessor_account_id(accounts(3)).build());

        contract.add_approved_measurements(dummy_measurements(), None);
    }

    #[test]
    fn test_remove_measurements() {
        let context = get_context();
        testing_env!(context.build());

        let mut contract = RegisterContract::new(accounts(1), accounts(2), accounts(3));
        let m = dummy_measurements();
        contract.add_approved_measurements(m.clone(), None);
        assert_eq!(contract.get_approved_measurements().len(), 1);

        contract.remove_approved_measurements(m);
        assert_eq!(contract.get_approved_measurements().len(), 0);
    }
}
