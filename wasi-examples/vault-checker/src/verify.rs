//! Pure verification logic, decoupled from the WASI host functions so
//! the unit tests can drive it with a mocked RPC trait without spinning
//! up an OutLayer worker.
//!
//! The on-chain checks performed (in order):
//!
//! 1. `keystore_dao.is_vault_verified(vault)` — fast positive: if already
//!    verified, return without re-running the full sweep.
//! 2. `view_account(vault)` → extract `code_hash`.
//! 3. `keystore_dao.is_vault_code_approved(code_hash)` → must be true.
//! 4. `view_access_key_list(vault)` → must contain exactly one key,
//!    function-call-restricted to `mpc_contract` + method
//!    `request_app_private_key`. **No full-access keys allowed.**
//! 5. `vault.get_state()` → must report `unlocked == false`,
//!    `recovery == None`, and `keystore_dao` matching the configured
//!    DAO account id.
//!
//! When all five checks pass and `KEYSTORE_BASE_URL` +
//! `KEYSTORE_AUTH_TOKEN` are set, the WASI host POSTs to
//! keystore-worker's internal `/sign-vault-verification` endpoint,
//! which re-verifies (defense in depth) and submits
//! `mark_vault_verified` on-chain. In state-only mode (env vars
//! absent), `Output.state_only = true` flags this to callers so they
//! don't mistake "checks passed" for "on-chain mark landed".

/// Trait the host RPC functions implement. Production wiring lives in
/// `main.rs` (calls into `near::rpc::api::*`); the unit tests provide
/// in-memory stubs.
pub trait NearRpc {
    fn view(
        &self,
        contract_id: &str,
        method: &str,
        args_json: &str,
    ) -> Result<String, String>;
    fn view_account(&self, account_id: &str) -> Result<String, String>;
    fn view_access_key_list(&self, account_id: &str) -> Result<String, String>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum VerifyError {
    AlreadyBanned,
    /// keystore-dao returned a response that didn't match the expected
    /// `bool` payload shape. Distinguished from `KeystoreDaoUnreachable`
    /// (RPC-level error) so an operator inspecting logs knows to look
    /// at the contract response, not the RPC layer.
    KeystoreDaoMalformed { method: String },
    KeystoreDaoUnreachable(String),
    AccountNotFound(String),
    CodeHashMissing,
    CodeHashNotApproved {
        code_hash: String,
    },
    AccessKeyListUnreachable(String),
    UnexpectedAccessKeyCount {
        expected: usize,
        got: usize,
    },
    /// A full-access key was found. This is the most dangerous failure
    /// mode — any party holding that key can subvert the vault's
    /// security guarantees.
    FullAccessKeyPresent,
    /// A function-call key was found whose receiver / method-list does
    /// not match `(mpc_contract, [request_app_private_key])`.
    FunctionCallKeyMisconfigured {
        receiver: String,
        methods: Vec<String>,
    },
    VaultStateUnreachable(String),
    VaultStateInvalid(String),
    /// The vault's state-reported `keystore_dao` doesn't match the
    /// configured DAO account id. Either the customer is pointing at
    /// the wrong DAO (operator misconfig) or the WASI agent itself was
    /// configured against a different network — surfacing both.
    KeystoreDaoMismatch {
        configured: String,
        on_chain: String,
    },
    /// State's `mpc_contract` doesn't match the configured MPC contract
    /// id. Defense in depth on top of the access-key-receiver check —
    /// catches a vault whose state advertises one MPC but whose access
    /// key targets another. Without this cross-check, vault-checker
    /// would say "verified ✓" while the keystore-worker would refuse
    /// (it now mirrors the same check), creating a layer-disagreement
    /// bug. Mirrors `keystore-worker/src/vault_verifier.rs::MpcContractMismatch`.
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
                "keystore-dao returned malformed response for {method} (expected bool payload)"
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
                "vault has a full-access key — security invariant broken; \
                 deploy a fresh vault that follows the atomic-deploy pattern"
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

/// Configuration the agent is invoked with — pinned at deploy time.
#[derive(Debug, Clone)]
pub struct VerifierConfig {
    pub keystore_dao: String,
    /// MPC contract account id. The TEE function-call key on the
    /// vault MUST be scoped to this exact `receiver`.
    pub mpc_contract: String,
}

/// Driver. Returns `Ok(())` if verification passed, `Err(VerifyError)`
/// otherwise.
pub fn verify_vault(
    rpc: &impl NearRpc,
    cfg: &VerifierConfig,
    vault_id: &str,
) -> Result<(), VerifyError> {
    // 0. Banned check (cheap, short-circuits everything).
    let is_banned_resp = rpc
        .view(
            &cfg.keystore_dao,
            "is_vault_banned",
            &serde_json::json!({ "vault_id": vault_id }).to_string(),
        )
        .map_err(VerifyError::KeystoreDaoUnreachable)?;
    let is_banned = extract_view_bool(&is_banned_resp).ok_or_else(|| {
        VerifyError::KeystoreDaoMalformed {
            method: "is_vault_banned".to_string(),
        }
    })?;
    if is_banned {
        return Err(VerifyError::AlreadyBanned);
    }

    // 1. Code hash check.
    let account_resp = rpc
        .view_account(vault_id)
        .map_err(VerifyError::AccountNotFound)?;
    let code_hash = extract_code_hash(&account_resp).ok_or(VerifyError::CodeHashMissing)?;

    let approved_resp = rpc
        .view(
            &cfg.keystore_dao,
            "is_vault_code_approved",
            &serde_json::json!({ "hash": code_hash }).to_string(),
        )
        .map_err(VerifyError::KeystoreDaoUnreachable)?;
    let approved = extract_view_bool(&approved_resp).ok_or_else(|| {
        VerifyError::KeystoreDaoMalformed {
            method: "is_vault_code_approved".to_string(),
        }
    })?;
    if !approved {
        return Err(VerifyError::CodeHashNotApproved { code_hash });
    }

    // 2. Access-key list check.
    //
    // Order matters: we scan the WHOLE list for any FullAccess key
    // BEFORE the count check. A vault with `[FullAccess, FunctionCall]`
    // would pass a naive "count != 1" check first and surface the wrong
    // error — the security-critical signal is "any full-access key at
    // all", not "wrong number of keys".
    let keys_resp = rpc
        .view_access_key_list(vault_id)
        .map_err(VerifyError::AccessKeyListUnreachable)?;
    let keys = extract_access_keys(&keys_resp)
        .map_err(VerifyError::AccessKeyListUnreachable)?;
    if keys
        .iter()
        .any(|k| matches!(k.permission, AccessKeyPermission::FullAccess))
    {
        return Err(VerifyError::FullAccessKeyPresent);
    }
    if keys.len() != 1 {
        return Err(VerifyError::UnexpectedAccessKeyCount {
            expected: 1,
            got: keys.len(),
        });
    }
    match &keys[0].permission {
        AccessKeyPermission::FullAccess => unreachable!("scanned above"),
        AccessKeyPermission::FunctionCall {
            receiver_id,
            method_names,
        } => {
            // `request_app_private_key` is the MPC contract's CKD
            // (Conditional Key Derivation) entry point. It's a method
            // NAME on `cfg.mpc_contract`, not an argument — no private
            // key is ever passed on the wire. Args are
            // `{ derivation_path, app_public_key, domain_id }` where
            // `app_public_key` is the caller's ephemeral public key.
            // The MPC network returns an encrypted CKD payload that the
            // keystore-worker decrypts inside its TEE, materialising
            // the per-vault master only inside the enclave.
            //
            // We accept the vault key ONLY if it is restricted to this
            // exact method on the configured MPC contract — nothing
            // else. Any extra method or a different receiver means the
            // vault was deployed against a different policy and we
            // cannot trust its security guarantees.
            let methods_ok = method_names.len() == 1
                && method_names[0] == "request_app_private_key";
            if receiver_id != &cfg.mpc_contract || !methods_ok {
                return Err(VerifyError::FunctionCallKeyMisconfigured {
                    receiver: receiver_id.clone(),
                    methods: method_names.clone(),
                });
            }
        }
    }

    // 3. Vault state check.
    let state_resp = rpc
        .view(
            vault_id,
            "get_state",
            "{}",
        )
        .map_err(VerifyError::VaultStateUnreachable)?;
    let state = extract_vault_state(&state_resp)
        .map_err(VerifyError::VaultStateInvalid)?;
    if state.keystore_dao != cfg.keystore_dao {
        return Err(VerifyError::KeystoreDaoMismatch {
            configured: cfg.keystore_dao.clone(),
            on_chain: state.keystore_dao,
        });
    }
    // Cross-check `state.mpc_contract` against the configured MPC.
    // The access-key-receiver check above enforces "TEE key targets X";
    // this enforces "vault state advertises X" — together they catch
    // a misdeployed vault whose state lies about which MPC backs it.
    // Mirrors `keystore-worker/src/vault_verifier.rs` so vault-checker
    // and the worker's defense-in-depth pass agree on the same set
    // of invariants — without this, vault-checker could approve a
    // vault that the worker would reject, forcing customers through
    // a verification pipeline that disagrees layer-to-layer.
    if state.mpc_contract != cfg.mpc_contract {
        return Err(VerifyError::MpcContractMismatch {
            configured: cfg.mpc_contract.clone(),
            on_chain: state.mpc_contract,
        });
    }
    if state.unlocked {
        return Err(VerifyError::VaultUnlocked);
    }
    if state.recovery_in_progress {
        return Err(VerifyError::VaultRecoveryInProgress);
    }

    Ok(())
}

/// Was-this-vault-already-verified shortcut — exposed separately because
/// the agent's `verify(vault_id)` action returns `already_verified =
/// true` when the DAO already remembers it. The caller skips the
/// keystore-worker HTTP round-trip in that case.
pub fn check_already_verified(
    rpc: &impl NearRpc,
    cfg: &VerifierConfig,
    vault_id: &str,
) -> Result<bool, VerifyError> {
    let resp = rpc
        .view(
            &cfg.keystore_dao,
            "is_vault_verified",
            &serde_json::json!({ "vault_id": vault_id }).to_string(),
        )
        .map_err(VerifyError::KeystoreDaoUnreachable)?;
    extract_view_bool(&resp).ok_or_else(|| VerifyError::KeystoreDaoMalformed {
        method: "is_vault_verified".to_string(),
    })
}

// ===== Response parsers =====
//
// NEAR's JSON-RPC view-call response wraps a base64-encoded payload
// inside `.result.result: [u8]`. For `view_account` and
// `view_access_key_list` the response is the typed object directly
// inside `.result`.

#[derive(Debug, Clone, PartialEq)]
struct AccessKeyEntry {
    public_key: String,
    permission: AccessKeyPermission,
}

#[derive(Debug, Clone, PartialEq)]
enum AccessKeyPermission {
    FullAccess,
    FunctionCall {
        receiver_id: String,
        method_names: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq)]
struct VaultStateView {
    keystore_dao: String,
    mpc_contract: String,
    unlocked: bool,
    recovery_in_progress: bool,
}

fn extract_view_bool(resp_json: &str) -> Option<bool> {
    // NEAR wraps view responses in `.result.result: [byte, byte, ...]`
    // where bytes are JSON-encoded. So `bool` view returns `[116,114,
    // 117,101]` for `true` (UTF-8 of "true").
    let v: serde_json::Value = serde_json::from_str(resp_json).ok()?;
    let bytes = v.pointer("/result/result")?.as_array()?;
    let mut s = String::with_capacity(bytes.len());
    for b in bytes {
        s.push(char::from_u32(b.as_u64()? as u32)?);
    }
    serde_json::from_str::<bool>(s.trim()).ok()
}

fn extract_code_hash(resp_json: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(resp_json).ok()?;
    let h = v.pointer("/result/code_hash")?.as_str()?;
    if h == "11111111111111111111111111111111" {
        // NEAR sentinel for "no code". Treat as missing.
        None
    } else {
        Some(h.to_string())
    }
}

fn extract_access_keys(resp_json: &str) -> Result<Vec<AccessKeyEntry>, String> {
    let v: serde_json::Value = serde_json::from_str(resp_json)
        .map_err(|e| format!("bad json: {e}"))?;
    let keys = v
        .pointer("/result/keys")
        .and_then(|k| k.as_array())
        .ok_or_else(|| "missing /result/keys".to_string())?;
    let mut out = Vec::with_capacity(keys.len());
    for k in keys {
        let pubkey = k
            .get("public_key")
            .and_then(|p| p.as_str())
            .ok_or_else(|| "missing public_key".to_string())?
            .to_string();
        let perm = k
            .pointer("/access_key/permission")
            .ok_or_else(|| "missing permission".to_string())?;
        let permission = if perm.as_str() == Some("FullAccess") {
            AccessKeyPermission::FullAccess
        } else if let Some(fc) = perm.get("FunctionCall") {
            let receiver_id = fc
                .get("receiver_id")
                .and_then(|r| r.as_str())
                .ok_or_else(|| "missing FunctionCall.receiver_id".to_string())?
                .to_string();
            let method_names = fc
                .get("method_names")
                .and_then(|m| m.as_array())
                .ok_or_else(|| "missing FunctionCall.method_names".to_string())?
                .iter()
                .filter_map(|m| m.as_str().map(str::to_string))
                .collect();
            AccessKeyPermission::FunctionCall {
                receiver_id,
                method_names,
            }
        } else {
            return Err(format!("unknown permission shape: {perm}"));
        };
        out.push(AccessKeyEntry {
            public_key: pubkey,
            permission,
        });
    }
    Ok(out)
}

fn extract_vault_state(resp_json: &str) -> Result<VaultStateView, String> {
    // Note: vault state has more fields than we read here (parent,
    // registered_tee_keys, unilateral_exit_window_secs).
    //
    // * `parent` is intentionally NOT validated. The verifier has no
    //   ground-truth `parent` to compare against — the customer asks
    //   "verify this vault", not "is this my vault". Anyone can be
    //   `parent`; the security invariants this agent enforces (no
    //   full-access keys, code hash whitelisted, etc.) hold regardless
    //   of who the parent account is.
    // * `mpc_contract` IS validated. Earlier this was deemed redundant
    //   with the access-key-receiver check, but the keystore-worker's
    //   mirror of this verifier checks it explicitly — so without
    //   matching the check here, vault-checker would have approved
    //   vaults that the worker would later reject. We extract it and
    //   the caller compares.
    // * `registered_tee_keys` is informational — the authoritative
    //   key list is `view_access_key_list`, which we already checked.
    // * `unilateral_exit_window_secs` is customer-configurable in the
    //   plan-allowed range and not security-critical for verification.
    let v: serde_json::Value = serde_json::from_str(resp_json)
        .map_err(|e| format!("bad json: {e}"))?;
    let bytes = v
        .pointer("/result/result")
        .and_then(|r| r.as_array())
        .ok_or_else(|| "missing /result/result".to_string())?;
    let mut s = String::with_capacity(bytes.len());
    for b in bytes {
        s.push(char::from_u32(b.as_u64().ok_or_else(|| "non-byte in result".to_string())? as u32)
            .ok_or_else(|| "invalid utf-8 byte".to_string())?);
    }
    let payload: serde_json::Value =
        serde_json::from_str(&s).map_err(|e| format!("payload not json: {e}"))?;
    let keystore_dao = payload
        .get("keystore_dao")
        .and_then(|x| x.as_str())
        .ok_or_else(|| "missing keystore_dao".to_string())?
        .to_string();
    let mpc_contract = payload
        .get("mpc_contract")
        .and_then(|x| x.as_str())
        .ok_or_else(|| "missing mpc_contract".to_string())?
        .to_string();
    let unlocked = payload
        .get("unlocked")
        .and_then(|x| x.as_bool())
        .ok_or_else(|| "missing unlocked".to_string())?;
    let recovery_in_progress = payload
        .get("recovery")
        .map(|r| !r.is_null())
        .unwrap_or(false);
    Ok(VaultStateView {
        keystore_dao,
        mpc_contract,
        unlocked,
        recovery_in_progress,
    })
}

