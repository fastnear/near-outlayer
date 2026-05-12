//! customer-recovery — standalone CKD recovery for an unlocked vault.
//!
//! After `finalize_recovery` flips a vault to `unlocked = true`, the
//! parent has FullAccess on the vault account. OutLayer's keystore
//! refuses to serve the per-vault master from then on (defense-in-depth
//! gate in `keystore-worker/src/mpc_ckd.rs::ensure_customer_loaded`).
//!
//! The customer recovers the master themselves by signing a fresh
//! `request_app_private_key` call to the MPC contract directly,
//! attaching 1 yoctoNEAR (FullAccess can attach deposit; the previous
//! TEE function-call key could not — that's why the vault contract
//! had a `request_master` proxy).
//!
//! With the master in hand, the customer can deterministically
//! re-derive every wallet keypair the keystore would have produced
//! (HMAC-SHA256(master, "wallet:<seed>")), so addresses persist
//! across the sovereignty handover.
//!
//! ## Usage
//!
//! ```bash
//! cargo run --release -- \
//!     --vault-id vault.alice.testnet \
//!     --signer-private-key ed25519:5m... \
//!     --derivation-path '<HMAC-derived hex from past on-chain tx>' \
//!     --mpc-contract v1.signer-prod.testnet \
//!     --mpc-public-key 'bls12381g2:...' \
//!     --mpc-domain-id 2 \
//!     --rpc-url https://rpc.testnet.fastnear.com
//! ```
//!
//! ## How to find `derivation_path`
//!
//! The keystore-worker computed it as
//! `HMAC-SHA256(default_master, "vault-master:<vault_id>")` and then
//! used it as a CKD argument. After the first request_app_private_key
//! call landed on chain, the path is publicly visible in the tx args.
//!
//! Easiest way: query NEARblocks for the vault's tx history, find the
//! `request_app_private_key` receipt to MPC, decode its args (base64
//! JSON), and copy the `derivation_path` field. The `--from-chain`
//! flag in this tool does that automatically when set.

use anyhow::{anyhow, Context, Result};
use blstrs::{G1Affine, G1Projective, G2Affine, G2Projective, Scalar};
use clap::Parser;
use elliptic_curve::{group::prime::PrimeCurveAffine, Field, Group};
use hkdf::Hkdf;
use near_crypto::{InMemorySigner, SecretKey};
use near_jsonrpc_client::{methods, JsonRpcClient};
use near_jsonrpc_primitives::types::query::QueryResponseKind;
use near_primitives::{
    transaction::{Action, FunctionCallAction, Transaction, TransactionV0},
    types::{AccountId, BlockReference, Finality},
    views::{FinalExecutionStatus, QueryRequest},
};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use sha3::{Digest, Sha3_256};

const BLS12381G1_PUBLIC_KEY_SIZE: usize = 48;
const OUTPUT_SECRET_SIZE: usize = 32;
const APP_ID_DERIVATION_PREFIX: &str = "near-mpc v0.1.0 app_id derivation:";
const NEAR_CKD_DOMAIN: &[u8] = b"NEAR BLS12381G1_XMD:SHA-256_SSWU_RO_";

#[derive(Parser, Debug)]
#[command(
    name = "customer-recovery",
    about = "Recover a vault's per-customer master after finalize_recovery"
)]
struct Cli {
    /// Vault account id (e.g. `vault.alice.testnet`). Used as the
    /// signer of the MPC tx — its FullAccess key must already be on
    /// chain (that's what `unlocked_add_key --full-access` produces).
    #[arg(long)]
    vault_id: AccountId,

    /// Vault's FullAccess private key in `ed25519:base58` form.
    #[arg(long, env = "VAULT_PRIVATE_KEY")]
    signer_private_key: String,

    /// Derivation path the keystore used. Either pass it directly here
    /// or use `--from-chain` to auto-extract it from the vault's tx
    /// history.
    #[arg(long)]
    derivation_path: Option<String>,

    /// Auto-extract derivation_path from on-chain tx history. Calls
    /// NEARblocks API to find the most recent successful
    /// `request_app_private_key` receipt to MPC and decodes its args.
    #[arg(long)]
    from_chain: bool,

    /// MPC contract account id.
    #[arg(long, default_value = "v1.signer-prod.testnet")]
    mpc_contract: AccountId,

    /// MPC G2 public key in `bls12381g2:base58` form. Same value the
    /// keystore-worker uses (`MPC_PUBLIC_KEY` env var).
    #[arg(long, env = "MPC_PUBLIC_KEY")]
    mpc_public_key: String,

    /// MPC domain id (2 for testnet per current keystore config).
    #[arg(long, default_value = "2")]
    mpc_domain_id: u64,

    /// NEAR RPC URL.
    #[arg(long, default_value = "https://rpc.testnet.fastnear.com")]
    rpc_url: String,

    /// NEARblocks-compatible API base for `--from-chain` lookup.
    #[arg(long, default_value = "https://api-testnet.nearblocks.io")]
    nearblocks_url: String,
}

#[derive(Debug, Serialize)]
struct CkdRequestArgs {
    request: CkdArgs,
}

#[derive(Debug, Serialize)]
struct CkdArgs {
    derivation_path: String,
    app_public_key: String,
    domain_id: u64,
}

#[derive(Debug, Deserialize)]
struct CkdResponse {
    big_y: String,
    big_c: String,
}

/// `customer-recovery generate-key` — emit a fresh ed25519 keypair
/// as JSON to stdout and exit. Used by `walkthrough.sh` step 1 so
/// the user doesn't need an external NEAR keygen tool installed.
/// The output shape matches what the rest of the walkthrough
/// expects: `{public_key: "ed25519:...", private_key: "ed25519:..."}`
/// — same format `near-cli-rs`'s key files use, so other NEAR tools
/// can read it.
fn generate_key_subcommand() -> Result<()> {
    let sk = SecretKey::from_random(near_crypto::KeyType::ED25519);
    let pk = sk.public_key();
    let json = serde_json::json!({
        "public_key": pk.to_string(),
        "private_key": sk.to_string(),
    });
    println!("{}", serde_json::to_string_pretty(&json)?);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Subcommand bypass: avoids restructuring clap for a single
    // 30-line side path. If we ever grow more subcommands, switch
    // to clap's Subcommand enum.
    let argv: Vec<String> = std::env::args().collect();
    if argv.len() == 2 && argv[1] == "generate-key" {
        return generate_key_subcommand();
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();

    let secret_key: SecretKey = cli
        .signer_private_key
        .parse()
        .context("Invalid --signer-private-key (expected ed25519:base58)")?;
    let signer = InMemorySigner::from_secret_key(cli.vault_id.clone(), secret_key);

    let derivation_path = if cli.from_chain {
        let path = fetch_derivation_path_from_chain(&cli.nearblocks_url, &cli.vault_id)
            .await
            .context(
                "could not auto-discover derivation_path from chain — pass --derivation-path manually",
            )?;
        tracing::info!(path = %path, "Discovered derivation_path on chain");
        path
    } else {
        cli.derivation_path
            .clone()
            .ok_or_else(|| anyhow!("either --derivation-path or --from-chain is required"))?
    };

    // 1. Generate ephemeral BLS12-381 G1 keypair (one-shot per request).
    let mut rng = OsRng;
    let ephemeral_private = Scalar::random(&mut rng);
    let ephemeral_public = G1Projective::generator() * ephemeral_private;
    let app_public_key = format!(
        "bls12381g1:{}",
        bs58::encode(&ephemeral_public.to_compressed()).into_string()
    );
    tracing::info!(app_public_key = %app_public_key, "Generated ephemeral BLS keypair");

    // 2. Build CKD request args.
    let args = CkdRequestArgs {
        request: CkdArgs {
            derivation_path: derivation_path.clone(),
            app_public_key: app_public_key.clone(),
            domain_id: cli.mpc_domain_id,
        },
    };

    // 3. Submit request_app_private_key tx with 1 yocto deposit.
    let client = JsonRpcClient::connect(&cli.rpc_url);
    let response =
        submit_mpc_call(&client, &signer, &cli.mpc_contract, &args).await?;

    // 4. Recompute app_id locally (must match MPC's hash).
    let app_id = derive_app_id(cli.vault_id.as_str(), &derivation_path);

    // 5. Decrypt + verify pairing.
    let mpc_g2 = parse_g2(&cli.mpc_public_key)?;
    let secret =
        decrypt_and_verify(response, ephemeral_private, &mpc_g2, &app_id)?;

    // 6. HKDF-stretch 48-byte G1 secret to 32-byte master.
    let master = derive_strong_key(secret)?;

    // 7. Print the master in hex (this is THE secret — protect it).
    println!();
    println!("# === Per-vault master recovered ===");
    println!("# vault_id       = {}", cli.vault_id);
    println!("# derivation_path= {}", derivation_path);
    println!();
    println!("master_hex={}", hex::encode(master));
    println!();
    println!(
        "# Now derive any wallet keypair: HMAC-SHA256(master, b\"wallet:<seed>\")"
    );
    println!(
        "# E.g. NEAR address: ed25519::SigningKey::from_bytes(&hmac_output[..32])"
    );

    Ok(())
}

async fn submit_mpc_call(
    client: &JsonRpcClient,
    signer: &InMemorySigner,
    mpc_contract: &AccountId,
    args: &CkdRequestArgs,
) -> Result<CkdResponse> {
    // Get nonce + recent block.
    let access_key_query = methods::query::RpcQueryRequest {
        block_reference: BlockReference::Finality(Finality::Final),
        request: QueryRequest::ViewAccessKey {
            account_id: signer.account_id.clone(),
            public_key: signer.public_key.clone(),
        },
    };
    let access_key_view = match client.call(access_key_query).await?.kind {
        QueryResponseKind::AccessKey(v) => v,
        other => anyhow::bail!("unexpected access-key response: {other:?}"),
    };
    let nonce = access_key_view.nonce + 1;
    let block = client
        .call(methods::block::RpcBlockRequest {
            block_reference: BlockReference::Finality(Finality::Final),
        })
        .await?;

    let serialized = serde_json::to_vec(&args)?;
    tracing::info!(
        deposit_yocto = 1u128,
        gas_tgas = 150,
        receiver = %mpc_contract,
        "Submitting request_app_private_key (FullAccess can attach deposit)"
    );

    let tx = TransactionV0 {
        signer_id: signer.account_id.clone(),
        public_key: signer.public_key.clone(),
        nonce,
        receiver_id: mpc_contract.clone(),
        block_hash: block.header.hash,
        actions: vec![Action::FunctionCall(Box::new(FunctionCallAction {
            method_name: "request_app_private_key".to_string(),
            args: serialized,
            gas: 150_000_000_000_000,
            deposit: 1, // assert_one_yocto
        }))],
    };
    let signed = near_primitives::transaction::SignedTransaction::new(
        signer.sign(Transaction::V0(tx.clone()).get_hash_and_size().0.as_ref()),
        Transaction::V0(tx),
    );
    let outcome = client
        .call(methods::broadcast_tx_commit::RpcBroadcastTxCommitRequest {
            signed_transaction: signed,
        })
        .await
        .context("broadcast_tx_commit")?;

    match outcome.status {
        FinalExecutionStatus::SuccessValue(value) => {
            let resp: CkdResponse = serde_json::from_slice(&value)
                .context("parse MPC CkdResponse")?;
            tracing::info!("✅ MPC returned encrypted CKD payload");
            Ok(resp)
        }
        FinalExecutionStatus::Failure(err) => {
            anyhow::bail!("MPC tx failed: {err:?}")
        }
        other => anyhow::bail!("unexpected outer status: {other:?}"),
    }
}

fn derive_app_id(predecessor_id: &str, derivation_path: &str) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    hasher.update(
        format!(
            "{}{},{}",
            APP_ID_DERIVATION_PREFIX, predecessor_id, derivation_path
        )
        .as_bytes(),
    );
    hasher.finalize().into()
}

fn parse_g1(s: &str) -> Result<G1Projective> {
    let b58 = s
        .strip_prefix("bls12381g1:")
        .ok_or_else(|| anyhow!("expected bls12381g1: prefix in {s}"))?;
    let bytes = bs58::decode(b58).into_vec()?;
    let mut compressed = [0u8; BLS12381G1_PUBLIC_KEY_SIZE];
    compressed.copy_from_slice(&bytes[..BLS12381G1_PUBLIC_KEY_SIZE]);
    G1Projective::from_compressed(&compressed)
        .into_option()
        .ok_or_else(|| anyhow!("invalid G1 point"))
}

fn parse_g2(s: &str) -> Result<G2Projective> {
    let b58 = s
        .strip_prefix("bls12381g2:")
        .ok_or_else(|| anyhow!("expected bls12381g2: prefix in {s}"))?;
    let bytes = bs58::decode(b58).into_vec()?;
    let mut compressed = [0u8; 96];
    compressed.copy_from_slice(&bytes[..96]);
    G2Projective::from_compressed(&compressed)
        .into_option()
        .ok_or_else(|| anyhow!("invalid G2 point"))
}

fn decrypt_and_verify(
    response: CkdResponse,
    private_key: Scalar,
    mpc_pub: &G2Projective,
    app_id: &[u8],
) -> Result<[u8; BLS12381G1_PUBLIC_KEY_SIZE]> {
    let big_y = parse_g1(&response.big_y)?;
    let big_c = parse_g1(&response.big_c)?;

    // secret = big_c - big_y * private_key
    let secret = big_c - big_y * private_key;

    // Pairing verification (matches MPC's signature scheme).
    let element1: G1Affine = secret.into();
    if (!element1.is_on_curve() | !element1.is_torsion_free() | element1.is_identity()).into() {
        anyhow::bail!("decrypted secret point invalid");
    }
    let element2: G2Affine = (*mpc_pub).into();
    if (!element2.is_on_curve() | !element2.is_torsion_free() | element2.is_identity()).into() {
        anyhow::bail!("MPC pubkey invalid");
    }

    let hash_input = [mpc_pub.to_compressed().as_slice(), app_id].concat();
    let base1 = G1Projective::hash_to_curve(&hash_input, NEAR_CKD_DOMAIN, &[]).into();
    let base2 = G2Affine::generator();
    if blstrs::pairing(&base1, &element2) != blstrs::pairing(&element1, &base2) {
        anyhow::bail!("MPC signature pairing verification failed");
    }

    Ok(secret.to_compressed())
}

fn derive_strong_key(
    ikm: [u8; BLS12381G1_PUBLIC_KEY_SIZE],
) -> Result<[u8; OUTPUT_SECRET_SIZE]> {
    let hk = Hkdf::<Sha256>::new(None, &ikm);
    let mut okm = [0u8; OUTPUT_SECRET_SIZE];
    hk.expand(b"", &mut okm)
        .map_err(|e| anyhow!("HKDF expand: {e}"))?;
    Ok(okm)
}

/// Query NEARblocks for the most recent `request_app_private_key`
/// receipt FROM `vault_id` to MPC, decode its args, and pull out the
/// `derivation_path` field.
async fn fetch_derivation_path_from_chain(
    nearblocks_url: &str,
    vault_id: &AccountId,
) -> Result<String> {
    let url = format!(
        "{}/v1/account/{}/txns?per_page=20",
        nearblocks_url, vault_id
    );
    let body: serde_json::Value = reqwest::get(&url)
        .await
        .context("query NEARblocks")?
        .json()
        .await
        .context("parse NEARblocks JSON")?;

    let txns = body
        .get("txns")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("NEARblocks: no txns array in response"))?;

    for t in txns {
        if t.get("predecessor_account_id").and_then(|v| v.as_str()) != Some(vault_id.as_str()) {
            continue;
        }
        let actions = t.get("actions").and_then(|v| v.as_array());
        let Some(actions) = actions else { continue };
        for a in actions {
            // Both `request_master` (the vault's proxy method) and
            // direct `request_app_private_key` carry the same args
            // shape — `{ request: { derivation_path, app_public_key,
            // domain_id } }` — so accept either. NEARblocks' txns
            // endpoint surfaces direct txs to/from the account but
            // not always cross-contract receipts; the proxy's outer
            // tx (vault → vault) is reliably listed and has the same
            // payload.
            let method = a
                .get("method")
                .and_then(|m| m.get("method_name"))
                .and_then(|v| v.as_str());
            if method != Some("request_app_private_key") && method != Some("request_master") {
                continue;
            }
            let args_b64 = a
                .get("method")
                .and_then(|m| m.get("args"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("request_app_private_key receipt missing args"))?;
            let raw = base64::Engine::decode(
                &base64::engine::general_purpose::STANDARD,
                args_b64,
            )
            .context("decode args base64")?;
            let json: serde_json::Value =
                serde_json::from_slice(&raw).context("parse args JSON")?;
            let path = json
                .get("request")
                .and_then(|r| r.get("derivation_path"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("args missing request.derivation_path"))?;
            return Ok(path.to_string());
        }
    }

    Err(anyhow!(
        "no past request_app_private_key tx found for {} on NEARblocks — submit at least one CKD call before this tool can auto-discover the path, or pass --derivation-path explicitly",
        vault_id
    ))
}
