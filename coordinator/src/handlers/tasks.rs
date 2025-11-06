use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use redis::AsyncCommands;
use serde::Deserialize;
use tracing::{debug, error};

use crate::models::{CreateTaskRequest, CreateTaskResponse, ExecutionRequest};
use crate::AppState;

#[derive(Deserialize)]
pub struct PollTaskQuery {
    #[serde(default = "default_timeout")]
    timeout: u64,
}

fn default_timeout() -> u64 {
    60
}

/// Long-poll for next execution request
/// Workers call this endpoint to receive work from the Redis queue
pub async fn poll_task(
    State(state): State<AppState>,
    Query(params): Query<PollTaskQuery>,
) -> Result<Json<ExecutionRequest>, StatusCode> {
    let timeout = params.timeout.min(120); // Max 2 minutes

    debug!("Polling for task with timeout {}s", timeout);

    // Get dedicated Redis connection for BRPOP (blocking operation)
    let client = state.redis.clone();
    let mut conn = client
        .get_async_connection()
        .await
        .map_err(|e| {
            error!("Failed to get Redis connection: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // BRPOP with timeout
    let result: Option<(String, String)> = conn
        .brpop(&state.config.redis_task_queue, timeout as f64)
        .await
        .map_err(|e| {
            error!("Redis BRPOP error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    match result {
        Some((_key, json)) => {
            debug!("ExecutionRequest received: {}", json);
            let request: ExecutionRequest = serde_json::from_str(&json).map_err(|e| {
                error!("Failed to deserialize execution request: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
            Ok(Json(request))
        }
        None => {
            debug!("Poll timeout - no execution requests available");
            Err(StatusCode::NO_CONTENT)
        }
    }
}

/// Create new task (called by event monitor)
/// This endpoint only pushes to Redis queue. Workers should use /jobs/claim to actually claim work.
pub async fn create_task(
    State(state): State<AppState>,
    Json(payload): Json<CreateTaskRequest>,
) -> Result<(StatusCode, Json<CreateTaskResponse>), StatusCode> {
    debug!("Creating task for request_id={} data_id={}", payload.request_id, payload.data_id);

    let request_id = payload.request_id;

    // Check if task already exists in database (to prevent duplicates)
    let existing_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM jobs WHERE request_id = $1 AND data_id = $2 AND job_type = 'execute'"
    )
    .bind(request_id as i64)
    .bind(&payload.data_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        error!("Failed to check existing task: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if existing_count.0 > 0 {
        debug!("Task already exists for request_id={} data_id={}, skipping", request_id, payload.data_id);
        return Ok((StatusCode::OK, Json(CreateTaskResponse {
            request_id: request_id as i64,
            created: false, // Already exists
        })));
    }

    // Normalize repo URL to full https:// format for git clone
    let code_source = payload.code_source.normalize();

    // Create execution request and push to Redis queue
    let execution_request = ExecutionRequest {
        request_id,
        data_id: payload.data_id.clone(),
        code_source,
        resource_limits: payload.resource_limits,
        input_data: payload.input_data,
        secrets_ref: payload.secrets_ref, // Reference to contract-stored secrets
        response_format: payload.response_format,
        context: payload.context,
        user_account_id: payload.user_account_id,
        near_payment_yocto: payload.near_payment_yocto,
        transaction_hash: payload.transaction_hash,
    };

    let request_json = serde_json::to_string(&execution_request).map_err(|e| {
        error!("Failed to serialize execution request: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let mut conn = state.redis.get_multiplexed_async_connection().await.map_err(|e| {
        error!("Failed to get Redis connection: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    conn.lpush::<_, _, ()>(&state.config.redis_task_queue, request_json)
        .await
        .map_err(|e| {
            error!("Failed to push execution request to Redis: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    debug!("ExecutionRequest {} pushed to queue", request_id);
    Ok((StatusCode::CREATED, Json(CreateTaskResponse {
        request_id: request_id as i64,
        created: true,
    })))
}
