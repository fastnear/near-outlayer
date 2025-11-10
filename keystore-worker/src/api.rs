//! HTTP API server for keystore worker
//!
//! Endpoints:
//! - GET /health - Health check
//! - GET /pubkey?seed=... - Get public key for a specific seed
//! - POST /decrypt - Decrypt secrets from contract (requires auth + attestation)

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

/// Application state shared across all handlers
#[derive(Clone)]
pub struct AppState {
    pub keystore: crate::crypto::Keystore,
    pub config: crate::config::Config,
    pub expected_measurements: crate::attestation::ExpectedMeasurements,
    pub near_client: Option<std::sync::Arc<crate::near::NearClient>>,
}

impl AppState {
    pub fn new(
        keystore: crate::crypto::Keystore,
        config: crate::config::Config,
        near_client: Option<crate::near::NearClient>,
    ) -> Self {
        Self {
            keystore,
            config: config.clone(),
            expected_measurements: crate::attestation::ExpectedMeasurements::default(),
            near_client: near_client.map(std::sync::Arc::new),
        }
    }
}

/// API error types
#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    Unauthorized(String),
    InternalError(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg),
            ApiError::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        (status, Json(serde_json::json!({"error": message}))).into_response()
    }
}

/// Request to decrypt secrets from contract
#[derive(Debug, Deserialize)]
pub struct DecryptRequest {
    /// Repository URL (will be normalized)
    pub repo: String,

    /// Optional branch name
    pub branch: Option<String>,

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

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub tee_mode: String,
}

/// Create the API router with all endpoints
/// All endpoints require auth (keystore is internal service, accessed only by coordinator)
pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/pubkey", post(pubkey_handler)) // Changed to POST to accept secrets for validation
        .route("/decrypt", post(decrypt_handler))
        .route("/add_generated_secret", post(add_generated_secret_handler)) // NEW: Add generated secrets
        // Auth middleware applies to all routes (keystore is internal)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Health check endpoint (no auth required)
async fn health_handler(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        tee_mode: format!("{:?}", state.config.tee_mode),
    })
}

/// Get public key for encryption AND validate secrets before encryption
async fn pubkey_handler(
    State(state): State<AppState>,
    Json(req): Json<PubkeyRequest>,
) -> Result<Json<PubkeyResponse>, ApiError> {
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
    let pubkey_hex = state
        .keystore
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
    let task_id_str = req.task_id.as_deref().unwrap_or("unknown");

    tracing::info!(
        task_id = %task_id_str,
        tee_type = %req.attestation.tee_type,
        repo = %req.repo,
        profile = %req.profile,
        owner = %req.owner,
        "Received decrypt request"
    );

    // 1. Verify TEE attestation (security-critical step)
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

    let secret_profile = near_client
        .get_secrets(&req.repo, req.branch.as_deref(), &req.profile, &req.owner)
        .await
        .map_err(|e| {
            tracing::error!(task_id = %task_id_str, error = %e, "Failed to read secrets from contract");
            ApiError::InternalError(format!("Failed to read secrets from contract: {}", e))
        })?
        .ok_or_else(|| {
            tracing::warn!(
                task_id = %task_id_str,
                repo = %req.repo,
                profile = %req.profile,
                owner = %req.owner,
                "Secrets not found in contract"
            );
            ApiError::BadRequest("Secrets not found in contract".to_string())
        })?;

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

    // 4. Normalize repo URL and build seed: repo:owner[:branch]
    // SECURITY NOTE:
    // - We use branch from SECRET PROFILE (not request) to construct seed
    // - This is correct because seed must match the one used during encryption
    // - Access control already validated above (only owner can decrypt their secrets)
    // - Contract already returned the correct secrets based on request parameters
    let normalized_repo = crate::utils::normalize_repo_url(&req.repo);

    let secret_branch = secret_profile["branch"].as_str();
    let request_branch = req.branch.as_deref();

    // Log branch matching for debugging
    match (request_branch, secret_branch) {
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
        // branch is null in contract - secrets encrypted without branch in seed
        format!("{}:{}", normalized_repo, req.owner)
    };

    tracing::info!(
        task_id = %task_id_str,
        repo_normalized = %normalized_repo,
        owner = %req.owner,
        secret_branch = ?secret_branch,
        seed = %seed,
        "🔓 DECRYPTION SEED (keystore)"
    );

    // 5. Decrypt using derived keypair
    let encrypted_secrets_base64 = secret_profile["encrypted_secrets"]
        .as_str()
        .ok_or_else(|| ApiError::InternalError("Missing encrypted_secrets field".to_string()))?;

    let encrypted_bytes = base64::decode(encrypted_secrets_base64)
        .map_err(|e| ApiError::InternalError(format!("Invalid base64 in encrypted_secrets: {}", e)))?;

    let plaintext_bytes = state.keystore.decrypt(&seed, &encrypted_bytes).map_err(|e| {
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
        let plaintext_bytes = state
            .keystore
            .decrypt(&req.seed, &encrypted_bytes)
            .map_err(|e| ApiError::InternalError(format!("Failed to decrypt existing secrets: {}", e)))?;

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

    let encrypted_bytes = state
        .keystore
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

/// Authentication middleware
///
/// Checks Bearer token in Authorization header.
/// Token is hashed with SHA256 and compared against allowed hashes.
async fn auth_middleware(
    State(state): State<AppState>,
    req: axum::http::Request<axum::body::Body>,
    next: middleware::Next,
) -> Result<Response, ApiError> {
    // Get Authorization header
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| ApiError::Unauthorized("Missing Authorization header".to_string()))?;

    // Extract Bearer token
    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| ApiError::Unauthorized("Invalid Authorization format".to_string()))?;

    // Hash token with SHA256
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let token_hash = hex::encode(hasher.finalize());

    // Check if hash is in allowed list
    if !state.config.allowed_worker_token_hashes.contains(&token_hash) {
        tracing::warn!(token_hash = %token_hash, "Unauthorized access attempt");
        return Err(ApiError::Unauthorized("Invalid token".to_string()));
    }

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
            "repo": "github.com/user/repo",
            "branch": "main",
            "profile": "production",
            "owner": "owner.testnet",
            "user_account_id": "caller.testnet",
            "attestation": {
                "tee_type": "simulated",
                "quote": "",
                "measurements": {},
                "timestamp": 1704067200
            },
            "task_id": "task123"
        }"#;

        let req: DecryptRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.repo, "github.com/user/repo");
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
}
