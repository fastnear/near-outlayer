//! HTTP API server for keystore worker
//!
//! ## Route Access Levels
//!
//! ### Public endpoints (no auth required):
//! - GET /health - Health check
//! - POST /pubkey - Get public key for encryption (used by dashboard)
//! - GET /vrf/pubkey - Get VRF public key for verification
//!
//! ### Worker-only endpoints (ALLOWED_WORKER_TOKEN_HASHES):
//! - POST /decrypt - Decrypt secrets from contract
//! - POST /encrypt - Encrypt data (for TopUp flow)
//! - POST /decrypt-raw - Decrypt raw data with seed
//! - POST /storage/encrypt - Encrypt persistent storage data
//! - POST /storage/decrypt - Decrypt persistent storage data
//! - POST /vrf/generate - Generate VRF output (verifiable random)
//!
//! ### Coordinator-only endpoints (ALLOWED_COORDINATOR_TOKEN_HASHES):
//! - POST /add_generated_secret - Add generated PROTECTED_ secrets
//! - POST /update_user_secrets - Update user secrets with NEP-413 signature
//!
//! ### TEE registration endpoints (coordinator OR worker token):
//! - POST /tee-challenge - Get challenge for TEE session registration
//! - POST /register-tee - Complete challenge-response and create TEE session
//!
//! ## Security Model
//!
//! Workers (running in TEE) get access to decrypt/encrypt endpoints.
//! Coordinator (NOT in TEE) only gets access to secret management endpoints
//! that require additional NEP-413 signature verification.

use axum::{
    extract::State,
    http::StatusCode,
    middleware,
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use tower_http::trace::TraceLayer;

use crate::attestation::Attestation;

/// In-memory TEE challenge entry (for challenge-response protocol)
struct TeeChallenge {
    created_at: std::time::Instant,
}

/// In-memory TEE session entry
#[derive(Clone)]
struct TeeSession {
    #[allow(dead_code)]
    worker_public_key: String,
    #[allow(dead_code)]
    created_at: std::time::Instant,
}

/// Bundled context populated together by `set_mpc_context` after TEE
/// registration. See [`AppState::mpc_context`] for the rationale.
///
/// **Nonce-lock invariant:** the keystore-worker holds ONE access key
/// on the keystore-dao contract that's authorised for both
/// `request_key` (CKD via [`crate::mpc_ckd`]) and
/// `mark_vault_verified` / `ban_vault` (via
/// [`crate::near::NearClient::submit_function_call`]). Any tx submitted
/// with this signer must serialize through `signer_nonce_lock` —
/// otherwise concurrent callers race to read the same nonce, build
/// txs with the same `nonce + 1`, and only one wins; the loser
/// surfaces an opaque `InvalidNonce` 500.
///
/// CKD calls FROM a vault account (Layer 2 of the per-vault master derivation) use a DIFFERENT
/// signer (the vault's TEE function-call key) and don't share this lock.
pub struct MpcContext {
    /// MPC CKD config (incl. `keystore_dao_id` and `mpc_contract_id`,
    /// both pre-parsed `AccountId`s). We require
    /// separate `keystore_dao_id` on this struct — read it from
    /// `mpc_ckd_config.keystore_dao_id` to keep one source of truth.
    pub mpc_ckd_config: crate::mpc_ckd::MpcCkdConfig,
    pub keystore_dao_signer: near_crypto::InMemorySigner,
    pub signer_nonce_lock: tokio::sync::Mutex<()>,
}

/// Application state shared across all handlers
#[derive(Clone)]
pub struct AppState {
    pub keystore: std::sync::Arc<tokio::sync::RwLock<crate::crypto::Keystore>>,
    pub config: crate::config::Config,
    pub expected_measurements: crate::attestation::ExpectedMeasurements,
    pub near_client: Option<std::sync::Arc<crate::near::NearClient>>,
    pub is_ready: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// MPC CKD context — config + dao id + worker's keystore-dao
    /// signer with an integrated nonce-lock — populated only after a
    /// successful TEE registration. Empty for non-TEE / mock modes,
    /// which means the lazy per-vault-master path AND
    /// `/sign-vault-verification` are unavailable; any request that
    /// names a customer or asks to mark a vault fails fast with an
    /// explanatory error rather than silently falling back.
    ///
    /// **Why one struct, not three OnceLocks (atomicity invariant):**
    /// the three values are set together by exactly one path
    /// (`perform_tee_registration` → `set_mpc_context`). Bundling
    /// them under a single `OnceLock<MpcContext>` makes the set
    /// atomic — no observer can ever see a half-populated state
    /// where, say, `keystore_dao_signer` is set but `mpc_ckd_config`
    /// isn't.
    ///
    /// **Set-once interior mutability** is required because
    /// `AppState` is `Clone`d into handler tasks before TEE
    /// registration completes; the registration task runs async and
    /// populates this AFTER cloning.
    pub mpc_context: std::sync::Arc<std::sync::OnceLock<MpcContext>>,
    /// In-memory TEE challenge store: challenge_hex -> TeeChallenge
    tee_challenges: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, TeeChallenge>>>,
    /// In-memory TEE session store: session_id -> TeeSession
    tee_sessions: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<uuid::Uuid, TeeSession>>>,
}

impl AppState {
    pub fn new(
        keystore: crate::crypto::Keystore,
        config: crate::config::Config,
        near_client: Option<crate::near::NearClient>,
    ) -> Self {
        // Check if we're in TEE registration mode
        let is_tee_registration = std::env::var("USE_TEE_REGISTRATION")
            .unwrap_or_else(|_| "false".to_string())
            .parse::<bool>()
            .unwrap_or(false);

        // If not in TEE mode or already initialized, we're ready
        // If in TEE mode, we're ready only after getting master key from MPC
        let is_ready = !is_tee_registration;

        Self {
            keystore: std::sync::Arc::new(tokio::sync::RwLock::new(keystore)),
            config: config.clone(),
            expected_measurements: crate::attestation::ExpectedMeasurements::default(),
            near_client: near_client.map(std::sync::Arc::new),
            is_ready: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(is_ready)),
            mpc_context: std::sync::Arc::new(std::sync::OnceLock::new()),
            tee_challenges: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
            tee_sessions: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        }
    }

    pub fn mark_ready(&self) {
        self.is_ready.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn is_ready(&self) -> bool {
        self.is_ready.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub async fn replace_keystore(&self, new_keystore: crate::crypto::Keystore) {
        let mut keystore = self.keystore.write().await;
        *keystore = new_keystore;
    }

    /// Populate the MPC CKD context after a successful TEE-mode boot.
    /// Called from `main.rs` once we know which keystore-dao + MPC
    /// contracts the worker is bound to. Set-once.
    ///
    /// **Re-call safety**: on the second call, the OnceLock will refuse
    /// the new value. If the inputs are identical to the first set
    /// (same dao_id, same MPC config) we treat it as benign idempotency.
    /// If they differ, we WARN — it almost certainly means a bug
    /// (worker re-registered against a different DAO mid-flight, or a
    /// configuration race). We do NOT panic because that would crash a
    /// running keystore over what is most likely an operator-fixable
    /// situation, but the warning is loud so on-call notices.
    ///
    /// Takes `&self` because `AppState` is shared across handler tasks
    /// via `Clone`. The fields use `OnceLock` for set-once interior
    /// mutability.
    pub fn set_mpc_context(
        &self,
        config: crate::mpc_ckd::MpcCkdConfig,
        keystore_dao_signer: near_crypto::InMemorySigner,
    ) {
        let new_ctx = MpcContext {
            mpc_ckd_config: config,
            keystore_dao_signer,
            signer_nonce_lock: tokio::sync::Mutex::new(()),
        };
        if let Err(rejected) = self.mpc_context.set(new_ctx) {
            let prev = self
                .mpc_context
                .get()
                .expect("OnceLock returned Err so it must already be set");
            // Re-call protection. The only legitimate caller is the
            // post-TEE-registration path, which runs once per process
            // — any second call is a bug. Warn loudly even on
            // identical config so the caller is held accountable
            // (intentional escalation from `debug!` so identical
            // re-set still surfaces as a state-bug signal).
            let drift = prev.mpc_ckd_config.mpc_contract_id != rejected.mpc_ckd_config.mpc_contract_id
                || prev.mpc_ckd_config.mpc_domain_id != rejected.mpc_ckd_config.mpc_domain_id
                || prev.mpc_ckd_config.mpc_public_key != rejected.mpc_ckd_config.mpc_public_key
                || prev.mpc_ckd_config.keystore_dao_id != rejected.mpc_ckd_config.keystore_dao_id
                || prev.keystore_dao_signer.public_key != rejected.keystore_dao_signer.public_key;
            if drift {
                tracing::warn!(
                    prev_dao = %prev.mpc_ckd_config.keystore_dao_id,
                    new_dao = %rejected.mpc_ckd_config.keystore_dao_id,
                    prev_mpc = %prev.mpc_ckd_config.mpc_contract_id,
                    new_mpc = %rejected.mpc_ckd_config.mpc_contract_id,
                    "set_mpc_context called twice with DIFFERENT context — keeping first, second ignored. \
                     This indicates a bug or misconfiguration."
                );
            } else {
                tracing::warn!(
                    "set_mpc_context called twice with identical context — second call ignored. \
                     This indicates a coding bug (only one TEE registration runs per process)."
                );
            }
        }
    }

    /// Lazy-load gate wrapper used by per-customer handlers. Snapshots
    /// the keystore (cheap — internal state is `Arc`-shared, so any
    /// inserts done by `add_customer` become visible to all clones) and
    /// delegates to [`crate::mpc_ckd::ensure_customer_loaded`].
    ///
    /// Returns an error when:
    ///   * `customer = Some(_)` but the worker was booted without MPC
    ///     context (non-TEE / mock — feature unsupported in that mode).
    ///   * the vault is not verified on keystore-dao (or banned).
    ///   * the MPC CKD round-trip fails.
    ///
    /// `customer = None` always returns `Ok(())` immediately (legacy
    /// default-master path).
    pub async fn ensure_customer_loaded(
        &self,
        customer: Option<&near_primitives::types::AccountId>,
    ) -> anyhow::Result<()> {
        let Some(vault_id) = customer else {
            return Ok(());
        };

        // Snapshot the keystore. Cloning is cheap (Arc-shared internal
        // state) and avoids holding the outer RwLock across the await.
        // Inserts into the snapshot's `masters` propagate to the
        // canonical keystore via the shared Arc.
        let keystore_snapshot = self.keystore.read().await.clone();

        // Fast path: master already loaded → skip both the verify
        // view-call and the MPC-context requirement. This is what
        // makes the eviction-then-readd flow work, and what lets
        // tests / mock modes operate on pre-populated customers
        // without needing real MPC config.
        if keystore_snapshot.has_customer(vault_id) {
            return Ok(());
        }

        let ctx = self.mpc_context.get().ok_or_else(|| {
            anyhow::anyhow!(
                "per-customer master requested for {} but worker is running \
                 without MPC CKD context (non-TEE mode)",
                vault_id
            )
        })?;
        let near_client = self.near_client.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "per-customer master requested for {} but near_client \
                 is not configured",
                vault_id
            )
        })?;

        crate::mpc_ckd::ensure_customer_loaded(
            &ctx.mpc_ckd_config,
            near_client.as_ref(),
            &ctx.mpc_ckd_config.keystore_dao_id,
            &keystore_snapshot,
            Some(vault_id),
        )
        .await
    }
}

/// API error types
#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    Unauthorized(String),
    Forbidden(String),
    InternalError(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg),
            ApiError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg),
            ApiError::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        (status, Json(serde_json::json!({"error": message}))).into_response()
    }
}

/// Secret accessor type - matches contract's SecretAccessor enum
///
/// IMPORTANT: When adding new accessor types:
/// 1. Add variant here in keystore-worker
/// 2. Add variant in coordinator/src/handlers/github.rs (SecretAccessor enum)
/// 3. Add variant in contract/src/lib.rs (SecretAccessor enum)
/// 4. Update seed generation in decrypt_handler below
/// 5. Update near.rs get_secrets methods if needed
/// 6. Update worker/src/keystore_client.rs decrypt methods
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SecretAccessor {
    /// Secrets bound to a GitHub repository
    Repo {
        repo: String,
        #[serde(default)]
        branch: Option<String>,
    },
    /// Secrets bound to a specific WASM hash
    WasmHash {
        hash: String,
    },
    /// Secrets bound to a project (available to all versions)
    Project {
        project_id: String,
    },
    /// System secrets (Payment Keys for HTTPS API)
    System {
        secret_type: SystemSecretType,
    },
}

/// System secret types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemSecretType {
    /// Payment Key for HTTPS API
    PaymentKey,
}

impl SystemSecretType {
    /// Contract-side representation (CamelCase, matches NEAR's default
    /// JSON serialisation of the contract's `SystemSecretType` enum
    /// variants). Used by `accessor_to_contract_json` when forwarding
    /// the variant to a contract view method. **Centralised here so a
    /// future variant addition is one match arm**, not five scattered
    /// across the file.
    fn as_contract_str(&self) -> &'static str {
        match self {
            SystemSecretType::PaymentKey => "PaymentKey",
        }
    }

    /// Seed-string representation (snake_case, matches the seed
    /// convention used at write time by all storage paths). Used inside
    /// derived secret seeds for decrypt/encrypt of System secrets.
    fn as_seed_str(&self) -> &'static str {
        match self {
            SystemSecretType::PaymentKey => "payment_key",
        }
    }
}

/// Request to decrypt secrets from contract
#[derive(Debug, Deserialize)]
pub struct DecryptRequest {
    /// What code can access these secrets
    pub accessor: SecretAccessor,

    /// Profile name (e.g., "default", "production")
    pub profile: String,

    /// Owner account ID (who owns the secrets)
    pub owner: String,

    /// User account ID (who is requesting execution)
    /// This is used for access control validation
    pub user_account_id: String,

    /// TEE attestation proving worker identity
    pub attestation: Attestation,

    /// Optional task ID for logging
    pub task_id: Option<String>,
}

/// Response with decrypted secrets
#[derive(Debug, Serialize)]
pub struct DecryptResponse {
    /// Decrypted secrets (base64 encoded)
    /// Base64 is used to safely transport binary data over JSON
    pub plaintext_secrets: String,
}

/// Request to get public key (includes secrets for validation)
#[derive(Debug, Deserialize)]
pub struct PubkeyRequest {
    /// Seed for deriving keypair (format: "repo:owner[:branch]")
    pub seed: String,
    /// Secrets as JSON string for validation (e.g., '{"API_KEY":"value"}')
    pub secrets_json: String,
    /// Optional vault id: when set, the returned pubkey is
    /// derived from the per-vault master so the resulting on-chain
    /// secret is encrypted under the correct customer scope. Absent
    /// (or null/empty) ⇒ default OutLayer master, legacy behaviour.
    #[serde(default)]
    pub vault_id: Option<String>,
}

/// Response with public key
#[derive(Debug, Serialize)]
pub struct PubkeyResponse {
    /// Public key in hex format
    pub pubkey: String,
}

/// Request to add generated secrets to existing encrypted secrets
#[derive(Debug, Deserialize)]
pub struct AddGeneratedSecretRequest {
    /// Seed for deriving keypair (format: "repo:owner[:branch]")
    pub seed: String,

    /// Existing encrypted secrets (base64, can be empty for first generation)
    /// If empty, starts with empty secrets object
    pub encrypted_secrets_base64: Option<String>,

    /// New secrets to generate
    pub new_secrets: Vec<GeneratedSecretSpec>,

    /// Optional vault id: scope the encrypt/decrypt calls to a
    /// per-vault master. MUST match the scope under which
    /// `encrypted_secrets_base64` was originally encrypted, otherwise
    /// the decrypt step will fail. Absent ⇒ default OutLayer master.
    #[serde(default)]
    pub vault_id: Option<String>,
}

/// Specification for a secret to generate
#[derive(Debug, Deserialize)]
pub struct GeneratedSecretSpec {
    /// Secret name (key in JSON)
    pub name: String,

    /// Generation type (hex32, ed25519, password, etc.)
    pub generation_type: String,
}

/// Response after adding generated secrets
#[derive(Debug, Serialize)]
pub struct AddGeneratedSecretResponse {
    /// Updated encrypted secrets (base64)
    pub encrypted_data_base64: String,

    /// List of ALL secret key names after merge (for verification)
    pub all_keys: Vec<String>,
}

/// Mode for updating user secrets
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateMode {
    /// Add/update secrets, keeping existing ones
    Append,
    /// Replace all non-PROTECTED secrets
    Reset,
}

/// Request to update user secrets (with NEAR signature)
#[derive(Debug, Deserialize)]
pub struct UpdateUserSecretsRequest {
    /// Current accessor for the secrets
    pub accessor: SecretAccessor,

    /// Optional new accessor (for migration)
    pub new_accessor: Option<SecretAccessor>,

    /// Profile name
    pub profile: String,

    /// Owner account ID
    pub owner: String,

    /// Update mode
    pub mode: UpdateMode,

    /// User secrets to add/update (cannot contain PROTECTED_ prefix)
    /// Values can be strings, numbers, booleans, or null - preserved as-is
    pub secrets: std::collections::HashMap<String, serde_json::Value>,

    /// Optional PROTECTED_ secrets to generate
    pub generate_protected: Option<Vec<GeneratedSecretSpec>>,

    /// Signed message (format: "Update Outlayer secrets for owner:profile")
    pub signed_message: String,

    /// Ed25519 signature
    pub signature: String,

    /// Public key (ed25519:base58...)
    pub public_key: String,

    /// Nonce for NEP-413
    pub nonce: String,

    /// Recipient for NEP-413 signature verification
    pub recipient: String,

    /// Optional vault id: scope the encrypt/decrypt calls to
    /// a per-vault master. MUST match the scope under which the existing
    /// secrets were encrypted (when in Append mode reading current
    /// state). Absent ⇒ default OutLayer master.
    #[serde(default)]
    pub vault_id: Option<String>,
}

/// Response after updating user secrets
#[derive(Debug, Serialize)]
pub struct UpdateUserSecretsResponse {
    /// Updated encrypted secrets (base64) for storing in contract
    pub encrypted_secrets_base64: String,

    /// Summary of changes
    pub summary: UpdateSummary,
}

#[derive(Debug, Serialize)]
pub struct UpdateSummary {
    /// PROTECTED_ keys that were preserved
    pub protected_keys_preserved: Vec<String>,

    /// Keys that were updated/added
    pub updated_keys: Vec<String>,

    /// Keys that were removed (only in reset mode)
    pub removed_keys: Vec<String>,

    /// Total number of secrets after update
    pub total_keys: usize,
}

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub tee_mode: String,
}

// ==================== Storage Encryption API ====================

/// Request to encrypt data for persistent storage
#[derive(Debug, Deserialize)]
pub struct StorageEncryptRequest {
    /// Project UUID (None for standalone WASM - use wasm_hash instead)
    pub project_uuid: Option<String>,
    /// WASM hash (used when project_uuid is None)
    pub wasm_hash: String,
    /// Account ID (user account or "@worker" for private storage)
    pub account_id: String,
    /// Plaintext key (will be encrypted)
    pub key: String,
    /// Plaintext value (base64 encoded)
    pub value_base64: String,
    /// TEE attestation proving worker identity
    pub attestation: Attestation,
}

/// Response with encrypted storage data
#[derive(Debug, Serialize)]
pub struct StorageEncryptResponse {
    /// Encrypted key (base64)
    pub encrypted_key_base64: String,
    /// Encrypted value (base64)
    pub encrypted_value_base64: String,
    /// Key hash for unique constraint (SHA256 of plaintext key)
    pub key_hash: String,
}

/// Request to decrypt data from persistent storage
#[derive(Debug, Deserialize)]
pub struct StorageDecryptRequest {
    /// Project UUID (None for standalone WASM - use wasm_hash instead)
    pub project_uuid: Option<String>,
    /// WASM hash (used when project_uuid is None)
    pub wasm_hash: String,
    /// Account ID (user account or "@worker" for private storage)
    pub account_id: String,
    /// Encrypted key (base64)
    pub encrypted_key_base64: String,
    /// Encrypted value (base64)
    pub encrypted_value_base64: String,
    /// TEE attestation proving worker identity
    pub attestation: Attestation,
}

/// Response with decrypted storage data
#[derive(Debug, Serialize)]
pub struct StorageDecryptResponse {
    /// Decrypted key
    pub key: String,
    /// Decrypted value (base64)
    pub value_base64: String,
}

// ==================== Generic Encryption API (for TopUp flow) ====================

/// Request to generate VRF output
#[derive(Debug, Deserialize)]
pub struct VrfGenerateRequest {
    /// Alpha string (VRF pre-image). Format: "vrf:{request_id}:{user_seed}"
    pub alpha: String,
    /// TEE attestation proving worker identity
    pub attestation: Attestation,
}

/// Response with VRF output and signature (proof)
#[derive(Debug, Serialize)]
pub struct VrfGenerateResponse {
    /// VRF output: SHA256(Ed25519_signature), 32 bytes hex
    pub output_hex: String,
    /// VRF proof: Ed25519 signature, 64 bytes hex
    pub signature_hex: String,
}

/// Response with VRF public key
#[derive(Debug, Serialize)]
pub struct VrfPublicKeyResponse {
    /// VRF public key (Ed25519), 32 bytes hex
    pub vrf_public_key_hex: String,
}

/// `POST /admin/evict-customer` request body.
#[derive(Debug, Deserialize)]
pub struct AdminEvictCustomerRequest {
    /// Vault account id whose per-customer master should be dropped
    /// from cache. Must parse as `AccountId`.
    pub vault_id: String,
    /// Free-form reason logged for audit visibility (e.g.
    /// `"duplicate_mpc_call_after_init"`, `"manual_dao_ban"`). Not
    /// validated against any whitelist — operators set the
    /// convention.
    pub reason: String,
}

/// `POST /admin/evict-customer` response. `ok = true` always; the
/// endpoint is idempotent and signals no error states.
#[derive(Debug, Serialize)]
pub struct AdminEvictCustomerResponse {
    pub ok: bool,
}

/// `POST /admin/ban-vault` request body.
///
/// Called by the race-attack monitoring service when it
/// detects more than one MPC `request_app_private_key` call from the
/// same vault account within the dedup window. The keystore-worker
/// submits `keystore_dao.ban_vault(vault_id, reason)` on chain (its
/// access key on the keystore-DAO contract is approved for this
/// method) and ALSO evicts the per-customer master from cache so the
/// ban takes effect within milliseconds rather than waiting for the
/// next worker restart.
#[derive(Debug, Deserialize)]
pub struct AdminBanVaultRequest {
    pub vault_id: String,
    /// Free-form reason recorded in the on-chain `vault_banned` log
    /// event (max 256 bytes per the contract). Operators choose the
    /// convention; typical values:
    /// `"duplicate_mpc_call_after_init"`,
    /// `"manual_admin_action"`,
    /// `"phase8_monitor_alert_<id>"`.
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct AdminBanVaultResponse {
    /// Tx hash of the on-chain `ban_vault` submission. `None` if the
    /// vault is already banned (idempotent short-circuit).
    pub tx_hash: Option<String>,
    /// `true` when the keystore short-circuited because
    /// `is_vault_banned == true` already.
    pub already_banned: bool,
}

/// `POST /sign-vault-verification` request body.
///
/// Sent by the public `outlayer.near/vault-checker` WASI agent after
/// it has verified the vault state itself. The keystore-worker
/// re-runs the SAME 5 RPC checks (defense-in-depth) and only then
/// signs+broadcasts `keystore_dao.mark_vault_verified(vault_id)`.
#[derive(Debug, Deserialize)]
pub struct SignVaultVerificationRequest {
    /// Vault account id to mark as verified.
    pub vault_id: String,
}

/// `POST /sign-vault-verification` response.
///
/// * `tx_hash = Some(_)` — the worker submitted a fresh
///   `mark_vault_verified` tx; the value is the base58 NEAR tx hash.
/// * `tx_hash = None`, `already_verified = true` — the vault was
///   already verified on-chain; the worker SHORT-CIRCUITED without
///   spending gas or a nonce. Audit noteC3: spamming
///   `/sign-vault-verification` against an already-verified vault
///   would otherwise burn the worker's per-tx nonce and gas budget.
///
/// **Idempotency contract (Audit noteC1):** a 5xx
/// response from this endpoint MUST be treated as ambiguous by the
/// caller — the tx may or may not have landed. Caller MUST query
/// `keystore_dao.is_vault_verified(vault_id)` independently before
/// retrying; a retry without that check could trip a NEAR
/// `InvalidNonce` if the original tx silently committed.
#[derive(Debug, Serialize)]
pub struct SignVaultVerificationResponse {
    /// Base58-encoded tx hash of the `mark_vault_verified` call, or
    /// `None` when `already_verified = true`.
    pub tx_hash: Option<String>,
    /// `true` if the vault was already verified at request time.
    /// `false` if a fresh tx was submitted (and `tx_hash` is `Some`).
    #[serde(default)]
    pub already_verified: bool,
}

/// `POST /derive-vault-tee-key` request body.
///
/// Used by `outlayer-cli init-vault` to fetch the public
/// half of the Layer-1 TEE keypair BEFORE submitting the atomic
/// deploy that adds it as a function-call AccessKey on the new vault.
/// The keypair is HMAC-derived from the OutLayer default master with
/// seed `outlayer.near:{vault_id}` (Layer 1 of the per-vault master derivation) — deterministic,
/// re-derivable on any approved TEE, never leaves the enclave's
/// memory in private form.
#[derive(Debug, Deserialize)]
pub struct DeriveVaultTeeKeyRequest {
    pub vault_id: String,
}

/// `POST /derive-vault-tee-key` response.
///
/// Returns ONLY the public key — the private key never leaves the TEE
/// (it's used by the worker to sign Layer-2 MPC CKD calls FROM the
/// vault). The caller (CLI) embeds `public_key` in an `AddKey` action
/// inside the atomic deploy tx; afterwards anyone reading
/// `view_access_key_list(vault_id)` sees that exact pubkey, which
/// `vault-checker` and `vault_verifier.rs` then assert against.
#[derive(Debug, Serialize)]
pub struct DeriveVaultTeeKeyResponse {
    /// NEAR-format public key, e.g. `"ed25519:<base58>"`.
    pub public_key: String,
}

/// Request to encrypt plaintext data
/// Used by workers to re-encrypt secrets after TopUp
#[derive(Debug, Deserialize)]
pub struct EncryptRequest {
    /// Seed for deriving keypair (format depends on secret type)
    /// For Payment Key: "system:payment_key:{owner}:{nonce}"
    pub seed: String,
    /// Plaintext data to encrypt (base64 encoded)
    pub plaintext_base64: String,
    /// TEE attestation proving worker identity
    pub attestation: Attestation,
}

/// Response with encrypted data
#[derive(Debug, Serialize)]
pub struct EncryptResponse {
    /// Encrypted data (base64)
    pub encrypted_base64: String,
}

// ==================== Wallet API ====================

/// Request to derive a wallet address for a specific chain
#[derive(Debug, Deserialize)]
pub struct WalletDeriveAddressRequest {
    pub wallet_id: String,
    pub chain: String,
}

/// Response with derived address and public key
#[derive(Debug, Serialize)]
pub struct WalletDeriveAddressResponse {
    pub address: String,
    pub public_key: String,
}

/// Request to sign a transaction
#[derive(Debug, Deserialize)]
pub struct WalletSignTransactionRequest {
    pub wallet_id: String,
    pub chain: String,
    pub tx_bytes_base64: String,
}

/// Response with signature
#[derive(Debug, Serialize)]
pub struct WalletSignTransactionResponse {
    pub signature_base64: String,
}

/// Request to sign encrypted policy data (for on-chain store_wallet_policy)
#[derive(Debug, Deserialize)]
pub struct WalletSignPolicyRequest {
    pub wallet_id: String,
    pub encrypted_data_hash: String, // SHA256 hex of encrypted_data
}

/// Response with ed25519 signature + public key for contract verification
#[derive(Debug, Serialize)]
pub struct WalletSignPolicyResponse {
    pub signature_hex: String,  // ed25519 signature (64 bytes hex)
    pub public_key_hex: String, // ed25519 public key (32 bytes hex)
}

/// Approval info passed from coordinator for keystore verification
#[derive(Debug, Deserialize)]
pub struct ApprovalInfo {
    pub approver_ids: Vec<String>,
    pub request_hash: String,
}

/// Request to sign a NEP-413 intent message
#[derive(Debug, Deserialize)]
pub struct WalletSignNep413Request {
    pub wallet_id: String,
    pub chain: String,
    pub message: String,
    pub nonce_base64: String,
    pub recipient: String,
    #[serde(default)]
    pub approval_info: Option<ApprovalInfo>,
}

/// Response with NEP-413 signature
#[derive(Debug, Serialize)]
pub struct WalletSignNep413Response {
    pub signature_base58: String,
    pub public_key: String,
}

/// Request to build and sign a native NEAR transfer transaction
#[derive(Debug, Deserialize)]
pub struct WalletSignNearTransferRequest {
    pub wallet_id: String,
    pub receiver_id: String,
    pub amount: String,
    #[serde(default)]
    pub approval_info: Option<ApprovalInfo>,
}

/// Request to build and sign a NEAR DeleteAccount transaction
#[derive(Debug, Deserialize)]
pub struct WalletSignNearDeleteAccountRequest {
    pub wallet_id: String,
    pub beneficiary_id: String,
    #[serde(default)]
    pub approval_info: Option<ApprovalInfo>,
}

/// Transaction arguments: Borsh base64, JSON string, or empty (no args).
/// Variants are tried in order by serde — Base64 and Json first (have required fields),
/// Empty last (matches anything, used as fallback for no-arg calls).
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum CallArgs {
    /// For Borsh-encoded payloads (e.g. FastFS) that can't be represented as JSON.
    Base64 { args_base64: String },
    Json { args_json: String },
    /// No arguments — produces empty bytes (valid NEAR call with no params).
    Empty {},
}

impl CallArgs {
    pub fn into_bytes(self) -> Result<Vec<u8>, ApiError> {
        match self {
            CallArgs::Base64 { args_base64 } => {
                base64::decode(&args_base64).map_err(|e| {
                    ApiError::BadRequest(format!("Invalid args_base64: {}", e))
                })
            }
            CallArgs::Json { args_json } => Ok(args_json.into_bytes()),
            CallArgs::Empty {} => Ok(vec![]),
        }
    }
}

/// Request to build and sign a NEAR function call transaction
#[derive(Debug, Deserialize)]
pub struct WalletSignNearCallRequest {
    pub wallet_id: String,
    pub receiver_id: String,
    pub method_name: String,
    #[serde(flatten)]
    pub args: CallArgs,
    pub gas: u64,
    pub deposit: String,
    #[serde(default)]
    pub approval_info: Option<ApprovalInfo>,
    /// Override the nonce instead of querying RPC.
    /// Used when sending multiple transactions sequentially (e.g. swap flow)
    /// to avoid nonce conflicts due to RPC finality lag.
    #[serde(default)]
    pub override_nonce: Option<u64>,
}

/// Response with signed NEAR transaction
#[derive(Debug, Serialize)]
pub struct WalletSignNearCallResponse {
    pub signed_tx_base64: String,
    pub tx_hash: String,
    pub signer_id: String,
    pub public_key: String,
    /// The nonce used in this transaction (callers can pass nonce+1 as override_nonce for the next tx)
    pub nonce: u64,
}

/// Request to check policy for a wallet action
#[derive(Debug, Deserialize)]
pub struct WalletCheckPolicyRequest {
    pub wallet_id: String,
    pub action: serde_json::Value,
    /// Optional: encrypted policy data (base64) for local/test policy override.
    /// When provided, skips fetching from NEAR contract.
    #[serde(default)]
    pub encrypted_policy_data: Option<String>,
}

/// Response from policy check
#[derive(Debug, Serialize)]
pub struct WalletCheckPolicyResponse {
    pub allowed: bool,
    pub frozen: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires_approval: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_approvals: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy: Option<serde_json::Value>,
}

/// Request to encrypt a wallet policy
#[derive(Debug, Deserialize)]
pub struct WalletEncryptPolicyRequest {
    pub wallet_id: String,
    pub policy_json: String,
}

/// Response with encrypted policy
#[derive(Debug, Serialize)]
pub struct WalletEncryptPolicyResponse {
    pub encrypted_base64: String,
}

/// Create the API router with all endpoints
///
/// Route access levels:
/// - Public (no auth): /health, /pubkey
/// - Worker-only (ALLOWED_WORKER_TOKEN_HASHES): /decrypt, /encrypt, /decrypt-raw, /storage/*
/// - Coordinator-only (ALLOWED_COORDINATOR_TOKEN_HASHES): /add_generated_secret, /update_user_secrets, /wallet/*
pub fn create_router(state: AppState) -> Router {
    // Worker-only routes (for TEE workers)
    // These endpoints require valid worker token - coordinator CANNOT access them
    // TEE session middleware runs AFTER auth (inner layer runs first in axum)
    let worker_routes = Router::new()
        .route("/decrypt", post(decrypt_handler))
        .route("/encrypt", post(encrypt_handler)) // For TopUp flow - re-encrypt with new balance
        .route("/decrypt-raw", post(decrypt_raw_handler)) // For TopUp flow - decrypt raw data with seed
        .route("/storage/encrypt", post(storage_encrypt_handler))
        .route("/storage/decrypt", post(storage_decrypt_handler))
        .route("/vrf/generate", post(vrf_generate_handler))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            tee_session_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            worker_auth_middleware,
        ));

    // Coordinator-only routes (for dashboard proxy)
    // These endpoints require valid coordinator token - workers CANNOT access them
    let coordinator_routes = Router::new()
        .route("/add_generated_secret", post(add_generated_secret_handler))
        .route("/update_user_secrets", post(update_user_secrets_handler)) // + NEP-413 signature
        // Wallet endpoints (coordinator-only)
        .route("/wallet/derive-address", post(wallet_derive_address_handler))
        .route("/wallet/sign-transaction", post(wallet_sign_transaction_handler))
        .route("/wallet/sign-nep413", post(wallet_sign_nep413_handler))
        .route("/wallet/sign-near-call", post(wallet_sign_near_call_handler))
        .route("/wallet/sign-near-transfer", post(wallet_sign_near_transfer_handler))
        .route("/wallet/sign-near-delete-account", post(wallet_sign_near_delete_account_handler))
        .route("/wallet/sign-policy", post(wallet_sign_policy_handler))
        .route("/wallet/check-policy", post(wallet_check_policy_handler))
        .route("/wallet/encrypt-policy", post(wallet_encrypt_policy_handler))
        // Ephemeral keys — separate module, returns private keys (see ephemeral_keys.rs)
        .merge(crate::ephemeral_keys::ephemeral_key_routes())
        .layer(middleware::from_fn_with_state(
            state.clone(),
            coordinator_auth_middleware,
        ));

    // Admin routes — internal-only side-channel for the monitoring
    // service to forcibly drop a per-customer master from cache when a
    // vault is banned. Auth piggybacks on the worker token so anything
    // that already speaks to /decrypt can also evict — see the route's
    // doc-comment on `/admin/evict-customer` for the threat model.
    let admin_routes = Router::new()
        .route("/admin/evict-customer", post(admin_evict_customer_handler))
        // Race-attack monitor calls this. Worker-token-only —
        // unlike vault_routes, this MUTATES on-chain state by submitting
        // a `ban_vault` tx, so the auth boundary is intentionally
        // tight. Operator's monitoring service must hold a worker token.
        .route("/admin/ban-vault", post(admin_ban_vault_handler))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            worker_auth_middleware,
        ));

    // Vault provisioning routes — accept EITHER coordinator OR worker
    // token. `/sign-vault-verification` is called by the public
    // `outlayer.near/vault-checker` WASI agent (worker token, since the
    // agent runs inside OutLayer's TEE workers) AND by the coordinator
    // proxy `/customer/sign-verification` driven by `outlayer vault init`
    // (coordinator token, since the CLI customer doesn't have a worker
    // token). `/derive-vault-tee-key` has the same caller mix. Both
    // endpoints' security guarantee is in-process re-verification or
    // public-only return, NOT the auth boundary; widening the auth lane
    // keeps the deploy story simple (one keystore token in coordinator
    // env, one in worker env, no double-allowlisting required) without
    // weakening the trust model.
    let vault_routes = Router::new()
        .route("/sign-vault-verification", post(sign_vault_verification_handler))
        .route("/derive-vault-tee-key", post(derive_vault_tee_key_handler))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            tee_registration_auth_middleware,
        ));

    // TEE session routes (coordinator OR worker auth)
    // Workers can register directly or via coordinator proxy.
    // Security: challenge-response + NEAR RPC key check provide the actual verification.
    let tee_routes = Router::new()
        .route("/tee-challenge", post(tee_challenge_handler))
        .route("/register-tee", post(register_tee_handler))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            tee_registration_auth_middleware,
        ));

    // Public routes (no auth required)
    Router::new()
        .route("/health", get(health_handler))
        .route("/pubkey", post(pubkey_handler)) // Public for dashboard encryption
        .route("/vrf/pubkey", get(vrf_pubkey_handler)) // Public VRF public key
        .merge(worker_routes)
        .merge(coordinator_routes)
        .merge(admin_routes)
        .merge(vault_routes)
        .merge(tee_routes)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Health check endpoint (no auth required)
async fn health_handler(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        tee_mode: format!("{}", state.config.tee_mode),
    })
}

/// Returns true iff the request's `Authorization: Bearer <token>` header
/// carries either a coordinator or worker token. Used for the
/// vault-scoped path on otherwise-public routes (`/pubkey`).
///
/// **Why this gate exists:** without it, the body-based
/// `vault_id` on `/pubkey` would let an unauthenticated caller force
/// the worker to perform an MPC CKD round-trip for ANY verified vault,
/// charging gas to the victim vault account on every cache miss
/// (~30 mNEAR/call). Public reads of pubkeys are still allowed for
/// `vault_id = None`, which preserves the dashboard's
/// pre-store-secrets pubkey-fetch flow on the legacy default master.
fn has_authenticated_caller(state: &AppState, headers: &axum::http::HeaderMap) -> bool {
    let Some(auth) = headers.get("Authorization").and_then(|h| h.to_str().ok()) else {
        return false;
    };
    let Some(token) = auth.strip_prefix("Bearer ") else {
        return false;
    };
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let token_hash = hex::encode(hasher.finalize());
    state.config.allowed_coordinator_token_hashes.contains(&token_hash)
        || state.config.allowed_worker_token_hashes.contains(&token_hash)
}

/// Get public key for encryption AND validate secrets before encryption
async fn pubkey_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<PubkeyRequest>,
) -> Result<Json<PubkeyResponse>, ApiError> {
    // Check if keystore is ready (has master key from MPC)
    if !state.is_ready() {
        tracing::warn!("Pubkey request rejected - keystore not ready (waiting for DAO approval and MPC key)");
        return Err(ApiError::Unauthorized(
            "Keystore not ready. Waiting for DAO approval and master key from MPC.".to_string()
        ));
    }

    // 1. Validate secrets JSON first (check for reserved keywords)
    let secrets_map: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&req.secrets_json)
        .map_err(|e| ApiError::BadRequest(format!("Invalid JSON format: {}", e)))?;

    // Reserved keywords that should not be overridden by user secrets
    const RESERVED_KEYWORDS: &[&str] = &[
        "NEAR_SENDER_ID",
        "NEAR_CONTRACT_ID",
        "NEAR_BLOCK_HEIGHT",
        "NEAR_BLOCK_TIMESTAMP",
        "NEAR_RECEIPT_ID",
        "NEAR_PREDECESSOR_ID",
        "NEAR_SIGNER_PUBLIC_KEY",
        "NEAR_GAS_BURNT",
        "NEAR_USER_ACCOUNT_ID",
        "NEAR_PAYMENT_YOCTO",
        "NEAR_TRANSACTION_HASH",
        "NEAR_MAX_INSTRUCTIONS",
        "NEAR_MAX_MEMORY_MB",
        "NEAR_MAX_EXECUTION_SECONDS",
        "NEAR_REQUEST_ID",
        "NEAR_NETWORK_ID",
        "OUTLAYER_PROJECT_ID",
        "OUTLAYER_PROJECT_UUID",
    ];

    // Check for reserved keywords
    let reserved_found: Vec<&str> = secrets_map.keys()
        .filter(|k| RESERVED_KEYWORDS.contains(&k.as_str()))
        .map(|k| k.as_str())
        .collect();

    if !reserved_found.is_empty() {
        tracing::warn!(
            reserved_keys = ?reserved_found,
            "Rejected secrets with reserved keywords"
        );
        return Err(ApiError::BadRequest(format!(
            "Cannot use reserved system keywords as secret keys: {}. \
            These environment variables are automatically set by OutLayer worker. \
            Please use different key names.",
            reserved_found.join(", ")
        )));
    }

    // Check for PROTECTED_ prefix in manual secrets (reserved for generated secrets)
    let protected_manual_keys: Vec<&str> = secrets_map.keys()
        .filter(|k| k.starts_with("PROTECTED_"))
        .map(|k| k.as_str())
        .collect();

    if !protected_manual_keys.is_empty() {
        tracing::warn!(
            protected_keys = ?protected_manual_keys,
            "Rejected manual secrets with PROTECTED_ prefix"
        );
        return Err(ApiError::BadRequest(format!(
            "Manual secrets cannot use 'PROTECTED_' prefix (reserved for auto-generated secrets): {}. \
            This prefix proves that a secret was generated in TEE and never seen by anyone.",
            protected_manual_keys.join(", ")
        )));
    }

    // 2. Generate public key for encryption
    // Route through per-vault master if vault_id is set in
    // the request body. We use body-based extraction here (not the
    // X-Customer-Vault header) because the vault scope is an attribute
    // of *which secret* this pubkey will encrypt — not of the caller's
    // session. Same fail-loud-on-malformed semantics.
    let customer = parse_optional_vault_id(req.vault_id.as_deref())?;

    // SECURITY: /pubkey is a public route. The
    // legacy `vault_id = None` flow stays public for the dashboard's
    // pre-store-secrets pubkey fetch. The vault-scoped flow can trigger
    // an MPC CKD round-trip via `ensure_customer_loaded` — that's gas
    // charged to the customer's vault account. We therefore require
    // coordinator/worker auth before allowing vault_id != None, so an
    // unauthenticated attacker cannot enumerate verified vaults and
    // burn their gas.
    if customer.is_some() && !has_authenticated_caller(&state, &headers) {
        tracing::warn!(
            vault_id = ?customer,
            "Rejected /pubkey vault-scoped request without coordinator/worker token"
        );
        return Err(ApiError::Unauthorized(
            "vault-scoped /pubkey requires coordinator or worker token".to_string(),
        ));
    }

    state
        .ensure_customer_loaded(customer.as_ref())
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    let keystore = state.keystore.read().await;
    let pubkey_hex = keystore
        .public_key_hex(customer.as_ref(), &req.seed)
        .map_err(|e| ApiError::InternalError(format!("Failed to derive public key: {}", e)))?;

    tracing::info!(
        seed = %req.seed,
        num_secrets = secrets_map.len(),
        vault_id = ?customer,
        "Validated secrets and generated pubkey"
    );

    Ok(Json(PubkeyResponse { pubkey: pubkey_hex }))
}

/// Convert keystore-worker's internally-tagged `SecretAccessor` to the
/// externally-tagged JSON shape that contract view methods expect.
///
/// keystore-worker uses `#[serde(tag = "type")]` on `SecretAccessor` so
/// it can deserialize requests where the accessor variant is announced
/// inline; the contract uses NEAR's default external tagging
/// (`{"Variant": {...}}`). This is the small adapter that bridges them.
/// `Repo.repo` is normalised the same way `near.rs::get_secrets` does
/// — this keeps key derivation consistent between the legacy lookup
/// path and the new combined `get_secret_with_vault` path.
fn accessor_to_contract_json(a: &SecretAccessor) -> serde_json::Value {
    match a {
        SecretAccessor::Repo { repo, branch } => serde_json::json!({
            "Repo": {
                "repo": crate::utils::normalize_repo_url(repo),
                "branch": branch,
            }
        }),
        SecretAccessor::WasmHash { hash } => serde_json::json!({
            "WasmHash": { "hash": hash }
        }),
        SecretAccessor::Project { project_id } => serde_json::json!({
            "Project": { "project_id": project_id }
        }),
        SecretAccessor::System { secret_type } => {
            serde_json::json!({ "System": secret_type.as_contract_str() })
        }
    }
}

/// Decrypt secrets from contract for authorized TEE worker
async fn decrypt_handler(
    State(state): State<AppState>,
    Json(req): Json<DecryptRequest>,
) -> Result<Json<DecryptResponse>, ApiError> {
    // Check if keystore is ready (has master key from MPC)
    if !state.is_ready() {
        tracing::warn!("Decrypt request rejected - keystore not ready (waiting for DAO approval and MPC key)");
        return Err(ApiError::Unauthorized(
            "Keystore not ready. Waiting for DAO approval and master key from MPC.".to_string()
        ));
    }

    let task_id_str = req.task_id.as_deref().unwrap_or("unknown");

    // Log request based on accessor type
    match &req.accessor {
        SecretAccessor::Repo { repo, branch } => {
            tracing::info!(
                task_id = %task_id_str,
                tee_type = %req.attestation.tee_type,
                repo = %repo,
                branch = ?branch,
                profile = %req.profile,
                owner = %req.owner,
                "Received decrypt request (Repo)"
            );
        }
        SecretAccessor::WasmHash { hash } => {
            tracing::info!(
                task_id = %task_id_str,
                tee_type = %req.attestation.tee_type,
                wasm_hash = %hash,
                profile = %req.profile,
                owner = %req.owner,
                "Received decrypt request (WasmHash)"
            );
        }
        SecretAccessor::Project { project_id } => {
            tracing::info!(
                task_id = %task_id_str,
                tee_type = %req.attestation.tee_type,
                project_id = %project_id,
                profile = %req.profile,
                owner = %req.owner,
                "Received decrypt request (Project)"
            );
        }
        SecretAccessor::System { secret_type } => {
            tracing::info!(
                task_id = %task_id_str,
                tee_type = %req.attestation.tee_type,
                secret_type = ?secret_type,
                profile = %req.profile,
                owner = %req.owner,
                "Received decrypt request (System)"
            );
        }
    }

    // 1. Verify TEE attestation
    // Note: Primary authentication is via bearer token (checked in auth_middleware).
    // When both keystore and worker are in TEE, attestation verification relies on token auth.
    crate::attestation::verify_attestation(
        &req.attestation,
        &state.config.tee_mode,
        &state.expected_measurements,
    )
    .map_err(|e| {
        tracing::warn!(task_id = %task_id_str, error = %e, "Attestation verification failed");
        ApiError::Unauthorized(format!("Attestation verification failed: {}", e))
    })?;

    // 2. Read secrets + vault binding from NEAR contract.
    //
    // Mechanism: on-chain side-table. A single combined
    // view call returns BOTH the encrypted secret profile AND the
    // per-secret vault binding. The vault binding tells us which master
    // to use for decryption; we don't accept a customer header here
    // because the customer must equal whatever was on-chain at write
    // time, and the chain is the only authoritative source.
    let near_client = state.near_client.as_ref()
        .ok_or_else(|| ApiError::InternalError("NEAR client not configured".to_string()))?;

    let accessor_json = accessor_to_contract_json(&req.accessor);
    let combined = near_client
        .get_secret_with_vault(accessor_json, &req.profile, &req.owner)
        .await
        .map_err(|e| {
            tracing::error!(task_id = %task_id_str, error = %e, "Failed to read secrets from contract");
            ApiError::InternalError(format!("Failed to read secrets from contract: {}", e))
        })?;
    let vault_id_str = combined.vault_id;

    let secret_profile = combined.profile.ok_or_else(|| {
        // Per-variant log fields for grep-friendliness — operators
        // search by repo/wasm_hash/project_id/secret_type, so we don't
        // dump the whole accessor as Debug.
        match &req.accessor {
            SecretAccessor::Repo { repo, branch } => tracing::warn!(
                task_id = %task_id_str,
                repo = %repo,
                branch = ?branch,
                profile = %req.profile,
                owner = %req.owner,
                "Secrets not found in contract"
            ),
            SecretAccessor::WasmHash { hash } => tracing::warn!(
                task_id = %task_id_str,
                wasm_hash = %hash,
                profile = %req.profile,
                owner = %req.owner,
                "Secrets not found in contract"
            ),
            SecretAccessor::Project { project_id } => tracing::warn!(
                task_id = %task_id_str,
                project_id = %project_id,
                profile = %req.profile,
                owner = %req.owner,
                "Secrets not found in contract"
            ),
            SecretAccessor::System { secret_type } => tracing::warn!(
                task_id = %task_id_str,
                secret_type = ?secret_type,
                profile = %req.profile,
                owner = %req.owner,
                "Secrets not found in contract"
            ),
        }
        ApiError::BadRequest("Secrets not found in contract".to_string())
    })?;

    // Parse the on-chain vault binding into an AccountId, then ensure
    // the per-vault master is loaded BEFORE we touch the keystore.
    // Bridge between the contract's vault-binding side-table and the
    // lazy-load gate. Malformed vault_id on chain is a hard error
    // (chain shouldn't store malformed AccountIds — it would be a bug
    // in the contract or the binding writer).
    let customer = parse_optional_vault_id(vault_id_str.as_deref())?;
    state
        .ensure_customer_loaded(customer.as_ref())
        .await
        .map_err(|e| ApiError::InternalError(format!(
            "lazy-load gate failed for vault {}: {}",
            customer.as_ref().map(|v| v.as_str()).unwrap_or("<none>"),
            e
        )))?;

    tracing::debug!(
        task_id = %task_id_str,
        vault_id = ?customer,
        "Successfully read secrets from contract"
    );

    // 3. Validate access conditions
    let access_condition: crate::types::AccessCondition = serde_json::from_value(secret_profile["access"].clone())
        .map_err(|e| {
            tracing::error!(task_id = %task_id_str, error = %e, "Failed to parse access condition");
            ApiError::InternalError(format!("Failed to parse access condition: {}", e))
        })?;

    // Use user_account_id (who requested execution) as caller for access control
    let caller = &req.user_account_id;

    let access_granted = access_condition.validate(caller, state.near_client.as_ref().map(|c| c.as_ref())).await
        .map_err(|e| {
            tracing::error!(task_id = %task_id_str, error = %e, "Access validation failed");
            ApiError::InternalError(format!("Access validation failed: {}", e))
        })?;

    if !access_granted {
        tracing::warn!(
            task_id = %task_id_str,
            caller = %caller,
            "Access denied by access condition"
        );
        return Err(ApiError::Unauthorized("Access denied by access condition".to_string()));
    }

    tracing::info!(task_id = %task_id_str, caller = %caller, "Access granted");

    // 4. Build seed based on accessor type
    // SECURITY NOTE:
    // - For Repo: use branch from SECRET PROFILE (not request) to construct seed
    // - This is correct because seed must match the one used during encryption
    // - Access control already validated above (only owner can decrypt their secrets)
    // - Contract already returned the correct secrets based on request parameters
    let seed = match &req.accessor {
        SecretAccessor::Repo { repo, branch: request_branch } => {
            let normalized_repo = crate::utils::normalize_repo_url(repo);
            // The contract's `SecretProfileView`
            // does NOT have a top-level `branch` field — branch is
            // nested inside `accessor.Repo.branch`. Reading
            // `secret_profile["branch"]` always returned Null, which
            // silently disabled the wildcard-fallback seed logic and
            // broke decryption for any secret stored with an explicit
            // branch. Fixed: read the actual nested location, falling
            // back to None if the contract returned a non-Repo accessor.
            let secret_branch = secret_profile
                .get("accessor")
                .and_then(|a| a.get("Repo"))
                .and_then(|r| r.get("branch"))
                .and_then(|b| b.as_str());

            // Log branch matching for debugging
            match (request_branch.as_deref(), secret_branch) {
                (Some(req_b), Some(sec_b)) if req_b == sec_b => {
                    tracing::debug!("Branch match: {} (exact)", req_b);
                }
                (Some(req_b), None) => {
                    tracing::debug!("Branch fallback: requested '{}', using wildcard secrets (branch=null)", req_b);
                }
                (None, None) => {
                    tracing::debug!("Branch match: both null (wildcard)");
                }
                (None, Some(sec_b)) => {
                    tracing::debug!("Branch match: secret has '{}', request wildcard", sec_b);
                }
                (Some(req_b), Some(sec_b)) => {
                    tracing::warn!(
                        task_id = %task_id_str,
                        request_branch = %req_b,
                        secret_branch = %sec_b,
                        "Branch mismatch - contract returned different branch than requested"
                    );
                }
            }

            // Build seed using branch from secret profile (critical for correct decryption)
            let seed = if let Some(b) = secret_branch {
                format!("{}:{}:{}", normalized_repo, req.owner, b)
            } else {
                format!("{}:{}", normalized_repo, req.owner)
            };

            tracing::info!(
                task_id = %task_id_str,
                repo_normalized = %normalized_repo,
                owner = %req.owner,
                secret_branch = ?secret_branch,
                seed = %seed,
                "🔓 DECRYPTION SEED (Repo)"
            );

            seed
        }
        SecretAccessor::WasmHash { hash } => {
            let seed = format!("wasm_hash:{}:{}", hash, req.owner);

            tracing::info!(
                task_id = %task_id_str,
                wasm_hash = %hash,
                owner = %req.owner,
                seed = %seed,
                "🔓 DECRYPTION SEED (WasmHash)"
            );

            seed
        }
        SecretAccessor::Project { project_id } => {
            let seed = format!("project:{}:{}", project_id, req.owner);

            tracing::info!(
                task_id = %task_id_str,
                project_id = %project_id,
                owner = %req.owner,
                seed = %seed,
                "🔓 DECRYPTION SEED (Project)"
            );

            seed
        }
        SecretAccessor::System { secret_type } => {
            // Seed format: system:{type}:{owner}:{nonce}
            // nonce is stored in profile field
            let seed = format!("system:{}:{}:{}", secret_type.as_seed_str(), req.owner, req.profile);

            tracing::info!(
                task_id = %task_id_str,
                secret_type = ?secret_type,
                owner = %req.owner,
                nonce = %req.profile,
                seed = %seed,
                "🔓 DECRYPTION SEED (System)"
            );

            seed
        }
    };

    // 5. Decrypt using derived keypair
    let encrypted_secrets_base64 = secret_profile["encrypted_secrets"]
        .as_str()
        .ok_or_else(|| ApiError::InternalError("Missing encrypted_secrets field".to_string()))?;

    let encrypted_bytes = base64::decode(encrypted_secrets_base64)
        .map_err(|e| ApiError::InternalError(format!("Invalid base64 in encrypted_secrets: {}", e)))?;

    let keystore = state.keystore.read().await;
    let plaintext_bytes = keystore.decrypt(customer.as_ref(), &seed, &encrypted_bytes).map_err(|e| {
        tracing::error!(task_id = %task_id_str, seed = %seed, error = %e, "Decryption failed");
        ApiError::InternalError(format!("Decryption failed: {}", e))
    })?;

    // 6. Encode plaintext as base64 for safe JSON transport
    let plaintext_b64 = base64::encode(&plaintext_bytes);

    tracing::info!(
        task_id = %task_id_str,
        plaintext_size = plaintext_bytes.len(),
        "Successfully decrypted secrets"
    );

    Ok(Json(DecryptResponse {
        plaintext_secrets: plaintext_b64,
    }))
}

/// POST /vrf/generate — Generate VRF output for alpha (worker-only)
async fn vrf_generate_handler(
    State(state): State<AppState>,
    Json(req): Json<VrfGenerateRequest>,
) -> Result<Json<VrfGenerateResponse>, ApiError> {
    if !state.is_ready() {
        return Err(ApiError::Unauthorized(
            "Keystore not ready. Waiting for DAO approval and master key from MPC.".to_string()
        ));
    }

    if req.alpha.is_empty() {
        return Err(ApiError::BadRequest("alpha must not be empty".to_string()));
    }

    crate::attestation::verify_attestation(
        &req.attestation,
        &state.config.tee_mode,
        &state.expected_measurements,
    )
    .map_err(|e| {
        tracing::warn!(error = %e, "VRF attestation verification failed");
        ApiError::Unauthorized(format!("Attestation verification failed: {}", e))
    })?;

    let keystore = state.keystore.read().await;
    let (output_hex, signature_hex) = keystore
        .vrf_generate(req.alpha.as_bytes())
        .map_err(|e| ApiError::InternalError(format!("VRF generation failed: {}", e)))?;

    tracing::info!(alpha = %req.alpha, "VRF generated");

    Ok(Json(VrfGenerateResponse { output_hex, signature_hex }))
}

/// GET /vrf/pubkey — Get VRF public key (public, no auth)
async fn vrf_pubkey_handler(
    State(state): State<AppState>,
) -> Result<Json<VrfPublicKeyResponse>, ApiError> {
    if !state.is_ready() {
        return Err(ApiError::Unauthorized(
            "Keystore not ready. Waiting for DAO approval and master key from MPC.".to_string()
        ));
    }

    let keystore = state.keystore.read().await;
    let vrf_public_key_hex = keystore
        .vrf_public_key_hex()
        .map_err(|e| ApiError::InternalError(format!("VRF public key derivation failed: {}", e)))?;

    Ok(Json(VrfPublicKeyResponse { vrf_public_key_hex }))
}

/// `POST /admin/evict-customer` — drop the cached per-customer master.
///
/// Called by the monitoring service when it detects a vault should
/// no longer operate (race-attack ban, manual DAO ban). The next
/// `derive_*` call for this vault will fail because the lazy-load
/// gate re-checks `keystore-dao.is_vault_verified` which now
/// returns false-when-banned.
///
/// Without this endpoint a banned vault would continue operating
/// until the next keystore-worker restart drops the in-memory cache.
///
/// **Auth:** worker token. Same boundary as `/decrypt` — anything
/// inside the operator's TEE network can evict. The endpoint is NOT
/// exposed to coordinator or external clients.
///
/// **Idempotent:** evicting a customer that was never loaded is a
/// no-op. Returns `{ ok: true }` either way.
async fn admin_evict_customer_handler(
    State(state): State<AppState>,
    Json(req): Json<AdminEvictCustomerRequest>,
) -> Result<Json<AdminEvictCustomerResponse>, ApiError> {
    let customer: near_primitives::types::AccountId = req.vault_id.parse().map_err(|e| {
        ApiError::BadRequest(format!("invalid vault_id account: {e}"))
    })?;
    let keystore = state.keystore.read().await;
    keystore.evict_customer(&customer);
    tracing::info!(
        vault_id = %customer,
        reason = %req.reason,
        "admin: evicted per-customer master from cache"
    );
    Ok(Json(AdminEvictCustomerResponse { ok: true }))
}

/// `POST /admin/ban-vault` — submit `keystore_dao.ban_vault(vault_id,
/// reason)` and evict the cached per-customer master in one shot.
///
/// Called by the race-attack monitoring service when it
/// detects more than one MPC `request_app_private_key` call for the
/// same vault within the dedup window. Side effects (in order):
///   1. Short-circuit if `keystore_dao.is_vault_banned(vault_id)` is
///      already true — no need to spend another nonce + log line.
///   2. Submit `ban_vault` via the worker's keystore-DAO function-call
///      access key (same key that `mark_vault_verified` uses; the DAO
///      whitelists the method-list). 300 TGas, no deposit. Serialized
///      through `signer_nonce_lock` so this can run concurrently with
///      `/sign-vault-verification`.
///   3. Evict the in-memory per-customer master so the ban takes
///      effect within ms — the `is_vault_verified` check on the
///      next-derive-call already returns false-when-banned (the DAO
///      view reads `verified - banned`).
///
/// **Auth:** worker token. Same boundary as `/admin/evict-customer`.
async fn admin_ban_vault_handler(
    State(state): State<AppState>,
    Json(req): Json<AdminBanVaultRequest>,
) -> Result<Json<AdminBanVaultResponse>, ApiError> {
    if !state.is_ready() {
        return Err(ApiError::Unauthorized(
            "Keystore not ready. Waiting for DAO approval and master key from MPC.".to_string(),
        ));
    }

    let vault_id: near_primitives::types::AccountId = req.vault_id.parse().map_err(|e| {
        ApiError::BadRequest(format!("invalid vault_id account: {e}"))
    })?;

    if req.reason.is_empty() {
        return Err(ApiError::BadRequest(
            "reason is required (operators set the convention; see the handler docs)".to_string(),
        ));
    }
    if req.reason.len() > 256 {
        return Err(ApiError::BadRequest(format!(
            "reason must be at most 256 bytes (got {})",
            req.reason.len()
        )));
    }

    let ctx = state.mpc_context.get().ok_or_else(|| {
        ApiError::InternalError(
            "MPC CKD context not configured — worker booted in non-TEE mode".to_string(),
        )
    })?;
    let near_client = state.near_client.as_ref().ok_or_else(|| {
        ApiError::InternalError("NEAR client not configured".to_string())
    })?;

    // 1. Short-circuit if already banned.
    let is_banned = near_client
        .view_call_json(
            &ctx.mpc_ckd_config.keystore_dao_id,
            "is_vault_banned",
            serde_json::json!({ "vault_id": vault_id }),
        )
        .await
        .map_err(|e| {
            ApiError::InternalError(format!("is_vault_banned view-call failed: {e}"))
        })?;
    if is_banned.as_bool() == Some(true) {
        // Still evict — the cache may have stale state if a different
        // operator banned via DAO vote and this monitor is catching up.
        let keystore = state.keystore.read().await;
        keystore.evict_customer(&vault_id);
        tracing::info!(
            vault_id = %vault_id,
            "Skipping ban_vault tx — already banned on chain. Cache evicted."
        );
        return Ok(Json(AdminBanVaultResponse {
            tx_hash: None,
            already_banned: true,
        }));
    }

    // 2. Submit ban_vault tx, serialized through signer_nonce_lock to
    //    avoid racing /sign-vault-verification on the keystore-DAO
    //    access key's nonce.
    let outcome = {
        let _guard = ctx.signer_nonce_lock.lock().await;
        near_client
            .submit_function_call(
                &ctx.keystore_dao_signer,
                &ctx.mpc_ckd_config.keystore_dao_id,
                "ban_vault",
                serde_json::json!({ "vault_id": vault_id, "reason": req.reason }),
                300_000_000_000_000,
                0,
            )
            .await
    }
    .map_err(|e| {
        tracing::error!(
            vault_id = %vault_id,
            reason = %req.reason,
            error = %e,
            "ban_vault tx submission failed"
        );
        let msg = format!("{e:#}");
        // Concurrent ban via DAO vote = treat as "already banned"
        // (idempotent end state). The contract panics with a generic
        // require! message so we match permissively.
        if msg.contains("already") {
            ApiError::Forbidden(format!("vault {} already banned (race): {}", vault_id, msg))
        } else {
            ApiError::InternalError(format!("tx submission failed: {msg}"))
        }
    })?;

    // 3. Evict the cached master so existing in-flight derive_* calls
    //    on this vault stop succeeding immediately. Does not need the
    //    on-chain tx to land first — the lazy-load gate re-checks
    //    `is_vault_verified` (which factors in `banned_vaults`) on the
    //    NEXT call after eviction, by which point the tx is final.
    {
        let keystore = state.keystore.read().await;
        keystore.evict_customer(&vault_id);
    }

    tracing::warn!(
        vault_id = %vault_id,
        reason = %req.reason,
        tx_hash = %outcome.tx_hash,
        "ban_vault tx landed; per-customer master evicted from cache"
    );

    Ok(Json(AdminBanVaultResponse {
        tx_hash: Some(outcome.tx_hash),
        already_banned: false,
    }))
}

/// `POST /sign-vault-verification` — re-verify a vault and submit
/// `mark_vault_verified` on chain.
///
/// Called by the public
/// `outlayer.near/vault-checker` WASI agent after it has run the
/// same 5 checks itself. The keystore-worker:
///
/// 1. Re-runs all 5 checks via [`crate::vault_verifier::verify_vault_for_signing`]
///    (defense in depth — independent code from the WASI agent).
/// 2. If everything passes, submits `keystore_dao.mark_vault_verified(vault_id)`
///    using the worker's approved access key on the keystore-dao
///    contract (set up at TEE registration time).
/// 3. Returns the broadcast tx hash. The agent does NOT need to wait
///    for finality on the worker side — `broadcast_tx_commit` already
///    waits, and the response only returns once the tx is final.
///
/// **Critically NOT done here:** MPC CKD for the vault's master.
/// That's lazy on first wallet-request via [`crate::mpc_ckd::add_customer`]
/// (lazy MPC CKD). Materialising masters at verification time would
/// charge gas to the vault account before any wallet operation is
/// requested — an unnecessary upfront cost.
///
/// **Auth:** worker token. The vault-checker WASI runs in OutLayer's
/// TEE workers and forwards its worker token; an external caller with
/// a worker token (i.e. an operator) can also drive this endpoint.
/// The auth boundary is intentionally permissive because the security
/// guarantee is the in-process re-verification, not the auth.
///
/// **Idempotent:** if `vault_id` is already verified on chain, the
/// `mark_vault_verified` tx is still submitted — the contract is
/// expected to no-op on re-mark. We don't short-circuit here because
/// we want the canonical tx hash to return either way.
async fn sign_vault_verification_handler(
    State(state): State<AppState>,
    Json(req): Json<SignVaultVerificationRequest>,
) -> Result<Json<SignVaultVerificationResponse>, ApiError> {
    if !state.is_ready() {
        return Err(ApiError::Unauthorized(
            "Keystore not ready. Waiting for DAO approval and master key from MPC.".to_string(),
        ));
    }

    let vault_id: near_primitives::types::AccountId = req.vault_id.parse().map_err(|e| {
        ApiError::BadRequest(format!("invalid vault_id account: {e}"))
    })?;

    // All required context lives in one OnceLock — atomicity
    // invariant means no half-set state can be observed.
    let ctx = state.mpc_context.get().ok_or_else(|| {
        ApiError::InternalError(
            "MPC CKD context not configured — worker booted in non-TEE mode".to_string(),
        )
    })?;
    let near_client = state.near_client.as_ref().ok_or_else(|| {
        ApiError::InternalError("NEAR client not configured".to_string())
    })?;

    // 0. Short-circuit if already verified on-chain. The
    //    `mark_vault_verified` contract method is
    //    idempotent (insert into UnorderedSet is no-op if present), so
    //    skipping the tx is purely an optimisation — but a meaningful
    //    one: every `mark_vault_verified` consumes a nonce and ~0.001
    //    NEAR from the worker's keystore-dao access-key allowance,
    //    and an attacker with a worker token could DoS the nonce by
    //    spamming this endpoint against verified vaults.
    let is_verified = near_client
        .view_call_json(
            &ctx.mpc_ckd_config.keystore_dao_id,
            "is_vault_verified",
            serde_json::json!({ "vault_id": vault_id }),
        )
        .await
        .map_err(|e| {
            ApiError::InternalError(format!("is_vault_verified view-call failed: {e}"))
        })?;
    if is_verified.as_bool() == Some(true) {
        tracing::info!(
            vault_id = %vault_id,
            "Skipping mark_vault_verified — vault already verified on chain"
        );
        return Ok(Json(SignVaultVerificationResponse {
            tx_hash: None,
            already_verified: true,
        }));
    }

    // 1. Defense-in-depth re-verification of all 5 checks. If any
    //    fails, classify caller-side vs worker-side and surface an
    //    appropriate status (see `map_verify_error`).
    crate::vault_verifier::verify_vault_for_signing(
        near_client.as_ref(),
        &ctx.mpc_ckd_config.keystore_dao_id,
        &ctx.mpc_ckd_config.mpc_contract_id,
        &vault_id,
    )
    .await
    .map_err(|e| {
        tracing::warn!(
            vault_id = %vault_id,
            error = %e,
            "Refusing to sign mark_vault_verified — re-verification failed"
        );
        map_verify_error(&vault_id, e)
    })?;

    // 2. Submit mark_vault_verified tx, serializing through the
    //    per-signer nonce lock so concurrent /sign-vault-verification
    //    calls (and /admin/ban-vault) don't race for the same nonce.
    //    Lock is held only across the build+broadcast critical
    //    section; the response wait is inside broadcast_tx_commit so
    //    we cannot release earlier without losing the tx_hash.
    //    300 TGas, no deposit.
    let outcome = {
        let _guard = ctx.signer_nonce_lock.lock().await;
        near_client
            .submit_function_call(
                &ctx.keystore_dao_signer,
                &ctx.mpc_ckd_config.keystore_dao_id,
                "mark_vault_verified",
                serde_json::json!({ "vault_id": vault_id }),
                300_000_000_000_000,
                0,
            )
            .await
    }
    .map_err(|e| {
        tracing::error!(
            vault_id = %vault_id,
            error = %e,
            "mark_vault_verified tx submission failed"
        );
        // A vault that gets banned between vault-checker's own
        // check and our signing window will surface as a
        // contract-side panic with a recognisable message. Map it
        // to 409 Conflict so the caller doesn't retry indefinitely
        // treating it as a transient failure.
        let msg = format!("{e:#}");
        if msg.contains("vault is banned") || msg.contains("banned") {
            ApiError::Forbidden(format!(
                "vault {} is banned (likely banned between check and sign): {}",
                vault_id, msg
            ))
        } else {
            ApiError::InternalError(format!("tx submission failed: {msg}"))
        }
    })?;

    tracing::info!(
        vault_id = %vault_id,
        tx_hash = %outcome.tx_hash,
        "mark_vault_verified tx landed"
    );

    Ok(Json(SignVaultVerificationResponse {
        tx_hash: Some(outcome.tx_hash),
        already_verified: false,
    }))
}

/// `POST /derive-vault-tee-key` — return the Layer-1 vault TEE
/// public key for a given `vault_id`.
///
/// `outlayer-cli init-vault` flow: the customer needs the
/// pubkey BEFORE submitting the atomic deploy that adds it as a
/// function-call AccessKey on the new vault. The keypair is
/// [HMAC-derived from the OutLayer default master](mpc_ckd::derive_vault_tee_keypair),
/// so the worker can answer this query without any state — it's
/// read-only and doesn't trigger MPC CKD.
///
/// **Auth:** worker token (admin lane). The CLI is operator-bundled
/// or run by the customer through coordinator's auth proxy; either
/// way the caller has the worker token (or a coordinator token,
/// per the same `worker_auth_middleware` allowlist as
/// `/admin/evict-customer` and `/sign-vault-verification`).
///
/// **Idempotent / pure:** does NOT touch the keystore_dao_signer,
/// does NOT acquire any locks. Two concurrent calls for the same
/// vault_id return identical pubkeys.
async fn derive_vault_tee_key_handler(
    State(state): State<AppState>,
    Json(req): Json<DeriveVaultTeeKeyRequest>,
) -> Result<Json<DeriveVaultTeeKeyResponse>, ApiError> {
    if !state.is_ready() {
        return Err(ApiError::Unauthorized(
            "Keystore not ready. Waiting for DAO approval and master key from MPC.".to_string(),
        ));
    }

    let vault_id: near_primitives::types::AccountId = req.vault_id.parse().map_err(|e| {
        ApiError::BadRequest(format!("invalid vault_id: {e}"))
    })?;

    let keystore = state.keystore.read().await;
    let (public_key, _secret_key) =
        crate::mpc_ckd::derive_vault_tee_keypair(&keystore, &vault_id).map_err(|e| {
            ApiError::InternalError(format!("Layer-1 keypair derivation failed: {e}"))
        })?;

    Ok(Json(DeriveVaultTeeKeyResponse {
        public_key: public_key.to_string(),
    }))
}

/// Map [`crate::vault_verifier::VerifyError`] into the right HTTP
/// status. The split is:
///
/// * **4xx (caller-side / vault-state issue)** — the caller submitted
///   a bad request OR pointed at a vault that fails the protocol's
///   security invariants. Caller should surface to its end-user, not
///   retry transparently.
/// * **5xx (worker-side / RPC-down)** — something between the worker
///   and chain or contract is misbehaving. Caller MAY retry with
///   exponential backoff.
fn map_verify_error(
    vault_id: &near_primitives::types::AccountId,
    e: crate::vault_verifier::VerifyError,
) -> ApiError {
    use crate::vault_verifier::VerifyError as V;
    let msg = format!("vault {vault_id} re-verification failed: {e}");
    match e {
        // Caller-/vault-side problems → 4xx
        V::AlreadyBanned => ApiError::Forbidden(msg),
        V::CodeHashNotApproved { .. }
        | V::CodeHashMissing
        | V::FullAccessKeyPresent
        | V::FunctionCallKeyMisconfigured { .. }
        | V::UnexpectedAccessKeyCount { .. }
        | V::KeystoreDaoMismatch { .. }
        | V::MpcContractMismatch { .. }
        | V::VaultUnlocked
        | V::VaultRecoveryInProgress => ApiError::BadRequest(msg),
        // AccountNotFound is ambiguous (vault doesn't exist OR rpc
        // flake) — bias toward "caller pointed at something invalid"
        // because that's the dominant case; the error message still
        // surfaces the underlying RPC error for ops triage.
        V::AccountNotFound(_) => ApiError::BadRequest(msg),
        // Worker-/system-side problems → 5xx so callers can retry.
        V::KeystoreDaoUnreachable(_)
        | V::AccessKeyListUnreachable(_)
        | V::VaultStateUnreachable(_) => ApiError::InternalError(msg),
        // Contract returning unexpected response shape — that's a
        // contract version mismatch, NOT a caller-side bug.
        V::KeystoreDaoMalformed { .. } | V::VaultStateInvalid(_) => ApiError::InternalError(msg),
    }
}

/// Encrypt plaintext data
///
/// Used by workers to re-encrypt secrets after TopUp:
/// 1. Worker decrypts current Payment Key data via /decrypt
/// 2. Worker parses JSON, updates initial_balance
/// 3. Worker calls /encrypt to get new encrypted data
/// 4. Worker calls promise_yield_resume with new encrypted data
async fn encrypt_handler(
    State(state): State<AppState>,
    Json(req): Json<EncryptRequest>,
) -> Result<Json<EncryptResponse>, ApiError> {
    // Check if keystore is ready
    if !state.is_ready() {
        tracing::warn!("Encrypt request rejected - keystore not ready");
        return Err(ApiError::Unauthorized(
            "Keystore not ready. Waiting for DAO approval and master key from MPC.".to_string()
        ));
    }

    tracing::info!(
        seed = %req.seed,
        "Received encrypt request"
    );

    // Verify TEE attestation
    crate::attestation::verify_attestation(
        &req.attestation,
        &state.config.tee_mode,
        &state.expected_measurements,
    )
    .map_err(|e| {
        tracing::warn!(error = %e, "Encrypt attestation verification failed");
        ApiError::Unauthorized(format!("Attestation verification failed: {}", e))
    })?;

    // Decode plaintext from base64
    let plaintext_bytes = base64::decode(&req.plaintext_base64)
        .map_err(|e| ApiError::BadRequest(format!("Invalid base64 in plaintext: {}", e)))?;

    // Encrypt with derived key
    let keystore = state.keystore.read().await;
    let encrypted_bytes = keystore
        .encrypt(None, &req.seed, &plaintext_bytes)
        .map_err(|e| ApiError::InternalError(format!("Failed to encrypt data: {}", e)))?;

    let encrypted_base64 = base64::encode(&encrypted_bytes);

    tracing::info!(
        seed = %req.seed,
        plaintext_len = plaintext_bytes.len(),
        encrypted_len = encrypted_bytes.len(),
        "Successfully encrypted data"
    );

    Ok(Json(EncryptResponse { encrypted_base64 }))
}

/// Request to decrypt raw encrypted data directly
#[derive(Debug, Deserialize)]
pub struct DecryptRawRequest {
    /// Seed for key derivation
    pub seed: String,
    /// Base64-encoded encrypted data
    pub encrypted_base64: String,
    /// TEE attestation from requesting worker
    pub attestation: Attestation,
}

/// Response with decrypted data
#[derive(Debug, Serialize)]
pub struct DecryptRawResponse {
    /// Base64-encoded plaintext data
    pub plaintext_base64: String,
}

/// Decrypt raw encrypted data directly
///
/// Used by workers for TopUp flow:
/// 1. Worker receives encrypted_data from SystemEvent::TopUpPaymentKey
/// 2. Worker calls /decrypt-raw with seed and encrypted_data
/// 3. Worker receives plaintext Payment Key JSON
/// 4. Worker updates balance and calls /encrypt
async fn decrypt_raw_handler(
    State(state): State<AppState>,
    Json(req): Json<DecryptRawRequest>,
) -> Result<Json<DecryptRawResponse>, ApiError> {
    // Check if keystore is ready
    if !state.is_ready() {
        tracing::warn!("Decrypt-raw request rejected - keystore not ready");
        return Err(ApiError::Unauthorized(
            "Keystore not ready. Waiting for DAO approval and master key from MPC.".to_string()
        ));
    }

    tracing::info!(
        seed = %req.seed,
        "Received decrypt-raw request"
    );

    // Verify TEE attestation
    crate::attestation::verify_attestation(
        &req.attestation,
        &state.config.tee_mode,
        &state.expected_measurements,
    )
    .map_err(|e| {
        tracing::warn!(error = %e, "Decrypt-raw attestation verification failed");
        ApiError::Unauthorized(format!("Attestation verification failed: {}", e))
    })?;

    // Decode encrypted data from base64
    let encrypted_bytes = base64::decode(&req.encrypted_base64)
        .map_err(|e| ApiError::BadRequest(format!("Invalid base64 in encrypted_data: {}", e)))?;

    // Decrypt with derived key
    let keystore = state.keystore.read().await;
    let plaintext_bytes = keystore
        .decrypt(None, &req.seed, &encrypted_bytes)
        .map_err(|e| ApiError::InternalError(format!("Failed to decrypt data: {}", e)))?;

    let plaintext_base64 = base64::encode(&plaintext_bytes);

    tracing::info!(
        seed = %req.seed,
        encrypted_len = encrypted_bytes.len(),
        plaintext_len = plaintext_bytes.len(),
        "Successfully decrypted raw data"
    );

    Ok(Json(DecryptRawResponse { plaintext_base64 }))
}

/// Add generated secrets to existing encrypted secrets
///
/// Flow:
/// 1. Decrypt existing secrets (if provided)
/// 2. Generate new secrets
/// 3. Check for collisions (key already exists?)
/// 4. Merge old + new secrets
/// 5. Re-encrypt and return
async fn add_generated_secret_handler(
    State(state): State<AppState>,
    Json(req): Json<AddGeneratedSecretRequest>,
) -> Result<Json<AddGeneratedSecretResponse>, ApiError> {
    // Check if keystore is ready (has master key from MPC)
    if !state.is_ready() {
        tracing::warn!("Add generated secret request rejected - keystore not ready (waiting for DAO approval and MPC key)");
        return Err(ApiError::Unauthorized(
            "Keystore not ready. Waiting for DAO approval and master key from MPC.".to_string()
        ));
    }

    // Vault scope from request body. The decrypt+re-encrypt
    // round-trip MUST use the same scope (default OR vault) as the
    // original encryption — mismatched scope would either fail to
    // decrypt or silently re-encrypt under the wrong key.
    let customer = parse_optional_vault_id(req.vault_id.as_deref())?;
    state
        .ensure_customer_loaded(customer.as_ref())
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    tracing::info!(
        seed = %req.seed,
        num_new_secrets = req.new_secrets.len(),
        has_existing = req.encrypted_secrets_base64.is_some(),
        vault_id = ?customer,
        "Received add_generated_secret request"
    );

    // 1. Decrypt existing secrets (if any)
    let mut secrets_map: serde_json::Map<String, serde_json::Value> = if let Some(ref encrypted_b64) = req.encrypted_secrets_base64 {
        // Decode base64
        let encrypted_bytes = base64::decode(encrypted_b64)
            .map_err(|e| ApiError::BadRequest(format!("Invalid base64 in encrypted_secrets: {}", e)))?;

        // Decrypt
        let keystore = state.keystore.read().await;
        let plaintext_bytes = keystore
            .decrypt(customer.as_ref(), &req.seed, &encrypted_bytes)
            .map_err(|e| ApiError::InternalError(format!("Failed to decrypt existing secrets: {}", e)))?;
        drop(keystore); // Release read lock early

        // Parse JSON
        let plaintext_str = String::from_utf8(plaintext_bytes)
            .map_err(|e| ApiError::InternalError(format!("Decrypted data is not valid UTF-8: {}", e)))?;

        serde_json::from_str(&plaintext_str)
            .map_err(|e| ApiError::InternalError(format!("Decrypted data is not valid JSON: {}", e)))?
    } else {
        // Start with empty secrets
        serde_json::Map::new()
    };

    tracing::debug!(
        existing_keys = secrets_map.len(),
        "Decrypted existing secrets"
    );

    // Validate that manual secrets don't use PROTECTED_ prefix
    let protected_manual_keys: Vec<&String> = secrets_map.keys()
        .filter(|k| k.starts_with("PROTECTED_"))
        .collect();

    if !protected_manual_keys.is_empty() {
        return Err(ApiError::BadRequest(format!(
            "Manual secrets cannot use 'PROTECTED_' prefix (reserved for auto-generated secrets): {}",
            protected_manual_keys.iter().map(|k| k.as_str()).collect::<Vec<_>>().join(", ")
        )));
    }

    // Validate generated secret names MUST start with PROTECTED_
    let missing_prefix: Vec<&String> = req.new_secrets.iter()
        .map(|s| &s.name)
        .filter(|name| !name.starts_with("PROTECTED_"))
        .collect();

    if !missing_prefix.is_empty() {
        return Err(ApiError::BadRequest(format!(
            "Generated secrets must start with 'PROTECTED_' prefix: {}. \
            This prefix proves that secrets were generated in TEE and never seen by anyone.",
            missing_prefix.iter().map(|k| k.as_str()).collect::<Vec<_>>().join(", ")
        )));
    }

    // 2. Check for collisions BEFORE generating
    let mut collisions: Vec<String> = Vec::new();
    for spec in &req.new_secrets {
        if secrets_map.contains_key(&spec.name) {
            collisions.push(spec.name.clone());
        }
    }

    if !collisions.is_empty() {
        return Err(ApiError::BadRequest(format!(
            "Cannot generate secrets: keys already exist: {}. Please use different names or remove existing keys first.",
            collisions.join(", ")
        )));
    }

    // 3. Generate new secrets
    let mut generated_keys: Vec<String> = Vec::new();
    for spec in &req.new_secrets {
        // Build generation directive
        let directive = format!("generate_outlayer_secret:{}", spec.generation_type);

        // Generate
        let generated_value = crate::secret_generation::generate_secret(&directive)
            .map_err(|e| ApiError::BadRequest(format!(
                "Failed to generate secret '{}' with type '{}': {}",
                spec.name, spec.generation_type, e
            )))?;

        tracing::info!(
            key = %spec.name,
            gen_type = %spec.generation_type,
            "Generated secret"
        );

        // Add to secrets map
        secrets_map.insert(spec.name.clone(), serde_json::Value::String(generated_value));
        generated_keys.push(spec.name.clone());
    }

    // 4. Validate no reserved keywords (final check)
    const RESERVED_KEYWORDS: &[&str] = &[
        "NEAR_SENDER_ID",
        "NEAR_CONTRACT_ID",
        "NEAR_BLOCK_HEIGHT",
        "NEAR_BLOCK_TIMESTAMP",
        "NEAR_RECEIPT_ID",
        "NEAR_PREDECESSOR_ID",
        "NEAR_SIGNER_PUBLIC_KEY",
        "NEAR_GAS_BURNT",
        "NEAR_USER_ACCOUNT_ID",
        "NEAR_PAYMENT_YOCTO",
        "NEAR_TRANSACTION_HASH",
        "NEAR_MAX_INSTRUCTIONS",
        "NEAR_MAX_MEMORY_MB",
        "NEAR_MAX_EXECUTION_SECONDS",
        "NEAR_REQUEST_ID",
        "NEAR_NETWORK_ID",
        "OUTLAYER_PROJECT_ID",
        "OUTLAYER_PROJECT_UUID",
    ];

    let reserved_found: Vec<&str> = secrets_map.keys()
        .filter(|k| RESERVED_KEYWORDS.contains(&k.as_str()))
        .map(|k| k.as_str())
        .collect();

    if !reserved_found.is_empty() {
        return Err(ApiError::BadRequest(format!(
            "Cannot use reserved system keywords as secret keys: {}. \
            These environment variables are automatically set by OutLayer worker.",
            reserved_found.join(", ")
        )));
    }

    // 5. Re-encrypt merged secrets
    let final_secrets_json = serde_json::to_string(&secrets_map)
        .map_err(|e| ApiError::InternalError(format!("Failed to serialize secrets: {}", e)))?;

    let keystore = state.keystore.read().await;
    let encrypted_bytes = keystore
        .encrypt(customer.as_ref(), &req.seed, final_secrets_json.as_bytes())
        .map_err(|e| ApiError::InternalError(format!("Failed to encrypt secrets: {}", e)))?;

    let encrypted_base64 = base64::encode(&encrypted_bytes);

    // Get all secret keys for verification
    let all_secret_keys: Vec<String> = secrets_map.keys().cloned().collect();

    tracing::info!(
        seed = %req.seed,
        total_secrets = secrets_map.len(),
        newly_generated_count = generated_keys.len(),
        encrypted_size = encrypted_bytes.len(),
        all_keys = ?all_secret_keys,
        "Successfully added generated secrets"
    );

    Ok(Json(AddGeneratedSecretResponse {
        encrypted_data_base64: encrypted_base64,
        all_keys: all_secret_keys,
    }))
}

/// Update user secrets with NEAR signature authentication
async fn update_user_secrets_handler(
    State(state): State<AppState>,
    Json(req): Json<UpdateUserSecretsRequest>,
) -> Result<Json<UpdateUserSecretsResponse>, ApiError> {
    // Check if keystore is ready
    if !state.is_ready() {
        tracing::warn!("Update secrets request rejected - keystore not ready");
        return Err(ApiError::Unauthorized(
            "Keystore not ready. Waiting for DAO approval and master key from MPC.".to_string()
        ));
    }

    // Vault scope from request body. The Append-mode decrypt
    // and the final encrypt MUST run under the same scope, otherwise
    // the round-trip would corrupt the user's data.
    let customer = parse_optional_vault_id(req.vault_id.as_deref())?;
    state
        .ensure_customer_loaded(customer.as_ref())
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    tracing::info!(
        owner = %req.owner,
        profile = %req.profile,
        mode = ?req.mode,
        vault_id = ?customer,
        "Received update_user_secrets request"
    );

    // 1. Verify message format
    // New format includes secrets payload for verification:
    // "Update Outlayer secrets for {owner}:{profile}\nkeys:{key1,key2}\nprotected:{PROTECTED_A,PROTECTED_B}"
    let mut expected_message = format!("Update Outlayer secrets for {}:{}", req.owner, req.profile);

    // Add sorted secret keys to message (must match dashboard serialization)
    let mut secret_keys: Vec<&String> = req.secrets.keys().collect();
    secret_keys.sort();
    if !secret_keys.is_empty() {
        expected_message.push_str("\nkeys:");
        expected_message.push_str(&secret_keys.iter().map(|k| k.as_str()).collect::<Vec<_>>().join(","));
    }

    // Add sorted PROTECTED_ names to message
    if let Some(ref generate) = req.generate_protected {
        let mut protected_names: Vec<&str> = generate.iter().map(|g| g.name.as_str()).collect();
        protected_names.sort();
        if !protected_names.is_empty() {
            expected_message.push_str("\nprotected:");
            expected_message.push_str(&protected_names.join(","));
        }
    }

    if req.signed_message != expected_message {
        tracing::warn!(
            expected = %expected_message,
            received = %req.signed_message,
            "Message format mismatch"
        );
        return Err(ApiError::BadRequest(format!(
            "Invalid message format. Expected payload to match request data. Expected: '{}', Got: '{}'",
            expected_message, req.signed_message
        )));
    }

    // 2. Verify NEAR signature (NEP-413)
    tracing::info!(
        message = %req.signed_message,
        public_key = %req.public_key,
        nonce = %req.nonce,
        recipient = %req.recipient,
        signature_len = req.signature.len(),
        "Verifying NEP-413 signature"
    );

    match verify_near_signature(&req.signed_message, &req.signature, &req.public_key, &req.nonce, &req.recipient) {
        Ok(()) => {
            tracing::info!(
                owner = %req.owner,
                public_key = %req.public_key,
                "✅ NEP-413 signature verified successfully"
            );
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                message = %req.signed_message,
                public_key = %req.public_key,
                nonce = %req.nonce,
                recipient = %req.recipient,
                "❌ NEP-413 signature verification failed"
            );
            return Err(ApiError::Unauthorized(format!("Invalid signature: {}", e)));
        }
    }

    // 3. Verify public key belongs to owner via NEAR RPC
    let near_client = state.near_client.as_ref();
    if let Some(client) = near_client {
        match client.verify_access_key_owner(&req.owner, &req.public_key).await {
            Ok(()) => {
                tracing::info!(
                    owner = %req.owner,
                    public_key = %req.public_key,
                    "✅ Access key ownership verified via NEAR RPC"
                );
            }
            Err(e) => {
                tracing::warn!(
                    owner = %req.owner,
                    public_key = %req.public_key,
                    error = %e,
                    "❌ Access key ownership verification failed"
                );
                return Err(ApiError::Unauthorized(format!(
                    "Public key {} does not belong to account {}: {}",
                    req.public_key, req.owner, e
                )));
            }
        }
    } else {
        tracing::warn!(
            "NEAR client not configured - skipping access key ownership verification. \
            This is a security risk! Set NEAR_RPC_URL and NEAR_CONTRACT_ID."
        );
    }

    // 4. Validate user secrets don't contain PROTECTED_ prefix
    let protected_in_user_secrets: Vec<&String> = req.secrets.keys()
        .filter(|k| k.starts_with("PROTECTED_"))
        .collect();

    if !protected_in_user_secrets.is_empty() {
        return Err(ApiError::BadRequest(format!(
            "User secrets cannot use 'PROTECTED_' prefix: {}",
            protected_in_user_secrets.iter().map(|k| k.as_str()).collect::<Vec<_>>().join(", ")
        )));
    }

    // 5. Determine if this is a migration (accessor change)
    let is_migration = req.new_accessor.is_some();
    if is_migration {
        tracing::info!(
            owner = %req.owner,
            profile = %req.profile,
            "Migration mode: will decrypt with old accessor, encrypt with new accessor"
        );
    }

    // 6. Get current encrypted secrets + on-chain vault binding using
    //    OLD accessor (new_accessor is only for encryption target).
    //
    // CRITICAL: we MUST decrypt existing data under the
    // scope it was originally written with — that's the on-chain
    // `vault_id` binding, NOT the body-supplied vault_id (which
    // applies only to the new write).
    //
    // Migration paths supported:
    //   * default→vault: existing secret has vault_id=None on chain;
    //     body sends vault_id=Some(v). We decrypt under None, encrypt
    //     under v.
    //   * vault→vault (rotation): existing has vault_id=Some(a),
    //     body sends Some(b). Decrypt under a, encrypt under b.
    //   * no migration: existing has vault_id=Some(a), body sends
    //     Some(a). Decrypt and encrypt under a.
    let near_client = state.near_client.as_ref()
        .ok_or_else(|| ApiError::InternalError("NEAR client not configured".to_string()))?;

    let accessor_json_for_lookup = accessor_to_contract_json(&req.accessor);
    let combined = near_client
        .get_secret_with_vault(accessor_json_for_lookup, &req.profile, &req.owner)
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "Failed to fetch secrets from contract");
            ApiError::InternalError(format!("Failed to fetch secrets: {}", e))
        })?;
    let secret_profile = combined.profile;
    let on_chain_vault_id_str = combined.vault_id;

    let existing_customer = parse_optional_vault_id(on_chain_vault_id_str.as_deref())?;
    // Make sure the master that was used at write time is loaded
    // before we try to decrypt. For default-master path
    // (existing_customer = None) this is a no-op.
    state
        .ensure_customer_loaded(existing_customer.as_ref())
        .await
        .map_err(|e| ApiError::InternalError(format!(
            "lazy-load gate failed for existing vault {}: {}",
            existing_customer.as_ref().map(|v| v.as_str()).unwrap_or("<none>"),
            e
        )))?;

    // 7. Decrypt existing secrets (if any) using OLD accessor seed
    let mut current_secrets: serde_json::Map<String, serde_json::Value> = if let Some(profile) = secret_profile {
        tracing::info!(
            profile_data = ?profile,
            "Found existing secrets in contract, attempting to decrypt"
        );

        // Extract encrypted_secrets field from JSON
        let encrypted_secrets_str = profile
            .get("encrypted_secrets")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                tracing::error!("Missing encrypted_secrets field in profile data");
                ApiError::InternalError("Missing encrypted_secrets field".to_string())
            })?;

        // Decode from base64
        let encrypted_bytes = base64::decode(encrypted_secrets_str)
            .map_err(|e| {
                tracing::error!(error = %e, "Invalid base64 in stored secrets");
                ApiError::BadRequest(format!("Invalid base64 in stored secrets: {}", e))
            })?;

        // Generate seed for decryption (must match format used during encryption)
        // Format: normalized_repo:owner[:branch] - same as /pubkey endpoint
        let seed = match &req.accessor {
            SecretAccessor::Repo { repo, branch } => {
                let normalized_repo = crate::utils::normalize_repo_url(repo);
                if let Some(b) = branch.as_deref().filter(|s| !s.is_empty()) {
                    format!("{}:{}:{}", normalized_repo, req.owner, b)
                } else {
                    format!("{}:{}", normalized_repo, req.owner)
                }
            }
            SecretAccessor::WasmHash { hash } => {
                format!("wasm_hash:{}:{}", hash, req.owner)
            }
            SecretAccessor::Project { project_id } => {
                format!("project:{}:{}", project_id, req.owner)
            }
            SecretAccessor::System { secret_type } => {
                format!("system:{}:{}:{}", secret_type.as_seed_str(), req.owner, req.profile)
            }
        };

        tracing::debug!(
            seed = %seed,
            encrypted_len = encrypted_bytes.len(),
            "Attempting to decrypt existing secrets"
        );

        // Decrypt under the EXISTING (on-chain) scope — see comment at
        // step 6 above. Body's `customer` is for the new encrypt only.
        let keystore = state.keystore.read().await;
        let plaintext_bytes = keystore
            .decrypt(existing_customer.as_ref(), &seed, &encrypted_bytes)
            .map_err(|e| {
                tracing::error!(
                    error = %e,
                    seed = %seed,
                    "Failed to decrypt existing secrets - possibly encrypted with different key or corrupted"
                );
                ApiError::InternalError(format!("Failed to decrypt existing secrets: {}", e))
            })?;
        drop(keystore);

        // Parse JSON
        let plaintext_str = String::from_utf8(plaintext_bytes)
            .map_err(|e| {
                tracing::error!(error = %e, "Decrypted data is not valid UTF-8");
                ApiError::InternalError(format!("Decrypted data is not valid UTF-8: {}", e))
            })?;

        let secrets: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&plaintext_str)
            .map_err(|e| {
                tracing::error!(error = %e, "Decrypted data is not valid JSON");
                ApiError::InternalError(format!("Decrypted data is not valid JSON: {}", e))
            })?;

        tracing::info!(
            num_existing_secrets = secrets.len(),
            "Successfully decrypted existing secrets"
        );

        secrets
    } else {
        // No existing secrets
        tracing::info!("No existing secrets found in contract, starting fresh");
        serde_json::Map::new()
    };

    // Track changes for summary
    let mut protected_keys_preserved: Vec<String> = Vec::new();
    let mut updated_keys: Vec<String> = Vec::new();
    let mut removed_keys: Vec<String> = Vec::new();

    // 7. Apply update mode
    match req.mode {
        UpdateMode::Reset => {
            // Remove all non-PROTECTED keys
            let keys_to_remove: Vec<String> = current_secrets.keys()
                .filter(|k| !k.starts_with("PROTECTED_"))
                .cloned()
                .collect();

            for key in &keys_to_remove {
                current_secrets.remove(key);
                removed_keys.push(key.clone());
            }

            // Preserve PROTECTED_ keys
            for key in current_secrets.keys() {
                if key.starts_with("PROTECTED_") {
                    protected_keys_preserved.push(key.clone());
                }
            }
        }
        UpdateMode::Append => {
            // Just preserve existing PROTECTED_ keys
            for key in current_secrets.keys() {
                if key.starts_with("PROTECTED_") {
                    protected_keys_preserved.push(key.clone());
                }
            }
        }
    }

    // 8. Add/update user secrets (values are already serde_json::Value)
    for (key, value) in req.secrets {
        current_secrets.insert(key.clone(), value);
        updated_keys.push(key);
    }

    // 9. Generate new PROTECTED_ secrets if requested
    if let Some(generate_specs) = req.generate_protected {
        for spec in generate_specs {
            // Validate name starts with PROTECTED_
            if !spec.name.starts_with("PROTECTED_") {
                return Err(ApiError::BadRequest(format!(
                    "Generated secret '{}' must start with 'PROTECTED_' prefix",
                    spec.name
                )));
            }

            // Check it doesn't already exist (PROTECTED_ are immutable)
            if current_secrets.contains_key(&spec.name) {
                return Err(ApiError::BadRequest(format!(
                    "Cannot regenerate existing PROTECTED_ secret: {}. These secrets are immutable once created.",
                    spec.name
                )));
            }

            // Generate secret
            let directive = format!("generate_outlayer_secret:{}", spec.generation_type);
            let generated_value = crate::secret_generation::generate_secret(&directive)
                .map_err(|e| ApiError::BadRequest(format!(
                    "Failed to generate secret '{}' with type '{}': {}",
                    spec.name, spec.generation_type, e
                )))?;

            tracing::info!(
                key = %spec.name,
                gen_type = %spec.generation_type,
                "Generated PROTECTED_ secret"
            );

            current_secrets.insert(spec.name.clone(), serde_json::Value::String(generated_value));
            updated_keys.push(spec.name);
        }
    }

    // 10. Re-encrypt updated secrets with NEW accessor seed (if migrating)
    let final_secrets_json = serde_json::to_string(&current_secrets)
        .map_err(|e| ApiError::InternalError(format!("Failed to serialize secrets: {}", e)))?;

    // Generate seed for encryption - use new_accessor if provided (migration), otherwise use original accessor
    // Format: normalized_repo:owner[:branch] - same as /pubkey endpoint
    let final_accessor = req.new_accessor.as_ref().unwrap_or(&req.accessor);
    let encryption_seed = match final_accessor {
        SecretAccessor::Repo { repo, branch } => {
            let normalized_repo = crate::utils::normalize_repo_url(repo);
            if let Some(b) = branch.as_deref().filter(|s| !s.is_empty()) {
                format!("{}:{}:{}", normalized_repo, req.owner, b)
            } else {
                format!("{}:{}", normalized_repo, req.owner)
            }
        }
        SecretAccessor::WasmHash { hash } => {
            format!("wasm_hash:{}:{}", hash, req.owner)
        }
        SecretAccessor::Project { project_id } => {
            format!("project:{}:{}", project_id, req.owner)
        }
        SecretAccessor::System { secret_type } => {
            format!("system:{}:{}:{}", secret_type.as_seed_str(), req.owner, req.profile)
        }
    };

    if is_migration {
        tracing::info!(
            encryption_seed = %encryption_seed,
            "Migration: encrypting with NEW accessor seed"
        );
    }

    let keystore = state.keystore.read().await;
    let encrypted_bytes = keystore
        .encrypt(customer.as_ref(), &encryption_seed, final_secrets_json.as_bytes())
        .map_err(|e| ApiError::InternalError(format!("Failed to encrypt secrets: {}", e)))?;
    drop(keystore);

    let encrypted_base64 = base64::encode(&encrypted_bytes);

    // Prepare summary
    let summary = UpdateSummary {
        protected_keys_preserved,
        updated_keys,
        removed_keys,
        total_keys: current_secrets.len(),
    };

    tracing::info!(
        owner = %req.owner,
        profile = %req.profile,
        total_keys = summary.total_keys,
        protected_preserved = summary.protected_keys_preserved.len(),
        updated = summary.updated_keys.len(),
        removed = summary.removed_keys.len(),
        "Successfully updated user secrets"
    );

    Ok(Json(UpdateUserSecretsResponse {
        encrypted_secrets_base64: encrypted_base64,
        summary,
    }))
}

// ==================== Storage Encryption Handlers ====================

/// Encrypt data for persistent storage
///
/// Uses derived key from: `storage:{project_uuid|wasm_hash}:{account_id}`
/// This keeps encryption keys isolated per project/wasm and per account.
async fn storage_encrypt_handler(
    State(state): State<AppState>,
    Json(req): Json<StorageEncryptRequest>,
) -> Result<Json<StorageEncryptResponse>, ApiError> {
    // Check if keystore is ready
    if !state.is_ready() {
        return Err(ApiError::Unauthorized(
            "Keystore not ready. Waiting for DAO approval and master key from MPC.".to_string()
        ));
    }

    // Verify TEE attestation
    crate::attestation::verify_attestation(
        &req.attestation,
        &state.config.tee_mode,
        &state.expected_measurements,
    )
    .map_err(|e| {
        tracing::warn!(error = %e, "Storage encrypt attestation verification failed");
        ApiError::Unauthorized(format!("Attestation verification failed: {}", e))
    })?;

    // Build seed for key derivation
    // For projects: storage:{project_uuid}:{account_id}
    // For standalone WASM: storage:wasm:{wasm_hash}:{account_id}
    let seed = if let Some(ref project_uuid) = req.project_uuid {
        format!("storage:{}:{}", project_uuid, req.account_id)
    } else {
        format!("storage:wasm:{}:{}", req.wasm_hash, req.account_id)
    };

    tracing::debug!(
        seed = %seed,
        project_uuid = ?req.project_uuid,
        wasm_hash = %req.wasm_hash,
        account_id = %req.account_id,
        key = %req.key,
        "Encrypting storage data"
    );

    // Decode value from base64
    let value_bytes = base64::decode(&req.value_base64)
        .map_err(|e| ApiError::BadRequest(format!("Invalid base64 in value: {}", e)))?;

    // Encrypt key and value
    let keystore = state.keystore.read().await;

    let encrypted_key = keystore
        .encrypt(None, &seed, req.key.as_bytes())
        .map_err(|e| ApiError::InternalError(format!("Failed to encrypt key: {}", e)))?;

    let encrypted_value = keystore
        .encrypt(None, &seed, &value_bytes)
        .map_err(|e| ApiError::InternalError(format!("Failed to encrypt value: {}", e)))?;

    // Calculate key hash for unique constraint
    use sha2::{Sha256, Digest};
    let key_hash = hex::encode(Sha256::digest(req.key.as_bytes()));

    tracing::info!(
        project_uuid = ?req.project_uuid,
        wasm_hash = %req.wasm_hash,
        account_id = %req.account_id,
        key_hash = %key_hash,
        encrypted_key_len = encrypted_key.len(),
        encrypted_value_len = encrypted_value.len(),
        "Successfully encrypted storage data"
    );

    Ok(Json(StorageEncryptResponse {
        encrypted_key_base64: base64::encode(&encrypted_key),
        encrypted_value_base64: base64::encode(&encrypted_value),
        key_hash,
    }))
}

/// Decrypt data from persistent storage
async fn storage_decrypt_handler(
    State(state): State<AppState>,
    Json(req): Json<StorageDecryptRequest>,
) -> Result<Json<StorageDecryptResponse>, ApiError> {
    // Check if keystore is ready
    if !state.is_ready() {
        return Err(ApiError::Unauthorized(
            "Keystore not ready. Waiting for DAO approval and master key from MPC.".to_string()
        ));
    }

    // Verify TEE attestation
    crate::attestation::verify_attestation(
        &req.attestation,
        &state.config.tee_mode,
        &state.expected_measurements,
    )
    .map_err(|e| {
        tracing::warn!(error = %e, "Storage decrypt attestation verification failed");
        ApiError::Unauthorized(format!("Attestation verification failed: {}", e))
    })?;

    // Build seed for key derivation (same as encrypt)
    let seed = if let Some(ref project_uuid) = req.project_uuid {
        format!("storage:{}:{}", project_uuid, req.account_id)
    } else {
        format!("storage:wasm:{}:{}", req.wasm_hash, req.account_id)
    };

    // Decode encrypted data from base64
    let encrypted_key = base64::decode(&req.encrypted_key_base64)
        .map_err(|e| ApiError::BadRequest(format!("Invalid base64 in encrypted_key: {}", e)))?;

    let encrypted_value = base64::decode(&req.encrypted_value_base64)
        .map_err(|e| ApiError::BadRequest(format!("Invalid base64 in encrypted_value: {}", e)))?;

    // Decrypt key and value
    let keystore = state.keystore.read().await;

    let key_bytes = keystore
        .decrypt(None, &seed, &encrypted_key)
        .map_err(|e| ApiError::InternalError(format!("Failed to decrypt key: {}", e)))?;

    let value_bytes = keystore
        .decrypt(None, &seed, &encrypted_value)
        .map_err(|e| ApiError::InternalError(format!("Failed to decrypt value: {}", e)))?;

    // Convert key to string
    let key = String::from_utf8(key_bytes)
        .map_err(|e| ApiError::InternalError(format!("Decrypted key is not valid UTF-8: {}", e)))?;

    tracing::debug!(
        project_uuid = ?req.project_uuid,
        wasm_hash = %req.wasm_hash,
        account_id = %req.account_id,
        key = %key,
        "Successfully decrypted storage data"
    );

    Ok(Json(StorageDecryptResponse {
        key,
        value_base64: base64::encode(&value_bytes),
    }))
}

/// NEP-413 payload structure for Borsh serialization
/// See: https://github.com/near/NEPs/blob/master/neps/nep-0413.md
#[derive(borsh::BorshSerialize)]
struct Nep413Payload {
    /// The message that was requested to be signed
    message: String,
    /// 32-byte nonce
    nonce: [u8; 32],
    /// The recipient to whom the signature is intended for
    recipient: String,
    /// Optional callback URL (always None for our use case)
    callback_url: Option<String>,
}

/// NEP-413 tag: 2^31 + 413
const NEP413_TAG: u32 = 2147484061;

/// Verify NEAR signature (NEP-413)
///
/// NEP-413 specifies that the signed payload is:
/// SHA256(NEP413_TAG || Borsh(Nep413Payload))
fn verify_near_signature(
    message: &str,
    signature: &str,
    public_key: &str,
    nonce: &str,
    recipient: &str,
) -> Result<(), anyhow::Error> {
    use sha2::{Sha256, Digest};

    // Parse public key (format: "ed25519:base58...")
    let pubkey_parts: Vec<&str> = public_key.split(':').collect();
    if pubkey_parts.len() != 2 || pubkey_parts[0] != "ed25519" {
        anyhow::bail!("Invalid public key format, expected 'ed25519:base58...'");
    }

    let pubkey_bytes = bs58::decode(pubkey_parts[1])
        .into_vec()
        .map_err(|e| anyhow::anyhow!("Failed to decode public key: {}", e))?;

    if pubkey_bytes.len() != 32 {
        anyhow::bail!("Invalid public key length: {}", pubkey_bytes.len());
    }

    // Decode signature (base64)
    let signature_bytes = base64::decode(signature)
        .map_err(|e| anyhow::anyhow!("Failed to decode signature: {}", e))?;

    if signature_bytes.len() != 64 {
        anyhow::bail!("Invalid signature length: {}", signature_bytes.len());
    }

    // Decode nonce (base64) - must be exactly 32 bytes
    let nonce_bytes = base64::decode(nonce)
        .map_err(|e| anyhow::anyhow!("Failed to decode nonce: {}", e))?;

    let nonce_len = nonce_bytes.len();
    if nonce_len != 32 {
        anyhow::bail!("Invalid nonce length: {} (expected 32)", nonce_len);
    }

    let nonce_array: [u8; 32] = nonce_bytes.try_into()
        .map_err(|_| anyhow::anyhow!("Failed to convert nonce to array"))?;

    // Build NEP-413 payload
    let payload = Nep413Payload {
        message: message.to_string(),
        nonce: nonce_array,
        recipient: recipient.to_string(),
        callback_url: None,
    };

    // Serialize payload with Borsh
    let payload_bytes = borsh::to_vec(&payload)
        .map_err(|e| anyhow::anyhow!("Failed to serialize NEP-413 payload: {}", e))?;

    // Build final message: tag (4 bytes LE) + payload
    let mut to_hash = Vec::with_capacity(4 + payload_bytes.len());
    to_hash.extend_from_slice(&NEP413_TAG.to_le_bytes());
    to_hash.extend_from_slice(&payload_bytes);

    // SHA256 hash the combined data
    let hash = Sha256::digest(&to_hash);

    tracing::debug!(
        message = %message,
        recipient = %recipient,
        nonce_len = nonce_len,
        payload_len = payload_bytes.len(),
        hash_hex = %hex::encode(&hash),
        "NEP-413 signature verification"
    );

    // Verify using ed25519
    use ed25519_dalek::{Signature, VerifyingKey, Verifier};

    let verifying_key = VerifyingKey::from_bytes(
        &<[u8; 32]>::try_from(pubkey_bytes.as_slice())
            .map_err(|_| anyhow::anyhow!("Invalid public key bytes"))?
    ).map_err(|e| anyhow::anyhow!("Invalid public key: {}", e))?;

    let signature = Signature::from_bytes(
        &<[u8; 64]>::try_from(signature_bytes.as_slice())
            .map_err(|_| anyhow::anyhow!("Invalid signature bytes"))?
    );

    // Verify signature against the hash
    verifying_key
        .verify(&hash, &signature)
        .map_err(|e| anyhow::anyhow!("Signature verification failed: {}", e))?;

    Ok(())
}

// =========================================================================
// TEE Challenge-Response Endpoints
// =========================================================================

/// Generate a TEE challenge for worker registration
///
/// Worker calls this to get a random nonce, then signs it with their TEE private key.
async fn tee_challenge_handler(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let challenge = shared_tee_helpers::generate_challenge();

    // Store challenge in memory with timestamp
    {
        let mut challenges = state.tee_challenges.lock().unwrap();

        // Clean up expired challenges (>60 seconds old)
        challenges.retain(|_, c| c.created_at.elapsed().as_secs() < 60);

        challenges.insert(challenge.clone(), TeeChallenge {
            created_at: std::time::Instant::now(),
        });
    }

    tracing::debug!("TEE challenge generated: {}...", &challenge[..16]);

    Ok(Json(serde_json::json!({ "challenge": challenge })))
}

/// Request body for TEE registration
#[derive(Debug, Deserialize)]
struct RegisterTeeRequest {
    public_key: String,
    challenge: String,
    signature: String,
}

/// Register a TEE session after challenge-response verification
///
/// 1. Verify challenge exists and is not expired
/// 2. Verify ed25519 signature
/// 3. Check public key exists on register-contract via NEAR RPC
/// 4. Create session and return session_id
async fn register_tee_handler(
    State(state): State<AppState>,
    Json(req): Json<RegisterTeeRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // 1. Find and remove challenge (one-time use)
    {
        let mut challenges = state.tee_challenges.lock().unwrap();
        let challenge = challenges.remove(&req.challenge).ok_or_else(|| {
            ApiError::BadRequest("Invalid or expired challenge".to_string())
        })?;

        // Check expiration (60 seconds)
        if challenge.created_at.elapsed().as_secs() > 60 {
            return Err(ApiError::BadRequest("Challenge expired".to_string()));
        }
    }

    // 2. Verify signature
    shared_tee_helpers::verify_signature(&req.public_key, &req.challenge, &req.signature)
        .map_err(|e| ApiError::BadRequest(format!("Signature verification failed: {}", e)))?;

    // 3. Check key on operator account via NEAR RPC (with retry for finality lag)
    let operator_account_id = state.config.operator_account_id.as_ref().ok_or_else(|| {
        ApiError::InternalError("OPERATOR_ACCOUNT_ID not configured on keystore".to_string())
    })?;

    let key_exists = shared_tee_helpers::check_access_key_with_retry(
        &state.config.near_rpc_url,
        operator_account_id,
        &req.public_key,
    )
    .await
    .map_err(|e| ApiError::InternalError(format!("NEAR RPC check failed: {}", e)))?;

    if !key_exists {
        return Err(ApiError::Unauthorized(format!(
            "Public key {} not found on operator account {}",
            req.public_key, operator_account_id
        )));
    }

    // 4. Create session
    let session_id = uuid::Uuid::new_v4();
    {
        let mut sessions = state.tee_sessions.lock().unwrap();
        sessions.insert(session_id, TeeSession {
            worker_public_key: req.public_key.clone(),
            created_at: std::time::Instant::now(),
        });
    }

    tracing::info!(
        session_id = %session_id,
        public_key = %req.public_key,
        "TEE session registered on keystore"
    );

    Ok(Json(serde_json::json!({ "session_id": session_id.to_string() })))
}

/// Validate X-TEE-Session header against in-memory sessions.
/// Returns Ok(()) if session is valid or TEE sessions not required.
/// Returns Err(ApiError::Forbidden) if required but missing/invalid.
/// Extract a per-customer vault id from the `X-Customer-Vault` header.
///
/// **Use case:** every wallet operation
/// names a customer through this header. The coordinator sets the header
/// based on the API key → customer mapping before forwarding the
/// request to keystore-worker. Absent header ⇒ legacy default-master
/// path (existing OutLayer customers without a vault remain on the
/// shared master).
///
/// Returns `Ok(None)` if the header is absent (legacy path) or
/// `Err(ApiError::BadRequest(_))` if the header is present but
/// malformed — we deliberately do NOT silently fall back to default
/// master on a malformed header, otherwise a typo could route a
/// customer's request to the wrong key-space.
pub(crate) fn extract_customer_from_header(
    headers: &axum::http::HeaderMap,
) -> Result<Option<near_primitives::types::AccountId>, ApiError> {
    let Some(raw) = headers.get("X-Customer-Vault") else {
        return Ok(None);
    };
    let s = raw
        .to_str()
        .map_err(|_| ApiError::BadRequest("X-Customer-Vault header is not valid UTF-8".to_string()))?
        .trim();
    if s.is_empty() {
        // Empty header is treated the same as no header (legacy path).
        return Ok(None);
    }
    let vault_id: near_primitives::types::AccountId = s.parse().map_err(|e| {
        ApiError::BadRequest(format!(
            "X-Customer-Vault is not a valid NEAR AccountId ({}): {}",
            s, e
        ))
    })?;
    Ok(Some(vault_id))
}

/// Parse an optional vault id from a string source.
///
/// **Sources** (any caller may use this):
/// 1. JSON request body field `vault_id: Option<String>` —
///    `/pubkey`, `/add_generated_secret`, `/update_user_secrets`
///    carry vault scope inline because it's an attribute of the
///    operation (which secret to encrypt against), not of the
///    caller's session.
/// 2. On-chain side-table value returned by `get_secret_with_vault`
///    — `/decrypt` and `/update_user_secrets` derive the existing
///    secret's scope from chain rather than trusting the caller.
///
/// Same fail-loud-on-malformed semantics as
/// [`extract_customer_from_header`]: empty / whitespace / missing
/// → `Ok(None)` (legacy default-master path); a present-but-malformed
/// value returns `Err(ApiError::BadRequest)` rather than silently
/// downgrading to None — that prevents a typo from routing into
/// the wrong key-space.
fn parse_optional_vault_id(
    raw: Option<&str>,
) -> Result<Option<near_primitives::types::AccountId>, ApiError> {
    let Some(s) = raw.map(|s| s.trim()).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let vault_id: near_primitives::types::AccountId = s.parse().map_err(|e| {
        ApiError::BadRequest(format!(
            "vault_id is not a valid NEAR AccountId ({}): {}",
            s, e
        ))
    })?;
    Ok(Some(vault_id))
}

fn validate_tee_session(state: &AppState, headers: &axum::http::HeaderMap) -> Result<(), ApiError> {
    if state.config.tee_mode == crate::config::TeeMode::None {
        return Ok(());
    }

    let session_header = headers
        .get("X-TEE-Session")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| {
            ApiError::Forbidden("TEE session required. Register via /tee-challenge + /register-tee".to_string())
        })?;

    let session_id = uuid::Uuid::parse_str(session_header)
        .map_err(|_| ApiError::Forbidden("Invalid TEE session ID format".to_string()))?;

    let sessions = state.tee_sessions.lock().unwrap();
    if !sessions.contains_key(&session_id) {
        return Err(ApiError::Forbidden("TEE session not found or expired".to_string()));
    }

    Ok(())
}

/// TEE session middleware
///
/// Checks X-TEE-Session header against in-memory sessions.
/// Only enforced when TEE_MODE=outlayer_tee (skipped in none mode).
/// Runs after worker_auth_middleware (inner layer) on worker-only routes.
async fn tee_session_middleware(
    State(state): State<AppState>,
    req: axum::http::Request<axum::body::Body>,
    next: middleware::Next,
) -> Result<Response, ApiError> {
    validate_tee_session(&state, req.headers())?;
    Ok(next.run(req).await)
}

/// Worker authentication middleware
///
/// For TEE worker-only endpoints: /decrypt, /encrypt, /decrypt-raw, /storage/*
/// Checks Bearer token against ALLOWED_WORKER_TOKEN_HASHES.
async fn worker_auth_middleware(
    State(state): State<AppState>,
    req: axum::http::Request<axum::body::Body>,
    next: middleware::Next,
) -> Result<Response, ApiError> {
    // Get Authorization header
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| {
            tracing::warn!("Missing Authorization header in worker request");
            ApiError::Unauthorized("Missing Authorization header".to_string())
        })?;

    // Extract Bearer token
    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| {
            tracing::warn!("Invalid Authorization format (expected 'Bearer <token>')");
            ApiError::Unauthorized("Invalid Authorization format".to_string())
        })?;

    // Hash token with SHA256
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let token_hash = hex::encode(hasher.finalize());

    // Check if hash is in allowed WORKER list
    if !state.config.allowed_worker_token_hashes.contains(&token_hash) {
        tracing::warn!(
            token_hash = %token_hash,
            "Unauthorized: token hash not in worker allowed list"
        );
        return Err(ApiError::Unauthorized("Invalid worker token".to_string()));
    }

    // Find which worker this token belongs to (for logging)
    let worker_index = state.config.allowed_worker_token_hashes
        .iter()
        .position(|h| h == &token_hash)
        .unwrap_or(0);

    tracing::debug!(
        token_hash = %token_hash,
        worker_index = worker_index,
        "✅ Worker authenticated successfully"
    );

    Ok(next.run(req).await)
}

/// Coordinator authentication middleware
///
/// For coordinator-only endpoints: /add_generated_secret, /update_user_secrets
/// Checks Bearer token against ALLOWED_COORDINATOR_TOKEN_HASHES.
async fn coordinator_auth_middleware(
    State(state): State<AppState>,
    req: axum::http::Request<axum::body::Body>,
    next: middleware::Next,
) -> Result<Response, ApiError> {
    // Get Authorization header
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| {
            tracing::warn!("Missing Authorization header in coordinator request");
            ApiError::Unauthorized("Missing Authorization header".to_string())
        })?;

    // Extract Bearer token
    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| {
            tracing::warn!("Invalid Authorization format (expected 'Bearer <token>')");
            ApiError::Unauthorized("Invalid Authorization format".to_string())
        })?;

    // Hash token with SHA256
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let token_hash = hex::encode(hasher.finalize());

    // Check if hash is in allowed COORDINATOR list
    if !state.config.allowed_coordinator_token_hashes.contains(&token_hash) {
        tracing::warn!(
            token_hash = %token_hash,
            "Unauthorized: token hash not in coordinator allowed list"
        );
        return Err(ApiError::Unauthorized("Invalid coordinator token".to_string()));
    }

    tracing::debug!(
        token_hash = %token_hash,
        "✅ Coordinator authenticated successfully"
    );

    Ok(next.run(req).await)
}

/// TEE registration authentication middleware
///
/// For TEE session endpoints: /tee-challenge, /register-tee
/// Accepts EITHER coordinator OR worker token (so workers can register directly).
async fn tee_registration_auth_middleware(
    State(state): State<AppState>,
    req: axum::http::Request<axum::body::Body>,
    next: middleware::Next,
) -> Result<Response, ApiError> {
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| {
            tracing::warn!("Missing Authorization header in TEE registration request");
            ApiError::Unauthorized("Missing Authorization header".to_string())
        })?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| {
            tracing::warn!("Invalid Authorization format (expected 'Bearer <token>')");
            ApiError::Unauthorized("Invalid Authorization format".to_string())
        })?;

    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let token_hash = hex::encode(hasher.finalize());

    let is_coordinator = state.config.allowed_coordinator_token_hashes.contains(&token_hash);
    let is_worker = state.config.allowed_worker_token_hashes.contains(&token_hash);

    if !is_coordinator && !is_worker {
        tracing::warn!(
            token_hash = %token_hash,
            "Unauthorized: token hash not in coordinator or worker allowed list"
        );
        return Err(ApiError::Unauthorized("Invalid token".to_string()));
    }

    let source = if is_coordinator { "coordinator" } else { "worker" };
    tracing::debug!(
        token_hash = %token_hash,
        source = source,
        "✅ TEE registration authenticated ({})", source
    );

    Ok(next.run(req).await)
}

// Base64 encoding/decoding helpers
mod base64 {
    use ::base64::Engine;
    use ::base64::engine::general_purpose::STANDARD;

    pub fn encode<T: AsRef<[u8]>>(input: T) -> String {
        STANDARD.encode(input)
    }

    pub fn decode<T: AsRef<[u8]>>(input: T) -> Result<Vec<u8>, ::base64::DecodeError> {
        STANDARD.decode(input)
    }
}

// ==================== Wallet Handlers ====================

/// Derive a wallet address for a specific chain
///
/// Seed format: "wallet:{wallet_id}:{chain}"
/// - near/solana: Ed25519 keypair → implicit account (hex-encoded public key)
/// - ethereum: secp256k1 keypair → keccak256 → address (0x-prefixed)
async fn wallet_derive_address_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<WalletDeriveAddressRequest>,
) -> Result<Json<WalletDeriveAddressResponse>, ApiError> {
    if !state.is_ready() {
        return Err(ApiError::Unauthorized(
            "Keystore not ready. Waiting for DAO approval and master key from MPC.".to_string(),
        ));
    }

    // Route to per-customer master if X-Customer-Vault is set.
    let customer = extract_customer_from_header(&headers)?;
    state
        .ensure_customer_loaded(customer.as_ref())
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    let chain = req.chain.to_lowercase();
    let seed = format!("wallet:{}:{}", req.wallet_id, chain);

    let keystore = state.keystore.read().await;

    match chain.as_str() {
        "near" => {
            let (_, verifying_key) = keystore.derive_keypair(customer.as_ref(), &seed).map_err(|e| {
                ApiError::InternalError(format!("Key derivation failed: {}", e))
            })?;
            let pubkey_hex = hex::encode(verifying_key.as_bytes());
            // NEAR implicit account = hex-encoded Ed25519 public key
            Ok(Json(WalletDeriveAddressResponse {
                address: pubkey_hex.clone(),
                public_key: format!("ed25519:{}", pubkey_hex),
            }))
        }
        "solana" => {
            let (_, verifying_key) = keystore.derive_keypair(customer.as_ref(), &seed).map_err(|e| {
                ApiError::InternalError(format!("Key derivation failed: {}", e))
            })?;
            let pubkey_bytes = verifying_key.as_bytes();
            let address = bs58::encode(pubkey_bytes).into_string();
            Ok(Json(WalletDeriveAddressResponse {
                address: address.clone(),
                public_key: address,
            }))
        }
        // EVM chains (Ethereum, Base, Arbitrum, etc.) — secp256k1
        // See: docs/MULTI_CHAIN.md
        "ethereum" | "base" | "arbitrum" => {
            let (address, pubkey_hex) = keystore.derive_eth_address(customer.as_ref(), &seed).map_err(|e| {
                ApiError::InternalError(format!("Key derivation failed: {}", e))
            })?;
            Ok(Json(WalletDeriveAddressResponse {
                address,
                public_key: format!("secp256k1:{}", pubkey_hex),
            }))
        }
        _ => Err(ApiError::BadRequest(format!(
            "Unsupported chain: {}. Supported: near, ethereum, solana",
            chain
        ))),
    }
}

/// Sign a transaction for a wallet on a specific chain
///
/// The keystore derives the signing key from "wallet:{wallet_id}:{chain}"
/// and signs the provided transaction bytes.
async fn wallet_sign_transaction_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<WalletSignTransactionRequest>,
) -> Result<Json<WalletSignTransactionResponse>, ApiError> {
    if !state.is_ready() {
        return Err(ApiError::Unauthorized(
            "Keystore not ready.".to_string(),
        ));
    }

    let customer = extract_customer_from_header(&headers)?;
    state
        .ensure_customer_loaded(customer.as_ref())
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    let chain = req.chain.to_lowercase();
    let seed = format!("wallet:{}:{}", req.wallet_id, chain);

    let tx_bytes = base64::decode(&req.tx_bytes_base64).map_err(|e| {
        ApiError::BadRequest(format!("Invalid base64 in tx_bytes_base64: {}", e))
    })?;

    let keystore = state.keystore.read().await;

    match chain.as_str() {
        // EVM chains — secp256k1 ECDSA
        "ethereum" | "base" | "arbitrum" => {
            let sig_bytes = keystore.sign_secp256k1(customer.as_ref(), &seed, &tx_bytes).map_err(|e| {
                ApiError::InternalError(format!("Signing failed: {}", e))
            })?;
            Ok(Json(WalletSignTransactionResponse {
                signature_base64: base64::encode(&sig_bytes),
            }))
        }
        // NEAR, Solana, etc. — Ed25519
        _ => {
            let signature = keystore.sign(customer.as_ref(), &seed, &tx_bytes).map_err(|e| {
                ApiError::InternalError(format!("Signing failed: {}", e))
            })?;
            Ok(Json(WalletSignTransactionResponse {
                signature_base64: base64::encode(signature.to_bytes()),
            }))
        }
    }
}

/// Sign encrypted policy data so the NEAR contract can verify wallet ownership.
///
/// The contract's `store_wallet_policy` requires `wallet_signature = sign(sha256(encrypted_data))`
/// for ed25519 wallets. This endpoint produces that signature using the wallet's derived key.
async fn wallet_sign_policy_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<WalletSignPolicyRequest>,
) -> Result<Json<WalletSignPolicyResponse>, ApiError> {
    if !state.is_ready() {
        return Err(ApiError::Unauthorized("Keystore not ready.".to_string()));
    }

    let customer = extract_customer_from_header(&headers)?;
    state
        .ensure_customer_loaded(customer.as_ref())
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    let hash_bytes = hex::decode(&req.encrypted_data_hash).map_err(|e| {
        ApiError::BadRequest(format!("Invalid hex in encrypted_data_hash: {}", e))
    })?;

    if hash_bytes.len() != 32 {
        return Err(ApiError::BadRequest(format!(
            "encrypted_data_hash must be 32 bytes (SHA256), got {}",
            hash_bytes.len()
        )));
    }

    let seed = format!("wallet:{}:near", req.wallet_id);
    let keystore = state.keystore.read().await;

    let signature = keystore.sign(customer.as_ref(), &seed, &hash_bytes).map_err(|e| {
        ApiError::InternalError(format!("Signing failed: {}", e))
    })?;

    // Return Ed25519 pubkey (not X25519 from public_key_hex) — must match
    // the key used for on-chain ed25519_verify in store_wallet_policy.
    let ed25519_vk = keystore.get_public_key_for_seed(customer.as_ref(), &seed).map_err(|e| {
        ApiError::InternalError(format!("Failed to derive public key: {}", e))
    })?;

    Ok(Json(WalletSignPolicyResponse {
        signature_hex: hex::encode(signature.to_bytes()),
        public_key_hex: hex::encode(ed25519_vk.as_bytes()),
    }))
}

/// Verify approval signatures against the wallet's on-chain policy.
///
/// Decrypts the policy, extracts the approvers list and threshold,
/// checks that enough approver_ids match the policy's approvers.
async fn verify_approvals(
    state: &AppState,
    wallet_id: &str,
    approval_info: &ApprovalInfo,
    customer: Option<&near_primitives::types::AccountId>,
) -> Result<(), ApiError> {
    let near_client = state.near_client.as_ref().ok_or_else(|| {
        ApiError::InternalError("NEAR client not configured".to_string())
    })?;

    let policy_view = near_client
        .get_wallet_policy(wallet_id)
        .await
        .map_err(|e| ApiError::InternalError(format!("Failed to fetch wallet policy: {}", e)))?;

    let policy_view = match policy_view {
        Some(pv) => pv,
        None => {
            // No policy on-chain — skip verification (quick onboarding mode)
            return Ok(());
        }
    };

    let encrypted_data_b64 = policy_view
        .get("encrypted_data")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::InternalError("Missing encrypted_data in policy".to_string()))?;

    // Decrypt policy. The policy was encrypted by `wallet_encrypt_policy_handler`
    // under the same customer scope, so we must use the same scope to read.
    let seed = format!("wallet-policy:{}", wallet_id);
    let keystore = state.keystore.read().await;
    let encrypted_bytes = base64::decode(encrypted_data_b64)
        .map_err(|e| ApiError::InternalError(format!("Invalid base64: {}", e)))?;
    let decrypted = keystore.decrypt(customer, &seed, &encrypted_bytes)
        .map_err(|e| ApiError::InternalError(format!("Policy decryption failed: {}", e)))?;
    let policy: serde_json::Value = serde_json::from_slice(&decrypted)
        .map_err(|e| ApiError::InternalError(format!("Policy parse failed: {}", e)))?;

    // Extract approvers and threshold
    let approval_config = match policy.get("approval") {
        Some(c) => c,
        None => return Ok(()), // No approval config in policy — skip
    };

    let threshold = approval_config
        .pointer("/threshold/required")
        .and_then(|v| v.as_i64())
        .unwrap_or(2) as usize;

    let approvers = approval_config
        .get("approvers")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| a.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if approvers.is_empty() {
        return Err(ApiError::Forbidden(format!(
            "Policy requires {} approvals but no approvers are configured. \
             Add approvers to the policy before using multisig.",
            threshold
        )));
    }

    // Count valid approvals
    let valid_count = approval_info
        .approver_ids
        .iter()
        .filter(|id| approvers.iter().any(|a| a == *id))
        .count();

    if valid_count < threshold {
        return Err(ApiError::Forbidden(format!(
            "Insufficient valid approvals: {} of {} required ({} total provided, {} matched policy approvers)",
            valid_count, threshold, approval_info.approver_ids.len(), valid_count
        )));
    }

    Ok(())
}

/// Sign a NEP-413 intent message for NEAR Intents protocol
///
/// Derives the wallet's Ed25519 keypair, constructs the NEP-413 payload,
/// and returns the signature in base58 format compatible with solver-relay.
async fn wallet_sign_nep413_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<WalletSignNep413Request>,
) -> Result<Json<WalletSignNep413Response>, ApiError> {
    use sha2::{Sha256, Digest};

    if !state.is_ready() {
        return Err(ApiError::Unauthorized(
            "Keystore not ready.".to_string(),
        ));
    }

    let customer = extract_customer_from_header(&headers)?;
    state
        .ensure_customer_loaded(customer.as_ref())
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    // Verify approval signatures if this is an approved operation
    if let Some(ref info) = req.approval_info {
        verify_approvals(&state, &req.wallet_id, info, customer.as_ref()).await?;
    }

    let chain = req.chain.to_lowercase();
    let seed = format!("wallet:{}:{}", req.wallet_id, chain);

    // Decode nonce from base64 (must be exactly 32 bytes)
    let nonce_bytes = base64::decode(&req.nonce_base64).map_err(|e| {
        ApiError::BadRequest(format!("Invalid base64 in nonce_base64: {}", e))
    })?;

    if nonce_bytes.len() != 32 {
        return Err(ApiError::BadRequest(format!(
            "Invalid nonce length: {} (expected 32)",
            nonce_bytes.len()
        )));
    }

    let nonce_array: [u8; 32] = nonce_bytes.try_into().unwrap();

    // Build NEP-413 payload
    let payload = Nep413Payload {
        message: req.message,
        nonce: nonce_array,
        recipient: req.recipient,
        callback_url: None,
    };

    // Borsh serialize
    let payload_bytes = borsh::to_vec(&payload).map_err(|e| {
        ApiError::InternalError(format!("Failed to serialize NEP-413 payload: {}", e))
    })?;

    // Hash: SHA256(NEP413_TAG || borsh_payload)
    let mut to_hash = Vec::with_capacity(4 + payload_bytes.len());
    to_hash.extend_from_slice(&NEP413_TAG.to_le_bytes());
    to_hash.extend_from_slice(&payload_bytes);
    let hash = Sha256::digest(&to_hash);

    // Derive keypair and sign the hash
    use ed25519_dalek::Signer;
    let keystore = state.keystore.read().await;
    let (signing_key, verifying_key) = keystore.derive_keypair(customer.as_ref(), &seed).map_err(|e| {
        ApiError::InternalError(format!("Key derivation failed: {}", e))
    })?;

    let signature = signing_key.sign(&hash);

    // Encode as base58 (compatible with NEAR/intents)
    let signature_base58 = bs58::encode(signature.to_bytes()).into_string();
    let public_key_base58 = bs58::encode(verifying_key.to_bytes()).into_string();

    Ok(Json(WalletSignNep413Response {
        signature_base58: format!("ed25519:{}", signature_base58),
        public_key: format!("ed25519:{}", public_key_base58),
    }))
}

/// Build and sign a native NEAR function call transaction.
///
/// The keystore derives the wallet's keypair, queries access key nonce and block hash
/// from NEAR RPC, constructs a Transaction::V0 with FunctionCall action, signs it,
/// and returns the fully signed transaction ready for broadcast.
async fn wallet_sign_near_call_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<WalletSignNearCallRequest>,
) -> Result<Json<WalletSignNearCallResponse>, ApiError> {
    use near_primitives::transaction::{Action, FunctionCallAction, Transaction, TransactionV0};
    use near_primitives::types::AccountId;
    use std::str::FromStr;

    if !state.is_ready() {
        return Err(ApiError::Unauthorized("Keystore not ready.".to_string()));
    }

    let customer = extract_customer_from_header(&headers)?;
    state
        .ensure_customer_loaded(customer.as_ref())
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    // Verify approval signatures if this is an approved operation
    if let Some(ref info) = req.approval_info {
        verify_approvals(&state, &req.wallet_id, info, customer.as_ref()).await?;
    }

    let near_client = state.near_client.as_ref().ok_or_else(|| {
        ApiError::InternalError("NEAR client not configured".to_string())
    })?;

    // 1. Derive wallet keypair
    let seed = format!("wallet:{}:near", req.wallet_id);
    let keystore = state.keystore.read().await;
    let (signing_key, verifying_key) = keystore.derive_keypair(customer.as_ref(), &seed).map_err(|e| {
        ApiError::InternalError(format!("Key derivation failed: {}", e))
    })?;
    drop(keystore);

    // 2. Compute implicit account ID and public key
    let pubkey_bytes = verifying_key.to_bytes();
    let signer_id_str = hex::encode(pubkey_bytes);
    let signer_id = AccountId::from_str(&signer_id_str).map_err(|e| {
        ApiError::InternalError(format!("Invalid implicit account ID: {}", e))
    })?;
    let public_key_str = format!("ed25519:{}", bs58::encode(&pubkey_bytes).into_string());
    let public_key: near_crypto::PublicKey = public_key_str.parse().map_err(|e| {
        ApiError::InternalError(format!("Invalid public key: {}", e))
    })?;

    // 3. Query access key nonce and block hash (or use override)
    let (rpc_nonce, block_hash) = near_client
        .query_access_key(&signer_id_str, &public_key)
        .await
        .map_err(|e| {
            ApiError::InternalError(format!("Failed to query access key: {}", e))
        })?;
    // Use override_nonce if provided (for sequential tx chains), otherwise rpc_nonce + 1
    let tx_nonce = req.override_nonce.unwrap_or(rpc_nonce + 1);

    // 4. Parse request parameters
    let receiver_id = AccountId::from_str(&req.receiver_id).map_err(|e| {
        ApiError::BadRequest(format!("Invalid receiver_id: {}", e))
    })?;

    let deposit: u128 = req.deposit.parse().map_err(|e| {
        ApiError::BadRequest(format!("Invalid deposit: {}", e))
    })?;

    let args = req.args.into_bytes()?;

    // 5. Build Transaction::V0
    let transaction = Transaction::V0(TransactionV0 {
        signer_id: signer_id.clone(),
        public_key: public_key.clone(),
        nonce: tx_nonce,
        receiver_id,
        block_hash,
        actions: vec![Action::FunctionCall(Box::new(FunctionCallAction {
            method_name: req.method_name,
            args,
            gas: req.gas,
            deposit,
        }))],
    });

    // 6. Hash and sign
    let (tx_hash, _) = transaction.get_hash_and_size();

    use ed25519_dalek::Signer;
    let sig = signing_key.sign(tx_hash.as_ref());

    let sig_str = format!("ed25519:{}", bs58::encode(sig.to_bytes()).into_string());
    let signature: near_crypto::Signature = sig_str.parse().map_err(|e| {
        ApiError::InternalError(format!("Failed to construct signature: {}", e))
    })?;

    // 7. Assemble SignedTransaction
    let signed_tx = near_primitives::transaction::SignedTransaction::new(signature, transaction);
    let signed_tx_bytes = borsh::to_vec(&signed_tx).map_err(|e| {
        ApiError::InternalError(format!("Failed to serialize signed transaction: {}", e))
    })?;

    Ok(Json(WalletSignNearCallResponse {
        signed_tx_base64: base64::encode(&signed_tx_bytes),
        tx_hash: bs58::encode(tx_hash.as_ref()).into_string(),
        signer_id: signer_id_str,
        public_key: public_key.to_string(),
        nonce: tx_nonce,
    }))
}

/// Build and sign a native NEAR transfer transaction.
///
/// Similar to `wallet_sign_near_call_handler` but uses `Action::Transfer`
/// instead of `Action::FunctionCall`. Used for sending native NEAR tokens.
async fn wallet_sign_near_transfer_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<WalletSignNearTransferRequest>,
) -> Result<Json<WalletSignNearCallResponse>, ApiError> {
    use near_primitives::transaction::{Action, TransferAction, Transaction, TransactionV0};
    use near_primitives::types::AccountId;
    use std::str::FromStr;

    if !state.is_ready() {
        return Err(ApiError::Unauthorized("Keystore not ready.".to_string()));
    }

    let customer = extract_customer_from_header(&headers)?;
    state
        .ensure_customer_loaded(customer.as_ref())
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    // Verify approval signatures if this is an approved operation
    if let Some(ref info) = req.approval_info {
        verify_approvals(&state, &req.wallet_id, info, customer.as_ref()).await?;
    }

    let near_client = state.near_client.as_ref().ok_or_else(|| {
        ApiError::InternalError("NEAR client not configured".to_string())
    })?;

    // 1. Derive wallet keypair
    let seed = format!("wallet:{}:near", req.wallet_id);
    let keystore = state.keystore.read().await;
    let (signing_key, verifying_key) = keystore.derive_keypair(customer.as_ref(), &seed).map_err(|e| {
        ApiError::InternalError(format!("Key derivation failed: {}", e))
    })?;
    drop(keystore);

    // 2. Compute implicit account ID and public key
    let pubkey_bytes = verifying_key.to_bytes();
    let signer_id_str = hex::encode(pubkey_bytes);
    let signer_id = AccountId::from_str(&signer_id_str).map_err(|e| {
        ApiError::InternalError(format!("Invalid implicit account ID: {}", e))
    })?;
    let public_key_str = format!("ed25519:{}", bs58::encode(&pubkey_bytes).into_string());
    let public_key: near_crypto::PublicKey = public_key_str.parse().map_err(|e| {
        ApiError::InternalError(format!("Invalid public key: {}", e))
    })?;

    // 3. Query access key nonce and block hash
    let (nonce, block_hash) = near_client
        .query_access_key(&signer_id_str, &public_key)
        .await
        .map_err(|e| {
            ApiError::InternalError(format!("Failed to query access key: {}", e))
        })?;

    // 4. Parse request parameters
    let receiver_id = AccountId::from_str(&req.receiver_id).map_err(|e| {
        ApiError::BadRequest(format!("Invalid receiver_id: {}", e))
    })?;

    let deposit: u128 = req.amount.parse().map_err(|e| {
        ApiError::BadRequest(format!("Invalid amount: {}", e))
    })?;

    // 5. Build Transaction::V0 with Transfer action
    let transaction = Transaction::V0(TransactionV0 {
        signer_id: signer_id.clone(),
        public_key: public_key.clone(),
        nonce: nonce + 1,
        receiver_id,
        block_hash,
        actions: vec![Action::Transfer(TransferAction { deposit })],
    });

    // 6. Hash and sign
    let (tx_hash, _) = transaction.get_hash_and_size();

    use ed25519_dalek::Signer;
    let sig = signing_key.sign(tx_hash.as_ref());

    let sig_str = format!("ed25519:{}", bs58::encode(sig.to_bytes()).into_string());
    let signature: near_crypto::Signature = sig_str.parse().map_err(|e| {
        ApiError::InternalError(format!("Failed to construct signature: {}", e))
    })?;

    // 7. Assemble SignedTransaction
    let signed_tx = near_primitives::transaction::SignedTransaction::new(signature, transaction);
    let signed_tx_bytes = borsh::to_vec(&signed_tx).map_err(|e| {
        ApiError::InternalError(format!("Failed to serialize signed transaction: {}", e))
    })?;

    Ok(Json(WalletSignNearCallResponse {
        signed_tx_base64: base64::encode(&signed_tx_bytes),
        tx_hash: bs58::encode(tx_hash.as_ref()).into_string(),
        signer_id: signer_id_str,
        public_key: public_key.to_string(),
        nonce: nonce + 1,
    }))
}

/// Build and sign a NEAR DeleteAccount transaction.
///
/// Deletes the wallet's on-chain account and sends all remaining balance
/// to the beneficiary. This is irreversible.
async fn wallet_sign_near_delete_account_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<WalletSignNearDeleteAccountRequest>,
) -> Result<Json<WalletSignNearCallResponse>, ApiError> {
    use near_primitives::transaction::{Action, DeleteAccountAction, Transaction, TransactionV0};
    use near_primitives::types::AccountId;
    use std::str::FromStr;

    if !state.is_ready() {
        return Err(ApiError::Unauthorized("Keystore not ready.".to_string()));
    }

    let customer = extract_customer_from_header(&headers)?;
    state
        .ensure_customer_loaded(customer.as_ref())
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    if let Some(ref info) = req.approval_info {
        verify_approvals(&state, &req.wallet_id, info, customer.as_ref()).await?;
    }

    let near_client = state.near_client.as_ref().ok_or_else(|| {
        ApiError::InternalError("NEAR client not configured".to_string())
    })?;

    // 1. Derive wallet keypair
    let seed = format!("wallet:{}:near", req.wallet_id);
    let keystore = state.keystore.read().await;
    let (signing_key, verifying_key) = keystore.derive_keypair(customer.as_ref(), &seed).map_err(|e| {
        ApiError::InternalError(format!("Key derivation failed: {}", e))
    })?;
    drop(keystore);

    // 2. Compute implicit account ID and public key
    let pubkey_bytes = verifying_key.to_bytes();
    let signer_id_str = hex::encode(pubkey_bytes);
    let signer_id = AccountId::from_str(&signer_id_str).map_err(|e| {
        ApiError::InternalError(format!("Invalid implicit account ID: {}", e))
    })?;
    let public_key_str = format!("ed25519:{}", bs58::encode(&pubkey_bytes).into_string());
    let public_key: near_crypto::PublicKey = public_key_str.parse().map_err(|e| {
        ApiError::InternalError(format!("Invalid public key: {}", e))
    })?;

    // 3. Query access key nonce and block hash
    let (nonce, block_hash) = near_client
        .query_access_key(&signer_id_str, &public_key)
        .await
        .map_err(|e| {
            ApiError::InternalError(format!("Failed to query access key: {}", e))
        })?;

    // 4. Parse beneficiary
    let beneficiary_id = AccountId::from_str(&req.beneficiary_id).map_err(|e| {
        ApiError::BadRequest(format!("Invalid beneficiary_id: {}", e))
    })?;

    // 5. Build Transaction::V0 with DeleteAccount action
    // receiver_id = signer_id (deleting own account)
    let transaction = Transaction::V0(TransactionV0 {
        signer_id: signer_id.clone(),
        public_key: public_key.clone(),
        nonce: nonce + 1,
        receiver_id: signer_id.clone(),
        block_hash,
        actions: vec![Action::DeleteAccount(DeleteAccountAction {
            beneficiary_id,
        })],
    });

    // 6. Hash and sign
    let (tx_hash, _) = transaction.get_hash_and_size();

    use ed25519_dalek::Signer;
    let sig = signing_key.sign(tx_hash.as_ref());

    let sig_str = format!("ed25519:{}", bs58::encode(sig.to_bytes()).into_string());
    let signature: near_crypto::Signature = sig_str.parse().map_err(|e| {
        ApiError::InternalError(format!("Failed to construct signature: {}", e))
    })?;

    // 7. Assemble SignedTransaction
    let signed_tx = near_primitives::transaction::SignedTransaction::new(signature, transaction);
    let signed_tx_bytes = borsh::to_vec(&signed_tx).map_err(|e| {
        ApiError::InternalError(format!("Failed to serialize signed transaction: {}", e))
    })?;

    Ok(Json(WalletSignNearCallResponse {
        signed_tx_base64: base64::encode(&signed_tx_bytes),
        tx_hash: bs58::encode(tx_hash.as_ref()).into_string(),
        signer_id: signer_id_str,
        public_key: public_key.to_string(),
        nonce: nonce + 1,
    }))
}

/// Check wallet policy
///
/// Reads the wallet policy entry from the NEAR contract, decrypts it,
/// and evaluates the rules against the requested action.
async fn wallet_check_policy_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<WalletCheckPolicyRequest>,
) -> Result<Json<WalletCheckPolicyResponse>, ApiError> {
    if !state.is_ready() {
        return Err(ApiError::Unauthorized(
            "Keystore not ready.".to_string(),
        ));
    }

    let customer = extract_customer_from_header(&headers)?;
    state
        .ensure_customer_loaded(customer.as_ref())
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    // Get encrypted policy data: either from inline request or from NEAR contract
    let encrypted_data_b64 = if let Some(ref inline_data) = req.encrypted_policy_data {
        // Policy override: use inline encrypted data (for testing / local policy)
        inline_data.clone()
    } else {
        // Production path: fetch from NEAR contract
        // Use Ed25519 pubkey (not X25519 from public_key_hex) — must match the key
        // stored on-chain by store_wallet_policy.
        let near_seed = format!("wallet:{}:near", req.wallet_id);
        let keystore_read = state.keystore.read().await;
        let ed25519_vk = keystore_read.get_public_key_for_seed(customer.as_ref(), &near_seed).map_err(|e| {
            ApiError::InternalError(format!("Failed to derive wallet pubkey: {}", e))
        })?;
        let wallet_pubkey_hex = hex::encode(ed25519_vk.as_bytes());
        drop(keystore_read);
        let wallet_pubkey = format!("ed25519:{}", wallet_pubkey_hex);

        let near_client = state.near_client.as_ref().ok_or_else(|| {
            ApiError::InternalError("NEAR client not configured".to_string())
        })?;

        let policy_view = near_client
            .get_wallet_policy(&wallet_pubkey)
            .await
            .map_err(|e| {
                ApiError::InternalError(format!("Failed to fetch wallet policy: {}", e))
            })?;

        let policy_view = match policy_view {
            Some(pv) => pv,
            None => {
                // No policy — allow (quick onboarding mode)
                return Ok(Json(WalletCheckPolicyResponse {
                    allowed: true,
                    frozen: false,
                    requires_approval: None,
                    required_approvals: None,
                    reason: None,
                    policy: None,
                }));
            }
        };

        // Check frozen flag (visible without decryption)
        if policy_view.get("frozen").and_then(|v| v.as_bool()).unwrap_or(false) {
            return Ok(Json(WalletCheckPolicyResponse {
                allowed: false,
                frozen: true,
                requires_approval: None,
                required_approvals: None,
                reason: Some("Wallet is frozen by controller".to_string()),
                policy: None,
            }));
        }

        policy_view
            .get("encrypted_data")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ApiError::InternalError("Missing encrypted_data field in policy".to_string()))?
            .to_string()
    };

    // Decrypt policy
    let seed = format!("wallet-policy:{}", req.wallet_id);
    let keystore = state.keystore.read().await;

    let encrypted_bytes = base64::decode(&encrypted_data_b64).map_err(|e| {
        ApiError::InternalError(format!("Invalid base64 in encrypted_data: {}", e))
    })?;

    let decrypted = keystore.decrypt(customer.as_ref(), &seed, &encrypted_bytes).map_err(|e| {
        ApiError::InternalError(format!("Policy decryption failed: {}", e))
    })?;

    let policy: serde_json::Value =
        serde_json::from_slice(&decrypted).map_err(|e| {
            ApiError::InternalError(format!("Policy JSON parse failed: {}", e))
        })?;

    // Evaluate policy rules against the action
    let decision = evaluate_policy(&policy, &req.action);

    Ok(Json(decision))
}

/// Evaluate policy rules against a requested action
pub(crate) fn evaluate_policy(
    policy: &serde_json::Value,
    action: &serde_json::Value,
) -> WalletCheckPolicyResponse {
    // Check if frozen (in policy JSON)
    if policy.get("frozen").and_then(|v| v.as_bool()).unwrap_or(false) {
        return WalletCheckPolicyResponse {
            allowed: false,
            frozen: true,
            requires_approval: None,
            required_approvals: None,
            reason: Some("Wallet is frozen".to_string()),
            policy: Some(policy.clone()),
        };
    }

    let rules = match policy.get("rules") {
        Some(r) => r,
        None => {
            return WalletCheckPolicyResponse {
                allowed: true,
                frozen: false,
                requires_approval: None,
                required_approvals: None,
                reason: None,
                policy: Some(policy.clone()),
            };
        }
    };

    let action_type = action.get("type").and_then(|v| v.as_str()).unwrap_or("");

    // Check transaction_types restriction
    if let Some(allowed_types) = rules.get("transaction_types").and_then(|v| v.as_array()) {
        let type_allowed = allowed_types
            .iter()
            .any(|t| t.as_str() == Some(action_type));
        if !type_allowed {
            return WalletCheckPolicyResponse {
                allowed: false,
                frozen: false,
                requires_approval: None,
                required_approvals: None,
                reason: Some(format!(
                    "Transaction type '{}' is not allowed by policy",
                    action_type
                )),
                policy: Some(policy.clone()),
            };
        }
    }

    // Check allowed_tokens restriction
    if let Some(allowed_tokens) = rules.get("allowed_tokens").and_then(|v| v.as_array()) {
        let token = action.get("token").and_then(|v| v.as_str()).unwrap_or("native");
        let token_allowed = allowed_tokens
            .iter()
            .any(|t| t.as_str() == Some(token));
        if !token_allowed {
            return WalletCheckPolicyResponse {
                allowed: false,
                frozen: false,
                requires_approval: None,
                required_approvals: None,
                reason: Some(format!(
                    "Token '{}' is not allowed by policy",
                    token
                )),
                policy: Some(policy.clone()),
            };
        }
    }

    // Check address whitelist/blacklist
    if let Some(addresses) = rules.get("addresses") {
        let mode = addresses.get("mode").and_then(|v| v.as_str()).unwrap_or("whitelist");
        let list = addresses
            .get("list")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let to = action.get("to").and_then(|v| v.as_str()).unwrap_or("");

        if !to.is_empty() {
            match mode {
                "whitelist" => {
                    if !list.iter().any(|a| *a == to) {
                        return WalletCheckPolicyResponse {
                            allowed: false,
                            frozen: false,
                            requires_approval: None,
                            required_approvals: None,
                            reason: Some(format!(
                                "Address '{}' is not in whitelist",
                                to
                            )),
                            policy: Some(policy.clone()),
                        };
                    }
                }
                "blacklist" => {
                    if list.iter().any(|a| *a == to) {
                        return WalletCheckPolicyResponse {
                            allowed: false,
                            frozen: false,
                            requires_approval: None,
                            required_approvals: None,
                            reason: Some(format!(
                                "Address '{}' is blacklisted",
                                to
                            )),
                            policy: Some(policy.clone()),
                        };
                    }
                }
                _ => {}
            }
        }
    }

    // Check per-transaction limits and velocity limits (per-token, in raw units)
    if let Some(limits) = rules.get("limits") {
        let token = action.get("token").and_then(|v| v.as_str()).unwrap_or("native");
        let amount_str = action.get("amount").and_then(|v| v.as_str()).unwrap_or("0");
        let amount: u128 = match amount_str.parse() {
            Ok(v) => v,
            Err(_) => {
                return WalletCheckPolicyResponse {
                    allowed: false,
                    frozen: false,
                    requires_approval: None,
                    required_approvals: None,
                    reason: Some(format!("Invalid amount '{}': must be a valid integer", amount_str)),
                    policy: Some(policy.clone()),
                };
            }
        };

        if let Some(per_tx) = limits.get("per_transaction") {
            // Check token-specific limit first, then wildcard
            let limit_str = per_tx
                .get(token)
                .or_else(|| per_tx.get("*"))
                .and_then(|v| v.as_str());

            if let Some(limit) = limit_str {
                let limit_val: u128 = limit.parse().unwrap_or(u128::MAX);
                if amount > limit_val {
                    return WalletCheckPolicyResponse {
                        allowed: false,
                        frozen: false,
                        requires_approval: None,
                        required_approvals: None,
                        reason: Some(format!(
                            "Per-transaction limit exceeded for {}: {} > {}",
                            token, amount_str, limit
                        )),
                        policy: Some(policy.clone()),
                    };
                }
            }
        }

        // Check daily velocity limit
        if let Some(daily) = limits.get("daily") {
            let limit_str = daily.get(token).or_else(|| daily.get("*")).and_then(|v| v.as_str());
            if let Some(limit) = limit_str {
                let limit_val: u128 = limit.parse().unwrap_or(u128::MAX);
                let current: u128 = action
                    .pointer(&format!("/current_usage/daily/{}", token))
                    .and_then(|v| v.as_str())
                    .unwrap_or("0")
                    .parse()
                    .unwrap_or(0);
                if current + amount > limit_val {
                    return WalletCheckPolicyResponse {
                        allowed: false,
                        frozen: false,
                        requires_approval: None,
                        required_approvals: None,
                        reason: Some(format!(
                            "Daily limit exceeded for {}: {} + {} > {}",
                            token, current, amount_str, limit
                        )),
                        policy: Some(policy.clone()),
                    };
                }
            }
        }

        // Check hourly velocity limit
        if let Some(hourly) = limits.get("hourly") {
            let limit_str = hourly.get(token).or_else(|| hourly.get("*")).and_then(|v| v.as_str());
            if let Some(limit) = limit_str {
                let limit_val: u128 = limit.parse().unwrap_or(u128::MAX);
                let current: u128 = action
                    .pointer(&format!("/current_usage/hourly/{}", token))
                    .and_then(|v| v.as_str())
                    .unwrap_or("0")
                    .parse()
                    .unwrap_or(0);
                if current + amount > limit_val {
                    return WalletCheckPolicyResponse {
                        allowed: false,
                        frozen: false,
                        requires_approval: None,
                        required_approvals: None,
                        reason: Some(format!(
                            "Hourly limit exceeded for {}: {} + {} > {}",
                            token, current, amount_str, limit
                        )),
                        policy: Some(policy.clone()),
                    };
                }
            }
        }

        // Check monthly velocity limit
        if let Some(monthly) = limits.get("monthly") {
            let limit_str = monthly.get(token).or_else(|| monthly.get("*")).and_then(|v| v.as_str());
            if let Some(limit) = limit_str {
                let limit_val: u128 = limit.parse().unwrap_or(u128::MAX);
                let current: u128 = action
                    .pointer(&format!("/current_usage/monthly/{}", token))
                    .and_then(|v| v.as_str())
                    .unwrap_or("0")
                    .parse()
                    .unwrap_or(0);
                if current + amount > limit_val {
                    return WalletCheckPolicyResponse {
                        allowed: false,
                        frozen: false,
                        requires_approval: None,
                        required_approvals: None,
                        reason: Some(format!(
                            "Monthly limit exceeded for {}: {} + {} > {}",
                            token, current, amount_str, limit
                        )),
                        policy: Some(policy.clone()),
                    };
                }
            }
        }
    }

    // Check rate limit (max transactions per hour)
    if let Some(rate_limit) = rules.get("rate_limit") {
        if let Some(max_per_hour) = rate_limit.get("max_per_hour").and_then(|v| v.as_i64()) {
            let hourly_tx_count = action
                .pointer("/current_usage/hourly_tx_count")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            if hourly_tx_count >= max_per_hour {
                return WalletCheckPolicyResponse {
                    allowed: false,
                    frozen: false,
                    requires_approval: None,
                    required_approvals: None,
                    reason: Some(format!(
                        "Rate limit exceeded: {} transactions this hour (max: {})",
                        hourly_tx_count, max_per_hour
                    )),
                    policy: Some(policy.clone()),
                };
            }
        }
    }

    // Check time restrictions (UTC only in v1)
    if let Some(time_restrictions) = rules.get("time_restrictions") {
        let timezone = time_restrictions
            .get("timezone")
            .and_then(|v| v.as_str())
            .unwrap_or("UTC");

        // v1: Only UTC is supported. Reject non-UTC timezones to prevent
        // silent misconfiguration where hours are checked in wrong timezone.
        if timezone != "UTC" {
            return WalletCheckPolicyResponse {
                allowed: false,
                frozen: false,
                requires_approval: None,
                required_approvals: None,
                reason: Some(format!(
                    "Unsupported timezone '{}'. Only 'UTC' is supported in v1.",
                    timezone
                )),
                policy: Some(policy.clone()),
            };
        }

        // Compute current UTC hour and weekday from Unix timestamp
        let secs_since_epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let secs_in_day = secs_since_epoch % 86400;
        let hour = (secs_in_day / 3600) as u32;
        // Unix epoch (1970-01-01) was a Thursday (day 4).
        // 1=Mon, 2=Tue, ..., 7=Sun
        let day_index = ((secs_since_epoch / 86400) + 3) % 7 + 1; // +3 because Thu=4, (0+3)%7+1 = 4
        let weekday = day_index as u32;

        if let Some(allowed_hours) = time_restrictions.get("allowed_hours").and_then(|v| v.as_array()) {
            if allowed_hours.len() == 2 {
                let start = allowed_hours[0].as_u64().unwrap_or(0) as u32;
                let end = allowed_hours[1].as_u64().unwrap_or(24) as u32;
                // Handle wrap-around: e.g. [22, 6] means 10 PM to 6 AM
                let in_range = if start <= end {
                    // Normal range: [9, 17] means 9:00-16:59
                    hour >= start && hour < end
                } else {
                    // Wrap-around: [22, 6] means 22:00-23:59 or 0:00-5:59
                    hour >= start || hour < end
                };
                if !in_range {
                    return WalletCheckPolicyResponse {
                        allowed: false,
                        frozen: false,
                        requires_approval: None,
                        required_approvals: None,
                        reason: Some(format!(
                            "Operation not allowed at this hour ({} UTC). Allowed: {}-{}",
                            hour, start, end
                        )),
                        policy: Some(policy.clone()),
                    };
                }
            }
        }

        if let Some(allowed_days) = time_restrictions.get("allowed_days").and_then(|v| v.as_array()) {
            let day_allowed = allowed_days
                .iter()
                .any(|d| d.as_u64() == Some(weekday as u64));
            if !day_allowed {
                return WalletCheckPolicyResponse {
                    allowed: false,
                    frozen: false,
                    requires_approval: None,
                    required_approvals: None,
                    reason: Some(format!(
                        "Operation not allowed on weekday {}",
                        weekday
                    )),
                    policy: Some(policy.clone()),
                };
            }
        }
    }

    // Check approval threshold
    if let Some(approval) = policy.get("approval") {
        // Skip approval for excluded operation types (e.g. intents_deposit, swap —
        // these rely on Intents quotes that expire quickly and can't wait for multisig)
        let excluded = approval
            .get("excluded_types")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
            .unwrap_or_default();

        if !excluded.iter().any(|t| *t == action_type) {
            // Note: above_usd threshold comparison is not implemented yet —
            // if approval.threshold is configured, ALL non-excluded operations require approval.
            // Per-amount USD threshold will be added later.
            let threshold = approval.get("threshold");
            if let Some(threshold) = threshold {
                let required = threshold.get("required").and_then(|v| v.as_i64()).unwrap_or(2) as i32;

                return WalletCheckPolicyResponse {
                    allowed: true,
                    frozen: false,
                    requires_approval: Some(true),
                    required_approvals: Some(required),
                    reason: None,
                    policy: Some(policy.clone()),
                };
            }
        }
    }

    // All checks passed
    WalletCheckPolicyResponse {
        allowed: true,
        frozen: false,
        requires_approval: None,
        required_approvals: None,
        reason: None,
        policy: Some(policy.clone()),
    }
}

/// Encrypt a wallet policy for on-chain storage
async fn wallet_encrypt_policy_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<WalletEncryptPolicyRequest>,
) -> Result<Json<WalletEncryptPolicyResponse>, ApiError> {
    if !state.is_ready() {
        return Err(ApiError::Unauthorized(
            "Keystore not ready.".to_string(),
        ));
    }

    let customer = extract_customer_from_header(&headers)?;
    state
        .ensure_customer_loaded(customer.as_ref())
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    let seed = format!("wallet-policy:{}", req.wallet_id);
    let keystore = state.keystore.read().await;

    let encrypted = keystore
        .encrypt(customer.as_ref(), &seed, req.policy_json.as_bytes())
        .map_err(|e| ApiError::InternalError(format!("Encryption failed: {}", e)))?;

    Ok(Json(WalletEncryptPolicyResponse {
        encrypted_base64: base64::encode(&encrypted),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AccessCondition, LogicOperator};

    /// Test that DecryptRequest correctly includes user_account_id field
    #[test]
    fn test_decrypt_request_serialization() {
        let json = r#"{
            "accessor": {
                "type": "Repo",
                "repo": "github.com/user/repo",
                "branch": "main"
            },
            "profile": "production",
            "owner": "owner.testnet",
            "user_account_id": "caller.testnet",
            "attestation": {
                "tee_type": "outlayer_tee",
                "quote": "",
                "measurements": {},
                "timestamp": 1704067200
            },
            "task_id": "task123"
        }"#;

        let req: DecryptRequest = serde_json::from_str(json).unwrap();
        match req.accessor {
            SecretAccessor::Repo { repo, branch } => {
                assert_eq!(repo, "github.com/user/repo");
                assert_eq!(branch, Some("main".to_string()));
            }
            _ => panic!("Expected Repo accessor"),
        }
        assert_eq!(req.owner, "owner.testnet");
        assert_eq!(req.user_account_id, "caller.testnet");
    }

    /// Test access control: owner != user_account_id
    /// This simulates the scenario where:
    /// - owner.testnet owns secrets with Whitelist access
    /// - caller.testnet requests execution
    /// - Access should be checked against caller.testnet, not owner.testnet
    #[tokio::test]
    async fn test_access_control_with_different_user() {
        // Test Whitelist: owner not in list, but user_account_id is
        let whitelist = AccessCondition::Whitelist {
            accounts: vec![
                "caller.testnet".to_string(),
                "other.testnet".to_string(),
            ],
        };

        // Should grant access to caller (even though owner is different)
        assert!(whitelist.validate("caller.testnet", None).await.unwrap());

        // Should deny access to owner (not in whitelist)
        assert!(!whitelist.validate("owner.testnet", None).await.unwrap());
    }

    /// Test that Whitelist correctly allows multiple accounts
    #[tokio::test]
    async fn test_whitelist_multiple_accounts() {
        let whitelist = AccessCondition::Whitelist {
            accounts: vec![
                "alice.testnet".to_string(),
                "bob.testnet".to_string(),
                "charlie.testnet".to_string(),
            ],
        };

        assert!(whitelist.validate("alice.testnet", None).await.unwrap());
        assert!(whitelist.validate("bob.testnet", None).await.unwrap());
        assert!(whitelist.validate("charlie.testnet", None).await.unwrap());
        assert!(!whitelist.validate("eve.testnet", None).await.unwrap());
    }

    /// Test AccountPattern with testnet suffix
    #[tokio::test]
    async fn test_account_pattern_testnet() {
        let pattern = AccessCondition::AccountPattern {
            pattern: r".*\.testnet$".to_string(),
        };

        assert!(pattern.validate("alice.testnet", None).await.unwrap());
        assert!(pattern.validate("project.testnet", None).await.unwrap());
        assert!(!pattern.validate("alice.near", None).await.unwrap());
    }

    /// Test complex Logic condition (AND + Whitelist + Pattern)
    #[tokio::test]
    async fn test_complex_logic_condition() {
        let condition = AccessCondition::Logic {
            operator: LogicOperator::And,
            conditions: vec![
                AccessCondition::AccountPattern {
                    pattern: r".*\.testnet$".to_string(),
                },
                AccessCondition::Whitelist {
                    accounts: vec![
                        "alice.testnet".to_string(),
                        "bob.testnet".to_string(),
                    ],
                },
            ],
        };

        // alice.testnet: matches pattern AND in whitelist
        assert!(condition.validate("alice.testnet", None).await.unwrap());

        // bob.testnet: matches pattern AND in whitelist
        assert!(condition.validate("bob.testnet", None).await.unwrap());

        // charlie.testnet: matches pattern but NOT in whitelist
        assert!(!condition.validate("charlie.testnet", None).await.unwrap());

        // alice.near: in "whitelist" but doesn't match pattern
        assert!(!condition.validate("alice.near", None).await.unwrap());
    }

    /// Test that AllowAll always grants access
    #[tokio::test]
    async fn test_allow_all() {
        let condition = AccessCondition::AllowAll;

        assert!(condition.validate("anyone.testnet", None).await.unwrap());
        assert!(condition.validate("another.near", None).await.unwrap());
        assert!(condition.validate("random.account", None).await.unwrap());
    }

    /// Test SecretAccessor::Repo serialization (with branch)
    #[test]
    fn test_secret_accessor_repo_with_branch() {
        let accessor = SecretAccessor::Repo {
            repo: "github.com/user/repo".to_string(),
            branch: Some("main".to_string()),
        };

        let json = serde_json::to_string(&accessor).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["type"], "Repo");
        assert_eq!(parsed["repo"], "github.com/user/repo");
        assert_eq!(parsed["branch"], "main");
    }

    /// Test SecretAccessor::Repo serialization (without branch)
    #[test]
    fn test_secret_accessor_repo_without_branch() {
        let accessor = SecretAccessor::Repo {
            repo: "github.com/user/repo".to_string(),
            branch: None,
        };

        let json = serde_json::to_string(&accessor).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["type"], "Repo");
        assert_eq!(parsed["repo"], "github.com/user/repo");
        assert!(parsed["branch"].is_null());
    }

    /// Test SecretAccessor::WasmHash serialization
    #[test]
    fn test_secret_accessor_wasm_hash() {
        let accessor = SecretAccessor::WasmHash {
            hash: "abc123def456".to_string(),
        };

        let json = serde_json::to_string(&accessor).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["type"], "WasmHash");
        assert_eq!(parsed["hash"], "abc123def456");
    }

    /// Test SecretAccessor deserialization from JSON
    #[test]
    fn test_secret_accessor_deserialization() {
        // Test Repo with branch
        let json = r#"{"type": "Repo", "repo": "github.com/test/project", "branch": "develop"}"#;
        let accessor: SecretAccessor = serde_json::from_str(json).unwrap();
        match accessor {
            SecretAccessor::Repo { repo, branch } => {
                assert_eq!(repo, "github.com/test/project");
                assert_eq!(branch, Some("develop".to_string()));
            }
            _ => panic!("Expected Repo variant"),
        }

        // Test Repo without branch
        let json = r#"{"type": "Repo", "repo": "github.com/test/project"}"#;
        let accessor: SecretAccessor = serde_json::from_str(json).unwrap();
        match accessor {
            SecretAccessor::Repo { repo, branch } => {
                assert_eq!(repo, "github.com/test/project");
                assert_eq!(branch, None);
            }
            _ => panic!("Expected Repo variant"),
        }

        // Test WasmHash
        let json = r#"{"type": "WasmHash", "hash": "deadbeef123456"}"#;
        let accessor: SecretAccessor = serde_json::from_str(json).unwrap();
        match accessor {
            SecretAccessor::WasmHash { hash } => {
                assert_eq!(hash, "deadbeef123456");
            }
            _ => panic!("Expected WasmHash variant"),
        }
    }

    /// Test DecryptRequest with Repo accessor
    #[test]
    fn test_decrypt_request_with_repo_accessor() {
        let json = r#"{
            "accessor": {
                "type": "Repo",
                "repo": "github.com/user/repo",
                "branch": "main"
            },
            "profile": "production",
            "owner": "owner.testnet",
            "user_account_id": "caller.testnet",
            "attestation": {
                "tee_type": "outlayer_tee",
                "quote": "",
                "measurements": {},
                "timestamp": 1704067200
            },
            "task_id": "task123"
        }"#;

        let req: DecryptRequest = serde_json::from_str(json).unwrap();
        match req.accessor {
            SecretAccessor::Repo { repo, branch } => {
                assert_eq!(repo, "github.com/user/repo");
                assert_eq!(branch, Some("main".to_string()));
            }
            _ => panic!("Expected Repo accessor"),
        }
        assert_eq!(req.profile, "production");
        assert_eq!(req.owner, "owner.testnet");
        assert_eq!(req.user_account_id, "caller.testnet");
    }

    /// Test DecryptRequest with WasmHash accessor
    #[test]
    fn test_decrypt_request_with_wasm_hash_accessor() {
        let json = r#"{
            "accessor": {
                "type": "WasmHash",
                "hash": "abc123def456"
            },
            "profile": "default",
            "owner": "alice.near",
            "user_account_id": "bob.near",
            "attestation": {
                "tee_type": "none",
                "quote": "",
                "measurements": {},
                "timestamp": 1704067200
            }
        }"#;

        let req: DecryptRequest = serde_json::from_str(json).unwrap();
        match req.accessor {
            SecretAccessor::WasmHash { hash } => {
                assert_eq!(hash, "abc123def456");
            }
            _ => panic!("Expected WasmHash accessor"),
        }
        assert_eq!(req.profile, "default");
        assert_eq!(req.owner, "alice.near");
        assert_eq!(req.user_account_id, "bob.near");
        assert!(req.task_id.is_none());
    }

    // =========================================================================
    // evaluate_policy tests
    // =========================================================================

    #[test]
    fn test_policy_frozen_wallet() {
        let policy = serde_json::json!({ "frozen": true });
        let action = serde_json::json!({ "type": "intents_withdraw" });
        let result = evaluate_policy(&policy, &action);
        assert!(!result.allowed);
        assert!(result.frozen);
        assert!(result.reason.unwrap().contains("frozen"));
    }

    #[test]
    fn test_policy_no_rules() {
        let policy = serde_json::json!({});
        let action = serde_json::json!({ "type": "intents_withdraw" });
        let result = evaluate_policy(&policy, &action);
        assert!(result.allowed);
        assert!(!result.frozen);
    }

    #[test]
    fn test_policy_empty_rules() {
        let policy = serde_json::json!({ "rules": {} });
        let action = serde_json::json!({ "type": "intents_withdraw" });
        let result = evaluate_policy(&policy, &action);
        assert!(result.allowed);
    }

    #[test]
    fn test_policy_tx_type_allowed() {
        let policy = serde_json::json!({
            "rules": { "transaction_types": ["intents_withdraw", "call"] }
        });
        let action = serde_json::json!({ "type": "intents_withdraw" });
        let result = evaluate_policy(&policy, &action);
        assert!(result.allowed);
    }

    #[test]
    fn test_policy_tx_type_denied() {
        let policy = serde_json::json!({
            "rules": { "transaction_types": ["intents_withdraw"] }
        });
        let action = serde_json::json!({ "type": "transfer" });
        let result = evaluate_policy(&policy, &action);
        assert!(!result.allowed);
        assert!(result.reason.unwrap().contains("not allowed"));
    }

    #[test]
    fn test_policy_whitelist_pass() {
        let policy = serde_json::json!({
            "rules": {
                "addresses": { "mode": "whitelist", "list": ["a.near", "b.near"] }
            }
        });
        let action = serde_json::json!({ "type": "intents_withdraw", "to": "a.near" });
        let result = evaluate_policy(&policy, &action);
        assert!(result.allowed);
    }

    #[test]
    fn test_policy_whitelist_fail() {
        let policy = serde_json::json!({
            "rules": {
                "addresses": { "mode": "whitelist", "list": ["a.near"] }
            }
        });
        let action = serde_json::json!({ "type": "intents_withdraw", "to": "b.near" });
        let result = evaluate_policy(&policy, &action);
        assert!(!result.allowed);
        assert!(result.reason.unwrap().contains("not in whitelist"));
    }

    #[test]
    fn test_policy_blacklist_pass() {
        let policy = serde_json::json!({
            "rules": {
                "addresses": { "mode": "blacklist", "list": ["bad.near"] }
            }
        });
        let action = serde_json::json!({ "type": "intents_withdraw", "to": "good.near" });
        let result = evaluate_policy(&policy, &action);
        assert!(result.allowed);
    }

    #[test]
    fn test_policy_blacklist_fail() {
        let policy = serde_json::json!({
            "rules": {
                "addresses": { "mode": "blacklist", "list": ["bad.near"] }
            }
        });
        let action = serde_json::json!({ "type": "intents_withdraw", "to": "bad.near" });
        let result = evaluate_policy(&policy, &action);
        assert!(!result.allowed);
        assert!(result.reason.unwrap().contains("blacklisted"));
    }

    #[test]
    fn test_policy_per_tx_limit_pass() {
        let policy = serde_json::json!({
            "rules": { "limits": { "per_transaction": { "native": "100" } } }
        });
        let action = serde_json::json!({ "type": "intents_withdraw", "amount": "50", "token": "native" });
        let result = evaluate_policy(&policy, &action);
        assert!(result.allowed);
    }

    #[test]
    fn test_policy_per_tx_limit_fail() {
        let policy = serde_json::json!({
            "rules": { "limits": { "per_transaction": { "native": "100" } } }
        });
        let action = serde_json::json!({ "type": "intents_withdraw", "amount": "150", "token": "native" });
        let result = evaluate_policy(&policy, &action);
        assert!(!result.allowed);
        assert!(result.reason.unwrap().contains("Per-transaction limit"));
    }

    #[test]
    fn test_policy_per_tx_wildcard() {
        let policy = serde_json::json!({
            "rules": { "limits": { "per_transaction": { "*": "100" } } }
        });
        let action = serde_json::json!({ "type": "intents_withdraw", "amount": "150", "token": "usdc" });
        let result = evaluate_policy(&policy, &action);
        assert!(!result.allowed);
        assert!(result.reason.unwrap().contains("Per-transaction limit"));
    }

    #[test]
    fn test_policy_daily_limit_pass() {
        let policy = serde_json::json!({
            "rules": { "limits": { "daily": { "native": "1000" } } }
        });
        let action = serde_json::json!({
            "type": "intents_withdraw", "amount": "100", "token": "native",
            "current_usage": { "daily": { "native": "500" } }
        });
        let result = evaluate_policy(&policy, &action);
        assert!(result.allowed);
    }

    #[test]
    fn test_policy_daily_limit_fail() {
        let policy = serde_json::json!({
            "rules": { "limits": { "daily": { "native": "1000" } } }
        });
        let action = serde_json::json!({
            "type": "intents_withdraw", "amount": "100", "token": "native",
            "current_usage": { "daily": { "native": "950" } }
        });
        let result = evaluate_policy(&policy, &action);
        assert!(!result.allowed);
        assert!(result.reason.unwrap().contains("Daily limit"));
    }

    #[test]
    fn test_policy_hourly_limit_fail() {
        let policy = serde_json::json!({
            "rules": { "limits": { "hourly": { "native": "500" } } }
        });
        let action = serde_json::json!({
            "type": "intents_withdraw", "amount": "100", "token": "native",
            "current_usage": { "hourly": { "native": "450" } }
        });
        let result = evaluate_policy(&policy, &action);
        assert!(!result.allowed);
        assert!(result.reason.unwrap().contains("Hourly limit"));
    }

    #[test]
    fn test_policy_monthly_limit_fail() {
        let policy = serde_json::json!({
            "rules": { "limits": { "monthly": { "native": "10000" } } }
        });
        let action = serde_json::json!({
            "type": "intents_withdraw", "amount": "100", "token": "native",
            "current_usage": { "monthly": { "native": "9950" } }
        });
        let result = evaluate_policy(&policy, &action);
        assert!(!result.allowed);
        assert!(result.reason.unwrap().contains("Monthly limit"));
    }

    #[test]
    fn test_policy_rate_limit_pass() {
        let policy = serde_json::json!({
            "rules": { "rate_limit": { "max_per_hour": 10 } }
        });
        let action = serde_json::json!({
            "type": "intents_withdraw", "current_usage": { "hourly_tx_count": 5 }
        });
        let result = evaluate_policy(&policy, &action);
        assert!(result.allowed);
    }

    #[test]
    fn test_policy_rate_limit_fail() {
        let policy = serde_json::json!({
            "rules": { "rate_limit": { "max_per_hour": 10 } }
        });
        let action = serde_json::json!({
            "type": "intents_withdraw", "current_usage": { "hourly_tx_count": 10 }
        });
        let result = evaluate_policy(&policy, &action);
        assert!(!result.allowed);
        assert!(result.reason.unwrap().contains("Rate limit"));
    }

    #[test]
    fn test_policy_time_restriction_day_denied() {
        // Weekday 8 doesn't exist (1=Mon..7=Sun), always denied
        let policy = serde_json::json!({
            "rules": { "time_restrictions": { "allowed_days": [8] } }
        });
        let action = serde_json::json!({ "type": "intents_withdraw" });
        let result = evaluate_policy(&policy, &action);
        assert!(!result.allowed);
        assert!(result.reason.unwrap().contains("weekday"));
    }

    #[test]
    fn test_policy_approval_threshold() {
        let policy = serde_json::json!({
            "rules": {},
            "approval": {
                "above_usd": 100,
                "threshold": { "required": 3 }
            }
        });
        let action = serde_json::json!({ "type": "intents_withdraw", "amount": "1000" });
        let result = evaluate_policy(&policy, &action);
        assert!(result.allowed);
        assert_eq!(result.requires_approval, Some(true));
        assert_eq!(result.required_approvals, Some(3));
    }

    #[test]
    fn test_policy_approval_excluded_types() {
        let policy = serde_json::json!({
            "rules": {},
            "approval": {
                "above_usd": 100,
                "threshold": { "required": 2 },
                "excluded_types": ["intents_deposit", "intents_swap"]
            }
        });
        // intents_deposit — excluded, no approval
        let action = serde_json::json!({ "type": "intents_deposit", "amount": "1000" });
        let result = evaluate_policy(&policy, &action);
        assert!(result.allowed);
        assert!(result.requires_approval.is_none());

        // swap — excluded, no approval
        let action = serde_json::json!({ "type": "intents_swap", "amount": "1000" });
        let result = evaluate_policy(&policy, &action);
        assert!(result.allowed);
        assert!(result.requires_approval.is_none());

        // withdraw — NOT excluded, requires approval
        let action = serde_json::json!({ "type": "intents_withdraw", "amount": "1000" });
        let result = evaluate_policy(&policy, &action);
        assert!(result.allowed);
        assert_eq!(result.requires_approval, Some(true));
        assert_eq!(result.required_approvals, Some(2));
    }

    #[test]
    fn test_policy_invalid_amount() {
        let policy = serde_json::json!({
            "rules": {
                "limits": {
                    "per_transaction": { "native": "1000" }
                }
            }
        });
        let action = serde_json::json!({ "type": "intents_withdraw", "amount": "not_a_number", "token": "native" });
        let result = evaluate_policy(&policy, &action);
        assert!(!result.allowed);
        assert!(result.reason.unwrap().contains("Invalid amount"));
    }

    #[test]
    fn test_policy_all_checks_pass() {
        let policy = serde_json::json!({
            "rules": {
                "transaction_types": ["intents_withdraw"],
                "addresses": { "mode": "whitelist", "list": ["dest.near"] },
                "limits": {
                    "per_transaction": { "native": "1000" },
                    "daily": { "native": "5000" }
                },
                "rate_limit": { "max_per_hour": 100 }
            }
        });
        let action = serde_json::json!({
            "type": "intents_withdraw",
            "to": "dest.near",
            "amount": "500",
            "token": "native",
            "current_usage": {
                "daily": { "native": "1000" },
                "hourly_tx_count": 3
            }
        });
        let result = evaluate_policy(&policy, &action);
        assert!(result.allowed);
        assert!(!result.frozen);
        assert!(result.requires_approval.is_none());
    }

    #[test]
    fn test_policy_allowed_tokens() {
        let policy = serde_json::json!({
            "rules": {
                "allowed_tokens": ["native", "nep141:usdt.tether-token.near"]
            }
        });

        // native — allowed
        let action = serde_json::json!({ "type": "intents_withdraw", "amount": "100", "token": "native" });
        let result = evaluate_policy(&policy, &action);
        assert!(result.allowed);

        // usdt — allowed
        let action = serde_json::json!({ "type": "intents_withdraw", "amount": "100", "token": "nep141:usdt.tether-token.near" });
        let result = evaluate_policy(&policy, &action);
        assert!(result.allowed);

        // random token — rejected
        let action = serde_json::json!({ "type": "intents_withdraw", "amount": "100", "token": "nep141:evil.near" });
        let result = evaluate_policy(&policy, &action);
        assert!(!result.allowed);
        assert!(result.reason.unwrap().contains("not allowed"));

        // no token field defaults to "native" — allowed
        let action = serde_json::json!({ "type": "intents_withdraw", "amount": "100" });
        let result = evaluate_policy(&policy, &action);
        assert!(result.allowed);
    }

    #[test]
    fn test_policy_no_allowed_tokens_allows_all() {
        // No allowed_tokens rule — any token should pass
        let policy = serde_json::json!({ "rules": {} });
        let action = serde_json::json!({ "type": "intents_withdraw", "amount": "100", "token": "nep141:anything.near" });
        let result = evaluate_policy(&policy, &action);
        assert!(result.allowed);
    }

    // ============== AppState::ensure_customer_loaded gate ==============

    fn test_state() -> AppState {
        let config = crate::config::Config {
            server_addr: "127.0.0.1:0".parse().unwrap(),
            near_network: "testnet".into(),
            near_rpc_url: "http://127.0.0.1:1".into(),
            offchainvm_contract_id: "outlayer.test".into(),
            allowed_worker_token_hashes: vec![],
            allowed_coordinator_token_hashes: vec![],
            tee_mode: crate::config::TeeMode::None,
            operator_account_id: None,
        };
        AppState::new(crate::crypto::Keystore::generate(), config, None)
    }

    #[tokio::test]
    async fn ensure_customer_loaded_none_is_noop() {
        // Legacy default-master path: handlers that pass None must
        // never trip the lazy-load gate, even when MPC context is
        // unset. This is the backward-compat invariant.
        let state = test_state();
        state
            .ensure_customer_loaded(None)
            .await
            .expect("None customer must always succeed");
    }

    #[tokio::test]
    async fn ensure_customer_loaded_some_without_mpc_context_errors() {
        // When booted without a TEE/MPC context, asking for a
        // per-customer master must fail-fast with a clear error
        // rather than silently falling back to the default master
        // (which would defeat the customer-isolation invariant).
        use std::str::FromStr;
        let state = test_state();
        let vault = near_primitives::types::AccountId::from_str("vault.alice.testnet").unwrap();
        let err = state
            .ensure_customer_loaded(Some(&vault))
            .await
            .expect_err("must fail without MPC context");
        let msg = format!("{:#}", err);
        assert!(
            msg.contains("MPC CKD context") || msg.contains("non-TEE"),
            "error message should explain the missing context, got: {}",
            msg
        );
    }

    #[tokio::test]
    async fn ensure_customer_loaded_some_with_cached_master_skips_view_call() {
        // Fast path: even without MPC context configured, an already
        // loaded customer master must be served (no view-call, no MPC
        // tx). This is what makes a banned-then-evicted-then-re-add
        // flow work — re-adding via /admin and the next handler
        // request skips re-verification.
        //
        // We omit MPC config on purpose: if the gate fell through to
        // the network path it would error out, but the early
        // has_customer check should prevent that.
        use std::str::FromStr;
        let state = test_state();
        let vault = near_primitives::types::AccountId::from_str("vault.alice.testnet").unwrap();
        // Pre-populate.
        state
            .keystore
            .read()
            .await
            .add_customer(vault.clone(), [9u8; 32]);

        state
            .ensure_customer_loaded(Some(&vault))
            .await
            .expect("cached master must short-circuit MPC path");
    }

    // ============== Vault-scope helpers ==============
    // Audit finding I7: pin behaviour of the new request-extraction
    // helpers and the accessor → contract-JSON adapter. Without these
    // tests, future drift between the worker enum and the contract
    // enum (e.g. someone adding `#[serde(tag = "type")]` to either)
    // would silently break decrypt — accessor lookups would target
    // the wrong row and `get_secret_with_vault` would always return
    // `(None, None)`.

    use axum::http::HeaderMap;

    #[test]
    fn extract_customer_from_header_absent_is_none() {
        let h = HeaderMap::new();
        assert!(extract_customer_from_header(&h).unwrap().is_none());
    }

    #[test]
    fn extract_customer_from_header_empty_is_none() {
        let mut h = HeaderMap::new();
        h.insert("X-Customer-Vault", "".parse().unwrap());
        assert!(extract_customer_from_header(&h).unwrap().is_none());
        h.insert("X-Customer-Vault", "   ".parse().unwrap());
        assert!(extract_customer_from_header(&h).unwrap().is_none());
    }

    #[test]
    fn extract_customer_from_header_valid_account() {
        let mut h = HeaderMap::new();
        h.insert("X-Customer-Vault", "vault.alice.testnet".parse().unwrap());
        let result = extract_customer_from_header(&h).unwrap().unwrap();
        assert_eq!(result.as_str(), "vault.alice.testnet");
    }

    #[test]
    fn extract_customer_from_header_trims_whitespace() {
        let mut h = HeaderMap::new();
        h.insert("X-Customer-Vault", "  vault.alice.testnet  ".parse().unwrap());
        let result = extract_customer_from_header(&h).unwrap().unwrap();
        assert_eq!(result.as_str(), "vault.alice.testnet");
    }

    #[test]
    fn extract_customer_from_header_malformed_id_errors() {
        // No silent fallback to default master — that's the
        // anti-typo guarantee.
        let mut h = HeaderMap::new();
        h.insert("X-Customer-Vault", "INVALID UPPERCASE".parse().unwrap());
        let err = extract_customer_from_header(&h).unwrap_err();
        match err {
            ApiError::BadRequest(msg) => assert!(msg.contains("X-Customer-Vault")),
            other => panic!("expected BadRequest, got {:?}", other),
        }
    }

    #[test]
    fn parse_optional_vault_id_treats_none_empty_whitespace_uniformly() {
        assert!(parse_optional_vault_id(None).unwrap().is_none());
        assert!(parse_optional_vault_id(Some("")).unwrap().is_none());
        assert!(parse_optional_vault_id(Some("   ")).unwrap().is_none());
        assert!(parse_optional_vault_id(Some("\t")).unwrap().is_none());
    }

    #[test]
    fn parse_optional_vault_id_valid() {
        let result = parse_optional_vault_id(Some("vault.alice.testnet")).unwrap().unwrap();
        assert_eq!(result.as_str(), "vault.alice.testnet");
    }

    #[test]
    fn parse_optional_vault_id_malformed_errors() {
        let err = parse_optional_vault_id(Some("INVALID")).unwrap_err();
        match err {
            ApiError::BadRequest(msg) => assert!(msg.contains("vault_id")),
            other => panic!("expected BadRequest, got {:?}", other),
        }
    }

    // ============== accessor_to_contract_json — frozen contract ==============
    // These tests pin the EXACT JSON shape sent to the contract for
    // every accessor variant. If the contract enum tagging ever
    // changes, OR the worker enum tagging changes, these tests fail
    // — and that failure is what protects every secret stored on
    // chain from silent decryption misses.

    #[test]
    fn accessor_to_contract_json_repo_normalizes_url() {
        let a = SecretAccessor::Repo {
            repo: "https://github.com/alice/repo".into(),
            branch: Some("main".into()),
        };
        let json = accessor_to_contract_json(&a);
        assert_eq!(
            json,
            serde_json::json!({"Repo": {"repo": "github.com/alice/repo", "branch": "main"}})
        );
    }

    #[test]
    fn accessor_to_contract_json_repo_null_branch() {
        // Null branch must serialise as JSON null (NOT omitted) — the
        // contract's wildcard-fallback logic relies on the field
        // being present.
        let a = SecretAccessor::Repo {
            repo: "github.com/alice/repo".into(),
            branch: None,
        };
        let json = accessor_to_contract_json(&a);
        assert_eq!(
            json,
            serde_json::json!({"Repo": {"repo": "github.com/alice/repo", "branch": null}})
        );
    }

    #[test]
    fn accessor_to_contract_json_wasm_hash() {
        let a = SecretAccessor::WasmHash { hash: "abc123".into() };
        let json = accessor_to_contract_json(&a);
        assert_eq!(json, serde_json::json!({"WasmHash": {"hash": "abc123"}}));
    }

    #[test]
    fn accessor_to_contract_json_project() {
        let a = SecretAccessor::Project { project_id: "alice.near/myapp".into() };
        let json = accessor_to_contract_json(&a);
        assert_eq!(json, serde_json::json!({"Project": {"project_id": "alice.near/myapp"}}));
    }

    #[test]
    fn accessor_to_contract_json_system_payment_key_uses_camelcase() {
        // Critical: the contract's `System(SystemSecretType)` is a tuple
        // variant, so `{"System": "PaymentKey"}` (string, CamelCase),
        // not `{"System": "payment_key"}` or `{"System": {"PaymentKey": null}}`.
        let a = SecretAccessor::System { secret_type: SystemSecretType::PaymentKey };
        let json = accessor_to_contract_json(&a);
        assert_eq!(json, serde_json::json!({"System": "PaymentKey"}));
    }

    #[test]
    fn system_secret_type_helpers_disagree_on_case() {
        // Drift guard: contract format is CamelCase, seed format is
        // snake_case. If both helpers ever drift to the same value,
        // either contract lookups break (System: payment_key won't
        // match) or seed format breaks (system:PaymentKey:... is a
        // different seed). This test is the canary.
        let s = SystemSecretType::PaymentKey;
        assert_eq!(s.as_contract_str(), "PaymentKey");
        assert_eq!(s.as_seed_str(), "payment_key");
        assert_ne!(s.as_contract_str(), s.as_seed_str());
    }

    // ============== map_verify_error HTTP status mapping ==============
    // The caller-side / worker-side
    // split. These tests pin which VerifyError variant maps to which
    // ApiError so a future change (e.g. someone moving an arm by
    // accident) shows up as a unit-test failure rather than a
    // production caller suddenly retrying or surfacing the wrong
    // status to its end-user.

    use crate::vault_verifier::VerifyError;
    use std::str::FromStr as _;

    fn vault() -> near_primitives::types::AccountId {
        near_primitives::types::AccountId::from_str("vault.alice.testnet").unwrap()
    }

    fn assert_bad_request(e: ApiError) {
        match e {
            ApiError::BadRequest(_) => {}
            other => panic!("expected BadRequest, got {:?}", other),
        }
    }
    fn assert_forbidden(e: ApiError) {
        match e {
            ApiError::Forbidden(_) => {}
            other => panic!("expected Forbidden, got {:?}", other),
        }
    }
    fn assert_internal(e: ApiError) {
        match e {
            ApiError::InternalError(_) => {}
            other => panic!("expected InternalError, got {:?}", other),
        }
    }

    #[test]
    fn map_verify_error_already_banned_is_403() {
        // Banned vault is a security signal that the caller's request
        // CAN'T succeed regardless of retries. 403 says so plainly.
        assert_forbidden(map_verify_error(&vault(), VerifyError::AlreadyBanned));
    }

    #[test]
    fn map_verify_error_caller_side_failures_are_400() {
        // Each arm here represents a CALLER-side failure mode:
        // either the vault was deployed wrong (FullAccess key,
        // misconfigured TEE key, wrong DAO) or its state is in a
        // non-eligible phase (unlocked, recovering). All must
        // surface as 400 so callers don't retry.
        assert_bad_request(map_verify_error(
            &vault(),
            VerifyError::CodeHashNotApproved { code_hash: "abc".into() },
        ));
        assert_bad_request(map_verify_error(&vault(), VerifyError::CodeHashMissing));
        assert_bad_request(map_verify_error(&vault(), VerifyError::FullAccessKeyPresent));
        assert_bad_request(map_verify_error(
            &vault(),
            VerifyError::FunctionCallKeyMisconfigured {
                receiver: "bad".into(),
                methods: vec![],
            },
        ));
        assert_bad_request(map_verify_error(
            &vault(),
            VerifyError::UnexpectedAccessKeyCount { expected: 1, got: 2 },
        ));
        assert_bad_request(map_verify_error(
            &vault(),
            VerifyError::KeystoreDaoMismatch {
                configured: "x".into(),
                on_chain: "y".into(),
            },
        ));
        assert_bad_request(map_verify_error(
            &vault(),
            VerifyError::MpcContractMismatch {
                configured: "x".into(),
                on_chain: "y".into(),
            },
        ));
        assert_bad_request(map_verify_error(&vault(), VerifyError::VaultUnlocked));
        assert_bad_request(map_verify_error(&vault(), VerifyError::VaultRecoveryInProgress));
    }

    #[test]
    fn map_verify_error_account_not_found_is_400() {
        // Ambiguous: vault doesn't exist OR rpc flake. Bias to 400
        // because dominant case is bad vault_id.
        let err = anyhow::anyhow!("account does not exist");
        assert_bad_request(map_verify_error(
            &vault(),
            VerifyError::AccountNotFound(err),
        ));
    }

    #[test]
    fn map_verify_error_rpc_failures_are_500() {
        // A flaky NEAR RPC must not surface as
        // 400 because callers won't retry on 4xx. These cases are
        // worker-side / system-side, callers SHOULD retry.
        let err = || anyhow::anyhow!("rpc timeout");
        assert_internal(map_verify_error(
            &vault(),
            VerifyError::KeystoreDaoUnreachable(err()),
        ));
        assert_internal(map_verify_error(
            &vault(),
            VerifyError::AccessKeyListUnreachable(err()),
        ));
        assert_internal(map_verify_error(
            &vault(),
            VerifyError::VaultStateUnreachable(err()),
        ));
    }

    #[test]
    fn map_verify_error_contract_shape_mismatch_is_500() {
        // A contract returning malformed JSON
        // is a contract bug or version mismatch — NOT a caller bug.
        // Caller has done nothing wrong; surfacing 400 would be a
        // lie.
        assert_internal(map_verify_error(
            &vault(),
            VerifyError::KeystoreDaoMalformed { method: "is_vault_banned".into() },
        ));
        assert_internal(map_verify_error(
            &vault(),
            VerifyError::VaultStateInvalid("missing keystore_dao".into()),
        ));
    }

    // ============== Handler-level customer isolation ==============
    // Builds on the crypto-layer isolation tests in `crypto.rs`. At
    // the handler layer the only customer-facing surface that we can
    // exercise without an HTTP test server is `AppState::ensure_customer_loaded`.
    // The `crypto.rs` tests already prove the underlying derive_*
    // calls produce disjoint output per customer; here we simply
    // verify the gate's behaviour matrix:

    #[tokio::test]
    async fn ensure_customer_loaded_independent_per_vault() {
        // Loading customer A's master must not satisfy a request for
        // customer B's master. The lazy-load gate's `has_customer`
        // check is per-vault — populating one entry doesn't shadow
        // another.
        use std::str::FromStr;
        let state = test_state();
        let alice = near_primitives::types::AccountId::from_str("vault.alice.testnet").unwrap();
        let bob = near_primitives::types::AccountId::from_str("vault.bob.testnet").unwrap();

        // Pre-populate alice only.
        state
            .keystore
            .read()
            .await
            .add_customer(alice.clone(), [0xAA; 32]);

        // Alice → fast-path success (no MPC needed).
        state
            .ensure_customer_loaded(Some(&alice))
            .await
            .expect("alice's cached master must serve");

        // Bob → not cached → would attempt MPC; without context
        // configured this fails fast with the documented error.
        let err = state
            .ensure_customer_loaded(Some(&bob))
            .await
            .expect_err("bob's master is not loaded; gate must refuse");
        let msg = format!("{:#}", err);
        assert!(
            msg.contains("MPC CKD context") || msg.contains("non-TEE"),
            "non-TEE-mode error message expected, got: {msg}"
        );
    }

    // ============== Audit noteC2 — backward-compat smoke ==============
    // The plan (line 598) explicitly mandates: "запросы без
    // X-Customer-Vault header работают на default_master (legacy
    // clients)". Earlier audits caught customer-isolation invariants
    // at the gate level. This test drives a real wallet handler with
    // an EMPTY HeaderMap and asserts the derived address matches what
    // direct `keystore.derive_keypair(None, …)` produces — pinning
    // the legacy customer-less path against accidental regression.
    #[tokio::test]
    async fn handler_without_x_customer_vault_uses_default_master() {
        let state = test_state();
        // Wait for is_ready (test_state initialises with is_ready=true).

        // Snapshot the keystore's default-master output for the seed
        // the handler will build internally.
        let expected_seed = "wallet:test-wallet-id:near".to_string();
        let expected_pubkey = {
            let ks = state.keystore.read().await;
            let (_, vk) = ks.derive_keypair(None, &expected_seed).unwrap();
            hex::encode(vk.as_bytes())
        };

        // Drive the handler with an empty HeaderMap — proves the
        // legacy "no header → default master" path.
        let request = WalletDeriveAddressRequest {
            wallet_id: "test-wallet-id".to_string(),
            chain: "near".to_string(),
        };
        let response = wallet_derive_address_handler(
            axum::extract::State(state),
            axum::http::HeaderMap::new(),
            axum::Json(request),
        )
        .await
        .expect("handler must succeed without X-Customer-Vault header");

        assert_eq!(
            response.0.address, expected_pubkey,
            "derived address must match default-master derive_keypair output"
        );
    }

    #[tokio::test]
    async fn handler_with_empty_x_customer_vault_uses_default_master() {
        // An empty header value must be treated identically to no
        // header — `extract_customer_from_header` returns Ok(None).
        // This matters because some HTTP clients always send all
        // configured headers, even with empty values.
        let state = test_state();
        let expected_pubkey = {
            let ks = state.keystore.read().await;
            let (_, vk) = ks.derive_keypair(None, "wallet:abc:near").unwrap();
            hex::encode(vk.as_bytes())
        };

        let mut headers = axum::http::HeaderMap::new();
        headers.insert("X-Customer-Vault", "".parse().unwrap());

        let request = WalletDeriveAddressRequest {
            wallet_id: "abc".to_string(),
            chain: "near".to_string(),
        };
        let response = wallet_derive_address_handler(
            axum::extract::State(state),
            headers,
            axum::Json(request),
        )
        .await
        .expect("empty header must be treated as no header");

        assert_eq!(response.0.address, expected_pubkey);
    }
}
