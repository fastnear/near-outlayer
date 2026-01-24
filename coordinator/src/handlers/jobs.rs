use axum::{extract::State, http::StatusCode, Json};
use redis::AsyncCommands;
use tracing::{debug, error, info, warn};

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

    // Extract code source fields for database
    let (github_repo, github_commit, wasm_url, wasm_content_hash, build_target) = match &payload.code_source {
        CodeSource::GitHub { repo, commit, build_target } => (
            Some(repo.clone()),
            Some(commit.clone()),
            None,
            None,
            Some(build_target.clone())
        ),
        CodeSource::WasmUrl { url, hash, build_target } => (
            None,
            None,
            Some(url.clone()),
            Some(hash.clone()),
            Some(build_target.clone())
        ),
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

    // Handle force_rebuild - for COMPILER: treat as if WASM doesn't exist
    // For EXECUTOR: use real wasm_file_exists (after compilation, WASM exists)
    let needs_compilation = if payload.force_rebuild {
        info!("🔄 force_rebuild=true, compiler will recompile");
        true
    } else {
        !wasm_file_exists
    };

    // Use has_compile_result from payload (passed by worker from ExecutionRequest)
    let has_compile_result = payload.has_compile_result;

    let mut jobs = Vec::new();

    // Determine job type based on WASM availability and worker capabilities
    if needs_compilation && can_compile && !has_compile_result {
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
            INSERT INTO jobs (request_id, data_id, job_type, worker_id, status, wasm_checksum, user_account_id, near_payment_yocto, github_repo, github_commit, transaction_hash, wasm_url, wasm_content_hash, build_target, created_at, updated_at)
            VALUES ($1, $2, 'compile', $3, 'in_progress', $4, $5, $6, $7, $8, $9, $10, $11, $12, NOW(), NOW())
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
            payload.transaction_hash.as_deref(),
            wasm_url.as_deref(),
            wasm_content_hash.as_deref(),
            build_target.as_deref()
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
                    compile_time_ms: None, // Will be set after compilation
                    project_uuid: payload.project_uuid.clone(),
                    project_id: payload.project_id.clone(),
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
    } else if can_execute && (!payload.compile_only || has_compile_result) {
        // Executor claiming job - either WASM exists, compilation failed, or has compile_result to send
        // Allow if compile_only=true but compile_result exists (need to send result to contract)

        // Check if compile job failed (executor needs to report error to contract)
        let compile_job = sqlx::query!(
            "SELECT compile_cost_yocto, compile_error, status, compile_time_ms FROM jobs WHERE request_id = $1 AND data_id = $2 AND job_type = 'compile'",
            payload.request_id as i64,
            payload.data_id
        )
        .fetch_optional(&state.db)
        .await
        .map_err(|e| {
            error!("Failed to fetch compile job: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        let (compile_cost_yocto, compile_error, compile_status, compile_time_ms) = compile_job
            .as_ref()
            .map(|j| (j.compile_cost_yocto.clone(), j.compile_error.clone(), Some(j.status.clone()), j.compile_time_ms.map(|t| t as u64)))
            .unwrap_or((None, None, None, None));

        // If WASM doesn't exist, no compile error, and no compile_result, executor can't do anything
        // Note: use wasm_file_exists (real state), not needs_compilation (force_rebuild affects only compiler)
        // Exception: WasmUrl sources - trigger on-demand download via compile queue
        if !wasm_file_exists && compile_error.is_none() && !has_compile_result {
            // Check if this is a WasmUrl source - can trigger on-demand download
            if matches!(payload.code_source, CodeSource::WasmUrl { .. }) {
                // Check if compile job already exists for downloading (another executor already triggered)
                let existing_download = sqlx::query!(
                    "SELECT job_id FROM jobs WHERE request_id = $1 AND data_id = $2 AND job_type = 'compile'",
                    payload.request_id as i64,
                    payload.data_id
                )
                .fetch_optional(&state.db)
                .await
                .map_err(|e| {
                    error!("Failed to check existing download job: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;

                if existing_download.is_none() {
                    info!(
                        "📥 WasmUrl not in cache - pushing to compile queue for download: request_id={} data_id={}",
                        payload.request_id, payload.data_id
                    );

                    // Get execution request and push to compile queue
                    // For HTTPS calls (request_id=0): use Redis cache
                    // For blockchain calls: use execution_requests table
                    let is_https_call = payload.request_id == 0;

                    if let Ok(mut conn) = state.redis.get_multiplexed_async_connection().await {
                        let request_json: Option<String> = if is_https_call {
                            // HTTPS call - get from Redis cache
                            let request_key = format!("https_request:{}", payload.data_id);
                            conn.get(&request_key).await.ok().flatten()
                        } else {
                            // Blockchain call - reconstruct from execution_requests table
                            // This is same logic as complete_job uses
                            match reconstruct_execution_request_json(&state.db, payload.request_id, &payload.data_id, &payload.code_source).await {
                                Ok(json) => Some(json),
                                Err(e) => {
                                    warn!("⚠️ Failed to reconstruct execution request: {}", e);
                                    None
                                }
                            }
                        };

                        if let Some(json) = request_json {
                            let compile_queue = &state.config.redis_queue_compile;
                            let push_result: Result<(), _> = conn.lpush(compile_queue, &json).await;
                            if push_result.is_ok() {
                                info!("📤 Pushed task to compile queue for WasmUrl download: {}", compile_queue);
                            } else {
                                error!("Failed to push to compile queue");
                            }
                        } else {
                            warn!("⚠️ No execution request found for request_id={} data_id={}", payload.request_id, payload.data_id);
                        }
                    }
                } else {
                    debug!("⏳ Compile job already exists for WasmUrl download, waiting...");
                }

                // Return empty jobs - task will come back via execute queue after download
                let pricing = state.pricing.read().await.clone();
                return Ok(Json(ClaimJobResponse { jobs: vec![], pricing }));
            }

            debug!(
                "❌ WASM not available and no compile error/result for request_id={}, executor cannot proceed",
                payload.request_id
            );
            let pricing = state.pricing.read().await.clone();
            return Ok(Json(ClaimJobResponse { jobs: vec![], pricing }));
        }

        // If force_rebuild=true, executor must wait for compile job to complete
        // Otherwise it will use old cached WASM instead of freshly compiled one
        // Only wait if compile job exists (compile_job.is_some()) - otherwise compiler hasn't picked it up yet
        if payload.force_rebuild && compile_job.is_some() && compile_status.as_deref() != Some("completed") && compile_error.is_none() {
            debug!(
                "⏳ force_rebuild=true but compile job not completed yet (status={:?}) for request_id={}, executor waiting",
                compile_status, payload.request_id
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
            INSERT INTO jobs (request_id, data_id, job_type, worker_id, status, wasm_checksum, user_account_id, near_payment_yocto, github_repo, github_commit, transaction_hash, wasm_url, wasm_content_hash, build_target, created_at, updated_at)
            VALUES ($1, $2, 'execute', $3, 'in_progress', $4, $5, $6, $7, $8, $9, $10, $11, $12, NOW(), NOW())
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
            payload.transaction_hash.as_deref(),
            wasm_url.as_deref(),
            wasm_content_hash.as_deref(),
            build_target.as_deref()
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
                    compile_time_ms,
                    project_uuid: payload.project_uuid.clone(),
                    project_id: payload.project_id.clone(),
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
        SELECT request_id, data_id, job_type, worker_id, wasm_checksum, user_account_id, near_payment_yocto, github_repo, github_commit, transaction_hash, wasm_url, wasm_content_hash, build_target
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
        let build_target = job.build_target.clone().unwrap_or_else(|| "wasm32-wasip1".to_string());
        let code_source = if let (Some(repo), Some(commit)) = (&job.github_repo, &job.github_commit) {
            CodeSource::GitHub {
                repo: repo.clone(),
                commit: commit.clone(),
                build_target,
            }
        } else if let (Some(url), Some(hash)) = (&job.wasm_url, &job.wasm_content_hash) {
            CodeSource::WasmUrl {
                url: url.clone(),
                hash: hash.clone(),
                build_target,
            }
        } else {
            error!("Missing code source fields for compile job {}", payload.job_id);
            return StatusCode::INTERNAL_SERVER_ERROR;
        };

        // Create execution request for execute queue
        // Note: We're missing some fields (input_data, resource_limits, secrets_ref, context)
        // These should be stored when the original task is created
        // For now, fetch from a pending execute task or store in a separate table

        // Try to get the original execution request from pending execute jobs or a task table
        use sqlx::Row;
        let original_request = sqlx::query(
            r#"
            SELECT input_data, max_instructions, max_memory_mb, max_execution_seconds,
                   secrets_profile, secrets_account_id, response_format,
                   context_sender_id, context_block_height, context_block_timestamp,
                   context_contract_id, context_transaction_hash, context_receipt_id,
                   context_predecessor_id, context_signer_public_key, context_gas_burnt,
                   compile_only, force_rebuild, store_on_fastfs, project_uuid, project_id,
                   attached_usd
            FROM execution_requests
            WHERE request_id = $1
            "#
        )
        .bind(job.request_id)
        .fetch_optional(&state.db)
        .await;

        let execution_request = match original_request {
            Ok(Some(row)) => {
                let project_uuid: Option<String> = row.get("project_uuid");
                let project_id: Option<String> = row.get("project_id");
                let compile_only: bool = row.get("compile_only");
                let attached_usd: Option<String> = row.get("attached_usd");

                info!("📋 Fetched execution_requests for request_id={}: project_uuid={:?} project_id={:?}",
                    job.request_id, project_uuid, project_id);
                // Check if this was compile-only request
                if compile_only {
                    // If compile_result is provided, create execute task for executor to send result to contract
                    if let Some(ref compile_result) = payload.compile_result {
                        info!(
                            "📤 Compile-only request with result for request_id={}, creating execute task to send result: {}",
                            job.request_id, compile_result
                        );
                        // Save compile_result to execution_requests for executor to pick up
                        if let Err(e) = sqlx::query(
                            "UPDATE execution_requests SET compile_result = $1 WHERE request_id = $2"
                        )
                        .bind(compile_result)
                        .bind(job.request_id)
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

                let max_instructions: Option<i64> = row.get("max_instructions");
                let max_memory_mb: Option<i32> = row.get("max_memory_mb");
                let max_execution_seconds: Option<i64> = row.get("max_execution_seconds");
                let input_data: Option<String> = row.get("input_data");
                let secrets_profile: Option<String> = row.get("secrets_profile");
                let secrets_account_id: Option<String> = row.get("secrets_account_id");
                let response_format: Option<String> = row.get("response_format");
                let context_sender_id: Option<String> = row.get("context_sender_id");
                let context_block_height: Option<i64> = row.get("context_block_height");
                let context_block_timestamp: Option<i64> = row.get("context_block_timestamp");
                let context_contract_id: Option<String> = row.get("context_contract_id");
                let context_transaction_hash: Option<String> = row.get("context_transaction_hash");
                let context_receipt_id: Option<String> = row.get("context_receipt_id");
                let context_predecessor_id: Option<String> = row.get("context_predecessor_id");
                let context_signer_public_key: Option<String> = row.get("context_signer_public_key");
                let context_gas_burnt: Option<i64> = row.get("context_gas_burnt");
                let force_rebuild: bool = row.get("force_rebuild");
                let store_on_fastfs: bool = row.get("store_on_fastfs");

                ExecutionRequest {
                    request_id: job.request_id as u64,
                    data_id: job.data_id.clone(),
                    code_source: Some(code_source),
                    resource_limits: ResourceLimits {
                        max_instructions: max_instructions.unwrap_or(1_000_000_000) as u64,
                        max_memory_mb: max_memory_mb.unwrap_or(128) as u32,
                        max_execution_seconds: max_execution_seconds.unwrap_or(60) as u64,
                    },
                    input_data: input_data.unwrap_or_default(),
                    secrets_ref: if let (Some(profile), Some(account_id)) = (secrets_profile.clone(), secrets_account_id.clone()) {
                        Some(SecretsReference { profile, account_id })
                    } else {
                        None
                    },
                    response_format: match response_format.as_deref() {
                        Some("bytes") => ResponseFormat::Bytes,
                        Some("json") => ResponseFormat::Json,
                        _ => ResponseFormat::Text,
                    },
                    context: ExecutionContext {
                        sender_id: context_sender_id,
                        block_height: context_block_height.map(|h| h as u64),
                        block_timestamp: context_block_timestamp.map(|t| t as u64),
                        contract_id: context_contract_id,
                        transaction_hash: context_transaction_hash,
                        receipt_id: context_receipt_id,
                        predecessor_id: context_predecessor_id,
                        signer_public_key: context_signer_public_key,
                        gas_burnt: context_gas_burnt.map(|g| g as u64),
                    },
                    user_account_id: job.user_account_id.clone(),
                    near_payment_yocto: job.near_payment_yocto.clone(),
                    attached_usd,
                    transaction_hash: job.transaction_hash.clone(),
                    compile_only,
                    force_rebuild,
                    store_on_fastfs,
                    compile_result: payload.compile_result.clone(),
                    project_uuid,
                    project_id,
                    // HTTPS API fields - not used for NEAR contract calls
                    is_https_call: false,
                    call_id: None,
                    payment_key_owner: None,
                    payment_key_nonce: None,
                    usd_payment: None,
                    compute_limit_usd: None,
                    attached_deposit_usd: None,
                }
            }
            Ok(None) => {
                // For HTTPS calls (request_id=0), try to get from Redis cache
                if job.request_id == 0 {
                    let request_key = format!("https_request:{}", job.data_id);
                    let mut found_request: Option<ExecutionRequest> = None;

                    if let Ok(mut conn) = state.redis.get_multiplexed_async_connection().await {
                        let cached: Result<Option<String>, _> = conn.get(&request_key).await;
                        if let Ok(Some(json)) = cached {
                            if let Ok(req) = serde_json::from_str::<ExecutionRequest>(&json) {
                                info!("📋 Fetched HTTPS execution request from Redis for data_id={}: project_uuid={:?} project_id={:?}",
                                    job.data_id, req.project_uuid, req.project_id);
                                // Return request with updated code_source (compiler may have updated it)
                                found_request = Some(ExecutionRequest {
                                    code_source: Some(code_source.clone()),
                                    compile_result: payload.compile_result.clone(),
                                    ..req
                                });
                            } else {
                                warn!("⚠️ Failed to parse HTTPS execution request from Redis for data_id={}", job.data_id);
                            }
                        } else {
                            warn!("⚠️ No HTTPS execution request in Redis for data_id={}", job.data_id);
                        }
                    } else {
                        warn!("⚠️ Failed to connect to Redis for HTTPS execution request");
                    }

                    found_request.unwrap_or_else(|| {
                        warn!("⚠️ Using defaults for HTTPS request (project_uuid=None)");
                        ExecutionRequest {
                            request_id: job.request_id as u64,
                            data_id: job.data_id.clone(),
                            code_source: Some(code_source.clone()),
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
                            attached_usd: None,
                            transaction_hash: job.transaction_hash.clone(),
                            compile_only: false,
                            force_rebuild: false,
                            store_on_fastfs: false,
                            compile_result: payload.compile_result.clone(),
                            project_uuid: None,
                            project_id: None,
                            is_https_call: true,
                            call_id: None,
                            payment_key_owner: None,
                            payment_key_nonce: None,
                            usd_payment: None,
                            compute_limit_usd: None,
                            attached_deposit_usd: None,
                        }
                    })
                } else {
                    // Fallback: create minimal request with defaults for blockchain calls
                    warn!("⚠️ No execution_requests record for request_id={}, using defaults (project_uuid=None, project_id=None)", job.request_id);
                    ExecutionRequest {
                        request_id: job.request_id as u64,
                        data_id: job.data_id.clone(),
                        code_source: Some(code_source),
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
                        attached_usd: None,
                        transaction_hash: job.transaction_hash.clone(),
                        compile_only: false,
                        force_rebuild: false,
                        store_on_fastfs: false,
                        compile_result: payload.compile_result.clone(),
                        project_uuid: None,
                        project_id: None,
                        is_https_call: false,
                        call_id: None,
                        payment_key_owner: None,
                        payment_key_nonce: None,
                        usd_payment: None,
                        compute_limit_usd: None,
                        attached_deposit_usd: None,
                    }
                }
            }
            Err(e) => {
                error!("Failed to fetch original execution request for request_id={}: {} (project_uuid=None, project_id=None)", job.request_id, e);
                // Continue with defaults rather than failing
                ExecutionRequest {
                    request_id: job.request_id as u64,
                    data_id: job.data_id.clone(),
                    code_source: Some(code_source),
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
                    attached_usd: None,
                    transaction_hash: job.transaction_hash.clone(),
                    compile_only: false,
                    force_rebuild: false,
                    store_on_fastfs: false,
                    compile_result: payload.compile_result.clone(),
                    project_uuid: None,
                    project_id: None,
                    is_https_call: false,
                    call_id: None,
                    payment_key_owner: None,
                    payment_key_nonce: None,
                    usd_payment: None,
                    compute_limit_usd: None,
                    attached_deposit_usd: None,
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
        debug!("📤 Execute task JSON: {}", request_json);

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
        let build_target = job.build_target.clone().unwrap_or_else(|| "wasm32-wasip1".to_string());
        let code_source = if let (Some(repo), Some(commit)) = (&job.github_repo, &job.github_commit) {
            CodeSource::GitHub {
                repo: repo.clone(),
                commit: commit.clone(),
                build_target,
            }
        } else if let (Some(url), Some(hash)) = (&job.wasm_url, &job.wasm_content_hash) {
            CodeSource::WasmUrl {
                url: url.clone(),
                hash: hash.clone(),
                build_target,
            }
        } else {
            error!("Missing code source fields for failed compile job {}", payload.job_id);
            return StatusCode::INTERNAL_SERVER_ERROR;
        };

        // Fetch project_uuid from execution_requests for storage support
        let project_uuid = sqlx::query_scalar!(
            "SELECT project_uuid FROM execution_requests WHERE request_id = $1",
            job.request_id
        )
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .flatten();

        // Create minimal execution request - executor will see compile_error and just report it
        let execution_request = ExecutionRequest {
            request_id: job.request_id as u64,
            data_id: job.data_id.clone(),
            code_source: Some(code_source),
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
            attached_usd: None,
            transaction_hash: job.transaction_hash.clone(),
            compile_only: false,
            force_rebuild: false,
            store_on_fastfs: false,
            compile_result: None,
            project_uuid,
            project_id: None, // Not needed for failed compile reporting
            // HTTPS API fields - not used for NEAR contract calls
            is_https_call: false,
            call_id: None,
            payment_key_owner: None,
            payment_key_nonce: None,
            usd_payment: None,
            compute_limit_usd: None,
            attached_deposit_usd: None,
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

    // Log developer earnings to earnings_history for blockchain calls
    // (only for successful execute jobs with attached_usd > 0)
    if job.job_type == "execute" && payload.success {
        // Fetch attached_usd and project_id from execution_requests
        if let Ok(Some(req_data)) = sqlx::query!(
            "SELECT attached_usd, project_id, context_sender_id FROM execution_requests WHERE request_id = $1",
            job.request_id
        )
        .fetch_optional(&state.db)
        .await
        {
            let attached_usd_str = req_data.attached_usd.unwrap_or_default();
            let attached_usd: i64 = attached_usd_str.parse().unwrap_or(0);

            if attached_usd > 0 {
                if let Some(ref project_id) = req_data.project_id {
                    // Extract project owner from project_id (format: "owner.near/project-name")
                    let project_owner = project_id.split('/').next().unwrap_or(project_id.as_str());

                    // Parse refund_usd from payload
                    let refund_usd: i64 = payload.refund_usd
                        .as_ref()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                    let developer_amount = attached_usd - refund_usd;

                    // Insert into earnings_history (blockchain calls only log history, balance is in contract)
                    let attached_usd_bd = sqlx::types::BigDecimal::from(attached_usd);
                    let refund_usd_bd = sqlx::types::BigDecimal::from(refund_usd);
                    let developer_amount_bd = sqlx::types::BigDecimal::from(developer_amount);

                    if let Err(e) = sqlx::query!(
                        r#"
                        INSERT INTO earnings_history
                        (project_owner, project_id, attached_usd, refund_usd, amount, source, tx_hash, caller, request_id)
                        VALUES ($1, $2, $3, $4, $5, 'blockchain', $6, $7, $8)
                        "#,
                        project_owner,
                        project_id,
                        attached_usd_bd,
                        refund_usd_bd,
                        developer_amount_bd,
                        job.transaction_hash.as_deref(),
                        req_data.context_sender_id.as_deref(),
                        job.request_id
                    )
                    .execute(&state.db)
                    .await
                    {
                        error!("Failed to log earnings_history for request_id={}: {}", job.request_id, e);
                    } else {
                        info!(
                            "💰 Logged blockchain earnings: project_owner={}, amount={} (attached={}, refund={})",
                            project_owner, developer_amount, attached_usd, refund_usd
                        );
                    }
                }
            }
        }
    }

    debug!("Job {} marked as {}", payload.job_id, status);
    StatusCode::OK
}

/// Reconstruct ExecutionRequest JSON from execution_requests table
/// Used for blockchain calls with WasmUrl that need to be pushed to compile queue
async fn reconstruct_execution_request_json(
    db: &sqlx::PgPool,
    request_id: u64,
    data_id: &str,
    code_source: &CodeSource,
) -> Result<String, String> {
    use sqlx::Row;

    let row = sqlx::query(
        r#"
        SELECT input_data, max_instructions, max_memory_mb, max_execution_seconds,
               secrets_profile, secrets_account_id, response_format,
               context_sender_id, context_block_height, context_block_timestamp,
               context_contract_id, context_transaction_hash, context_receipt_id,
               context_predecessor_id, context_signer_public_key, context_gas_burnt,
               compile_only, force_rebuild, store_on_fastfs, project_uuid, project_id,
               attached_usd
        FROM execution_requests WHERE request_id = $1
        "#
    )
    .bind(request_id as i64)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or_else(|| format!("No execution_requests record for request_id={}", request_id))?;

    let code_source_json = match code_source {
        CodeSource::GitHub { repo, commit, build_target } => serde_json::json!({
            "GitHub": {
                "repo": repo,
                "commit": commit,
                "build_target": build_target
            }
        }),
        CodeSource::WasmUrl { url, hash, build_target } => serde_json::json!({
            "WasmUrl": {
                "url": url,
                "hash": hash,
                "build_target": build_target
            }
        }),
    };

    let input_data: Option<String> = row.get("input_data");
    let max_instructions: Option<i64> = row.get("max_instructions");
    let max_memory_mb: Option<i32> = row.get("max_memory_mb");
    let max_execution_seconds: Option<i64> = row.get("max_execution_seconds");
    let secrets_profile: Option<String> = row.get("secrets_profile");
    let secrets_account_id: Option<String> = row.get("secrets_account_id");
    let response_format: Option<String> = row.get("response_format");
    let context_sender_id: Option<String> = row.get("context_sender_id");
    let context_block_height: Option<i64> = row.get("context_block_height");
    let context_block_timestamp: Option<i64> = row.get("context_block_timestamp");
    let context_contract_id: Option<String> = row.get("context_contract_id");
    let context_transaction_hash: Option<String> = row.get("context_transaction_hash");
    let context_receipt_id: Option<String> = row.get("context_receipt_id");
    let context_predecessor_id: Option<String> = row.get("context_predecessor_id");
    let context_signer_public_key: Option<String> = row.get("context_signer_public_key");
    let context_gas_burnt: Option<i64> = row.get("context_gas_burnt");
    let compile_only: bool = row.get("compile_only");
    let force_rebuild: bool = row.get("force_rebuild");
    let store_on_fastfs: bool = row.get("store_on_fastfs");
    let project_uuid: Option<String> = row.get("project_uuid");
    let project_id: Option<String> = row.get("project_id");
    let attached_usd: Option<String> = row.get("attached_usd");

    let execution_request = serde_json::json!({
        "request_id": request_id,
        "data_id": data_id,
        "code_source": code_source_json,
        "resource_limits": {
            "max_instructions": max_instructions.unwrap_or(1_000_000_000),
            "max_memory_mb": max_memory_mb.unwrap_or(128),
            "max_execution_seconds": max_execution_seconds.unwrap_or(60)
        },
        "input_data": input_data.unwrap_or_default(),
        "secrets_ref": if secrets_profile.is_some() && secrets_account_id.is_some() {
            serde_json::json!({
                "profile": secrets_profile,
                "account_id": secrets_account_id
            })
        } else {
            serde_json::Value::Null
        },
        "response_format": response_format.unwrap_or_else(|| "text".to_string()),
        "context": {
            "sender_id": context_sender_id,
            "block_height": context_block_height,
            "block_timestamp": context_block_timestamp,
            "contract_id": context_contract_id,
            "transaction_hash": context_transaction_hash,
            "receipt_id": context_receipt_id,
            "predecessor_id": context_predecessor_id,
            "signer_public_key": context_signer_public_key,
            "gas_burnt": context_gas_burnt
        },
        "compile_only": compile_only,
        "force_rebuild": force_rebuild,
        "store_on_fastfs": store_on_fastfs,
        "project_uuid": project_uuid,
        "project_id": project_id,
        "attached_usd": attached_usd
    });

    serde_json::to_string(&execution_request)
        .map_err(|e| format!("JSON serialize error: {}", e))
}
