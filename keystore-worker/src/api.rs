//! HTTP API server for keystore worker
//!
//! Provides endpoints for executor workers to decrypt secrets.
//! All endpoints are async and non-blocking for high concurrency.

use crate::attestation::{self, Attestation, ExpectedMeasurements};
use crate::config::Config;
use crate::crypto::Keystore;
use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::trace::TraceLayer;

/// Shared application state (thread-safe, cloneable)
#[derive(Clone)]
pub struct AppState {
    /// The keystore (holds private key in memory)
    keystore: Arc<Keystore>,
    /// Configuration
    config: Arc<Config>,
    /// Expected code measurements for worker verification
    expected_measurements: Arc<ExpectedMeasurements>,
}

impl AppState {
    pub fn new(keystore: Keystore, config: Config) -> Self {
        Self {
            keystore: Arc::new(keystore),
            config: Arc::new(config),
            expected_measurements: Arc::new(ExpectedMeasurements::default()),
        }
    }
}

/// Request to decrypt secrets
#[derive(Debug, Deserialize)]
pub struct DecryptRequest {
    /// Encrypted secrets (base64 encoded)
    pub encrypted_secrets: String,

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

/// Error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub public_key: String,
    pub tee_mode: String,
}

/// Create the API router with all endpoints
pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/decrypt", post(decrypt_handler))
        .route("/pubkey", get(pubkey_handler))
        // Add auth middleware to decrypt endpoint only
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
        public_key: state.keystore.public_key_hex(),
        tee_mode: format!("{:?}", state.config.tee_mode),
    })
}

/// Get public key endpoint (no auth required)
async fn pubkey_handler(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "public_key_hex": state.keystore.public_key_hex(),
        "public_key_base58": state.keystore.public_key_base58(),
    }))
}

/// Decrypt secrets for authorized TEE worker
///
/// This is the core endpoint - must be fast and non-blocking.
/// Multiple workers can call this simultaneously.
async fn decrypt_handler(
    State(state): State<AppState>,
    Json(req): Json<DecryptRequest>,
) -> Result<Json<DecryptResponse>, ApiError> {
    let task_id = req.task_id.as_deref().unwrap_or("unknown");

    tracing::info!(
        task_id = %task_id,
        tee_type = %req.attestation.tee_type,
        "Received decrypt request"
    );

    // 1. Verify TEE attestation (this is the security-critical step)
    attestation::verify_attestation(
        &req.attestation,
        &state.config.tee_mode,
        &state.expected_measurements,
    )
    .map_err(|e| {
        tracing::warn!(
            task_id = %task_id,
            error = %e,
            "Attestation verification failed"
        );
        ApiError::Unauthorized(format!("Attestation verification failed: {}", e))
    })?;

    // 2. Decode encrypted secrets from base64
    let encrypted_bytes = base64::decode(&req.encrypted_secrets)
        .map_err(|e| ApiError::BadRequest(format!("Invalid base64: {}", e)))?;

    // 3. Decrypt using keystore private key (this is fast, O(n) XOR operation)
    let plaintext_bytes = state.keystore.decrypt(&encrypted_bytes).map_err(|e| {
        tracing::error!(
            task_id = %task_id,
            error = %e,
            "Decryption failed"
        );
        ApiError::InternalError(format!("Decryption failed: {}", e))
    })?;

    // 4. Encode plaintext as base64 for safe JSON transport
    let plaintext_b64 = base64::encode(&plaintext_bytes);

    tracing::info!(
        task_id = %task_id,
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
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    // Skip auth for health and pubkey endpoints
    let path = request.uri().path();
    if path == "/health" || path == "/pubkey" {
        return Ok(next.run(request).await);
    }

    // Extract Authorization header
    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ApiError::Unauthorized("Missing Authorization header".to_string()))?;

    // Parse Bearer token
    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| ApiError::Unauthorized("Invalid Authorization header format".to_string()))?;

    // Hash token with SHA256
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let token_hash = hex::encode(hasher.finalize());

    // Check if hash is in allowed list
    if !state.config.allowed_worker_token_hashes.contains(&token_hash) {
        tracing::warn!(
            token_hash = %token_hash,
            "Unauthorized access attempt with invalid token"
        );
        return Err(ApiError::Unauthorized("Invalid token".to_string()));
    }

    Ok(next.run(request).await)
}

/// API errors with proper HTTP status codes
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

        let body = Json(ErrorResponse { error: message });

        (status, body).into_response()
    }
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
