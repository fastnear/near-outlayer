use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, error, warn};

use crate::AppState;

/// NEAR RPC request structure
#[derive(Debug, Deserialize, Serialize)]
pub struct NearRpcRequest {
    pub jsonrpc: String,
    pub id: String,
    pub method: String,
    pub params: Value,
}

/// NEAR RPC response structure
#[derive(Debug, Deserialize, Serialize)]
pub struct NearRpcResponse {
    pub jsonrpc: String,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
}

/// Generic external API request
#[derive(Debug, Deserialize)]
pub struct ExternalApiRequest {
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub body: Option<Value>,
}

/// Generic external API response
#[derive(Debug, Serialize)]
pub struct ExternalApiResponse {
    pub status: u16,
    pub headers: std::collections::HashMap<String, String>,
    pub body: Value,
}

/// Proxy NEAR RPC requests
/// POST /near-rpc
/// Body: NearRpcRequest
///
/// This endpoint:
/// 1. Receives RPC request from browser/worker
/// 2. Forwards to upstream NEAR RPC (with throttling applied via middleware)
/// 3. Returns response to caller
///
/// Throttling is handled by middleware layer, not here.
pub async fn proxy_near_rpc(
    State(state): State<Arc<AppState>>,
    Json(request): Json<NearRpcRequest>,
) -> Response {
    debug!(
        "Proxying NEAR RPC: method={}, id={}",
        request.method, request.id
    );

    // Get upstream NEAR RPC URL from config
    let near_rpc_url = &state.config.near_rpc_url;

    // Forward request to NEAR RPC
    let client = reqwest::Client::new();
    let result = client
        .post(near_rpc_url)
        .json(&request)
        .send()
        .await;

    match result {
        Ok(response) => {
            let status = response.status();
            debug!("NEAR RPC response: status={}", status);

            // Handle rate limiting from upstream
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                warn!("Upstream NEAR RPC returned 429 - propagating to client");

                // Extract Retry-After header if present
                let retry_after = response
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("60");

                return (
                    StatusCode::TOO_MANY_REQUESTS,
                    [("Retry-After", retry_after)],
                    "Upstream rate limit exceeded".to_string(),
                )
                    .into_response();
            }

            // Parse JSON response
            match response.json::<NearRpcResponse>().await {
                Ok(rpc_response) => {
                    // Check for RPC error in response
                    if let Some(error) = &rpc_response.error {
                        warn!("NEAR RPC error: {:?}", error);
                    }

                    (StatusCode::OK, Json(rpc_response)).into_response()
                }
                Err(e) => {
                    error!("Failed to parse NEAR RPC response: {}", e);
                    (
                        StatusCode::BAD_GATEWAY,
                        format!("Invalid response from NEAR RPC: {}", e),
                    )
                        .into_response()
                }
            }
        }
        Err(e) => {
            error!("Failed to proxy NEAR RPC request: {}", e);

            // Check if timeout or connection error
            if e.is_timeout() {
                (
                    StatusCode::GATEWAY_TIMEOUT,
                    "NEAR RPC request timed out".to_string(),
                )
                    .into_response()
            } else if e.is_connect() {
                (
                    StatusCode::BAD_GATEWAY,
                    "Failed to connect to NEAR RPC".to_string(),
                )
                    .into_response()
            } else {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("NEAR RPC proxy error: {}", e),
                )
                    .into_response()
            }
        }
    }
}

/// Proxy external API requests (OpenAI, other services)
/// POST /external/{service}
/// Body: ExternalApiRequest
///
/// This is a generic proxy for third-party APIs that contracts might need.
/// Throttling prevents abuse of our infrastructure.
pub async fn proxy_external_api(
    State(_state): State<Arc<AppState>>,
    Path(service): Path<String>,
    Json(request): Json<ExternalApiRequest>,
) -> Response {
    debug!("Proxying external API: service={}, url={}", service, request.url);

    // Validate service is allowed
    let allowed_services = vec!["openai", "anthropic", "coingecko", "etherscan"];
    if !allowed_services.contains(&service.as_str()) {
        return (
            StatusCode::FORBIDDEN,
            format!("Service '{}' not allowed", service),
        )
            .into_response();
    }

    // Build request
    let client = reqwest::Client::new();
    let mut req = match request.method.to_uppercase().as_str() {
        "GET" => client.get(&request.url),
        "POST" => client.post(&request.url),
        "PUT" => client.put(&request.url),
        "DELETE" => client.delete(&request.url),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                format!("Unsupported method: {}", request.method),
            )
                .into_response()
        }
    };

    // Add headers (but filter out dangerous ones)
    for (key, value) in &request.headers {
        // Don't allow overriding host or other critical headers
        if key.to_lowercase() != "host" && key.to_lowercase() != "content-length" {
            req = req.header(key, value);
        }
    }

    // Add body if present
    if let Some(body) = request.body {
        req = req.json(&body);
    }

    // Execute request
    match req.send().await {
        Ok(response) => {
            let status = response.status().as_u16();

            // Extract headers
            let mut headers = std::collections::HashMap::new();
            for (key, value) in response.headers() {
                if let Ok(value_str) = value.to_str() {
                    headers.insert(key.to_string(), value_str.to_string());
                }
            }

            // Parse body as JSON
            match response.json::<Value>().await {
                Ok(body) => {
                    let api_response = ExternalApiResponse {
                        status,
                        headers,
                        body,
                    };
                    (StatusCode::OK, Json(api_response)).into_response()
                }
                Err(e) => {
                    error!("Failed to parse external API response: {}", e);
                    (
                        StatusCode::BAD_GATEWAY,
                        format!("Invalid response from external API: {}", e),
                    )
                        .into_response()
                }
            }
        }
        Err(e) => {
            error!("Failed to proxy external API request: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("External API proxy error: {}", e),
            )
                .into_response()
        }
    }
}

/// Get throttle metrics (for monitoring/debugging)
/// GET /throttle/metrics
pub async fn get_throttle_metrics(
    State(state): State<Arc<AppState>>,
) -> Response {
    let metrics = state.throttle_manager.get_metrics().await;
    (StatusCode::OK, Json(metrics)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_near_rpc_request_serialization() {
        let request = NearRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: "dontcare".to_string(),
            method: "query".to_string(),
            params: serde_json::json!({
                "request_type": "view_account",
                "finality": "final",
                "account_id": "example.near"
            }),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("view_account"));
    }

    #[test]
    fn test_external_api_request_deserialization() {
        let json = r#"{
            "method": "POST",
            "url": "https://api.openai.com/v1/completions",
            "headers": {
                "Authorization": "Bearer sk-..."
            },
            "body": {
                "model": "gpt-4",
                "prompt": "Hello"
            }
        }"#;

        let request: ExternalApiRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.method, "POST");
        assert!(request.headers.contains_key("Authorization"));
    }
}
