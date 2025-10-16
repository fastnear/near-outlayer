use axum::{
    body::Body,
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use sha2::{Digest, Sha256};
use tracing::{debug, warn};

use crate::AppState;

pub async fn auth_middleware(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let path = req.uri().path();

    // Skip auth for health check and all public endpoints
    if path == "/health" || path.starts_with("/public/") {
        return Ok(next.run(req).await);
    }

    // Skip auth if not required (dev mode)
    if !state.config.require_auth {
        debug!("Auth disabled (dev mode)");
        return Ok(next.run(req).await);
    }

    // Get Authorization header
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| {
            warn!("Missing Authorization header");
            StatusCode::UNAUTHORIZED
        })?;

    if !auth_header.starts_with("Bearer ") {
        warn!("Invalid Authorization header format");
        return Err(StatusCode::UNAUTHORIZED);
    }

    let token = &auth_header[7..];

    // Hash token
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let token_hash = format!("{:x}", hasher.finalize());

    // Check in database
    let record = sqlx::query!(
        "SELECT is_active FROM worker_auth_tokens WHERE token_hash = $1",
        token_hash
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        warn!("Database error during auth: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let is_valid = match record {
        Some(r) => r.is_active,
        None => false,
    };

    if !is_valid {
        warn!("Invalid or inactive token");
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Update last_used_at (fire and forget)
    let db = state.db.clone();
    tokio::spawn(async move {
        let _ = sqlx::query!(
            "UPDATE worker_auth_tokens SET last_used_at = NOW() WHERE token_hash = $1",
            token_hash
        )
        .execute(&db)
        .await;
    });

    debug!("Auth successful");
    Ok(next.run(req).await)
}
