//! WASI Preview 2 (Component Model) executor
//!
//! Executes WASM components compiled with wasm32-wasip2 target.
//!
//! ## Features
//! - Component model with typed interfaces
//! - HTTP/HTTPS requests via wasi-http
//! - NEAR RPC proxy via host functions `near:rpc/api@0.1.0` (when ExecutionContext is provided)
//! - Advanced filesystem operations
//! - Async execution
//!
//! ## Requirements
//! - wasmtime 28+
//! - WASM component format (not core module)
//! - wasi:cli/run interface

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use tracing::debug;
use wasmtime::component::{Component, Linker};
use wasmtime::*;

/// Global WASM engine for WASI P2 components (component model)
///
/// IMPORTANT: This engine is ONLY for P2 components. P1 modules have their own engine.
///
/// Configuration:
/// - wasm_component_model = true (required for P2)
/// - async_support = true (required for wasi-http)
/// - consume_fuel = true (instruction metering)
///
/// Creating Engine is expensive (~50-100ms). By reusing a single instance,
/// we avoid this overhead on every execution.
///
/// Note: CompiledCache entries are tied to this Engine configuration.
/// If config changes, cached entries will fail to deserialize and be recompiled.
static WASM_ENGINE_P2: OnceLock<Engine> = OnceLock::new();

/// Get or initialize the global P2 engine
///
/// This engine has component_model=true and is NOT compatible with P1 modules.
fn get_p2_engine() -> &'static Engine {
    WASM_ENGINE_P2.get_or_init(|| {
        let mut config = Config::new();
        config.wasm_component_model(true); // P2 ONLY: component model
        config.async_support(true);        // Required for wasi-http
        config.consume_fuel(true);         // Instruction metering
        tracing::info!("⚡ Initialized global WASM engine for P2 (component model)");
        Engine::new(&config).expect("Failed to create P2 WASM engine")
    })
}
use wasmtime_wasi::{DirPerms, FilePerms, ResourceTable, WasiCtx, WasiCtxBuilder, WasiView};
use wasmtime_wasi::bindings::Command;
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpView};

use crate::api_client::ResourceLimits;
use crate::compiled_cache::CompiledCache;
use crate::outlayer_rpc::RpcHostState;
use crate::outlayer_storage::{StorageClient, StorageHostState, add_storage_to_linker};
use crate::outlayer_payment::{PaymentHostState, add_payment_to_linker};

use super::ExecutionContext;

/// Host state for WASI P2 execution
///
/// Contains WASI context, HTTP context, and optionally RPC proxy, storage and payment state.
struct HostState {
    wasi_ctx: WasiCtx,
    wasi_http_ctx: WasiHttpCtx,
    table: ResourceTable,
    /// RPC proxy state (only present if ExecutionContext has outlayer_rpc)
    rpc_state: Option<RpcHostState>,
    /// Storage state (only present if ExecutionContext has storage_config)
    storage_state: Option<StorageHostState>,
    /// Payment state (only present if attached_usd > 0)
    payment_state: Option<PaymentHostState>,
}

impl WasiView for HostState {
    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.wasi_ctx
    }

    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }
}

impl WasiHttpView for HostState {
    fn ctx(&mut self) -> &mut WasiHttpCtx {
        &mut self.wasi_http_ctx
    }

    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }

    /// Increase the max chunk size for outgoing HTTP request bodies
    /// Default is too small for large attachments (wasi-http-client sends entire body in one write)
    fn outgoing_body_buffer_chunks(&mut self) -> usize {
        tracing::info!("🔧 outgoing_body_buffer_chunks called, returning 16");
        16 // Allow more buffered chunks (default is 1)
    }

    fn outgoing_body_chunk_size(&mut self) -> usize {
        tracing::info!("🔧 outgoing_body_chunk_size called, returning 16MB");
        16 * 1024 * 1024 // 16MB max per write (default might be too small)
    }
}

impl HostState {
    /// Get RPC host state (for host function callbacks)
    fn rpc_state_mut(&mut self) -> &mut RpcHostState {
        self.rpc_state.as_mut().expect("RPC state not initialized")
    }

    /// Get storage host state (for host function callbacks)
    fn storage_state_mut(&mut self) -> &mut StorageHostState {
        self.storage_state.as_mut().expect("Storage state not initialized")
    }

    /// Get payment host state (for host function callbacks)
    fn payment_state_mut(&mut self) -> &mut PaymentHostState {
        self.payment_state.as_mut().expect("Payment state not initialized")
    }
}

/// Execute WASI Preview 2 component
///
/// # Arguments
/// * `wasm_bytes` - WASM component binary
/// * `wasm_checksum` - SHA256 checksum of WASM bytes (for compiled cache key)
/// * `compiled_cache` - Optional compiled component cache for ~10x speedup
/// * `input_data` - JSON input via stdin
/// * `limits` - Resource limits (memory, instructions, time)
/// * `env_vars` - Environment variables (from encrypted secrets, includes ATTACHED_USD)
/// * `print_stderr` - Print WASM stderr to worker logs
/// * `exec_ctx` - Execution context with optional RPC proxy
///
/// # Returns
/// * `Ok((output, fuel_consumed, refund_usd))` - Execution succeeded
///   - `refund_usd` is Some if WASM called refund_usd() host function
/// * `Err(_)` - Not a valid P2 component or execution failed
pub async fn execute(
    wasm_bytes: &[u8],
    wasm_checksum: Option<&str>,
    compiled_cache: Option<&Arc<Mutex<CompiledCache>>>,
    input_data: &[u8],
    limits: &ResourceLimits,
    env_vars: Option<HashMap<String, String>>,
    print_stderr: bool,
    exec_ctx: Option<&ExecutionContext>,
) -> Result<(Vec<u8>, u64, Option<u64>)> {
    // Use global P2 engine (avoids ~50-100ms overhead per execution)
    let engine = get_p2_engine();

    // Try to load from compiled cache first (if checksum provided)
    let component = if let (Some(checksum), Some(cache)) = (wasm_checksum, compiled_cache) {
        // Try cache hit
        let cached = cache.lock().ok().and_then(|mut c| c.get(checksum, &engine));

        if let Some(cached_component) = cached {
            debug!("⚡ Using compiled cache for {}", checksum);
            cached_component
        } else {
            // Cache miss - compile from bytes
            debug!("🔨 Compiling component (cache miss): {}", checksum);
            let component = Component::from_binary(&engine, wasm_bytes)
                .context("Not a valid WASI Preview 2 component")?;

            // Store in cache for next time
            if let Ok(mut c) = cache.lock() {
                if let Err(e) = c.put(checksum, &component) {
                    tracing::warn!("Failed to cache compiled component: {}", e);
                }
            }

            component
        }
    } else {
        // No cache available - compile directly
        Component::from_binary(&engine, wasm_bytes)
            .context("Not a valid WASI Preview 2 component")?
    };

    debug!("Loaded as WASI Preview 2 component");

    // Check for storage import if running as project
    // For P2 components, we check if they import `near:storage/api` interface
    // This indicates the WASM is built with OutLayer SDK and knows about project context
    // Note: The `__outlayer_get_metadata` export doesn't work in component model
    // because #[no_mangle] exports are not visible in component WIT
    let has_storage_import = component.component_type().imports(&engine)
        .any(|(name, _)| name.contains("near:storage/api"));

    let storage_config = exec_ctx.and_then(|ctx| ctx.storage_config.as_ref());

    // For P2 components: if running as project, WASM must import storage interface
    // This ensures the WASM was built with OutLayer SDK
    if storage_config.is_some() && !has_storage_import {
        anyhow::bail!(
            "WASM running in project context must import `near:storage/api` interface.\n\
            This import is provided by the `outlayer` crate.\n\
            Without this import, persistent storage is not available.\n\
            \n\
            To fix:\n\
            1. Add `outlayer` crate to your dependencies\n\
            2. Use storage functions from `outlayer::storage` module\n\
            \n\
            Or run this WASM as standalone (without project) if you don't need persistent storage."
        );
    }

    // If WASM imports storage but we don't have storage config, fail early with helpful message
    if has_storage_import && storage_config.is_none() {
        anyhow::bail!(
            "WASM imports `near:storage/api` but storage is not configured.\n\
            \n\
            This WASM was built with the `outlayer` crate and expects persistent storage.\n\
            \n\
            Possible causes:\n\
            1. WASM is running standalone (not as part of a project)\n\
            2. Keystore is not configured (KEYSTORE_BASE_URL/KEYSTORE_AUTH_TOKEN)\n\
            3. Project UUID is missing from execution request\n\
            \n\
            To fix:\n\
            1. Run this WASM through a project (request_execution_version with project_id)\n\
            2. Ensure keystore is properly configured in worker environment\n\
            3. Or rebuild WASM without `outlayer` crate if you don't need storage"
        );
    }

    if storage_config.is_some() && has_storage_import {
        tracing::debug!("✅ Project WASM imports near:storage/api interface");
    }

    // Create linker with WASI and HTTP support
    let mut linker: Linker<HostState> = Linker::new(&engine);
    wasmtime_wasi::add_to_linker_async(&mut linker)?;
    wasmtime_wasi_http::add_only_http_to_linker_async(&mut linker)?;

    // Add NEAR RPC host functions if context has RPC proxy
    let rpc_state = if let Some(ctx) = exec_ctx {
        if let Some(outlayer_rpc) = &ctx.outlayer_rpc {
            debug!("Adding NEAR RPC host functions to linker");

            // Create sync RPC proxy for host functions
            let rpc_url = outlayer_rpc.get_rpc_url();
            let sync_proxy = crate::outlayer_rpc::host_functions_sync::RpcProxy::new(
                rpc_url,
                100, // max_calls
                true, // allow_transactions
                None, // No default signer - WASM provides signing keys
            )?;

            // Add RPC host functions to linker
            crate::outlayer_rpc::add_rpc_to_linker(&mut linker, |state: &mut HostState| {
                state.rpc_state_mut()
            })?;

            Some(RpcHostState::new(sync_proxy))
        } else {
            debug!("No RPC proxy in execution context");
            None
        }
    } else {
        debug!("No execution context provided");
        None
    };

    // Add storage host functions if context has storage config
    let storage_state = if let Some(ctx) = exec_ctx {
        if let Some(storage_config) = &ctx.storage_config {
            debug!("Adding storage host functions to linker");

            // Create storage client
            let storage_client = StorageClient::new(storage_config.clone())
                .context("Failed to create storage client")?;

            // Add storage host functions to linker
            add_storage_to_linker(&mut linker, |state: &mut HostState| {
                state.storage_state_mut()
            })?;

            Some(StorageHostState::from_client(storage_client))
        } else {
            debug!("No storage config in execution context");
            None
        }
    } else {
        None
    };

    // Extract attached_usd from env_vars for payment state
    // Must be done before env_vars is consumed by wasi_builder
    let attached_usd: u64 = env_vars
        .as_ref()
        .and_then(|env| env.get("ATTACHED_USD"))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    // Check if component imports payment interface
    let has_payment_import = component.component_type().imports(&engine)
        .any(|(name, _)| name.contains("near:payment/api"));

    // Add payment host functions if WASM imports payment interface
    // Even if attached_usd=0, we add the linker so WASM doesn't crash when calling refund_usd
    // Instead, it will get an error message "Refund amount X exceeds attached USD 0"
    let payment_state = if has_payment_import {
        debug!("Adding payment host functions to linker, attached_usd={}", attached_usd);

        // Add payment host functions to linker
        add_payment_to_linker(&mut linker, |state: &mut HostState| {
            state.payment_state_mut()
        })?;

        Some(PaymentHostState::new(attached_usd))
    } else if attached_usd > 0 {
        // WASM has attached_usd but doesn't import payment interface - log warning
        debug!("WASM has attached_usd={} but doesn't import near:payment/api interface, refund not possible", attached_usd);
        None
    } else {
        None
    };

    // Prepare stdin/stdout/stderr pipes
    let stdin_pipe = wasmtime_wasi::pipe::MemoryInputPipe::new(input_data.to_vec());
    let stdout_pipe =
        wasmtime_wasi::pipe::MemoryOutputPipe::new((limits.max_memory_mb as usize) * 1024 * 1024);
    let stderr_pipe = wasmtime_wasi::pipe::MemoryOutputPipe::new(1024 * 1024);

    // Build WASI context
    let mut wasi_builder = WasiCtxBuilder::new();
    wasi_builder.stdin(stdin_pipe);
    wasi_builder.stdout(stdout_pipe.clone());
    wasi_builder.stderr(stderr_pipe.clone());

    // Add preopened directory (required for WASI P2 filesystem interface)
    wasi_builder.preopened_dir(
        "/tmp",      // host_path
        ".",         // guest_path
        DirPerms::all(),
        FilePerms::all(),
    )?;

    // Add environment variables (from encrypted secrets)
    if let Some(env_map) = env_vars {
        for (key, value) in env_map {
            wasi_builder.env(&key, &value);
            debug!("Added env var: {}", key);
        }
    }

    // Add indicator that RPC proxy is available
    if rpc_state.is_some() {
        wasi_builder.env("NEAR_RPC_PROXY_AVAILABLE", "1");
        debug!("Added env var: NEAR_RPC_PROXY_AVAILABLE=1");
    }

    let host_state = HostState {
        wasi_ctx: wasi_builder.build(),
        wasi_http_ctx: WasiHttpCtx::new(),
        table: ResourceTable::new(),
        rpc_state,
        storage_state,
        payment_state,
    };

    // Create store with fuel limit
    let mut store = Store::new(&engine, host_state);
    store.set_fuel(limits.max_instructions)?;

    // Instantiate and execute component
    debug!("Instantiating component");
    let command = Command::instantiate_async(&mut store, &component, &linker)
        .await
        .map_err(|e| {
            tracing::error!("Failed to instantiate component: {}", e);
            tracing::error!("Error details: {:?}", e);
            e
        })
        .context("Failed to instantiate component")?;

    debug!("Running wasi:cli/run");
    let execution_result = command
        .wasi_cli_run()
        .call_run(&mut store)
        .await;

    // Get fuel consumed before checking result
    let fuel_consumed = limits.max_instructions - store.get_fuel().unwrap_or(0);
    debug!("Component consumed {} instructions", fuel_consumed);

    // Log RPC call count if available
    if let Some(ref rpc_state) = store.data().rpc_state {
        let call_count = rpc_state.proxy.get_call_count();
        if call_count > 0 {
            debug!("Component made {} RPC calls", call_count);
        }
    }

    // Get refund_usd from payment state (if WASM called refund_usd())
    let refund_usd = store.data().payment_state.as_ref().map(|ps| {
        let refund = ps.get_refund_usd();
        if refund > 0 {
            debug!("Component requested refund of {} USD", refund);
        }
        refund
    }).filter(|&r| r > 0);

    // Check execution result
    // Read stderr for debugging (if flag is enabled)
    let stderr_contents = stderr_pipe.contents();
    if print_stderr && !stderr_contents.is_empty() {
        let stderr_str = String::from_utf8_lossy(&stderr_contents);
        tracing::info!("📝 WASM stderr output:\n{}", stderr_str);
    }

    match execution_result {
        Ok(Ok(())) => {
            debug!("Component execution completed successfully");
            let output = stdout_pipe.contents().to_vec();
            Ok((output, fuel_consumed, refund_usd))
        }
        Ok(Err(_)) | Err(_) => {
            // Component exited with error or trapped
            let error_msg = if !stderr_contents.is_empty() {
                String::from_utf8_lossy(&stderr_contents).to_string()
            } else {
                "Component execution failed (no error message in stderr)".to_string()
            };

            debug!("Component execution failed: {}", error_msg);
            Err(anyhow::anyhow!("{}", error_msg))
        }
    }
}
