//! WASI Preview 1 executor
//!
//! Executes WASM modules compiled with wasm32-wasip1 or wasm32-wasi targets.
//!
//! ## Features
//! - Standard WASI functions (stdio, random, environment)
//! - Binary format with `main()` entry point
//! - Fuel metering for instruction counting
//! - **Phase 1 Hardening**: Epoch deadline + deterministic WASI environment
//!
//! ## Requirements
//! - wasmtime 28+ with WASI P1 compatibility layer
//! - Core WASM module (not component)
//! - `_start` export (created by Rust from `fn main()`)
//!
//! ## Phase 1: Principal Engineer Hardening
//! - Epoch-based wall-clock deadline (cannot be bypassed by idle syscalls)
//! - Deterministic WASI environment (TZ=UTC, LANG=C, no ambient RNG/network)
//! - Fuel + epoch dual protection for resource limits

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::time::Duration;
use tracing::debug;
use wasmtime::*;
use wasmtime_wasi::preview1::{self, WasiP1Ctx};

use crate::api_client::ResourceLimits;
use super::wasmtime_cfg;
use super::wasi_env;

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
/// * `Ok((output, fuel_consumed))` - Execution succeeded
/// * `Err(_)` - Not a valid P1 module or execution failed
pub async fn execute(
    wasm_bytes: &[u8],
    input_data: &[u8],
    limits: &ResourceLimits,
    env_vars: Option<HashMap<String, String>>,
    print_stderr: bool,
) -> Result<(Vec<u8>, u64)> {
    // Phase 1 Hardening: Use deterministic engine configuration
    // - Fuel metering (per-instruction accounting)
    // - Epoch interruption (hard wall-clock deadline)
    // - Disabled non-deterministic features
    let engine = wasmtime_cfg::engine_with_limits()?;

    debug!("Phase 1: Engine configured with fuel + epoch deadline");

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

    // Phase 1 Hardening: Build deterministic WASI context
    // - Deterministic environment (TZ=UTC, LANG=C, no ambient RNG)
    // - Network-off by default (secure by default)
    // - Only explicitly provided env vars (from encrypted secrets)
    let mut wasi_ctx = if let Some(env_map) = env_vars {
        debug!("Phase 1: Using custom env vars (from secrets)");
        wasi_env::wasi_with_env(env_map)?
    } else {
        debug!("Phase 1: Using deterministic WASI environment");
        wasi_env::deterministic_wasi()?
    };

    // Override stdio (WASI builder doesn't let us set both env and stdio easily,
    // so we use the ctx directly with wasmtime-wasi's internal API)
    // For now, rebuild with stdio:
    let mut wasi_builder = wasmtime_wasi::WasiCtxBuilder::new();
    wasi_builder.stdin(stdin_pipe);
    wasi_builder.stdout(stdout_pipe.clone());
    wasi_builder.stderr(stderr_pipe.clone());

    // Add deterministic environment or custom env vars
    if let Some(env_map) = env_vars {
        for (key, value) in env_map {
            wasi_builder.env(&key, &value);
            debug!("Added custom env var: {}", key);
        }
    } else {
        // Add deterministic defaults
        for (key, value) in wasi_env::WasiEnvBuilder::default_env_vars() {
            wasi_builder.env(&key, &value);
        }
    }

    let wasi_p1_ctx = wasi_builder.build_p1();

    // Create store with fuel + epoch deadline
    let mut store = Store::new(&engine, wasi_p1_ctx);

    // Phase 1 Hardening: Configure store limits (fuel + epoch)
    let deadline_task = wasmtime_cfg::configure_store_limits(
        &engine,
        &mut store,
        limits.max_instructions,
        Duration::from_secs(limits.max_execution_seconds)
    )?;

    debug!("Phase 1: Store configured with {} fuel, {}s epoch deadline",
           limits.max_instructions, limits.max_execution_seconds);

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

    // Phase 1 Hardening: Clean up deadline task
    deadline_task.abort();
    debug!("Phase 1: Epoch deadline task aborted");

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

    Ok((output, fuel_consumed))
}
