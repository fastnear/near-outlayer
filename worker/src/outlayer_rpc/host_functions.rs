//! WASI P2 Host Functions for NEAR RPC
//!
//! This module provides host function bindings that WASM code can call
//! to make NEAR RPC requests through the worker's proxy.
//!
//! ## WIT Interface
//!
//! The guest crate (near-rpc-guest) defines a WIT interface that matches
//! these host functions. WASM code imports the interface and calls functions
//! that are implemented here.
//!
//! ## Function Naming Convention
//!
//! All functions are prefixed with `near_rpc_` to avoid conflicts:
//! - `near_rpc_view` - View call (call_function)
//! - `near_rpc_view_account` - Get account info
//! - `near_rpc_view_access_key` - Get access key info
//! - `near_rpc_block` - Get block
//! - `near_rpc_gas_price` - Get gas price
//! - `near_rpc_send_tx` - Send signed transaction
//! - `near_rpc_raw` - Raw JSON-RPC call (for unsupported methods)

use std::sync::Arc;
use tokio::sync::Mutex;
use wasmtime::component::*;
use base64::Engine;

use super::RpcProxy;

/// Host state containing RPC proxy and runtime
pub struct RpcHostState {
    pub proxy: Arc<Mutex<RpcProxy>>,
    pub runtime: tokio::runtime::Handle,
}

impl RpcHostState {
    pub fn new(proxy: RpcProxy, runtime: tokio::runtime::Handle) -> Self {
        Self {
            proxy: Arc::new(Mutex::new(proxy)),
            runtime,
        }
    }
}

/// Result type for host functions - returns (ok_value, err_value) tuple
/// If ok_value is non-empty, it succeeded; if err_value is non-empty, it failed
type RpcResult = (String, String);

/// Add NEAR RPC host functions to a wasmtime component linker
///
/// This adds the `near:rpc/api` interface functions that WASM components can import.
///
/// # Arguments
/// * `linker` - Wasmtime component linker
///
/// # Example WIT interface (for guest crate):
/// ```wit
/// package near:rpc;
///
/// interface api {
///     // View call - returns JSON result or error
///     // Returns (ok_result, err_message) - check err_message first
///     view: func(contract-id: string, method-name: string, args-json: string) -> tuple<string, string>;
///
///     // Account info - returns JSON or error
///     view-account: func(account-id: string) -> tuple<string, string>;
///
///     // Access key - returns JSON or error
///     view-access-key: func(account-id: string, public-key: string) -> tuple<string, string>;
///
///     // Block info - returns JSON or error
///     block: func(finality-or-block-id: string) -> tuple<string, string>;
///
///     // Gas price - returns price string or error
///     gas-price: func() -> tuple<string, string>;
///
///     // Send signed transaction - returns JSON or error
///     send-tx: func(signed-tx-base64: string, wait-until: string) -> tuple<string, string>;
///
///     // Raw JSON-RPC call - returns JSON or error
///     raw: func(method: string, params-json: string) -> tuple<string, string>;
///
///     // Call contract method with transaction (WASM provides signing key)
///     call: func(signer-id: string, signer-key: string, receiver-id: string,
///                method-name: string, args-json: string, deposit-yocto: string,
///                gas: string) -> tuple<string, string>;
///
///     // Transfer NEAR tokens (WASM provides signing key)
///     transfer: func(signer-id: string, signer-key: string, receiver-id: string,
///                    amount-yocto: string) -> tuple<string, string>;
/// }
///
/// world near-rpc-guest {
///     import near:rpc/api;
/// }
/// ```
pub fn add_rpc_to_linker<T: Send + 'static>(
    linker: &mut Linker<T>,
    get_state: impl Fn(&mut T) -> &mut RpcHostState + Send + Sync + Copy + 'static,
) -> anyhow::Result<()> {
    tracing::info!("🔧 Adding NEAR RPC host functions to linker...");

    // Define the interface instance
    let mut interface = linker.instance("near:rpc/api")?;
    tracing::debug!("   Created interface instance: near:rpc/api");

    // near_rpc_view: Call a view function on a contract
    interface.func_wrap_async(
        "view",
        move |mut caller: wasmtime::StoreContextMut<'_, T>,
              (contract_id, method_name, args_json): (String, String, String)| {
            Box::new(async move {
                let state = get_state(caller.data_mut());
                let proxy = state.proxy.clone();
                let runtime = state.runtime.clone();

                let result: RpcResult = runtime
                    .spawn(async move {
                        let proxy = proxy.lock().await;

                        // Encode args to base64
                        let args_base64 = base64::engine::general_purpose::STANDARD
                            .encode(args_json.as_bytes());

                        match proxy
                            .call_function(&contract_id, &method_name, &args_base64, Some("final"), None)
                            .await
                        {
                            Ok(result) => {
                                // Extract result bytes and decode
                                if let Some(result_array) = result.get("result").and_then(|r| r.get("result")) {
                                    if let Some(arr) = result_array.as_array() {
                                        let bytes: Vec<u8> = arr
                                            .iter()
                                            .filter_map(|v| v.as_u64().map(|n| n as u8))
                                            .collect();
                                        return (String::from_utf8_lossy(&bytes).to_string(), String::new());
                                    }
                                }
                                // Return full response if result extraction failed
                                (serde_json::to_string(&result).unwrap_or_default(), String::new())
                            }
                            Err(e) => (String::new(), e.to_string()),
                        }
                    })
                    .await
                    .unwrap_or_else(|e| (String::new(), format!("spawn error: {}", e)));

                Ok(result)
            })
        },
    )?;

    // near_rpc_view_account: Get account information
    interface.func_wrap_async(
        "view-account",
        move |mut caller: wasmtime::StoreContextMut<'_, T>, (account_id,): (String,)| {
            Box::new(async move {
                let state = get_state(caller.data_mut());
                let proxy = state.proxy.clone();
                let runtime = state.runtime.clone();

                let result: RpcResult = runtime
                    .spawn(async move {
                        let proxy = proxy.lock().await;
                        match proxy.view_account(&account_id, Some("final"), None).await {
                            Ok(result) => (serde_json::to_string(&result).unwrap_or_default(), String::new()),
                            Err(e) => (String::new(), e.to_string()),
                        }
                    })
                    .await
                    .unwrap_or_else(|e| (String::new(), format!("spawn error: {}", e)));

                Ok(result)
            })
        },
    )?;

    // near_rpc_view_access_key: Get access key information
    interface.func_wrap_async(
        "view-access-key",
        move |mut caller: wasmtime::StoreContextMut<'_, T>,
              (account_id, public_key): (String, String)| {
            Box::new(async move {
                let state = get_state(caller.data_mut());
                let proxy = state.proxy.clone();
                let runtime = state.runtime.clone();

                let result: RpcResult = runtime
                    .spawn(async move {
                        let proxy = proxy.lock().await;
                        match proxy
                            .view_access_key(&account_id, &public_key, Some("final"), None)
                            .await
                        {
                            Ok(result) => (serde_json::to_string(&result).unwrap_or_default(), String::new()),
                            Err(e) => (String::new(), e.to_string()),
                        }
                    })
                    .await
                    .unwrap_or_else(|e| (String::new(), format!("spawn error: {}", e)));

                Ok(result)
            })
        },
    )?;

    // near_rpc_block: Get block information
    interface.func_wrap_async(
        "block",
        move |mut caller: wasmtime::StoreContextMut<'_, T>,
              (finality_or_block_id,): (String,)| {
            Box::new(async move {
                let state = get_state(caller.data_mut());
                let proxy = state.proxy.clone();
                let runtime = state.runtime.clone();

                let result: RpcResult = runtime
                    .spawn(async move {
                        let proxy = proxy.lock().await;

                        // Parse finality_or_block_id
                        let (finality, block_id) = if finality_or_block_id == "final"
                            || finality_or_block_id == "optimistic"
                        {
                            (Some(finality_or_block_id.as_str()), None)
                        } else if let Ok(height) = finality_or_block_id.parse::<u64>() {
                            (None, Some(serde_json::json!(height)))
                        } else {
                            (None, Some(serde_json::json!(finality_or_block_id)))
                        };

                        match proxy.block(finality, block_id).await {
                            Ok(result) => (serde_json::to_string(&result).unwrap_or_default(), String::new()),
                            Err(e) => (String::new(), e.to_string()),
                        }
                    })
                    .await
                    .unwrap_or_else(|e| (String::new(), format!("spawn error: {}", e)));

                Ok(result)
            })
        },
    )?;

    // near_rpc_gas_price: Get current gas price
    interface.func_wrap_async(
        "gas-price",
        move |mut caller: wasmtime::StoreContextMut<'_, T>, (): ()| {
            Box::new(async move {
                let state = get_state(caller.data_mut());
                let proxy = state.proxy.clone();
                let runtime = state.runtime.clone();

                let result: RpcResult = runtime
                    .spawn(async move {
                        let proxy = proxy.lock().await;
                        match proxy.gas_price(None).await {
                            Ok(result) => {
                                // Extract gas_price value
                                if let Some(price) = result.get("result").and_then(|r| r.get("gas_price")) {
                                    return (price.to_string(), String::new());
                                }
                                (serde_json::to_string(&result).unwrap_or_default(), String::new())
                            }
                            Err(e) => (String::new(), e.to_string()),
                        }
                    })
                    .await
                    .unwrap_or_else(|e| (String::new(), format!("spawn error: {}", e)));

                Ok(result)
            })
        },
    )?;

    // near_rpc_send_tx: Send a signed transaction
    interface.func_wrap_async(
        "send-tx",
        move |mut caller: wasmtime::StoreContextMut<'_, T>,
              (signed_tx_base64, wait_until): (String, String)| {
            Box::new(async move {
                let state = get_state(caller.data_mut());
                let proxy = state.proxy.clone();
                let runtime = state.runtime.clone();

                let result: RpcResult = runtime
                    .spawn(async move {
                        let proxy = proxy.lock().await;

                        let wait = if wait_until.is_empty() {
                            None
                        } else {
                            Some(wait_until.as_str())
                        };

                        match proxy.send_tx(&signed_tx_base64, wait).await {
                            Ok(result) => (serde_json::to_string(&result).unwrap_or_default(), String::new()),
                            Err(e) => (String::new(), e.to_string()),
                        }
                    })
                    .await
                    .unwrap_or_else(|e| (String::new(), format!("spawn error: {}", e)));

                Ok(result)
            })
        },
    )?;

    // near_rpc_raw: Raw JSON-RPC call for any method
    interface.func_wrap_async(
        "raw",
        move |mut caller: wasmtime::StoreContextMut<'_, T>,
              (method, params_json): (String, String)| {
            Box::new(async move {
                let state = get_state(caller.data_mut());
                let proxy = state.proxy.clone();
                let runtime = state.runtime.clone();

                let result: RpcResult = runtime
                    .spawn(async move {
                        let proxy = proxy.lock().await;

                        let params: serde_json::Value =
                            serde_json::from_str(&params_json).unwrap_or(serde_json::json!([]));

                        match proxy.call_method(&method, params).await {
                            Ok(result) => (serde_json::to_string(&result).unwrap_or_default(), String::new()),
                            Err(e) => (String::new(), e.to_string()),
                        }
                    })
                    .await
                    .unwrap_or_else(|e| (String::new(), format!("spawn error: {}", e)));

                Ok(result)
            })
        },
    )?;

    // near_rpc_call: Call a contract method with transaction (WASM provides signing key)
    interface.func_wrap_async(
        "call",
        move |mut caller: wasmtime::StoreContextMut<'_, T>,
              (signer_id, signer_key, receiver_id, method_name, args_json, deposit_yocto, gas): (String, String, String, String, String, String, String)| {
            Box::new(async move {
                let state = get_state(caller.data_mut());
                let proxy = state.proxy.clone();
                let runtime = state.runtime.clone();

                let result: RpcResult = runtime
                    .spawn(async move {
                        let proxy = proxy.lock().await;

                        // Call method through proxy (proxy will create and sign transaction)
                        match proxy.call_contract_method(
                            &signer_id,
                            &signer_key,
                            &receiver_id,
                            &method_name,
                            &args_json,
                            &deposit_yocto,
                            &gas,
                        ).await {
                            Ok(tx_hash) => (tx_hash, String::new()),
                            Err(e) => (String::new(), e.to_string()),
                        }
                    })
                    .await
                    .unwrap_or_else(|e| (String::new(), format!("spawn error: {}", e)));

                Ok(result)
            })
        },
    )?;

    // near_rpc_transfer: Transfer NEAR tokens (WASM provides signing key)
    interface.func_wrap_async(
        "transfer",
        move |mut caller: wasmtime::StoreContextMut<'_, T>,
              (signer_id, signer_key, receiver_id, amount_yocto): (String, String, String, String)| {
            Box::new(async move {
                let state = get_state(caller.data_mut());
                let proxy = state.proxy.clone();
                let runtime = state.runtime.clone();

                let result: RpcResult = runtime
                    .spawn(async move {
                        let proxy = proxy.lock().await;

                        // Transfer through proxy (proxy will create and sign transaction)
                        match proxy.transfer(&signer_id, &signer_key, &receiver_id, &amount_yocto).await {
                            Ok(tx_hash) => (tx_hash, String::new()),
                            Err(e) => (String::new(), e.to_string()),
                        }
                    })
                    .await
                    .unwrap_or_else(|e| (String::new(), format!("spawn error: {}", e)));

                Ok(result)
            })
        },
    )?;

    tracing::info!("✅ Added all NEAR RPC host functions:");
    tracing::info!("   - view");
    tracing::info!("   - view-account");
    tracing::info!("   - view-access-key");
    tracing::info!("   - block");
    tracing::info!("   - gas-price");
    tracing::info!("   - send-tx");
    tracing::info!("   - raw");
    tracing::info!("   - call");
    tracing::info!("   - transfer");

    Ok(())
}

#[cfg(test)]
mod tests {
    // Host function tests require full wasmtime setup
    // Integration tests will be in a separate file
}
