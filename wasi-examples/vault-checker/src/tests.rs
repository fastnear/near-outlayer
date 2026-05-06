use super::verify::*;
use std::cell::RefCell;
use std::collections::HashMap;

/// In-memory RPC double. Each call type is keyed by a tuple of its
/// salient args; the test sets up the responses ahead of time.
#[derive(Default)]
struct MockRpc {
    /// `(contract_id, method_name, args_json)` → JSON response or
    /// `Err(string)`.
    view_calls: RefCell<HashMap<(String, String, String), Result<String, String>>>,
    /// `account_id` → JSON response.
    view_account_calls: RefCell<HashMap<String, Result<String, String>>>,
    /// `account_id` → JSON response.
    view_access_key_list_calls: RefCell<HashMap<String, Result<String, String>>>,
}

impl MockRpc {
    fn expect_view(&self, contract: &str, method: &str, args: &str, response: &str) {
        self.view_calls.borrow_mut().insert(
            (contract.into(), method.into(), args.into()),
            Ok(response.into()),
        );
    }
    fn expect_account(&self, account_id: &str, response: &str) {
        self.view_account_calls
            .borrow_mut()
            .insert(account_id.into(), Ok(response.into()));
    }
    fn expect_access_key_list(&self, account_id: &str, response: &str) {
        self.view_access_key_list_calls
            .borrow_mut()
            .insert(account_id.into(), Ok(response.into()));
    }
}

impl NearRpc for MockRpc {
    fn view(&self, contract_id: &str, method: &str, args_json: &str) -> Result<String, String> {
        let key = (contract_id.into(), method.into(), args_json.into());
        self.view_calls
            .borrow()
            .get(&key)
            .cloned()
            .unwrap_or_else(|| Err(format!("MockRpc: unexpected view call {key:?}")))
    }
    fn view_account(&self, account_id: &str) -> Result<String, String> {
        self.view_account_calls
            .borrow()
            .get(account_id)
            .cloned()
            .unwrap_or_else(|| Err(format!("MockRpc: unexpected view_account({account_id})")))
    }
    fn view_access_key_list(&self, account_id: &str) -> Result<String, String> {
        self.view_access_key_list_calls
            .borrow()
            .get(account_id)
            .cloned()
            .unwrap_or_else(|| {
                Err(format!("MockRpc: unexpected view_access_key_list({account_id})"))
            })
    }
}

const VAULT: &str = "vault.alice.near";
const DAO: &str = "keystore-dao.outlayer.near";
const MPC: &str = "v1.signer.near";
const TEE_PUBKEY: &str = "ed25519:H9k5eiU4xXS3M4z8HzKJSLaZdqGdGwBG49o7orNC5LJ";
const APPROVED_HASH: &str = "AbCDeFgHiJkLmNoPqRsTuVwXyZ123456789AbcDeFGH";

fn cfg() -> VerifierConfig {
    VerifierConfig {
        keystore_dao: DAO.into(),
        mpc_contract: MPC.into(),
    }
}

/// `bool` view response shape: NEAR encodes the result as a UTF-8 byte
/// array inside `.result.result`. `true` → [116,114,117,101].
fn bool_view_response(value: bool) -> String {
    let s = if value { "true" } else { "false" };
    let bytes: Vec<u8> = s.bytes().collect();
    serde_json::json!({
        "result": { "result": bytes }
    })
    .to_string()
}

fn account_response(code_hash: &str) -> String {
    serde_json::json!({
        "result": {
            "amount": "5000000000000000000000000",
            "code_hash": code_hash,
            "storage_usage": 0u64,
            "block_height": 1u64,
            "block_hash": "11111111111111111111111111111111",
        }
    })
    .to_string()
}

fn key_list_response(keys: Vec<serde_json::Value>) -> String {
    serde_json::json!({
        "result": {
            "keys": keys,
            "block_hash": "11111111111111111111111111111111",
            "block_height": 1u64,
        }
    })
    .to_string()
}

fn fc_key(receiver: &str, methods: &[&str]) -> serde_json::Value {
    serde_json::json!({
        "public_key": TEE_PUBKEY,
        "access_key": {
            "nonce": 0u64,
            "permission": {
                "FunctionCall": {
                    "allowance": null,
                    "receiver_id": receiver,
                    "method_names": methods,
                }
            }
        }
    })
}

fn full_access_key(pubkey: &str) -> serde_json::Value {
    serde_json::json!({
        "public_key": pubkey,
        "access_key": {
            "nonce": 0u64,
            "permission": "FullAccess",
        }
    })
}

fn vault_state_response(keystore_dao: &str, unlocked: bool, recovery: serde_json::Value) -> String {
    vault_state_response_full(keystore_dao, MPC, unlocked, recovery)
}

/// Variant that lets a test override `mpc_contract` separately — used
/// by the explicit `MpcContractMismatch` test.
fn vault_state_response_full(
    keystore_dao: &str,
    mpc_contract: &str,
    unlocked: bool,
    recovery: serde_json::Value,
) -> String {
    let payload = serde_json::json!({
        "parent": "alice.near",
        "keystore_dao": keystore_dao,
        "mpc_contract": mpc_contract,
        "registered_tee_keys": [TEE_PUBKEY],
        "recovery": recovery,
        "unlocked": unlocked,
        "unilateral_exit_window_secs": 86400u64,
    });
    let s = payload.to_string();
    let bytes: Vec<u8> = s.bytes().collect();
    serde_json::json!({
        "result": { "result": bytes }
    })
    .to_string()
}

/// Configure the mock for a "happy" vault: not banned, approved code,
/// single TEE function-call key, locked, no recovery in flight.
fn happy_mock() -> MockRpc {
    let m = MockRpc::default();
    m.expect_view(
        DAO,
        "is_vault_banned",
        &serde_json::json!({ "vault_id": VAULT }).to_string(),
        &bool_view_response(false),
    );
    m.expect_account(VAULT, &account_response(APPROVED_HASH));
    m.expect_view(
        DAO,
        "is_vault_code_approved",
        &serde_json::json!({ "hash": APPROVED_HASH }).to_string(),
        &bool_view_response(true),
    );
    m.expect_access_key_list(
        VAULT,
        &key_list_response(vec![fc_key(MPC, &["request_app_private_key"])]),
    );
    m.expect_view(
        VAULT,
        "get_state",
        "{}",
        &vault_state_response(DAO, false, serde_json::Value::Null),
    );
    m
}

// ===== Happy path =====

#[test]
fn verify_vault_happy_path() {
    let m = happy_mock();
    verify_vault(&m, &cfg(), VAULT).expect("happy path must verify");
}

#[test]
fn check_already_verified_true() {
    let m = MockRpc::default();
    m.expect_view(
        DAO,
        "is_vault_verified",
        &serde_json::json!({ "vault_id": VAULT }).to_string(),
        &bool_view_response(true),
    );
    assert!(check_already_verified(&m, &cfg(), VAULT).unwrap());
}

#[test]
fn check_already_verified_false() {
    let m = MockRpc::default();
    m.expect_view(
        DAO,
        "is_vault_verified",
        &serde_json::json!({ "vault_id": VAULT }).to_string(),
        &bool_view_response(false),
    );
    assert!(!check_already_verified(&m, &cfg(), VAULT).unwrap());
}

// ===== Banned =====

#[test]
fn verify_vault_banned() {
    let m = MockRpc::default();
    m.expect_view(
        DAO,
        "is_vault_banned",
        &serde_json::json!({ "vault_id": VAULT }).to_string(),
        &bool_view_response(true),
    );
    let err = verify_vault(&m, &cfg(), VAULT).unwrap_err();
    assert_eq!(err, VerifyError::AlreadyBanned);
}

// ===== Code hash =====

#[test]
fn verify_vault_no_code_deployed() {
    let m = MockRpc::default();
    m.expect_view(
        DAO,
        "is_vault_banned",
        &serde_json::json!({ "vault_id": VAULT }).to_string(),
        &bool_view_response(false),
    );
    // NEAR sentinel for accounts with no contract deployed.
    m.expect_account(VAULT, &account_response("11111111111111111111111111111111"));
    let err = verify_vault(&m, &cfg(), VAULT).unwrap_err();
    assert_eq!(err, VerifyError::CodeHashMissing);
}

#[test]
fn verify_vault_code_hash_not_approved() {
    let m = MockRpc::default();
    m.expect_view(
        DAO,
        "is_vault_banned",
        &serde_json::json!({ "vault_id": VAULT }).to_string(),
        &bool_view_response(false),
    );
    let unknown_hash = "ZZZZZ123456789AbcDeFGHzZZZZ123456789Abc";
    m.expect_account(VAULT, &account_response(unknown_hash));
    m.expect_view(
        DAO,
        "is_vault_code_approved",
        &serde_json::json!({ "hash": unknown_hash }).to_string(),
        &bool_view_response(false),
    );
    let err = verify_vault(&m, &cfg(), VAULT).unwrap_err();
    match err {
        VerifyError::CodeHashNotApproved { code_hash } => assert_eq!(code_hash, unknown_hash),
        other => panic!("expected CodeHashNotApproved, got {other:?}"),
    }
}

// ===== Access keys =====

#[test]
fn verify_vault_full_access_key_present() {
    let m = MockRpc::default();
    m.expect_view(
        DAO,
        "is_vault_banned",
        &serde_json::json!({ "vault_id": VAULT }).to_string(),
        &bool_view_response(false),
    );
    m.expect_account(VAULT, &account_response(APPROVED_HASH));
    m.expect_view(
        DAO,
        "is_vault_code_approved",
        &serde_json::json!({ "hash": APPROVED_HASH }).to_string(),
        &bool_view_response(true),
    );
    m.expect_access_key_list(
        VAULT,
        &key_list_response(vec![full_access_key(TEE_PUBKEY)]),
    );
    let err = verify_vault(&m, &cfg(), VAULT).unwrap_err();
    assert_eq!(err, VerifyError::FullAccessKeyPresent);
}

#[test]
fn verify_vault_extra_access_key() {
    // Two keys present, even if both are correctly-scoped function-call
    // keys, must still fail the count check — the plan says EXACTLY one.
    let m = MockRpc::default();
    m.expect_view(
        DAO,
        "is_vault_banned",
        &serde_json::json!({ "vault_id": VAULT }).to_string(),
        &bool_view_response(false),
    );
    m.expect_account(VAULT, &account_response(APPROVED_HASH));
    m.expect_view(
        DAO,
        "is_vault_code_approved",
        &serde_json::json!({ "hash": APPROVED_HASH }).to_string(),
        &bool_view_response(true),
    );
    m.expect_access_key_list(
        VAULT,
        &key_list_response(vec![
            fc_key(MPC, &["request_app_private_key"]),
            fc_key(MPC, &["request_app_private_key"]),
        ]),
    );
    let err = verify_vault(&m, &cfg(), VAULT).unwrap_err();
    assert_eq!(
        err,
        VerifyError::UnexpectedAccessKeyCount { expected: 1, got: 2 }
    );
}

#[test]
fn verify_vault_fc_key_wrong_receiver() {
    let m = MockRpc::default();
    m.expect_view(
        DAO,
        "is_vault_banned",
        &serde_json::json!({ "vault_id": VAULT }).to_string(),
        &bool_view_response(false),
    );
    m.expect_account(VAULT, &account_response(APPROVED_HASH));
    m.expect_view(
        DAO,
        "is_vault_code_approved",
        &serde_json::json!({ "hash": APPROVED_HASH }).to_string(),
        &bool_view_response(true),
    );
    m.expect_access_key_list(
        VAULT,
        &key_list_response(vec![fc_key("evil.near", &["request_app_private_key"])]),
    );
    let err = verify_vault(&m, &cfg(), VAULT).unwrap_err();
    match err {
        VerifyError::FunctionCallKeyMisconfigured { receiver, methods } => {
            assert_eq!(receiver, "evil.near");
            assert_eq!(methods, vec!["request_app_private_key".to_string()]);
        }
        other => panic!("expected FunctionCallKeyMisconfigured, got {other:?}"),
    }
}

#[test]
fn verify_vault_fc_key_wrong_method() {
    let m = MockRpc::default();
    m.expect_view(
        DAO,
        "is_vault_banned",
        &serde_json::json!({ "vault_id": VAULT }).to_string(),
        &bool_view_response(false),
    );
    m.expect_account(VAULT, &account_response(APPROVED_HASH));
    m.expect_view(
        DAO,
        "is_vault_code_approved",
        &serde_json::json!({ "hash": APPROVED_HASH }).to_string(),
        &bool_view_response(true),
    );
    m.expect_access_key_list(
        VAULT,
        &key_list_response(vec![fc_key(MPC, &["transfer", "deploy_contract"])]),
    );
    let err = verify_vault(&m, &cfg(), VAULT).unwrap_err();
    match err {
        VerifyError::FunctionCallKeyMisconfigured { methods, .. } => {
            assert_eq!(methods, vec!["transfer".to_string(), "deploy_contract".to_string()]);
        }
        other => panic!("expected FunctionCallKeyMisconfigured, got {other:?}"),
    }
}

// ===== Vault state =====

#[test]
fn verify_vault_keystore_dao_mismatch() {
    let m = happy_mock();
    // Re-stub get_state with a different keystore_dao value.
    m.expect_view(
        VAULT,
        "get_state",
        "{}",
        &vault_state_response("evil-dao.near", false, serde_json::Value::Null),
    );
    let err = verify_vault(&m, &cfg(), VAULT).unwrap_err();
    match err {
        VerifyError::KeystoreDaoMismatch { configured, on_chain } => {
            assert_eq!(configured, DAO);
            assert_eq!(on_chain, "evil-dao.near");
        }
        other => panic!("expected KeystoreDaoMismatch, got {other:?}"),
    }
}

#[test]
fn verify_vault_mpc_contract_mismatch() {
    // vault-checker must mirror the worker's `MpcContractMismatch`
    // cross-check, otherwise the two layers disagree on what
    // "verified" means and a customer hits a confusing pipeline
    // (vault-checker says ok, worker rejects).
    let m = happy_mock();
    m.expect_view(
        VAULT,
        "get_state",
        "{}",
        &vault_state_response_full(DAO, "evil.signer.testnet", false, serde_json::Value::Null),
    );
    let err = verify_vault(&m, &cfg(), VAULT).unwrap_err();
    match err {
        VerifyError::MpcContractMismatch { configured, on_chain } => {
            assert_eq!(configured, MPC);
            assert_eq!(on_chain, "evil.signer.testnet");
        }
        other => panic!("expected MpcContractMismatch, got {other:?}"),
    }
}

#[test]
fn verify_vault_already_unlocked() {
    let m = happy_mock();
    m.expect_view(
        VAULT,
        "get_state",
        "{}",
        &vault_state_response(DAO, true, serde_json::Value::Null),
    );
    let err = verify_vault(&m, &cfg(), VAULT).unwrap_err();
    assert_eq!(err, VerifyError::VaultUnlocked);
}

#[test]
fn verify_vault_recovery_in_progress() {
    let m = happy_mock();
    m.expect_view(
        VAULT,
        "get_state",
        "{}",
        &vault_state_response(
            DAO,
            false,
            serde_json::json!({
                "initiated_at": 1u64,
                "finalize_after": 2u64,
                "finalize_before": 3u64,
                "trigger": "Cessation",
            }),
        ),
    );
    let err = verify_vault(&m, &cfg(), VAULT).unwrap_err();
    assert_eq!(err, VerifyError::VaultRecoveryInProgress);
}

// ===== RPC errors =====

#[test]
fn verify_vault_view_rpc_failure() {
    // The is_vault_banned call is the very first one; if it fails the
    // verifier must surface KeystoreDaoUnreachable rather than continue.
    let m = MockRpc::default();
    let err = verify_vault(&m, &cfg(), VAULT).unwrap_err();
    match err {
        VerifyError::KeystoreDaoUnreachable(_) => {}
        other => panic!("expected KeystoreDaoUnreachable, got {other:?}"),
    }
}

#[test]
fn verify_vault_full_access_takes_priority_over_count_mismatch() {
    // Audit B3: a vault with [FullAccess, FunctionCall] must surface
    // FullAccessKeyPresent (the security-critical signal), NOT
    // UnexpectedAccessKeyCount{1, 2}. The latter would understate the
    // severity for an operator scanning logs.
    let m = MockRpc::default();
    m.expect_view(
        DAO,
        "is_vault_banned",
        &serde_json::json!({ "vault_id": VAULT }).to_string(),
        &bool_view_response(false),
    );
    m.expect_account(VAULT, &account_response(APPROVED_HASH));
    m.expect_view(
        DAO,
        "is_vault_code_approved",
        &serde_json::json!({ "hash": APPROVED_HASH }).to_string(),
        &bool_view_response(true),
    );
    m.expect_access_key_list(
        VAULT,
        &key_list_response(vec![
            full_access_key("ed25519:Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            fc_key(MPC, &["request_app_private_key"]),
        ]),
    );
    let err = verify_vault(&m, &cfg(), VAULT).unwrap_err();
    assert_eq!(err, VerifyError::FullAccessKeyPresent);
}

#[test]
fn verify_vault_zero_keys_fails_count_check() {
    // Edge case: vault has no access keys at all. Should fail
    // UnexpectedAccessKeyCount, not panic.
    let m = MockRpc::default();
    m.expect_view(
        DAO,
        "is_vault_banned",
        &serde_json::json!({ "vault_id": VAULT }).to_string(),
        &bool_view_response(false),
    );
    m.expect_account(VAULT, &account_response(APPROVED_HASH));
    m.expect_view(
        DAO,
        "is_vault_code_approved",
        &serde_json::json!({ "hash": APPROVED_HASH }).to_string(),
        &bool_view_response(true),
    );
    m.expect_access_key_list(VAULT, &key_list_response(vec![]));
    let err = verify_vault(&m, &cfg(), VAULT).unwrap_err();
    assert_eq!(
        err,
        VerifyError::UnexpectedAccessKeyCount { expected: 1, got: 0 }
    );
}

#[test]
fn verify_vault_keystore_dao_returns_malformed_bool() {
    // Audit C1: keystore-dao replies with garbage where a bool is
    // expected. We must surface KeystoreDaoMalformed rather than
    // silently treat as "false" (which would either fail-open or
    // fail-closed depending on the call — both unsafe).
    let m = MockRpc::default();
    // Empty/garbage response — no .result.result array.
    m.expect_view(
        DAO,
        "is_vault_banned",
        &serde_json::json!({ "vault_id": VAULT }).to_string(),
        r#"{"result": {"some_other_field": 42}}"#,
    );
    let err = verify_vault(&m, &cfg(), VAULT).unwrap_err();
    match err {
        VerifyError::KeystoreDaoMalformed { method } => {
            assert_eq!(method, "is_vault_banned");
        }
        other => panic!("expected KeystoreDaoMalformed, got {other:?}"),
    }
}

#[test]
fn check_already_verified_malformed_bool() {
    let m = MockRpc::default();
    m.expect_view(
        DAO,
        "is_vault_verified",
        &serde_json::json!({ "vault_id": VAULT }).to_string(),
        r#"{"result": {"junk": null}}"#,
    );
    let err = check_already_verified(&m, &cfg(), VAULT).unwrap_err();
    match err {
        VerifyError::KeystoreDaoMalformed { method } => {
            assert_eq!(method, "is_vault_verified");
        }
        other => panic!("expected KeystoreDaoMalformed, got {other:?}"),
    }
}
