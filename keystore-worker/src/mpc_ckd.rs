//! NEAR MPC Chain Key Derivation (CKD) integration for deterministic master key generation
//!
//! This module implements deterministic master key generation inside TEE using NEAR MPC network.
//! The master key is derived deterministically and can only be generated inside a verified TEE.
//!
//! Flow:
//! 1. TEE boots and generates/loads a NEAR keypair
//! 2. TEE generates attestation evidence with its public key
//! 3. TEE submits registration to DAO contract
//! 4. DAO members vote to approve the keystore
//! 5. After approval, TEE requests secret from MPC network using CKD
//! 6. MPC network derives and returns deterministic secret
//! 7. TEE uses this secret as master key for all keystore operations

use anyhow::{Context, Result};
use blstrs::{G1Affine, G1Projective, G2Affine, G2Projective, Scalar};
use elliptic_curve::{Field as _, Group as _, group::prime::PrimeCurveAffine as _};
use hkdf::Hkdf;
use near_jsonrpc_client::{methods, JsonRpcClient};
use near_jsonrpc_primitives::types::query::QueryResponseKind;
use near_primitives::transaction::{Action, FunctionCallAction, Transaction, TransactionV0};
use near_primitives::types::{BlockReference, Finality, FunctionArgs};
use near_primitives::views::{QueryRequest, CallResult, FinalExecutionStatus};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use sha3::{Digest, Sha3_256};

// Constants from Bowen's example
const BLS12381G1_PUBLIC_KEY_SIZE: usize = 48;
const NEAR_CKD_DOMAIN: &[u8] = b"NEAR BLS12381G1_XMD:SHA-256_SSWU_RO_";
const OUTPUT_SECRET_SIZE: usize = 32;

// MPC app_id derivation prefix (must match MPC contract exactly)
const APP_ID_DERIVATION_PREFIX: &str = "near-mpc v0.1.0 app_id derivation:";

/// Derive app_id the same way MPC contract does
/// app_id = SHA3-256("{prefix}{predecessor_id},{derivation_path}")
fn derive_app_id(predecessor_id: &str, derivation_path: &str) -> [u8; 32] {
    let derivation_string = format!("{}{},{}", APP_ID_DERIVATION_PREFIX, predecessor_id, derivation_path);
    let mut hasher = Sha3_256::new();
    hasher.update(derivation_string.as_bytes());
    hasher.finalize().into()
}

/// MPC CKD configuration from environment
#[derive(Debug, Clone)]
pub struct MpcCkdConfig {
    /// MPC contract ID (e.g., "v1.signer.testnet")
    pub mpc_contract_id: String,

    /// MPC domain ID for BLS12-381 (usually 2)
    pub mpc_domain_id: u64,

    /// MPC public key for the domain (BLS12-381 G2)
    pub mpc_public_key: String,

    /// NEAR RPC URL
    pub near_rpc_url: String,

    /// Keystore DAO contract ID
    pub keystore_dao_contract: String,
}

impl MpcCkdConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            // CRITICAL: No fallback - must be explicitly configured
            mpc_contract_id: std::env::var("MPC_CONTRACT_ID")
                .context("MPC_CONTRACT_ID is required for MPC CKD")?,

            // CRITICAL: No fallback - wrong domain gives wrong keys
            mpc_domain_id: std::env::var("MPC_DOMAIN_ID")
                .unwrap_or_else(|_| "2".to_string())
                .parse()
                .context("Invalid MPC_DOMAIN_ID - must be a number")?,

            mpc_public_key: std::env::var("MPC_PUBLIC_KEY")
                .context("MPC_PUBLIC_KEY required for CKD")?,

            
            near_rpc_url: std::env::var("NEAR_RPC_URL")
                 .context("NEAR_RPC_URL is required for MPC CKD")?,
            
            keystore_dao_contract: std::env::var("KEYSTORE_DAO_CONTRACT")
                .context("KEYSTORE_DAO_CONTRACT is required for TEE registration")?,
        })
    }
}

/// CKD request arguments (matching MPC contract interface)
#[derive(Debug, Serialize, Deserialize)]
pub struct CkdRequestArgs {
    pub request: CkdArgs,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CkdArgs {
    pub derivation_path: String,  // Empty string "" for keystore master key
    pub app_public_key: String,   // BLS12-381 G1 public key in NEAR format
    pub domain_id: u64,
}

/// CKD response from MPC network
#[derive(Debug, Serialize, Deserialize)]
pub struct CkdResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheme: Option<String>,  // Optional field for compatibility
    pub big_y: String,  // BLS12-381 G1 point
    pub big_c: String,  // BLS12-381 G1 point
}

/// MPC CKD client for requesting deterministic secrets
pub struct MpcCkdClient {
    config: MpcCkdConfig,
    rpc_client: JsonRpcClient,
    signer: near_crypto::InMemorySigner,
}

impl MpcCkdClient {
    /// Create new MPC CKD client
    pub fn new(config: MpcCkdConfig, signer: near_crypto::InMemorySigner) -> Self {
        let rpc_client = JsonRpcClient::connect(&config.near_rpc_url);
        Self {
            config,
            rpc_client,
            signer,
        }
    }

    /// Request deterministic secret from MPC network using CKD
    ///
    /// This function:
    /// 1. Generates ephemeral BLS12-381 keypair
    /// 2. Calls MPC contract's request_app_private_key
    /// 3. Decrypts and verifies the response
    /// 4. Derives final 32-byte master secret using HKDF
    pub async fn request_master_secret(&self, signer_account_id: &str) -> Result<[u8; 32]> {
        tracing::info!("Requesting master secret from MPC network via CKD");
        tracing::info!("Account: {}, Domain: {}", signer_account_id, self.config.mpc_domain_id);

        // Generate ephemeral BLS12-381 G1 keypair
        let (ephemeral_private_key, ephemeral_public_key) = self.generate_ephemeral_key();

        // Convert public key to NEAR format
        let app_public_key = self.g1_to_near_format(ephemeral_public_key);

        tracing::debug!("Ephemeral public key: {}", app_public_key);

        // Create CKD request
        let request_args = CkdRequestArgs {
            request: CkdArgs {
                derivation_path: "".to_string(),  // Empty path for keystore master key
                app_public_key,
                domain_id: self.config.mpc_domain_id,
            },
        };

        // Call MPC contract
        let response = self.call_mpc_contract(signer_account_id, request_args).await?;

        // Decrypt and verify response
        // app_id must be derived the same way MPC contract does it
        let derivation_path = "";  // Empty path for keystore master key
        let app_id = derive_app_id(signer_account_id, derivation_path);
        tracing::debug!("Derived app_id: {:?}", app_id);
        tracing::debug!("Derived app_id (hex): {}", hex::encode(&app_id));
        let secret = self.decrypt_secret_and_verify(
            response.big_y,
            response.big_c,
            ephemeral_private_key,
            &app_id,
        )?;

        // Derive final key using HKDF
        let master_secret = self.derive_strong_key(secret, b"")?;

        tracing::info!("✅ Master secret successfully derived from MPC network");
        Ok(master_secret)
    }

    /// Generate ephemeral BLS12-381 G1 keypair
    fn generate_ephemeral_key(&self) -> (Scalar, G1Projective) {
        let mut rng = OsRng;
        let private_key = Scalar::random(&mut rng);
        let public_key = G1Projective::generator() * private_key;
        (private_key, public_key)
    }

    /// Convert G1 point to NEAR format (bls12381g1:base58...)
    fn g1_to_near_format(&self, point: G1Projective) -> String {
        let compressed = point.to_compressed();
        let base58 = bs58::encode(&compressed).into_string();
        format!("bls12381g1:{}", base58)
    }

    /// Parse NEAR format to G1 point
    fn near_format_to_g1(&self, s: &str) -> Result<G1Projective> {
        // Remove prefix
        let base58_part = s.strip_prefix("bls12381g1:")
            .context("Invalid BLS12-381 G1 format")?;

        // Decode base58
        let bytes = bs58::decode(base58_part)
            .into_vec()
            .context("Invalid base58 encoding")?;

        // Convert to compressed array
        let mut compressed = [0u8; 48];
        compressed.copy_from_slice(&bytes[..48]);

        // Parse as G1 point
        G1Projective::from_compressed(&compressed)
            .into_option()
            .context("Invalid G1 point")
    }

    /// Parse NEAR format to G2 point
    fn near_format_to_g2(&self, s: &str) -> Result<G2Projective> {
        // Remove prefix
        let base58_part = s.strip_prefix("bls12381g2:")
            .context("Invalid BLS12-381 G2 format")?;

        // Decode base58
        let bytes = bs58::decode(base58_part)
            .into_vec()
            .context("Invalid base58 encoding")?;

        // Convert to compressed array
        let mut compressed = [0u8; 96];
        compressed.copy_from_slice(&bytes[..96]);

        // Parse as G2 point
        G2Projective::from_compressed(&compressed)
            .into_option()
            .context("Invalid G2 point")
    }

    /// Call MPC contract's request_app_private_key method via transaction
    async fn call_mpc_contract(
        &self,
        _signer_account_id: &str,
        request: CkdRequestArgs,
    ) -> Result<CkdResponse> {
        tracing::info!("Calling MPC contract: {} using dao proxy contract", self.config.mpc_contract_id);

        // Serialize request to JSON
        let args = serde_json::to_vec(&request)?;

        // Get access key information for nonce.
        // Retry: after DAO approval the new access key may not be visible to the RPC node yet.
        let mut nonce = 0u64;
        let max_retries = 5;
        for attempt in 1..=max_retries {
            let access_key_query = methods::query::RpcQueryRequest {
                block_reference: BlockReference::Finality(Finality::Final),
                request: QueryRequest::ViewAccessKey {
                    account_id: self.signer.account_id.clone(),
                    public_key: self.signer.public_key.clone(),
                },
            };

            match self.rpc_client.call(access_key_query).await {
                Ok(response) => {
                    if let QueryResponseKind::AccessKey(access_key_view) = response.kind {
                        nonce = access_key_view.nonce + 1;
                        break;
                    } else {
                        anyhow::bail!("Failed to get access key nonce");
                    }
                }
                Err(e) if attempt < max_retries => {
                    tracing::warn!(
                        attempt,
                        max_retries,
                        "Access key not yet visible on RPC node, retrying in 3s... ({})", e
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                }
                Err(e) => {
                    return Err(e).context("Failed to query access key after retries");
                }
            }
        }

        // Get current block hash
        let block = self.rpc_client
            .call(methods::block::RpcBlockRequest {
                block_reference: BlockReference::Finality(Finality::Final),
            })
            .await
            .context("Failed to get latest block")?;

        // Create transaction
        let transaction_v0 = TransactionV0 {
            signer_id: self.signer.account_id.clone(),
            public_key: self.signer.public_key.clone(),
            nonce,
            receiver_id: self.signer.account_id.clone(), // call dao contract
            block_hash: block.header.hash,
            actions: vec![Action::FunctionCall(Box::new(FunctionCallAction {
                method_name: "request_key".to_string(),
                args,
                gas: 300_000_000_000_000, // 300 TGas
                deposit: 0,
            }))],
        };

        let transaction = Transaction::V0(transaction_v0);

        // Sign and send transaction
        let signature = self.signer.sign(transaction.get_hash_and_size().0.as_ref());
        let request = methods::broadcast_tx_commit::RpcBroadcastTxCommitRequest {
            signed_transaction: near_primitives::transaction::SignedTransaction::new(
                signature,
                transaction,
            ),
        };

        let outcome = self.rpc_client.call(request).await
            .context("Failed to call MPC contract")?;

        // Check transaction status and extract result
        match &outcome.status {
            FinalExecutionStatus::SuccessValue(value) => {
                // Parse response from transaction result
                let ckd_response: CkdResponse = serde_json::from_slice(value)
                    .context("Failed to parse MPC response")?;
                Ok(ckd_response)
            }
            FinalExecutionStatus::Failure(err) => {
                Err(anyhow::anyhow!("MPC transaction failed: {:?}", err))
            }
            other => {
                Err(anyhow::anyhow!("Unexpected transaction status: {:?}", other))
            }
        }
    }

    /// Decrypt secret and verify signature (from Bowen's example)
    fn decrypt_secret_and_verify(
        &self,
        big_y: String,
        big_c: String,
        private_key: Scalar,
        app_id: &[u8],
    ) -> Result<[u8; BLS12381G1_PUBLIC_KEY_SIZE]> {
        // Parse G1 points
        let big_y = self.near_format_to_g1(&big_y)?;
        let big_c = self.near_format_to_g1(&big_c)?;

        // Parse MPC public key (G2)
        let mpc_public_key = self.near_format_to_g2(&self.config.mpc_public_key)?;

        // Decrypt the secret: secret = big_c - big_y * private_key
        let secret = big_c - big_y * private_key;

        // Verify the signature using pairing
        if !self.verify_signature(&mpc_public_key, app_id, &secret) {
            anyhow::bail!("MPC signature verification failed");
        }

        // Return secret as bytes
        Ok(secret.to_compressed())
    }

    /// Verify MPC signature using pairing (from NEAR MPC ckd-example-cli)
    fn verify_signature(&self, public_key: &G2Projective, app_id: &[u8], signature: &G1Projective) -> bool {
        let element1: G1Affine = signature.into();
        if (!element1.is_on_curve() | !element1.is_torsion_free() | element1.is_identity()).into() {
            return false;
        }

        let element2: G2Affine = public_key.into();
        if (!element2.is_on_curve() | !element2.is_torsion_free() | element2.is_identity()).into() {
            return false;
        }

        // Hash input = MPC public key || app_id (must match MPC contract)
        let hash_input = [public_key.to_compressed().as_slice(), app_id].concat();
        let base1 = G1Projective::hash_to_curve(&hash_input, NEAR_CKD_DOMAIN, &[]).into();
        let base2 = G2Affine::generator();

        // Verify pairing equation
        blstrs::pairing(&base1, &element2) == blstrs::pairing(&element1, &base2)
    }

    /// Derive strong 32-byte key using HKDF (from Bowen's example)
    fn derive_strong_key(
        &self,
        ikm: [u8; BLS12381G1_PUBLIC_KEY_SIZE],
        info: &[u8],
    ) -> Result<[u8; OUTPUT_SECRET_SIZE]> {
        let hk = Hkdf::<Sha256>::new(None, &ikm);
        let mut okm = [0u8; OUTPUT_SECRET_SIZE];
        hk.expand(info, &mut okm)
            .map_err(|e| anyhow::anyhow!("HKDF expansion failed: {}", e))?;
        Ok(okm)
    }
}

/// Check if keystore is approved by DAO contract
pub async fn check_keystore_approval(
    near_rpc_url: &str,
    dao_contract: &str,
    public_key: &str,
) -> Result<bool> {
    let client = JsonRpcClient::connect(near_rpc_url);

    // Prepare view call arguments
    let args = serde_json::json!({
        "public_key": public_key
    });

    let request = methods::query::RpcQueryRequest {
        block_reference: BlockReference::Finality(Finality::Final),
        request: QueryRequest::CallFunction {
            account_id: dao_contract.parse()?,
            method_name: "is_keystore_approved".to_string(),
            args: FunctionArgs::from(serde_json::to_vec(&args)?),
        },
    };

    let response = client.call(request).await?;

    if let QueryResponseKind::CallResult(CallResult { result, .. }) = response.kind {
        let approved: bool = serde_json::from_slice(&result)?;
        Ok(approved)
    } else {
        Ok(false)
    }
}

/// Initialize keystore with MPC-derived master secret
pub async fn initialize_mpc_keystore(
    dao_contract_id: String,
    keystore_secret_key: near_crypto::SecretKey,
) -> Result<crate::crypto::Keystore> {
    tracing::info!("Initializing keystore with MPC CKD...");

    // Load configuration
    let mut config = MpcCkdConfig::from_env()?;
    config.keystore_dao_contract = dao_contract_id.clone();
    tracing::info!("MPC config: contract={}, domain={}",
        config.mpc_contract_id,
        config.mpc_domain_id
    );

    // The keystore DAO contract account is the signer for CKD
    // All approved keystores use the same account → get same secret
    let signer_account_id = config.keystore_dao_contract.clone();
    tracing::info!("Using DAO contract as signer: {}", signer_account_id);

    // Create signer for DAO contract using keystore's key (added as access key after approval)
    let signer = near_crypto::InMemorySigner::from_secret_key(
        signer_account_id.parse()?,
        keystore_secret_key,
    );

    // Create MPC client with signer
    let mpc_client = MpcCkdClient::new(config, signer);

    // Request deterministic master secret from MPC
    let master_secret = mpc_client.request_master_secret(&signer_account_id).await?;

    // Create keystore with MPC-derived master secret
    let keystore = crate::crypto::Keystore::from_master_secret(&master_secret)?;

    tracing::info!("✅ Keystore initialized with MPC-derived master secret");
    tracing::info!("   This secret is deterministic for account: {}", signer_account_id);
    tracing::info!("   All approved keystores will derive the same secret");

    Ok(keystore)
}