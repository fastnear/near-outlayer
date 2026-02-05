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
    let token_hash_for_update = token_hash.clone();
    tokio::spawn(async move {
        let _ = sqlx::query!(
            "UPDATE worker_auth_tokens SET last_used_at = NOW() WHERE token_hash = $1",
            token_hash_for_update
        )
        .execute(&db)
        .await;
    });

    debug!("Auth successful");

    // Extract and validate TEE session only when the feature is enabled.
    // This avoids a DB query on every request when TEE sessions aren't required.
    let tee_session = if state.config.require_tee_session {
        let tee_session_id = req
            .headers()
            .get("X-TEE-Session")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| uuid::Uuid::parse_str(s).ok());

        if let Some(session_id) = tee_session_id {
            let row = sqlx::query_scalar::<_, bool>(
                "SELECT is_active FROM worker_tee_sessions WHERE session_id = $1"
            )
            .bind(session_id)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();

            match row {
                Some(true) => Some(session_id),
                _ => {
                    debug!("Invalid or inactive TEE session: {}", session_id);
                    None
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    // Store token_hash and TEE session info in request extensions
    let mut req = req;
    req.extensions_mut().insert(WorkerTokenHash(token_hash));
    req.extensions_mut().insert(TeeSessionInfo(tee_session));

    Ok(next.run(req).await)
}

/// Worker token hash stored in request extensions
#[derive(Clone)]
pub struct WorkerTokenHash(pub String);

/// TEE session info stored in request extensions (None if not provided or invalid)
#[derive(Clone)]
pub struct TeeSessionInfo(pub Option<uuid::Uuid>);
