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

/// Application state shared across all handlers
#[derive(Clone)]
pub struct AppState {
    pub keystore: std::sync::Arc<tokio::sync::RwLock<crate::crypto::Keystore>>,
    pub config: crate::config::Config,
    pub expected_measurements: crate::attestation::ExpectedMeasurements,
    pub near_client: Option<std::sync::Arc<crate::near::NearClient>>,
    pub is_ready: std::sync::Arc<std::sync::atomic::AtomicBool>,
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

/// Create the API router with all endpoints
///
/// Route access levels:
/// - Public (no auth): /health, /pubkey
/// - Worker-only (ALLOWED_WORKER_TOKEN_HASHES): /decrypt, /encrypt, /decrypt-raw, /storage/*
/// - Coordinator-only (ALLOWED_COORDINATOR_TOKEN_HASHES): /add_generated_secret, /update_user_secrets
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
        .layer(middleware::from_fn_with_state(
            state.clone(),
            coordinator_auth_middleware,
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

/// Get public key for encryption AND validate secrets before encryption
async fn pubkey_handler(
    State(state): State<AppState>,
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
    let keystore = state.keystore.read().await;
    let pubkey_hex = keystore
        .public_key_hex(&req.seed)
        .map_err(|e| ApiError::InternalError(format!("Failed to derive public key: {}", e)))?;

    tracing::info!(
        seed = %req.seed,
        num_secrets = secrets_map.len(),
        "Validated secrets and generated pubkey"
    );

    Ok(Json(PubkeyResponse { pubkey: pubkey_hex }))
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

    // 2. Read secrets from NEAR contract
    let near_client = state.near_client.as_ref()
        .ok_or_else(|| ApiError::InternalError("NEAR client not configured".to_string()))?;

    let secret_profile = match &req.accessor {
        SecretAccessor::Repo { repo, branch } => {
            near_client
                .get_secrets(repo, branch.as_deref(), &req.profile, &req.owner)
                .await
                .map_err(|e| {
                    tracing::error!(task_id = %task_id_str, error = %e, "Failed to read secrets from contract");
                    ApiError::InternalError(format!("Failed to read secrets from contract: {}", e))
                })?
                .ok_or_else(|| {
                    tracing::warn!(
                        task_id = %task_id_str,
                        repo = %repo,
                        profile = %req.profile,
                        owner = %req.owner,
                        "Secrets not found in contract"
                    );
                    ApiError::BadRequest("Secrets not found in contract".to_string())
                })?
        }
        SecretAccessor::WasmHash { hash } => {
            near_client
                .get_secrets_by_wasm_hash(hash, &req.profile, &req.owner)
                .await
                .map_err(|e| {
                    tracing::error!(task_id = %task_id_str, error = %e, "Failed to read secrets from contract");
                    ApiError::InternalError(format!("Failed to read secrets from contract: {}", e))
                })?
                .ok_or_else(|| {
                    tracing::warn!(
                        task_id = %task_id_str,
                        wasm_hash = %hash,
                        profile = %req.profile,
                        owner = %req.owner,
                        "Secrets not found in contract"
                    );
                    ApiError::BadRequest("Secrets not found in contract".to_string())
                })?
        }
        SecretAccessor::Project { project_id } => {
            near_client
                .get_secrets_by_project(project_id, &req.profile, &req.owner)
                .await
                .map_err(|e| {
                    tracing::error!(task_id = %task_id_str, error = %e, "Failed to read secrets from contract");
                    ApiError::InternalError(format!("Failed to read secrets from contract: {}", e))
                })?
                .ok_or_else(|| {
                    tracing::warn!(
                        task_id = %task_id_str,
                        project_id = %project_id,
                        profile = %req.profile,
                        owner = %req.owner,
                        "Secrets not found in contract"
                    );
                    ApiError::BadRequest("Secrets not found in contract".to_string())
                })?
        }
        SecretAccessor::System { secret_type } => {
            // Convert SystemSecretType to contract format string
            let secret_type_str = match secret_type {
                SystemSecretType::PaymentKey => "PaymentKey",
            };
            near_client
                .get_secrets_by_system(secret_type_str, &req.profile, &req.owner)
                .await
                .map_err(|e| {
                    tracing::error!(task_id = %task_id_str, error = %e, "Failed to read secrets from contract");
                    ApiError::InternalError(format!("Failed to read secrets from contract: {}", e))
                })?
                .ok_or_else(|| {
                    tracing::warn!(
                        task_id = %task_id_str,
                        secret_type = ?secret_type,
                        profile = %req.profile,
                        owner = %req.owner,
                        "Secrets not found in contract"
                    );
                    ApiError::BadRequest("Secrets not found in contract".to_string())
                })?
        }
    };

    tracing::debug!(task_id = %task_id_str, "Successfully read secrets from contract");

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
            let secret_branch = secret_profile["branch"].as_str();

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
            let type_str = match secret_type {
                SystemSecretType::PaymentKey => "payment_key",
            };
            let seed = format!("system:{}:{}:{}", type_str, req.owner, req.profile);

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
    let plaintext_bytes = keystore.decrypt(&seed, &encrypted_bytes).map_err(|e| {
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
        .encrypt(&req.seed, &plaintext_bytes)
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
        .decrypt(&req.seed, &encrypted_bytes)
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

    tracing::info!(
        seed = %req.seed,
        num_new_secrets = req.new_secrets.len(),
        has_existing = req.encrypted_secrets_base64.is_some(),
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
            .decrypt(&req.seed, &encrypted_bytes)
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
        .encrypt(&req.seed, final_secrets_json.as_bytes())
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

    tracing::info!(
        owner = %req.owner,
        profile = %req.profile,
        mode = ?req.mode,
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

    // 6. Get current encrypted secrets from contract using OLD accessor
    // (new_accessor is only for encryption target, not for fetching)
    let near_client = state.near_client.as_ref()
        .ok_or_else(|| ApiError::InternalError("NEAR client not configured".to_string()))?;

    let secret_profile = match &req.accessor {
        SecretAccessor::Repo { repo, branch } => {
            near_client
                .get_secrets(repo, branch.as_deref(), &req.profile, &req.owner)
                .await
                .map_err(|e| {
                    tracing::warn!(error = %e, "Failed to fetch secrets from contract");
                    ApiError::InternalError(format!("Failed to fetch secrets: {}", e))
                })?
        }
        SecretAccessor::WasmHash { hash } => {
            near_client
                .get_secrets_by_wasm_hash(hash, &req.profile, &req.owner)
                .await
                .map_err(|e| {
                    tracing::warn!(error = %e, "Failed to fetch secrets from contract");
                    ApiError::InternalError(format!("Failed to fetch secrets: {}", e))
                })?
        }
        SecretAccessor::Project { project_id } => {
            near_client
                .get_secrets_by_project(project_id, &req.profile, &req.owner)
                .await
                .map_err(|e| {
                    tracing::warn!(error = %e, "Failed to fetch secrets from contract");
                    ApiError::InternalError(format!("Failed to fetch secrets: {}", e))
                })?
        }
        SecretAccessor::System { secret_type } => {
            let secret_type_str = match secret_type {
                SystemSecretType::PaymentKey => "PaymentKey",
            };
            near_client
                .get_secrets_by_system(secret_type_str, &req.profile, &req.owner)
                .await
                .map_err(|e| {
                    tracing::warn!(error = %e, "Failed to fetch secrets from contract");
                    ApiError::InternalError(format!("Failed to fetch secrets: {}", e))
                })?
        }
    };

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
                let type_str = match secret_type {
                    SystemSecretType::PaymentKey => "payment_key",
                };
                format!("system:{}:{}:{}", type_str, req.owner, req.profile)
            }
        };

        tracing::debug!(
            seed = %seed,
            encrypted_len = encrypted_bytes.len(),
            "Attempting to decrypt existing secrets"
        );

        // Decrypt
        let keystore = state.keystore.read().await;
        let plaintext_bytes = keystore
            .decrypt(&seed, &encrypted_bytes)
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
            let type_str = match secret_type {
                SystemSecretType::PaymentKey => "payment_key",
            };
            format!("system:{}:{}:{}", type_str, req.owner, req.profile)
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
        .encrypt(&encryption_seed, final_secrets_json.as_bytes())
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
        .encrypt(&seed, req.key.as_bytes())
        .map_err(|e| ApiError::InternalError(format!("Failed to encrypt key: {}", e)))?;

    let encrypted_value = keystore
        .encrypt(&seed, &value_bytes)
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
        .decrypt(&seed, &encrypted_key)
        .map_err(|e| ApiError::InternalError(format!("Failed to decrypt key: {}", e)))?;

    let value_bytes = keystore
        .decrypt(&seed, &encrypted_value)
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
    let challenge = tee_auth::generate_challenge();

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
    tee_auth::verify_signature(&req.public_key, &req.challenge, &req.signature)
        .map_err(|e| ApiError::BadRequest(format!("Signature verification failed: {}", e)))?;

    // 3. Check key on operator account via NEAR RPC (with retry for finality lag)
    let operator_account_id = state.config.operator_account_id.as_ref().ok_or_else(|| {
        ApiError::InternalError("OPERATOR_ACCOUNT_ID not configured on keystore".to_string())
    })?;

    let key_exists = tee_auth::check_access_key_with_retry(
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
}
