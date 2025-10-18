use axum::{extract::State, http::StatusCode, Json};
use tracing::{debug, error, info};

use crate::models::{ClaimJobRequest, ClaimJobResponse, CompleteJobRequest, JobInfo, JobType, CodeSource};
use crate::AppState;

/// Claim jobs for a task - coordinator decides what jobs are needed
pub async fn claim_job(
    State(state): State<AppState>,
    Json(payload): Json<ClaimJobRequest>,
) -> Result<Json<ClaimJobResponse>, StatusCode> {
    debug!(
        "Worker {} claiming task: request_id={} data_id={}",
        payload.worker_id, payload.request_id, payload.data_id
    );

    // Check if this task was already claimed by another worker
    let existing = sqlx::query!(
        "SELECT COUNT(*) as count FROM jobs WHERE request_id = $1 AND data_id = $2",
        payload.request_id as i64,
        payload.data_id
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        error!("Failed to check existing jobs: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if existing.count.unwrap_or(0) > 0 {
        debug!(
            "❌ Task already claimed by another worker: request_id={} data_id={}",
            payload.request_id, payload.data_id
        );
        let pricing = state.pricing.read().await.clone();
        return Ok(Json(ClaimJobResponse { jobs: vec![], pricing }));
    }

    // Calculate WASM checksum from code_source
    let wasm_checksum = calculate_wasm_checksum(&payload.code_source);

    // Extract GitHub repo and commit for database
    let (github_repo, github_commit) = match &payload.code_source {
        CodeSource::GitHub { repo, commit, .. } => (Some(repo.clone()), Some(commit.clone())),
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

    let mut jobs = Vec::new();

    // Create compile job if WASM not in cache
    if wasm_exists.is_none() {
        info!(
            "🔨 WASM not in cache, creating compile job for request_id={} checksum={}",
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
                    wasm_checksum: None,
                    allowed: true,
                });
            }
            Err(e) => {
                // Check if this is a duplicate key error (another worker already claimed it)
                if let Some(db_err) = e.as_database_error() {
                    if db_err.is_unique_violation() {
                        info!("⚠️ Compile job already claimed by another worker for request_id={}", payload.request_id);
                        // Return empty jobs array - another worker is already handling this
                        let pricing = state.pricing.read().await.clone();
                        return Ok(Json(ClaimJobResponse { jobs: vec![], pricing }));
                    }
                }
                // Other database errors are still internal errors
                error!("Failed to create compile job: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    } else {
        info!(
            "✅ WASM found in cache, skipping compilation for request_id={} checksum={}",
            payload.request_id, wasm_checksum
        );
    }

    // Always create execute job
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
                wasm_checksum: Some(wasm_checksum.clone()),  // Always provide checksum, even if WASM not yet compiled
                allowed: true,
            });
        }
        Err(e) => {
            // Check if this is a duplicate key error (another worker already claimed it)
            if let Some(db_err) = e.as_database_error() {
                if db_err.is_unique_violation() {
                    info!("⚠️ Execute job already claimed by another worker for request_id={}", payload.request_id);
                    // Return empty jobs array - another worker is already handling this
                    let pricing = state.pricing.read().await.clone();
                    return Ok(Json(ClaimJobResponse { jobs: vec![], pricing }));
                }
            }
            // Other database errors are still internal errors
            error!("Failed to create execute job: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    debug!(
        "✅ Task claimed: request_id={} data_id={} worker={} jobs_count={}",
        payload.request_id,
        payload.data_id,
        payload.worker_id,
        jobs.len()
    );

    // Get current pricing to send to worker
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
    }
}

/// Complete a job - worker finished the job
pub async fn complete_job(
    State(state): State<AppState>,
    Json(payload): Json<CompleteJobRequest>,
) -> StatusCode {
    debug!(
        "Completing job {}: success={} time_ms={}",
        payload.job_id, payload.success, payload.time_ms
    );

    let status = if payload.success {
        "completed"
    } else {
        "failed"
    };

    // Update job status
    let update_result = sqlx::query!(
        r#"
        UPDATE jobs
        SET status = $1,
            wasm_checksum = $2,
            completed_at = NOW(),
            updated_at = NOW()
        WHERE job_id = $3
        "#,
        status,
        payload.wasm_checksum,
        payload.job_id
    )
    .execute(&state.db)
    .await;

    if let Err(e) = update_result {
        error!("Failed to update job {}: {}", payload.job_id, e);
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    // Get job details for history
    let job = sqlx::query!(
        r#"
        SELECT request_id, data_id, job_type, worker_id, user_account_id, near_payment_yocto, github_repo, github_commit, transaction_hash
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
