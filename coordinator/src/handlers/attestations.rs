use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};

use crate::auth::WorkerTokenHash;
use crate::models::{AttestationResponse, StoreAttestationRequest, TaskAttestation};
use crate::AppState;

/// Public endpoint: Get attestation by task ID
///
/// Requires valid API key in X-API-Key header
pub async fn get_attestation(
    Path(task_id): Path<i64>,
    State(state): State<AppState>,
) -> Result<Json<AttestationResponse>, StatusCode> {
    let attestation = sqlx::query_as!(
        TaskAttestation,
        "SELECT * FROM task_attestations WHERE task_id = $1",
        task_id
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch attestation for task {}: {}", task_id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or_else(|| {
        tracing::debug!("Attestation not found for task {}", task_id);
        StatusCode::NOT_FOUND
    })?;

    Ok(Json(attestation.into()))
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

    // Store in database
    // Note: ON CONFLICT removed because task_attestations doesn't have UNIQUE(task_id)
    // Multiple attestations can exist for the same task (e.g., retries, different workers)
    sqlx::query!(
        "INSERT INTO task_attestations
         (task_id, task_type, tdx_quote, worker_measurement,
          request_id, caller_account_id, transaction_hash, block_height,
          repo_url, commit_hash, build_target,
          wasm_hash, input_hash, output_hash)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)",
        req.task_id,
        req.task_type.as_str(),
        quote_bytes,
        worker_measurement,
        req.request_id,
        req.caller_account_id,
        req.transaction_hash,
        req.block_height.map(|h| h as i64),
        req.repo_url,
        req.commit_hash,
        req.build_target,
        req.wasm_hash,
        req.input_hash,
        req.output_hash
    )
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
        let result = sqlx::query!(
            "UPDATE worker_auth_tokens
             SET last_seen_rtmr3 = $1, last_attestation_at = NOW()
             WHERE token_hash = $2",
            rtmr3,
            token_hash
        )
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
    // TDX quote v4 structure (same as Phala dstack):
    // - Header: 48 bytes
    // - Body (TDReport):
    //   - RTMR0: offset 112 (48 bytes)
    //   - RTMR1: offset 160 (48 bytes)
    //   - RTMR2: offset 208 (48 bytes)
    //   - RTMR3: offset 256 (48 bytes)  ← we need this
    //
    // Note: We use raw offset extraction instead of dcap-qvl::quote::Quote::parse()
    // because Phala dstack returns quote v4 which dcap-qvl doesn't support yet.
    // Register-contract uses verify::verify() which handles this internally.

    const RTMR3_OFFSET: usize = 256;
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
    Ok(hex::encode(rtmr3_bytes))
}
