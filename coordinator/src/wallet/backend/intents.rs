//! NEAR Intents backend implementation
//!
//! Swap: 1Click REST API (https://1click.chaindefuser.com)
//! Withdraw: direct ft_withdraw on intents.near (single NEAR transaction)

use super::*;
use anyhow::{Context, Result};
use reqwest::Client;
use serde::Serialize;
use tracing::{debug, warn};

// =============================================================================
// 1Click polling config
// =============================================================================

const ONECLICK_POLL_INTERVAL_MS: u64 = 2_000;
const ONECLICK_POLL_TIMEOUT_MS: u64 = 120_000;

// =============================================================================
// IntentsBackend
// =============================================================================

pub struct IntentsBackend {
    client: Client,
    oneclick_base_url: String,
    oneclick_jwt: Option<String>,
}

impl IntentsBackend {
    pub fn new(oneclick_base_url: String, oneclick_jwt: Option<String>) -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .expect("Failed to create HTTP client"),
            oneclick_base_url,
            oneclick_jwt,
        }
    }

    // =========================================================================
    // 1Click API methods (swap)
    // =========================================================================

    fn oneclick_auth_header(&self) -> Option<String> {
        self.oneclick_jwt.as_ref().map(|jwt| format!("Bearer {}", jwt))
    }

    async fn oneclick_post<T: Serialize>(&self, path: &str, body: &T) -> Result<serde_json::Value> {
        let url = format!("{}{}", self.oneclick_base_url, path);
        let mut req = self.client.post(&url).json(body);
        if let Some(auth) = self.oneclick_auth_header() {
            req = req.header("Authorization", auth);
        }

        let response = req.send().await.context(format!("Failed to connect to 1Click API: {}", url))?;
        let status = response.status();
        let text = response.text().await.context("Failed to read 1Click response body")?;

        debug!("1Click POST {} → {} : {}", path, status, text);

        if !status.is_success() {
            anyhow::bail!("1Click API {} returned HTTP {}: {}", path, status, text);
        }

        serde_json::from_str(&text).context(format!("Failed to parse 1Click response: {}", text))
    }

    async fn oneclick_get(&self, path: &str) -> Result<serde_json::Value> {
        let url = format!("{}{}", self.oneclick_base_url, path);
        let mut req = self.client.get(&url);
        if let Some(auth) = self.oneclick_auth_header() {
            req = req.header("Authorization", auth);
        }

        let response = req.send().await.context(format!("Failed to connect to 1Click API: {}", url))?;
        let status = response.status();
        let text = response.text().await.context("Failed to read 1Click response body")?;

        debug!("1Click GET {} → {} : {}", path, status, text);

        if !status.is_success() {
            anyhow::bail!("1Click API {} returned HTTP {}: {}", path, status, text);
        }

        serde_json::from_str(&text).context(format!("Failed to parse 1Click response: {}", text))
    }
}

// =============================================================================
// WalletBackend trait implementation
// =============================================================================

#[async_trait::async_trait]
impl WalletBackend for IntentsBackend {
    async fn list_tokens(&self) -> Result<Vec<BackendTokenInfo>> {
        Ok(supported_tokens())
    }

    // =========================================================================
    // 1Click API trait methods
    // =========================================================================

    async fn oneclick_quote(
        &self,
        req: OneClickQuoteRequest,
    ) -> Result<OneClickQuoteResponse> {
        let result = self.oneclick_post("/v0/quote", &req).await?;
        let response: OneClickQuoteResponse = serde_json::from_value(result)
            .context("Failed to parse 1Click quote response")?;
        debug!(
            "1Click quote: deposit_address={}, amount_out={}, deadline={}",
            response.quote.deposit_address, response.quote.amount_out, response.quote.deadline
        );
        Ok(response)
    }

    async fn oneclick_submit_deposit(
        &self,
        req: OneClickSubmitDeposit,
    ) -> Result<()> {
        self.oneclick_post("/v0/deposit/submit", &req).await?;
        debug!("1Click deposit submitted: tx={}, deposit_addr={}", req.tx_hash, req.deposit_address);
        Ok(())
    }

    async fn oneclick_poll_status(
        &self,
        deposit_address: &str,
    ) -> Result<OneClickStatusResponse> {
        let max_attempts = ONECLICK_POLL_TIMEOUT_MS / ONECLICK_POLL_INTERVAL_MS;

        for attempt in 0..max_attempts {
            let path = format!("/v0/status?depositAddress={}", deposit_address);
            match self.oneclick_get(&path).await {
                Ok(result) => {
                    let resp: OneClickStatusResponse = serde_json::from_value(result)
                        .context("Failed to parse 1Click status response")?;

                    match resp.status.as_str() {
                        "SUCCESS" => {
                            debug!("1Click swap succeeded after {} polls", attempt + 1);
                            return Ok(resp);
                        }
                        "FAILED" => {
                            anyhow::bail!("1Click swap failed");
                        }
                        "REFUNDED" => {
                            anyhow::bail!("1Click swap was refunded — tokens returned to wallet");
                        }
                        status => {
                            debug!("1Click poll {}: status={}", attempt + 1, status);
                        }
                    }
                }
                Err(e) => {
                    warn!("1Click status poll error (attempt {}): {}", attempt + 1, e);
                }
            }

            tokio::time::sleep(std::time::Duration::from_millis(ONECLICK_POLL_INTERVAL_MS)).await;
        }

        // Timeout — return processing status
        Ok(OneClickStatusResponse {
            status: "PROCESSING".to_string(),
            swap_details: None,
        })
    }
}

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
