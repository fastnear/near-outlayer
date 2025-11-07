use outlayer_quickjs_executor::{Invocation, InvocationResult, QuickJsConfig, QuickJsExecutor};
use std::fs;

fn load_quickjs_wasm() -> Vec<u8> {
    // Keep this flexible: point an env var to your quickjs.wasm during CI/dev.
    let path = std::env::var("QJS_WASM")
        .expect("set QJS_WASM=/path/to/quickjs.wasm (e.g., second-state/quickjs-wasi build)");
    fs::read(path).expect("read quickjs.wasm")
}

fn load_contract_src() -> String {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("counter_contract.js");
    fs::read_to_string(p).expect("read counter_contract.js")
}

#[test]
fn counter_persists_state_across_invocations() {
    let wasm = load_quickjs_wasm();
    let cfg = QuickJsConfig {
        max_wall_time: std::time::Duration::from_millis(250),
        max_fuel: 10_000_000,
    };
    let exec = QuickJsExecutor::new(&wasm, cfg).expect("executor");

    let contract = load_contract_src();

    // 1st call
    let inv1 = Invocation {
        contract_source: &contract,
        function: "increment",
        args: serde_json::json!([]),
        prior_state_json: b"{}",
    };
    let out1: InvocationResult = exec.execute(&inv1).expect("execute 1");
    let s1: serde_json::Value = serde_json::from_slice(&out1.new_state_json).unwrap();
    assert_eq!(s1.get("count").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(out1.result.get("count").and_then(|v| v.as_i64()), Some(1));

    // 2nd call with prior state
    let inv2 = Invocation {
        contract_source: &contract,
        function: "increment",
        args: serde_json::json!([]),
        prior_state_json: &out1.new_state_json,
    };
    let out2 = exec.execute(&inv2).expect("execute 2");
    let s2: serde_json::Value = serde_json::from_slice(&out2.new_state_json).unwrap();
    assert_eq!(s2.get("count").and_then(|v| v.as_i64()), Some(2));
    assert_eq!(out2.result.get("count").and_then(|v| v.as_i64()), Some(2));
}

#[test]
fn add_is_pure_function_no_state_change() {
    let wasm = load_quickjs_wasm();
    let cfg = QuickJsConfig {
        max_wall_time: std::time::Duration::from_millis(250),
        max_fuel: 10_000_000,
    };
    let exec = QuickJsExecutor::new(&wasm, cfg).expect("executor");
    let contract = load_contract_src();

    let inv = Invocation {
        contract_source: &contract,
        function: "add",
        args: serde_json::json!([40, 2]),
        prior_state_json: b"{}",
    };
    let out = exec.execute(&inv).expect("execute");
    assert_eq!(out.result.get("sum").and_then(|v| v.as_i64()), Some(42));
    // state remains {}
    let s: serde_json::Value = serde_json::from_slice(&out.new_state_json).unwrap();
    assert_eq!(s, serde_json::json!({}));
}
