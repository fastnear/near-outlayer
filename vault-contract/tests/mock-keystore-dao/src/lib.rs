//! Minimal mock of `keystore-dao-contract` for vault-contract integration
//! tests. Only exposes the surface that vault-contract actually
//! cross-contracts to:
//!
//! * `is_keystore_approved(public_key: String) -> bool`
//! * `is_ceased() -> bool`
//!
//! Both backed by mutable in-memory state with simple setters so tests
//! can flip the gates between scenarios. Keeps the vault-contract test
//! suite self-contained — no need to deploy the real keystore-dao
//! contract just to exercise vault flows.

use near_sdk::borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::collections::UnorderedSet;
use near_sdk::{env, near_bindgen, BorshStorageKey, PanicOnDefault, PublicKey};

#[derive(BorshSerialize, BorshStorageKey)]
#[borsh(crate = "near_sdk::borsh")]
enum StorageKey {
    ApprovedKeystores,
}

#[near_bindgen]
#[derive(BorshSerialize, BorshDeserialize, PanicOnDefault)]
#[borsh(crate = "near_sdk::borsh")]
pub struct MockKeystoreDao {
    pub ceased: bool,
    pub approved_keystores: UnorderedSet<PublicKey>,
}

#[near_bindgen]
impl MockKeystoreDao {
    #[init]
    pub fn new() -> Self {
        Self {
            ceased: false,
            approved_keystores: UnorderedSet::new(StorageKey::ApprovedKeystores),
        }
    }

    // ===== Vault-facing API (matches the real keystore-dao surface) =====

    /// Mirrors the real `keystore-dao-contract::is_keystore_approved`
    /// signature exactly: argument is `String`, parsed into a
    /// `PublicKey` inside. Lets the vault-contract integration tests
    /// exercise the same wire shape that production deploys hit.
    pub fn is_keystore_approved(&self, public_key: String) -> bool {
        let Ok(parsed) = public_key.parse::<PublicKey>() else {
            return false;
        };
        self.approved_keystores.contains(&parsed)
    }

    pub fn is_ceased(&self) -> bool {
        self.ceased
    }

    // ===== Test-only setters =====
    //
    // MOCK-ONLY: The real `keystore-dao-contract` does NOT expose
    // direct setters for cessation or approved-keystores state. In
    // production those flips happen through DAO governance (vote-driven
    // proposals). These setters exist purely so vault integration tests
    // can flip the gates between scenarios without standing up a full
    // DAO governance flow. Future maintainers: do NOT bake a
    // `revoke_keystore` call directly into vault contract tests as if
    // it represents real DAO behaviour.

    pub fn set_ceased(&mut self, value: bool) {
        self.ceased = value;
        env::log_str(&format!("mock_set_ceased {}", value));
    }

    pub fn approve_keystore(&mut self, public_key: PublicKey) {
        self.approved_keystores.insert(&public_key);
        env::log_str("mock_approve_keystore");
    }

    pub fn revoke_keystore(&mut self, public_key: PublicKey) {
        self.approved_keystores.remove(&public_key);
        env::log_str("mock_revoke_keystore");
    }
}
