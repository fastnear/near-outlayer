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

const RAW_SIGNING_FIELDS: &[&str] = &[
    "private_key",
    "signer_key",
    "signer_private_key",
    "signed_tx_base64",
    "signed_delegate_base64",
    "delegate_action",
    "sign_nep366_delegate",
    "sign_near_transaction",
];

fn parse_sequence_calls_json(calls_json: &str) -> Result<serde_json::Value, String> {
    if calls_json.trim().is_empty() {
        return Err("calls_json is required".to_string());
    }

    let calls: serde_json::Value =
        serde_json::from_str(calls_json).map_err(|e| format!("Invalid calls_json: {}", e))?;

    let array = calls
        .as_array()
        .ok_or_else(|| "calls_json must be a JSON array".to_string())?;

    if array.is_empty() {
        return Err("sequential batch requires at least one call".to_string());
    }
    if array.len() > 3 {
        return Err("sequential batch supports at most 3 calls".to_string());
    }

    for (index, call) in array.iter().enumerate() {
        let object = call
            .as_object()
            .ok_or_else(|| format!("call {} must be a JSON object", index))?;

        for field in ["receiver_id", "method_name", "gas", "deposit"] {
            let value = object
                .get(field)
                .and_then(|v| v.as_str())
                .ok_or_else(|| format!("call {} missing string field {}", index, field))?;
            if value.is_empty() {
                return Err(format!("call {} field {} must not be empty", index, field));
            }
        }

        let has_args_base64 = object.get("args_base64").is_some();
        let has_args_json = object.get("args_json").is_some();
        let has_near_intents = object.get("near_intents").is_some();
        let payload_modes = [has_args_base64, has_args_json, has_near_intents]
            .iter()
            .filter(|present| **present)
            .count();
        if payload_modes != 1 {
            return Err(format!(
                "call {} must include exactly one of args_base64, args_json, or near_intents",
                index
            ));
        }

        if let Some(args_base64) = object.get("args_base64") {
            if !args_base64.is_string() {
                return Err(format!("call {} field args_base64 must be a string", index));
            }
        }

        if let Some(args_json) = object.get("args_json") {
            if !args_json.is_string() {
                return Err(format!("call {} field args_json must be a string", index));
            }
        }

        if let Some(near_intents) = object.get("near_intents") {
            if !near_intents.is_object() {
                return Err(format!(
                    "call {} field near_intents must be an object",
                    index
                ));
            }

            let receiver_id = object
                .get("receiver_id")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            if receiver_id != "intents.near" {
                return Err(format!(
                    "call {} near_intents requires receiver_id intents.near",
                    index
                ));
            }

            let method_name = object
                .get("method_name")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            if method_name != "execute_intents" {
                return Err(format!(
                    "call {} near_intents requires method_name execute_intents",
                    index
                ));
            }

            let deposit = object
                .get("deposit")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            if deposit != "0" {
                return Err(format!("call {} near_intents requires deposit 0", index));
            }
        }
    }

    Ok(calls)
}

fn parse_wallet_workflow_json(workflow_json: &str) -> Result<serde_json::Value, String> {
    if workflow_json.trim().is_empty() {
        return Err("workflow_json is required".to_string());
    }

    let workflow: serde_json::Value =
        serde_json::from_str(workflow_json).map_err(|e| format!("Invalid workflow_json: {}", e))?;
    reject_raw_signing_fields("$", &workflow)?;

    let object = workflow
        .as_object()
        .ok_or_else(|| "workflow_json must be a JSON object".to_string())?;
    let steps = object
        .get("steps")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "workflow_json must include a steps array".to_string())?;

    if steps.is_empty() {
        return Err("wallet workflow requires at least one step".to_string());
    }

    for (index, step) in steps.iter().enumerate() {
        validate_workflow_step(index, step)?;
    }

    Ok(workflow)
}

fn reject_raw_signing_fields(path: &str, value: &serde_json::Value) -> Result<(), String> {
    match value {
        serde_json::Value::Object(object) => {
            for (key, child) in object {
                if RAW_SIGNING_FIELDS.contains(&key.as_str()) {
                    return Err(format!(
                        "wallet workflow must not include raw signing field {} at {}",
                        key, path
                    ));
                }
                reject_raw_signing_fields(&format!("{}.{}", path, key), child)?;
            }
        }
        serde_json::Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                reject_raw_signing_fields(&format!("{}[{}]", path, index), child)?;
            }
        }
        _ => {}
    }

    Ok(())
}

fn validate_workflow_step(index: usize, step: &serde_json::Value) -> Result<(), String> {
    let object = step
        .as_object()
        .ok_or_else(|| format!("workflow step {} must be a JSON object", index))?;

    let kind = string_field(object, index, "kind")?;
    match kind {
        "intents.transfer" | "intents.swap" | "intents.execute_raw" => Ok(()),
        "funding.wrap_near"
        | "funding.intents_deposit"
        | "funding.balance_check"
        | "funding.storage_deposit" => Ok(()),
        "near.function_call" => validate_direct_user_function_call_step(index, object),
        _ => Err(format!(
            "workflow step {} has unsupported kind {}",
            index, kind
        )),
    }
}

fn validate_direct_user_function_call_step(
    index: usize,
    object: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    let predecessor_requirement = string_field(object, index, "predecessor_requirement")?;
    if predecessor_requirement != "user_required" {
        return Err(format!(
            "workflow step {} near.function_call must set predecessor_requirement user_required",
            index
        ));
    }

    string_field(object, index, "user_id")?;
    string_field(object, index, "receiver_id")?;

    if let Some(actions) = object.get("actions") {
        let actions = actions
            .as_array()
            .ok_or_else(|| format!("workflow step {} field actions must be an array", index))?;
        if actions.is_empty() {
            return Err(format!(
                "workflow step {} near.function_call requires at least one action",
                index
            ));
        }
        for (action_index, action) in actions.iter().enumerate() {
            validate_function_call_action(index, action_index, action)?;
        }
        return Ok(());
    }

    string_field(object, index, "method_name")?;
    string_field(object, index, "gas")?;
    string_field(object, index, "deposit")?;
    Ok(())
}

fn validate_function_call_action(
    step_index: usize,
    action_index: usize,
    action: &serde_json::Value,
) -> Result<(), String> {
    let object = action.as_object().ok_or_else(|| {
        format!(
            "workflow step {} action {} must be a JSON object",
            step_index, action_index
        )
    })?;

    for field in ["method_name", "gas", "deposit"] {
        let value = object.get(field).and_then(|v| v.as_str()).ok_or_else(|| {
            format!(
                "workflow step {} action {} missing string field {}",
                step_index, action_index, field
            )
        })?;
        if value.is_empty() {
            return Err(format!(
                "workflow step {} action {} field {} must not be empty",
                step_index, action_index, field
            ));
        }
    }

    Ok(())
}

fn string_field<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    index: usize,
    field: &str,
) -> Result<&'a str, String> {
    let value = object
        .get(field)
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("workflow step {} missing string field {}", index, field))?;
    if value.is_empty() {
        return Err(format!(
            "workflow step {} field {} must not be empty",
            index, field
        ));
    }
    Ok(value)
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
    pub fn new(wallet_id: &str, coordinator_url: &str, wallet_auth_token: &str) -> Self {
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
            _ => {
                return (
                    String::new(),
                    format!("Unsupported HTTP method: {}", method),
                )
            }
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
                                let msg = json["message"]
                                    .as_str()
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
        debug!(
            "wallet::get_address chain={}, wallet_id={}",
            chain, self.wallet_id
        );

        if let Some(err) = self.check_rate_limit() {
            return (String::new(), err);
        }

        if chain.is_empty() {
            return (String::new(), "chain parameter is required".to_string());
        }

        let path = format!("/wallet/v1/address?chain={}", urlencoding::encode(&chain));
        self.call_coordinator("GET", &path, None)
    }

    fn withdraw(
        &mut self,
        chain: String,
        to: String,
        amount: String,
        token: String,
    ) -> WalletResult {
        debug!(
            "wallet::withdraw chain={}, to={}, amount={}, token={}, wallet_id={}",
            chain, to, amount, token, self.wallet_id
        );

        if let Some(err) = self.check_rate_limit() {
            return (String::new(), err);
        }

        if chain.is_empty() || to.is_empty() || amount.is_empty() {
            return (
                String::new(),
                "chain, to, and amount are required".to_string(),
            );
        }

        let body = serde_json::json!({
            "chain": chain,
            "to": to,
            "amount": amount,
            "token": if token.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(token) },
        });

        self.call_coordinator("POST", "/wallet/v1/intents/withdraw", Some(&body))
    }

    fn withdraw_dry_run(
        &mut self,
        chain: String,
        to: String,
        amount: String,
        token: String,
    ) -> WalletResult {
        debug!(
            "wallet::withdraw_dry_run chain={}, to={}, amount={}, token={}, wallet_id={}",
            chain, to, amount, token, self.wallet_id
        );

        if let Some(err) = self.check_rate_limit() {
            return (String::new(), err);
        }

        if chain.is_empty() || to.is_empty() || amount.is_empty() {
            return (
                String::new(),
                "chain, to, and amount are required".to_string(),
            );
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

    fn sequence_calls(
        &mut self,
        gate_id: String,
        calls_json: String,
        idempotency_key: String,
    ) -> WalletResult {
        debug!(
            "wallet::sequence_calls gate_id={}, wallet_id={}",
            gate_id, self.wallet_id
        );

        if let Some(err) = self.check_rate_limit() {
            return (String::new(), err);
        }

        if gate_id.is_empty() {
            return (String::new(), "gate_id is required".to_string());
        }

        let calls = match parse_sequence_calls_json(&calls_json) {
            Ok(calls) => calls,
            Err(err) => return (String::new(), err),
        };

        let mut body = serde_json::json!({
            "gate_id": gate_id,
            "calls": calls,
        });

        if !idempotency_key.is_empty() {
            if let Some(object) = body.as_object_mut() {
                object.insert(
                    "idempotency_key".to_string(),
                    serde_json::Value::String(idempotency_key),
                );
            }
        }

        self.call_coordinator("POST", "/wallet/v1/sequential-batch", Some(&body))
    }

    fn get_sequence_status(&mut self, request_id: String) -> WalletResult {
        debug!(
            "wallet::get_sequence_status request_id={}, wallet_id={}",
            request_id, self.wallet_id
        );

        if let Some(err) = self.check_rate_limit() {
            return (String::new(), err);
        }

        if request_id.is_empty() {
            return (String::new(), "request_id is required".to_string());
        }

        let path = format!(
            "/wallet/v1/sequential-batch/{}",
            urlencoding::encode(&request_id)
        );
        self.call_coordinator("GET", &path, None)
    }

    fn plan_wallet_workflow(&mut self, workflow_json: String) -> WalletResult {
        debug!("wallet::plan_wallet_workflow wallet_id={}", self.wallet_id);

        if let Some(err) = self.check_rate_limit() {
            return (String::new(), err);
        }

        let workflow = match parse_wallet_workflow_json(&workflow_json) {
            Ok(workflow) => workflow,
            Err(err) => return (String::new(), err),
        };

        self.call_coordinator("POST", "/wallet/v1/workflows/plan", Some(&workflow))
    }

    fn execute_wallet_workflow(
        &mut self,
        workflow_json: String,
        idempotency_key: String,
    ) -> WalletResult {
        debug!(
            "wallet::execute_wallet_workflow wallet_id={}",
            self.wallet_id
        );

        if let Some(err) = self.check_rate_limit() {
            return (String::new(), err);
        }

        let mut workflow = match parse_wallet_workflow_json(&workflow_json) {
            Ok(workflow) => workflow,
            Err(err) => return (String::new(), err),
        };

        if !idempotency_key.is_empty() {
            if let Some(object) = workflow.as_object_mut() {
                object.insert(
                    "idempotency_key".to_string(),
                    serde_json::Value::String(idempotency_key),
                );
            }
        }

        self.call_coordinator("POST", "/wallet/v1/workflows/execute", Some(&workflow))
    }

    fn get_wallet_workflow_status(&mut self, request_id: String) -> WalletResult {
        debug!(
            "wallet::get_wallet_workflow_status request_id={}, wallet_id={}",
            request_id, self.wallet_id
        );

        if let Some(err) = self.check_rate_limit() {
            return (String::new(), err);
        }

        if request_id.is_empty() {
            return (String::new(), "request_id is required".to_string());
        }

        let path = format!("/wallet/v1/workflows/{}", urlencoding::encode(&request_id));
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

    #[test]
    fn test_parse_sequence_calls_json_accepts_three_calls() {
        let calls = r#"[
            {"receiver_id":"intents.near","method_name":"execute","args_json":"{}","gas":"30000000000000","deposit":"0"},
            {"receiver_id":"intents.near","method_name":"execute","args_base64":"","gas":"30000000000000","deposit":"0"},
            {"receiver_id":"intents.near","method_name":"execute","args_json":"{}","gas":"30000000000000","deposit":"0"}
        ]"#;

        let parsed = parse_sequence_calls_json(calls).unwrap();
        assert_eq!(parsed.as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_parse_sequence_calls_json_rejects_oversized_batch() {
        let calls = r#"[
            {"receiver_id":"a.near","method_name":"m","args_json":"{}","gas":"1","deposit":"0"},
            {"receiver_id":"a.near","method_name":"m","args_json":"{}","gas":"1","deposit":"0"},
            {"receiver_id":"a.near","method_name":"m","args_json":"{}","gas":"1","deposit":"0"},
            {"receiver_id":"a.near","method_name":"m","args_json":"{}","gas":"1","deposit":"0"}
        ]"#;

        let err = parse_sequence_calls_json(calls).unwrap_err();
        assert!(err.contains("at most 3"));
    }

    #[test]
    fn test_parse_sequence_calls_json_requires_args() {
        let calls = r#"[
            {"receiver_id":"a.near","method_name":"m","gas":"1","deposit":"0"}
        ]"#;

        let err = parse_sequence_calls_json(calls).unwrap_err();
        assert!(err.contains("exactly one"));
    }

    #[test]
    fn test_parse_sequence_calls_json_accepts_near_intents_mode() {
        let calls = r#"[
            {
                "receiver_id":"intents.near",
                "method_name":"execute_intents",
                "near_intents":{
                    "signer_id":"1111111111111111111111111111111111111111111111111111111111111111",
                    "deadline":"2026-04-24T20:00:00Z",
                    "intents":[
                        {
                            "intent":"transfer",
                            "receiver_id":"mike.near",
                            "tokens":{"nep141:wrap.near":"1000000000000000000"}
                        }
                    ]
                },
                "gas":"100000000000000",
                "deposit":"0"
            }
        ]"#;

        let parsed = parse_sequence_calls_json(calls).unwrap();
        assert_eq!(parsed.as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_parse_sequence_calls_json_rejects_multiple_payload_modes() {
        let calls = r#"[
            {
                "receiver_id":"intents.near",
                "method_name":"execute_intents",
                "args_json":"{}",
                "near_intents":{},
                "gas":"100000000000000",
                "deposit":"0"
            }
        ]"#;

        let err = parse_sequence_calls_json(calls).unwrap_err();
        assert!(err.contains("exactly one"));
    }

    #[test]
    fn test_parse_sequence_calls_json_near_intents_requires_intents_receiver() {
        let calls = r#"[
            {
                "receiver_id":"wrap.near",
                "method_name":"execute_intents",
                "near_intents":{},
                "gas":"100000000000000",
                "deposit":"0"
            }
        ]"#;

        let err = parse_sequence_calls_json(calls).unwrap_err();
        assert!(err.contains("receiver_id intents.near"));
    }

    #[test]
    fn test_parse_sequence_calls_json_near_intents_requires_execute_intents() {
        let calls = r#"[
            {
                "receiver_id":"intents.near",
                "method_name":"ft_withdraw",
                "near_intents":{},
                "gas":"100000000000000",
                "deposit":"0"
            }
        ]"#;

        let err = parse_sequence_calls_json(calls).unwrap_err();
        assert!(err.contains("method_name execute_intents"));
    }

    #[test]
    fn test_parse_sequence_calls_json_near_intents_requires_zero_deposit() {
        let calls = r#"[
            {
                "receiver_id":"intents.near",
                "method_name":"execute_intents",
                "near_intents":{},
                "gas":"100000000000000",
                "deposit":"1"
            }
        ]"#;

        let err = parse_sequence_calls_json(calls).unwrap_err();
        assert!(err.contains("deposit 0"));
    }

    #[test]
    fn test_sequence_calls_requires_gate_id() {
        use outlayer::wallet::api::Host;
        let mut state = make_state();
        let (_, err) = state.sequence_calls(
            String::new(),
            r#"[{"receiver_id":"a.near","method_name":"m","args_json":"{}","gas":"1","deposit":"0"}]"#
                .to_string(),
            String::new(),
        );
        assert!(err.contains("gate_id"));
    }

    #[test]
    fn test_get_sequence_status_requires_request_id() {
        use outlayer::wallet::api::Host;
        let mut state = make_state();
        let (_, err) = state.get_sequence_status(String::new());
        assert!(err.contains("request_id"));
    }

    #[test]
    fn test_parse_wallet_workflow_accepts_gate_intents_and_direct_user_steps() {
        let workflow = r#"{
            "steps": [
                {"kind":"intents.transfer","token":"nep141:wrap.near","amount":"1","receiver_id":"mike.near"},
                {"kind":"funding.wrap_near","amount":"1000000000000000000000000"},
                {
                    "kind":"near.function_call",
                    "predecessor_requirement":"user_required",
                    "user_id":"mike.near",
                    "receiver_id":"staking.pool.near",
                    "actions":[
                        {"method_name":"withdraw_reward","args_json":"{}","gas":"30000000000000","deposit":"0"},
                        {"method_name":"deposit_and_stake","args_json":"{}","gas":"100000000000000","deposit":"1"}
                    ]
                }
            ]
        }"#;

        let parsed = parse_wallet_workflow_json(workflow).unwrap();
        assert_eq!(parsed["steps"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_parse_wallet_workflow_rejects_raw_signing_fields() {
        let workflow = r#"{
            "steps": [
                {"kind":"intents.transfer","signed_tx_base64":"abc"}
            ]
        }"#;

        let err = parse_wallet_workflow_json(workflow).unwrap_err();
        assert!(err.contains("raw signing field"));
    }

    #[test]
    fn test_parse_wallet_workflow_rejects_ambiguous_function_call() {
        let workflow = r#"{
            "steps": [
                {
                    "kind":"near.function_call",
                    "predecessor_requirement":"wallet",
                    "user_id":"mike.near",
                    "receiver_id":"staking.pool.near",
                    "method_name":"withdraw_reward",
                    "gas":"30000000000000",
                    "deposit":"0"
                }
            ]
        }"#;

        let err = parse_wallet_workflow_json(workflow).unwrap_err();
        assert!(err.contains("predecessor_requirement user_required"));
    }

    #[test]
    fn test_parse_wallet_workflow_rejects_empty_actions() {
        let workflow = r#"{
            "steps": [
                {
                    "kind":"near.function_call",
                    "predecessor_requirement":"user_required",
                    "user_id":"mike.near",
                    "receiver_id":"staking.pool.near",
                    "actions":[]
                }
            ]
        }"#;

        let err = parse_wallet_workflow_json(workflow).unwrap_err();
        assert!(err.contains("at least one action"));
    }

    #[test]
    fn test_plan_wallet_workflow_rate_limit_applies_before_proxy() {
        use outlayer::wallet::api::Host;
        let mut state = make_state();
        state.call_count = state.max_calls;

        let (_, err) = state.plan_wallet_workflow(r#"{"steps":[]}"#.to_string());
        assert!(err.contains("rate limit"));
    }

    #[test]
    fn test_get_wallet_workflow_status_requires_request_id() {
        use outlayer::wallet::api::Host;
        let mut state = make_state();
        let (_, err) = state.get_wallet_workflow_status(String::new());
        assert!(err.contains("request_id"));
    }
}
