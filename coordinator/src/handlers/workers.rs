use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

use crate::AppState;
use crate::auth::WorkerTokenHash;

/// Worker heartbeat request
#[derive(Debug, Deserialize)]
pub struct HeartbeatRequest {
    pub worker_id: String,
    pub worker_name: String,
    pub status: WorkerStatusEnum, // 'online', 'busy', 'offline'
    pub current_task_id: Option<i64>,
    /// Event monitor's current block height (None if event monitor not running)
    #[serde(default)]
    pub event_monitor_block_height: Option<i64>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkerStatusEnum {
    Online,
    Busy,
    Offline,
}

impl std::fmt::Display for WorkerStatusEnum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkerStatusEnum::Online => write!(f, "online"),
            WorkerStatusEnum::Busy => write!(f, "busy"),
            WorkerStatusEnum::Offline => write!(f, "offline"),
        }
    }
}

/// Worker heartbeat endpoint
///
/// Workers should call this every 30-60 seconds to report their status
pub async fn heartbeat(
    State(state): State<AppState>,
    Json(payload): Json<HeartbeatRequest>,
) -> StatusCode {
    debug!(
        "Worker heartbeat: {} ({}) - status: {:?}",
        payload.worker_id, payload.worker_name, payload.status
    );

    let status_str = payload.status.to_string();

    let result = sqlx::query(
        r#"
        INSERT INTO worker_status (worker_id, worker_name, status, current_task_id, event_monitor_block_height, event_monitor_updated_at, last_heartbeat_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, CASE WHEN $5 IS NOT NULL THEN NOW() ELSE NULL END, NOW(), NOW())
        ON CONFLICT (worker_id)
        DO UPDATE SET
            worker_name = EXCLUDED.worker_name,
            status = EXCLUDED.status,
            current_task_id = EXCLUDED.current_task_id,
            event_monitor_block_height = COALESCE(EXCLUDED.event_monitor_block_height, worker_status.event_monitor_block_height),
            event_monitor_updated_at = CASE
                WHEN EXCLUDED.event_monitor_block_height IS NOT NULL THEN NOW()
                ELSE worker_status.event_monitor_updated_at
            END,
            last_heartbeat_at = NOW(),
            updated_at = NOW()
        "#,
    )
    .bind(&payload.worker_id)
    .bind(&payload.worker_name)
    .bind(&status_str)
    .bind(payload.current_task_id)
    .bind(payload.event_monitor_block_height)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            debug!("Worker {} heartbeat recorded", payload.worker_id);
            StatusCode::OK
        }
        Err(e) => {
            error!("Failed to record worker heartbeat: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

/// Task completion notification
///
/// Called by worker after completing a task to update stats
#[derive(Debug, Deserialize)]
pub struct TaskCompletionNotification {
    pub worker_id: String,
    pub success: bool,
}

pub async fn notify_task_completion(
    State(state): State<AppState>,
    Json(payload): Json<TaskCompletionNotification>,
) -> StatusCode {
    debug!(
        "Task completion notification from worker {}: success={}",
        payload.worker_id, payload.success
    );

    let result = if payload.success {
        sqlx::query!(
            r#"
            UPDATE worker_status
            SET
                total_tasks_completed = total_tasks_completed + 1,
                last_task_completed_at = NOW(),
                status = 'online',
                current_task_id = NULL,
                updated_at = NOW()
            WHERE worker_id = $1
            "#,
            payload.worker_id
        )
        .execute(&state.db)
        .await
    } else {
        sqlx::query!(
            r#"
            UPDATE worker_status
            SET
                total_tasks_failed = total_tasks_failed + 1,
                status = 'online',
                current_task_id = NULL,
                updated_at = NOW()
            WHERE worker_id = $1
            "#,
            payload.worker_id
        )
        .execute(&state.db)
        .await
    };

    match result {
        Ok(_) => {
            debug!("Worker {} stats updated", payload.worker_id);
            StatusCode::OK
        }
        Err(e) => {
            error!("Failed to update worker stats: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

/// Delete a worker record from worker_status by worker_id.
/// Admin-only endpoint for cleaning up stale/zombie workers.
pub async fn delete_worker(
    State(state): State<AppState>,
    Path(worker_id): Path<String>,
) -> StatusCode {
    let result = sqlx::query(
        "DELETE FROM worker_status WHERE worker_id = $1",
    )
    .bind(&worker_id)
    .execute(&state.db)
    .await;

    match result {
        Ok(r) => {
            if r.rows_affected() == 0 {
                warn!("Admin: worker {} not found for deletion", worker_id);
                StatusCode::NOT_FOUND
            } else {
                info!("Admin: deleted worker {}", worker_id);
                StatusCode::OK
            }
        }
        Err(e) => {
            error!("Admin: failed to delete worker {}: {}", worker_id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

// ========== TEE Session Management ==========

#[derive(Debug, Serialize)]
pub struct TeeChallenge {
    pub challenge: String,
}

/// Generate a one-time challenge for TEE session registration.
///
/// The worker must sign this challenge with its TEE private key
/// and submit it to `POST /workers/register-tee`.
pub async fn tee_challenge(
    State(state): State<AppState>,
    axum::Extension(token_hash): axum::Extension<WorkerTokenHash>,
) -> Result<Json<TeeChallenge>, StatusCode> {
    let challenge = tee_auth::generate_challenge();

    // Store challenge in DB (expires in 60 seconds, cleaned up periodically)
    sqlx::query(
        "INSERT INTO tee_challenges (challenge, token_hash) VALUES ($1, $2)"
    )
    .bind(&challenge)
    .bind(&token_hash.0)
    .execute(&state.db)
    .await
    .map_err(|e| {
        error!("Failed to store TEE challenge: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    debug!("TEE challenge generated for token {}", &token_hash.0[..8]);
    Ok(Json(TeeChallenge { challenge }))
}

#[derive(Debug, Deserialize)]
pub struct RegisterTeeRequest {
    pub public_key: String,
    pub challenge: String,
    pub signature: String,
}

#[derive(Debug, Serialize)]
pub struct RegisterTeeResponse {
    pub session_id: uuid::Uuid,
}

/// Register a TEE session via challenge-response.
///
/// 1. Validates the challenge belongs to this worker and is fresh (<60s)
/// 2. Verifies the ed25519 signature over the challenge
/// 3. Checks the public key exists on the register-contract (NEAR RPC)
/// 4. Creates a TEE session in the database
pub async fn register_tee(
    State(state): State<AppState>,
    axum::Extension(token_hash): axum::Extension<WorkerTokenHash>,
    Json(payload): Json<RegisterTeeRequest>,
) -> Result<Json<RegisterTeeResponse>, (StatusCode, String)> {
    // 1. Find and validate challenge
    let challenge_row = sqlx::query_as::<_, (String, chrono::DateTime<chrono::Utc>)>(
        "DELETE FROM tee_challenges WHERE challenge = $1 AND token_hash = $2 RETURNING challenge, created_at"
    )
    .bind(&payload.challenge)
    .bind(&token_hash.0)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        error!("DB error looking up TEE challenge: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, "Database error".to_string())
    })?;

    let (_, created_at) = challenge_row.ok_or_else(|| {
        warn!("TEE challenge not found or wrong token");
        (StatusCode::BAD_REQUEST, "Invalid or expired challenge".to_string())
    })?;

    // Check challenge is not older than 60 seconds
    let age = chrono::Utc::now() - created_at;
    if age.num_seconds() > 60 {
        warn!("TEE challenge expired (age: {}s)", age.num_seconds());
        return Err((StatusCode::BAD_REQUEST, "Challenge expired".to_string()));
    }

    // 2. Verify ed25519 signature
    tee_auth::verify_signature(&payload.public_key, &payload.challenge, &payload.signature)
        .map_err(|e| {
            warn!("TEE signature verification failed: {}", e);
            (StatusCode::FORBIDDEN, format!("Signature verification failed: {}", e))
        })?;

    // 3. Check key exists on operator account (where register-contract adds keys)
    let operator_account_id = state.config.operator_account_id.as_ref().ok_or_else(|| {
        error!("OPERATOR_ACCOUNT_ID not configured on coordinator");
        (StatusCode::INTERNAL_SERVER_ERROR, "TEE verification not configured".to_string())
    })?;

    let key_exists = crate::near_client::check_access_key_exists(
        &state.config.near_rpc_url,
        operator_account_id,
        &payload.public_key,
    )
    .await
    .map_err(|e| {
        error!("NEAR RPC check failed: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Key verification failed: {}", e))
    })?;

    if !key_exists {
        warn!("Public key {} not found on operator account {}", payload.public_key, operator_account_id);
        return Err((StatusCode::FORBIDDEN, "Public key not registered on contract".to_string()));
    }

    // 4. Deactivate old sessions for this token
    let _ = sqlx::query(
        "UPDATE worker_tee_sessions SET is_active = FALSE WHERE token_hash = $1 AND is_active = TRUE"
    )
    .bind(&token_hash.0)
    .execute(&state.db)
    .await;

    // 5. Create new session
    let session_id = sqlx::query_scalar::<_, uuid::Uuid>(
        "INSERT INTO worker_tee_sessions (token_hash, worker_public_key) VALUES ($1, $2) RETURNING session_id"
    )
    .bind(&token_hash.0)
    .bind(&payload.public_key)
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        error!("Failed to create TEE session: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, "Failed to create session".to_string())
    })?;

    info!(
        "TEE session created: session={}, key={}...{}",
        session_id,
        &payload.public_key[..12.min(payload.public_key.len())],
        &payload.public_key[payload.public_key.len().saturating_sub(4)..]
    );

    Ok(Json(RegisterTeeResponse { session_id }))
}
