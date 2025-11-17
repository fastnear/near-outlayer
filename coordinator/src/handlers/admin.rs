use axum::{extract::State, http::StatusCode, Json};
use rand::{thread_rng, Rng};
use sha2::{Digest, Sha256};

use crate::models::{CreateApiKeyRequest, CreateApiKeyResponse};
use crate::AppState;

/// Generate random API key (32 bytes = 64 hex chars)
fn generate_api_key() -> String {
    let mut rng = thread_rng();
    let bytes: [u8; 32] = rng.gen();
    hex::encode(bytes)
}

/// Admin endpoint: Create new API key
///
/// Requires admin bearer token in Authorization header
pub async fn create_api_key(
    State(state): State<AppState>,
    Json(req): Json<CreateApiKeyRequest>,
) -> Result<Json<CreateApiKeyResponse>, StatusCode> {
    // Validate request
    req.validate()
        .map_err(|e| {
            tracing::warn!("Invalid API key request: {}", e);
            StatusCode::BAD_REQUEST
        })?;

    // Determine rate limit
    let rate_limit = req
        .rate_limit_per_minute
        .unwrap_or(state.config.default_rate_limit as i32)
        .min(state.config.max_rate_limit as i32);

    // Generate API key
    let api_key_plaintext = generate_api_key();
    let api_key_hash = format!("{:x}", Sha256::digest(api_key_plaintext.as_bytes()));

    // Store in database
    let result = sqlx::query!(
        "INSERT INTO api_keys (api_key, near_account_id, key_name, rate_limit_per_minute)
         VALUES ($1, $2, $3, $4)
         RETURNING created_at",
        api_key_hash,
        req.near_account_id,
        req.key_name,
        rate_limit
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create API key: {}", e);
        if e.to_string().contains("duplicate") {
            StatusCode::CONFLICT
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    })?;

    tracing::info!(
        "Created API key for NEAR account: {} (rate_limit: {}/min)",
        req.near_account_id,
        rate_limit
    );

    Ok(Json(CreateApiKeyResponse {
        api_key: api_key_plaintext,
        near_account_id: req.near_account_id,
        rate_limit_per_minute: rate_limit,
        created_at: result.created_at.map(|dt| dt.and_utc().timestamp()).unwrap_or(0),
    }))
}
