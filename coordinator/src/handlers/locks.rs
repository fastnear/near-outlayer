use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use redis::AsyncCommands;
use tracing::{debug, error};

use crate::models::{AcquireLockRequest, AcquireLockResponse};
use crate::AppState;

/// Acquire distributed lock
pub async fn acquire_lock(
    State(state): State<AppState>,
    Json(payload): Json<AcquireLockRequest>,
) -> Result<Json<AcquireLockResponse>, StatusCode> {
    debug!(
        "Acquiring lock: {} by worker {}",
        payload.lock_key, payload.worker_id
    );

    let mut conn = state
        .redis
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| {
            error!("Failed to get Redis connection: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Try to set lock with NX (only if not exists) and EX (expiration)
    let lock_key = format!("lock:{}", payload.lock_key);
    let acquired: bool = redis::cmd("SET")
        .arg(&lock_key)
        .arg(&payload.worker_id)
        .arg("NX") // Only set if not exists
        .arg("EX") // Set expiration
        .arg(payload.ttl_seconds)
        .query_async(&mut conn)
        .await
        .map(|v: Option<String>| v.is_some())
        .unwrap_or(false);

    debug!("Lock {} acquired: {}", payload.lock_key, acquired);

    Ok(Json(AcquireLockResponse { acquired }))
}

/// Release distributed lock
pub async fn release_lock(
    State(state): State<AppState>,
    Path(lock_key): Path<String>,
) -> StatusCode {
    debug!("Releasing lock: {}", lock_key);

    let mut conn = match state.redis.get_multiplexed_async_connection().await {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to get Redis connection: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    let full_lock_key = format!("lock:{}", lock_key);
    let result: Result<(), redis::RedisError> = conn.del(&full_lock_key).await;

    match result {
        Ok(_) => {
            debug!("Lock {} released", lock_key);
            StatusCode::OK
        }
        Err(e) => {
            error!("Failed to release lock: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}
