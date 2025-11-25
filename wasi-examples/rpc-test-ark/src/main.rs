//! RPC Test Ark - Comprehensive test suite for OutLayer RPC host functions
//!
//! This WASM component tests the near:rpc/api@0.1.0 host functions provided by OutLayer worker.
//! It performs various RPC calls and outputs the results as JSON.
//!
//! For transaction tests (call/transfer): Requires env NEAR_SENDER_ID, NEAR_SENDER_PRIVATE_KEY
//! For view-only tests: No credentials required

use serde::Serialize;
use std::env;
use std::io::Write;

// Generate bindings for near:rpc/api@0.1.0 interface
wit_bindgen::generate!({
    world: "rpc-test",
    path: "wit",
});

#[derive(Debug, Serialize)]
struct Output {
    success: bool,
    total: usize,
    passed: usize,
    failed: usize,
    results: Vec<TestResult>,
}

#[derive(Debug, Serialize)]
struct TestResult {
    name: String,
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

fn main() {
    println!("=== NEAR RPC Test Suite (API 0.1.0) ===\n");

    let mut results = Vec::new();
    let mut passed = 0;
    let mut failed = 0;

    // Get signer from env (for transaction tests)
    let signer_id = env::var("NEAR_SENDER_ID").unwrap_or_else(|_| "test.testnet".to_string());
    let signer_key = env::var("NEAR_SENDER_PRIVATE_KEY").unwrap_or_default();
    let has_signer = !signer_key.is_empty();

    println!("Signer: {} (credentials: {})\n", signer_id, if has_signer { "yes" } else { "no" });

    // ==================== Query Methods Tests ====================
    println!("--- Query Methods ---");

    let query_tests: Vec<TestResult> = vec![
        test_view_account("outlayer.testnet", ""),  // default = final
        test_view_account_at_block("outlayer.testnet", "optimistic"),
        test_view("wrap.testnet", "ft_metadata", "{}", ""),
        test_view_access_key("outlayer.testnet", "ed25519:2nVT8TeatXPpcj6BuZCEJ8UmoEx7kKLJdC4fVnKc4MU9", ""),
        test_view_access_key_list("outlayer.testnet", ""),
        test_view_code("outlayer.testnet", ""),
        test_view_state("outlayer.testnet", "", ""),  // empty prefix = all keys
    ];

    for test in query_tests {
        print_test_result(&test);
        if test.success { passed += 1; } else { failed += 1; }
        results.push(test);
    }

    // ==================== Block Methods Tests ====================
    println!("\n--- Block Methods ---");

    let block_tests: Vec<TestResult> = vec![
        test_block("final"),
        test_block("optimistic"),
        test_gas_price(""),  // latest
        test_chunk_recent(),
        test_changes("final"),
    ];

    for test in block_tests {
        print_test_result(&test);
        if test.success { passed += 1; } else { failed += 1; }
        results.push(test);
    }

    // ==================== Network Methods Tests ====================
    println!("\n--- Network Methods ---");

    let network_tests: Vec<TestResult> = vec![
        test_status(),
        test_network_info(),
        test_validators(""),  // current epoch
    ];

    for test in network_tests {
        print_test_result(&test);
        if test.success { passed += 1; } else { failed += 1; }
        results.push(test);
    }

    // ==================== Transaction Methods Tests ====================
    println!("\n--- Transaction Methods ---");

    // Test tx_status and receipt with real blockchain data
    let tx_tests: Vec<TestResult> = vec![
        test_tx_status("2e2FsbSyiSNaHkTAAAT2KXuhEEx7sFqw3azSGMioxjg5", "wasmhub.testnet", ""),
        test_receipt("AsbeN6M4EmG54q3oMFLcqguKoHJ587SJSS38BE3yhFAb"),
        test_send_tx("EAAAAHphdm9kaWwyLnRlc3RuZXQA8qlWaBB8bvOfL6DPmtnMfTXB1HRMELWcq4GMYmUDDZbDh1vSyVUAAA8AAAB3YXNtaHViLnRlc3RuZXRvBUurS3y5EPorGiF9VdCuquYIcnRDJgMs/HLXVeaeBgEAAAACBAAAAHRlc3QNAAAAeyJmb28iOiJiYXIifQBAehDzWgAAAAAAAAAAAAAAAAAAAAAAAABlFsfE6oJWkhXnWAY6PJS3VAABDwo4SL3xLfdcI+ydRMyTjwwOyxwtkkdIEUtfkXwk+FXW2uDPgKIdNxdAuXUC", ""),
    ];

    for test in tx_tests {
        print_test_result(&test);
        if test.success { passed += 1; } else { failed += 1; }
        results.push(test);
    }

    // Test call and transfer only if signer credentials provided
    if has_signer {
        let signer_tests: Vec<TestResult> = vec![
            test_call(&signer_id, &signer_key, "aurora.pool.f863973.m0", "deposit_and_stake", "{}", "10000000000000000000", "50000000000000", ""),
            test_transfer(&signer_id, &signer_key, "outlayer.testnet", "1", ""),
        ];

        for test in signer_tests {
            print_test_result(&test);
            if test.success { passed += 1; } else { failed += 1; }
            results.push(test);
        }
    } else {
        println!("[SKIP] call() and transfer() tests (no signer credentials provided)");
        println!("       Set NEAR_SENDER_ID and NEAR_SENDER_PRIVATE_KEY to enable");
    }

    // ==================== Low-level API Tests ====================
    println!("\n--- Low-level API ---");

    let raw_tests: Vec<TestResult> = vec![
        test_raw("status", "[]"),
    ];

    for test in raw_tests {
        print_test_result(&test);
        if test.success { passed += 1; } else { failed += 1; }
        results.push(test);
    }

    // ==================== Summary ====================
    let total = passed + failed;
    println!("\n=== Results: {}/{} passed ({} failed) ===", passed, total, failed);

    let all_success = failed == 0;

    // Output JSON for machine parsing
    let output = Output {
        success: all_success,
        total,
        passed,
        failed,
        results,
    };

    println!("\nJSON output:");
    let output_json = serde_json::to_string(&output).expect("Failed to serialize output");
    print!("{}", output_json);
    std::io::stdout().flush().expect("Failed to flush stdout");
}

fn print_test_result(test: &TestResult) {
    if test.success {
        print!("[PASS] {}", test.name);
        if let Some(ref result) = test.result {
            // Show tx_hash for transactions
            if let Some(tx_hash) = result.get("tx_hash") {
                print!(" -> tx: {}", tx_hash.as_str().unwrap_or("?"));
            }
            // Show block height for blocks
            if let Some(height) = result.get("height") {
                print!(" -> height: {}", height);
            }
        }
        println!();
    } else {
        println!("[FAIL] {} - {}", test.name, test.error.as_deref().unwrap_or("unknown error"));
    }
}

// ==================== Query Methods ====================

fn test_view_account(account_id: &str, finality: &str) -> TestResult {
    eprintln!("Testing view_account: {} (finality: {})", account_id, if finality.is_empty() { "final" } else { finality });

    let (result, error) = near::rpc::api::view_account(account_id, finality);

    if !error.is_empty() {
        return TestResult {
            name: format!("view_account({}, {})", account_id, if finality.is_empty() { "final" } else { finality }),
            success: false,
            result: None,
            error: Some(error),
        };
    }

    match serde_json::from_str::<serde_json::Value>(&result) {
        Ok(json) => {
            let has_amount = json.get("result").and_then(|r| r.get("amount")).is_some();
            let has_block_height = json.get("result").and_then(|r| r.get("block_height")).is_some();

            if has_amount && has_block_height {
                TestResult {
                    name: format!("view_account({})", account_id),
                    success: true,
                    result: Some(serde_json::json!({
                        "amount": json.get("result").and_then(|r| r.get("amount")),
                        "block_height": json.get("result").and_then(|r| r.get("block_height"))
                    })),
                    error: None,
                }
            } else {
                TestResult {
                    name: format!("view_account({})", account_id),
                    success: false,
                    result: Some(json),
                    error: Some("Missing expected fields (amount, block_height)".to_string()),
                }
            }
        }
        Err(e) => TestResult {
            name: format!("view_account({})", account_id),
            success: false,
            result: None,
            error: Some(format!("Failed to parse result: {}", e)),
        },
    }
}

fn test_view_account_at_block(account_id: &str, finality: &str) -> TestResult {
    eprintln!("Testing view_account at block: {} (finality: {})", account_id, finality);

    let (result, error) = near::rpc::api::view_account(account_id, finality);

    if !error.is_empty() {
        return TestResult {
            name: format!("view_account_at_block({}, {})", account_id, finality),
            success: false,
            result: None,
            error: Some(error),
        };
    }

    match serde_json::from_str::<serde_json::Value>(&result) {
        Ok(json) => {
            TestResult {
                name: format!("view_account_at_block({})", finality),
                success: true,
                result: Some(serde_json::json!({
                    "finality": finality,
                    "block_height": json.get("result").and_then(|r| r.get("block_height"))
                })),
                error: None,
            }
        }
        Err(e) => TestResult {
            name: format!("view_account_at_block({})", finality),
            success: false,
            result: None,
            error: Some(format!("Failed to parse: {}", e)),
        },
    }
}

fn test_view(contract_id: &str, method_name: &str, args_json: &str, finality: &str) -> TestResult {
    eprintln!("Testing view: {}.{}({}) finality={}", contract_id, method_name, args_json, if finality.is_empty() { "final" } else { finality });

    let (result, error) = near::rpc::api::view(contract_id, method_name, args_json, finality);

    if !error.is_empty() {
        return TestResult {
            name: format!("view({}.{})", contract_id, method_name),
            success: false,
            result: None,
            error: Some(error),
        };
    }

    let parsed: serde_json::Value = serde_json::from_str(&result)
        .unwrap_or_else(|_| serde_json::json!(result));

    TestResult {
        name: format!("view({}.{})", contract_id, method_name),
        success: true,
        result: Some(parsed),
        error: None,
    }
}

fn test_view_access_key(account_id: &str, public_key: &str, finality: &str) -> TestResult {
    eprintln!("Testing view_access_key: {} key={}", account_id, public_key);

    let (result, error) = near::rpc::api::view_access_key(account_id, public_key, finality);

    if !error.is_empty() {
        return TestResult {
            name: format!("view_access_key({},{})", account_id, &public_key[0..20]),
            success: false,
            result: None,
            error: Some(error),
        };
    }

    match serde_json::from_str::<serde_json::Value>(&result) {
        Ok(json) => {
            let nonce = json.get("result").and_then(|r| r.get("nonce"));

            TestResult {
                name: format!("view_access_key({},{})", account_id, &public_key[0..20]),
                success: true,
                result: Some(serde_json::json!({
                    "nonce": nonce
                })),
                error: None,
            }
        }
        Err(e) => TestResult {
            name: format!("view_access_key({},{})", account_id, &public_key[0..20]),
            success: false,
            result: None,
            error: Some(format!("Failed to parse: {}", e)),
        },
    }
}

fn test_view_access_key_list(account_id: &str, finality: &str) -> TestResult {
    eprintln!("Testing view_access_key_list: {}", account_id);

    let (result, error) = near::rpc::api::view_access_key_list(account_id, finality);

    if !error.is_empty() {
        return TestResult {
            name: format!("view_access_key_list({})", account_id),
            success: false,
            result: None,
            error: Some(error),
        };
    }

    match serde_json::from_str::<serde_json::Value>(&result) {
        Ok(json) => {
            let keys = json.get("result").and_then(|r| r.get("keys"));
            let key_count = keys.and_then(|k| k.as_array()).map(|arr| arr.len()).unwrap_or(0);

            TestResult {
                name: format!("view_access_key_list({})", account_id),
                success: true,
                result: Some(serde_json::json!({
                    "key_count": key_count
                })),
                error: None,
            }
        }
        Err(e) => TestResult {
            name: format!("view_access_key_list({})", account_id),
            success: false,
            result: None,
            error: Some(format!("Failed to parse: {}", e)),
        },
    }
}

fn test_view_code(account_id: &str, finality: &str) -> TestResult {
    eprintln!("Testing view_code: {}", account_id);

    let (result, error) = near::rpc::api::view_code(account_id, finality);

    if !error.is_empty() {
        return TestResult {
            name: format!("view_code({})", account_id),
            success: false,
            result: None,
            error: Some(error),
        };
    }

    match serde_json::from_str::<serde_json::Value>(&result) {
        Ok(json) => {
            let code_base64 = json.get("result").and_then(|r| r.get("code_base64"));
            let hash = json.get("result").and_then(|r| r.get("hash"));

            TestResult {
                name: format!("view_code({})", account_id),
                success: true,
                result: Some(serde_json::json!({
                    "has_code": code_base64.is_some(),
                    "hash": hash
                })),
                error: None,
            }
        }
        Err(e) => TestResult {
            name: format!("view_code({})", account_id),
            success: false,
            result: None,
            error: Some(format!("Failed to parse: {}", e)),
        },
    }
}

fn test_view_state(account_id: &str, prefix: &str, finality: &str) -> TestResult {
    eprintln!("Testing view_state: {} (prefix: {})", account_id, if prefix.is_empty() { "all" } else { prefix });

    let (result, error) = near::rpc::api::view_state(account_id, prefix, finality);

    if !error.is_empty() {
        return TestResult {
            name: format!("view_state({})", account_id),
            success: false,
            result: None,
            error: Some(error),
        };
    }

    match serde_json::from_str::<serde_json::Value>(&result) {
        Ok(json) => {
            let values = json.get("result").and_then(|r| r.get("values"));
            let value_count = values.and_then(|v| v.as_array()).map(|arr| arr.len()).unwrap_or(0);

            TestResult {
                name: format!("view_state({})", account_id),
                success: true,
                result: Some(serde_json::json!({
                    "value_count": value_count
                })),
                error: None,
            }
        }
        Err(e) => TestResult {
            name: format!("view_state({})", account_id),
            success: false,
            result: None,
            error: Some(format!("Failed to parse: {}", e)),
        },
    }
}

// ==================== Block Methods ====================

fn test_block(finality: &str) -> TestResult {
    eprintln!("Testing block: {}", finality);

    let (result, error) = near::rpc::api::block(finality);

    if !error.is_empty() {
        return TestResult {
            name: format!("block({})", finality),
            success: false,
            result: None,
            error: Some(error),
        };
    }

    match serde_json::from_str::<serde_json::Value>(&result) {
        Ok(json) => {
            let has_header = json.get("result").and_then(|r| r.get("header")).is_some();
            let has_chunks = json.get("result").and_then(|r| r.get("chunks")).is_some();
            let height = json
                .get("result")
                .and_then(|r| r.get("header"))
                .and_then(|h| h.get("height"));

            if has_header && has_chunks {
                TestResult {
                    name: format!("block({})", finality),
                    success: true,
                    result: Some(serde_json::json!({
                        "height": height
                    })),
                    error: None,
                }
            } else {
                TestResult {
                    name: format!("block({})", finality),
                    success: false,
                    result: Some(json),
                    error: Some("Missing expected fields (header, chunks)".to_string()),
                }
            }
        }
        Err(e) => TestResult {
            name: format!("block({})", finality),
            success: false,
            result: None,
            error: Some(format!("Failed to parse result: {}", e)),
        },
    }
}

fn test_gas_price(block_id: &str) -> TestResult {
    eprintln!("Testing gas_price: {}", if block_id.is_empty() { "latest" } else { block_id });

    let (result, error) = near::rpc::api::gas_price(block_id);

    if !error.is_empty() {
        return TestResult {
            name: "gas_price".to_string(),
            success: false,
            result: None,
            error: Some(error),
        };
    }

    if result.is_empty() {
        return TestResult {
            name: "gas_price".to_string(),
            success: false,
            result: None,
            error: Some("Empty gas price result".to_string()),
        };
    }

    let gas_price: serde_json::Value = serde_json::from_str(&result)
        .unwrap_or_else(|_| serde_json::json!(result));

    TestResult {
        name: "gas_price".to_string(),
        success: true,
        result: Some(serde_json::json!({ "gas_price": gas_price })),
        error: None,
    }
}

fn test_chunk_recent() -> TestResult {
    eprintln!("Testing chunk (getting recent block first)");

    // First get recent block to get chunk info
    let (block_result, block_error) = near::rpc::api::block("final");

    if !block_error.is_empty() {
        return TestResult {
            name: "chunk".to_string(),
            success: false,
            result: None,
            error: Some(format!("Failed to get block: {}", block_error)),
        };
    }

    // Parse block to get chunk ID
    let block_json: serde_json::Value = serde_json::from_str(&block_result)
        .unwrap_or(serde_json::json!({}));

    let chunk_id = block_json
        .get("result")
        .and_then(|r| r.get("chunks"))
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|chunk| chunk.get("chunk_hash"))
        .and_then(|h| h.as_str())
        .unwrap_or("");

    if chunk_id.is_empty() {
        return TestResult {
            name: "chunk".to_string(),
            success: false,
            result: None,
            error: Some("Could not get chunk ID from block".to_string()),
        };
    }

    let (result, error) = near::rpc::api::chunk(chunk_id);

    if !error.is_empty() {
        return TestResult {
            name: "chunk".to_string(),
            success: false,
            result: None,
            error: Some(error),
        };
    }

    match serde_json::from_str::<serde_json::Value>(&result) {
        Ok(json) => {
            let has_header = json.get("result").and_then(|r| r.get("header")).is_some();

            TestResult {
                name: "chunk".to_string(),
                success: has_header,
                result: Some(serde_json::json!({ "has_header": has_header })),
                error: if !has_header { Some("Missing header".to_string()) } else { None },
            }
        }
        Err(e) => TestResult {
            name: "chunk".to_string(),
            success: false,
            result: None,
            error: Some(format!("Failed to parse: {}", e)),
        },
    }
}

fn test_changes(finality: &str) -> TestResult {
    eprintln!("Testing changes: {}", finality);

    let (result, error) = near::rpc::api::changes(finality);

    if !error.is_empty() {
        return TestResult {
            name: format!("changes({})", finality),
            success: false,
            result: None,
            error: Some(error),
        };
    }

    match serde_json::from_str::<serde_json::Value>(&result) {
        Ok(_json) => {
            TestResult {
                name: format!("changes({})", finality),
                success: true,
                result: Some(serde_json::json!({ "response": "ok" })),
                error: None,
            }
        }
        Err(e) => TestResult {
            name: format!("changes({})", finality),
            success: false,
            result: None,
            error: Some(format!("Failed to parse: {}", e)),
        },
    }
}

// ==================== Network Methods ====================

fn test_status() -> TestResult {
    eprintln!("Testing status");

    let (result, error) = near::rpc::api::status();

    if !error.is_empty() {
        return TestResult {
            name: "status".to_string(),
            success: false,
            result: None,
            error: Some(error),
        };
    }

    match serde_json::from_str::<serde_json::Value>(&result) {
        Ok(json) => {
            let chain_id = json.get("result").and_then(|r| r.get("chain_id"));
            let version = json.get("result").and_then(|r| r.get("version"));

            TestResult {
                name: "status".to_string(),
                success: true,
                result: Some(serde_json::json!({
                    "chain_id": chain_id,
                    "version": version
                })),
                error: None,
            }
        }
        Err(e) => TestResult {
            name: "status".to_string(),
            success: false,
            result: None,
            error: Some(format!("Failed to parse: {}", e)),
        },
    }
}

fn test_network_info() -> TestResult {
    eprintln!("Testing network_info");

    let (result, error) = near::rpc::api::network_info();

    if !error.is_empty() {
        return TestResult {
            name: "network_info".to_string(),
            success: false,
            result: None,
            error: Some(error),
        };
    }

    match serde_json::from_str::<serde_json::Value>(&result) {
        Ok(json) => {
            let active_peers = json.get("result").and_then(|r| r.get("active_peers"));
            let peer_count = active_peers.and_then(|p| p.as_array()).map(|arr| arr.len()).unwrap_or(0);

            TestResult {
                name: "network_info".to_string(),
                success: true,
                result: Some(serde_json::json!({
                    "peer_count": peer_count
                })),
                error: None,
            }
        }
        Err(e) => TestResult {
            name: "network_info".to_string(),
            success: false,
            result: None,
            error: Some(format!("Failed to parse: {}", e)),
        },
    }
}

fn test_tx_status(tx_hash: &str, sender_account_id: &str, wait_until: &str) -> TestResult {
    eprintln!("Testing tx_status: {} from {}", tx_hash, sender_account_id);

    let (result, error) = near::rpc::api::tx_status(tx_hash, sender_account_id, wait_until);

    if !error.is_empty() {
        return TestResult {
            name: format!("tx_status({})", &tx_hash[0..8]),
            success: false,
            result: None,
            error: Some(error),
        };
    }

    match serde_json::from_str::<serde_json::Value>(&result) {
        Ok(json) => {
            let status = json.get("result").and_then(|r| r.get("status"));
            let final_execution_status = json.get("result")
                .and_then(|r| r.get("final_execution_status"))
                .or(status);

            TestResult {
                name: format!("tx_status({})", &tx_hash[0..8]),
                success: true,
                result: Some(serde_json::json!({
                    "status": final_execution_status
                })),
                error: None,
            }
        }
        Err(e) => TestResult {
            name: format!("tx_status({})", &tx_hash[0..8]),
            success: false,
            result: None,
            error: Some(format!("Failed to parse: {}", e)),
        },
    }
}

fn test_receipt(receipt_id: &str) -> TestResult {
    eprintln!("Testing receipt: {}", receipt_id);

    let (result, error) = near::rpc::api::receipt(receipt_id);

    if !error.is_empty() {
        return TestResult {
            name: format!("receipt({})", &receipt_id[0..8]),
            success: false,
            result: None,
            error: Some(error),
        };
    }

    match serde_json::from_str::<serde_json::Value>(&result) {
        Ok(json) => {
            let has_result = json.get("result").is_some();

            TestResult {
                name: format!("receipt({})", &receipt_id[0..8]),
                success: true,
                result: Some(serde_json::json!({
                    "has_receipt": has_result
                })),
                error: None,
            }
        }
        Err(e) => TestResult {
            name: format!("receipt({})", &receipt_id[0..8]),
            success: false,
            result: None,
            error: Some(format!("Failed to parse: {}", e)),
        },
    }
}

fn test_send_tx(signed_tx_base64: &str, wait_until: &str) -> TestResult {
    eprintln!("Testing send_tx: {} bytes", signed_tx_base64.len());

    let (result, error) = near::rpc::api::send_tx(signed_tx_base64, wait_until);

    if !error.is_empty() {
        return TestResult {
            name: "send_tx".to_string(),
            success: false,
            result: None,
            error: Some(error),
        };
    }

    match serde_json::from_str::<serde_json::Value>(&result) {
        Ok(json) => {
            let tx_hash = json.get("result")
                .and_then(|r| r.get("transaction"))
                .and_then(|t| t.get("hash"))
                .and_then(|h| h.as_str())
                .unwrap_or("unknown");

            TestResult {
                name: "send_tx".to_string(),
                success: true,
                result: Some(serde_json::json!({
                    "tx_hash": tx_hash
                })),
                error: None,
            }
        }
        Err(e) => TestResult {
            name: "send_tx".to_string(),
            success: false,
            result: None,
            error: Some(format!("Failed to parse: {}", e)),
        },
    }
}

fn test_call(signer_id: &str, signer_key: &str, receiver_id: &str, method_name: &str, args_json: &str, deposit_yocto: &str, gas: &str, wait_until: &str) -> TestResult {
    eprintln!("Testing call: {}.{}() from {}", receiver_id, method_name, signer_id);

    let (tx_hash, error) = near::rpc::api::call(
        signer_id,
        signer_key,
        receiver_id,
        method_name,
        args_json,
        deposit_yocto,
        gas,
        wait_until,
    );

    if !error.is_empty() {
        return TestResult {
            name: format!("call({}.{})", receiver_id, method_name),
            success: false,
            result: None,
            error: Some(error),
        };
    }

    TestResult {
        name: format!("call({}.{})", receiver_id, method_name),
        success: true,
        result: Some(serde_json::json!({
            "tx_hash": tx_hash
        })),
        error: None,
    }
}

fn test_validators(epoch_id: &str) -> TestResult {
    eprintln!("Testing validators: {}", if epoch_id.is_empty() { "current" } else { epoch_id });

    let (result, error) = near::rpc::api::validators(epoch_id);

    if !error.is_empty() {
        return TestResult {
            name: "validators".to_string(),
            success: false,
            result: None,
            error: Some(error),
        };
    }

    match serde_json::from_str::<serde_json::Value>(&result) {
        Ok(json) => {
            let current_validators = json.get("result").and_then(|r| r.get("current_validators"));
            let validator_count = current_validators.and_then(|v| v.as_array()).map(|arr| arr.len()).unwrap_or(0);

            TestResult {
                name: "validators".to_string(),
                success: true,
                result: Some(serde_json::json!({
                    "validator_count": validator_count
                })),
                error: None,
            }
        }
        Err(e) => TestResult {
            name: "validators".to_string(),
            success: false,
            result: None,
            error: Some(format!("Failed to parse: {}", e)),
        },
    }
}

// ==================== Transaction Methods ====================

fn test_transfer(signer_id: &str, signer_key: &str, receiver_id: &str, amount_yocto: &str, wait_until: &str) -> TestResult {
    eprintln!("Testing transfer: {} yoctoNEAR from {} to {} (wait: {})",
        amount_yocto, signer_id, receiver_id, if wait_until.is_empty() { "FINAL" } else { wait_until });

    let (tx_hash, error) = near::rpc::api::transfer(signer_id, signer_key, receiver_id, amount_yocto, wait_until);

    if !error.is_empty() {
        return TestResult {
            name: format!("transfer({})", receiver_id),
            success: false,
            result: None,
            error: Some(error),
        };
    }

    TestResult {
        name: format!("transfer({})", receiver_id),
        success: true,
        result: Some(serde_json::json!({ "tx_hash": tx_hash })),
        error: None,
    }
}

// ==================== Low-level API ====================

fn test_raw(method: &str, params_json: &str) -> TestResult {
    eprintln!("Testing raw RPC: {}", method);

    let (result, error) = near::rpc::api::raw(method, params_json);

    if !error.is_empty() {
        return TestResult {
            name: format!("raw({})", method),
            success: false,
            result: None,
            error: Some(error),
        };
    }

    match serde_json::from_str::<serde_json::Value>(&result) {
        Ok(json) => {
            let chain_id = json.get("result").and_then(|r| r.get("chain_id"));
            let version = json.get("result").and_then(|r| r.get("version"));

            TestResult {
                name: format!("raw({})", method),
                success: true,
                result: Some(serde_json::json!({
                    "chain_id": chain_id,
                    "version": version
                })),
                error: None,
            }
        }
        Err(e) => TestResult {
            name: format!("raw({})", method),
            success: false,
            result: None,
            error: Some(format!("Failed to parse result: {}", e)),
        },
    }
}
