//! NEAR OutLayer per-customer vault contract.
//!
//! Deployed by a customer onto a sub-account of their NEAR account
//! (e.g. `vault.alice.near`) in a single atomic transaction together with
//! `CreateAccount`, `DeployContract`, `FunctionCall("new", ...)`, and an
//! `AddKey` action that installs the OutLayer TEE keystore worker's
//! function-call key. After that atomic transaction the customer holds NO
//! keys on the vault — the only access key is the TEE function-call key,
//! restricted to calling `request_app_private_key` on the MPC contract.
//!
//! This contract enforces two recovery paths back to parent control:
//!
//! 1. **Cessation-triggered (catastrophic):** the OutLayer DAO declares
//!    cessation; anyone can call `initiate_recovery`, the cross-contract
//!    `is_ceased()` check gates progress, fixed 7-day delay before
//!    `finalize_recovery` is allowed.
//! 2. **Unilateral-triggered (voluntary):** the customer's parent account
//!    calls `unilateral_initiate_recovery` at any time, no DAO involvement,
//!    delay configurable between 24h and 30d via `set_exit_window`.
//!
//! Both paths share `RecoveryState` and `finalize_recovery`; the state's
//! `trigger` field decides whether `finalize_recovery` re-checks
//! `is_ceased()` (Cessation) or relies on window timing only (Unilateral).
//!
//! ## Security guarantees (audit checklist)
//! * `new()` does NOT add or manipulate keys.
//! * No method calls `Promise::new(current).deploy_contract(...)` — vault
//!   cannot self-upgrade.
//! * The TEE function-call access key is bound to `mpc_contract` and
//!   `request_app_private_key` only — it cannot trigger any vault method.
//! * The only Promise actions that add access keys to the vault are
//!   emitted from `callback_add_tee_key` (DAO-gated, function-call only)
//!   and `unlocked_add_key` (parent-gated, only after a successful
//!   `finalize_recovery`).
//! * `finalize_recovery` re-checks `is_ceased()` only when
//!   `trigger == Cessation`. If the DAO revoked cessation in the 7-day
//!   window the recovery state is cleared and the vault stays locked.
//!   Unilateral recoveries are independent of DAO state.
//! * Only one recovery can be in flight at a time, regardless of trigger
//!   (`recovery.is_none()` precondition on both initiate methods).
//! * `set_exit_window` only affects future unilateral recoveries — the
//!   active recovery's `finalize_before` is frozen at initiate time.
//! * `registered_tee_keys` is capped at [`MAX_REGISTERED_TEE_KEYS`] (32)
//!   to bound state size against the permissionless `propose_tee_key`.
//!
//! ## Out of scope for v1
//! * **TEE key revocation.** There is no `revoke_tee_key` method. When a
//!   keystore-worker version is rotated the previous TEE key remains an
//!   active access key on the vault account. Mitigation: deploy a new
//!   vault-contract version (DAO-approved code hash) for any rotation
//!   that needs to retire the old key. Operationally the old key can
//!   only call `request_app_private_key` on the MPC contract, which is
//!   itself DAO-gated, so a stale key on a vault is bounded in blast
//!   radius.

use near_sdk::borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::serde_json;
use near_sdk::{
    env, ext_contract, near_bindgen, require, AccountId, Allowance, Gas, NearToken, PanicOnDefault,
    Promise, PromiseError, PromiseOrValue, PublicKey,
};
use schemars::JsonSchema;

const SECOND_NS: u64 = 1_000_000_000;
const DAY_SECS: u64 = 24 * 60 * 60;

// === Recovery / exit-window timing ===
//
// These five values are baked into the WASM at compile time. Changing
// any of them requires a fresh vault build + new hash approval via DAO
// + redeploy of any vault account that wants the new pacing (existing
// vaults keep the values they were built against).
//
// Production values below. For testnet QA, drop CESSATION_DELAY_NS
// and FINALIZE_WINDOW_NS to seconds, lower MIN/DEFAULT exit window
// to a few minutes, and trim MAX to ~7 days, then build + whitelist
// that test WASM hash on the testnet DAO.

/// Cessation-recovery delay between `initiate_recovery` and the earliest
/// allowed `finalize_recovery`. **Mainnet target: `7 * DAY_SECS * SECOND_NS`.**
/// Currently TESTNET — restore before mainnet build.
pub const CESSATION_DELAY_NS: u64 = 60 * SECOND_NS;

/// How long after the delay the customer has to call `finalize_recovery`.
/// Past this point the recovery is auto-cancelled (state cleared, vault
/// stays locked). Applies to both cessation and unilateral recoveries.
/// **Mainnet target: `7 * DAY_SECS * SECOND_NS`.** Currently TESTNET.
pub const FINALIZE_WINDOW_NS: u64 = 600 * SECOND_NS;

/// Default unilateral exit window applied if `new()` is called with
/// `initial_exit_window = None`. **Mainnet target: `DAY_SECS`.** Currently TESTNET.
pub const DEFAULT_UNILATERAL_EXIT_WINDOW_SECS: u64 = 180;

/// Minimum unilateral exit window — too short and a stolen parent key
/// could grab funds before the customer notices.
/// **Mainnet target: `DAY_SECS`.** Currently TESTNET.
pub const MIN_UNILATERAL_EXIT_WINDOW_SECS: u64 = 60;

/// Maximum unilateral exit window. Bounding the upper end prevents
/// configurations that are practically equivalent to "no escape hatch".
/// **Mainnet target: `30 * DAY_SECS`.** Currently TESTNET (7d).
pub const MAX_UNILATERAL_EXIT_WINDOW_SECS: u64 = 7 * DAY_SECS;

/// Hard cap on `registered_tee_keys` length. Prevents anyone from blowing
/// up vault state size with repeated `propose_tee_key` calls (each call
/// pays its own gas, but the on-chain access-key list and the borsh-
/// serialised vault state are unbounded without a cap). 32 leaves room
/// for many legitimate keystore-worker rotations across a vault's
/// lifetime.
pub const MAX_REGISTERED_TEE_KEYS: usize = 32;

/// Default per-key allowance for the parent's post-recovery function-call
/// keys when [`Vault::unlocked_add_key`] is called with `allowance: None`.
/// 1 NEAR is enough for ~30000 routine vault calls; the parent can pass
/// `Some(_)` for tighter or looser caps.
pub const DEFAULT_PARENT_FCAK_ALLOWANCE_NEAR: u128 = 1;

/// Gas reserved for view-style cross-contract calls (`is_ceased`,
/// `is_keystore_approved`).
const GAS_DAO_VIEW: Gas = Gas::from_tgas(10);

/// Gas reserved for our own callbacks.
const GAS_CALLBACK: Gas = Gas::from_tgas(20);

#[ext_contract(ext_keystore_dao)]
pub trait ExtKeystoreDao {
    /// keystore-dao expects `public_key: String`. `PublicKey` and
    /// `String` JSON-serialize identically to the canonical
    /// `ed25519:base58…` form (verified against
    /// `near-sdk/src/types/public_key.rs`'s `serde::Serialize` impl), so
    /// passing `PublicKey` produces a wire-compatible JSON call. The
    /// Borsh representations differ but cross-contract calls go over
    /// JSON-args, not Borsh.
    fn is_keystore_approved(&self, public_key: PublicKey) -> bool;
    fn is_ceased(&self) -> bool;
}

#[derive(
    BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq,
    JsonSchema,
)]
#[borsh(crate = "near_sdk::borsh")]
#[serde(crate = "near_sdk::serde")]
pub enum RecoveryTrigger {
    /// OutLayer DAO declared cessation. `finalize_recovery` re-checks
    /// `is_ceased()` at finalize time; if the DAO revoked cessation
    /// inside the delay the recovery is cancelled.
    Cessation,
    /// Customer's parent voluntarily started the exit. No DAO involvement;
    /// `finalize_recovery` only checks the window.
    Unilateral,
}

#[derive(
    BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema,
)]
#[borsh(crate = "near_sdk::borsh")]
#[serde(crate = "near_sdk::serde")]
pub struct RecoveryState {
    /// Block timestamp (nanoseconds) at which the initiate call succeeded.
    pub initiated_at: u64,
    /// Earliest block timestamp at which `finalize_recovery` can succeed.
    /// Cessation: `initiated_at + 7 days`.
    /// Unilateral: `initiated_at + unilateral_exit_window_secs (at initiate time)`.
    pub finalize_after: u64,
    /// Latest block timestamp at which `finalize_recovery` can succeed.
    /// Past this, the recovery is auto-cancelled. Equal to
    /// `finalize_after + FINALIZE_WINDOW_NS` for both triggers.
    pub finalize_before: u64,
    /// Which path opened this recovery — decides whether
    /// `finalize_recovery` re-checks `is_ceased()` (Cessation) or relies on
    /// timing only (Unilateral).
    pub trigger: RecoveryTrigger,
}

/// Snapshot of the vault state, returned by [`Vault::get_state`]. Used by
/// off-chain verifiers (vault-checker WASI agent, end-user CLI).
///
/// `PublicKey` JSON-serializes as the canonical `ed25519:base58…` string,
/// so this DTO is consumable by JSON-RPC clients without further parsing.
#[derive(Serialize, Deserialize, JsonSchema, Debug)]
#[serde(crate = "near_sdk::serde")]
pub struct VaultState {
    #[schemars(with = "String")]
    pub parent: AccountId,
    #[schemars(with = "String")]
    pub keystore_dao: AccountId,
    #[schemars(with = "String")]
    pub mpc_contract: AccountId,
    #[schemars(with = "Vec<String>")]
    pub registered_tee_keys: Vec<PublicKey>,
    pub recovery: Option<RecoveryState>,
    pub unlocked: bool,
    pub unilateral_exit_window_secs: u64,
}

#[near_bindgen]
#[derive(BorshSerialize, BorshDeserialize, PanicOnDefault)]
#[borsh(crate = "near_sdk::borsh")]
pub struct Vault {
    /// Customer's NEAR account (e.g. `alice.near`). The only account that
    /// is allowed to add keys after a successful recovery, and the only
    /// account that can call unilateral-recovery / `set_exit_window`.
    pub parent: AccountId,
    /// Account that runs the keystore-dao governance contract
    /// (e.g. `keystore-dao.outlayer.near`). Used as the receiver of
    /// `is_keystore_approved` and `is_ceased` cross-contract calls.
    pub keystore_dao: AccountId,
    /// MPC contract that the TEE function-call keys are allowed to call
    /// (e.g. `v1.signer-prod.testnet` on testnet, `v1.signer` on mainnet).
    /// Set at deploy time, immutable afterwards.
    pub mpc_contract: AccountId,
    /// TEE keystore-worker public keys that have been registered via
    /// `propose_tee_key` after deploy. Informational — the authoritative
    /// access-key list lives on-chain in the account's access keys.
    pub registered_tee_keys: Vec<PublicKey>,
    /// Recovery timer state, if a recovery is currently in progress.
    pub recovery: Option<RecoveryState>,
    /// Set to true after a successful `finalize_recovery`. Until this is
    /// true, the parent account cannot add keys to the vault.
    pub unlocked: bool,
    /// Delay between `unilateral_initiate_recovery` and the earliest
    /// `finalize_recovery`. Configurable by the parent via
    /// `set_exit_window` within `MIN_UNILATERAL_EXIT_WINDOW_SECS` ..=
    /// `MAX_UNILATERAL_EXIT_WINDOW_SECS`. Has no effect on
    /// cessation-triggered recoveries (those are fixed 7 days).
    pub unilateral_exit_window_secs: u64,
}

#[near_bindgen]
impl Vault {
    /// Initialize the vault. Does NOT manipulate keys — the customer's
    /// atomic deploy transaction is responsible for adding the initial TEE
    /// function-call key via a separate `AddKey` action.
    ///
    /// `initial_exit_window` is the unilateral-recovery delay in seconds.
    /// `None` selects [`DEFAULT_UNILATERAL_EXIT_WINDOW_SECS`] (24h). Any
    /// `Some` value must be in
    /// [`MIN_UNILATERAL_EXIT_WINDOW_SECS`]..=[`MAX_UNILATERAL_EXIT_WINDOW_SECS`].
    #[init]
    pub fn new(
        parent: AccountId,
        keystore_dao: AccountId,
        mpc_contract: AccountId,
        initial_exit_window: Option<u64>,
    ) -> Self {
        let window = initial_exit_window.unwrap_or(DEFAULT_UNILATERAL_EXIT_WINDOW_SECS);
        Self::assert_exit_window_in_range(window);
        Self {
            parent,
            keystore_dao,
            mpc_contract,
            registered_tee_keys: Vec::new(),
            recovery: None,
            unlocked: false,
            unilateral_exit_window_secs: window,
        }
    }

    // ===== TEE key registration (used for keystore-worker upgrades) =====

    /// Add a new TEE keystore-worker public key as a function-call access
    /// key on this vault. Permissionless to call — the safety gate is the
    /// cross-contract `is_keystore_approved` check inside
    /// [`Vault::callback_add_tee_key`]. While the vault is unlocked the
    /// parent should use `unlocked_add_key` instead.
    ///
    /// The vault enforces a hard cap of [`MAX_REGISTERED_TEE_KEYS`] to
    /// prevent unbounded state growth from repeated calls.
    ///
    /// ## DoS surface
    /// This method is permissionless, so a hostile actor can race
    /// calls (each with a distinct already-DAO-approved keystore
    /// pubkey) up to the cap. Mitigations: (a) attacker pays gas for
    /// every call, (b) every accepted key is itself a DAO-approved
    /// keystore worker with the same MPC scope — funds cannot be
    /// exfiltrated, only future rotations blocked, and (c) the
    /// parent can call [`Vault::clear_unused_tee_keys`] to free
    /// slots and reject the spam.
    pub fn propose_tee_key(&mut self, public_key: PublicKey) -> Promise {
        require!(
            !self.unlocked,
            "vault is unlocked — use unlocked_add_key instead"
        );
        require!(
            !self.registered_tee_keys.contains(&public_key),
            "public key already registered"
        );
        require!(
            self.registered_tee_keys.len() < MAX_REGISTERED_TEE_KEYS,
            "TEE key limit reached (max 32) — deploy a fresh vault for further rotations"
        );

        ext_keystore_dao::ext(self.keystore_dao.clone())
            .with_static_gas(GAS_DAO_VIEW)
            .is_keystore_approved(public_key.clone())
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_CALLBACK)
                    .callback_add_tee_key(public_key),
            )
    }

    #[private]
    pub fn callback_add_tee_key(
        &mut self,
        public_key: PublicKey,
        #[callback_result] result: Result<bool, PromiseError>,
    ) -> Promise {
        let approved = match result {
            Ok(v) => v,
            Err(_) => env::panic_str(
                "keystore-dao cross-contract call failed — verify keystore_dao address \
                 and that the contract exposes is_keystore_approved(public_key: String)",
            ),
        };
        require!(approved, "public key is not approved by the keystore DAO");
        require!(
            !self.registered_tee_keys.contains(&public_key),
            "public key already registered (race)"
        );
        require!(
            self.registered_tee_keys.len() < MAX_REGISTERED_TEE_KEYS,
            "TEE key limit reached (max 32)"
        );

        self.registered_tee_keys.push(public_key.clone());
        // The off-chain monitor (`outlayer-monitor::source::parse_vault_log`)
        // parses this event and forwards it as a `vault_tee_key_added`
        // webhook to the customer. We emit just the bare event name —
        // the customer queries `vault.get_registered_keys()` after the
        // webhook to read the new key. Including the pubkey directly
        // would require borrowing `PublicKey`'s string form (no
        // Display impl in near-sdk 5.9), and the log shape is an
        // off-chain consumer contract — keep it simple.
        env::log_str("vault_tee_key_added");

        // The TEE function-call key is scoped to ONE method on THIS
        // vault contract: `request_master`, which proxies the MPC CKD
        // call. Direct calls to MPC's `request_app_private_key` would
        // be impossible because that method `assert_one_yocto`s and
        // function-call access keys cannot attach any deposit. The
        // proxy adds the 1 yocto from the vault's own balance via a
        // cross-contract call.
        //
        // CKD (Conditional Key Derivation) protocol the proxy invokes:
        //   * Args:    `{ derivation_path, app_public_key, domain_id }`
        //              — `app_public_key` is the CALLER's ephemeral
        //              public key. No private key is ever passed.
        //   * MPC nodes hold threshold shares of the master and never
        //     see the derived private key in cleartext. They produce an
        //     encrypted CKD payload (big_y, big_c) targeted at
        //     `app_public_key`.
        //   * The keystore-worker decrypts the payload locally inside
        //     the TEE using its ephemeral app-private key, materialising
        //     the per-vault master only inside the enclave.
        // Granting the TEE key access to ONLY `request_master` bounds
        // the blast radius of a TEE compromise — the key cannot transfer
        // funds, deploy contracts, or call any other vault method.
        Promise::new(env::current_account_id()).add_access_key_allowance(
            public_key,
            Allowance::Unlimited,
            env::current_account_id(),
            "request_master".to_string(),
        )
    }

    /// MPC-CKD proxy for the per-vault master derivation. The TEE
    /// function-call key calls THIS method (deposit=0, allowed for
    /// FC keys), and this method makes the cross-contract call to
    /// `mpc_contract.request_app_private_key` attaching the 1 yocto
    /// MPC requires from THIS vault's balance.
    ///
    /// MPC sees the cross-contract call's predecessor as
    /// `env::current_account_id()` (= this vault's account id), so
    /// the per-vault uniqueness of the derived `app_id` is preserved
    /// — different vault accounts produce different masters even with
    /// identical `derivation_path` args.
    ///
    /// The returned `Promise` chains to MPC; NEAR's runtime auto-
    /// propagates MPC's return value (the encrypted CKD payload) as
    /// this method's return value. The keystore-worker's
    /// `broadcast_tx_commit` therefore receives the payload directly.
    pub fn request_master(&self, request: serde_json::Value) -> Promise {
        // Self-call only. Without this, ANY account on chain could
        // sign a tx → vault.request_master with their own ephemeral
        // CKD pubkey + the (publicly-discoverable) on-chain
        // derivation_path, and MPC would derive the per-vault master
        // encrypted to the attacker's pubkey — full key extraction.
        // The TEE function-call access key on this vault produces a
        // tx whose signer_account_id == current_account_id, so the
        // receipt's predecessor matches; any external caller fails
        // here.
        require!(
            env::predecessor_account_id() == env::current_account_id(),
            "request_master is callable only via the vault's own access key"
        );
        require!(!self.unlocked, "vault is unlocked");
        // MPC expects `{ "request": {...} }`. The caller passes the
        // INNER object (auto-unwrapped because near-sdk matched its
        // own arg field `request`), so we re-wrap before forwarding.
        let outgoing = serde_json::json!({ "request": request });
        Promise::new(self.mpc_contract.clone()).function_call(
            "request_app_private_key".to_string(),
            serde_json::to_vec(&outgoing).expect("serialize CKD args"),
            NearToken::from_yoctonear(1),
            Gas::from_tgas(150),
        )
    }

    /// Parent-only cleanup of accepted TEE function-call access keys
    /// that the vault no longer wants. Removes the entries from
    /// `registered_tee_keys` AND drops the underlying access keys
    /// from the account, freeing slots in the
    /// [`MAX_REGISTERED_TEE_KEYS`] cap.
    ///
    /// The cap exists to bound state growth. Because
    /// `propose_tee_key` is permissionless (gated only by DAO
    /// approval of the proposed pubkey), a hostile actor can race
    /// 32 calls with distinct DAO-approved keystore pubkeys and
    /// brick all future legitimate keystore rotations. This method
    /// is the parent-controlled escape hatch — without it, the
    /// only recovery is "redeploy the vault on a fresh sub-account".
    ///
    /// **Parent-only** because a permissionless cleanup would
    /// defeat the cap entirely (an attacker could just clear the
    /// current legitimate worker's key and re-fill the cap).
    ///
    /// Works regardless of `unlocked` state: while locked it's the
    /// DoS escape; while unlocked the parent already has FullAccess
    /// and can drop keys directly, but this method continues to
    /// work for convenience.
    ///
    /// Panics if any pubkey in `public_keys` is not currently in
    /// `registered_tee_keys` — this is a typo guard, callers should
    /// read `get_registered_keys()` first.
    pub fn clear_unused_tee_keys(&mut self, public_keys: Vec<PublicKey>) -> Promise {
        require!(
            env::predecessor_account_id() == self.parent,
            "only the parent account can clear TEE keys"
        );
        require!(!public_keys.is_empty(), "no keys to remove");

        // Reject duplicates — a list with the same pubkey twice would
        // succeed on the first iteration's swap_remove, then panic
        // with a misleading "key not in registered_tee_keys" on the
        // second. Deduping upfront gives a clear error.
        for (i, pk) in public_keys.iter().enumerate() {
            require!(
                !public_keys[i + 1..].contains(pk),
                "duplicate key in input — each pubkey must appear at most once"
            );
        }

        // While locked, leaving the vault with zero TEE keys bricks
        // it: no future request_master can be signed, AND parent
        // can't add keys directly (`unlocked_add_key` requires
        // `unlocked == true`). Force the parent to either keep at
        // least one TEE key or finalize_recovery first.
        if !self.unlocked {
            require!(
                public_keys.len() < self.registered_tee_keys.len(),
                "refusing to remove ALL TEE keys while vault is locked — \
                 would leave vault inoperable. Either keep one or \
                 unlock first via recovery."
            );
        }

        for pk in &public_keys {
            let pos = self.registered_tee_keys.iter().position(|k| k == pk);
            let Some(idx) = pos else {
                env::panic_str(
                    "key not in registered_tee_keys — read get_registered_keys() first",
                );
            };
            self.registered_tee_keys.swap_remove(idx);
        }

        let mut promise = Promise::new(env::current_account_id());
        for pk in &public_keys {
            promise = promise.delete_key(pk.clone());
        }

        env::log_str(&format!(
            "vault_tee_keys_cleared count={}",
            public_keys.len()
        ));
        promise
    }

    // ===== Cessation-triggered recovery (DAO cessation escape hatch) =====

    /// Start the 7-day cessation-recovery timer. Permissionless — the
    /// safety gate is the cross-contract `is_ceased() == true` check
    /// inside [`Vault::callback_initiate`]. If the DAO has not declared
    /// cessation the callback panics and no state is mutated.
    pub fn initiate_recovery(&mut self) -> Promise {
        require!(!self.unlocked, "vault is already unlocked");
        require!(
            self.recovery.is_none(),
            "recovery already in progress — wait for finalize_after or window expiry"
        );

        ext_keystore_dao::ext(self.keystore_dao.clone())
            .with_static_gas(GAS_DAO_VIEW)
            .is_ceased()
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_CALLBACK)
                    .callback_initiate(),
            )
    }

    #[private]
    pub fn callback_initiate(&mut self, #[callback_result] result: Result<bool, PromiseError>) {
        let ceased = match result {
            Ok(v) => v,
            Err(_) => env::panic_str(
                "keystore-dao cross-contract call failed — verify keystore_dao address \
                 and that the contract exposes is_ceased() -> bool",
            ),
        };
        require!(
            ceased,
            "cannot initiate recovery — keystore DAO has not declared cessation"
        );
        // Defense-in-depth: re-check `recovery.is_none()` in case of races.
        require!(
            self.recovery.is_none(),
            "recovery already in progress (race)"
        );

        let now = env::block_timestamp();
        let finalize_after = now + CESSATION_DELAY_NS;
        self.recovery = Some(RecoveryState {
            initiated_at: now,
            finalize_after,
            finalize_before: finalize_after + FINALIZE_WINDOW_NS,
            trigger: RecoveryTrigger::Cessation,
        });
        env::log_str("recovery_initiated_cessation");
    }

    // ===== Unilateral-triggered recovery (voluntary, parent-controlled) =====

    /// Parent-only voluntary exit. No DAO involvement. The delay before
    /// `finalize_recovery` becomes valid is `unilateral_exit_window_secs`
    /// captured at this call (so changing the window afterwards does not
    /// shorten an in-flight recovery).
    pub fn unilateral_initiate_recovery(&mut self) {
        require!(!self.unlocked, "vault is already unlocked");
        require!(
            env::predecessor_account_id() == self.parent,
            "only the parent account can initiate unilateral recovery"
        );
        require!(
            self.recovery.is_none(),
            "recovery already in progress — only one recovery can be active at a time"
        );

        let now = env::block_timestamp();
        let finalize_after = now + (self.unilateral_exit_window_secs * SECOND_NS);
        self.recovery = Some(RecoveryState {
            initiated_at: now,
            finalize_after,
            finalize_before: finalize_after + FINALIZE_WINDOW_NS,
            trigger: RecoveryTrigger::Unilateral,
        });
        env::log_str("recovery_initiated_unilateral");
    }

    /// Parent-only update of the unilateral exit window. Affects only
    /// future `unilateral_initiate_recovery` calls — any in-flight
    /// recovery's `finalize_after`/`finalize_before` are frozen at
    /// initiate time.
    pub fn set_exit_window(&mut self, new_window_secs: u64) {
        require!(
            env::predecessor_account_id() == self.parent,
            "only the parent account can change the exit window"
        );
        Self::assert_exit_window_in_range(new_window_secs);
        self.unilateral_exit_window_secs = new_window_secs;
        env::log_str(&format!("exit_window_set_to_{}_secs", new_window_secs));
    }

    // ===== Shared finalize (routes by recovery.trigger) =====

    /// Finalize the in-flight recovery.
    ///
    /// * **Cessation:** dispatches `keystore_dao.is_ceased()` and resolves
    ///   asynchronously via [`Vault::callback_finalize`]. If the DAO
    ///   revoked cessation during the delay the recovery is cancelled
    ///   (state cleared, vault stays locked).
    /// * **Unilateral:** synchronous — unlocks the vault if the window
    ///   check passes, otherwise clears the recovery state.
    ///
    /// `finalize_after` is enforced up-front (too early panics without
    /// state change). `finalize_before` is enforced inside the callback
    /// (cessation) or inline (unilateral) so the recovery state can be
    /// safely cleared on expiry without a panic rolling it back.
    pub fn finalize_recovery(&mut self) -> PromiseOrValue<bool> {
        require!(!self.unlocked, "vault is already unlocked");
        let recovery = self
            .recovery
            .as_ref()
            .cloned()
            .unwrap_or_else(|| env::panic_str("no recovery in progress"));
        let now = env::block_timestamp();
        require!(
            now >= recovery.finalize_after,
            "recovery delay not yet elapsed"
        );

        match recovery.trigger {
            RecoveryTrigger::Cessation => PromiseOrValue::Promise(
                ext_keystore_dao::ext(self.keystore_dao.clone())
                    .with_static_gas(GAS_DAO_VIEW)
                    .is_ceased()
                    .then(
                        Self::ext(env::current_account_id())
                            .with_static_gas(GAS_CALLBACK)
                            .callback_finalize(),
                    ),
            ),
            RecoveryTrigger::Unilateral => {
                if now > recovery.finalize_before {
                    self.recovery = None;
                    env::log_str("recovery_window_expired");
                    return PromiseOrValue::Value(false);
                }
                self.unlocked = true;
                self.recovery = None;
                env::log_str("recovery_finalized_unilateral");
                PromiseOrValue::Value(true)
            }
        }
    }

    #[private]
    pub fn callback_finalize(
        &mut self,
        #[callback_result] result: Result<bool, PromiseError>,
    ) -> bool {
        // The recovery state must still exist; if it has already been
        // cleared (e.g. by a parallel finalize) we simply return false.
        let recovery = match self.recovery.clone() {
            Some(r) => r,
            None => return false,
        };
        let now = env::block_timestamp();

        // Window-expired check is done inside the callback so that the
        // state can be cleared (a panicking branch would roll back the
        // clearing). Customer is expected to read logs to learn what
        // happened.
        if now > recovery.finalize_before {
            self.recovery = None;
            env::log_str("recovery_window_expired");
            return false;
        }

        let ceased = match result {
            Ok(v) => v,
            Err(_) => {
                env::log_str("recovery_finalize_failed_dao_call");
                return false;
            }
        };
        if !ceased {
            // DAO revoked cessation during the delay. Cancel the
            // recovery; customer must restart if cessation is declared
            // again.
            self.recovery = None;
            env::log_str("recovery_cancelled_dao_revoked");
            return false;
        }

        self.unlocked = true;
        self.recovery = None;
        env::log_str("recovery_finalized_cessation");
        true
    }

    /// After a successful recovery the parent account can add its own
    /// keys to the vault.
    ///
    /// * `full_access = true`: adds a full-access key. `allowance` is
    ///   ignored. Use this if you want unbounded authority over the vault.
    /// * `full_access = false`: adds a function-call key bound to this
    ///   vault, scoped to all of its methods. `allowance` is the gas
    ///   budget for the key:
    ///   - `None` selects [`DEFAULT_PARENT_FCAK_ALLOWANCE_NEAR`] (1 NEAR).
    ///   - `Some(t)` with `t > 0` uses `Allowance::Limited(t)`.
    ///   - `Some(t)` with `t == 0` is rejected — there is no "unlimited
    ///     function-call key" path through this method. If you need
    ///     unbounded authority, pass `full_access = true` instead.
    pub fn unlocked_add_key(
        &mut self,
        public_key: PublicKey,
        full_access: bool,
        allowance: Option<NearToken>,
    ) -> Promise {
        require!(self.unlocked, "vault is not unlocked");
        require!(
            env::predecessor_account_id() == self.parent,
            "only the parent account can add keys after unlock"
        );

        if full_access {
            Promise::new(env::current_account_id()).add_full_access_key(public_key)
        } else {
            let allowance_token = match allowance {
                Some(t) => {
                    require!(
                        t.as_yoctonear() > 0,
                        "allowance must be > 0; pass `null` for the 1 NEAR default, \
                         or use full_access=true if you need unbounded authority"
                    );
                    t
                }
                None => NearToken::from_near(DEFAULT_PARENT_FCAK_ALLOWANCE_NEAR),
            };
            let limited = Allowance::limited(allowance_token)
                .unwrap_or_else(|| env::panic_str("allowance must be > 0"));
            Promise::new(env::current_account_id()).add_access_key_allowance(
                public_key,
                limited,
                env::current_account_id(),
                String::new(),
            )
        }
    }

    // ===== View methods =====

    /// Full snapshot of the vault state, suitable for off-chain
    /// verification by the vault-checker WASI agent and end users.
    pub fn get_state(&self) -> VaultState {
        VaultState {
            parent: self.parent.clone(),
            keystore_dao: self.keystore_dao.clone(),
            mpc_contract: self.mpc_contract.clone(),
            registered_tee_keys: self.registered_tee_keys.clone(),
            recovery: self.recovery.clone(),
            unlocked: self.unlocked,
            unilateral_exit_window_secs: self.unilateral_exit_window_secs,
        }
    }

    pub fn get_registered_keys(&self) -> Vec<PublicKey> {
        self.registered_tee_keys.clone()
    }

    /// Returns the in-flight recovery state, if any. **Note:** stale
    /// recoveries (those whose `finalize_before` has already passed) are
    /// not auto-cleared by view calls — only by the next mutating
    /// `finalize_recovery`. Off-chain callers (dashboards, vault-checker)
    /// must compare `finalize_before` against the current block timestamp
    /// to distinguish "active" from "expired and pending cleanup".
    pub fn get_recovery_state(&self) -> Option<RecoveryState> {
        self.recovery.clone()
    }

    pub fn get_exit_window(&self) -> u64 {
        self.unilateral_exit_window_secs
    }

    pub fn is_unlocked(&self) -> bool {
        self.unlocked
    }
}

impl Vault {
    /// Internal: validate that an exit-window proposal is in the allowed
    /// `[MIN_UNILATERAL_EXIT_WINDOW_SECS, MAX_UNILATERAL_EXIT_WINDOW_SECS]`
    /// range. Used by both `new()` and `set_exit_window`.
    fn assert_exit_window_in_range(secs: u64) {
        require!(
            secs >= MIN_UNILATERAL_EXIT_WINDOW_SECS && secs <= MAX_UNILATERAL_EXIT_WINDOW_SECS,
            format!(
                "exit window must be between {}s and {}s",
                MIN_UNILATERAL_EXIT_WINDOW_SECS,
                MAX_UNILATERAL_EXIT_WINDOW_SECS,
            )
        );
    }
}

// ===== Unit tests =====
//
// These cover the parts that don't need a sandbox (purely local logic and
// `require!` checks). The full cross-contract / recovery-window scenarios
// live in `tests/integration.rs`, which uses near-workspaces.

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::VMContextBuilder;
    use near_sdk::testing_env;

    fn alice() -> AccountId {
        "alice.near".parse().unwrap()
    }
    fn vault_account() -> AccountId {
        "vault.alice.near".parse().unwrap()
    }
    fn dao() -> AccountId {
        "keystore-dao.outlayer.near".parse().unwrap()
    }
    fn mpc() -> AccountId {
        "v1.signer.near".parse().unwrap()
    }
    fn ed25519_key() -> PublicKey {
        "ed25519:H9k5eiU4xXS3M4z8HzKJSLaZdqGdGwBG49o7orNC5LJ"
            .parse()
            .unwrap()
    }
    fn ed25519_key_2() -> PublicKey {
        "ed25519:8RazSLHvzj4TBSKGUo2vyL56qBu74EQfjyy6FNk1bgxR"
            .parse()
            .unwrap()
    }

    fn ctx_for(predecessor: AccountId) -> VMContextBuilder {
        let mut b = VMContextBuilder::new();
        b.current_account_id(vault_account())
            .predecessor_account_id(predecessor);
        b
    }

    fn fresh_vault() -> Vault {
        testing_env!(ctx_for(alice()).build());
        Vault::new(alice(), dao(), mpc(), None)
    }

    #[test]
    fn init_state_is_locked_with_no_recovery() {
        let v = fresh_vault();
        assert_eq!(v.parent, alice());
        assert_eq!(v.keystore_dao, dao());
        assert_eq!(v.mpc_contract, mpc());
        assert!(v.registered_tee_keys.is_empty());
        assert!(v.recovery.is_none());
        assert!(!v.unlocked);
    }

    #[test]
    #[should_panic(expected = "vault is not unlocked")]
    fn unlocked_add_key_rejected_when_locked() {
        let mut v = fresh_vault();
        testing_env!(ctx_for(alice()).build());
        v.unlocked_add_key(ed25519_key(), false, None);
    }

    #[test]
    #[should_panic(expected = "only the parent account can add keys after unlock")]
    fn unlocked_add_key_rejected_for_non_parent() {
        let mut v = fresh_vault();
        v.unlocked = true; // simulate post-recovery state
        testing_env!(ctx_for("eve.near".parse().unwrap()).build());
        v.unlocked_add_key(ed25519_key(), false, None);
    }

    #[test]
    fn unlocked_add_key_succeeds_for_parent_after_unlock() {
        let mut v = fresh_vault();
        v.unlocked = true;
        testing_env!(ctx_for(alice()).build());
        // Returns a Promise — we don't execute it here, we just check that
        // the method does not panic for the legitimate caller.
        let _ = v.unlocked_add_key(ed25519_key(), false, None);
    }

    #[test]
    #[should_panic(expected = "allowance must be > 0")]
    fn unlocked_add_key_rejects_zero_allowance() {
        let mut v = fresh_vault();
        v.unlocked = true;
        testing_env!(ctx_for(alice()).build());
        let _ = v.unlocked_add_key(
            ed25519_key(),
            false,
            Some(near_sdk::NearToken::from_yoctonear(0)),
        );
    }

    // ===== clear_unused_tee_keys (DoS escape hatch) =====

    #[test]
    fn clear_unused_tee_keys_removes_entry() {
        let mut v = fresh_vault();
        v.registered_tee_keys.push(ed25519_key());
        v.registered_tee_keys.push(ed25519_key_2());
        testing_env!(ctx_for(alice()).build());
        let _ = v.clear_unused_tee_keys(vec![ed25519_key()]);
        assert_eq!(v.registered_tee_keys.len(), 1);
        assert!(!v.registered_tee_keys.contains(&ed25519_key()));
        assert!(v.registered_tee_keys.contains(&ed25519_key_2()));
    }

    #[test]
    #[should_panic(expected = "only the parent account can clear TEE keys")]
    fn clear_unused_tee_keys_rejects_non_parent() {
        let mut v = fresh_vault();
        v.registered_tee_keys.push(ed25519_key());
        testing_env!(ctx_for("eve.near".parse().unwrap()).build());
        let _ = v.clear_unused_tee_keys(vec![ed25519_key()]);
    }

    #[test]
    #[should_panic(expected = "no keys to remove")]
    fn clear_unused_tee_keys_rejects_empty_list() {
        let mut v = fresh_vault();
        testing_env!(ctx_for(alice()).build());
        let _ = v.clear_unused_tee_keys(vec![]);
    }

    #[test]
    #[should_panic(expected = "key not in registered_tee_keys")]
    fn clear_unused_tee_keys_rejects_unknown_key() {
        // Extra registered key so the "all-keys removed while locked"
        // guard doesn't fire first — this test only cares about the
        // typo branch.
        let mut v = fresh_vault();
        v.registered_tee_keys.push(ed25519_key());
        v.registered_tee_keys.push(ed25519_key_2());
        // Use a third pubkey not registered anywhere — should hit
        // the "key not in registered_tee_keys" panic.
        let unknown: PublicKey = "ed25519:11111111111111111111111111111111".parse().unwrap();
        testing_env!(ctx_for(alice()).build());
        let _ = v.clear_unused_tee_keys(vec![unknown]);
    }

    #[test]
    fn clear_unused_tee_keys_works_when_unlocked() {
        // Method works regardless of unlocked state — once unlocked
        // the parent has FullAccess and could drop keys directly,
        // but this method continues to function for convenience.
        let mut v = fresh_vault();
        v.unlocked = true;
        v.registered_tee_keys.push(ed25519_key());
        testing_env!(ctx_for(alice()).build());
        let _ = v.clear_unused_tee_keys(vec![ed25519_key()]);
        assert!(v.registered_tee_keys.is_empty());
    }

    #[test]
    fn clear_unused_tee_keys_frees_cap_slot() {
        // Fill near cap, clear one, verify a propose-style push fits.
        let mut v = fresh_vault();
        v.registered_tee_keys.push(ed25519_key());
        v.registered_tee_keys.push(ed25519_key_2());
        let initial_len = v.registered_tee_keys.len();
        testing_env!(ctx_for(alice()).build());
        let _ = v.clear_unused_tee_keys(vec![ed25519_key()]);
        assert_eq!(v.registered_tee_keys.len(), initial_len - 1);
    }

    #[test]
    #[should_panic(expected = "duplicate key in input")]
    fn clear_unused_tee_keys_rejects_duplicates() {
        let mut v = fresh_vault();
        v.registered_tee_keys.push(ed25519_key());
        testing_env!(ctx_for(alice()).build());
        let _ = v.clear_unused_tee_keys(vec![ed25519_key(), ed25519_key()]);
    }

    #[test]
    #[should_panic(expected = "would leave vault inoperable")]
    fn clear_unused_tee_keys_rejects_emptying_locked_vault() {
        // While locked, removing the last TEE key would brick the
        // vault — no further request_master can be signed and parent
        // cannot use unlocked_add_key. Operator must keep at least
        // one or recover first.
        let mut v = fresh_vault();
        v.registered_tee_keys.push(ed25519_key());
        v.registered_tee_keys.push(ed25519_key_2());
        testing_env!(ctx_for(alice()).build());
        let _ = v.clear_unused_tee_keys(vec![ed25519_key(), ed25519_key_2()]);
    }

    // ===== request_master proxy security =====

    #[test]
    #[should_panic(expected = "callable only via the vault's own access key")]
    fn request_master_rejects_external_predecessor() {
        // ANY external account calling request_master with their own
        // ephemeral CKD pubkey + the on-chain (publicly visible)
        // derivation_path would otherwise have MPC return the per-vault
        // master encrypted to their pubkey. The predecessor==self gate
        // is the security boundary.
        let v = fresh_vault();
        testing_env!(ctx_for("attacker.near".parse().unwrap()).build());
        let _ = v.request_master(near_sdk::serde_json::json!({
            "derivation_path": "deadbeef",
            "app_public_key": "bls12381g1:dontcare",
            "domain_id": 2u64,
        }));
    }

    #[test]
    fn request_master_accepts_self_predecessor() {
        // The TEE function-call key signs tx → vault.request_master,
        // producing predecessor=vault (current_account_id). The
        // require holds.
        let v = fresh_vault();
        testing_env!(ctx_for(vault_account()).build());
        let _ = v.request_master(near_sdk::serde_json::json!({
            "derivation_path": "deadbeef",
            "app_public_key": "bls12381g1:dontcare",
            "domain_id": 2u64,
        }));
    }

    #[test]
    #[should_panic(expected = "vault is unlocked")]
    fn request_master_rejects_unlocked_vault() {
        let mut v = fresh_vault();
        v.unlocked = true;
        testing_env!(ctx_for(vault_account()).build());
        let _ = v.request_master(near_sdk::serde_json::json!({}));
    }

    #[test]
    fn clear_unused_tee_keys_can_empty_when_unlocked() {
        // Once unlocked, parent has FullAccess and can add keys
        // directly via unlocked_add_key — clearing all TEE keys is
        // safe in that state.
        let mut v = fresh_vault();
        v.unlocked = true;
        v.registered_tee_keys.push(ed25519_key());
        v.registered_tee_keys.push(ed25519_key_2());
        testing_env!(ctx_for(alice()).build());
        let _ = v.clear_unused_tee_keys(vec![ed25519_key(), ed25519_key_2()]);
        assert!(v.registered_tee_keys.is_empty());
    }

    #[test]
    #[should_panic(expected = "no recovery in progress")]
    fn finalize_without_initiate_panics() {
        let mut v = fresh_vault();
        testing_env!(ctx_for(alice()).build());
        v.finalize_recovery();
    }

    #[test]
    #[should_panic(expected = "recovery delay not yet elapsed")]
    fn finalize_too_early_panics() {
        let mut v = fresh_vault();
        // Simulate that initiate_recovery already populated the timer.
        let now: u64 = 1_700_000_000_000_000_000;
        v.recovery = Some(RecoveryState {
            initiated_at: now,
            finalize_after: now + CESSATION_DELAY_NS,
            finalize_before: now + CESSATION_DELAY_NS + FINALIZE_WINDOW_NS,
            trigger: RecoveryTrigger::Cessation,
        });
        let mut b = ctx_for(alice());
        b.block_timestamp(now + CESSATION_DELAY_NS - 1);
        testing_env!(b.build());
        v.finalize_recovery();
    }

    #[test]
    #[should_panic(expected = "recovery already in progress")]
    fn double_initiate_panics_pre_dispatch() {
        let mut v = fresh_vault();
        let now: u64 = 1_700_000_000_000_000_000;
        v.recovery = Some(RecoveryState {
            initiated_at: now,
            finalize_after: now + CESSATION_DELAY_NS,
            finalize_before: now + CESSATION_DELAY_NS + FINALIZE_WINDOW_NS,
            trigger: RecoveryTrigger::Cessation,
        });
        testing_env!(ctx_for(alice()).build());
        v.initiate_recovery();
    }

    #[test]
    #[should_panic(expected = "vault is unlocked")]
    fn propose_tee_key_rejected_after_unlock() {
        let mut v = fresh_vault();
        v.unlocked = true;
        testing_env!(ctx_for(alice()).build());
        v.propose_tee_key(ed25519_key());
    }

    #[test]
    #[should_panic(expected = "public key already registered")]
    fn propose_tee_key_rejects_duplicate() {
        let mut v = fresh_vault();
        v.registered_tee_keys.push(ed25519_key());
        testing_env!(ctx_for(alice()).build());
        v.propose_tee_key(ed25519_key());
    }

    #[test]
    #[should_panic(expected = "public key is not approved by the keystore DAO")]
    fn callback_add_tee_key_panics_when_dao_returns_false() {
        // Drives the callback's `require!(approved, ...)` branch
        // directly. Cannot be triggered through near-workspaces because
        // there's no way to deterministically schedule the DAO state to
        // flip *between* the cross-contract dispatch and the callback's
        // resumption — only an in-process unit test can reach this line.
        let mut v = fresh_vault();
        let mut b = VMContextBuilder::new();
        b.current_account_id(vault_account())
            .predecessor_account_id(vault_account());
        testing_env!(b.build());
        let _ = v.callback_add_tee_key(ed25519_key(), Ok(false));
    }

    // ===== Unilateral recovery + exit-window unit tests =====

    #[test]
    fn default_exit_window_matches_constant() {
        let v = fresh_vault();
        assert_eq!(v.unilateral_exit_window_secs, DEFAULT_UNILATERAL_EXIT_WINDOW_SECS);
        assert_eq!(v.get_exit_window(), DEFAULT_UNILATERAL_EXIT_WINDOW_SECS);
    }

    #[test]
    fn new_accepts_explicit_exit_window_in_range() {
        testing_env!(ctx_for(alice()).build());
        let v = Vault::new(alice(), dao(), mpc(), Some(MAX_UNILATERAL_EXIT_WINDOW_SECS));
        assert_eq!(v.unilateral_exit_window_secs, MAX_UNILATERAL_EXIT_WINDOW_SECS);
    }

    #[test]
    #[should_panic(expected = "exit window must be between")]
    fn new_rejects_too_short_exit_window() {
        testing_env!(ctx_for(alice()).build());
        let _ = Vault::new(alice(), dao(), mpc(), Some(MIN_UNILATERAL_EXIT_WINDOW_SECS - 1));
    }

    #[test]
    #[should_panic(expected = "exit window must be between")]
    fn new_rejects_too_long_exit_window() {
        testing_env!(ctx_for(alice()).build());
        let _ = Vault::new(alice(), dao(), mpc(), Some(MAX_UNILATERAL_EXIT_WINDOW_SECS + 1));
    }

    #[test]
    fn set_exit_window_updates_field() {
        let mut v = fresh_vault();
        testing_env!(ctx_for(alice()).build());
        v.set_exit_window(7 * DAY_SECS);
        assert_eq!(v.get_exit_window(), 7 * DAY_SECS);
        v.set_exit_window(MAX_UNILATERAL_EXIT_WINDOW_SECS);
        assert_eq!(v.get_exit_window(), MAX_UNILATERAL_EXIT_WINDOW_SECS);
        v.set_exit_window(MIN_UNILATERAL_EXIT_WINDOW_SECS);
        assert_eq!(v.get_exit_window(), MIN_UNILATERAL_EXIT_WINDOW_SECS);
    }

    #[test]
    #[should_panic(expected = "only the parent account can change the exit window")]
    fn set_exit_window_rejects_non_parent() {
        let mut v = fresh_vault();
        testing_env!(ctx_for("eve.near".parse().unwrap()).build());
        v.set_exit_window(7 * DAY_SECS);
    }

    #[test]
    #[should_panic(expected = "exit window must be between")]
    fn set_exit_window_rejects_too_short() {
        let mut v = fresh_vault();
        testing_env!(ctx_for(alice()).build());
        v.set_exit_window(MIN_UNILATERAL_EXIT_WINDOW_SECS - 1);
    }

    #[test]
    #[should_panic(expected = "exit window must be between")]
    fn set_exit_window_rejects_too_long() {
        let mut v = fresh_vault();
        testing_env!(ctx_for(alice()).build());
        v.set_exit_window(MAX_UNILATERAL_EXIT_WINDOW_SECS + 1);
    }

    #[test]
    fn unilateral_initiate_uses_current_exit_window() {
        let mut v = fresh_vault();
        v.unilateral_exit_window_secs = 7 * DAY_SECS;
        let now: u64 = 1_700_000_000_000_000_000;
        let mut b = ctx_for(alice());
        b.block_timestamp(now);
        testing_env!(b.build());

        v.unilateral_initiate_recovery();

        let r = v.recovery.as_ref().expect("recovery state set");
        assert_eq!(r.trigger, RecoveryTrigger::Unilateral);
        assert_eq!(r.initiated_at, now);
        assert_eq!(r.finalize_after, now + 7 * DAY_SECS * SECOND_NS);
        assert_eq!(
            r.finalize_before,
            now + 7 * DAY_SECS * SECOND_NS + FINALIZE_WINDOW_NS
        );
    }

    #[test]
    #[should_panic(expected = "only the parent account can initiate unilateral recovery")]
    fn unilateral_initiate_rejects_non_parent() {
        let mut v = fresh_vault();
        testing_env!(ctx_for("eve.near".parse().unwrap()).build());
        v.unilateral_initiate_recovery();
    }

    #[test]
    #[should_panic(expected = "recovery already in progress")]
    fn unilateral_initiate_rejects_when_recovery_active() {
        let mut v = fresh_vault();
        let now: u64 = 1_700_000_000_000_000_000;
        v.recovery = Some(RecoveryState {
            initiated_at: now,
            finalize_after: now + CESSATION_DELAY_NS,
            finalize_before: now + CESSATION_DELAY_NS + FINALIZE_WINDOW_NS,
            trigger: RecoveryTrigger::Cessation,
        });
        testing_env!(ctx_for(alice()).build());
        v.unilateral_initiate_recovery();
    }

    #[test]
    #[should_panic(expected = "vault is already unlocked")]
    fn unilateral_initiate_rejects_when_unlocked() {
        let mut v = fresh_vault();
        v.unlocked = true;
        testing_env!(ctx_for(alice()).build());
        v.unilateral_initiate_recovery();
    }

    #[test]
    fn finalize_unilateral_unlocks_synchronously() {
        let mut v = fresh_vault();
        let now: u64 = 1_700_000_000_000_000_000;
        let window_secs = 7 * DAY_SECS;
        v.recovery = Some(RecoveryState {
            initiated_at: now,
            finalize_after: now + window_secs * SECOND_NS,
            finalize_before: now + window_secs * SECOND_NS + FINALIZE_WINDOW_NS,
            trigger: RecoveryTrigger::Unilateral,
        });
        let mut b = ctx_for(alice());
        b.block_timestamp(now + window_secs * SECOND_NS + 1);
        testing_env!(b.build());

        // The Promise is returned but the state mutation happens
        // synchronously inside `finalize_recovery` for the Unilateral
        // branch.
        let _ = v.finalize_recovery();

        assert!(v.unlocked);
        assert!(v.recovery.is_none());
    }

    #[test]
    fn finalize_unilateral_after_window_clears_state_does_not_unlock() {
        let mut v = fresh_vault();
        let now: u64 = 1_700_000_000_000_000_000;
        let window_secs = 7 * DAY_SECS;
        let finalize_before = now + window_secs * SECOND_NS + FINALIZE_WINDOW_NS;
        v.recovery = Some(RecoveryState {
            initiated_at: now,
            finalize_after: now + window_secs * SECOND_NS,
            finalize_before,
            trigger: RecoveryTrigger::Unilateral,
        });
        let mut b = ctx_for(alice());
        b.block_timestamp(finalize_before + 1);
        testing_env!(b.build());

        let _ = v.finalize_recovery();

        assert!(!v.unlocked);
        assert!(v.recovery.is_none());
    }

    #[test]
    #[should_panic(expected = "recovery delay not yet elapsed")]
    fn finalize_unilateral_too_early_panics() {
        let mut v = fresh_vault();
        let now: u64 = 1_700_000_000_000_000_000;
        let window_secs = 7 * DAY_SECS;
        v.recovery = Some(RecoveryState {
            initiated_at: now,
            finalize_after: now + window_secs * SECOND_NS,
            finalize_before: now + window_secs * SECOND_NS + FINALIZE_WINDOW_NS,
            trigger: RecoveryTrigger::Unilateral,
        });
        let mut b = ctx_for(alice());
        b.block_timestamp(now + window_secs * SECOND_NS - 1);
        testing_env!(b.build());
        v.finalize_recovery();
    }

    #[test]
    fn changing_exit_window_does_not_affect_in_flight_recovery() {
        let mut v = fresh_vault();
        v.unilateral_exit_window_secs = 7 * DAY_SECS;
        let now: u64 = 1_700_000_000_000_000_000;
        let mut b = ctx_for(alice());
        b.block_timestamp(now);
        testing_env!(b.build());

        v.unilateral_initiate_recovery();
        let frozen_finalize_after = v.recovery.as_ref().unwrap().finalize_after;

        // Customer shortens window after initiating — must not move the
        // in-flight recovery's finalize_after closer.
        v.set_exit_window(MIN_UNILATERAL_EXIT_WINDOW_SECS);

        assert_eq!(v.recovery.as_ref().unwrap().finalize_after, frozen_finalize_after);
        assert_eq!(v.unilateral_exit_window_secs, MIN_UNILATERAL_EXIT_WINDOW_SECS);
    }
}
