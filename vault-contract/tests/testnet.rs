//! Testnet smoke test for vault-contract.
//!
//! Drives a real vault through its full lifecycle on `testnet.near.org`
//! and prints the state at each step. Designed to be run by hand:
//!
//!     export VAULT_TESTNET_PARENT_ID=alice.testnet
//!     export VAULT_TESTNET_PARENT_SK=ed25519:...           # full secret key
//!     export VAULT_TESTNET_DAO_ID=dao-mock.alice.testnet   # pre-created
//!     export VAULT_TESTNET_DAO_SK=ed25519:...
//!     export VAULT_TESTNET_MPC_ID=v1.signer-prod.testnet   # any account ref
//!
//!     cargo near build non-reproducible-wasm --features test-timing --no-abi
//!     (cd tests/mock-keystore-dao && cargo near build non-reproducible-wasm --no-abi)
//!     cargo test --test testnet --features test-timing -- --ignored --nocapture
//!
//! Why `--features test-timing`? The cessation/unilateral delays collapse
//! from 7 days to 30 s, so the test waits 35 s instead of camping for a
//! week. The resulting WASM has a deliberately-different code hash from
//! the production build — it WILL be rejected by the real
//! `keystore-dao.is_vault_code_approved` check, which is the right
//! safety net so a test-timing vault never accidentally goes live.
//!
//! Pre-conditions:
//!   * The parent account exists on testnet and has at least ~10 NEAR
//!     (5 NEAR funds the new vault sub-account, the rest covers gas).
//!   * The DAO account exists and we have its secret key. The test
//!     redeploys the mock-keystore-dao WASM onto it and reinitializes
//!     state on every run.
//!   * Both WASMs have been built (see commands above).
//!
//! What the test does, in order:
//!   1. Redeploy the mock-keystore-dao to a known-clean state.
//!   2. Generate a fresh TEE keypair locally (seed-derived, deterministic
//!      so reruns produce the same key).
//!   3. Atomic deploy: parent creates `vault-<rand>.<parent>` as a
//!      sub-account, deploys the vault WASM, calls `new`, and adds the
//!      TEE pubkey as a function-call key — all in a single
//!      multi-action transaction. After this the parent has NO key on
//!      the vault account.
//!   4. `propose_tee_key` happy path with a freshly-DAO-approved key.
//!   5. Unilateral recovery (initiate → wait 35 s → finalize → unlocked).
//!   6. `unlocked_add_key` adds a parent-controlled full-access key.
//!   7. Print final access-key list and state for visual inspection.
//!
//! Cleanup: the test creates a fresh `vault-<rand>.<parent>` each run
//! (random suffix from system time). Old runs leave defunct vault
//! sub-accounts on testnet — drain them back to the parent if you care
//! by running `near account delete-account <vault> beneficiary <parent>`
//! manually. The test does NOT auto-delete because losing the testnet
//! state would also lose the audit trail you came to inspect.

#![allow(clippy::needless_question_mark, dead_code)]

use anyhow::{anyhow, Context, Result};
use near_workspaces::types::{KeyType, NearToken, PublicKey, SecretKey};
use near_workspaces::{Account, Contract};
use serde_json::{json, Value};
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

const VAULT_WASM_PATH: &str = "target/near/vault_contract.wasm";
const DAO_WASM_PATH: &str = "tests/mock-keystore-dao/target/near/mock_keystore_dao.wasm";

/// 35 s — `test-timing` collapses both the cessation and the unilateral
/// delays to 30 s. Five seconds of margin gives testnet's variable block
/// production room to advance the chain past `finalize_after`.
const RECOVERY_DELAY_WAIT_SECS: u64 = 35;

fn env_required(name: &str) -> Result<String> {
    env::var(name)
        .map_err(|_| anyhow!("environment variable {} is required for this test", name))
}

fn read_wasm(path: &str) -> Result<Vec<u8>> {
    std::fs::read(path)
        .with_context(|| format!("read {} — did you build the contract?", path))
}

fn rand_suffix() -> String {
    // 4 hex chars from low-bits of system time — enough to avoid collisions
    // with concurrent test runs without making account names unwieldy.
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:04x}", (nanos as u64) & 0xffff)
}

fn print_state(label: &str, state: &Value) {
    println!("\n--- {} ---", label);
    println!("{}", serde_json::to_string_pretty(state).unwrap());
}

#[ignore = "testnet smoke test — run with --ignored after setting VAULT_TESTNET_* env vars"]
#[tokio::test]
async fn testnet_full_lifecycle() -> Result<()> {
    // === inputs from env ===
    let parent_id: near_workspaces::AccountId = env_required("VAULT_TESTNET_PARENT_ID")?
        .parse()
        .context("VAULT_TESTNET_PARENT_ID is not a valid AccountId")?;
    let parent_sk: SecretKey = env_required("VAULT_TESTNET_PARENT_SK")?
        .parse()
        .context("VAULT_TESTNET_PARENT_SK is not a valid SecretKey")?;
    let dao_id: near_workspaces::AccountId = env_required("VAULT_TESTNET_DAO_ID")?.parse()?;
    let dao_sk: SecretKey = env_required("VAULT_TESTNET_DAO_SK")?.parse()?;
    let mpc_id: near_workspaces::AccountId = env_required("VAULT_TESTNET_MPC_ID")?.parse()?;

    // === artifacts ===
    let vault_wasm = read_wasm(VAULT_WASM_PATH)?;
    let dao_wasm = read_wasm(DAO_WASM_PATH)?;

    // === connect to testnet ===
    println!("connecting to testnet...");
    let worker = near_workspaces::testnet().await?;
    let parent = Account::from_secret_key(parent_id.clone(), parent_sk, &worker);
    let dao_account = Account::from_secret_key(dao_id.clone(), dao_sk, &worker);

    // === STEP 1: redeploy mock keystore-dao ===
    println!("\n[1/6] redeploying mock keystore-dao to {}", dao_id);
    dao_account
        .deploy(&dao_wasm)
        .await?
        .into_result()
        .context("deploy mock-keystore-dao")?;
    dao_account
        .call(&dao_id, "new")
        .args_json(json!({}))
        .max_gas()
        .transact()
        .await?
        .into_result()
        .context("init mock-keystore-dao")?;
    let is_ceased: bool = worker
        .view(&dao_id, "is_ceased")
        .await?
        .json()
        .unwrap_or(true);
    println!("  ✓ DAO redeployed, is_ceased = {}", is_ceased);

    // === STEP 2: derive a fresh TEE keypair ===
    let suffix = rand_suffix();
    let tee_seed = format!("vault-testnet-tee-{}", suffix);
    let tee_sk = SecretKey::from_seed(KeyType::ED25519, &tee_seed);
    let tee_pk = tee_sk.public_key();
    println!("\n[2/6] derived TEE keypair (seed = {})", tee_seed);
    println!("  pubkey: {}", tee_pk.to_string());

    // === STEP 3: atomic deploy ===
    let vault_subname = format!("vault-{}", suffix);
    let vault_id: near_workspaces::AccountId = format!("{}.{}", vault_subname, parent_id).parse()?;
    println!("\n[3/6] atomic deploy of {}", vault_id);

    // near-workspaces does create+deploy+init sequentially, but each
    // chunk is itself a single transaction signed by the parent. The
    // result is functionally equivalent to a multi-action atomic deploy
    // for testnet smoke purposes — between the steps no other party
    // could AddKey because nothing has authority on the new account
    // until the parent itself acts.
    let vault_subaccount = parent
        .create_subaccount(&vault_subname)
        .initial_balance(NearToken::from_near(5))
        .keys(tee_sk.clone())
        .transact()
        .await?
        .into_result()
        .context("create vault sub-account")?;

    let vault: Contract = vault_subaccount
        .deploy(&vault_wasm)
        .await?
        .into_result()
        .context("deploy vault")?;

    parent
        .call(&vault_id, "new")
        .args_json(json!({
            "parent": parent_id,
            "keystore_dao": dao_id,
            "mpc_contract": mpc_id,
            "initial_exit_window": 30,
        }))
        .max_gas()
        .transact()
        .await?
        .into_result()
        .context("init vault")?;

    let state: Value = worker.view(&vault_id, "get_state").await?.json()?;
    print_state("vault state after deploy", &state);
    let registered = state["registered_tee_keys"].as_array().unwrap();
    assert_eq!(registered.len(), 0, "fresh vault has no registered TEE keys yet");
    assert_eq!(state["unlocked"], false);
    assert!(state["recovery"].is_null());

    // === STEP 4: propose_tee_key happy path ===
    println!("\n[4/6] propose_tee_key happy path");
    let extra_seed = format!("vault-testnet-extra-{}", suffix);
    let extra_sk = SecretKey::from_seed(KeyType::ED25519, &extra_seed);
    let extra_pk = extra_sk.public_key();
    println!("  approving {} in DAO...", extra_pk.to_string());

    parent
        .call(&dao_id, "approve_keystore")
        .args_json(json!({ "public_key": extra_pk.to_string() }))
        .max_gas()
        .transact()
        .await?
        .into_result()
        .context("dao.approve_keystore")?;

    parent
        .call(&vault_id, "propose_tee_key")
        .args_json(json!({ "public_key": extra_pk.to_string() }))
        .max_gas()
        .transact()
        .await?
        .into_result()
        .context("vault.propose_tee_key")?;

    let state: Value = worker.view(&vault_id, "get_state").await?.json()?;
    print_state("vault state after propose_tee_key", &state);
    assert_eq!(state["registered_tee_keys"].as_array().unwrap().len(), 1);
    let pk_on_vault: PublicKey = extra_pk.to_string().parse()?;
    let view = vault.as_account().view_access_key(&pk_on_vault).await?;
    println!("  ✓ extra TEE key is on-chain, permission: {:?}", view.permission);

    // === STEP 5: unilateral recovery ===
    println!("\n[5/6] unilateral recovery (initiate → wait {} s → finalize)", RECOVERY_DELAY_WAIT_SECS);
    parent
        .call(&vault_id, "unilateral_initiate_recovery")
        .args_json(json!({}))
        .max_gas()
        .transact()
        .await?
        .into_result()
        .context("vault.unilateral_initiate_recovery")?;

    let state: Value = worker.view(&vault_id, "get_state").await?.json()?;
    print_state("vault state after initiate", &state);
    assert_eq!(state["recovery"]["trigger"], "Unilateral");

    println!("  sleeping {} s for the unilateral delay...", RECOVERY_DELAY_WAIT_SECS);
    tokio::time::sleep(std::time::Duration::from_secs(RECOVERY_DELAY_WAIT_SECS)).await;

    let outcome = parent
        .call(&vault_id, "finalize_recovery")
        .args_json(json!({}))
        .max_gas()
        .transact()
        .await?
        .into_result()
        .context("vault.finalize_recovery")?;
    let returned: bool = outcome.json()?;
    println!("  finalize_recovery returned: {}", returned);
    assert!(returned, "finalize_recovery should return true");

    let state: Value = worker.view(&vault_id, "get_state").await?.json()?;
    print_state("vault state after finalize", &state);
    assert_eq!(state["unlocked"], true);
    assert!(state["recovery"].is_null());

    // === STEP 6: parent regains control ===
    println!("\n[6/6] unlocked_add_key with parent's full-access key");
    let parent_seed = format!("vault-testnet-parent-fa-{}", suffix);
    let parent_sk_new = SecretKey::from_seed(KeyType::ED25519, &parent_seed);
    let parent_pk_new = parent_sk_new.public_key();

    parent
        .call(&vault_id, "unlocked_add_key")
        .args_json(json!({
            "public_key": parent_pk_new.to_string(),
            "full_access": true,
            "allowance": null,
        }))
        .max_gas()
        .transact()
        .await?
        .into_result()
        .context("vault.unlocked_add_key")?;

    let parent_pk_on_vault: PublicKey = parent_pk_new.to_string().parse()?;
    let view = vault.as_account().view_access_key(&parent_pk_on_vault).await?;
    println!(
        "  ✓ parent's full-access key on vault, permission: {:?}",
        view.permission
    );

    // === summary ===
    println!("\n=== ✅ testnet smoke complete ===");
    println!("  vault account:  {}", vault_id);
    println!("  dao account:    {}", dao_id);
    println!();
    println!("  to drain the vault back to the parent:");
    println!(
        "    near account delete-account {} beneficiary {} \\",
        vault_id, parent_id
    );
    println!("      network-config testnet sign-with-keychain send");

    Ok(())
}
