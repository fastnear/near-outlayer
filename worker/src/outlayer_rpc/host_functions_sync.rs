//! NEAR RPC Proxy for wasi-test-runner
//!
//! Provides RPC host functions for WASM components that import `near:rpc/api`.

use anyhow::{Context, Result};
use base64::Engine;
use near_crypto::{InMemorySigner, SecretKey};
use near_primitives::action::{Action, FunctionCallAction, TransferAction};
use near_primitives::hash::CryptoHash;
use near_primitives::transaction::{Transaction, TransactionV0, SignedTransaction};
use near_primitives::types::{AccountId, BlockHeight, Nonce};
use serde_json::{json, Value};
use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use wasmtime::component::Linker;

// Generate bindings from WIT - sync mode for simpler implementation
wasmtime::component::bindgen!({
    path: "wit",
    world: "rpc-host",
});

/// RPC Proxy client with rate limiting (using blocking HTTP)
pub struct RpcProxy {
    /// HTTP client for RPC requests (blocking)
    client: reqwest::blocking::Client,
    /// RPC URL
    rpc_url: String,
    /// Maximum calls per execution
    max_calls: u32,
    /// Allow transaction methods
    allow_transactions: bool,
    /// Call counter for rate limiting
    call_count: Arc<AtomicU32>,
    /// Optional signer for transactions (account_id, private_key)
    signer: Option<(String, String)>,
}

impl RpcProxy {
    /// Create a new RPC proxy
    pub fn new(
        rpc_url: &str,
        max_calls: u32,
        allow_transactions: bool,
        signer: Option<(String, String)>,
    ) -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            client,
            rpc_url: rpc_url.to_string(),
            max_calls,
            allow_transactions,
            call_count: Arc::new(AtomicU32::new(0)),
            signer,
        })
    }

    /// Reset call counter
    #[allow(dead_code)]
    pub fn reset_call_count(&self) {
        self.call_count.store(0, Ordering::SeqCst);
    }


    /// Get current call count
    pub fn get_call_count(&self) -> u32 {
        self.call_count.load(Ordering::SeqCst)
    }
    /// Check rate limit and increment counter
    fn check_rate_limit(&self) -> Result<()> {
        let count = self.call_count.fetch_add(1, Ordering::SeqCst);
        if count >= self.max_calls {
            anyhow::bail!(
                "RPC rate limit exceeded: {} calls (max: {})",
                count + 1,
                self.max_calls
            );
        }
        Ok(())
    }

    /// Send JSON-RPC request (blocking)
    pub fn call_method(&self, method: &str, params: Value) -> Result<Value> {
        let is_tx_method = matches!(
            method,
            "send_tx" | "broadcast_tx_async" | "broadcast_tx_commit"
        );

        if is_tx_method && !self.allow_transactions {
            anyhow::bail!("Transaction method '{}' is disabled", method);
        }

        self.check_rate_limit()?;

        let request = json!({
            "jsonrpc": "2.0",
            "id": "proxy",
            "method": method,
            "params": params
        });

        eprintln!("[RPC] Sending {} request to {}", method, self.rpc_url);

        let response = self
            .client
            .post(&self.rpc_url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .context("Failed to send RPC request")?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().unwrap_or_default();
            anyhow::bail!("RPC returned status {}: {}", status, error_text);
        }

        let body: Value = response
            .json()
            .context("Failed to parse RPC response")?;
        Ok(body)
    }

    pub fn view_account(&self, account_id: &str) -> Result<Value> {
        let params = json!({
            "request_type": "view_account",
            "account_id": account_id,
            "finality": "final"
        });
        self.call_method("query", params)
    }

    pub fn call_function(&self, account_id: &str, method_name: &str, args_base64: &str) -> Result<Value> {
        let params = json!({
            "request_type": "call_function",
            "account_id": account_id,
            "method_name": method_name,
            "args_base64": args_base64,
            "finality": "final"
        });
        self.call_method("query", params)
    }

    pub fn view_access_key(&self, account_id: &str, public_key: &str) -> Result<Value> {
        let params = json!({
            "request_type": "view_access_key",
            "account_id": account_id,
            "public_key": public_key,
            "finality": "final"
        });
        self.call_method("query", params)
    }

    pub fn block(&self, finality: Option<&str>, block_id: Option<Value>) -> Result<Value> {
        let params = if let Some(fin) = finality {
            json!({ "finality": fin })
        } else if let Some(id) = block_id {
            json!({ "block_id": id })
        } else {
            json!({ "finality": "final" })
        };
        self.call_method("block", params)
    }

    pub fn gas_price(&self) -> Result<Value> {
        self.call_method("gas_price", json!([null]))
    }

    pub fn send_tx(&self, signed_tx_base64: &str, wait_until: Option<&str>) -> Result<Value> {
        let mut params = json!({
            "signed_tx_base64": signed_tx_base64
        });
        if let Some(wait) = wait_until {
            params["wait_until"] = json!(wait);
        }
        self.call_method("send_tx", params)
    }

    /// Get access key info for nonce
    fn get_access_key_nonce(&self, account_id: &str, public_key: &str) -> Result<(Nonce, CryptoHash, BlockHeight)> {
        let result = self.view_access_key(account_id, public_key)?;

        let nonce = result
            .get("result")
            .and_then(|r| r.get("nonce"))
            .and_then(|n| n.as_u64())
            .ok_or_else(|| anyhow::anyhow!("Failed to get nonce"))?;

        let block_hash_str = result
            .get("result")
            .and_then(|r| r.get("block_hash"))
            .and_then(|h| h.as_str())
            .ok_or_else(|| anyhow::anyhow!("Failed to get block_hash"))?;

        let block_height = result
            .get("result")
            .and_then(|r| r.get("block_height"))
            .and_then(|h| h.as_u64())
            .ok_or_else(|| anyhow::anyhow!("Failed to get block_height"))?;

        let block_hash = CryptoHash::from_str(block_hash_str)
            .map_err(|e| anyhow::anyhow!("Invalid block hash: {}", e))?;

        Ok((nonce, block_hash, block_height))
    }

    /// Sign and send a transaction with actions (using configured signer)
    #[allow(dead_code)]
    pub fn sign_and_send_tx(&self, receiver_id: &str, actions: Vec<Action>) -> Result<String> {
        let (signer_account, secret_key_str) = self
            .signer
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No signer configured. Use --rpc-signer-account and --rpc-signer-key"))?;

        self.sign_and_send_tx_as(signer_account, secret_key_str, receiver_id, actions)
    }

    /// Sign and send a transaction with explicit signer
    pub fn sign_and_send_tx_as(
        &self,
        signer_account: &str,
        secret_key_str: &str,
        receiver_id: &str,
        actions: Vec<Action>,
    ) -> Result<String> {
        eprintln!("[RPC_PROXY] sign_and_send_tx_as called: signer={}, receiver={}", signer_account, receiver_id);

        let secret_key = SecretKey::from_str(secret_key_str)
            .map_err(|e| {
                eprintln!("[RPC_PROXY] Failed to parse secret key: {}", e);
                anyhow::anyhow!("Invalid secret key: {}", e)
            })?;

        let signer_id = AccountId::from_str(signer_account)
            .map_err(|e| {
                eprintln!("[RPC_PROXY] Failed to parse signer account: {}", e);
                anyhow::anyhow!("Invalid signer account: {}", e)
            })?;

        let receiver = AccountId::from_str(receiver_id)
            .map_err(|e| {
                eprintln!("[RPC_PROXY] Failed to parse receiver account: {}", e);
                anyhow::anyhow!("Invalid receiver account: {}", e)
            })?;

        let signer = InMemorySigner::from_secret_key(signer_id.clone(), secret_key);
        let public_key = signer.public_key();

        eprintln!("[RPC_PROXY] About to get access key nonce for {}, pubkey: {}", signer_account, public_key);

        // Get nonce and block hash
        let (nonce, block_hash, _) = self.get_access_key_nonce(signer_account, &public_key.to_string())
            .map_err(|e| {
                eprintln!("[RPC_PROXY] Failed to get access key nonce: {}", e);
                e
            })?;

        eprintln!("[RPC_PROXY] Got nonce={}, block_hash={}", nonce, block_hash);

        let tx = Transaction::V0(TransactionV0 {
            signer_id,
            public_key,
            nonce: nonce + 1,
            receiver_id: receiver,
            block_hash,
            actions,
        });

        // Get transaction hash before signing (this is what will be the final tx hash)
        let tx_hash = tx.get_hash_and_size().0;
        let tx_hash_base58 = bs58::encode(tx_hash.as_ref()).into_string();

        eprintln!("[RPC_PROXY] Transaction hash: {}", tx_hash_base58);

        // Sign transaction
        let signature = signer.sign(tx_hash.as_ref());
        let signed_tx = SignedTransaction::new(signature, tx);

        // Serialize and encode
        let signed_tx_bytes = borsh::to_vec(&signed_tx)
            .map_err(|e| {
                eprintln!("[RPC_PROXY] Failed to serialize tx: {}", e);
                anyhow::anyhow!("Failed to serialize tx: {}", e)
            })?;
        let signed_tx_base64 = base64::engine::general_purpose::STANDARD.encode(&signed_tx_bytes);

        eprintln!("[RPC_PROXY] About to send transaction to RPC...");

        // Send transaction and wait for finalization
        let result = self.send_tx(&signed_tx_base64, Some("FINAL"))
            .map_err(|e| {
                eprintln!("[RPC_PROXY] Failed to send tx: {}", e);
                e
            })?;

        eprintln!("[RPC_PROXY] Transaction sent, checking status...");

        // Print full result for debugging
        eprintln!("[RPC_PROXY] Full RPC result: {}", serde_json::to_string_pretty(&result).unwrap_or_else(|_| format!("{:?}", result)));

        // Check for RPC-level error first (e.g., invalid transaction, not enough balance)
        if let Some(error) = result.get("error") {
            let error_msg = if let Some(msg) = error.get("message").and_then(|m| m.as_str()) {
                msg.to_string()
            } else {
                format!("{:?}", error)
            };
            eprintln!("[RPC_PROXY] RPC error: {}", error_msg);
            return Err(anyhow::anyhow!("RPC error: {}", error_msg));
        }

        // Check if transaction was successful - check both top-level and nested status
        if let Some(status) = result.get("result").and_then(|r| r.get("status")) {
            // Check for Failure variant
            if let Some(failure) = status.get("Failure") {
                eprintln!("[RPC_PROXY] Transaction failed: {:?}", failure);
                return Err(anyhow::anyhow!("Transaction failed: {:?}", failure));
            }
        } else if let Some(status) = result.get("status") {
            // Check top-level status (some RPC responses have it here)
            if let Some(failure) = status.get("Failure") {
                eprintln!("[RPC_PROXY] Transaction failed (top-level): {:?}", failure);
                return Err(anyhow::anyhow!("Transaction failed: {:?}", failure));
            }
        }

        eprintln!("[RPC_PROXY] Transaction completed successfully: {}", tx_hash_base58);

        // Return the transaction hash we computed (not from RPC response)
        Ok(tx_hash_base58)
    }
}

/// Host state for RPC host functions
pub struct RpcHostState {
    pub proxy: RpcProxy,
}

impl RpcHostState {
    pub fn new(proxy: RpcProxy) -> Self {
        Self { proxy }
    }
}

/// Implement the generated Host trait for RpcHostState
impl near::rpc::api::Host for RpcHostState {
    fn view(&mut self, contract_id: String, method_name: String, args_json: String) -> (String, String) {
        let args_base64 = base64::engine::general_purpose::STANDARD.encode(args_json.as_bytes());

        match self.proxy.call_function(&contract_id, &method_name, &args_base64) {
            Ok(result) => {
                if let Some(result_array) = result.get("result").and_then(|r| r.get("result")) {
                    if let Some(arr) = result_array.as_array() {
                        let bytes: Vec<u8> = arr
                            .iter()
                            .filter_map(|v| v.as_u64().map(|n| n as u8))
                            .collect();
                        return (String::from_utf8_lossy(&bytes).to_string(), String::new());
                    }
                }
                (serde_json::to_string(&result).unwrap_or_default(), String::new())
            }
            Err(e) => (String::new(), e.to_string()),
        }
    }

    fn view_account(&mut self, account_id: String) -> (String, String) {
        match self.proxy.view_account(&account_id) {
            Ok(result) => (serde_json::to_string(&result).unwrap_or_default(), String::new()),
            Err(e) => (String::new(), e.to_string()),
        }
    }

    fn view_access_key(&mut self, account_id: String, public_key: String) -> (String, String) {
        match self.proxy.view_access_key(&account_id, &public_key) {
            Ok(result) => (serde_json::to_string(&result).unwrap_or_default(), String::new()),
            Err(e) => (String::new(), e.to_string()),
        }
    }

    fn block(&mut self, finality_or_block_id: String) -> (String, String) {
        let (finality, block_id) =
            if finality_or_block_id == "final" || finality_or_block_id == "optimistic" {
                (Some(finality_or_block_id.as_str()), None)
            } else if let Ok(height) = finality_or_block_id.parse::<u64>() {
                (None, Some(json!(height)))
            } else {
                (None, Some(json!(finality_or_block_id)))
            };

        match self.proxy.block(finality, block_id) {
            Ok(result) => (serde_json::to_string(&result).unwrap_or_default(), String::new()),
            Err(e) => (String::new(), e.to_string()),
        }
    }

    fn gas_price(&mut self) -> (String, String) {
        match self.proxy.gas_price() {
            Ok(result) => {
                if let Some(price) = result.get("result").and_then(|r| r.get("gas_price")) {
                    return (price.to_string(), String::new());
                }
                (serde_json::to_string(&result).unwrap_or_default(), String::new())
            }
            Err(e) => (String::new(), e.to_string()),
        }
    }

    fn send_tx(&mut self, signed_tx_base64: String, wait_until: String) -> (String, String) {
        let wait = if wait_until.is_empty() { None } else { Some(wait_until.as_str()) };

        match self.proxy.send_tx(&signed_tx_base64, wait) {
            Ok(result) => (serde_json::to_string(&result).unwrap_or_default(), String::new()),
            Err(e) => (String::new(), e.to_string()),
        }
    }

    fn raw(&mut self, method: String, params_json: String) -> (String, String) {
        let params: Value = serde_json::from_str(&params_json).unwrap_or(json!([]));

        match self.proxy.call_method(&method, params) {
            Ok(result) => (serde_json::to_string(&result).unwrap_or_default(), String::new()),
            Err(e) => (String::new(), e.to_string()),
        }
    }

    /// CRITICAL: Worker NEVER signs with its own key!
    /// This function receives signer_id and signer_key FROM WASM (via secrets).
    /// Worker only proxies the transaction - does NOT use its own credentials.
    fn call(
        &mut self,
        signer_id: String,        // From WASM (user-provided)
        signer_key: String,       // From WASM (user-provided via secrets)
        receiver_id: String,
        method_name: String,
        args_json: String,
        deposit_yocto: String,
        gas: String,
    ) -> (String, String) {
        eprintln!("[HOST] call() invoked: signer={}, receiver={}, method={}, deposit={}, gas={}",
            signer_id, receiver_id, method_name, deposit_yocto, gas);

        let deposit: u128 = match deposit_yocto.parse() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("[HOST] Invalid deposit: {}", e);
                return (String::new(), format!("Invalid deposit: {}", e));
            }
        };

        let gas_amount: u64 = match gas.parse() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("[HOST] Invalid gas: {}", e);
                return (String::new(), format!("Invalid gas: {}", e));
            }
        };

        eprintln!("[HOST] Creating FunctionCall action...");

        let action = Action::FunctionCall(Box::new(FunctionCallAction {
            method_name: method_name.clone(),
            args: args_json.clone().into_bytes(),
            gas: gas_amount,
            deposit,
        }));

        eprintln!("[HOST] Calling sign_and_send_tx_as...");

        match self.proxy.sign_and_send_tx_as(&signer_id, &signer_key, &receiver_id, vec![action]) {
            Ok(tx_hash) => {
                eprintln!("[HOST] Transaction successful: {}", tx_hash);
                (tx_hash, String::new())
            }
            Err(e) => {
                eprintln!("[HOST] Transaction failed: {}", e);
                (String::new(), e.to_string())
            }
        }
    }

    /// CRITICAL: Worker NEVER signs with its own key!
    /// This function receives signer_id and signer_key FROM WASM (via secrets).
    /// Worker only proxies the transaction - does NOT use its own credentials.
    fn transfer(
        &mut self,
        signer_id: String,        // From WASM (user-provided)
        signer_key: String,       // From WASM (user-provided via secrets)
        receiver_id: String,
        amount_yocto: String,
    ) -> (String, String) {
        let amount: u128 = match amount_yocto.parse() {
            Ok(a) => a,
            Err(e) => return (String::new(), format!("Invalid amount: {}", e)),
        };

        let action = Action::Transfer(TransferAction { deposit: amount });

        match self.proxy.sign_and_send_tx_as(&signer_id, &signer_key, &receiver_id, vec![action]) {
            Ok(tx_hash) => (tx_hash, String::new()),
            Err(e) => (String::new(), e.to_string()),
        }
    }
}

/// Add NEAR RPC host functions to a wasmtime component linker
pub fn add_rpc_to_linker<T: Send + 'static>(
    linker: &mut Linker<T>,
    get_state: impl Fn(&mut T) -> &mut RpcHostState + Send + Sync + Copy + 'static,
) -> anyhow::Result<()> {
    near::rpc::api::add_to_linker(linker, get_state)
}
