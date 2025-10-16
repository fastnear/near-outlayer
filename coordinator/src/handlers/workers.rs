use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, error};

use crate::AppState;

/// Worker heartbeat request
#[derive(Debug, Deserialize)]
pub struct HeartbeatRequest {
    pub worker_id: String,
    pub worker_name: String,
    pub status: WorkerStatusEnum, // 'online', 'busy', 'offline'
    pub current_task_id: Option<i64>,
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

    let result = sqlx::query!(
        r#"
        INSERT INTO worker_status (worker_id, worker_name, status, current_task_id, last_heartbeat_at, updated_at)
        VALUES ($1, $2, $3, $4, NOW(), NOW())
        ON CONFLICT (worker_id)
        DO UPDATE SET
            worker_name = EXCLUDED.worker_name,
            status = EXCLUDED.status,
            current_task_id = EXCLUDED.current_task_id,
            last_heartbeat_at = NOW(),
            updated_at = NOW()
        "#,
        payload.worker_id,
        payload.worker_name,
        status_str,
        payload.current_task_id
    )
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
