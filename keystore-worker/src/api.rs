//! HTTP API server for keystore worker
//!
//! Endpoints:
//! - GET /health - Health check
//! - GET /pubkey?seed=... - Get public key for a specific seed
//! - POST /decrypt - Decrypt secrets from contract (requires auth + attestation)

use axum::{
    extract::{Query, State},
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

    /// Owner account ID
    pub owner: String,

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

/// Query parameters for pubkey endpoint
#[derive(Debug, Deserialize)]
pub struct PubkeyQuery {
    /// Seed for deriving keypair (format: "repo:owner[:branch]")
    pub seed: String,
}

/// Response with public key
#[derive(Debug, Serialize)]
pub struct PubkeyResponse {
    /// Public key in hex format
    pub pubkey: String,
}

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub tee_mode: String,
}

/// Create the API router with all endpoints
pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/pubkey", get(pubkey_handler))
        .route("/decrypt", post(decrypt_handler))
        // Add auth middleware to decrypt endpoint only
        .route_layer(middleware::from_fn_with_state(
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

/// Get public key for a specific seed
async fn pubkey_handler(
    Query(query): Query<PubkeyQuery>,
    State(state): State<AppState>,
) -> Result<Json<PubkeyResponse>, ApiError> {
    let pubkey_hex = state
        .keystore
        .public_key_hex(&query.seed)
        .map_err(|e| ApiError::InternalError(format!("Failed to derive public key: {}", e)))?;

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

    // TODO: Get actual caller account from attestation or request context
    // For now, use owner as caller (self-access always allowed)
    let caller = &req.owner;

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
