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

#[derive(Deserialize)]
pub struct PollTaskQuery {
    #[serde(default = "default_timeout")]
    timeout: u64,
    /// Which queue to poll: "compile" or "execute"
    /// If not specified, defaults based on capabilities
    #[serde(default)]
    queue: Option<String>,
    /// Worker capabilities: "compilation,execution" or "compilation" or "execution"
    #[serde(default, deserialize_with = "deserialize_capabilities")]
    capabilities: Vec<String>,
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

    // Determine which queue to poll based on capabilities or explicit queue parameter
    let queue_name = if let Some(ref queue) = params.queue {
        match queue.as_str() {
            "compile" => state.config.redis_queue_compile.clone(),
            "execute" => state.config.redis_queue_execute.clone(),
            _ => {
                error!("Invalid queue name: {}. Use 'compile' or 'execute'", queue);
                return Err(StatusCode::BAD_REQUEST);
            }
        }
    } else {
        // Determine from capabilities
        let can_compile = params.capabilities.contains(&"compilation".to_string());
        let can_execute = params.capabilities.contains(&"execution".to_string());

        match (can_compile, can_execute) {
            (true, false) => state.config.redis_queue_compile.clone(),
            (false, true) => state.config.redis_queue_execute.clone(),
            (true, true) => {
                // Full worker - poll both queues (compile first, then execute)
                // For now, poll compile queue - full workers should handle compilation priority
                state.config.redis_queue_compile.clone()
            }
            (false, false) => {
                error!("Worker has no capabilities specified");
                return Err(StatusCode::BAD_REQUEST);
            }
        }
    };

    debug!("Polling queue '{}' with timeout {}s", queue_name, timeout);

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
        .brpop(&queue_name, timeout as f64)
        .await
        .map_err(|e| {
            error!("Redis BRPOP error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    match result {
        Some((_key, json)) => {
            debug!("ExecutionRequest received from {}: {}", queue_name, json);
            let request: ExecutionRequest = serde_json::from_str(&json).map_err(|e| {
                error!("Failed to deserialize execution request: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
            Ok(Json(request))
        }
        None => {
            debug!("Poll timeout on queue '{}' - no tasks available", queue_name);
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
        "SELECT COUNT(*) FROM jobs WHERE request_id = $1 AND data_id = $2"
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

    // Calculate WASM checksum to check if compilation is needed
    let wasm_checksum = calculate_wasm_checksum(&code_source);

    // Check if WASM already exists in cache
    let wasm_exists = sqlx::query!(
        "SELECT checksum FROM wasm_cache WHERE checksum = $1",
        wasm_checksum
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        error!("Failed to check WASM cache: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Verify physical file exists if DB record exists
    let wasm_file_exists = if wasm_exists.is_some() {
        let wasm_path = state.config.wasm_cache_dir.join(&format!("{}.wasm", wasm_checksum));
        wasm_path.exists()
    } else {
        false
    };

    // Determine which queue to use
    // force_rebuild forces compilation even if WASM exists
    let needs_compilation = !wasm_file_exists || payload.force_rebuild;

    let queue_name = if needs_compilation {
        // Needs compilation - push to compile queue
        if payload.force_rebuild && wasm_file_exists {
            debug!("WASM {} found but force_rebuild=true, pushing to compile queue", wasm_checksum);
        } else {
            debug!("WASM {} not in cache, pushing to compile queue", wasm_checksum);
        }
        &state.config.redis_queue_compile
    } else {
        // WASM exists - go directly to execute queue
        debug!("WASM {} found in cache, pushing to execute queue", wasm_checksum);
        &state.config.redis_queue_execute
    };

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
        compile_only: payload.compile_only,
        force_rebuild: payload.force_rebuild,
        store_on_fastfs: payload.store_on_fastfs,
    };

    let request_json = serde_json::to_string(&execution_request).map_err(|e| {
        error!("Failed to serialize execution request: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let mut conn = state.redis.get_multiplexed_async_connection().await.map_err(|e| {
        error!("Failed to get Redis connection: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    conn.lpush::<_, _, ()>(queue_name, request_json)
        .await
        .map_err(|e| {
            error!("Failed to push execution request to Redis: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Store full request data for retrieval after compilation
    // This is needed when complete_job creates execute task
    let secrets_profile = execution_request.secrets_ref.as_ref().map(|s| s.profile.clone());
    let secrets_account_id = execution_request.secrets_ref.as_ref().map(|s| s.account_id.clone());
    let response_format = match execution_request.response_format {
        crate::models::ResponseFormat::Bytes => "bytes",
        crate::models::ResponseFormat::Text => "text",
        crate::models::ResponseFormat::Json => "json",
    };

    let _ = sqlx::query!(
        r#"
        INSERT INTO execution_requests (
            request_id, data_id, input_data,
            max_instructions, max_memory_mb, max_execution_seconds,
            secrets_profile, secrets_account_id, response_format,
            context_sender_id, context_block_height, context_block_timestamp,
            context_contract_id, context_transaction_hash, context_receipt_id,
            context_predecessor_id, context_signer_public_key, context_gas_burnt,
            compile_only, force_rebuild, store_on_fastfs
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21
        )
        ON CONFLICT (request_id) DO NOTHING
        "#,
        request_id as i64,
        execution_request.data_id,
        execution_request.input_data,
        execution_request.resource_limits.max_instructions as i64,
        execution_request.resource_limits.max_memory_mb as i32,
        execution_request.resource_limits.max_execution_seconds as i64,
        secrets_profile,
        secrets_account_id,
        response_format,
        execution_request.context.sender_id,
        execution_request.context.block_height.map(|h| h as i64),
        execution_request.context.block_timestamp.map(|t| t as i64),
        execution_request.context.contract_id,
        execution_request.context.transaction_hash,
        execution_request.context.receipt_id,
        execution_request.context.predecessor_id,
        execution_request.context.signer_public_key,
        execution_request.context.gas_burnt.map(|g| g as i64),
        execution_request.compile_only,
        execution_request.force_rebuild,
        execution_request.store_on_fastfs
    )
    .execute(&state.db)
    .await
    .map_err(|e| {
        error!("Failed to store execution request: {}", e);
        // Don't fail - task is already in queue
    });

    debug!("ExecutionRequest {} pushed to queue '{}'", request_id, queue_name);
    Ok((StatusCode::CREATED, Json(CreateTaskResponse {
        request_id: request_id as i64,
        created: true,
    })))
}

/// Calculate WASM checksum from code source
fn calculate_wasm_checksum(code_source: &crate::models::CodeSource) -> String {
    use sha2::{Sha256, Digest};
    match code_source {
        crate::models::CodeSource::GitHub { repo, commit, build_target } => {
            let input = format!("{}:{}:{}", repo, commit, build_target);
            let hash = Sha256::digest(input.as_bytes());
            hex::encode(hash)
        }
        crate::models::CodeSource::WasmUrl { hash, .. } => {
            // For WasmUrl, use the provided hash as checksum
            hash.clone()
        }
    }
}
