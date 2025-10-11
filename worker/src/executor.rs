use anyhow::{Context, Result};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{debug, info};
use wasmi::*;

use crate::api_client::{ExecutionResult, ResourceLimits};

/// WASI environment state
struct WasiEnv {
    /// Environment variables as "KEY=VALUE\0" strings
    env_buffer: Vec<u8>,
    /// Offsets of each env var in the buffer
    env_offsets: Vec<usize>,
}

impl WasiEnv {
    fn new(env_vars: Option<HashMap<String, String>>) -> Self {
        let env_vars = env_vars.unwrap_or_default();
        let mut env_buffer = Vec::new();
        let mut env_offsets = Vec::new();

        for (key, value) in &env_vars {
            env_offsets.push(env_buffer.len());
            let env_string = format!("{}={}", key, value);
            env_buffer.extend_from_slice(env_string.as_bytes());
            env_buffer.push(0); // null terminator
        }

        debug!(
            "Prepared {} environment variables, total size {} bytes",
            env_vars.len(),
            env_buffer.len()
        );

        Self {
            env_buffer,
            env_offsets,
        }
    }
}

/// WASM executor with resource metering
pub struct Executor {
    /// Maximum instructions allowed per execution
    default_max_instructions: u64,
}

impl Executor {
    /// Create a new executor
    pub fn new(default_max_instructions: u64) -> Self {
        Self {
            default_max_instructions,
        }
    }

    /// Execute WASM binary with resource limits
    ///
    /// # Arguments
    /// * `wasm_bytes` - Compiled WASM binary
    /// * `input_data` - Input data to pass to the WASM module
    /// * `limits` - Resource limits for execution
    /// * `env_vars` - Environment variables to pass to WASI (from decrypted secrets)
    ///
    /// # Returns
    /// * `Ok(ExecutionResult)` - Execution completed (success or error)
    /// * `Err(_)` - Fatal execution error
    pub async fn execute(
        &self,
        wasm_bytes: &[u8],
        input_data: &[u8],
        limits: &ResourceLimits,
        env_vars: Option<HashMap<String, String>>,
    ) -> Result<ExecutionResult> {
        let start_time = Instant::now();

        info!(
            "Starting WASM execution with limits: {} instructions, {} MB memory, {} seconds",
            limits.max_instructions, limits.max_memory_mb, limits.max_execution_seconds
        );

        // Execute with timeout
        let timeout = Duration::from_secs(limits.max_execution_seconds);
        let result = tokio::time::timeout(
            timeout,
            tokio::task::spawn_blocking({
                let wasm_bytes = wasm_bytes.to_vec();
                let input_data = input_data.to_vec();
                let limits = limits.clone();
                let env_vars = env_vars.clone();
                move || Self::execute_sync(&wasm_bytes, &input_data, &limits, env_vars)
            }),
        )
        .await;

        let execution_time_ms = start_time.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(inner_result)) => {
                // Execution completed within timeout
                match inner_result {
                    Ok((output, instructions)) => {
                        info!("WASM execution succeeded in {} ms, consumed {} instructions", execution_time_ms, instructions);
                        Ok(ExecutionResult {
                            success: true,
                            output: Some(output),
                            error: None,
                            execution_time_ms,
                            instructions,
                        })
                    }
                    Err(e) => {
                        info!("WASM execution failed: {}", e);
                        Ok(ExecutionResult {
                            success: false,
                            output: None,
                            error: Some(e.to_string()),
                            execution_time_ms,
                            instructions: 0,
                        })
                    }
                }
            }
            Ok(Err(e)) => {
                // Task panicked
                Ok(ExecutionResult {
                    success: false,
                    output: None,
                    error: Some(format!("Execution panicked: {}", e)),
                    execution_time_ms,
                    instructions: 0,
                })
            }
            Err(_) => {
                // Timeout exceeded
                Ok(ExecutionResult {
                    success: false,
                    output: None,
                    error: Some(format!(
                        "Execution timeout exceeded ({} seconds)",
                        limits.max_execution_seconds
                    )),
                    execution_time_ms,
                    instructions: 0,
                })
            }
        }
    }

    /// Synchronous execution (runs in blocking thread)
    fn execute_sync(
        wasm_bytes: &[u8],
        input_data: &[u8],
        limits: &ResourceLimits,
        env_vars: Option<HashMap<String, String>>,
    ) -> Result<(Vec<u8>, u64)> {
        // Create WASM engine with fuel metering
        let mut config = Config::default();
        config.consume_fuel(true);

        let engine = Engine::new(&config);
        let module = Module::new(&engine, wasm_bytes).context("Failed to parse WASM module")?;

        // Create WASI environment with env vars
        let wasi_env = WasiEnv::new(env_vars);

        // Create store with fuel limit (instruction metering)
        let mut store = Store::new(&engine, wasi_env);
        store
            .set_fuel(limits.max_instructions)
            .map_err(|e| anyhow::anyhow!("Failed to set fuel limit: {:?}", e))?;

        // Define host functions (WASI-like minimal interface)
        let mut linker = Linker::new(&engine);

        // Add minimal WASI support with environment variables
        Self::add_wasi_functions(&mut linker)?;

        // Create instance
        let instance = linker
            .instantiate(&mut store, &module)
            .context("Failed to instantiate WASM module")?
            .start(&mut store)
            .context("Failed to start WASM module")?;

        // Get memory export
        let memory = instance
            .get_memory(&store, "memory")
            .context("WASM module must export 'memory'")?;

        // Write input data to memory at a known location (e.g., offset 0)
        let input_ptr = 0;
        memory
            .write(&mut store, input_ptr, input_data)
            .map_err(|e| anyhow::anyhow!("Failed to write input data to WASM memory: {:?}", e))?;

        // Get the main execution function
        let execute_func = instance
            .get_typed_func::<(i32, i32), i32>(&store, "execute")
            .context("WASM module must export 'execute(input_ptr: i32, input_len: i32) -> i32' function")?;

        // Call the execute function
        debug!("Calling WASM execute function");
        let result_ptr = execute_func
            .call(&mut store, (input_ptr as i32, input_data.len() as i32))
            .context("WASM execution failed")?;

        if result_ptr < 0 {
            anyhow::bail!("WASM execution returned error code: {}", result_ptr);
        }

        // Read output length (first 4 bytes at result_ptr)
        let mut len_bytes = [0u8; 4];
        memory
            .read(&store, result_ptr as usize, &mut len_bytes)
            .map_err(|e| anyhow::anyhow!("Failed to read output length: {:?}", e))?;
        let output_len = u32::from_le_bytes(len_bytes) as usize;

        // Validate output length
        if output_len > (limits.max_memory_mb as usize * 1024 * 1024) {
            anyhow::bail!("Output size exceeds memory limit");
        }

        // Read output data
        let mut output = vec![0u8; output_len];
        memory
            .read(&store, result_ptr as usize + 4, &mut output)
            .map_err(|e| anyhow::anyhow!("Failed to read output data: {:?}", e))?;

        // Check remaining fuel (for metrics)
        let fuel_consumed = limits.max_instructions - store.get_fuel().unwrap_or(0);
        debug!("WASM execution consumed {} instructions", fuel_consumed);

        Ok((output, fuel_consumed))
    }

    /// Add minimal WASI host functions
    fn add_wasi_functions(linker: &mut Linker<WasiEnv>) -> Result<()> {
        // wasi_snapshot_preview1::random_get
        linker
            .func_wrap(
                "wasi_snapshot_preview1",
                "random_get",
                |mut caller: Caller<'_, WasiEnv>, buf_ptr: i32, buf_len: i32| -> i32 {
                    use rand::RngCore;

                    // Get memory from caller
                    let memory = match caller.get_export("memory") {
                        Some(Extern::Memory(mem)) => mem,
                        _ => return 1, // Error: memory not found
                    };

                    // Generate random bytes
                    let mut rng = rand::thread_rng();
                    let mut random_bytes = vec![0u8; buf_len as usize];
                    rng.fill_bytes(&mut random_bytes);

                    // Write random bytes to WASM memory
                    if let Err(_) = memory.write(&mut caller, buf_ptr as usize, &random_bytes) {
                        return 1; // Error: failed to write
                    }

                    0 // Success
                },
            )
            .context("Failed to add random_get function")?;

        // wasi_snapshot_preview1::fd_write (for debugging)
        linker
            .func_wrap(
                "wasi_snapshot_preview1",
                "fd_write",
                |_caller: Caller<'_, WasiEnv>, _fd: i32, _iovs: i32, _iovs_len: i32, _nwritten: i32| -> i32 {
                    // Ignore writes (or log them in debug mode)
                    0
                },
            )
            .context("Failed to add fd_write function")?;

        // wasi_snapshot_preview1::proc_exit
        linker
            .func_wrap(
                "wasi_snapshot_preview1",
                "proc_exit",
                |_caller: Caller<'_, WasiEnv>, _code: i32| {
                    // Do nothing - we handle exit via return value
                },
            )
            .context("Failed to add proc_exit function")?;

        // wasi_snapshot_preview1::environ_sizes_get
        linker
            .func_wrap(
                "wasi_snapshot_preview1",
                "environ_sizes_get",
                |mut caller: Caller<'_, WasiEnv>, count_ptr: i32, buf_size_ptr: i32| -> i32 {
                    let env = caller.data();
                    let count = env.env_offsets.len() as u32;
                    let buf_size = env.env_buffer.len() as u32;

                    // Get memory
                    let memory = match caller.get_export("memory") {
                        Some(Extern::Memory(mem)) => mem,
                        _ => return 1, // Error
                    };

                    // Write count
                    if let Err(_) = memory.write(&mut caller, count_ptr as usize, &count.to_le_bytes()) {
                        return 1;
                    }

                    // Write buf_size
                    if let Err(_) = memory.write(&mut caller, buf_size_ptr as usize, &buf_size.to_le_bytes()) {
                        return 1;
                    }

                    0 // Success
                },
            )
            .context("Failed to add environ_sizes_get function")?;

        // wasi_snapshot_preview1::environ_get
        linker
            .func_wrap(
                "wasi_snapshot_preview1",
                "environ_get",
                |mut caller: Caller<'_, WasiEnv>, environ_ptr: i32, environ_buf_ptr: i32| -> i32 {
                    // Clone data to avoid borrow issues
                    let env_offsets = caller.data().env_offsets.clone();
                    let env_buffer = caller.data().env_buffer.clone();

                    // Get memory
                    let memory = match caller.get_export("memory") {
                        Some(Extern::Memory(mem)) => mem,
                        _ => return 1, // Error
                    };

                    // Write pointers array
                    let mut ptr_offset = environ_ptr as usize;
                    for offset in &env_offsets {
                        let str_ptr = (environ_buf_ptr as usize + offset) as u32;
                        if let Err(_) = memory.write(&mut caller, ptr_offset, &str_ptr.to_le_bytes()) {
                            return 1;
                        }
                        ptr_offset += 4;
                    }

                    // Write environment buffer
                    if let Err(_) = memory.write(&mut caller, environ_buf_ptr as usize, &env_buffer) {
                        return 1;
                    }

                    0 // Success
                },
            )
            .context("Failed to add environ_get function")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_executor_creation() {
        let executor = Executor::new(10_000_000_000);
        assert_eq!(executor.default_max_instructions, 10_000_000_000);
    }

    #[tokio::test]
    async fn test_execution_timeout() {
        let executor = Executor::new(10_000_000_000);

        // Create a simple WASM module that loops forever
        // (We can't test this without a real WASM binary)
        let limits = ResourceLimits {
            max_instructions: 1_000_000,
            max_memory_mb: 1,
            max_execution_seconds: 1,
        };

        // This would need a real infinite loop WASM for proper testing
        // For now, just verify the executor structure works
    }
}
