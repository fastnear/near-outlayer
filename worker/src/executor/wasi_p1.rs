//! WASI Preview 1 executor
//!
//! Executes WASM modules compiled with wasm32-wasip1 or wasm32-wasi targets.
//!
//! ## Features
//! - Standard WASI functions (stdio, random, environment)
//! - Binary format with `main()` entry point
//! - Fuel metering for instruction counting
//!
//! ## Requirements
//! - wasmtime 28+ with WASI P1 compatibility layer
//! - Core WASM module (not component)
//! - `_start` export (created by Rust from `fn main()`)

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::OnceLock;
use tracing::debug;
use wasmtime::*;
use wasmtime_wasi::preview1::{self, WasiP1Ctx};
use wasmtime_wasi::WasiCtxBuilder;

use crate::api_client::ResourceLimits;

/// Global WASM engine for WASI P1 modules (core modules, NOT components)
///
/// IMPORTANT: This engine is ONLY for P1 modules. P2 components have their own engine.
///
/// Configuration:
/// - wasm_component_model = false (P1 uses core modules)
/// - async_support = true (for async execution)
/// - consume_fuel = true (instruction metering)
///
/// Creating Engine is expensive (~50-100ms). By reusing a single instance,
/// we avoid this overhead on every execution.
static WASM_ENGINE_P1: OnceLock<Engine> = OnceLock::new();

/// Get or initialize the global P1 engine
///
/// This engine has component_model=false and is NOT compatible with P2 components.
fn get_p1_engine() -> &'static Engine {
    WASM_ENGINE_P1.get_or_init(|| {
        let mut config = Config::new();
        // NO component_model - P1 uses core modules
        config.async_support(true);   // Async execution
        config.consume_fuel(true);    // Instruction metering
        tracing::info!("⚡ Initialized global WASM engine for P1 (core modules)");
        Engine::new(&config).expect("Failed to create P1 WASM engine")
    })
}

/// Execute WASI Preview 1 module
///
/// # Arguments
/// * `wasm_bytes` - WASM module binary
/// * `input_data` - JSON input via stdin
/// * `limits` - Resource limits (memory, instructions, time)
/// * `env_vars` - Environment variables (from encrypted secrets)
/// * `print_stderr` - Print WASM stderr to worker logs
///
/// # Returns
/// * `Ok((output, fuel_consumed, refund_usd))` - Execution succeeded
///   - `refund_usd` is always None for P1 (no payment host function support)
/// * `Err(_)` - Not a valid P1 module or execution failed
pub async fn execute(
    wasm_bytes: &[u8],
    input_data: &[u8],
    limits: &ResourceLimits,
    env_vars: Option<HashMap<String, String>>,
    print_stderr: bool,
) -> Result<(Vec<u8>, u64, Option<u64>)> {
    // Use global P1 engine (avoids ~50-100ms overhead per execution)
    let engine = get_p1_engine();

    // Try to load as module
    let module = wasmtime::Module::from_binary(&engine, wasm_bytes)
        .context("Not a valid WASI Preview 1 module")?;

    debug!("Loaded as WASI Preview 1 module (wasmtime)");

    // Create linker for WASI P1
    let mut linker = wasmtime::Linker::new(&engine);
    preview1::add_to_linker_async(&mut linker, |t: &mut WasiP1Ctx| t)?;

    // Prepare stdin/stdout pipes
    let stdin_pipe = wasmtime_wasi::pipe::MemoryInputPipe::new(input_data.to_vec());
    let stdout_pipe =
        wasmtime_wasi::pipe::MemoryOutputPipe::new((limits.max_memory_mb as usize) * 1024 * 1024);
    let stderr_pipe = wasmtime_wasi::pipe::MemoryOutputPipe::new(1024 * 1024);

    // Build WASI P1 context
    let mut wasi_builder = WasiCtxBuilder::new();
    wasi_builder.stdin(stdin_pipe);
    wasi_builder.stdout(stdout_pipe.clone());
    wasi_builder.stderr(stderr_pipe.clone());

    // Add environment variables (from encrypted secrets)
    if let Some(env_map) = env_vars {
        for (key, value) in env_map {
            wasi_builder.env(&key, &value);
            debug!("Added env var: {}", key);
        }
    }

    let wasi_p1_ctx = wasi_builder.build_p1();

    // Create store with fuel limit
    let mut store = Store::new(&engine, wasi_p1_ctx);
    store.set_fuel(limits.max_instructions)?;

    // Instantiate module
    debug!("Instantiating WASI P1 module");
    let instance = linker
        .instantiate_async(&mut store, &module)
        .await
        .context("Failed to instantiate WASI P1 module")?;

    // Get and call _start function (WASI entry point from main())
    debug!("Calling _start");
    let start = instance
        .get_typed_func::<(), ()>(&mut store, "_start")
        .context(
            "Failed to find _start function. \
             Make sure you're using [[bin]] format with fn main(), not [lib] with cdylib",
        )?;

    let call_result = start.call_async(&mut store, ()).await;

    if let Err(e) = call_result {
        let error_str = e.to_string();
        tracing::error!("❌ WASI P1 _start failed: {}", error_str);

        // Read stderr to get program's error message
        let stderr_contents = stderr_pipe.contents();
        let stderr_msg = if !stderr_contents.is_empty() {
            String::from_utf8_lossy(&stderr_contents).to_string()
        } else {
            String::new()
        };

        // If it's an exit code error, include stderr and input data in error message
        if error_str.contains("Exited with i32 exit status") {
            if !stderr_msg.is_empty() {
                // Program printed error to stderr
                return Err(anyhow::anyhow!("{}", stderr_msg));
            }

            let input_preview = String::from_utf8_lossy(input_data);
            let preview = if input_preview.len() > 200 {
                format!("{}...", &input_preview[..200])
            } else {
                input_preview.to_string()
            };

            return Err(anyhow::anyhow!(
                "WASM program exited with error status. This usually means invalid input_data or panic in code. Input received: {}. Original error: {}",
                preview,
                error_str
            ));
        }

        // Other execution errors
        return Err(anyhow::anyhow!("WASM execution failed: {}", error_str));
    }

    debug!("WASI P1 module execution completed");

    // Get results
    let fuel_consumed = limits.max_instructions - store.get_fuel().unwrap_or(0);
    debug!("WASM execution consumed {} instructions", fuel_consumed);

    // Print stderr if flag is enabled (even on success)
    let stderr_contents = stderr_pipe.contents();
    if print_stderr && !stderr_contents.is_empty() {
        let stderr_str = String::from_utf8_lossy(&stderr_contents);
        tracing::info!("📝 WASM stderr output:\n{}", stderr_str);
    }

    let output = stdout_pipe.contents().to_vec();

    // P1 does not support payment host functions, so refund_usd is always None
    Ok((output, fuel_consumed, None))
}
