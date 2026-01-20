//! HTTPS API Call handlers
//!
//! Provides REST API for executing WASM without NEAR transactions.
//! Uses Payment Keys for authentication and billing.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::Row;
use sqlx::types::BigDecimal;
use std::str::FromStr;
use tracing::{debug, info};
use uuid::Uuid;

use crate::models::ResourceLimits;
use crate::AppState;

// ============================================================================
// Request/Response types
// ============================================================================

/// HTTPS API call request body
#[derive(Debug, Deserialize)]
pub struct HttpsCallRequest {
    /// Input data for WASM execution
    pub input: serde_json::Value,
    /// Resource limits (optional, defaults from project settings)
    #[serde(default)]
    pub resource_limits: Option<ResourceLimitsInput>,
    /// Async mode: false = wait for result (default), true = return call_id immediately
    #[serde(default)]
    pub r#async: bool,
}

/// Resource limits from request (all optional)
#[derive(Debug, Deserialize)]
pub struct ResourceLimitsInput {
    pub max_instructions: Option<u64>,
    pub max_memory_mb: Option<u32>,
    pub max_execution_seconds: Option<u64>,
}

/// HTTPS API call response
#[derive(Debug, Serialize)]
pub struct HttpsCallResponse {
    pub call_id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Compute cost in minimal token units (e.g., 1920 = $0.001920 for 6-decimal token)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compute_cost: Option<String>,
    /// Number of WASM instructions executed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<u64>,
    /// Execution time in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attestation_url: Option<String>,
}

/// Parsed Payment Key header
#[derive(Debug, Clone)]
pub struct PaymentKeyHeader {
    pub owner: String,
    pub nonce: u32,
    pub key: String,
}

/// Cached payment key metadata (from keystore decrypt)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentKeyMetadata {
    pub initial_balance: String,
    pub project_ids: Vec<String>,
    pub max_per_call: Option<String>,
    pub key_hash: String, // SHA256 hash of the key for validation
}


// ============================================================================
// Error types
// ============================================================================

#[derive(Debug)]
pub enum CallError {
    MissingPaymentKey,
    InvalidPaymentKeyFormat(String),
    InvalidKey,
    ProjectNotAllowed,
    InsufficientBalance { available: u128, required: u128 },
    MaxPerCallExceeded { max: u128, requested: u128 },
    ComputeLimitTooLow { min: u128, provided: u128 },
    RateLimitExceeded,
    TooManyConcurrentCalls,
    ProjectNotFound,
    KeystoreError(String),
    InternalError(String),
}

impl axum::response::IntoResponse for CallError {
    fn into_response(self) -> axum::response::Response {
        let (status, error_msg) = match self {
            CallError::MissingPaymentKey => {
                (StatusCode::UNAUTHORIZED, "Missing X-Payment-Key header".to_string())
            }
            CallError::InvalidPaymentKeyFormat(msg) => {
                (StatusCode::BAD_REQUEST, format!("Invalid X-Payment-Key format: {}", msg))
            }
            CallError::InvalidKey => {
                (StatusCode::UNAUTHORIZED, "Invalid payment key".to_string())
            }
            CallError::ProjectNotAllowed => {
                (StatusCode::FORBIDDEN, "Project not allowed for this payment key".to_string())
            }
            CallError::InsufficientBalance { available, required } => {
                (StatusCode::PAYMENT_REQUIRED, format!(
                    "Insufficient balance: available={}, required={}", available, required
                ))
            }
            CallError::MaxPerCallExceeded { max, requested } => {
                (StatusCode::BAD_REQUEST, format!(
                    "Max per call exceeded: max={}, requested={}", max, requested
                ))
            }
            CallError::ComputeLimitTooLow { min, provided } => {
                (StatusCode::BAD_REQUEST, format!(
                    "Compute limit too low: min={}, provided={}", min, provided
                ))
            }
            CallError::RateLimitExceeded => {
                (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded".to_string())
            }
            CallError::TooManyConcurrentCalls => {
                (StatusCode::TOO_MANY_REQUESTS, "Too many concurrent calls".to_string())
            }
            CallError::ProjectNotFound => {
                (StatusCode::NOT_FOUND, "Project not found".to_string())
            }
            CallError::KeystoreError(msg) => {
                (StatusCode::INTERNAL_SERVER_ERROR, format!("Keystore error: {}", msg))
            }
            CallError::InternalError(msg) => {
                (StatusCode::INTERNAL_SERVER_ERROR, msg)
            }
        };

        let body = Json(serde_json::json!({
            "error": error_msg,
        }));

        (status, body).into_response()
    }
}

// ============================================================================
// Handlers
// ============================================================================

/// POST /{project_owner}/{project_name} - Execute WASM via HTTPS API
///
/// Headers:
/// - X-Payment-Key: {owner}:{nonce}:{key}
/// - X-Compute-Limit: budget in minimal token units (optional)
/// - X-Attached-Deposit: payment to project owner (optional)
pub async fn https_call(
    State(state): State<AppState>,
    Path((project_owner, project_name)): Path<(String, String)>,
    headers: HeaderMap,
    Json(body): Json<HttpsCallRequest>,
) -> Result<Json<HttpsCallResponse>, CallError> {
    let project_id = format!("{}/{}", project_owner, project_name);
    info!("📥 HTTPS call: project={}", project_id);

    // 1. Parse X-Payment-Key header
    let payment_key = parse_payment_key_header(&headers)?;
    debug!("Payment key: owner={} nonce={}", payment_key.owner, payment_key.nonce);

    // 2. Parse X-Compute-Limit and X-Attached-Deposit
    let compute_limit = parse_header_u128(&headers, "X-Compute-Limit")
        .unwrap_or(state.config.default_compute_limit);
    let attached_deposit = parse_header_u128(&headers, "X-Attached-Deposit")
        .unwrap_or(0);

    // 3. Validate compute limit
    if compute_limit < state.config.min_compute_limit {
        return Err(CallError::ComputeLimitTooLow {
            min: state.config.min_compute_limit,
            provided: compute_limit,
        });
    }

    // 4. Check rate limit for payment key
    check_payment_key_rate_limit(&state, &payment_key).await?;

    // 5. Validate payment key and get metadata
    let metadata = validate_payment_key(&state, &payment_key).await?;

    // 6. Check project is allowed
    if !metadata.project_ids.is_empty() && !metadata.project_ids.contains(&project_id) {
        return Err(CallError::ProjectNotAllowed);
    }

    // 7. Check max_per_call (0 or empty = unlimited)
    let total_amount = compute_limit + attached_deposit;
    if let Some(max_str) = &metadata.max_per_call {
        // Treat "0" or "" as unlimited
        if !max_str.is_empty() && max_str != "0" {
            let max: u128 = max_str.parse().unwrap_or(u128::MAX);
            if total_amount > max {
                return Err(CallError::MaxPerCallExceeded {
                    max,
                    requested: total_amount,
                });
            }
        }
    }

    // 8. Check and reserve balance
    let initial_balance: u128 = metadata.initial_balance.parse().unwrap_or(0);
    let (spent, reserved) = fetch_payment_key_balance(&state, &payment_key.owner, payment_key.nonce).await?;
    let available = initial_balance.saturating_sub(spent).saturating_sub(reserved);

    if available < total_amount {
        return Err(CallError::InsufficientBalance {
            available,
            required: total_amount,
        });
    }

    // 9. Reserve balance
    reserve_balance(&state, &payment_key.owner, payment_key.nonce, total_amount).await?;

    // 10. Generate call_id
    let call_id = Uuid::new_v4();

    // 11. Create resource limits
    let resource_limits = ResourceLimits {
        max_instructions: body.resource_limits.as_ref()
            .and_then(|r| r.max_instructions)
            .unwrap_or(10_000_000_000), // 10B default
        max_memory_mb: body.resource_limits.as_ref()
            .and_then(|r| r.max_memory_mb)
            .unwrap_or(128),
        max_execution_seconds: body.resource_limits.as_ref()
            .and_then(|r| r.max_execution_seconds)
            .unwrap_or(60),
    };

    // 13. Store call in database
    let input_json = serde_json::to_string(&body.input).unwrap_or_default();
    create_https_call(
        &state,
        call_id,
        &payment_key.owner,
        payment_key.nonce,
        &project_id,
        &input_json,
        attached_deposit,
    ).await?;

    // 13. Create execution task (worker will resolve project_id -> code_source)
    let task_created = create_execution_task(
        &state,
        call_id,
        &project_id,
        &input_json,
        resource_limits.clone(),
        &payment_key.owner,
        payment_key.nonce,
        compute_limit,
        attached_deposit,
    ).await?;

    if !task_created {
        // Release reservation on failure
        release_balance(&state, &payment_key.owner, payment_key.nonce, total_amount).await?;
        return Err(CallError::InternalError("Failed to create execution task".to_string()));
    }

    // 15. Return response based on mode
    if body.r#async {
        // Async mode - return call_id immediately
        Ok(Json(HttpsCallResponse {
            call_id: call_id.to_string(),
            status: "pending".to_string(),
            output: None,
            error: None,
            compute_cost: None,
            instructions: None,
            time_ms: None,
            poll_url: Some(format!("/calls/{}", call_id)),
            attestation_url: None,
        }))
    } else {
        // Sync mode - wait for result
        let result = wait_for_result(&state, call_id, state.config.https_call_timeout_seconds).await;

        // Handle timeout only - successful calls are finalized by complete_https_call
        if result.is_err() {
            // Timeout - release reservation and mark as failed
            release_balance(&state, &payment_key.owner, payment_key.nonce, total_amount).await?;
            update_call_status(&state, call_id, "failed", None, Some("Timeout")).await?;
        }

        result
    }
}

/// GET /calls/{call_id} - Poll for call result (async mode)
pub async fn get_call_result(
    State(state): State<AppState>,
    Path(call_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<HttpsCallResponse>, CallError> {
    // Validate payment key owns this call
    let payment_key = parse_payment_key_header(&headers)?;

    // Get call from database
    let call = sqlx::query(
        r#"
        SELECT call_id, owner, nonce, project_id, status, output_data, error_message,
               compute_cost, instructions, time_ms, attestation_url
        FROM https_calls
        WHERE call_id = $1
        "#
    )
    .bind(call_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| CallError::InternalError(format!("Database error: {}", e)))?
    .ok_or(CallError::InternalError("Call not found".to_string()))?;

    // Verify ownership
    let owner: String = call.get("owner");
    let nonce: i32 = call.get("nonce");
    if owner != payment_key.owner || nonce as u32 != payment_key.nonce {
        return Err(CallError::InvalidKey);
    }

    let status: String = call.get("status");
    let output_data: Option<String> = call.get("output_data");
    let error_message: Option<String> = call.get("error_message");
    let compute_cost: Option<BigDecimal> = call.get("compute_cost");
    let instructions: Option<i64> = call.get("instructions");
    let time_ms: Option<i64> = call.get("time_ms");
    let attestation_url: Option<String> = None; // TODO: implement

    let output = output_data.and_then(|s| serde_json::from_str(&s).ok());

    Ok(Json(HttpsCallResponse {
        call_id: call_id.to_string(),
        status,
        output,
        error: error_message,
        compute_cost: compute_cost.map(|d| d.to_string()),
        instructions: instructions.map(|i| i as u64),
        time_ms: time_ms.map(|t| t as u64),
        poll_url: None,
        attestation_url,
    }))
}

// ============================================================================
// Helper functions
// ============================================================================

/// Parse X-Payment-Key header: {owner}:{nonce}:{key}
fn parse_payment_key_header(headers: &HeaderMap) -> Result<PaymentKeyHeader, CallError> {
    let header_value = headers
        .get("X-Payment-Key")
        .ok_or(CallError::MissingPaymentKey)?
        .to_str()
        .map_err(|_| CallError::InvalidPaymentKeyFormat("Invalid UTF-8".to_string()))?;

    let parts: Vec<&str> = header_value.splitn(3, ':').collect();
    if parts.len() != 3 {
        return Err(CallError::InvalidPaymentKeyFormat(
            "Expected format: owner:nonce:key".to_string()
        ));
    }

    let owner = parts[0].to_string();
    let nonce: u32 = parts[1].parse()
        .map_err(|_| CallError::InvalidPaymentKeyFormat("Invalid nonce".to_string()))?;
    let key = parts[2].to_string();

    // Validate owner is valid NEAR account
    if owner.is_empty() || owner.len() > 64 {
        return Err(CallError::InvalidPaymentKeyFormat("Invalid owner".to_string()));
    }

    // Validate key is not empty
    if key.is_empty() {
        return Err(CallError::InvalidPaymentKeyFormat("Empty key".to_string()));
    }

    // Validate key is valid hex and exactly 64 characters (32 bytes)
    // Hex format: 0-9, a-f, A-F (alphanumeric only, no special chars)
    if key.len() != 64 {
        return Err(CallError::InvalidPaymentKeyFormat(
            format!("Key must be exactly 64 hex characters (got {} chars)", key.len())
        ));
    }

    // Validate all characters are hex
    if !key.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(CallError::InvalidPaymentKeyFormat(
            "Key must contain only hex characters (0-9, a-f, A-F)".to_string()
        ));
    }

    // Normalize to lowercase for consistency
    let key = key.to_lowercase();

    Ok(PaymentKeyHeader { owner, nonce, key })
}

/// Parse numeric header value as u128 (minimal token units)
fn parse_header_u128(headers: &HeaderMap, name: &str) -> Option<u128> {
    headers.get(name)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse().ok())
}

/// Check rate limit for payment key
async fn check_payment_key_rate_limit(
    state: &AppState,
    payment_key: &PaymentKeyHeader,
) -> Result<(), CallError> {
    let mut conn = state.redis.get_multiplexed_async_connection().await
        .map_err(|e| CallError::InternalError(format!("Redis error: {}", e)))?;

    let rate_key = format!("pk_rate:{}:{}", payment_key.owner, payment_key.nonce);
    let count: i64 = conn.incr(&rate_key, 1).await
        .map_err(|e| CallError::InternalError(format!("Redis error: {}", e)))?;

    // Set TTL on first request
    if count == 1 {
        let _: () = conn.expire(&rate_key, 60).await
            .map_err(|e| CallError::InternalError(format!("Redis error: {}", e)))?;
    }

    if count > state.config.payment_key_rate_limit_per_minute as i64 {
        return Err(CallError::RateLimitExceeded);
    }

    Ok(())
}

/// Validate payment key via PostgreSQL and return metadata
///
/// Payment key data is synced from contract to PostgreSQL via TopUp events.
/// This function does NOT call keystore or read from contract - only PostgreSQL.
async fn validate_payment_key(
    state: &AppState,
    payment_key: &PaymentKeyHeader,
) -> Result<PaymentKeyMetadata, CallError> {
    // Query PostgreSQL for payment key metadata
    let row = sqlx::query(
        r#"
        SELECT key_hash, initial_balance, project_ids, max_per_call
        FROM payment_keys
        WHERE owner = $1 AND nonce = $2 AND deleted_at IS NULL
        "#
    )
    .bind(&payment_key.owner)
    .bind(payment_key.nonce as i32)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| CallError::InternalError(format!("Database error: {}", e)))?;

    // Key not found or deleted
    let row = row.ok_or_else(|| {
        debug!(
            "Payment key not found in PostgreSQL: owner={} nonce={}",
            payment_key.owner, payment_key.nonce
        );
        CallError::InvalidKey
    })?;

    // Extract fields from database row
    let stored_key_hash: String = row.get("key_hash");
    let initial_balance: String = row.get("initial_balance");
    let project_ids: Vec<String> = row.get("project_ids");
    let max_per_call: Option<String> = row.get("max_per_call");

    // Validate key hash: SHA256(provided_key) == stored_key_hash
    let provided_key_hash = hex::encode(Sha256::digest(payment_key.key.as_bytes()));
    if provided_key_hash != stored_key_hash {
        debug!(
            "Key hash mismatch: provided={} stored={}",
            &provided_key_hash[..8], &stored_key_hash[..8.min(stored_key_hash.len())]
        );
        return Err(CallError::InvalidKey);
    }

    Ok(PaymentKeyMetadata {
        key_hash: stored_key_hash,
        initial_balance,
        project_ids,
        max_per_call,
    })
}

/// Fetch payment key balance (spent + reserved) - internal helper
async fn fetch_payment_key_balance(
    state: &AppState,
    owner: &str,
    nonce: u32,
) -> Result<(u128, u128), CallError> {
    let row = sqlx::query(
        "SELECT spent, reserved FROM payment_key_balances WHERE owner = $1 AND nonce = $2"
    )
    .bind(owner)
    .bind(nonce as i32)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| CallError::InternalError(format!("Database error: {}", e)))?;

    match row {
        Some(row) => {
            let spent: BigDecimal = row.get("spent");
            let reserved: BigDecimal = row.get("reserved");
            Ok((
                spent.to_string().parse().unwrap_or(0),
                reserved.to_string().parse().unwrap_or(0),
            ))
        }
        None => Ok((0, 0)), // No record = no spending yet
    }
}

/// Reserve balance for a call
async fn reserve_balance(
    state: &AppState,
    owner: &str,
    nonce: u32,
    amount: u128,
) -> Result<(), CallError> {
    sqlx::query(
        r#"
        INSERT INTO payment_key_balances (owner, nonce, reserved, last_reserved_at)
        VALUES ($1, $2, $3, NOW())
        ON CONFLICT (owner, nonce) DO UPDATE
        SET reserved = payment_key_balances.reserved + $3,
            last_reserved_at = NOW()
        "#
    )
    .bind(owner)
    .bind(nonce as i32)
    .bind(BigDecimal::from_str(&amount.to_string()).unwrap_or_default())
    .execute(&state.db)
    .await
    .map_err(|e| CallError::InternalError(format!("Database error: {}", e)))?;

    Ok(())
}

/// Release reserved balance
async fn release_balance(
    state: &AppState,
    owner: &str,
    nonce: u32,
    amount: u128,
) -> Result<(), CallError> {
    sqlx::query(
        r#"
        UPDATE payment_key_balances
        SET reserved = GREATEST(0, reserved - $3),
            last_reserved_at = NOW()
        WHERE owner = $1 AND nonce = $2
        "#
    )
    .bind(owner)
    .bind(nonce as i32)
    .bind(BigDecimal::from_str(&amount.to_string()).unwrap_or_default())
    .execute(&state.db)
    .await
    .map_err(|e| CallError::InternalError(format!("Database error: {}", e)))?;

    Ok(())
}

/// Create HTTPS call record in database
async fn create_https_call(
    state: &AppState,
    call_id: Uuid,
    owner: &str,
    nonce: u32,
    project_id: &str,
    input_data: &str,
    attached_deposit: u128,
) -> Result<(), CallError> {
    sqlx::query(
        r#"
        INSERT INTO https_calls (call_id, owner, nonce, project_id, status, input_data, attached_deposit)
        VALUES ($1, $2, $3, $4, 'pending', $5, $6)
        "#
    )
    .bind(call_id)
    .bind(owner)
    .bind(nonce as i32)
    .bind(project_id)
    .bind(input_data)
    .bind(BigDecimal::from_str(&attached_deposit.to_string()).unwrap_or_default())
    .execute(&state.db)
    .await
    .map_err(|e| CallError::InternalError(format!("Database error: {}", e)))?;

    Ok(())
}

/// Create execution task in Redis queue
async fn create_execution_task(
    state: &AppState,
    call_id: Uuid,
    project_id: &str,
    input_data: &str,
    resource_limits: ResourceLimits,
    sender_id: &str,
    payment_key_nonce: u32,
    compute_limit: u128,
    attached_deposit: u128,
) -> Result<bool, CallError> {
    // Create execution request for HTTPS call
    // Worker will resolve project_id -> code_source from contract
    let execution_request = serde_json::json!({
        "request_id": 0, // HTTPS calls don't have request_id
        "data_id": call_id.to_string(),
        "project_id": project_id, // Worker resolves this to code_source
        "resource_limits": resource_limits,
        "input_data": input_data,
        "response_format": "Json",
        "context": {
            "sender_id": sender_id
        },
        "user_account_id": sender_id,
        "is_https_call": true,
        "call_id": call_id.to_string(),
        "compute_limit_usd": compute_limit.to_string(),
        "attached_deposit_usd": attached_deposit.to_string(),
        // HTTPS-specific: env vars for WASM execution
        "payment_key_owner": sender_id, // Payment Key owner (NEAR account)
        "payment_key_nonce": payment_key_nonce, // Payment Key nonce (for attestation)
        "usd_payment": attached_deposit.to_string() // USD payment to project owner (X-Attached-Deposit)
    });

    let request_json = serde_json::to_string(&execution_request)
        .map_err(|e| CallError::InternalError(format!("JSON serialize error: {}", e)))?;

    let mut conn = state.redis.get_multiplexed_async_connection().await
        .map_err(|e| CallError::InternalError(format!("Redis error: {}", e)))?;

    // Push to execute queue (assuming WASM is compiled via projects)
    let queue_name = &state.config.redis_queue_execute;
    let _: () = conn.lpush(queue_name, request_json).await
        .map_err(|e| CallError::InternalError(format!("Redis error: {}", e)))?;

    info!("📤 HTTPS call task created: call_id={} project_id={}", call_id, project_id);

    Ok(true)
}

/// Wait for execution result (sync mode)
async fn wait_for_result(
    state: &AppState,
    call_id: Uuid,
    timeout_seconds: u64,
) -> Result<Json<HttpsCallResponse>, CallError> {
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(timeout_seconds);

    loop {
        // Check if call is complete
        let row = sqlx::query(
            "SELECT status, output_data, error_message, compute_cost, instructions, time_ms FROM https_calls WHERE call_id = $1"
        )
        .bind(call_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| CallError::InternalError(format!("Database error: {}", e)))?;

        if let Some(row) = row {
            let status: String = row.get("status");
            if status == "completed" || status == "failed" {
                let output_data: Option<String> = row.get("output_data");
                let error_message: Option<String> = row.get("error_message");
                let compute_cost: Option<BigDecimal> = row.get("compute_cost");
                let instructions: Option<i64> = row.get("instructions");
                let time_ms: Option<i64> = row.get("time_ms");

                let output = output_data.and_then(|s| serde_json::from_str(&s).ok());

                return Ok(Json(HttpsCallResponse {
                    call_id: call_id.to_string(),
                    status,
                    output,
                    error: error_message,
                    compute_cost: compute_cost.map(|d| d.to_string()),
                    instructions: instructions.map(|i| i as u64),
                    time_ms: time_ms.map(|t| t as u64),
                    poll_url: None,
                    attestation_url: None,
                }));
            }
        }

        // Check timeout
        if start.elapsed() >= timeout {
            return Err(CallError::InternalError("Timeout waiting for result".to_string()));
        }

        // Poll interval
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

/// Update call status in database
async fn update_call_status(
    state: &AppState,
    call_id: Uuid,
    status: &str,
    output: Option<&str>,
    error: Option<&str>,
) -> Result<(), CallError> {
    sqlx::query(
        r#"
        UPDATE https_calls
        SET status = $2, output_data = $3, error_message = $4, completed_at = NOW()
        WHERE call_id = $1
        "#
    )
    .bind(call_id)
    .bind(status)
    .bind(output)
    .bind(error)
    .execute(&state.db)
    .await
    .map_err(|e| CallError::InternalError(format!("Database error: {}", e)))?;

    Ok(())
}

/// Finalize call - update balances and record usage
async fn finalize_call(
    state: &AppState,
    call_id: Uuid,
    owner: &str,
    nonce: u32,
    project_id: &str,
    project_owner: &str,
    reserved_amount: u128,
    actual_cost: u128,
    attached_deposit: u128,
    refund_usd: u128,
    success: bool,
    error: Option<String>,
    job_id: Option<i64>,
) -> Result<(), CallError> {
    let mut tx = state.db.begin().await
        .map_err(|e| CallError::InternalError(format!("Database error: {}", e)))?;

    // Calculate developer earnings: attached_deposit - refund
    let developer_amount = attached_deposit.saturating_sub(refund_usd);

    // 1. Release reserved and update spent
    // Note: spent includes actual_cost + developer_amount (refund goes back to available)
    let total_spent = actual_cost + developer_amount;
    sqlx::query(
        r#"
        UPDATE payment_key_balances
        SET reserved = GREATEST(0, reserved - $3),
            spent = spent + $4,
            last_used_at = NOW()
        WHERE owner = $1 AND nonce = $2
        "#
    )
    .bind(owner)
    .bind(nonce as i32)
    .bind(BigDecimal::from_str(&reserved_amount.to_string()).unwrap_or_default())
    .bind(BigDecimal::from_str(&total_spent.to_string()).unwrap_or_default())
    .execute(&mut *tx)
    .await
    .map_err(|e| CallError::InternalError(format!("Database error: {}", e)))?;

    // 2. Record usage
    let status_str = if success { "completed" } else { "failed" };
    sqlx::query(
        r#"
        INSERT INTO payment_key_usage (owner, nonce, call_id, project_id, compute_cost, attached_deposit, status, error_message, job_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        "#
    )
    .bind(owner)
    .bind(nonce as i32)
    .bind(call_id)
    .bind(project_id)
    .bind(BigDecimal::from_str(&actual_cost.to_string()).unwrap_or_default())
    .bind(BigDecimal::from_str(&attached_deposit.to_string()).unwrap_or_default())
    .bind(status_str)
    .bind(&error)
    .bind(job_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| CallError::InternalError(format!("Database error: {}", e)))?;

    // 3. Update project owner earnings (only if developer_amount > 0)
    if developer_amount > 0 {
        sqlx::query(
            r#"
            INSERT INTO project_owner_earnings (project_owner, balance, total_earned, updated_at)
            VALUES ($1, $2, $2, NOW())
            ON CONFLICT (project_owner) DO UPDATE
            SET balance = project_owner_earnings.balance + $2,
                total_earned = project_owner_earnings.total_earned + $2,
                updated_at = NOW()
            "#
        )
        .bind(project_owner)
        .bind(BigDecimal::from_str(&developer_amount.to_string()).unwrap_or_default())
        .execute(&mut *tx)
        .await
        .map_err(|e| CallError::InternalError(format!("Database error: {}", e)))?;

        // 4. Record in earnings_history for HTTPS calls
        sqlx::query(
            r#"
            INSERT INTO earnings_history (
                project_owner, project_id, attached_usd, refund_usd, amount, source,
                call_id, payment_key_owner, payment_key_nonce
            )
            VALUES ($1, $2, $3, $4, $5, 'https', $6, $7, $8)
            "#
        )
        .bind(project_owner)
        .bind(project_id)
        .bind(BigDecimal::from_str(&attached_deposit.to_string()).unwrap_or_default())
        .bind(BigDecimal::from_str(&refund_usd.to_string()).unwrap_or_default())
        .bind(BigDecimal::from_str(&developer_amount.to_string()).unwrap_or_default())
        .bind(call_id)
        .bind(owner)
        .bind(nonce as i32)
        .execute(&mut *tx)
        .await
        .map_err(|e| CallError::InternalError(format!("Database error: {}", e)))?;
    }

    tx.commit().await
        .map_err(|e| CallError::InternalError(format!("Database error: {}", e)))?;

    info!(
        "💰 HTTPS call finalized: call_id={} cost={} attached_deposit={} refund={} developer_amount={} status={}",
        call_id, actual_cost, attached_deposit, refund_usd, developer_amount, status_str
    );

    Ok(())
}

// ============================================================================
// Worker endpoints for HTTPS call completion
// ============================================================================

/// Request to complete an HTTPS call (from worker)
#[derive(Debug, Deserialize)]
pub struct CompleteHttpsCallRequest {
    pub call_id: Uuid,
    pub success: bool,
    #[serde(default)]
    pub output: Option<serde_json::Value>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub instructions: u64,
    #[serde(default)]
    pub time_ms: u64,
    /// Refund amount to return to user from attached_usd (stablecoin, minimal token units)
    #[serde(default)]
    pub refund_usd: Option<u64>,
}

/// Response for completing HTTPS call
#[derive(Debug, Serialize)]
pub struct CompleteHttpsCallResponse {
    pub call_id: String,
    pub finalized: bool,
}

/// POST /calls/complete - Worker notifies coordinator that HTTPS call is done
///
/// This endpoint is called by workers when they finish executing an HTTPS call.
/// The coordinator then:
/// 1. Updates call status in database
/// 2. Calculates actual compute cost
/// 3. Finalizes balances (release reserved, add to spent)
pub async fn complete_https_call(
    State(state): State<AppState>,
    Json(req): Json<CompleteHttpsCallRequest>,
) -> Result<Json<CompleteHttpsCallResponse>, CallError> {
    let call_id = req.call_id;
    info!("📤 HTTPS call completion: call_id={} success={}", call_id, req.success);

    // Get call details from database
    let call = sqlx::query(
        r#"
        SELECT owner, nonce, project_id, attached_deposit, status
        FROM https_calls
        WHERE call_id = $1
        "#
    )
    .bind(call_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| CallError::InternalError(format!("Database error: {}", e)))?
    .ok_or(CallError::InternalError("Call not found".to_string()))?;

    let status: String = call.get("status");
    if status == "completed" || status == "failed" {
        // Already finalized
        return Ok(Json(CompleteHttpsCallResponse {
            call_id: call_id.to_string(),
            finalized: false,
        }));
    }

    let owner: String = call.get("owner");
    let nonce: i32 = call.get("nonce");
    let project_id: String = call.get("project_id");
    let attached_deposit: BigDecimal = call.get("attached_deposit");
    let attached_deposit_u128: u128 = attached_deposit.to_string().parse().unwrap_or(0);

    // Calculate compute cost based on actual resources used
    // For now, use a simple formula based on instructions and time
    let pricing = state.pricing.read().await;
    let per_instruction_usd = 1u128; // 0.000001 USD per million instructions
    let per_ms_usd = 10u128; // 0.00001 USD per ms

    let instruction_cost = (req.instructions / 1_000_000) as u128 * per_instruction_usd;
    let time_cost = req.time_ms as u128 * per_ms_usd;
    let base_fee_usd = 1000u128; // $0.001 base fee
    let actual_cost = base_fee_usd + instruction_cost + time_cost;
    drop(pricing);

    // Get reserved amount (compute_limit + attached_deposit)
    // For simplicity, estimate based on actual cost + attached_deposit
    let reserved_amount = actual_cost + attached_deposit_u128 + 10000; // Add buffer for estimation error

    // Update call in database
    let output_json = req.output.as_ref().map(|o| o.to_string());
    let new_status = if req.success { "completed" } else { "failed" };

    sqlx::query(
        r#"
        UPDATE https_calls
        SET status = $2,
            output_data = $3,
            error_message = $4,
            instructions = $5,
            time_ms = $6,
            compute_cost = $7,
            completed_at = NOW()
        WHERE call_id = $1
        "#
    )
    .bind(call_id)
    .bind(new_status)
    .bind(output_json)
    .bind(&req.error)
    .bind(req.instructions as i64)
    .bind(req.time_ms as i64)
    .bind(BigDecimal::from_str(&actual_cost.to_string()).unwrap_or_default())
    .execute(&state.db)
    .await
    .map_err(|e| CallError::InternalError(format!("Database error: {}", e)))?;

    // Extract project owner from project_id (format: "owner.near/project-name")
    let project_owner = project_id.split('/').next().unwrap_or(&project_id).to_string();

    // Find execute job's job_id for attestation lookup
    let job_id: Option<i64> = sqlx::query_scalar(
        r#"
        SELECT job_id FROM execution_history
        WHERE data_id = $1 AND job_type = 'execute'
        ORDER BY created_at DESC
        LIMIT 1
        "#
    )
    .bind(call_id.to_string())
    .fetch_optional(&state.db)
    .await
    .map_err(|e| CallError::InternalError(format!("Database error: {}", e)))?;

    // Get refund_usd from request (default 0)
    let refund_usd = req.refund_usd.unwrap_or(0) as u128;

    // Finalize balances
    finalize_call(
        &state,
        call_id,
        &owner,
        nonce as u32,
        &project_id,
        &project_owner,
        reserved_amount,
        actual_cost,
        attached_deposit_u128,
        refund_usd,
        req.success,
        req.error.clone(),
        job_id,
    ).await?;

    info!(
        "✅ HTTPS call finalized: call_id={} cost={} status={}",
        call_id, actual_cost, new_status
    );

    Ok(Json(CompleteHttpsCallResponse {
        call_id: call_id.to_string(),
        finalized: true,
    }))
}

// ============================================================================
// Public endpoints for Payment Key balance and usage
// ============================================================================

/// Payment key balance response
#[derive(Debug, Serialize)]
pub struct PaymentKeyBalanceResponse {
    pub owner: String,
    pub nonce: u32,
    pub initial_balance: String,
    pub spent: String,
    pub reserved: String,
    pub available: String,
    pub last_used_at: Option<String>,
}

/// Payment key usage response
#[derive(Debug, Serialize)]
pub struct PaymentKeyUsageResponse {
    pub usage: Vec<PaymentKeyUsageItem>,
    pub total: i64,
    pub offset: i64,
    pub limit: i64,
}

/// Query params for usage pagination
#[derive(Debug, Deserialize)]
pub struct UsagePaginationQuery {
    #[serde(default)]
    pub offset: Option<i64>,
    #[serde(default = "default_usage_limit")]
    pub limit: Option<i64>,
}

fn default_usage_limit() -> Option<i64> {
    Some(20)
}

/// Single usage item
#[derive(Debug, Serialize)]
pub struct PaymentKeyUsageItem {
    pub id: String,
    pub call_id: String,
    pub job_id: Option<i64>,
    pub project_id: String,
    pub compute_cost: String,
    pub attached_deposit: String,
    pub status: String,
    pub created_at: String,
}

/// Endpoint: GET /public/payment-keys/{owner}/{nonce}/balance
///
/// Returns the current balance for a payment key.
/// initial_balance is fetched from payment_keys table (PostgreSQL - source of truth).
/// spent/reserved from payment_key_balances table.
pub async fn get_payment_key_balance(
    State(state): State<AppState>,
    Path((owner, nonce)): Path<(String, u32)>,
) -> Result<Json<PaymentKeyBalanceResponse>, (StatusCode, String)> {
    // Get initial_balance from payment_keys table (PostgreSQL - source of truth)
    // This table is populated by worker after TopUp via POST /topup/complete
    let initial_balance: String = {
        let row = sqlx::query(
            r#"
            SELECT initial_balance
            FROM payment_keys
            WHERE owner = $1 AND nonce = $2 AND deleted_at IS NULL
            "#
        )
        .bind(&owner)
        .bind(nonce as i32)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e)))?;

        match row {
            Some(r) => r.get("initial_balance"),
            None => "0".to_string(), // Key not initialized yet (TopUp not processed)
        }
    };

    // Get spent and reserved from payment_key_balances table
    let row = sqlx::query(
        r#"
        SELECT spent, reserved, last_used_at
        FROM payment_key_balances
        WHERE owner = $1 AND nonce = $2
        "#
    )
    .bind(&owner)
    .bind(nonce as i32)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e)))?;

    let (spent, reserved, last_used_at): (String, String, Option<String>) = match row {
        Some(r) => {
            let spent: BigDecimal = r.get("spent");
            let reserved: BigDecimal = r.get("reserved");
            let last_used: Option<chrono::DateTime<chrono::Utc>> = r.get("last_used_at");
            (spent.to_string(), reserved.to_string(), last_used.map(|d| d.to_rfc3339()))
        }
        None => ("0".to_string(), "0".to_string(), None),
    };

    // Calculate available
    let initial = BigDecimal::from_str(&initial_balance).unwrap_or_default();
    let spent_dec = BigDecimal::from_str(&spent).unwrap_or_default();
    let reserved_dec = BigDecimal::from_str(&reserved).unwrap_or_default();
    let available = &initial - &spent_dec - &reserved_dec;

    Ok(Json(PaymentKeyBalanceResponse {
        owner,
        nonce,
        initial_balance,
        spent,
        reserved,
        available: available.to_string(),
        last_used_at,
    }))
}

/// Endpoint: GET /public/payment-keys/{owner}/{nonce}/usage
///
/// Returns usage history for a payment key with pagination.
/// Query params: offset (default 0), limit (default 20, max 100)
pub async fn get_payment_key_usage(
    State(state): State<AppState>,
    Path((owner, nonce)): Path<(String, u32)>,
    Query(pagination): Query<UsagePaginationQuery>,
) -> Result<Json<PaymentKeyUsageResponse>, (StatusCode, String)> {
    let offset = pagination.offset.unwrap_or(0).max(0);
    let limit = pagination.limit.unwrap_or(20).clamp(1, 100);

    // Get total count
    let total: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM payment_key_usage
        WHERE owner = $1 AND nonce = $2
        "#
    )
    .bind(&owner)
    .bind(nonce as i32)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e)))?;

    // Get paginated rows
    let rows = sqlx::query(
        r#"
        SELECT id, call_id, job_id, project_id, compute_cost, attached_deposit, status, created_at
        FROM payment_key_usage
        WHERE owner = $1 AND nonce = $2
        ORDER BY created_at DESC
        LIMIT $3 OFFSET $4
        "#
    )
    .bind(&owner)
    .bind(nonce as i32)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e)))?;

    let usage: Vec<PaymentKeyUsageItem> = rows
        .iter()
        .map(|r| {
            let id: i64 = r.get("id");
            let call_id: Uuid = r.get("call_id");
            let job_id: Option<i64> = r.get("job_id");
            let project_id: String = r.get("project_id");
            let compute_cost: BigDecimal = r.get("compute_cost");
            let attached_deposit: BigDecimal = r.get("attached_deposit");
            let status: String = r.get("status");
            let created_at: chrono::DateTime<chrono::Utc> = r.get("created_at");

            PaymentKeyUsageItem {
                id: id.to_string(),
                call_id: call_id.to_string(),
                job_id,
                project_id,
                compute_cost: compute_cost.to_string(),
                attached_deposit: attached_deposit.to_string(),
                status,
                created_at: created_at.to_rfc3339(),
            }
        })
        .collect();

    Ok(Json(PaymentKeyUsageResponse { usage, total, offset, limit }))
}
