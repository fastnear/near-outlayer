//! WASM executor with support for multiple WASI versions
//!
//! This module provides execution for different WASM formats:
//! - WASI Preview 2 (P2): Modern component model with HTTP support
//! - WASI Preview 1 (P1): Standard WASI modules
//!
//! ## Adding New Build Targets
//!
//! To add support for a new build target (e.g., wasm32-unknown-unknown):
//!
//! 1. Create a new module file: `src/executor/wasi_unknown.rs`
//! 2. Implement the executor function with signature:
//!    ```rust,ignore
//!    pub async fn execute(
//!        wasm_bytes: &[u8],
//!        input_data: &[u8],
//!        limits: &ResourceLimits,
//!        env_vars: Option<HashMap<String, String>>,
//!    ) -> Result<(Vec<u8>, u64)>
//!    ```
//! 3. Add module declaration: `mod wasi_unknown;`
//! 4. Add detection logic in `execute_async()` to try loading the new format
//! 5. Add unit tests in `tests/` directory
//!
//! ## Architecture
//!
//! The executor tries formats in order of priority:
//! 1. WASI P2 component (most modern, has HTTP)
//! 2. WASI P1 module (standard, widely compatible)
//! 3. Return error if no format matches

use anyhow::Result;
use std::collections::HashMap;
use std::time::Instant;
use tracing::info;

use crate::api_client::{ExecutionOutput, ExecutionResult, ResourceLimits, ResponseFormat};

mod wasi_p1;
mod wasi_p2;

/// WASM executor supporting multiple WASI versions
pub struct Executor {
    /// Maximum instructions allowed per execution (default)
    _default_max_instructions: u64,
    /// Print WASM stderr to worker logs
    print_wasm_stderr: bool,
}

impl Executor {
    /// Create a new executor
    pub fn new(default_max_instructions: u64, print_wasm_stderr: bool) -> Self {
        Self {
            _default_max_instructions: default_max_instructions,
            print_wasm_stderr,
        }
    }

    /// Execute WASM with input data
    ///
    /// Returns ExecutionResult with success/failure and optional output
    pub async fn execute(
        &self,
        wasm_bytes: &[u8],
        input_data: &[u8],
        limits: &ResourceLimits,
        env_vars: Option<HashMap<String, String>>,
        build_target: Option<&str>,
        response_format: &ResponseFormat,
    ) -> Result<ExecutionResult> {
        info!(
            "Starting WASM execution: {} instructions, {} MB memory, {} seconds, target: {:?}, format: {:?}",
            limits.max_instructions, limits.max_memory_mb, limits.max_execution_seconds, build_target, response_format
        );

        let start = Instant::now();

        // Try to execute with different WASI versions
        let result = Self::execute_async(wasm_bytes, input_data, limits, env_vars, build_target, self.print_wasm_stderr).await;

        let execution_time_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok((output_bytes, instructions)) => {
                info!(
                    "WASM execution succeeded in {} ms, consumed {} instructions",
                    execution_time_ms, instructions
                );

                // Convert output based on requested format
                let output = match response_format {
                    ResponseFormat::Bytes => {
                        Some(ExecutionOutput::Bytes(output_bytes))
                    }
                    ResponseFormat::Text => {
                        let text = String::from_utf8(output_bytes)
                            .unwrap_or_else(|e| format!("Invalid UTF-8 output: {}", e));
                        Some(ExecutionOutput::Text(text))
                    }
                    ResponseFormat::Json => {
                        // Parse output as JSON
                        match serde_json::from_slice::<serde_json::Value>(&output_bytes) {
                            Ok(json_value) => {
                                Some(ExecutionOutput::Json(json_value))
                            }
                            Err(e) => {
                                // If JSON parsing fails, return error
                                return Ok(ExecutionResult {
                                    success: false,
                                    output: None,
                                    error: Some(format!(
                                        "Failed to parse output as JSON: {}. Output was: {}",
                                        e,
                                        String::from_utf8_lossy(&output_bytes)
                                    )),
                                    execution_time_ms,
                                    instructions,
                                    compile_time_ms: None, // Compilation not tracked in executor
                                    compilation_note: None,
                                });
                            }
                        }
                    }
                };

                Ok(ExecutionResult {
                    success: true,
                    output,
                    error: None,
                    execution_time_ms,
                    instructions,
                    compile_time_ms: None, // Compilation not tracked in executor
                    compilation_note: None,
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
                    compile_time_ms: None, // Compilation not tracked in executor
                    compilation_note: None,
                })
            }
        }
    }

    /// Try to execute WASM with different formats
    ///
    /// If build_target is known, try that format first for performance.
    /// Otherwise, try all formats in priority order.
    ///
    /// Priority order:
    /// 1. WASI Preview 2 component (HTTP, modern features)
    /// 2. WASI Preview 1 module (standard WASI)
    /// 3. Error if no format matches
    async fn execute_async(
        wasm_bytes: &[u8],
        input_data: &[u8],
        limits: &ResourceLimits,
        env_vars: Option<HashMap<String, String>>,
        build_target: Option<&str>,
        print_wasm_stderr: bool,
    ) -> Result<(Vec<u8>, u64)> {
        // Optimize: if we know build_target, try appropriate executor first
        if let Some(target) = build_target {
            tracing::debug!("🎯 Build target specified: {:?}", target);
            match target {
                "wasm32-wasip2" => {
                    tracing::debug!("🔹 Trying WASI P2 executor (target: wasm32-wasip2)");
                    // When target is known, return error directly (don't fallback to other formats)
                    return wasi_p2::execute(wasm_bytes, input_data, limits, env_vars, print_wasm_stderr).await;
                }
                "wasm32-wasip1" | "wasm32-wasi" => {
                    tracing::debug!("🔹 Trying WASI P1 executor (target: {})", target);
                    // When target is known, return error directly (don't fallback to other formats)
                    return wasi_p1::execute(wasm_bytes, input_data, limits, env_vars, print_wasm_stderr).await;
                }
                _ => {
                    tracing::debug!("⚠️ Unknown target '{}', fallback to auto-detection", target);
                    // Unknown target, fallback to auto-detection below
                }
            }
        } else {
            tracing::debug!("🔍 No build target specified, auto-detecting format");
        }

        // Fallback: auto-detect format (for unknown targets or if specific executor failed)
        // Try WASI P2 component first
        if let Ok(result) = wasi_p2::execute(wasm_bytes, input_data, limits, env_vars.clone(), print_wasm_stderr).await
        {
            return Ok(result);
        }

        // Try WASI P1 module
        if let Ok(result) = wasi_p1::execute(wasm_bytes, input_data, limits, env_vars.clone(), print_wasm_stderr).await
        {
            return Ok(result);
        }

        // If nothing worked, return error
        anyhow::bail!(
            "Failed to load WASM binary: not a valid WASI P2 component or WASI P1 module.\n\
             Build target: {:?}\n\
             Supported formats:\n\
             - WASI Preview 2 components (wasm32-wasip2)\n\
             - WASI Preview 1 modules (wasm32-wasip1, wasm32-wasi)\n\
             \n\
             If you need to add support for a new target, see module documentation.",
            build_target
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_executor_creation() {
        let executor = Executor::new(10_000_000_000, false);
        assert_eq!(executor._default_max_instructions, 10_000_000_000);
        assert_eq!(executor.print_wasm_stderr, false);
    }
}
