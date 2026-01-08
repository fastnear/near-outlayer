//! Storage Test Ark - Test suite for OutLayer persistent storage
//!
//! Uses the `outlayer` SDK for storage operations.

use outlayer::{metadata, storage, env};
use serde::{Deserialize, Serialize};

// Required for project-based execution
// IMPORTANT: project must match your project_id on the OutLayer contract
metadata! {
    project: "zavodil2.testnet/test-storage",
    version: "1.0.0",
}

#[derive(Debug, Deserialize)]
struct Input {
    command: String,
    #[serde(default)]
    key: String,
    #[serde(default)]
    value: String,
    #[serde(default)]
    prefix: String,
}

#[derive(Debug, Serialize)]
struct Output {
    success: bool,
    command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exists: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    deleted: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    keys: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tests: Option<TestResults>,
}

#[derive(Debug, Serialize)]
struct TestResults {
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
    error: Option<String>,
}

fn main() {
    let output = match env::input_json::<Input>() {
        Ok(Some(input)) => process_command(&input),
        Ok(None) => Output {
            success: false,
            command: "parse".to_string(),
            value: None,
            exists: None,
            deleted: None,
            keys: None,
            error: Some("No input provided".to_string()),
            tests: None,
        },
        Err(e) => Output {
            success: false,
            command: "parse".to_string(),
            value: None,
            exists: None,
            deleted: None,
            keys: None,
            error: Some(format!("Failed to parse input: {}", e)),
            tests: None,
        },
    };

    let _ = env::output_json(&output);
}

fn process_command(input: &Input) -> Output {
    match input.command.as_str() {
        "set" => cmd_set(&input.key, &input.value),
        "get" => cmd_get(&input.key),
        "delete" => cmd_delete(&input.key),
        "has" => cmd_has(&input.key),
        "list" => cmd_list(&input.prefix),
        "set_worker" => cmd_set_worker(&input.key, &input.value),
        "get_worker" => cmd_get_worker(&input.key),
        "clear_all" => cmd_clear_all(),
        "test_all" => cmd_test_all(),
        _ => Output {
            success: false,
            command: input.command.clone(),
            value: None,
            exists: None,
            deleted: None,
            keys: None,
            error: Some(format!("Unknown command: {}", input.command)),
            tests: None,
        },
    }
}

fn cmd_set(key: &str, value: &str) -> Output {
    match storage::set(key, value.as_bytes()) {
        Ok(()) => Output {
            success: true,
            command: "set".to_string(),
            value: Some(format!("Stored {} bytes at key '{}'", value.len(), key)),
            exists: None,
            deleted: None,
            keys: None,
            error: None,
            tests: None,
        },
        Err(e) => Output {
            success: false,
            command: "set".to_string(),
            value: None,
            exists: None,
            deleted: None,
            keys: None,
            error: Some(e.to_string()),
            tests: None,
        },
    }
}

fn cmd_get(key: &str) -> Output {
    match storage::get(key) {
        Ok(Some(data)) => {
            let value = String::from_utf8(data)
                .unwrap_or_else(|e| format!("<binary data, {} bytes>", e.as_bytes().len()));
            Output {
                success: true,
                command: "get".to_string(),
                value: Some(value),
                exists: Some(true),
                deleted: None,
                keys: None,
                error: None,
                tests: None,
            }
        }
        Ok(None) => Output {
            success: true,
            command: "get".to_string(),
            value: None,
            exists: Some(false),
            deleted: None,
            keys: None,
            error: None,
            tests: None,
        },
        Err(e) => Output {
            success: false,
            command: "get".to_string(),
            value: None,
            exists: None,
            deleted: None,
            keys: None,
            error: Some(e.to_string()),
            tests: None,
        },
    }
}

fn cmd_delete(key: &str) -> Output {
    let deleted = storage::delete(key);
    Output {
        success: true,
        command: "delete".to_string(),
        value: None,
        exists: None,
        deleted: Some(deleted),
        keys: None,
        error: None,
        tests: None,
    }
}

fn cmd_has(key: &str) -> Output {
    let exists = storage::has(key);
    Output {
        success: true,
        command: "has".to_string(),
        value: None,
        exists: Some(exists),
        deleted: None,
        keys: None,
        error: None,
        tests: None,
    }
}

fn cmd_list(prefix: &str) -> Output {
    match storage::list_keys(prefix) {
        Ok(keys) => Output {
            success: true,
            command: "list".to_string(),
            value: None,
            exists: None,
            deleted: None,
            keys: Some(keys),
            error: None,
            tests: None,
        },
        Err(e) => Output {
            success: false,
            command: "list".to_string(),
            value: None,
            exists: None,
            deleted: None,
            keys: None,
            error: Some(e.to_string()),
            tests: None,
        },
    }
}

fn cmd_set_worker(key: &str, value: &str) -> Output {
    match storage::set_worker(key, value.as_bytes()) {
        Ok(()) => Output {
            success: true,
            command: "set_worker".to_string(),
            value: Some(format!("Stored {} bytes at worker key '{}'", value.len(), key)),
            exists: None,
            deleted: None,
            keys: None,
            error: None,
            tests: None,
        },
        Err(e) => Output {
            success: false,
            command: "set_worker".to_string(),
            value: None,
            exists: None,
            deleted: None,
            keys: None,
            error: Some(e.to_string()),
            tests: None,
        },
    }
}

fn cmd_get_worker(key: &str) -> Output {
    match storage::get_worker(key) {
        Ok(Some(data)) => {
            let value = String::from_utf8(data)
                .unwrap_or_else(|e| format!("<binary data, {} bytes>", e.as_bytes().len()));
            Output {
                success: true,
                command: "get_worker".to_string(),
                value: Some(value),
                exists: Some(true),
                deleted: None,
                keys: None,
                error: None,
                tests: None,
            }
        }
        Ok(None) => Output {
            success: true,
            command: "get_worker".to_string(),
            value: None,
            exists: Some(false),
            deleted: None,
            keys: None,
            error: None,
            tests: None,
        },
        Err(e) => Output {
            success: false,
            command: "get_worker".to_string(),
            value: None,
            exists: None,
            deleted: None,
            keys: None,
            error: Some(e.to_string()),
            tests: None,
        },
    }
}

fn cmd_clear_all() -> Output {
    match storage::clear_all() {
        Ok(()) => Output {
            success: true,
            command: "clear_all".to_string(),
            value: Some("All storage cleared".to_string()),
            exists: None,
            deleted: None,
            keys: None,
            error: None,
            tests: None,
        },
        Err(e) => Output {
            success: false,
            command: "clear_all".to_string(),
            value: None,
            exists: None,
            deleted: None,
            keys: None,
            error: Some(e.to_string()),
            tests: None,
        },
    }
}

fn cmd_test_all() -> Output {
    let mut results = Vec::new();
    let mut passed = 0;
    let mut failed = 0;

    // Clear all storage first to ensure clean state
    let _ = storage::clear_all();

    // Test 1: Set a value
    let test = test_set("test-key-1", "Hello, Storage!");
    if test.success { passed += 1; } else { failed += 1; }
    results.push(test);

    // Test 2: Get the value back
    let test = test_get("test-key-1", Some("Hello, Storage!"));
    if test.success { passed += 1; } else { failed += 1; }
    results.push(test);

    // Test 3: Check key exists
    let test = test_has("test-key-1", true);
    if test.success { passed += 1; } else { failed += 1; }
    results.push(test);

    // Test 4: Check non-existent key
    let test = test_has("non-existent-key", false);
    if test.success { passed += 1; } else { failed += 1; }
    results.push(test);

    // Test 5: Get non-existent key
    let test = test_get("non-existent-key", None);
    if test.success { passed += 1; } else { failed += 1; }
    results.push(test);

    // Test 6: Set multiple keys
    let _ = storage::set("prefix:key1", b"value1");
    let _ = storage::set("prefix:key2", b"value2");
    let _ = storage::set("other:key3", b"value3");
    let test = test_list_with_prefix("prefix:", 2);
    if test.success { passed += 1; } else { failed += 1; }
    results.push(test);

    // Test 7: List all keys
    let test = test_list_all_exists();
    if test.success { passed += 1; } else { failed += 1; }
    results.push(test);

    // Test 8: Delete a key
    let test = test_delete("test-key-1", true);
    if test.success { passed += 1; } else { failed += 1; }
    results.push(test);

    // Test 9: Verify key is deleted
    let test = test_has("test-key-1", false);
    if test.success { passed += 1; } else { failed += 1; }
    results.push(test);

    // Test 10: Delete non-existent key
    let test = test_delete("non-existent-key", false);
    if test.success { passed += 1; } else { failed += 1; }
    results.push(test);

    // Test 11: Worker-private storage set
    let test = test_set_worker("worker-secret", "secret-data");
    if test.success { passed += 1; } else { failed += 1; }
    results.push(test);

    // Test 12: Worker-private storage get
    let test = test_get_worker("worker-secret", Some("secret-data"));
    if test.success { passed += 1; } else { failed += 1; }
    results.push(test);

    // Test 13: Clear all and verify
    let test = test_clear_all_and_verify();
    if test.success { passed += 1; } else { failed += 1; }
    results.push(test);

    Output {
        success: failed == 0,
        command: "test_all".to_string(),
        value: Some(format!("{}/{} tests passed", passed, passed + failed)),
        exists: None,
        deleted: None,
        keys: None,
        error: if failed > 0 { Some(format!("{} tests failed", failed)) } else { None },
        tests: Some(TestResults {
            total: passed + failed,
            passed,
            failed,
            results,
        }),
    }
}

// ==================== Test Functions ====================

fn test_set(key: &str, value: &str) -> TestResult {
    match storage::set(key, value.as_bytes()) {
        Ok(()) => TestResult {
            name: format!("set({})", key),
            success: true,
            error: None,
        },
        Err(e) => TestResult {
            name: format!("set({})", key),
            success: false,
            error: Some(e.to_string()),
        },
    }
}

fn test_get(key: &str, expected: Option<&str>) -> TestResult {
    match storage::get(key) {
        Ok(data) => {
            match (data, expected) {
                (Some(bytes), Some(exp)) => {
                    let actual = String::from_utf8(bytes).unwrap_or_default();
                    if actual == exp {
                        TestResult { name: format!("get({})", key), success: true, error: None }
                    } else {
                        TestResult { name: format!("get({})", key), success: false, error: Some(format!("Expected '{}' but got '{}'", exp, actual)) }
                    }
                }
                (None, None) => TestResult { name: format!("get({}) -> None", key), success: true, error: None },
                (Some(_), None) => TestResult { name: format!("get({}) -> None", key), success: false, error: Some("Expected None but got data".to_string()) },
                (None, Some(exp)) => TestResult { name: format!("get({})", key), success: false, error: Some(format!("Expected '{}' but got None", exp)) },
            }
        }
        Err(e) => TestResult {
            name: format!("get({})", key),
            success: false,
            error: Some(e.to_string()),
        },
    }
}

fn test_has(key: &str, expected: bool) -> TestResult {
    let exists = storage::has(key);
    if exists == expected {
        TestResult { name: format!("has({}) == {}", key, expected), success: true, error: None }
    } else {
        TestResult { name: format!("has({}) == {}", key, expected), success: false, error: Some(format!("Expected {} but got {}", expected, exists)) }
    }
}

fn test_delete(key: &str, expected_existed: bool) -> TestResult {
    let deleted = storage::delete(key);
    if deleted == expected_existed {
        TestResult { name: format!("delete({}) == {}", key, expected_existed), success: true, error: None }
    } else {
        TestResult { name: format!("delete({}) == {}", key, expected_existed), success: false, error: Some(format!("Expected {} but got {}", expected_existed, deleted)) }
    }
}

fn test_list_with_prefix(prefix: &str, expected_count: usize) -> TestResult {
    match storage::list_keys(prefix) {
        Ok(keys) => {
            if keys.len() == expected_count {
                TestResult { name: format!("list_keys({}) -> {} keys", prefix, expected_count), success: true, error: None }
            } else {
                // Include actual keys in error for debugging
                TestResult {
                    name: format!("list_keys({}) -> {} keys", prefix, expected_count),
                    success: false,
                    error: Some(format!("Expected {} keys but got {}: {:?}", expected_count, keys.len(), keys))
                }
            }
        }
        Err(e) => TestResult {
            name: format!("list_keys({})", prefix),
            success: false,
            error: Some(e.to_string()),
        },
    }
}

fn test_list_all_exists() -> TestResult {
    match storage::list_keys("") {
        Ok(keys) => {
            if !keys.is_empty() {
                TestResult { name: format!("list_keys() -> {} keys", keys.len()), success: true, error: None }
            } else {
                TestResult { name: "list_keys() not empty".to_string(), success: false, error: Some("Expected keys but got empty list".to_string()) }
            }
        }
        Err(e) => TestResult {
            name: "list_keys() not empty".to_string(),
            success: false,
            error: Some(e.to_string()),
        },
    }
}

fn test_set_worker(key: &str, value: &str) -> TestResult {
    match storage::set_worker(key, value.as_bytes()) {
        Ok(()) => TestResult { name: format!("set_worker({})", key), success: true, error: None },
        Err(e) => TestResult { name: format!("set_worker({})", key), success: false, error: Some(e.to_string()) },
    }
}

fn test_get_worker(key: &str, expected: Option<&str>) -> TestResult {
    match storage::get_worker(key) {
        Ok(data) => {
            match (data, expected) {
                (Some(bytes), Some(exp)) => {
                    let actual = String::from_utf8(bytes).unwrap_or_default();
                    if actual == exp {
                        TestResult { name: format!("get_worker({})", key), success: true, error: None }
                    } else {
                        TestResult { name: format!("get_worker({})", key), success: false, error: Some(format!("Expected '{}' but got '{}'", exp, actual)) }
                    }
                }
                (None, None) => TestResult { name: format!("get_worker({}) -> None", key), success: true, error: None },
                (Some(_), None) => TestResult { name: format!("get_worker({}) -> None", key), success: false, error: Some("Expected None but got data".to_string()) },
                (None, Some(exp)) => TestResult { name: format!("get_worker({})", key), success: false, error: Some(format!("Expected '{}' but got None", exp)) },
            }
        }
        Err(e) => TestResult {
            name: format!("get_worker({})", key),
            success: false,
            error: Some(e.to_string()),
        },
    }
}

fn test_clear_all_and_verify() -> TestResult {
    if let Err(e) = storage::clear_all() {
        return TestResult { name: "clear_all()".to_string(), success: false, error: Some(e.to_string()) };
    }

    // Verify keys are cleared
    match storage::list_keys("") {
        Ok(keys) => {
            if keys.is_empty() {
                TestResult { name: "clear_all() + verify empty".to_string(), success: true, error: None }
            } else {
                TestResult { name: "clear_all() + verify empty".to_string(), success: false, error: Some(format!("Expected empty storage but found {} keys", keys.len())) }
            }
        }
        Err(e) => TestResult { name: "clear_all() + verify empty".to_string(), success: false, error: Some(e.to_string()) },
    }
}
