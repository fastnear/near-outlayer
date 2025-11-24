use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};

use crate::AppState;
#[allow(unused_imports)]
use crate::models::{StoreSystemLogRequest, SystemHiddenLog};

/// POST /internal/system-logs
///
/// ⚠️ SECURITY WARNING ⚠️
/// Stores RAW stderr/stdout which may contain system file contents from malicious code
/// This data is stored in system_hidden_logs table - NEVER expose via /public/* API
///
/// Store raw system logs (compilation/execution) for admin debugging
/// NO AUTH REQUIRED - internal endpoint for workers only
pub async fn store_system_log(
    State(state): State<AppState>,
    Json(payload): Json<StoreSystemLogRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    // Insert system log into database
    let result = sqlx::query!(
        r#"
        INSERT INTO system_hidden_logs (request_id, job_id, log_type, stderr, stdout, exit_code, execution_error)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id
        "#,
        payload.request_id,
        payload.job_id,
        payload.log_type,
        payload.stderr,
        payload.stdout,
        payload.exit_code,
        payload.execution_error
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to store system log: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    tracing::debug!(
        "Stored system log ({}) for request_id={} (log_id={})",
        payload.log_type,
        payload.request_id,
        result.id
    );

    Ok((StatusCode::CREATED, Json(serde_json::json!({"id": result.id}))))
}

/// GET /admin/system-logs/:request_id
///
/// ⚠️ ADMIN ONLY - NEVER EXPOSE VIA PUBLIC API ⚠️
/// Returns RAW stderr/stdout which may contain leaked system files
/// Access this endpoint ONLY via localhost/SSH, NOT through public URL
///
/// Retrieve raw system logs for a specific request (admin debugging)
/// NO AUTH REQUIRED in dev mode - in production, add proper admin authentication
/// This endpoint should NOT be accessible from external networks
pub async fn get_system_logs(
    State(state): State<AppState>,
    Path(request_id): Path<i64>,
) -> Result<impl IntoResponse, StatusCode> {
    // Fetch all system logs for this request_id
    let logs = sqlx::query_as!(
        SystemHiddenLog,
        r#"
        SELECT
            id,
            request_id,
            job_id,
            log_type,
            stderr,
            stdout,
            exit_code,
            execution_error,
            created_at::TEXT as "created_at!"
        FROM system_hidden_logs
        WHERE request_id = $1
        ORDER BY created_at DESC
        "#,
        request_id
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch system logs: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if logs.is_empty() {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(Json(logs))
}
