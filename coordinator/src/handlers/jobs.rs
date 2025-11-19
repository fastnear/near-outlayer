use axum::{extract::State, http::StatusCode, Json};
use redis::AsyncCommands;
use tracing::{debug, error, info};

use crate::models::{ClaimJobRequest, ClaimJobResponse, CompleteJobRequest, ExecutionRequest, JobInfo, JobType, CodeSource, ResourceLimits, SecretsReference, ResponseFormat, ExecutionContext};
use crate::AppState;

/// Claim jobs for a task - worker claims a single job from its queue
///
/// With separate queues:
/// - Compile workers poll compile queue → claim compile job
/// - Execute workers poll execute queue → claim execute job
/// - Full workers poll compile queue first → claim compile OR execute job
pub async fn claim_job(
    State(state): State<AppState>,
    Json(payload): Json<ClaimJobRequest>,
) -> Result<Json<ClaimJobResponse>, StatusCode> {
    debug!(
        "Worker {} claiming task: request_id={} data_id={} capabilities={:?}",
        payload.worker_id, payload.request_id, payload.data_id, payload.capabilities
    );

    // Check worker capabilities
    let can_compile = payload.capabilities.contains(&"compilation".to_string());
    let can_execute = payload.capabilities.contains(&"execution".to_string());

    if !can_compile && !can_execute {
        error!(
            "❌ Worker {} has no capabilities (must have at least 'compilation' or 'execution')",
            payload.worker_id
        );
        return Err(StatusCode::BAD_REQUEST);
    }

    // Calculate WASM checksum from code_source
    let wasm_checksum = calculate_wasm_checksum(&payload.code_source);

    // Extract GitHub repo and commit for database
    let (github_repo, github_commit) = match &payload.code_source {
        CodeSource::GitHub { repo, commit, .. } => (Some(repo.clone()), Some(commit.clone())),
        CodeSource::WasmUrl { .. } => (None, None),
    };

    // Check if WASM exists in cache
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

    let wasm_file_exists = if wasm_exists.is_some() {
        let wasm_path = state.config.wasm_cache_dir.join(&format!("{}.wasm", wasm_checksum));
        let file_exists = wasm_path.exists();

        if !file_exists {
            info!(
                "⚠️ WASM checksum {} in DB but file missing at {:?}, will recompile",
                wasm_checksum, wasm_path
            );
            // Delete stale DB record
            if let Err(e) = sqlx::query!(
                "DELETE FROM wasm_cache WHERE checksum = $1",
                wasm_checksum
            )
            .execute(&state.db)
            .await {
                error!("Failed to delete stale WASM cache record: {}", e);
            }
        }

        file_exists
    } else {
        false
    };

    // Handle force_rebuild - treat as if WASM doesn't exist
    let effective_wasm_exists = if payload.force_rebuild {
        info!("🔄 force_rebuild=true, ignoring cached WASM");
        false
    } else {
        wasm_file_exists
    };

    let mut jobs = Vec::new();

    // Determine job type based on WASM availability and worker capabilities
    if !effective_wasm_exists && can_compile {
        // Need compilation - check if compile job already exists
        let existing_compile = sqlx::query!(
            "SELECT job_id FROM jobs WHERE request_id = $1 AND data_id = $2 AND job_type = 'compile'",
            payload.request_id as i64,
            payload.data_id
        )
        .fetch_optional(&state.db)
        .await
        .map_err(|e| {
            error!("Failed to check existing compile job: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        if existing_compile.is_some() {
            debug!("❌ Compile job already exists for request_id={}", payload.request_id);
            let pricing = state.pricing.read().await.clone();
            return Ok(Json(ClaimJobResponse { jobs: vec![], pricing }));
        }

        debug!(
            "🔨 Creating compile job for request_id={} checksum={}",
            payload.request_id, wasm_checksum
        );

        let compile_job_result = sqlx::query!(
            r#"
            INSERT INTO jobs (request_id, data_id, job_type, worker_id, status, wasm_checksum, user_account_id, near_payment_yocto, github_repo, github_commit, transaction_hash, created_at, updated_at)
            VALUES ($1, $2, 'compile', $3, 'in_progress', $4, $5, $6, $7, $8, $9, NOW(), NOW())
            RETURNING job_id
            "#,
            payload.request_id as i64,
            payload.data_id,
            payload.worker_id,
            wasm_checksum,
            payload.user_account_id.as_deref(),
            payload.near_payment_yocto.as_deref(),
            github_repo.as_deref(),
            github_commit.as_deref(),
            payload.transaction_hash.as_deref()
        )
        .fetch_one(&state.db)
        .await;

        match compile_job_result {
            Ok(compile_job) => {
                jobs.push(JobInfo {
                    job_id: compile_job.job_id,
                    job_type: JobType::Compile,
                    wasm_checksum: Some(wasm_checksum.clone()),
                    allowed: true,
                    compile_cost_yocto: None, // Not applicable for compile jobs
                    compile_error: None,
                });
            }
            Err(e) => {
                if let Some(db_err) = e.as_database_error() {
                    if db_err.is_unique_violation() {
                        info!("⚠️ Compile job already claimed by another worker for request_id={}", payload.request_id);
                        let pricing = state.pricing.read().await.clone();
                        return Ok(Json(ClaimJobResponse { jobs: vec![], pricing }));
                    }
                }
                error!("Failed to create compile job: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    } else if can_execute && !payload.compile_only {
        // Executor claiming job - either WASM exists or compilation failed
        // Skip if compile_only=true (only compilation was requested)

        // Check if compile job failed (executor needs to report error to contract)
        let compile_job = sqlx::query!(
            "SELECT compile_cost_yocto, compile_error, status FROM jobs WHERE request_id = $1 AND data_id = $2 AND job_type = 'compile'",
            payload.request_id as i64,
            payload.data_id
        )
        .fetch_optional(&state.db)
        .await
        .map_err(|e| {
            error!("Failed to fetch compile job: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        let (compile_cost_yocto, compile_error) = compile_job
            .as_ref()
            .map(|j| (j.compile_cost_yocto.clone(), j.compile_error.clone()))
            .unwrap_or((None, None));

        // If WASM doesn't exist and no compile error, executor can't do anything
        if !effective_wasm_exists && compile_error.is_none() {
            debug!(
                "❌ WASM not available and no compile error for request_id={}, executor cannot proceed",
                payload.request_id
            );
            let pricing = state.pricing.read().await.clone();
            return Ok(Json(ClaimJobResponse { jobs: vec![], pricing }));
        }

        // Check if execute job already exists
        let existing_execute = sqlx::query!(
            "SELECT job_id FROM jobs WHERE request_id = $1 AND data_id = $2 AND job_type = 'execute'",
            payload.request_id as i64,
            payload.data_id
        )
        .fetch_optional(&state.db)
        .await
        .map_err(|e| {
            error!("Failed to check existing execute job: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        if existing_execute.is_some() {
            debug!("❌ Execute job already exists for request_id={}", payload.request_id);
            let pricing = state.pricing.read().await.clone();
            return Ok(Json(ClaimJobResponse { jobs: vec![], pricing }));
        }

        if compile_error.is_some() {
            debug!(
                "⚡ Creating execute job for request_id={} to report compile error",
                payload.request_id
            );
        } else {
            debug!(
                "⚡ Creating execute job for request_id={} checksum={}",
                payload.request_id, wasm_checksum
            );
        }

        let execute_job_result = sqlx::query!(
            r#"
            INSERT INTO jobs (request_id, data_id, job_type, worker_id, status, wasm_checksum, user_account_id, near_payment_yocto, github_repo, github_commit, transaction_hash, created_at, updated_at)
            VALUES ($1, $2, 'execute', $3, 'in_progress', $4, $5, $6, $7, $8, $9, NOW(), NOW())
            RETURNING job_id
            "#,
            payload.request_id as i64,
            payload.data_id,
            payload.worker_id,
            wasm_checksum,
            payload.user_account_id.as_deref(),
            payload.near_payment_yocto.as_deref(),
            github_repo.as_deref(),
            github_commit.as_deref(),
            payload.transaction_hash.as_deref()
        )
        .fetch_one(&state.db)
        .await;

        match execute_job_result {
            Ok(execute_job) => {
                jobs.push(JobInfo {
                    job_id: execute_job.job_id,
                    job_type: JobType::Execute,
                    wasm_checksum: Some(wasm_checksum.clone()),
                    allowed: true,
                    compile_cost_yocto,
                    compile_error,
                });
            }
            Err(e) => {
                if let Some(db_err) = e.as_database_error() {
                    if db_err.is_unique_violation() {
                        info!("⚠️ Execute job already claimed by another worker for request_id={}", payload.request_id);
                        let pricing = state.pricing.read().await.clone();
                        return Ok(Json(ClaimJobResponse { jobs: vec![], pricing }));
                    }
                }
                error!("Failed to create execute job: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    } else {
        // Worker cannot handle this task (wrong capabilities for current state)
        debug!(
            "❌ Worker {} cannot handle task: wasm_exists={} can_compile={} can_execute={}",
            payload.worker_id, wasm_file_exists, can_compile, can_execute
        );
        let pricing = state.pricing.read().await.clone();
        return Ok(Json(ClaimJobResponse { jobs: vec![], pricing }));
    }

    debug!(
        "✅ Task claimed: request_id={} data_id={} worker={} jobs={:?}",
        payload.request_id,
        payload.data_id,
        payload.worker_id,
        jobs.iter().map(|j| format!("{:?}", j.job_type)).collect::<Vec<_>>()
    );

    let pricing = state.pricing.read().await.clone();
    Ok(Json(ClaimJobResponse { jobs, pricing }))
}

/// Calculate WASM checksum from code source
fn calculate_wasm_checksum(code_source: &CodeSource) -> String {
    match code_source {
        CodeSource::GitHub { repo, commit, build_target } => {
            use sha2::{Sha256, Digest};
            let input = format!("{}:{}:{}", repo, commit, build_target);
            let hash = Sha256::digest(input.as_bytes());
            hex::encode(hash)
        }
        CodeSource::WasmUrl { hash, .. } => {
            // For WasmUrl, use the provided hash as checksum
            hash.clone()
        }
    }
}

/// Complete a job - worker finished the job
pub async fn complete_job(
    State(state): State<AppState>,
    Json(payload): Json<CompleteJobRequest>,
) -> StatusCode {
    debug!(
        "Completing job {}: success={} time_ms={} error_category={:?}",
        payload.job_id, payload.success, payload.time_ms, payload.error_category
    );

    // Determine status based on success flag and error category
    let status = if payload.success {
        "completed"
    } else {
        // Use error_category if provided, otherwise default to "failed"
        payload.error_category
            .as_ref()
            .map(|c| c.as_str())
            .unwrap_or("failed")
    };

    // Error details (if failure)
    let error_details = if !payload.success {
        payload.error.as_deref()
    } else {
        None
    };

    // Update job status with error details and compile cost
    let update_result = sqlx::query!(
        r#"
        UPDATE jobs
        SET status = $1,
            wasm_checksum = $2,
            error_details = $3,
            compile_cost_yocto = $4,
            compile_time_ms = $5,
            compile_error = $6,
            completed_at = NOW(),
            updated_at = NOW()
        WHERE job_id = $7
        "#,
        status,
        payload.wasm_checksum,
        error_details,
        payload.compile_cost_yocto,
        if payload.success { Some(payload.time_ms as i64) } else { None },
        if !payload.success { payload.error.as_deref() } else { None },
        payload.job_id
    )
    .execute(&state.db)
    .await;

    if let Err(e) = update_result {
        error!("Failed to update job {}: {}", payload.job_id, e);
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    // Get job details for history and to create execute task after compile
    let job = sqlx::query!(
        r#"
        SELECT request_id, data_id, job_type, worker_id, wasm_checksum, user_account_id, near_payment_yocto, github_repo, github_commit, transaction_hash
        FROM jobs
        WHERE job_id = $1
        "#,
        payload.job_id
    )
    .fetch_optional(&state.db)
    .await;

    let job = match job {
        Ok(Some(j)) => j,
        Ok(None) => {
            error!("Job {} not found", payload.job_id);
            return StatusCode::NOT_FOUND;
        }
        Err(e) => {
            error!("Failed to fetch job details: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    // If compile job completed successfully, create execute task in execute queue
    if job.job_type == "compile" && payload.success {
        info!(
            "📦 Compile job {} completed, creating execute task for request_id={}",
            payload.job_id, job.request_id
        );

        // Get full execution request data to create execute task
        // We need to fetch the original request data (input_data, resource_limits, secrets_ref, etc.)
        // For now, we'll create a minimal execute request - the worker will need to handle this

        // Build code source from stored data
        let code_source = if let (Some(repo), Some(commit)) = (&job.github_repo, &job.github_commit) {
            CodeSource::GitHub {
                repo: repo.clone(),
                commit: commit.clone(),
                build_target: "wasm32-wasip1".to_string(), // Default, could be stored in DB
            }
        } else {
            error!("Missing github_repo or github_commit for compile job {}", payload.job_id);
            return StatusCode::INTERNAL_SERVER_ERROR;
        };

        // Create execution request for execute queue
        // Note: We're missing some fields (input_data, resource_limits, secrets_ref, context)
        // These should be stored when the original task is created
        // For now, fetch from a pending execute task or store in a separate table

        // Try to get the original execution request from pending execute jobs or a task table
        let original_request = sqlx::query!(
            r#"
            SELECT input_data, max_instructions, max_memory_mb, max_execution_seconds,
                   secrets_profile, secrets_account_id, response_format,
                   context_sender_id, context_block_height, context_block_timestamp,
                   context_contract_id, context_transaction_hash, context_receipt_id,
                   context_predecessor_id, context_signer_public_key, context_gas_burnt,
                   compile_only, force_rebuild, store_on_fastfs
            FROM execution_requests
            WHERE request_id = $1
            "#,
            job.request_id
        )
        .fetch_optional(&state.db)
        .await;

        let execution_request = match original_request {
            Ok(Some(req)) => {
                // Check if this was compile-only request
                if req.compile_only {
                    // If compile_result is provided, create execute task for executor to send result to contract
                    if let Some(ref compile_result) = payload.compile_result {
                        info!(
                            "📤 Compile-only request with result for request_id={}, creating execute task to send result: {}",
                            job.request_id, compile_result
                        );
                        // Save compile_result to execution_requests for executor to pick up
                        if let Err(e) = sqlx::query!(
                            "UPDATE execution_requests SET compile_result = $1 WHERE request_id = $2",
                            compile_result,
                            job.request_id
                        )
                        .execute(&state.db)
                        .await {
                            error!("Failed to save compile_result: {}", e);
                        }
                    } else {
                        // No compile_result - nothing to send to contract
                        info!(
                            "✅ Compile-only request completed for request_id={}, no result to send",
                            job.request_id
                        );
                        return StatusCode::OK;
                    }
                }

                ExecutionRequest {
                    request_id: job.request_id as u64,
                    data_id: job.data_id.clone(),
                    code_source,
                    resource_limits: ResourceLimits {
                        max_instructions: req.max_instructions.unwrap_or(1_000_000_000) as u64,
                        max_memory_mb: req.max_memory_mb.unwrap_or(128) as u32,
                        max_execution_seconds: req.max_execution_seconds.unwrap_or(60) as u64,
                    },
                    input_data: req.input_data.unwrap_or_default(),
                    secrets_ref: if let (Some(profile), Some(account_id)) = (req.secrets_profile.clone(), req.secrets_account_id.clone()) {
                        Some(SecretsReference { profile, account_id })
                    } else {
                        None
                    },
                    response_format: match req.response_format.as_deref() {
                        Some("bytes") => ResponseFormat::Bytes,
                        Some("json") => ResponseFormat::Json,
                        _ => ResponseFormat::Text,
                    },
                    context: ExecutionContext {
                        sender_id: req.context_sender_id,
                        block_height: req.context_block_height.map(|h| h as u64),
                        block_timestamp: req.context_block_timestamp.map(|t| t as u64),
                        contract_id: req.context_contract_id,
                        transaction_hash: req.context_transaction_hash,
                        receipt_id: req.context_receipt_id,
                        predecessor_id: req.context_predecessor_id,
                        signer_public_key: req.context_signer_public_key,
                        gas_burnt: req.context_gas_burnt.map(|g| g as u64),
                    },
                    user_account_id: job.user_account_id.clone(),
                    near_payment_yocto: job.near_payment_yocto.clone(),
                    transaction_hash: job.transaction_hash.clone(),
                    compile_only: req.compile_only,
                    force_rebuild: req.force_rebuild,
                    store_on_fastfs: req.store_on_fastfs,
                    compile_result: payload.compile_result.clone(),
                }
            }
            Ok(None) => {
                // Fallback: create minimal request with defaults
                // This happens if execution_requests table doesn't have the data
                info!("⚠️ No execution_requests record for request_id={}, using defaults", job.request_id);
                ExecutionRequest {
                    request_id: job.request_id as u64,
                    data_id: job.data_id.clone(),
                    code_source,
                    resource_limits: ResourceLimits {
                        max_instructions: 1_000_000_000,
                        max_memory_mb: 128,
                        max_execution_seconds: 60,
                    },
                    input_data: String::new(),
                    secrets_ref: None,
                    response_format: ResponseFormat::Text,
                    context: ExecutionContext::default(),
                    user_account_id: job.user_account_id.clone(),
                    near_payment_yocto: job.near_payment_yocto.clone(),
                    transaction_hash: job.transaction_hash.clone(),
                    compile_only: false,
                    force_rebuild: false,
                    store_on_fastfs: false,
                    compile_result: None,
                }
            }
            Err(e) => {
                error!("Failed to fetch original execution request: {}", e);
                // Continue with defaults rather than failing
                ExecutionRequest {
                    request_id: job.request_id as u64,
                    data_id: job.data_id.clone(),
                    code_source,
                    resource_limits: ResourceLimits {
                        max_instructions: 1_000_000_000,
                        max_memory_mb: 128,
                        max_execution_seconds: 60,
                    },
                    input_data: String::new(),
                    secrets_ref: None,
                    response_format: ResponseFormat::Text,
                    context: ExecutionContext::default(),
                    user_account_id: job.user_account_id.clone(),
                    near_payment_yocto: job.near_payment_yocto.clone(),
                    transaction_hash: job.transaction_hash.clone(),
                    compile_only: false,
                    force_rebuild: false,
                    store_on_fastfs: false,
                    compile_result: None,
                }
            }
        };

        // Serialize and push to execute queue
        let request_json = match serde_json::to_string(&execution_request) {
            Ok(json) => json,
            Err(e) => {
                error!("Failed to serialize execution request: {}", e);
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

        if let Err(e) = conn.lpush::<_, _, ()>(&state.config.redis_queue_execute, request_json).await {
            error!("Failed to push execute task to Redis: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR;
        }

        info!(
            "✅ Execute task created for request_id={} in queue '{}'",
            job.request_id, state.config.redis_queue_execute
        );
    } else if job.job_type == "compile" && !payload.success {
        // Compilation failed - still need to create execute task so executor can send error to contract
        info!(
            "❌ Compile job {} failed, creating execute task to report error for request_id={}",
            payload.job_id, job.request_id
        );

        // Build minimal code source for the execute task
        let code_source = if let (Some(repo), Some(commit)) = (&job.github_repo, &job.github_commit) {
            CodeSource::GitHub {
                repo: repo.clone(),
                commit: commit.clone(),
                build_target: "wasm32-wasip1".to_string(),
            }
        } else {
            error!("Missing github_repo or github_commit for failed compile job {}", payload.job_id);
            return StatusCode::INTERNAL_SERVER_ERROR;
        };

        // Create minimal execution request - executor will see compile_error and just report it
        let execution_request = ExecutionRequest {
            request_id: job.request_id as u64,
            data_id: job.data_id.clone(),
            code_source,
            resource_limits: ResourceLimits {
                max_instructions: 0, // Not used
                max_memory_mb: 0,
                max_execution_seconds: 0,
            },
            input_data: String::new(),
            secrets_ref: None,
            response_format: ResponseFormat::Text,
            context: ExecutionContext::default(),
            user_account_id: job.user_account_id.clone(),
            near_payment_yocto: job.near_payment_yocto.clone(),
            transaction_hash: job.transaction_hash.clone(),
            compile_only: false,
            force_rebuild: false,
            store_on_fastfs: false,
            compile_result: None,
        };

        let request_json = match serde_json::to_string(&execution_request) {
            Ok(json) => json,
            Err(e) => {
                error!("Failed to serialize execution request: {}", e);
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

        if let Err(e) = conn.lpush::<_, _, ()>(&state.config.redis_queue_execute, request_json).await {
            error!("Failed to push execute task to Redis: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR;
        }

        info!(
            "✅ Execute task (with compile error) created for request_id={} in queue '{}'",
            job.request_id, state.config.redis_queue_execute
        );
    }

    // Save to execution history
    let history_result = sqlx::query!(
        r#"
        INSERT INTO execution_history
        (job_id, request_id, data_id, job_type, worker_id, success,
         execution_time_ms, compile_time_ms, instructions_used,
         user_account_id, near_payment_yocto, actual_cost_yocto, compile_cost_yocto,
         github_repo, github_commit, transaction_hash,
         created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, NOW())
        "#,
        payload.job_id,
        job.request_id,
        job.data_id,
        job.job_type,
        job.worker_id,
        payload.success,
        // execution_time_ms - only for execute jobs
        if job.job_type == "execute" {
            Some(payload.time_ms as i64)
        } else {
            None
        },
        // compile_time_ms - only for compile jobs
        if job.job_type == "compile" {
            Some(payload.time_ms as i64)
        } else {
            None
        },
        // instructions_used - only for execute jobs
        if job.job_type == "execute" {
            Some(payload.instructions as i64)
        } else {
            None
        },
        job.user_account_id.as_deref(),
        job.near_payment_yocto.as_deref(),
        payload.actual_cost_yocto.as_deref(),
        payload.compile_cost_yocto.as_deref(),
        job.github_repo.as_deref(),
        job.github_commit.as_deref(),
        job.transaction_hash.as_deref()
    )
    .execute(&state.db)
    .await;

    if let Err(e) = history_result {
        error!("Failed to save execution history for job {}: {}", payload.job_id, e);
        // Don't fail the request, just log the error
    }

    debug!("Job {} marked as {}", payload.job_id, status);
    StatusCode::OK
}
