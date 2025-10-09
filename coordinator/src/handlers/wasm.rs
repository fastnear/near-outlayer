use axum::{
    body::Bytes,
    extract::{Multipart, Path, State},
    http::StatusCode,
    Json,
};
use tracing::{debug, error, info};

use crate::models::WasmExistsResponse;
use crate::AppState;

/// Download WASM file by checksum
pub async fn get_wasm(
    State(state): State<AppState>,
    Path(checksum): Path<String>,
) -> Result<Bytes, StatusCode> {
    debug!("Fetching WASM: {}", checksum);

    let wasm_path = state
        .config
        .wasm_cache_dir
        .join(format!("{}.wasm", checksum));

    // Update last_accessed_at in database
    let _ = sqlx::query!(
        "UPDATE wasm_cache SET last_accessed_at = NOW(), access_count = access_count + 1 WHERE checksum = $1",
        checksum
    )
    .execute(&state.db)
    .await;

    // Read and return file
    let bytes = tokio::fs::read(wasm_path).await.map_err(|e| {
        error!("Failed to read WASM file {}: {}", checksum, e);
        StatusCode::NOT_FOUND
    })?;

    debug!("WASM {} sent ({} bytes)", checksum, bytes.len());
    Ok(Bytes::from(bytes))
}

/// Check if WASM exists in cache
pub async fn wasm_exists(
    State(state): State<AppState>,
    Path(checksum): Path<String>,
) -> Json<WasmExistsResponse> {
    debug!("Checking if WASM exists: {}", checksum);

    let wasm_path = state
        .config
        .wasm_cache_dir
        .join(format!("{}.wasm", checksum));

    let exists = wasm_path.exists();
    debug!("WASM {} exists: {}", checksum, exists);

    Json(WasmExistsResponse { exists })
}

/// Upload compiled WASM file
pub async fn upload_wasm(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> StatusCode {
    let mut checksum = String::new();
    let mut repo_url = String::new();
    let mut commit_hash = String::new();
    let mut wasm_bytes: Option<Vec<u8>> = None;

    // Parse multipart form data
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "checksum" => {
                checksum = field.text().await.unwrap_or_default();
            }
            "repo_url" => {
                repo_url = field.text().await.unwrap_or_default();
            }
            "commit_hash" => {
                commit_hash = field.text().await.unwrap_or_default();
            }
            "wasm_file" => {
                wasm_bytes = field.bytes().await.ok().map(|b| b.to_vec());
            }
            _ => {}
        }
    }

    let wasm_bytes = match wasm_bytes {
        Some(b) => b,
        None => {
            error!("No WASM file in upload");
            return StatusCode::BAD_REQUEST;
        }
    };

    if checksum.is_empty() || repo_url.is_empty() || commit_hash.is_empty() {
        error!("Missing required fields in upload");
        return StatusCode::BAD_REQUEST;
    }

    info!(
        "Uploading WASM: {} ({} bytes) from {}@{}",
        checksum,
        wasm_bytes.len(),
        repo_url,
        commit_hash
    );

    // Save to filesystem
    let wasm_path = state
        .config
        .wasm_cache_dir
        .join(format!("{}.wasm", checksum));

    if let Err(e) = tokio::fs::write(&wasm_path, &wasm_bytes).await {
        error!("Failed to write WASM file: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    // Insert metadata into database
    let file_size = wasm_bytes.len() as i64;
    let result = sqlx::query!(
        r#"
        INSERT INTO wasm_cache (checksum, repo_url, commit_hash, file_size, created_at, last_accessed_at)
        VALUES ($1, $2, $3, $4, NOW(), NOW())
        ON CONFLICT (checksum) DO UPDATE SET last_accessed_at = NOW()
        "#,
        checksum,
        repo_url,
        commit_hash,
        file_size
    )
    .execute(&state.db)
    .await;

    if let Err(e) = result {
        error!("Failed to insert WASM metadata: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    // Trigger LRU eviction check (non-blocking)
    let lru_eviction = state.lru_eviction.clone();
    tokio::spawn(async move {
        if let Err(e) = lru_eviction.check_and_evict().await {
            error!("LRU eviction error: {}", e);
        }
    });

    info!("WASM {} uploaded successfully", checksum);
    StatusCode::CREATED
}
