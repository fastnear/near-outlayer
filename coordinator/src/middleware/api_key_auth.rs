use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use sqlx::PgPool;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use crate::config::Config;

/// Middleware to verify API key and store key ID in request extensions
pub async fn api_key_auth(
    State((pool, config)): State<(PgPool, Arc<Config>)>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Skip API key check if disabled in config (dev mode)
    if !config.require_attestation_api_key {
        tracing::debug!("API key check disabled (REQUIRE_ATTESTATION_API_KEY=false)");
        return Ok(next.run(request).await);
    }

    let api_key = request
        .headers()
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Hash the API key for lookup
    let api_key_hash = format!("{:x}", Sha256::digest(api_key.as_bytes()));

    // Verify API key exists and is active
    let key_record = sqlx::query!(
        "SELECT id, is_active, rate_limit_per_minute
         FROM api_keys
         WHERE api_key = $1",
        api_key_hash
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        tracing::error!("Database error during API key auth: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::UNAUTHORIZED)?;

    if !key_record.is_active.unwrap_or(false) {
        tracing::warn!("Inactive API key attempted access: id={}", key_record.id);
        return Err(StatusCode::FORBIDDEN);
    }

    // Update last_used_at (fire-and-forget)
    let pool_clone = pool.clone();
    let key_id = key_record.id;
    tokio::spawn(async move {
        let _ = sqlx::query!(
            "UPDATE api_keys SET last_used_at = NOW() WHERE id = $1",
            key_id
        )
        .execute(&pool_clone)
        .await;
    });

    // Store API key ID and rate limit in request extensions for rate limiting
    request.extensions_mut().insert(key_record.id);
    request.extensions_mut().insert(key_record.rate_limit_per_minute);

    Ok(next.run(request).await)
}
