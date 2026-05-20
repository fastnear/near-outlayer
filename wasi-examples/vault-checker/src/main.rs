//! OutLayer vault-checker WASI agent.
//!
//! Off-chain verifies a customer vault account's state (code hash
//! whitelisted by the DAO, exactly one TEE function-call key, locked,
//! no recovery in flight) and instructs the keystore-worker to record
//! the verification on-chain.
//!
//! ## I/O contract
//!
//! Input (JSON on stdin):
//!
//! ```json
//! {
//!   "action": "verify",          // or "verify_status"
//!   "vault_id": "vault.alice.testnet"
//! }
//! ```
//!
//! Output (JSON on stdout):
//!
//! For `verify`:
//! ```json
//! {
//!   "verified": true,              // all state checks passed
//!   "reason": null,                // or string explanation when false
//!   "already_verified": false,     // true → DAO already records this
//!                                  //   vault as verified; agent
//!                                  //   short-circuited.
//!   "state_only": false,           // when KEYSTORE_BASE_URL +
//!                                  //   KEYSTORE_AUTH_TOKEN env vars
//!                                  //   are set, the agent forwards
//!                                  //   to keystore-worker's
//!                                  //   `/sign-vault-verification`
//!                                  //   after a local PASS — flips
//!                                  //   this to false + populates
//!                                  //   tx_hash. Without those env
//!                                  //   vars (legacy operator setup)
//!                                  //   the agent stays state-only
//!                                  //   and returns true here.
//!   "tx_hash": "..."               // present iff state_only = false
//!                                  //   AND a fresh `mark_vault_verified`
//!                                  //   tx was actually submitted.
//!                                  //   `None` when already_verified
//!                                  //   is true (idempotent skip).
//! }
//! ```
//!
//! For `verify_status`:
//! ```json
//! {
//!   "verified": true,              // = keystore_dao.is_vault_verified
//!   "reason": null
//! }
//! ```
//!
//! ## Configuration via env vars
//!
//! Injected by the OutLayer worker at execution time:
//!
//! * `KEYSTORE_DAO_ID` — e.g. `keystore-dao.outlayer.testnet`
//! * `MPC_CONTRACT_ID` — e.g. `v1.signer-prod.testnet`
//! * `KEYSTORE_BASE_URL` — internal URL of the keystore-worker
//!   (e.g. `https://keystore-abc123.phala.cloud`).
//! * `KEYSTORE_AUTH_TOKEN` — bearer token authorised on the keystore's
//!   `/sign-vault-verification` route (worker-token or coord-or-worker
//!   per its router).
//!
//! These are deployment-time constants for the deployed agent. Pinning
//! them in the agent's secrets / project config (rather than reading
//! from on-chain config) keeps the agent's trust surface narrow — it
//! cannot be fooled by a customer who points it at a fake DAO.

mod verify;
#[cfg(test)]
mod tests;

#[cfg(target_arch = "wasm32")]
wit_bindgen::generate!({
    world: "vault-checker",
    path: "wit",
});

#[cfg(target_arch = "wasm32")]
use verify::{check_already_verified, verify_vault, NearRpc, VerifierConfig};

#[cfg(target_arch = "wasm32")]
use serde::{Deserialize, Serialize};
#[cfg(target_arch = "wasm32")]
use std::io::{self, Read, Write};

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum Input {
    Verify { vault_id: String },
    VerifyStatus { vault_id: String },
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Serialize)]
#[serde(untagged)]
enum Output {
    Verify {
        /// All five on-chain state checks passed. **Does NOT mean the
        /// vault is on-chain-verified yet** — see `state_only`.
        verified: bool,
        reason: Option<String>,
        /// `true` ⇒ keystore-dao already had this vault in its
        /// `verified_vaults` set; the agent short-circuited.
        already_verified: bool,
        /// `true` ⇒ the agent only ran the state inspection and did
        /// NOT cause `mark_vault_verified` to land on-chain. Operators
        /// and downstream UX MUST treat `verified: true,
        /// state_only: true` as "passed local checks, on-chain mark
        /// still pending". Set to `false` once the keystore-worker
        /// HTTP forward succeeds.
        state_only: bool,
        /// Populated once `/sign-vault-verification` finalises. `None`
        /// in state-only mode.
        tx_hash: Option<String>,
    },
    Status {
        verified: bool,
        reason: Option<String>,
    },
}

#[cfg(target_arch = "wasm32")]
struct HostRpc;

#[cfg(target_arch = "wasm32")]
impl NearRpc for HostRpc {
    fn view(
        &self,
        contract_id: &str,
        method: &str,
        args_json: &str,
    ) -> Result<String, String> {
        let (result, error) = near::rpc::api::view(contract_id, method, args_json, "");
        if error.is_empty() { Ok(result) } else { Err(error) }
    }
    fn view_account(&self, account_id: &str) -> Result<String, String> {
        let (result, error) = near::rpc::api::view_account(account_id, "");
        if error.is_empty() { Ok(result) } else { Err(error) }
    }
    fn view_access_key_list(&self, account_id: &str) -> Result<String, String> {
        let (result, error) = near::rpc::api::view_access_key_list(account_id, "");
        if error.is_empty() { Ok(result) } else { Err(error) }
    }
}

#[cfg(target_arch = "wasm32")]
fn load_config() -> Result<VerifierConfig, String> {
    let keystore_dao = std::env::var("KEYSTORE_DAO_ID")
        .map_err(|_| "KEYSTORE_DAO_ID env var not set".to_string())?;
    let mpc_contract = std::env::var("MPC_CONTRACT_ID")
        .map_err(|_| "MPC_CONTRACT_ID env var not set".to_string())?;
    Ok(VerifierConfig {
        keystore_dao,
        mpc_contract,
    })
}

/// Optional sign-verification config. When BOTH `KEYSTORE_BASE_URL`
/// and `KEYSTORE_AUTH_TOKEN` are set, the agent will POST to
/// `/sign-vault-verification` after the local 5 checks pass and
/// surface the resulting tx_hash. When EITHER is missing, the agent
/// falls back to "state-only" behaviour and returns
/// `state_only: true, tx_hash: None`. This keeps the same binary
/// deployable in both setups (legacy state-only and HTTP-forward
/// mode) without a feature flag.
#[cfg(target_arch = "wasm32")]
struct SignConfig {
    keystore_base_url: String,
    auth_token: String,
}

#[cfg(target_arch = "wasm32")]
fn load_sign_config() -> Option<SignConfig> {
    let url = std::env::var("KEYSTORE_BASE_URL").ok()?;
    let token = std::env::var("KEYSTORE_AUTH_TOKEN").ok()?;
    if url.is_empty() || token.is_empty() {
        return None;
    }
    Some(SignConfig {
        keystore_base_url: url,
        auth_token: token,
    })
}

/// POST `vault_id` to keystore-worker `/sign-vault-verification` with
/// bearer auth. Returns `(tx_hash, already_verified)` on success.
///
/// The keystore RE-RUNS the same 5 RPC checks in-process before
/// signing — this defence-in-depth means the only authority a
/// caller of the WASI agent gains is "trigger a re-verification of a
/// vault that already passes all on-chain checks". A wrong answer
/// from this agent cannot mark a non-conforming vault verified.
#[cfg(target_arch = "wasm32")]
fn forward_sign_verification(
    sign: &SignConfig,
    vault_id: &str,
) -> Result<(Option<String>, bool), String> {
    let body = serde_json::json!({ "vault_id": vault_id });
    let body_bytes = serde_json::to_vec(&body)
        .map_err(|e| format!("failed to serialize body: {e}"))?;

    // No explicit timeout: `wasi-http-client = "0.2"` only exposes
    // `connect_timeout`, which bounds the TCP handshake (typically
    // <100 ms) — NOT the response wait. The keystore's
    // `broadcast_tx_commit` can take 10-20s on testnet, so any
    // `connect_timeout` value gave a misleading impression of
    // bounded latency. The agent's overall execution time is bounded
    // by the worker's outer WASI execution-time budget, which is the
    // actual upper bound. If the forward times out at the worker
    // level, `run_verify`'s fallback re-checks `is_vault_verified`
    // to recover the canonical state.
    let url = format!("{}/sign-vault-verification", sign.keystore_base_url);
    let auth = format!("Bearer {}", sign.auth_token);
    let response = wasi_http_client::Client::new()
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Authorization", auth.as_str())
        .body(&body_bytes)
        .send()
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    let status = response.status();
    let resp_body = response.body().map_err(|e| format!("read body: {e}"))?;
    if status != 200 {
        let body_str = String::from_utf8_lossy(&resp_body);
        return Err(format!(
            "/sign-vault-verification returned {status}: {body_str}"
        ));
    }

    #[derive(serde::Deserialize)]
    struct Resp {
        tx_hash: Option<String>,
        already_verified: bool,
    }
    let parsed: Resp = serde_json::from_slice(&resp_body)
        .map_err(|e| format!("parse keystore response: {e}"))?;
    Ok((parsed.tx_hash, parsed.already_verified))
}

#[cfg(target_arch = "wasm32")]
fn main() {
    let mut input = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut input) {
        emit_error(&format!("read stdin: {e}"));
        return;
    }

    let parsed: Input = match serde_json::from_str(input.trim()) {
        Ok(v) => v,
        Err(e) => {
            emit_error(&format!("parse input: {e}"));
            return;
        }
    };
    let cfg = match load_config() {
        Ok(c) => c,
        Err(e) => {
            emit_error(&e);
            return;
        }
    };
    let rpc = HostRpc;
    let output = match parsed {
        Input::Verify { vault_id } => run_verify(&rpc, &cfg, &vault_id),
        Input::VerifyStatus { vault_id } => run_verify_status(&rpc, &cfg, &vault_id),
    };
    emit(&output);
}

// Non-wasm entry exists so `cargo test` can compile main.rs without
// pulling in wit-bindgen / the WASI host.
#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("vault-checker is a WASI module — build with wasm32-wasip2 and run inside an OutLayer worker");
}

#[cfg(target_arch = "wasm32")]
fn run_verify(rpc: &impl NearRpc, cfg: &VerifierConfig, vault_id: &str) -> Output {
    let sign_cfg = load_sign_config();

    match check_already_verified(rpc, cfg, vault_id) {
        Ok(true) => {
            return Output::Verify {
                verified: true,
                reason: None,
                already_verified: true,
                state_only: false,
                tx_hash: None,
            };
        }
        Ok(false) => {}
        Err(e) => {
            // State check itself failed — surface and bail. Sign
            // forwarding is irrelevant when we don't even know the
            // current verification status.
            return Output::Verify {
                verified: false,
                reason: Some(format!("{e}")),
                already_verified: false,
                state_only: sign_cfg.is_none(),
                tx_hash: None,
            };
        }
    }
    match verify_vault(rpc, cfg, vault_id) {
        Ok(()) => {
            // Local 5-check PASS. Forward to keystore-worker
            // `/sign-vault-verification` if configured. Without
            // sign-config we keep the state-only contract for
            // back-compat with operators who haven't yet wired
            // KEYSTORE_BASE_URL / KEYSTORE_AUTH_TOKEN.
            match sign_cfg {
                Some(sign) => match forward_sign_verification(&sign, vault_id) {
                    Ok((tx_hash, already_verified)) => Output::Verify {
                        verified: true,
                        reason: None,
                        already_verified,
                        state_only: false,
                        tx_hash,
                    },
                    Err(forward_err) => {
                        // The keystore's `broadcast_tx_commit` may
                        // have committed even if our HTTP wait timed
                        // out / dropped. Re-check `is_vault_verified`
                        // on chain — if it flipped to `true`, the tx
                        // landed and we should report success rather
                        // than misleading the caller into a retry
                        // storm.
                        match check_already_verified(rpc, cfg, vault_id) {
                            Ok(true) => Output::Verify {
                                verified: true,
                                reason: Some(format!(
                                    "sign-verification forward failed but on-chain state is verified: {forward_err}"
                                )),
                                already_verified: true,
                                state_only: false,
                                // tx_hash is unrecoverable through
                                // this path; the caller can find it
                                // by querying the keystore's tx log
                                // for the vault_id.
                                tx_hash: None,
                            },
                            _ => Output::Verify {
                                verified: false,
                                reason: Some(format!(
                                    "local checks passed but sign-verification forward failed: {forward_err}"
                                )),
                                already_verified: false,
                                state_only: false,
                                tx_hash: None,
                            },
                        }
                    }
                },
                None => Output::Verify {
                    verified: true,
                    reason: None,
                    already_verified: false,
                    state_only: true,
                    tx_hash: None,
                },
            }
        }
        Err(e) => Output::Verify {
            verified: false,
            reason: Some(format!("{e}")),
            already_verified: false,
            state_only: sign_cfg.is_none(),
            tx_hash: None,
        },
    }
}

#[cfg(target_arch = "wasm32")]
fn run_verify_status(rpc: &impl NearRpc, cfg: &VerifierConfig, vault_id: &str) -> Output {
    match check_already_verified(rpc, cfg, vault_id) {
        Ok(verified) => Output::Status {
            verified,
            reason: None,
        },
        Err(e) => Output::Status {
            verified: false,
            reason: Some(format!("{e}")),
        },
    }
}

#[cfg(target_arch = "wasm32")]
fn emit(output: &Output) {
    let serialized = serde_json::to_string(output).unwrap_or_else(|_| "{}".to_string());
    let _ = io::stdout().write_all(serialized.as_bytes());
    let _ = io::stdout().flush();
}

#[cfg(target_arch = "wasm32")]
fn emit_error(msg: &str) {
    let serialized = serde_json::to_string(&serde_json::json!({
        "verified": false,
        "reason": msg,
        "already_verified": false,
        "state_only": true,
        "tx_hash": null,
    }))
    .unwrap_or_default();
    let _ = io::stdout().write_all(serialized.as_bytes());
    let _ = io::stdout().flush();
}

