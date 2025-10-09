use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use redis::AsyncCommands;
use serde::Deserialize;
use tracing::{debug, error, warn};

use crate::models::{CompleteTaskRequest, CreateTaskRequest, FailTaskRequest, Task};
use crate::AppState;

#[derive(Deserialize)]
pub struct PollTaskQuery {
    #[serde(default = "default_timeout")]
    timeout: u64,
}

fn default_timeout() -> u64 {
    60
}

/// Long-poll for next task
pub async fn poll_task(
    State(state): State<AppState>,
    Query(params): Query<PollTaskQuery>,
) -> Result<Json<Task>, StatusCode> {
    let timeout = params.timeout.min(120); // Max 2 minutes

    debug!("Polling for task with timeout {}s", timeout);

    // Get dedicated Redis connection for BRPOP (blocking operation)
    // IMPORTANT: BRPOP blocks the connection, so we need a dedicated connection
    // instead of multiplexed connection
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
            debug!("Task received: {}", json);
            let task: Task = serde_json::from_str(&json).map_err(|e| {
                error!("Failed to deserialize task: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
            Ok(Json(task))
        }
        None => {
            debug!("Poll timeout - no tasks available");
            Err(StatusCode::NO_CONTENT)
        }
    }
}

/// Complete task with result
pub async fn complete_task(
    State(state): State<AppState>,
    Json(payload): Json<CompleteTaskRequest>,
) -> StatusCode {
    debug!("Completing task {} (success: {})", payload.request_id, payload.success);

    // Update status in database
    let result = sqlx::query!(
        "UPDATE execution_requests SET status = 'completed', updated_at = NOW() WHERE request_id = $1",
        payload.request_id as i64
    )
    .execute(&state.db)
    .await;

    if let Err(e) = result {
        error!("Failed to update task {}: {}", payload.request_id, e);
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    // If this was a successful Compile task, create Execute task
    if payload.success && payload.output.is_some() {
        // Output contains the WASM checksum from compilation
        let checksum = String::from_utf8_lossy(&payload.output.unwrap()).to_string();

        debug!("Compilation succeeded, creating Execute task for request {}", payload.request_id);

        //TODO: Fetch code_source, resource_limits, and data_id from database
        // For now, we need these from the original request
        // This is a limitation - we should store these in DB when task is created

        warn!("TODO: Create Execute task after Compile - need to store task metadata in DB");
    }

    debug!("Task {} marked as completed", payload.request_id);
    StatusCode::OK
}

/// Mark task as failed
pub async fn fail_task(
    State(state): State<AppState>,
    Json(payload): Json<FailTaskRequest>,
) -> StatusCode {
    debug!("Failing task {}: {}", payload.request_id, payload.error);

    let result = sqlx::query!(
        "UPDATE execution_requests SET status = 'failed', updated_at = NOW() WHERE request_id = $1",
        payload.request_id as i64
    )
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            debug!("Task {} marked as failed", payload.request_id);
            StatusCode::OK
        }
        Err(e) => {
            error!("Failed to update task {}: {}", payload.request_id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

/// Create new task (called by event monitor)
pub async fn create_task(
    State(state): State<AppState>,
    Json(payload): Json<CreateTaskRequest>,
) -> StatusCode {
    debug!("Creating task for request {}", payload.request_id);

    // Insert into database
    let insert_result = sqlx::query!(
        r#"
        INSERT INTO execution_requests (request_id, data_id, status, created_at, updated_at)
        VALUES ($1, $2, 'pending', NOW(), NOW())
        ON CONFLICT (request_id) DO NOTHING
        "#,
        payload.request_id as i64,
        payload.data_id
    )
    .execute(&state.db)
    .await;

    if let Err(e) = insert_result {
        error!("Failed to insert request: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    // Push to Redis queue - now includes data_id, resource_limits and input_data
    let task = Task::Compile {
        request_id: payload.request_id,
        data_id: payload.data_id.clone(),
        code_source: payload.code_source,
        resource_limits: payload.resource_limits,
        input_data: payload.input_data,
    };

    let task_json = match serde_json::to_string(&task) {
        Ok(json) => json,
        Err(e) => {
            error!("Failed to serialize task: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    let mut conn = match state.redis.get_multiplexed_async_connection().await {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to get Redis connection: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    let push_result: Result<(), redis::RedisError> =
        conn.lpush(&state.config.redis_task_queue, task_json).await;

    match push_result {
        Ok(_) => {
            debug!("Task {} pushed to queue", payload.request_id);
            StatusCode::CREATED
        }
        Err(e) => {
            error!("Failed to push task to Redis: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}
