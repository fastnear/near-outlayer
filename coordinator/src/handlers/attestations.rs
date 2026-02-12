use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use sqlx::Row;

use crate::auth::WorkerTokenHash;
#[allow(unused_imports)]
use crate::models::{AttestationResponse, StoreAttestationRequest, TaskAttestation};
use crate::AppState;

/// Public endpoint: Get attestation by job ID (task ID)
pub async fn get_attestation(
    Path(job_id): Path<i64>,
    State(state): State<AppState>,
) -> Result<Json<AttestationResponse>, StatusCode> {
    let row = sqlx::query(
        r#"SELECT id, task_id, task_type, tdx_quote, worker_measurement,
                  request_id, caller_account_id, transaction_hash, block_height,
                  call_id, payment_key_owner, payment_key_nonce,
                  repo_url, commit_hash, build_target,
                  wasm_hash, input_hash, output_hash,
                  project_id, secrets_ref, attached_usd,
                  created_at
           FROM task_attestations WHERE task_id = $1"#
    )
    .bind(job_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch attestation for job {}: {}", job_id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or_else(|| {
        tracing::debug!("Attestation not found for job {}", job_id);
        StatusCode::NOT_FOUND
    })?;

    let attestation = row_to_attestation(&row);
    Ok(Json(attestation.into()))
}

/// Convert database row to TaskAttestation
fn row_to_attestation(row: &sqlx::postgres::PgRow) -> TaskAttestation {
    TaskAttestation {
        id: row.get("id"),
        task_id: row.get("task_id"),
        task_type: row.get("task_type"),
        tdx_quote: row.get("tdx_quote"),
        worker_measurement: row.get("worker_measurement"),
        request_id: row.get("request_id"),
        caller_account_id: row.get("caller_account_id"),
        transaction_hash: row.get("transaction_hash"),
        block_height: row.get("block_height"),
        call_id: row.get("call_id"),
        payment_key_owner: row.get("payment_key_owner"),
        payment_key_nonce: row.get("payment_key_nonce"),
        repo_url: row.get("repo_url"),
        commit_hash: row.get("commit_hash"),
        build_target: row.get("build_target"),
        wasm_hash: row.get("wasm_hash"),
        input_hash: row.get("input_hash"),
        output_hash: row.get("output_hash"),
        // V1 fields
        project_id: row.get("project_id"),
        secrets_ref: row.get("secrets_ref"),
        attached_usd: row.get("attached_usd"),
        created_at: row.get("created_at"),
    }
}

/// Protected endpoint: Store attestation (from worker)
///
/// Requires worker auth token
pub async fn store_attestation(
    State(state): State<AppState>,
    Extension(worker_token): Extension<WorkerTokenHash>,
    Json(req): Json<StoreAttestationRequest>,
) -> Result<StatusCode, StatusCode> {
    // Validate request
    req.validate().map_err(|e| {
        tracing::warn!("Invalid attestation request: {}", e);
        StatusCode::BAD_REQUEST
    })?;

    // Decode TDX quote from base64
    use base64::Engine;
    let quote_bytes = base64::engine::general_purpose::STANDARD
        .decode(&req.tdx_quote)
        .map_err(|_| {
            tracing::warn!("Invalid base64 in tdx_quote");
            StatusCode::BAD_REQUEST
        })?;

    // Parse TDX quote and extract RTMR3 measurement (production implementation)
    let worker_measurement = parse_tdx_quote_rtmr3(&quote_bytes).map_err(|e| {
        tracing::warn!("Failed to parse TDX quote: {}", e);
        StatusCode::BAD_REQUEST
    })?;

    tracing::debug!(
        "Storing attestation for task {} (type: {})",
        req.task_id,
        req.task_type.as_str()
    );

    // Parse call_id to UUID if present
    let call_id_uuid = req.call_id.as_ref().and_then(|id| {
        uuid::Uuid::parse_str(id).ok()
    });

    // Store in database
    // Note: ON CONFLICT removed because task_attestations doesn't have UNIQUE(task_id)
    // Multiple attestations can exist for the same task (e.g., retries, different workers)
    // If timestamp is provided, use it; otherwise use NOW() as default
    sqlx::query(
        "INSERT INTO task_attestations
         (task_id, task_type, tdx_quote, worker_measurement,
          request_id, caller_account_id, transaction_hash, block_height,
          call_id, payment_key_owner, payment_key_nonce,
          repo_url, commit_hash, build_target,
          wasm_hash, input_hash, output_hash,
          project_id, secrets_ref, attached_usd, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
                 COALESCE(to_timestamp($21), NOW()))"
    )
    .bind(req.task_id)
    .bind(req.task_type.as_str())
    .bind(&quote_bytes)
    .bind(&worker_measurement)
    .bind(req.request_id)
    .bind(&req.caller_account_id)
    .bind(&req.transaction_hash)
    .bind(req.block_height.map(|h| h as i64))
    .bind(call_id_uuid)
    .bind(&req.payment_key_owner)
    .bind(req.payment_key_nonce)
    .bind(&req.repo_url)
    .bind(&req.commit_hash)
    .bind(&req.build_target)
    .bind(&req.wasm_hash)
    .bind(&req.input_hash)
    .bind(&req.output_hash)
    .bind(&req.project_id)
    .bind(&req.secrets_ref)
    .bind(&req.attached_usd)
    .bind(req.timestamp.map(|t| t as f64))
    .execute(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to store attestation: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    tracing::info!(
        "Stored attestation for task {} (type: {}, request_id: {:?})",
        req.task_id,
        req.task_type.as_str(),
        req.request_id
    );

    // Update worker's last seen RTMR3 and attestation timestamp (fire and forget)
    let db = state.db.clone();
    let token_hash = worker_token.0.clone();
    let rtmr3 = worker_measurement.clone();
    tokio::spawn(async move {
        let result = sqlx::query(
            "UPDATE worker_auth_tokens
             SET last_seen_rtmr3 = $1, last_attestation_at = NOW()
             WHERE token_hash = $2"
        )
        .bind(&rtmr3)
        .bind(&token_hash)
        .execute(&db)
        .await;

        if let Err(e) = result {
            tracing::warn!("Failed to update worker RTMR3 tracking: {}", e);
        } else {
            tracing::debug!("Updated worker RTMR3: {}", rtmr3);
        }
    });

    Ok(StatusCode::CREATED)
}

/// Parse TDX quote and extract RTMR3 measurement
///
/// Returns RTMR3 as 96 hex characters (48 bytes)
fn parse_tdx_quote_rtmr3(quote_bytes: &[u8]) -> Result<String, String> {
    // Extract RTMR3 from raw TDX quote bytes at fixed offset
    // TDX Quote v4 = Header (48 bytes) + TD10 Report Body (584 bytes) + Auth Data
    // RTMR3 body offset: 472, absolute offset: 48 + 472 = 520

    const RTMR3_OFFSET: usize = 520;
    const RTMR3_SIZE: usize = 48;

    if quote_bytes.len() < RTMR3_OFFSET + RTMR3_SIZE {
        return Err(format!(
            "TDX quote too short: {} bytes (need at least {})",
            quote_bytes.len(),
            RTMR3_OFFSET + RTMR3_SIZE
        ));
    }

    // Extract RTMR3 bytes
    let rtmr3_bytes = &quote_bytes[RTMR3_OFFSET..RTMR3_OFFSET + RTMR3_SIZE];

    // Convert to 96 hex characters for storage and comparison
    let rtmr3_hex = hex::encode(rtmr3_bytes);

    tracing::info!(
        "📏 Extracted RTMR3 from TDX quote (offset {}, {} bytes): {}",
        RTMR3_OFFSET,
        RTMR3_SIZE,
        rtmr3_hex
    );

    Ok(rtmr3_hex)
}
