//! Wallet host functions for WASM components
//!
//! Implements the `outlayer:wallet/api` WIT interface.
//! Proxies wallet operations to the coordinator's wallet REST API.

use anyhow::Result;
use tracing::debug;
use wasmtime::component::Linker;

// Generate bindings from WIT
wasmtime::component::bindgen!({
    path: "wit",
    world: "outlayer:wallet/wallet-host",
});

/// Result type: (json_result, error)
type WalletResult = (String, String);

/// Host state for wallet functions
pub struct WalletHostState {
    /// Wallet ID from execution context (e.g. "ed25519:abc...")
    wallet_id: String,
    /// Blocking HTTP client for coordinator wallet API calls
    http_client: reqwest::blocking::Client,
    /// Coordinator base URL (e.g. "http://localhost:8080")
    coordinator_url: String,
    /// Wallet signature for authenticating requests
    /// Pre-computed by the worker using the keystore
    wallet_auth_token: String,
    /// Call counter for rate limiting
    call_count: u32,
    /// Max wallet calls per execution
    max_calls: u32,
}

impl WalletHostState {
    /// Create wallet host state
    ///
    /// `wallet_id` is the wallet pubkey identifier from X-Wallet-Id header.
    /// `coordinator_url` is the coordinator base URL.
    /// `wallet_auth_token` is the internal auth token for coordinator wallet API.
    pub fn new(
        wallet_id: &str,
        coordinator_url: &str,
        wallet_auth_token: &str,
    ) -> Self {
        let http_client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(5))
            .build()
            .expect("failed to build wallet HTTP client");

        Self {
            wallet_id: wallet_id.to_string(),
            http_client,
            coordinator_url: coordinator_url.to_string(),
            wallet_auth_token: wallet_auth_token.to_string(),
            call_count: 0,
            max_calls: 50,
        }
    }

    /// Check rate limit, returns error string if exceeded
    fn check_rate_limit(&mut self) -> Option<String> {
        if self.call_count >= self.max_calls {
            Some(format!(
                "Wallet rate limit exceeded: {} calls (max: {})",
                self.call_count + 1,
                self.max_calls
            ))
        } else {
            self.call_count += 1;
            None
        }
    }

    /// Make an internal wallet API call to the coordinator
    fn call_coordinator(
        &self,
        method: &str,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> WalletResult {
        let url = format!("{}{}", self.coordinator_url, path);

        let mut request_builder = match method {
            "GET" => self.http_client.get(&url),
            "POST" => self.http_client.post(&url),
            _ => return (String::new(), format!("Unsupported HTTP method: {}", method)),
        };

        // Add internal auth headers
        request_builder = request_builder
            .header("X-Wallet-Id", &self.wallet_id)
            .header("X-Internal-Wallet-Auth", &self.wallet_auth_token);

        if let Some(json_body) = body {
            request_builder = request_builder.json(json_body);
        }

        match request_builder.send() {
            Ok(response) => {
                let status = response.status();
                match response.text() {
                    Ok(text) => {
                        if status.is_success() {
                            (text, String::new())
                        } else {
                            // Extract error message from JSON response if possible
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                                let msg = json["message"].as_str()
                                    .or_else(|| json["error"].as_str())
                                    .unwrap_or(&text);
                                (String::new(), msg.to_string())
                            } else {
                                (String::new(), text)
                            }
                        }
                    }
                    Err(e) => (String::new(), format!("Failed to read response: {}", e)),
                }
            }
            Err(e) => (String::new(), format!("Wallet request failed: {}", e)),
        }
    }
}

impl outlayer::wallet::api::Host for WalletHostState {
    fn get_id(&mut self) -> WalletResult {
        debug!("wallet::get_id wallet_id={}", self.wallet_id);
        (self.wallet_id.clone(), String::new())
    }

    fn get_address(&mut self, chain: String) -> WalletResult {
        debug!("wallet::get_address chain={}, wallet_id={}", chain, self.wallet_id);

        if let Some(err) = self.check_rate_limit() {
            return (String::new(), err);
        }

        if chain.is_empty() {
            return (String::new(), "chain parameter is required".to_string());
        }

        let path = format!("/wallet/v1/address?chain={}", urlencoding::encode(&chain));
        self.call_coordinator("GET", &path, None)
    }

    fn withdraw(&mut self, chain: String, to: String, amount: String, token: String) -> WalletResult {
        debug!(
            "wallet::withdraw chain={}, to={}, amount={}, token={}, wallet_id={}",
            chain, to, amount, token, self.wallet_id
        );

        if let Some(err) = self.check_rate_limit() {
            return (String::new(), err);
        }

        if chain.is_empty() || to.is_empty() || amount.is_empty() {
            return (String::new(), "chain, to, and amount are required".to_string());
        }

        let body = serde_json::json!({
            "chain": chain,
            "to": to,
            "amount": amount,
            "token": if token.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(token) },
        });

        self.call_coordinator("POST", "/wallet/v1/withdraw", Some(&body))
    }

    fn withdraw_dry_run(&mut self, chain: String, to: String, amount: String, token: String) -> WalletResult {
        debug!(
            "wallet::withdraw_dry_run chain={}, to={}, amount={}, token={}, wallet_id={}",
            chain, to, amount, token, self.wallet_id
        );

        if let Some(err) = self.check_rate_limit() {
            return (String::new(), err);
        }

        if chain.is_empty() || to.is_empty() || amount.is_empty() {
            return (String::new(), "chain, to, and amount are required".to_string());
        }

        let body = serde_json::json!({
            "chain": chain,
            "to": to,
            "amount": amount,
            "token": if token.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(token) },
        });

        self.call_coordinator("POST", "/wallet/v1/withdraw/dry-run", Some(&body))
    }

    fn get_request_status(&mut self, request_id: String) -> WalletResult {
        debug!(
            "wallet::get_request_status request_id={}, wallet_id={}",
            request_id, self.wallet_id
        );

        if let Some(err) = self.check_rate_limit() {
            return (String::new(), err);
        }

        if request_id.is_empty() {
            return (String::new(), "request_id is required".to_string());
        }

        let path = format!("/wallet/v1/requests/{}", urlencoding::encode(&request_id));
        self.call_coordinator("GET", &path, None)
    }

    fn list_tokens(&mut self) -> WalletResult {
        debug!("wallet::list_tokens wallet_id={}", self.wallet_id);

        if let Some(err) = self.check_rate_limit() {
            return (String::new(), err);
        }

        self.call_coordinator("GET", "/wallet/v1/tokens", None)
    }
}

/// Add wallet host functions to a wasmtime component linker
pub fn add_wallet_to_linker<T: Send + 'static>(
    linker: &mut Linker<T>,
    get_state: impl Fn(&mut T) -> &mut WalletHostState + Send + Sync + Copy + 'static,
) -> Result<()> {
    outlayer::wallet::api::add_to_linker(linker, get_state)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state() -> WalletHostState {
        WalletHostState {
            wallet_id: "ed25519:abc123".to_string(),
            http_client: reqwest::blocking::Client::new(),
            coordinator_url: "http://localhost:9999".to_string(),
            wallet_auth_token: "test-token".to_string(),
            call_count: 0,
            max_calls: 50,
        }
    }

    #[test]
    fn test_rate_limit_under_max() {
        let mut state = make_state();
        for _ in 0..50 {
            assert!(state.check_rate_limit().is_none());
        }
    }

    #[test]
    fn test_rate_limit_at_max() {
        let mut state = make_state();
        // Use up all 50 calls
        for _ in 0..50 {
            assert!(state.check_rate_limit().is_none());
        }
        // 51st call should fail
        let err = state.check_rate_limit();
        assert!(err.is_some());
        assert!(err.unwrap().contains("rate limit"));
    }

    #[test]
    fn test_get_id_returns_wallet_id() {
        use outlayer::wallet::api::Host;
        let mut state = make_state();
        let (id, err) = state.get_id();
        assert_eq!(id, "ed25519:abc123");
        assert!(err.is_empty());
    }
}
