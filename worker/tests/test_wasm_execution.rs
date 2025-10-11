/// Test WASM execution separately from the main flow
use std::path::PathBuf;

#[tokio::test]
async fn test_wasm_execution() {
    // Initialize logging
    let _ = tracing_subscriber::fmt()
        .with_env_filter("offchainvm_worker=debug")
        .try_init();

    // Path to test WASM file
    let test_wasm_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("wasi-examples/get-random/target/wasm32-wasip1/release/get_random_example.wasm");

    println!("Looking for WASM at: {}", test_wasm_path.display());

    if !test_wasm_path.exists() {
        panic!(
            "Test WASM not found! Build it first:\n\
             cd ../wasi-examples/get-random && cargo build --release --target wasm32-wasip1"
        );
    }

    // Read WASM file
    let wasm_bytes = std::fs::read(&test_wasm_path)
        .expect("Failed to read test WASM");

    println!("✅ Loaded WASM: {} bytes", wasm_bytes.len());

    // Create executor
    use offchainvm_worker::executor::Executor;
    use offchainvm_worker::api_client::ResourceLimits;

    let executor = Executor::new(10_000_000_000); // 10B instructions

    let resource_limits = ResourceLimits {
        max_instructions: 10_000_000_000,
        max_memory_mb: 128,
        max_execution_seconds: 60,
    };

    // Create valid JSON input for get-random example
    let input_json = r#"{"min": 0, "max": 100}"#;
    let input_data = input_json.as_bytes().to_vec();

    println!("📝 Input JSON: {}", input_json);

    // Execute WASM
    println!("⚙️  Executing WASM...");
    match executor.execute(&wasm_bytes, &input_data, &resource_limits, None).await {
        Ok(result) => {
            println!("✅ Execution result:");
            println!("   Success: {}", result.success);
            println!("   Time: {}ms", result.execution_time_ms);
            println!("   Output: {:?}", result.output);
            println!("   Error: {:?}", result.error);

            if !result.success {
                panic!("WASM execution failed: {:?}", result.error);
            }

            if let Some(output) = result.output {
                println!("   Output as string: {}", String::from_utf8_lossy(&output));
            }
        }
        Err(e) => {
            panic!("❌ Executor error: {}", e);
        }
    }
}

#[tokio::test]
async fn test_minimal_wasm() {
    // Test with minimal valid WASM module
    let minimal_wasm = vec![
        0x00, 0x61, 0x73, 0x6d, // WASM magic number
        0x01, 0x00, 0x00, 0x00, // Version 1
    ];

    use offchainvm_worker::executor::Executor;
    use offchainvm_worker::api_client::ResourceLimits;

    let executor = Executor::new(1_000_000);

    let resource_limits = ResourceLimits {
        max_instructions: 1_000_000,
        max_memory_mb: 1,
        max_execution_seconds: 1,
    };

    println!("⚙️  Testing minimal WASM...");
    match executor.execute(&minimal_wasm, &[], &resource_limits, None).await {
        Ok(result) => {
            println!("Result: success={}, error={:?}", result.success, result.error);
        }
        Err(e) => {
            println!("Expected error for minimal WASM: {}", e);
        }
    }
}
