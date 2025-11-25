//! Integration tests for OutLayer RPC Proxy
//!
//! These tests compare RPC proxy responses with direct NEAR RPC responses
//! to ensure compatibility and correctness.
//!
//! Run with: cargo test --test outlayer_rpc_test -- --ignored --nocapture
//!
//! Note: Tests marked with #[ignore] require network access to NEAR testnet RPC.
//! They are skipped by default in CI but can be run manually for verification.

use offchainvm_worker::config::RpcProxyConfig;
use offchainvm_worker::outlayer_rpc::RpcProxy;
use serde_json::Value;

/// Test RPC URL for NEAR testnet
const TESTNET_RPC_URL: &str = "https://rpc.testnet.near.org";

/// Well-known testnet accounts for testing
const TEST_ACCOUNT: &str = "outlayer.testnet";
const TEST_CONTRACT: &str = "wrap.testnet";

fn create_test_proxy() -> RpcProxy {
    let config = RpcProxyConfig {
        enabled: true,
        rpc_url: Some(TESTNET_RPC_URL.to_string()),
        max_calls_per_execution: 100,
        allow_transactions: true,
    };
    RpcProxy::new(config, TESTNET_RPC_URL).unwrap()
}

fn create_direct_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap()
}

/// Make a direct RPC call without using the proxy
async fn direct_rpc_call(client: &reqwest::Client, method: &str, params: Value) -> Value {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "direct",
        "method": method,
        "params": params
    });

    let response = client
        .post(TESTNET_RPC_URL)
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .expect("Failed to send direct RPC request");

    response.json().await.expect("Failed to parse direct RPC response")
}

// ============================================================================
// View Account Tests
// ============================================================================

#[tokio::test]
#[ignore] // Requires network access
async fn test_view_account_matches_direct_rpc() {
    let proxy = create_test_proxy();
    let client = create_direct_client();

    // Via proxy
    let proxy_response = proxy
        .view_account(TEST_ACCOUNT, Some("final"), None)
        .await
        .expect("Proxy view_account failed");

    // Direct RPC
    let direct_params = serde_json::json!({
        "request_type": "view_account",
        "account_id": TEST_ACCOUNT,
        "finality": "final"
    });
    let direct_response = direct_rpc_call(&client, "query", direct_params).await;

    // Compare results
    let proxy_result = proxy_response.get("result");
    let direct_result = direct_response.get("result");

    assert!(proxy_result.is_some(), "Proxy should return result");
    assert!(direct_result.is_some(), "Direct should return result");

    // Compare account data (amount may change between calls, so check structure)
    let proxy_account = proxy_result.unwrap();
    let direct_account = direct_result.unwrap();

    assert!(proxy_account.get("amount").is_some(), "Proxy should return amount");
    assert!(direct_account.get("amount").is_some(), "Direct should return amount");
    assert!(proxy_account.get("code_hash").is_some(), "Proxy should return code_hash");

    println!("Proxy response: {}", serde_json::to_string_pretty(&proxy_response).unwrap());
    println!("Direct response: {}", serde_json::to_string_pretty(&direct_response).unwrap());
}

#[tokio::test]
#[ignore] // Requires network access
async fn test_view_account_nonexistent() {
    let proxy = create_test_proxy();

    let result = proxy
        .view_account("nonexistent-account-12345.testnet", Some("final"), None)
        .await;

    // Should succeed but return an error in the response
    assert!(result.is_ok(), "RPC call should succeed");
    let response = result.unwrap();

    // Check for error in response
    let error = response.get("error");
    assert!(error.is_some(), "Should return error for nonexistent account");

    println!("Error response: {}", serde_json::to_string_pretty(&response).unwrap());
}

// ============================================================================
// Block Tests
// ============================================================================

#[tokio::test]
#[ignore] // Requires network access
async fn test_block_final_matches_direct_rpc() {
    let proxy = create_test_proxy();
    let client = create_direct_client();

    // Via proxy
    let proxy_response = proxy
        .block(Some("final"), None)
        .await
        .expect("Proxy block failed");

    // Direct RPC
    let direct_params = serde_json::json!({ "finality": "final" });
    let direct_response = direct_rpc_call(&client, "block", direct_params).await;

    // Compare structure (block content will differ due to timing)
    let proxy_result = proxy_response.get("result");
    let direct_result = direct_response.get("result");

    assert!(proxy_result.is_some(), "Proxy should return result");
    assert!(direct_result.is_some(), "Direct should return result");

    // Check required block fields exist
    let proxy_block = proxy_result.unwrap();
    assert!(proxy_block.get("header").is_some(), "Should have header");
    assert!(proxy_block.get("chunks").is_some(), "Should have chunks");

    println!("Proxy block height: {}",
        proxy_block.get("header").and_then(|h| h.get("height")).unwrap_or(&Value::Null));
}

#[tokio::test]
#[ignore] // Requires network access
async fn test_block_by_height() {
    let proxy = create_test_proxy();

    // First get latest block to find a valid height
    let latest = proxy
        .block(Some("final"), None)
        .await
        .expect("Failed to get latest block");

    let height = latest
        .get("result")
        .and_then(|r| r.get("header"))
        .and_then(|h| h.get("height"))
        .and_then(|h| h.as_u64())
        .expect("Failed to extract block height");

    // Now fetch by specific height
    let by_height = proxy
        .block(None, Some(serde_json::json!(height)))
        .await
        .expect("Failed to get block by height");

    let result_height = by_height
        .get("result")
        .and_then(|r| r.get("header"))
        .and_then(|h| h.get("height"))
        .and_then(|h| h.as_u64());

    assert_eq!(result_height, Some(height), "Block height should match");
}

// ============================================================================
// Gas Price Tests
// ============================================================================

#[tokio::test]
#[ignore] // Requires network access
async fn test_gas_price_matches_direct_rpc() {
    let proxy = create_test_proxy();
    let client = create_direct_client();

    // Via proxy
    let proxy_response = proxy
        .gas_price(None)
        .await
        .expect("Proxy gas_price failed");

    // Direct RPC
    let direct_response = direct_rpc_call(&client, "gas_price", serde_json::json!([null])).await;

    // Compare results
    let proxy_result = proxy_response.get("result");
    let direct_result = direct_response.get("result");

    assert!(proxy_result.is_some(), "Proxy should return result");
    assert!(direct_result.is_some(), "Direct should return result");

    // Gas price should be a string number
    let proxy_price = proxy_result.and_then(|r| r.get("gas_price"));
    assert!(proxy_price.is_some(), "Should return gas_price");

    println!("Gas price: {}", proxy_price.unwrap());
}

// ============================================================================
// View Function (call_function) Tests
// ============================================================================

#[tokio::test]
#[ignore] // Requires network access
async fn test_call_function_ft_balance() {
    let proxy = create_test_proxy();

    // Call ft_balance_of on wrap.testnet
    let args = serde_json::json!({ "account_id": TEST_ACCOUNT });
    let args_base64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        args.to_string().as_bytes()
    );

    let response = proxy
        .call_function(TEST_CONTRACT, "ft_balance_of", &args_base64, Some("final"), None)
        .await
        .expect("call_function failed");

    // Should return result or error
    if let Some(result) = response.get("result") {
        // Decode the result bytes
        if let Some(result_bytes) = result.get("result").and_then(|r| r.as_array()) {
            let bytes: Vec<u8> = result_bytes
                .iter()
                .filter_map(|v| v.as_u64().map(|n| n as u8))
                .collect();
            let result_str = String::from_utf8_lossy(&bytes);
            println!("FT balance result: {}", result_str);
        }
    } else if let Some(error) = response.get("error") {
        println!("FT balance error (expected if account has no balance): {}", error);
    }
}

#[tokio::test]
#[ignore] // Requires network access
async fn test_call_function_matches_direct_rpc() {
    let proxy = create_test_proxy();
    let client = create_direct_client();

    let args = serde_json::json!({});
    let args_base64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        args.to_string().as_bytes()
    );

    // Via proxy - get ft_metadata
    let proxy_response = proxy
        .call_function(TEST_CONTRACT, "ft_metadata", &args_base64, Some("final"), None)
        .await
        .expect("Proxy call_function failed");

    // Direct RPC
    let direct_params = serde_json::json!({
        "request_type": "call_function",
        "account_id": TEST_CONTRACT,
        "method_name": "ft_metadata",
        "args_base64": args_base64,
        "finality": "final"
    });
    let direct_response = direct_rpc_call(&client, "query", direct_params).await;

    // Compare result structure
    let proxy_result = proxy_response.get("result").and_then(|r| r.get("result"));
    let direct_result = direct_response.get("result").and_then(|r| r.get("result"));

    assert!(proxy_result.is_some(), "Proxy should return result.result");
    assert!(direct_result.is_some(), "Direct should return result.result");

    // Decode and compare
    if let (Some(proxy_arr), Some(direct_arr)) = (proxy_result.and_then(|r| r.as_array()), direct_result.and_then(|r| r.as_array())) {
        let proxy_bytes: Vec<u8> = proxy_arr.iter().filter_map(|v| v.as_u64().map(|n| n as u8)).collect();
        let direct_bytes: Vec<u8> = direct_arr.iter().filter_map(|v| v.as_u64().map(|n| n as u8)).collect();

        // Results should be identical (same block finality)
        assert_eq!(proxy_bytes, direct_bytes, "Results should match");

        let result_str = String::from_utf8_lossy(&proxy_bytes);
        println!("FT metadata: {}", result_str);
    }
}

// ============================================================================
// Access Key Tests
// ============================================================================

#[tokio::test]
#[ignore] // Requires network access
async fn test_view_access_key_list() {
    let proxy = create_test_proxy();

    let response = proxy
        .view_access_key_list(TEST_ACCOUNT, Some("final"), None)
        .await
        .expect("view_access_key_list failed");

    let result = response.get("result");
    assert!(result.is_some(), "Should return result");

    let keys = result.and_then(|r| r.get("keys")).and_then(|k| k.as_array());
    assert!(keys.is_some(), "Should return keys array");

    println!("Account {} has {} access keys", TEST_ACCOUNT, keys.unwrap().len());
}

// ============================================================================
// Network Status Tests
// ============================================================================

#[tokio::test]
#[ignore] // Requires network access
async fn test_status_matches_direct_rpc() {
    let proxy = create_test_proxy();
    let client = create_direct_client();

    // Via proxy
    let proxy_response = proxy
        .status()
        .await
        .expect("Proxy status failed");

    // Direct RPC
    let direct_response = direct_rpc_call(&client, "status", serde_json::json!([])).await;

    // Compare structure
    let proxy_result = proxy_response.get("result");
    let direct_result = direct_response.get("result");

    assert!(proxy_result.is_some(), "Proxy should return result");
    assert!(direct_result.is_some(), "Direct should return result");

    // Check required fields
    let proxy_status = proxy_result.unwrap();
    assert!(proxy_status.get("chain_id").is_some(), "Should have chain_id");
    assert!(proxy_status.get("sync_info").is_some(), "Should have sync_info");

    let chain_id = proxy_status.get("chain_id").and_then(|c| c.as_str());
    assert_eq!(chain_id, Some("testnet"), "Should be testnet");

    println!("Node version: {}", proxy_status.get("version").unwrap_or(&Value::Null));
}

// ============================================================================
// Rate Limiting Tests
// ============================================================================

#[tokio::test]
async fn test_rate_limiting() {
    let config = RpcProxyConfig {
        enabled: true,
        rpc_url: Some(TESTNET_RPC_URL.to_string()),
        max_calls_per_execution: 3,
        allow_transactions: false,
    };
    let proxy = RpcProxy::new(config, TESTNET_RPC_URL).unwrap();

    // First 3 calls should increment counter
    assert_eq!(proxy.get_call_count(), 0);

    // Simulate calls by accessing internal rate limit check
    // We'll use gas_price since it's simple (but will fail network-wise)
    let _ = proxy.gas_price(None).await; // call 1
    assert_eq!(proxy.get_call_count(), 1);

    let _ = proxy.gas_price(None).await; // call 2
    assert_eq!(proxy.get_call_count(), 2);

    let _ = proxy.gas_price(None).await; // call 3
    assert_eq!(proxy.get_call_count(), 3);

    // 4th call should fail with rate limit
    let result = proxy.gas_price(None).await;
    assert!(result.is_err(), "Should fail on rate limit");
    assert!(result.unwrap_err().to_string().contains("rate limit"), "Error should mention rate limit");
}

#[tokio::test]
async fn test_rate_limit_reset() {
    let config = RpcProxyConfig {
        enabled: true,
        rpc_url: Some(TESTNET_RPC_URL.to_string()),
        max_calls_per_execution: 2,
        allow_transactions: false,
    };
    let proxy = RpcProxy::new(config, TESTNET_RPC_URL).unwrap();

    // Make 2 calls
    let _ = proxy.gas_price(None).await;
    let _ = proxy.gas_price(None).await;
    assert_eq!(proxy.get_call_count(), 2);

    // Reset
    proxy.reset_call_count();
    assert_eq!(proxy.get_call_count(), 0);

    // Should be able to make calls again
    let _ = proxy.gas_price(None).await;
    assert_eq!(proxy.get_call_count(), 1);
}

// ============================================================================
// Transaction Method Blocking Tests
// ============================================================================

#[tokio::test]
async fn test_transaction_methods_blocked() {
    let config = RpcProxyConfig {
        enabled: true,
        rpc_url: Some(TESTNET_RPC_URL.to_string()),
        max_calls_per_execution: 100,
        allow_transactions: false, // Transactions disabled
    };
    let proxy = RpcProxy::new(config, TESTNET_RPC_URL).unwrap();

    // send_tx should be blocked
    let result = proxy.send_tx("fake_base64_tx", Some("EXECUTED")).await;
    assert!(result.is_err(), "send_tx should be blocked");
    assert!(result.unwrap_err().to_string().contains("disabled"), "Error should mention disabled");

    // broadcast_tx_async should be blocked
    let result = proxy.broadcast_tx_async("fake_base64_tx").await;
    assert!(result.is_err(), "broadcast_tx_async should be blocked");

    // broadcast_tx_commit should be blocked
    let result = proxy.broadcast_tx_commit("fake_base64_tx").await;
    assert!(result.is_err(), "broadcast_tx_commit should be blocked");
}

#[tokio::test]
async fn test_transaction_methods_allowed() {
    let config = RpcProxyConfig {
        enabled: true,
        rpc_url: Some(TESTNET_RPC_URL.to_string()),
        max_calls_per_execution: 100,
        allow_transactions: true, // Transactions enabled
    };
    let proxy = RpcProxy::new(config, TESTNET_RPC_URL).unwrap();

    // send_tx should NOT be blocked (will fail for other reasons - invalid tx)
    let result = proxy.send_tx("ZmFrZV90eA==", Some("NONE")).await;
    // Should get past the permission check and fail on actual RPC
    // The error should NOT be about "disabled"
    assert!(result.is_ok() || !result.as_ref().unwrap_err().to_string().contains("disabled"));
}

// ============================================================================
// Error Response Format Tests
// ============================================================================

#[tokio::test]
#[ignore] // Requires network access
async fn test_rpc_error_format() {
    let proxy = create_test_proxy();

    // Call with invalid method
    let response = proxy
        .call_function("wrap.testnet", "nonexistent_method", "e30=", Some("final"), None)
        .await;

    // Should succeed at HTTP level but contain RPC error
    assert!(response.is_ok(), "HTTP request should succeed");

    let response = response.unwrap();

    // Check error structure matches official RPC format
    if let Some(error) = response.get("error") {
        assert!(error.get("cause").is_some() || error.get("code").is_some(),
            "Error should have cause or code field");
        println!("RPC error format: {}", serde_json::to_string_pretty(&error).unwrap());
    }
}

// ============================================================================
// Validators Test
// ============================================================================

#[tokio::test]
#[ignore] // Requires network access
async fn test_validators() {
    let proxy = create_test_proxy();

    let response = proxy
        .validators(None)
        .await
        .expect("validators call failed");

    let result = response.get("result");
    assert!(result.is_some(), "Should return result");

    // Check for current/next validators
    let validators = result.unwrap();
    assert!(validators.get("current_validators").is_some() || validators.get("epoch_height").is_some(),
        "Should have validator info");

    println!("Validators response structure: {:?}", validators.as_object().map(|o| o.keys().collect::<Vec<_>>()));
}

// ============================================================================
// Protocol Config Test
// ============================================================================

#[tokio::test]
#[ignore] // Requires network access
async fn test_protocol_config() {
    let proxy = create_test_proxy();

    let response = proxy
        .protocol_config(Some("final"), None)
        .await
        .expect("protocol_config call failed");

    let result = response.get("result");
    assert!(result.is_some(), "Should return result");

    // Check for runtime config
    let config = result.unwrap();
    assert!(config.get("runtime_config").is_some() || config.get("protocol_version").is_some(),
        "Should have protocol config info");

    if let Some(version) = config.get("protocol_version") {
        println!("Protocol version: {}", version);
    }
}
