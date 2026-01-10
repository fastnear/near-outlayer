//! System Callbacks handlers (TopUp, Delete, etc.)
//!
//! Handles contract business logic that requires yield/resume mechanism:
//! - TopUp: Update Payment Key balance
//! - Delete: Delete Payment Key
//! - (Future: Withdraw, UpdateLimits, etc.)
//!
//! All tasks are stored in a single Redis queue `system_callbacks_queue` with task type tag.
//! Workers poll this queue and dispatch based on task type.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

use crate::AppState;

/// Custom deserializer for comma-separated capabilities
fn deserialize_capabilities<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: Option<String> = serde::Deserialize::deserialize(deserializer)?;
    match s {
        Some(s) if !s.is_empty() => Ok(s.split(',').map(|s| s.trim().to_string()).collect()),
        _ => Ok(Vec::new()),
    }
}

/// Queue name for all system callback tasks
const SYSTEM_CALLBACKS_QUEUE: &str = "system_callbacks_queue";

// =============================================================================
// Unified System Callback Task Type
// =============================================================================

/// System callback task types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "task_type", rename_all = "snake_case")]
pub enum SystemCallbackTask {
    /// TopUp Payment Key - requires keystore to decrypt/re-encrypt
    TopUp(TopUpTaskPayload),
    /// Delete Payment Key - no keystore needed
    DeletePaymentKey(DeletePaymentKeyTaskPayload),
    // Future: Withdraw, UpdateLimits, etc.
}

/// TopUp task payload (stored in unified queue)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopUpTaskPayload {
    pub task_id: i64,
    pub data_id: String,
    pub owner: String,
    pub nonce: u32,
    pub amount: String,
    pub encrypted_data: String,
    pub created_at: i64,
}

/// DeletePaymentKey task payload (stored in unified queue)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletePaymentKeyTaskPayload {
    pub task_id: i64,
    pub data_id: String,
    pub owner: String,
    pub nonce: u32,
    pub created_at: i64,
}

// =============================================================================
// TopUp handlers
// =============================================================================

/// Request to create a TopUp task
#[derive(Debug, Deserialize)]
pub struct CreateTopUpRequest {
    /// data_id for yield/resume (hex encoded)
    pub data_id: String,
    /// Payment Key owner
    pub owner: String,
    /// Payment Key nonce (profile)
    pub nonce: u32,
    /// TopUp amount in minimal token units
    pub amount: String,
    /// Current encrypted secret (base64)
    pub encrypted_data: String,
}

/// Response for create TopUp task
#[derive(Debug, Serialize)]
pub struct CreateTopUpResponse {
    pub task_id: i64,
    pub created: bool,
}

/// Create a new TopUp task
///
/// This endpoint is called by the event_monitor when it detects
/// a SystemEvent::TopUpPaymentKey event from the contract.
///
/// The task is stored in Redis queue for workers to process.
pub async fn create_topup_task(
    State(state): State<AppState>,
    Json(req): Json<CreateTopUpRequest>,
) -> Result<Json<CreateTopUpResponse>, StatusCode> {
    info!(
        "Creating TopUp task: data_id={} owner={} nonce={} amount={}",
        req.data_id, req.owner, req.nonce, req.amount
    );

    // Check for duplicate by data_id
    let exists_key = format!("topup:data_id:{}", req.data_id);
    let mut conn = state.redis.get_async_connection().await.map_err(|e| {
        error!("Failed to get Redis connection: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Check if task already exists
    let exists: bool = conn.exists(&exists_key).await.map_err(|e| {
        error!("Failed to check task existence: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if exists {
        // Return existing task_id
        let task_id: i64 = conn.get(&exists_key).await.unwrap_or(0);
        info!(
            "TopUp task already exists: data_id={} task_id={}",
            req.data_id, task_id
        );
        return Ok(Json(CreateTopUpResponse {
            task_id,
            created: false,
        }));
    }

    // Generate new task_id
    let task_id: i64 = conn.incr("topup:task_id_counter", 1).await.map_err(|e| {
        error!("Failed to generate task_id: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Create unified task payload
    let payload = TopUpTaskPayload {
        task_id,
        data_id: req.data_id.clone(),
        owner: req.owner,
        nonce: req.nonce,
        amount: req.amount,
        encrypted_data: req.encrypted_data,
        created_at: chrono::Utc::now().timestamp(),
    };

    // Wrap in unified task type
    let task = SystemCallbackTask::TopUp(payload);

    // Serialize task
    let task_json = serde_json::to_string(&task).map_err(|e| {
        error!("Failed to serialize task: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Store task in unified Redis queue
    let _: () = conn.lpush(SYSTEM_CALLBACKS_QUEUE, &task_json).await.map_err(|e| {
        error!("Failed to push task to queue: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Mark data_id as processed (with TTL of 1 hour to handle timeouts)
    let _: () = conn
        .set_ex(&exists_key, task_id, 3600)
        .await
        .map_err(|e| {
            error!("Failed to mark task as existing: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    info!(
        "TopUp task created: task_id={} data_id={}",
        task_id, req.data_id
    );

    Ok(Json(CreateTopUpResponse {
        task_id,
        created: true,
    }))
}

/// Request to complete a TopUp (called by worker after resume_topup)
#[derive(Debug, Deserialize)]
pub struct CompleteTopUpRequest {
    /// Payment Key owner
    pub owner: String,
    /// Payment Key nonce
    pub nonce: u32,
    /// New initial_balance after top-up (full amount, not delta)
    pub new_initial_balance: String,
    /// SHA256 hash of the actual key (hex encoded) - used for validation without decryption
    pub key_hash: String,
    /// Project IDs this key can access (empty = all projects)
    #[serde(default)]
    pub project_ids: Vec<String>,
    /// Max amount per API call (optional)
    #[serde(default)]
    pub max_per_call: Option<String>,
}

/// Complete a TopUp - store payment key metadata in PostgreSQL
///
/// Called by worker after successfully calling resume_topup on the contract.
/// Worker decrypts the payment key secret, extracts metadata, and sends it here.
/// This is the ONLY source of truth for payment key validation (no contract reads).
///
/// NOTE: This endpoint requires worker auth (in authenticated routes).
pub async fn complete_topup(
    State(state): State<AppState>,
    Json(req): Json<CompleteTopUpRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!(
        "TopUp complete: owner={} nonce={} new_balance={} key_hash={}...",
        req.owner, req.nonce, req.new_initial_balance,
        &req.key_hash[..8.min(req.key_hash.len())]
    );

    // Insert or update payment_keys table (PostgreSQL - source of truth)
    let result = sqlx::query(
        r#"
        INSERT INTO payment_keys (owner, nonce, key_hash, initial_balance, project_ids, max_per_call, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW())
        ON CONFLICT (owner, nonce) DO UPDATE
        SET key_hash = EXCLUDED.key_hash,
            initial_balance = EXCLUDED.initial_balance,
            project_ids = EXCLUDED.project_ids,
            max_per_call = EXCLUDED.max_per_call,
            updated_at = NOW(),
            deleted_at = NULL
        "#
    )
    .bind(&req.owner)
    .bind(req.nonce as i32)
    .bind(&req.key_hash)
    .bind(&req.new_initial_balance)
    .bind(&req.project_ids)
    .bind(&req.max_per_call)
    .execute(&state.db)
    .await;

    if let Err(e) = result {
        error!("Failed to update payment_keys: {}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    info!(
        "Payment key updated in PostgreSQL: owner={} nonce={}",
        req.owner, req.nonce
    );

    Ok(Json(serde_json::json!({ "success": true })))
}

/// Request to delete a Payment Key (soft delete)
#[derive(Debug, Deserialize)]
pub struct DeletePaymentKeyRequest {
    /// Payment Key owner
    pub owner: String,
    /// Payment Key nonce
    pub nonce: u32,
}

/// Delete a Payment Key - soft delete in PostgreSQL
///
/// Called by worker after processing DeletePaymentKey event from contract.
/// Sets deleted_at timestamp so key is no longer valid for HTTPS API calls.
///
/// NOTE: This endpoint requires worker auth (in authenticated routes).
pub async fn delete_payment_key(
    State(state): State<AppState>,
    Json(req): Json<DeletePaymentKeyRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!(
        "Delete payment key: owner={} nonce={}",
        req.owner, req.nonce
    );

    // Soft delete - set deleted_at timestamp
    let result = sqlx::query(
        r#"
        UPDATE payment_keys
        SET deleted_at = NOW(), updated_at = NOW()
        WHERE owner = $1 AND nonce = $2 AND deleted_at IS NULL
        "#
    )
    .bind(&req.owner)
    .bind(req.nonce as i32)
    .execute(&state.db)
    .await;

    match result {
        Ok(result) => {
            if result.rows_affected() > 0 {
                info!(
                    "Payment key deleted: owner={} nonce={}",
                    req.owner, req.nonce
                );
            } else {
                info!(
                    "Payment key not found or already deleted: owner={} nonce={}",
                    req.owner, req.nonce
                );
            }
            Ok(Json(serde_json::json!({
                "success": true,
                "deleted": result.rows_affected() > 0
            })))
        }
        Err(e) => {
            error!("Failed to delete payment key: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// =============================================================================
// DeletePaymentKey Task Queue (for yield/resume flow)
// =============================================================================

/// Request to create a DeletePaymentKey task
#[derive(Debug, Deserialize)]
pub struct CreateDeletePaymentKeyTaskRequest {
    /// data_id for yield/resume (hex encoded)
    pub data_id: String,
    /// Payment Key owner
    pub owner: String,
    /// Payment Key nonce
    pub nonce: u32,
}

/// Response for create delete task
#[derive(Debug, Serialize)]
pub struct CreateDeletePaymentKeyTaskResponse {
    pub task_id: i64,
    pub created: bool,
}

/// Create a new DeletePaymentKey task
///
/// Called by event_monitor when it detects a SystemEvent::DeletePaymentKey event.
/// Task is stored in Redis queue for workers to process.
pub async fn create_delete_payment_key_task(
    State(state): State<AppState>,
    Json(req): Json<CreateDeletePaymentKeyTaskRequest>,
) -> Result<Json<CreateDeletePaymentKeyTaskResponse>, StatusCode> {
    info!(
        "Creating DeletePaymentKey task: data_id={} owner={} nonce={}",
        req.data_id, req.owner, req.nonce
    );

    // Check for duplicate by data_id
    let exists_key = format!("delete_payment_key:data_id:{}", req.data_id);
    let mut conn = state.redis.get_async_connection().await.map_err(|e| {
        error!("Failed to get Redis connection: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Check if task already exists
    let exists: bool = conn.exists(&exists_key).await.map_err(|e| {
        error!("Failed to check task existence: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if exists {
        let task_id: i64 = conn.get(&exists_key).await.unwrap_or(0);
        info!(
            "DeletePaymentKey task already exists: data_id={} task_id={}",
            req.data_id, task_id
        );
        return Ok(Json(CreateDeletePaymentKeyTaskResponse {
            task_id,
            created: false,
        }));
    }

    // Generate task_id
    let task_id: i64 = conn.incr("delete_payment_key_task_id_counter", 1).await.map_err(|e| {
        error!("Failed to generate task_id: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Create unified task payload
    let payload = DeletePaymentKeyTaskPayload {
        task_id,
        data_id: req.data_id.clone(),
        owner: req.owner,
        nonce: req.nonce,
        created_at: chrono::Utc::now().timestamp(),
    };

    // Wrap in unified task type
    let task = SystemCallbackTask::DeletePaymentKey(payload);

    // Serialize task
    let task_json = serde_json::to_string(&task).map_err(|e| {
        error!("Failed to serialize task: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Store task in unified Redis queue
    let _: () = conn.lpush(SYSTEM_CALLBACKS_QUEUE, &task_json).await.map_err(|e| {
        error!("Failed to push task to queue: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Mark data_id as processed (with TTL of 1 hour)
    let _: () = conn
        .set_ex(&exists_key, task_id, 3600)
        .await
        .map_err(|e| {
            error!("Failed to mark task as existing: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    info!(
        "DeletePaymentKey task created: task_id={} data_id={}",
        task_id, req.data_id
    );

    Ok(Json(CreateDeletePaymentKeyTaskResponse {
        task_id,
        created: true,
    }))
}

// =============================================================================
// Unified System Callbacks Poll Endpoint
// =============================================================================

fn default_poll_timeout() -> u64 {
    60 // Same as execution queue
}

/// Query parameters for unified system callbacks poll
#[derive(Debug, Deserialize)]
pub struct PollSystemCallbacksQuery {
    /// Timeout in seconds (default: 60, max: 120)
    #[serde(default = "default_poll_timeout")]
    timeout: u64,
    /// Worker capabilities: "compilation,execution" or "compilation" or "execution"
    /// System callbacks require "execution" capability
    #[serde(default, deserialize_with = "deserialize_capabilities")]
    capabilities: Vec<String>,
}

/// Poll for ANY system callback task (TopUp, Delete, etc.) from unified queue
///
/// Workers call this single endpoint to receive all system callback tasks.
/// This replaces separate poll_topup_task and poll_delete_payment_key_task endpoints.
///
/// Query parameters:
/// - timeout: seconds to wait (default 30, use 0 for non-blocking)
///
/// Returns:
/// - JSON with task_type field: "top_up" or "delete_payment_key"
/// - Worker dispatches to appropriate handler based on task_type
pub async fn poll_system_callback_task(
    State(state): State<AppState>,
    Query(params): Query<PollSystemCallbacksQuery>,
) -> Result<Json<Option<SystemCallbackTask>>, StatusCode> {
    // System callbacks require "execution" capability
    // Compile-only workers should not process TopUp/Delete tasks
    let has_execution = params.capabilities.contains(&"execution".to_string());

    if !has_execution && !params.capabilities.is_empty() {
        // Worker has capabilities specified but no "execution" - reject
        warn!(
            "Worker with capabilities {:?} tried to poll system callbacks (requires 'execution')",
            params.capabilities
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // If capabilities is empty, allow (backwards compatibility) but log warning
    if params.capabilities.is_empty() {
        debug!("Worker polling system callbacks without capabilities specified");
    }

    let mut conn = state.redis.get_async_connection().await.map_err(|e| {
        error!("Failed to get Redis connection: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let timeout = params.timeout.min(120); // Max 2 minutes

    let task_json: Option<String> = if timeout == 0 {
        // Non-blocking RPOP
        conn.rpop(SYSTEM_CALLBACKS_QUEUE, None)
            .await
            .map_err(|e| {
                error!("Redis RPOP error: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
    } else {
        // Blocking BRPOP with timeout
        let result: Option<(String, String)> = conn.brpop(SYSTEM_CALLBACKS_QUEUE, timeout as f64)
            .await
            .map_err(|e| {
                error!("Redis BRPOP error: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        result.map(|(_, json)| json)
    };

    match task_json {
        Some(task_json) => {
            let task: SystemCallbackTask = serde_json::from_str(&task_json).map_err(|e| {
                error!("Failed to parse system callback task: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

            // Log which task type was received
            match &task {
                SystemCallbackTask::TopUp(payload) => {
                    info!(
                        "📥 System callback task polled: type=TopUp task_id={} owner={} nonce={}",
                        payload.task_id, payload.owner, payload.nonce
                    );
                }
                SystemCallbackTask::DeletePaymentKey(payload) => {
                    info!(
                        "📥 System callback task polled: type=DeletePaymentKey task_id={} owner={} nonce={}",
                        payload.task_id, payload.owner, payload.nonce
                    );
                }
            }

            Ok(Json(Some(task)))
        }
        None => {
            debug!("No system callback tasks available");
            Ok(Json(None))
        }
    }
}
