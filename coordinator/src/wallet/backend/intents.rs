//! NEAR Intents backend implementation
//!
//! Uses NEAR Intents protocol via solver-relay JSON-RPC.
//! Withdraw: build ft_withdraw intent → NEP-413 sign → publish_intent → poll settlement.
//! Reference: wasi-examples/intents-ark/src/main.rs

use super::*;
use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

/// Solver-relay JSON-RPC request
#[derive(Serialize)]
struct JsonRpcRequest<T: Serialize> {
    id: u32,
    jsonrpc: &'static str,
    method: &'static str,
    params: Vec<T>,
}

/// Signed data for publish_intent
#[derive(Serialize)]
struct PublishIntentParams {
    signed_data: SignedData,
    #[serde(skip_serializing_if = "Option::is_none")]
    quote_hashes: Option<Vec<String>>,
}

#[derive(Serialize)]
struct SignedData {
    payload: SignedPayload,
    standard: String,
    signature: String,
    public_key: String,
}

#[derive(Serialize)]
struct SignedPayload {
    message: String,
    nonce: String,
    recipient: String,
}

/// get_status request params
#[derive(Serialize)]
struct GetStatusParams {
    intent_hash: String,
}

/// JSON-RPC response
#[derive(Deserialize)]
struct JsonRpcResponse {
    #[serde(default)]
    result: Option<serde_json::Value>,
    #[serde(default)]
    error: Option<serde_json::Value>,
}

/// Settlement polling config
const POLL_INTERVAL_MS: u64 = 250;
const POLL_TIMEOUT_MS: u64 = 30_000;

pub struct IntentsBackend {
    client: Client,
    solver_relay_url: String,
}

impl IntentsBackend {
    pub fn new(solver_relay_url: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("Failed to create HTTP client"),
            solver_relay_url,
        }
    }

    /// Call solver-relay JSON-RPC method
    async fn rpc_call<T: Serialize>(&self, method: &'static str, params: Vec<T>) -> Result<serde_json::Value> {
        let request = JsonRpcRequest {
            id: 1,
            jsonrpc: "2.0",
            method,
            params,
        };

        let response = self
            .client
            .post(&self.solver_relay_url)
            .json(&request)
            .send()
            .await
            .context("Failed to connect to solver-relay")?;

        let response_text = response
            .text()
            .await
            .context("Failed to read solver-relay response body")?;

        debug!("Solver-relay {} response: {}", method, response_text);

        let rpc_response: JsonRpcResponse = serde_json::from_str(&response_text)
            .context(format!("Failed to parse solver-relay response: {}", response_text))?;

        if let Some(error) = rpc_response.error {
            anyhow::bail!("Solver-relay {} error: {}", method, error);
        }

        rpc_response
            .result
            .ok_or_else(|| anyhow::anyhow!("Solver-relay {} returned empty result", method))
    }

    /// Publish a signed intent to solver-relay
    async fn publish_intent(&self, signed_intent: &serde_json::Value) -> Result<String> {
        let params = PublishIntentParams {
            signed_data: SignedData {
                payload: SignedPayload {
                    message: signed_intent["message"]
                        .as_str()
                        .ok_or_else(|| anyhow::anyhow!("Missing message in signed intent"))?
                        .to_string(),
                    nonce: signed_intent["nonce"]
                        .as_str()
                        .ok_or_else(|| anyhow::anyhow!("Missing nonce in signed intent"))?
                        .to_string(),
                    recipient: signed_intent["recipient"]
                        .as_str()
                        .ok_or_else(|| anyhow::anyhow!("Missing recipient in signed intent"))?
                        .to_string(),
                },
                standard: signed_intent["standard"]
                    .as_str()
                    .unwrap_or("nep413")
                    .to_string(),
                signature: signed_intent["signature"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing signature in signed intent"))?
                    .to_string(),
                public_key: signed_intent["public_key"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing public_key in signed intent"))?
                    .to_string(),
            },
            quote_hashes: None,
        };

        let result = self.rpc_call("publish_intent", vec![params]).await?;

        // Result can be a string or {"status":"OK","intent_hash":"..."}
        if let Some(hash) = result.as_str() {
            return Ok(hash.to_string());
        }
        if let Some(hash) = result.get("intent_hash").and_then(|h| h.as_str()) {
            return Ok(hash.to_string());
        }
        anyhow::bail!("publish_intent: unexpected result format: {}", result);
    }

    /// Poll get_status until settlement or timeout
    async fn wait_for_settlement(&self, intent_hash: &str) -> Result<String> {
        let max_attempts = POLL_TIMEOUT_MS / POLL_INTERVAL_MS;

        for attempt in 0..max_attempts {
            let params = GetStatusParams {
                intent_hash: intent_hash.to_string(),
            };

            match self.rpc_call("get_status", vec![params]).await {
                Ok(result) => {
                    let status = result
                        .as_str()
                        .or_else(|| result.get("status").and_then(|s| s.as_str()))
                        .unwrap_or("UNKNOWN");

                    match status {
                        "SETTLED" => {
                            debug!("Intent {} settled after {} attempts", intent_hash, attempt + 1);
                            return Ok("success".to_string());
                        }
                        "NOT_FOUND_OR_NOT_VALID" | "NOT_FOUND_OR_NOT_VALID_ANYMORE" | "FAILED" => {
                            anyhow::bail!("Intent {} failed with status: {}", intent_hash, status);
                        }
                        _ => {
                            // Still pending, continue polling
                        }
                    }
                }
                Err(e) => {
                    warn!("get_status poll error (attempt {}): {}", attempt + 1, e);
                }
            }

            tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS)).await;
        }

        // Timeout — return processing (intent may still settle later)
        Ok("processing".to_string())
    }
}

#[async_trait::async_trait]
impl WalletBackend for IntentsBackend {
    async fn withdraw(
        &self,
        wallet: &WalletInfo,
        req: BackendWithdrawRequest,
        signed_tx_base64: &str,
    ) -> Result<BackendWithdrawResponse> {
        debug!(
            "Intents withdraw: wallet={}, chain={}, to={}, amount={}, token={:?}",
            wallet.wallet_id, req.chain, req.to, req.amount, req.token
        );

        // Decode signed intent from base64
        let signed_intent_bytes = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            signed_tx_base64,
        )
        .context("Failed to decode signed intent base64")?;

        let signed_intent: serde_json::Value =
            serde_json::from_slice(&signed_intent_bytes)
                .context("Failed to parse signed intent JSON")?;

        // Publish intent to solver-relay
        let intent_hash = self
            .publish_intent(&signed_intent)
            .await
            .context("Failed to publish intent")?;

        debug!("Intent published: hash={}", intent_hash);

        // Poll for settlement
        let status = self.wait_for_settlement(&intent_hash).await?;

        Ok(BackendWithdrawResponse {
            operation_id: intent_hash.clone(),
            status,
            tx_hash: Some(intent_hash),
            fee: None,
            fee_token: None,
        })
    }

    async fn deposit_quote(
        &self,
        wallet: &WalletInfo,
        _req: BackendDepositRequest,
    ) -> Result<BackendDepositResponse> {
        debug!(
            "Intents deposit: wallet={}, chain={}",
            wallet.wallet_id, wallet.chain
        );

        // Deposits require ft_transfer_call (real NEAR transaction) — not yet implemented.
        // Return wallet's intents address so the user can deposit directly.
        anyhow::bail!(
            "Direct deposits via intents require ft_transfer_call to intents.near. \
             Deposit tokens to wallet address {} on NEAR first.",
            wallet.chain_address
        );
    }

    async fn operation_status(&self, operation_id: &str) -> Result<OperationStatus> {
        debug!("Intents operation status: {}", operation_id);

        let params = GetStatusParams {
            intent_hash: operation_id.to_string(),
        };

        let result = self
            .rpc_call("get_status", vec![params])
            .await
            .context("Failed to get intent status")?;

        let status_str = result
            .as_str()
            .or_else(|| result.get("status").and_then(|s| s.as_str()))
            .unwrap_or("UNKNOWN");

        let status = match status_str {
            "SETTLED" => "success",
            "NOT_FOUND_OR_NOT_VALID" | "NOT_FOUND_OR_NOT_VALID_ANYMORE" | "FAILED" => "failed",
            _ => "processing",
        };

        Ok(OperationStatus {
            operation_id: operation_id.to_string(),
            status: status.to_string(),
            tx_hash: if status == "success" {
                Some(operation_id.to_string())
            } else {
                None
            },
            fee: None,
            fee_token: None,
            error: if status == "failed" {
                Some(format!("Intent status: {}", status_str))
            } else {
                None
            },
        })
    }

    async fn list_tokens(&self) -> Result<Vec<BackendTokenInfo>> {
        // Solver-relay doesn't have a token list endpoint.
        // Return known supported tokens for NEAR Intents.
        Ok(supported_tokens())
    }
}

/// Supported tokens for NEAR Intents protocol
fn supported_tokens() -> Vec<BackendTokenInfo> {
    vec![
        BackendTokenInfo {
            id: "wrap.near".to_string(),
            symbol: "wNEAR".to_string(),
            chains: vec!["near".to_string()],
        },
        BackendTokenInfo {
            id: "17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1".to_string(),
            symbol: "USDC".to_string(),
            chains: vec!["near".to_string(), "ethereum".to_string(), "base".to_string()],
        },
        BackendTokenInfo {
            id: "usdt.tether-token.near".to_string(),
            symbol: "USDT".to_string(),
            chains: vec!["near".to_string(), "ethereum".to_string()],
        },
        BackendTokenInfo {
            id: "aurora".to_string(),
            symbol: "AURORA".to_string(),
            chains: vec!["near".to_string()],
        },
    ]
}
