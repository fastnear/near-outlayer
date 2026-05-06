//! Defense-in-depth re-verification of vault state, called from
//! `/sign-vault-verification` before the worker submits
//! `mark_vault_verified` on chain.
//!
//! **Why this duplicates `wasi-examples/vault-checker/src/verify.rs`**:
//! the vault-checker WASI agent runs the same checks first and is the
//! agent the customer pays to verify. But `mark_vault_verified` lands
//! on chain via the keystore-worker's approved access key — so a bug
//! in vault-checker that let a malicious vault through would let the
//! worker mark it verified, locking in the lie. Defense-in-depth:
//! the worker re-runs the SAME checks before signing the tx.
//!
//! These two implementations are intentionally INDEPENDENT (not a
//! shared crate). Different parsing approaches (typed NEAR primitives
//! here vs raw JSON in vault-checker) reduce the chance of a common
//! bug hiding in shared code. The trade-off — ~150 lines of duplicated
//! validation — is acceptable for custody-grade defense in depth.
//!
//! Checks (in order, short-circuit on first failure):
//!
//! 1. `keystore_dao.is_vault_banned(vault_id)` → fast deny.
//! 2. `view_account(vault_id)` → extract `code_hash`.
//! 3. `keystore_dao.is_vault_code_approved(code_hash)` → must be true.
//! 4. `view_access_key_list(vault_id)` → must contain exactly one key,
//!    function-call-restricted to `mpc_contract` + method
//!    `request_app_private_key`. **No FullAccess keys allowed**;
//!    scan-then-count ordering matches vault-checker exactly so a
//!    `[FullAccess, FunctionCall]` vault surfaces the right error.
//! 5. `vault.get_state()` → must report `unlocked == false`,
//!    `recovery == None`, `keystore_dao` matching the configured DAO id.

use near_primitives::types::AccountId;
use near_primitives::views::{AccessKeyInfoView, AccessKeyPermissionView};
use serde_json::json;

use crate::near::NearClient;

/// Failure modes — kept structurally close to
/// `wasi-examples/vault-checker/src/verify.rs::VerifyError` so logs
/// and error strings are recognisable across the two layers, but
/// re-declared here so the two crates evolve independently.
#[derive(Debug)]
pub enum VerifyError {
    AlreadyBanned,
    KeystoreDaoMalformed { method: String },
    KeystoreDaoUnreachable(anyhow::Error),
    AccountNotFound(anyhow::Error),
    CodeHashMissing,
    CodeHashNotApproved { code_hash: String },
    AccessKeyListUnreachable(anyhow::Error),
    UnexpectedAccessKeyCount { expected: usize, got: usize },
    /// A FullAccess key was found — the most dangerous failure mode.
    /// Any party holding that key can subvert the vault's security.
    FullAccessKeyPresent,
    /// A function-call key was found whose receiver / method-list does
    /// not match `(mpc_contract, [request_app_private_key])`.
    FunctionCallKeyMisconfigured {
        receiver: String,
        methods: Vec<String>,
    },
    VaultStateUnreachable(anyhow::Error),
    VaultStateInvalid(String),
    KeystoreDaoMismatch {
        configured: String,
        on_chain: String,
    },
    /// State's `mpc_contract` field does not match the worker's
    /// configured MPC contract id. Defense in
    /// depth on top of the access-key-receiver check, catches a vault
    /// whose state advertises one MPC but whose access key targets
    /// another (a misconfigured deploy that vault-checker's
    /// access-key-only check would miss).
    MpcContractMismatch {
        configured: String,
        on_chain: String,
    },
    VaultUnlocked,
    VaultRecoveryInProgress,
}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerifyError::AlreadyBanned => write!(f, "vault is currently banned by the keystore DAO"),
            VerifyError::KeystoreDaoMalformed { method } => write!(
                f,
                "keystore-dao returned malformed response for {method} (expected bool)"
            ),
            VerifyError::KeystoreDaoUnreachable(e) => write!(f, "keystore-dao RPC failed: {e}"),
            VerifyError::AccountNotFound(e) => write!(f, "vault account RPC failed: {e}"),
            VerifyError::CodeHashMissing => write!(f, "vault account has no code_hash (not deployed?)"),
            VerifyError::CodeHashNotApproved { code_hash } => {
                write!(f, "vault code hash {code_hash} is not approved by the DAO")
            }
            VerifyError::AccessKeyListUnreachable(e) => {
                write!(f, "view_access_key_list RPC failed: {e}")
            }
            VerifyError::UnexpectedAccessKeyCount { expected, got } => write!(
                f,
                "vault must have exactly {expected} access key(s); found {got}"
            ),
            VerifyError::FullAccessKeyPresent => write!(
                f,
                "vault has a FullAccess key — security invariant broken"
            ),
            VerifyError::FunctionCallKeyMisconfigured { receiver, methods } => write!(
                f,
                "TEE function-call key has wrong scope: receiver={receiver}, methods={methods:?}"
            ),
            VerifyError::VaultStateUnreachable(e) => write!(f, "vault.get_state RPC failed: {e}"),
            VerifyError::VaultStateInvalid(e) => write!(f, "vault.get_state response invalid: {e}"),
            VerifyError::KeystoreDaoMismatch { configured, on_chain } => write!(
                f,
                "vault's on-chain keystore_dao={on_chain} does not match configured {configured}"
            ),
            VerifyError::MpcContractMismatch { configured, on_chain } => write!(
                f,
                "vault's on-chain mpc_contract={on_chain} does not match configured {configured}"
            ),
            VerifyError::VaultUnlocked => write!(
                f,
                "vault is already unlocked — recovery happened; not eligible for verification"
            ),
            VerifyError::VaultRecoveryInProgress => write!(
                f,
                "vault has a recovery in progress; verification deferred"
            ),
        }
    }
}

impl std::error::Error for VerifyError {}

/// Run all five defense-in-depth checks. Returns `Ok(())` only if
/// every check passes; otherwise returns the first failure.
///
/// The caller (`/sign-vault-verification` handler) MUST refuse to
/// sign `mark_vault_verified` unless this function returns `Ok(())`.
pub async fn verify_vault_for_signing(
    near_client: &NearClient,
    keystore_dao: &AccountId,
    mpc_contract: &AccountId,
    vault_id: &AccountId,
) -> Result<(), VerifyError> {
    // 0. Banned check — fast deny.
    let is_banned = view_call_bool(
        near_client,
        keystore_dao,
        "is_vault_banned",
        json!({ "vault_id": vault_id }),
    )
    .await?;
    if is_banned {
        return Err(VerifyError::AlreadyBanned);
    }

    // 1. Account → code_hash.
    let code_hash = near_client
        .view_account_code_hash(vault_id)
        .await
        .map_err(VerifyError::AccountNotFound)?
        .ok_or(VerifyError::CodeHashMissing)?;

    // 1b. Code hash whitelist check on the DAO.
    let approved = view_call_bool(
        near_client,
        keystore_dao,
        "is_vault_code_approved",
        json!({ "hash": &code_hash }),
    )
    .await?;
    if !approved {
        return Err(VerifyError::CodeHashNotApproved { code_hash });
    }

    // 2. Access key list — scan for FullAccess BEFORE counting.
    //    Order matters: a vault with [FullAccess, FunctionCall] would
    //    pass a naive count check first and surface the wrong error.
    //    The security-critical signal is "any FullAccess at all".
    let keys = near_client
        .view_access_key_list_typed(vault_id)
        .await
        .map_err(VerifyError::AccessKeyListUnreachable)?;
    if keys
        .iter()
        .any(|k| matches!(k.access_key.permission, AccessKeyPermissionView::FullAccess))
    {
        return Err(VerifyError::FullAccessKeyPresent);
    }
    if keys.len() != 1 {
        return Err(VerifyError::UnexpectedAccessKeyCount {
            expected: 1,
            got: keys.len(),
        });
    }
    check_function_call_key(&keys[0], mpc_contract)?;

    // 3. Vault state — keystore_dao binding, mpc_contract binding,
    //    locked, no recovery.
    //
    // Cross-check `state.mpc_contract` against
    // the worker's configured MPC contract. The access-key-receiver
    // check above enforces "TEE key targets X"; this enforces "vault
    // state advertises X" — together they catch a misdeployed vault
    // whose state lies about which MPC backs it. End-users reading
    // `state.mpc_contract` would otherwise be misled if those two
    // diverged.
    let state = near_client
        .view_call_json(vault_id, "get_state", json!({}))
        .await
        .map_err(VerifyError::VaultStateUnreachable)?;
    let parsed = parse_vault_state(&state)?;
    if parsed.keystore_dao != keystore_dao.as_str() {
        return Err(VerifyError::KeystoreDaoMismatch {
            configured: keystore_dao.to_string(),
            on_chain: parsed.keystore_dao,
        });
    }
    if parsed.mpc_contract != mpc_contract.as_str() {
        return Err(VerifyError::MpcContractMismatch {
            configured: mpc_contract.to_string(),
            on_chain: parsed.mpc_contract,
        });
    }
    if parsed.unlocked {
        return Err(VerifyError::VaultUnlocked);
    }
    if parsed.recovery_in_progress {
        return Err(VerifyError::VaultRecoveryInProgress);
    }

    // Triage breadcrumb (debug-level) — surfaces the parent so an
    // operator looking at logs after a verification PASS can correlate
    // the vault back to a customer NEAR account without an extra RPC.
    // Intentionally `debug` (not `info`) to avoid noisy steady-state
    // logging; flip the env-filter when investigating.
    tracing::debug!(
        vault_id = %vault_id,
        parent = %parsed.parent,
        "vault verification PASSED"
    );

    Ok(())
}

// ===== helpers =====

async fn view_call_bool(
    near_client: &NearClient,
    contract_id: &AccountId,
    method: &str,
    args: serde_json::Value,
) -> Result<bool, VerifyError> {
    let response = near_client
        .view_call_json(contract_id, method, args)
        .await
        .map_err(VerifyError::KeystoreDaoUnreachable)?;
    response
        .as_bool()
        .ok_or_else(|| VerifyError::KeystoreDaoMalformed { method: method.to_string() })
}

fn check_function_call_key(
    key: &AccessKeyInfoView,
    mpc_contract: &AccountId,
) -> Result<(), VerifyError> {
    match &key.access_key.permission {
        AccessKeyPermissionView::FullAccess => {
            // Unreachable in production because we scanned for FullAccess
            // first; kept as defense-in-depth in case the caller refactors.
            Err(VerifyError::FullAccessKeyPresent)
        }
        AccessKeyPermissionView::FunctionCall {
            allowance: _, // allowance not part of the security invariant
            receiver_id,
            method_names,
        } => {
            // `request_app_private_key` is the MPC contract's CKD entry
            // point. The TEE key MUST be restricted to this method on
            // exactly the configured MPC contract — anything else means
            // the vault was deployed against a different policy and we
            // cannot trust its security guarantees.
            let methods_ok =
                method_names.len() == 1 && method_names[0] == "request_app_private_key";
            if receiver_id != mpc_contract.as_str() || !methods_ok {
                return Err(VerifyError::FunctionCallKeyMisconfigured {
                    receiver: receiver_id.to_string(),
                    methods: method_names.clone(),
                });
            }
            Ok(())
        }
    }
}

/// Parsed view of `vault.get_state()`.
///
/// `parent` is informational — the verifier has no ground-truth for
/// what the customer's parent account "should" be, so it is NEVER
/// gated on. It is parsed and surfaced purely so that incident-triage
/// log lines (`tracing::warn!` calls in `verify_vault_for_signing`
/// and the `/sign-vault-verification` handler) can identify *whose*
/// vault failed verification without a separate RPC round-trip.
#[derive(Debug)]
struct ParsedVaultState {
    parent: String,
    keystore_dao: String,
    mpc_contract: String,
    unlocked: bool,
    recovery_in_progress: bool,
}

fn parse_vault_state(payload: &serde_json::Value) -> Result<ParsedVaultState, VerifyError> {
    let parent = payload
        .get("parent")
        .and_then(|x| x.as_str())
        .ok_or_else(|| VerifyError::VaultStateInvalid("missing parent".to_string()))?
        .to_string();
    let keystore_dao = payload
        .get("keystore_dao")
        .and_then(|x| x.as_str())
        .ok_or_else(|| VerifyError::VaultStateInvalid("missing keystore_dao".to_string()))?
        .to_string();
    let mpc_contract = payload
        .get("mpc_contract")
        .and_then(|x| x.as_str())
        .ok_or_else(|| VerifyError::VaultStateInvalid("missing mpc_contract".to_string()))?
        .to_string();
    let unlocked = payload
        .get("unlocked")
        .and_then(|x| x.as_bool())
        .ok_or_else(|| VerifyError::VaultStateInvalid("missing unlocked".to_string()))?;
    let recovery_in_progress = payload
        .get("recovery")
        .map(|r| !r.is_null())
        .unwrap_or(false);
    Ok(ParsedVaultState {
        parent,
        keystore_dao,
        mpc_contract,
        unlocked,
        recovery_in_progress,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn dao() -> AccountId {
        AccountId::from_str("keystore-dao.testnet").unwrap()
    }

    fn mpc() -> AccountId {
        AccountId::from_str("v1.signer.testnet").unwrap()
    }

    // Pure parsers — testable without a NearClient.

    fn full_state(unlocked: bool, recovery_present: bool) -> serde_json::Value {
        json!({
            "parent": "alice.testnet",
            "keystore_dao": "keystore-dao.testnet",
            "mpc_contract": "v1.signer.testnet",
            "registered_tee_keys": [],
            "unlocked": unlocked,
            "recovery": if recovery_present {
                json!({ "initiated_at": 0, "finalize_after": 0, "finalize_before": 0, "trigger": "Cessation" })
            } else {
                serde_json::Value::Null
            },
            "unilateral_exit_window_secs": 86400,
        })
    }

    #[test]
    fn parse_vault_state_locked_no_recovery() {
        let parsed = parse_vault_state(&full_state(false, false)).unwrap();
        assert_eq!(parsed.keystore_dao, "keystore-dao.testnet");
        assert_eq!(parsed.mpc_contract, "v1.signer.testnet");
        assert!(!parsed.unlocked);
        assert!(!parsed.recovery_in_progress);
    }

    #[test]
    fn parse_vault_state_recovery_in_progress() {
        let parsed = parse_vault_state(&full_state(false, true)).unwrap();
        assert!(parsed.recovery_in_progress, "non-null recovery must flag");
    }

    #[test]
    fn parse_vault_state_unlocked_flag() {
        let parsed = parse_vault_state(&full_state(true, false)).unwrap();
        assert!(parsed.unlocked);
    }

    #[test]
    fn parse_vault_state_missing_keystore_dao_errors() {
        let mut p = full_state(false, false);
        p.as_object_mut().unwrap().remove("keystore_dao");
        let err = parse_vault_state(&p).unwrap_err();
        assert!(matches!(err, VerifyError::VaultStateInvalid(_)));
    }

    #[test]
    fn parse_vault_state_missing_mpc_contract_errors() {
        // Missing mpc_contract must surface, not
        // be silently treated as "matches anything".
        let mut p = full_state(false, false);
        p.as_object_mut().unwrap().remove("mpc_contract");
        let err = parse_vault_state(&p).unwrap_err();
        assert!(matches!(err, VerifyError::VaultStateInvalid(_)));
    }

    #[test]
    fn parse_vault_state_missing_unlocked_errors() {
        let mut p = full_state(false, false);
        p.as_object_mut().unwrap().remove("unlocked");
        let err = parse_vault_state(&p).unwrap_err();
        assert!(matches!(err, VerifyError::VaultStateInvalid(_)));
    }

    #[test]
    fn check_function_call_key_happy_path() {
        let key = AccessKeyInfoView {
            public_key: "ed25519:11111111111111111111111111111111".parse().unwrap(),
            access_key: near_primitives::views::AccessKeyView {
                nonce: 0,
                permission: AccessKeyPermissionView::FunctionCall {
                    allowance: None,
                    receiver_id: mpc().to_string(),
                    method_names: vec!["request_app_private_key".to_string()],
                },
            },
        };
        check_function_call_key(&key, &mpc()).expect("matches mpc + correct method");
    }

    #[test]
    fn check_function_call_key_wrong_receiver() {
        let key = AccessKeyInfoView {
            public_key: "ed25519:11111111111111111111111111111111".parse().unwrap(),
            access_key: near_primitives::views::AccessKeyView {
                nonce: 0,
                permission: AccessKeyPermissionView::FunctionCall {
                    allowance: None,
                    receiver_id: "bogus.testnet".to_string(),
                    method_names: vec!["request_app_private_key".to_string()],
                },
            },
        };
        let err = check_function_call_key(&key, &mpc()).unwrap_err();
        assert!(matches!(err, VerifyError::FunctionCallKeyMisconfigured { .. }));
    }

    #[test]
    fn check_function_call_key_extra_method() {
        // Even one extra method breaks the security invariant.
        let key = AccessKeyInfoView {
            public_key: "ed25519:11111111111111111111111111111111".parse().unwrap(),
            access_key: near_primitives::views::AccessKeyView {
                nonce: 0,
                permission: AccessKeyPermissionView::FunctionCall {
                    allowance: None,
                    receiver_id: mpc().to_string(),
                    method_names: vec![
                        "request_app_private_key".to_string(),
                        "evil".to_string(),
                    ],
                },
            },
        };
        let err = check_function_call_key(&key, &mpc()).unwrap_err();
        assert!(matches!(err, VerifyError::FunctionCallKeyMisconfigured { .. }));
    }

    #[test]
    fn check_function_call_key_wrong_method() {
        let key = AccessKeyInfoView {
            public_key: "ed25519:11111111111111111111111111111111".parse().unwrap(),
            access_key: near_primitives::views::AccessKeyView {
                nonce: 0,
                permission: AccessKeyPermissionView::FunctionCall {
                    allowance: None,
                    receiver_id: mpc().to_string(),
                    method_names: vec!["request_key".to_string()],
                },
            },
        };
        let err = check_function_call_key(&key, &mpc()).unwrap_err();
        assert!(matches!(err, VerifyError::FunctionCallKeyMisconfigured { .. }));
    }

    #[test]
    fn check_function_call_key_full_access_returns_error() {
        let key = AccessKeyInfoView {
            public_key: "ed25519:11111111111111111111111111111111".parse().unwrap(),
            access_key: near_primitives::views::AccessKeyView {
                nonce: 0,
                permission: AccessKeyPermissionView::FullAccess,
            },
        };
        let err = check_function_call_key(&key, &mpc()).unwrap_err();
        assert!(matches!(err, VerifyError::FullAccessKeyPresent));
    }

    // sanity: keystore_dao mismatch case
    #[test]
    fn keystore_dao_mismatch_is_distinct_error() {
        let mut p = full_state(false, false);
        p["keystore_dao"] = json!("other-dao.testnet");
        let parsed = parse_vault_state(&p).unwrap();
        assert_ne!(parsed.keystore_dao, dao().as_str());
    }

    // state.mpc_contract must drive its own
    // mismatch surface, distinct from keystore_dao.
    #[test]
    fn mpc_contract_mismatch_surfaces_distinct_field() {
        let mut p = full_state(false, false);
        p["mpc_contract"] = json!("scam.testnet");
        let parsed = parse_vault_state(&p).unwrap();
        assert_eq!(parsed.mpc_contract, "scam.testnet");
        assert_ne!(parsed.mpc_contract, mpc().as_str());
    }
}
