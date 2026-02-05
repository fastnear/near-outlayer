use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};

use crate::AppState;

/// Shared HTTP client for keystore proxy requests (connection pooling + timeout).
fn proxy_client() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("failed to build keystore proxy client")
    })
}

/// Proxy TEE challenge request to keystore
///
/// Worker calls: POST /keystore/tee-challenge (with coordinator auth)
/// Coordinator forwards to: POST {keystore_url}/tee-challenge (with keystore auth token)
pub async fn tee_challenge(
    State(state): State<AppState>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let keystore_url = state.config.keystore_base_url.as_ref().ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Keystore is not configured"})),
        )
    })?;

    let client = proxy_client();
    let mut request_builder = client.post(format!("{}/tee-challenge", keystore_url));

    if let Some(ref token) = state.config.keystore_auth_token {
        request_builder = request_builder.header("Authorization", format!("Bearer {}", token));
    }

    let keystore_response = request_builder.send().await.map_err(|e| {
        tracing::error!("Failed to proxy TEE challenge to keystore: {}", e);
        (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({"error": format!("Keystore unreachable: {}", e)})),
        )
    })?;

    let status = keystore_response.status();
    let body: serde_json::Value = keystore_response.json().await.map_err(|e| {
        tracing::error!("Failed to parse keystore TEE challenge response: {}", e);
        (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({"error": "Invalid keystore response"})),
        )
    })?;

    let axum_status =
        StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    Ok((axum_status, Json(body)).into_response())
}

/// Proxy TEE registration request to keystore
///
/// Worker calls: POST /keystore/register-tee (with coordinator auth)
/// Coordinator forwards to: POST {keystore_url}/register-tee (with keystore auth token)
pub async fn register_tee(
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let keystore_url = state.config.keystore_base_url.as_ref().ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Keystore is not configured"})),
        )
    })?;

    let client = proxy_client();
    let mut request_builder = client
        .post(format!("{}/register-tee", keystore_url))
        .json(&payload);

    if let Some(ref token) = state.config.keystore_auth_token {
        request_builder = request_builder.header("Authorization", format!("Bearer {}", token));
    }

    let keystore_response = request_builder.send().await.map_err(|e| {
        tracing::error!("Failed to proxy TEE registration to keystore: {}", e);
        (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({"error": format!("Keystore unreachable: {}", e)})),
        )
    })?;

    let status = keystore_response.status();
    let body: serde_json::Value = keystore_response.json().await.map_err(|e| {
        tracing::error!("Failed to parse keystore TEE registration response: {}", e);
        (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({"error": "Invalid keystore response"})),
        )
    })?;

    let axum_status =
        StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    Ok((axum_status, Json(body)).into_response())
}
