use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::AppState;

/// Public endpoint: List all workers and their status
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct WorkerInfo {
    pub worker_id: String,
    pub worker_name: String,
    pub status: String,
    pub current_task_id: Option<i64>,
    pub last_heartbeat_at: String,
    pub total_tasks_completed: i64,
    pub total_tasks_failed: i64,
    pub uptime_seconds: Option<i64>,
}

pub async fn list_workers(
    State(state): State<AppState>,
) -> Result<Json<Vec<WorkerInfo>>, StatusCode> {
    let workers: Vec<WorkerInfo> = sqlx::query_as(
        r#"
        SELECT
            ws.worker_id,
            ws.worker_name,
            ws.status,
            ws.current_task_id,
            to_char(ws.last_heartbeat_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') as last_heartbeat_at,
            COALESCE(COUNT(*) FILTER (WHERE eh.success = true), 0)::BIGINT as total_tasks_completed,
            COALESCE(COUNT(*) FILTER (WHERE eh.success = false), 0)::BIGINT as total_tasks_failed,
            EXTRACT(EPOCH FROM (NOW() - ws.created_at))::BIGINT as uptime_seconds
        FROM worker_status ws
        LEFT JOIN execution_history eh ON eh.worker_id = ws.worker_id
        GROUP BY ws.worker_id, ws.worker_name, ws.status, ws.current_task_id, ws.last_heartbeat_at, ws.created_at
        ORDER BY ws.last_heartbeat_at DESC
        "#
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        error!("Failed to fetch workers: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(workers))
}

/// Public endpoint: List job history
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct JobHistoryEntry {
    pub id: i64,
    pub job_id: Option<i64>,
    pub request_id: i64,
    pub data_id: Option<String>,
    pub worker_id: String,
    pub success: bool,
    pub status: Option<String>, // actual job status from jobs table
    pub error_details: Option<String>, // detailed error message
    pub job_type: Option<String>,
    pub execution_time_ms: Option<i64>,
    pub compile_time_ms: Option<i64>,
    pub instructions_used: Option<i64>,
    pub resolve_tx_id: Option<String>,
    pub user_account_id: Option<String>,
    pub near_payment_yocto: Option<String>,
    pub actual_cost_yocto: Option<String>,
    pub compile_cost_yocto: Option<String>,
    pub github_repo: Option<String>,
    pub github_commit: Option<String>,
    pub transaction_hash: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct JobHistoryQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
    pub user_account_id: Option<String>,
}

fn default_limit() -> i64 {
    50
}

pub async fn list_jobs(
    State(state): State<AppState>,
    Query(params): Query<JobHistoryQuery>,
) -> Result<Json<Vec<JobHistoryEntry>>, StatusCode> {
    let limit = params.limit.min(100); // Max 100 per page

    let jobs: Vec<JobHistoryEntry> = if let Some(user_id) = params.user_account_id {
        sqlx::query_as(
            r#"
            SELECT
                eh.id,
                eh.job_id,
                eh.request_id,
                eh.data_id,
                eh.worker_id,
                eh.success,
                j.status,
                j.error_details,
                eh.job_type,
                eh.execution_time_ms,
                eh.compile_time_ms,
                eh.instructions_used,
                eh.resolve_tx_id,
                eh.user_account_id,
                eh.near_payment_yocto,
                eh.actual_cost_yocto,
                eh.compile_cost_yocto,
                eh.github_repo,
                eh.github_commit,
                eh.transaction_hash,
                to_char(eh.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') as created_at
            FROM execution_history eh
            LEFT JOIN jobs j ON eh.job_id = j.job_id
            WHERE eh.user_account_id = $1
            ORDER BY eh.created_at DESC
            LIMIT $2 OFFSET $3
            "#
        )
        .bind(user_id)
        .bind(limit)
        .bind(params.offset)
        .fetch_all(&state.db)
        .await
    } else {
        sqlx::query_as(
            r#"
            SELECT
                eh.id,
                eh.job_id,
                eh.request_id,
                eh.data_id,
                eh.worker_id,
                eh.success,
                j.status,
                j.error_details,
                eh.job_type,
                eh.execution_time_ms,
                eh.compile_time_ms,
                eh.instructions_used,
                eh.resolve_tx_id,
                eh.user_account_id,
                eh.near_payment_yocto,
                eh.actual_cost_yocto,
                eh.compile_cost_yocto,
                eh.github_repo,
                eh.github_commit,
                eh.transaction_hash,
                to_char(eh.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') as created_at
            FROM execution_history eh
            LEFT JOIN jobs j ON eh.job_id = j.job_id
            ORDER BY eh.created_at DESC
            LIMIT $1 OFFSET $2
            "#
        )
        .bind(limit)
        .bind(params.offset)
        .fetch_all(&state.db)
        .await
    }
    .map_err(|e| {
        error!("Failed to fetch job history: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(jobs))
}

/// Public endpoint: Get execution statistics
#[derive(Debug, Serialize)]
pub struct ExecutionStats {
    pub total_executions: i64,
    pub successful_executions: i64,
    pub failed_executions: i64,
    pub total_instructions_used: i64,
    pub average_execution_time_ms: i64,
    pub total_near_paid_yocto: String,
    pub unique_users: i64,
    pub active_workers: i64,
}

pub async fn get_stats(State(state): State<AppState>) -> Result<Json<ExecutionStats>, StatusCode> {
    #[derive(sqlx::FromRow)]
    struct StatsRow {
        total: i64,
        successful: i64,
        failed: i64,
        total_instructions: i64,
        avg_time_ms: i64,
        unique_users: i64,
    }

    // Get execution stats
    // Note: "failed" means infrastructure errors (status='failed'), not user errors
    let exec_stats: StatsRow = sqlx::query_as(
        r#"
        SELECT
            COUNT(*)::BIGINT as total,
            COUNT(*) FILTER (WHERE eh.success = true)::BIGINT as successful,
            COUNT(*) FILTER (WHERE j.status = 'failed')::BIGINT as failed,
            COALESCE(SUM(eh.instructions_used), 0)::BIGINT as total_instructions,
            COALESCE(AVG(eh.execution_time_ms), 0)::BIGINT as avg_time_ms,
            COUNT(DISTINCT eh.user_account_id)::BIGINT as unique_users
        FROM execution_history eh
        LEFT JOIN jobs j ON eh.job_id = j.job_id
        "#
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        error!("Failed to fetch execution stats: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    #[derive(sqlx::FromRow)]
    struct WorkerCount {
        count: i64,
    }

    // Get active workers count
    let active_workers: WorkerCount = sqlx::query_as(
        r#"
        SELECT COUNT(*)::BIGINT as count
        FROM worker_status
        WHERE status IN ('online', 'busy')
        AND last_heartbeat_at > NOW() - INTERVAL '5 minutes'
        "#
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        error!("Failed to fetch active workers: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    #[derive(sqlx::FromRow)]
    struct TotalNear {
        total: String,
    }

    // Calculate total NEAR paid (sum all payments - this is approximate)
    let total_near: TotalNear = sqlx::query_as(
        r#"
        SELECT
            COALESCE(SUM(CAST(near_payment_yocto AS NUMERIC)), 0)::TEXT as total
        FROM execution_history
        WHERE near_payment_yocto IS NOT NULL
        "#
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        error!("Failed to calculate total NEAR paid: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(ExecutionStats {
        total_executions: exec_stats.total,
        successful_executions: exec_stats.successful,
        failed_executions: exec_stats.failed,
        total_instructions_used: exec_stats.total_instructions,
        average_execution_time_ms: exec_stats.avg_time_ms,
        total_near_paid_yocto: total_near.total,
        unique_users: exec_stats.unique_users,
        active_workers: active_workers.count,
    }))
}

/// Public endpoint: Check if WASM exists for repo/commit/target
#[derive(Debug, Serialize)]
pub struct WasmInfoResponse {
    pub exists: bool,
    pub checksum: Option<String>,
    pub file_size: Option<i64>,
    pub created_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WasmInfoQuery {
    pub repo_url: String,
    pub commit_hash: String,
    #[serde(default = "default_build_target")]
    pub build_target: String,
}

fn default_build_target() -> String {
    "wasm32-wasip1".to_string()
}

#[derive(sqlx::FromRow)]
struct WasmRow {
    checksum: String,
    file_size: i64,
    created_at: String,
}

pub async fn get_wasm_info(
    State(state): State<AppState>,
    Query(params): Query<WasmInfoQuery>,
) -> Result<Json<WasmInfoResponse>, StatusCode> {
    let result: Option<WasmRow> = sqlx::query_as(
        r#"
        SELECT checksum, file_size, created_at::TEXT as created_at
        FROM wasm_cache
        WHERE repo_url = $1 AND commit_hash = $2 AND build_target = $3
        "#
    )
    .bind(params.repo_url)
    .bind(params.commit_hash)
    .bind(params.build_target)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        error!("Failed to check WASM cache: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match result {
        Some(row) => Ok(Json(WasmInfoResponse {
            exists: true,
            checksum: Some(row.checksum),
            file_size: Some(row.file_size),
            created_at: Some(row.created_at),
        })),
        None => Ok(Json(WasmInfoResponse {
            exists: false,
            checksum: None,
            file_size: None,
            created_at: None,
        })),
    }
}

/// Public endpoint: Get user's earnings
#[derive(Debug, Serialize)]
pub struct UserEarnings {
    pub user_account_id: String,
    pub total_executions: i64,
    pub successful_executions: i64,
    pub total_near_spent_yocto: String,
    pub total_instructions_used: i64,
    pub average_execution_time_ms: i64,
}

#[derive(sqlx::FromRow)]
struct UserStatsRow {
    total: i64,
    successful: i64,
    total_spent: String,
    total_instructions: i64,
    avg_time_ms: i64,
}

pub async fn get_user_earnings(
    State(state): State<AppState>,
    Path(user_account_id): Path<String>,
) -> Result<Json<UserEarnings>, StatusCode> {
    let stats: UserStatsRow = sqlx::query_as(
        r#"
        SELECT
            COUNT(*)::BIGINT as total,
            COUNT(*) FILTER (WHERE success = true)::BIGINT as successful,
            COALESCE(SUM(CAST(COALESCE(near_payment_yocto, '0') AS NUMERIC)), 0)::TEXT as total_spent,
            COALESCE(SUM(instructions_used), 0)::BIGINT as total_instructions,
            COALESCE(AVG(execution_time_ms), 0)::BIGINT as avg_time_ms
        FROM execution_history
        WHERE user_account_id = $1
        "#
    )
    .bind(&user_account_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        error!("Failed to fetch user earnings: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(UserEarnings {
        user_account_id,
        total_executions: stats.total,
        successful_executions: stats.successful,
        total_near_spent_yocto: stats.total_spent,
        total_instructions_used: stats.total_instructions,
        average_execution_time_ms: stats.avg_time_ms,
    }))
}

/// Public endpoint: Get popular repositories
#[derive(Debug, Serialize)]
pub struct PopularRepo {
    pub github_repo: String,
    pub total_executions: i64,
    pub successful_executions: i64,
    pub last_commit: Option<String>,
}

#[derive(sqlx::FromRow)]
struct PopularRepoRow {
    github_repo: String,
    total_executions: i64,
    successful_executions: i64,
    last_commit: Option<String>,
}

pub async fn get_popular_repos(
    State(state): State<AppState>,
) -> Result<Json<Vec<PopularRepo>>, StatusCode> {
    let repos: Vec<PopularRepoRow> = sqlx::query_as(
        r#"
        SELECT
            eh.github_repo,
            COUNT(*)::BIGINT as total_executions,
            COUNT(*) FILTER (WHERE eh.success = true)::BIGINT as successful_executions,
            (ARRAY_AGG(eh.github_commit ORDER BY eh.created_at DESC))[1] as last_commit
        FROM execution_history eh
        LEFT JOIN jobs j ON eh.job_id = j.job_id
        WHERE eh.github_repo IS NOT NULL
        GROUP BY eh.github_repo
        ORDER BY total_executions DESC
        LIMIT 10
        "#
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        error!("Failed to fetch popular repos: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let result = repos
        .into_iter()
        .map(|r| PopularRepo {
            github_repo: r.github_repo,
            total_executions: r.total_executions,
            successful_executions: r.successful_executions,
            last_commit: r.last_commit,
        })
        .collect();

    Ok(Json(result))
}
