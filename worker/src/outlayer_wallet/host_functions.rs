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

/// Turn a coordinator error body into the one string a guest gets back.
///
/// `"<code>: <message>"`, plus `key=value` for anything the caller has to ACT
/// on. That shape is what `wallet.wit` documents (`"policy_denied: ..."`), and
/// a guest needs the code far more than the prose: `wallet_busy` clears by
/// itself and is worth retrying, `policy_denied` never will be. A guest handed
/// only a sentence has to match on wording, and wording changes.
///
/// Appended fields are the ones that change what to do next, not everything in
/// the body:
/// * `terminal` — retrying an Agent Connect refusal that is terminal spins
///   forever while the owner is never told to act;
/// * `in_flight_request_id` — what to poll instead of retrying blindly;
/// * `in_flight_operation` — what to wait FOR. It is set even when there is no
///   id yet, and it is the difference between retrying at once and backing off:
///   a transfer clears in seconds, a cross-chain withdraw can run for minutes.
///
/// A body that is not the expected JSON comes back verbatim: inventing a code
/// for it would be worse than passing on what the server actually said.
fn guest_error(body: &str) -> String {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(body) else {
        return body.to_string();
    };
    let Some(code) = json["error"].as_str() else {
        // No code to lead with; the message alone still beats raw JSON.
        return json["message"].as_str().unwrap_or(body).to_string();
    };
    let message = json["message"].as_str().unwrap_or_default();

    let mut out = if message.is_empty() {
        code.to_string()
    } else {
        format!("{code}: {message}")
    };
    if let Some(terminal) = json["terminal"].as_bool() {
        out.push_str(&format!(" terminal={terminal}"));
    }
    if let Some(id) = json["in_flight_request_id"].as_str() {
        out.push_str(&format!(" in_flight_request_id={id}"));
    }
    // LAST, and the order is load-bearing: the guest reads these fields off the
    // END and truncates the message at each one it takes. Appended after the
    // id, it is removed along with it; appended before, it would be left
    // dangling in the message text once the id was stripped.
    if let Some(op) = json["in_flight_operation"].as_str() {
        out.push_str(&format!(" in_flight_operation={op}"));
    }
    out
}


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
                            (String::new(), guest_error(&text))
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

        self.call_coordinator("POST", "/wallet/v1/intents/withdraw", Some(&body))
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

        self.call_coordinator("POST", "/wallet/v1/intents/withdraw/dry-run", Some(&body))
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

    fn transfer(&mut self, chain: String, to: String, amount: String) -> WalletResult {
        debug!(
            "wallet::transfer chain={}, to={}, amount={}, wallet_id={}",
            chain, to, amount, self.wallet_id
        );

        if let Some(err) = self.check_rate_limit() {
            return (String::new(), err);
        }

        if to.is_empty() || amount.is_empty() {
            return (String::new(), "to and amount are required".to_string());
        }

        let body = serde_json::json!({
            "chain": if chain.is_empty() { "near".to_string() } else { chain },
            "receiver_id": to,
            "amount": amount,
        });

        self.call_coordinator("POST", "/wallet/v1/transfer", Some(&body))
    }

    fn get_balance(&mut self, chain: String, token: String) -> WalletResult {
        debug!(
            "wallet::get_balance chain={}, token={}, wallet_id={}",
            chain, token, self.wallet_id
        );

        if let Some(err) = self.check_rate_limit() {
            return (String::new(), err);
        }

        let chain_param = if chain.is_empty() { "near" } else { &chain };
        let path = if token.is_empty() {
            format!("/wallet/v1/balance?chain={}", chain_param)
        } else {
            format!(
                "/wallet/v1/balance?chain={}&token={}",
                chain_param,
                urlencoding::encode(&token)
            )
        };

        self.call_coordinator("GET", &path, None)
    }

    fn intents_deposit(&mut self, token: String, amount: String) -> WalletResult {
        debug!(
            "wallet::intents_deposit token={}, amount={}, wallet_id={}",
            token, amount, self.wallet_id
        );

        if let Some(err) = self.check_rate_limit() {
            return (String::new(), err);
        }

        if token.is_empty() || amount.is_empty() {
            return (String::new(), "token and amount are required".to_string());
        }

        let body = serde_json::json!({
            "token": token,
            "amount": amount,
        });

        self.call_coordinator("POST", "/wallet/v1/intents/deposit", Some(&body))
    }

    fn swap(
        &mut self,
        token_in: String,
        token_out: String,
        amount_in: String,
        min_amount_out: String,
    ) -> WalletResult {
        debug!(
            "wallet::swap token_in={}, token_out={}, amount_in={}, min_amount_out={}, wallet_id={}",
            token_in, token_out, amount_in, min_amount_out, self.wallet_id
        );

        if let Some(err) = self.check_rate_limit() {
            return (String::new(), err);
        }

        if token_in.is_empty() || token_out.is_empty() || amount_in.is_empty() {
            return (
                String::new(),
                "token_in, token_out, and amount_in are required".to_string(),
            );
        }

        let body = serde_json::json!({
            "token_in": token_in,
            "token_out": token_out,
            "amount_in": amount_in,
            "min_amount_out": if min_amount_out.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(min_amount_out) },
        });

        self.call_coordinator("POST", "/wallet/v1/intents/swap", Some(&body))
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

#[cfg(test)]
mod guest_error_tests {
    use super::guest_error;

    /// The code comes FIRST, because that is the only part a guest can route
    /// on. `wallet.wit` documents this shape (`"policy_denied: ..."`), and a
    /// guest handed prose alone would have to match on wording.
    #[test]
    fn the_code_leads_and_the_message_follows() {
        let body = r#"{"error":"policy_denied","message":"daily limit exceeded"}"#;
        assert_eq!(guest_error(body), "policy_denied: daily limit exceeded");
    }

    /// Busy clears by itself, so the guest's move is to wait — and it is told
    /// exactly what to wait for rather than retrying blindly.
    #[test]
    fn busy_carries_the_operation_to_poll() {
        let body = r#"{"error":"wallet_busy",
                       "message":"another operation is using this wallet",
                       "in_flight_request_id":"req-42",
                       "in_flight_operation":"swap"}"#;
        let out = guest_error(body);
        assert!(out.starts_with("wallet_busy: "), "{out}");
        assert!(out.contains("in_flight_request_id=req-42"), "{out}");
        assert!(out.contains("in_flight_operation=swap"), "{out}");
        assert!(
            out.find("in_flight_request_id=").unwrap() < out.find("in_flight_operation=").unwrap(),
            "the operation must come after the id — the guest strips fields from the end and \
             truncates at each, so a field before the id is orphaned in the message: {out}"
        );

        // No id yet, which is the case the operation exists for: the wallet was
        // taken a moment ago and the row is not written. The guest must still
        // learn what it is waiting for.
        let pending = r#"{"error":"wallet_busy",
                          "message":"a transfer is using this wallet and has not written its request yet",
                          "in_flight_request_id":null,
                          "in_flight_operation":"transfer"}"#;
        let out = guest_error(pending);
        assert!(!out.contains("in_flight_request_id="), "no id may be implied: {out}");
        assert!(out.contains("in_flight_operation=transfer"), "{out}");
    }

    /// The field that decides whether retrying is pointless at all. An agent
    /// that retries a terminal refusal spins forever while its owner is never
    /// told to act.
    #[test]
    fn a_refusal_says_whether_retrying_is_pointless() {
        let body = r#"{"error":"agent_connect_denied","message":"the grant is spent",
                       "class":"grant_exhausted","terminal":true}"#;
        let out = guest_error(body);
        assert!(out.starts_with("agent_connect_denied: "), "{out}");
        assert!(out.contains("terminal=true"), "{out}");
    }

    /// Anything that is not the expected JSON is passed on WHOLE. Inventing a
    /// code for it would be worse than repeating what the server said.
    #[test]
    fn an_unexpected_body_is_passed_on_verbatim() {
        assert_eq!(guest_error("502 Bad Gateway"), "502 Bad Gateway");
        assert_eq!(guest_error(r#"{"detail":"nope"}"#), r#"{"detail":"nope"}"#);
        // JSON with only a message still beats handing back raw JSON.
        assert_eq!(guest_error(r#"{"message":"plain"}"#), "plain");
    }
}
