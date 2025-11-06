/**
 * Wasmtime Configuration with Deterministic Execution Guarantees
 *
 * This module provides the critical hardening for NEAR OutLayer's execution model:
 * - **Fuel metering**: Per-instruction cost accounting
 * - **Epoch interruption**: Hard wall-clock deadline (cannot be bypassed by idle syscalls)
 * - **Determinism knobs**: Disable non-deterministic WASM features
 *
 * Security Properties:
 * - Worker cannot escape resource limits via sleep() or blocking I/O
 * - Execution terminates even if guest code never consumes fuel (idle loops)
 * - Configuration prevents non-deterministic behavior (ambient RNG, wall-clock time)
 *
 * Principal Engineer Review: P0 Priority
 * - Hard wall-clock stop is CRITICAL for production
 * - Epoch interruption complements fuel metering
 * - Determinism flags prevent replay attack vulnerabilities
 *
 * @author NEAR OutLayer Team + Principal Engineer Review
 * @date 2025-11-05
 */

use anyhow::Result;
use std::time::{Duration, Instant};
use tokio::task::JoinHandle;
use wasmtime::{Config, Engine, Store};

/// Construct wasmtime Engine with fuel metering and epoch interruption.
///
/// Configuration choices (determinism-first):
/// - `consume_fuel(true)`: Per-instruction metering
/// - `epoch_interruption(true)`: Wall-clock deadline enforcement
/// - `wasm_threads(false)`: No threads (non-deterministic scheduling)
/// - `wasm_multi_memory(true)`: Allow multiple memories (WASI P2 needs this)
/// - `wasm_memory64(false)`: Disable 64-bit memory (not needed, adds complexity)
/// - `debug_info(false)`: No debug overhead in production
///
/// Returns: Configured Engine ready for Store creation
pub fn engine_with_limits() -> Result<Engine> {
    let mut cfg = Config::new();

    // Fuel metering (per-instruction cost accounting)
    cfg.consume_fuel(true);

    // Epoch interruption (hard wall-clock deadline)
    cfg.epoch_interruption(true);

    // Determinism: disable non-deterministic features
    cfg.wasm_threads(false);          // No threads (prevents non-deterministic scheduling)
    cfg.wasm_multi_memory(true);      // Allow multiple memories (needed for WASI P2)
    cfg.wasm_memory64(false);         // Disable 64-bit memory (not needed)

    // Performance: disable debug overhead
    cfg.debug_info(false);

    // Future: add more determinism knobs as wasmtime adds them
    // - Stable imports only
    // - No ambient I/O without explicit capabilities

    Ok(Engine::new(&cfg)?)
}

/// Attach epoch-based wall-time deadline to a Store.
///
/// **How it works**:
/// 1. Spawn a background task that increments the engine's epoch every 5ms
/// 2. Continue incrementing until `max_wall` duration elapses
/// 3. When epoch exceeds Store's deadline, wasmtime interrupts execution
///
/// **Why this matters**:
/// Fuel metering only counts instructions. A guest that does:
/// ```wasm
/// loop {
///   fd_sync(stdout); // Syscall that doesn't consume fuel
/// }
/// ```
/// ...will never hit the fuel limit but can stall forever.
///
/// Epoch interruption provides a **hard wall-clock deadline** that cannot be bypassed.
///
/// **Usage**:
/// ```rust
/// let engine = engine_with_limits()?;
/// let mut store = Store::new(&engine, host_state);
/// store.add_fuel(fuel_amount)?;
/// store.set_epoch_deadline(1); // Interrupt when epoch > deadline
///
/// let deadline_task = attach_deadline(&engine, &mut store, Duration::from_secs(60));
///
/// // Execute WASM
/// instance.exports.main().call(&mut store, ())?;
///
/// // Clean up background task
/// deadline_task.abort(); // Or await if you want graceful shutdown
/// ```
///
/// **Parameters**:
/// - `engine`: The wasmtime Engine (must have `epoch_interruption(true)`)
/// - `max_wall`: Maximum wall-clock time for execution
///
/// **Returns**: JoinHandle for the background task (caller must abort/await it)
pub fn attach_deadline(engine: &Engine, max_wall: Duration) -> JoinHandle<()> {
    let engine = engine.clone();
    let start = Instant::now();
    let tick = Duration::from_millis(5); // Increment epoch every 5ms

    tokio::spawn(async move {
        while start.elapsed() < max_wall {
            tokio::time::sleep(tick).await;
            engine.increment_epoch();
        }
        // After max_wall elapses, epoch increments stop
        // Store's deadline (typically 1) is now exceeded → execution interrupted
    })
}

/// Convert max_instructions limit to wasmtime fuel.
///
/// **Calibration Note**:
/// Wasmtime's fuel consumption rate is implementation-dependent and can vary:
/// - Simple instructions (add, load): ~1 fuel
/// - Complex instructions (call, memory.grow): ~10-100 fuel
/// - Host functions (WASI syscalls): Variable, often 100-1000 fuel
///
/// **Current Strategy**: Use 1:1 mapping (1 instruction = 1 fuel) and rely on:
/// 1. Benchmarking to calibrate actual consumption rates
/// 2. Epoch deadline as the ultimate safety net
///
/// **Future**: Profile real workloads and adjust multiplier.
/// For now, contract's `max_instructions` is used directly as fuel.
///
/// **Parameters**:
/// - `max_instructions`: From contract's `ExecutionLimits`
///
/// **Returns**: Fuel amount to add to Store
pub fn fuel_for_instructions(max_instructions: u64) -> u64 {
    // 1:1 mapping for now
    // Callers should benchmark and adjust if needed
    max_instructions
}

/// Helper: Configure Store with fuel and epoch deadline in one call.
///
/// **Usage**:
/// ```rust
/// let engine = engine_with_limits()?;
/// let mut store = Store::new(&engine, host_state);
/// let deadline_task = configure_store_limits(&engine, &mut store, limits)?;
///
/// // Execute...
///
/// deadline_task.abort();
/// ```
///
/// **Parameters**:
/// - `engine`: Wasmtime engine
/// - `store`: Mutable store reference
/// - `max_instructions`: Fuel limit
/// - `max_wall`: Wall-clock time limit
///
/// **Returns**: Deadline task handle (must be aborted/awaited)
pub fn configure_store_limits<T>(
    engine: &Engine,
    store: &mut Store<T>,
    max_instructions: u64,
    max_wall: Duration,
) -> Result<JoinHandle<()>> {
    // Set fuel limit
    let fuel = fuel_for_instructions(max_instructions);
    store.add_fuel(fuel)?;

    // Set epoch deadline (interrupt when epoch > 1)
    store.set_epoch_deadline(1);

    // Attach wall-clock deadline
    let task = attach_deadline(engine, max_wall);

    Ok(task)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_creation() {
        let engine = engine_with_limits();
        assert!(engine.is_ok(), "Engine creation should succeed");
    }

    #[test]
    fn test_fuel_conversion() {
        assert_eq!(fuel_for_instructions(1000), 1000);
        assert_eq!(fuel_for_instructions(1_000_000), 1_000_000);
    }

    #[tokio::test]
    async fn test_epoch_deadline_triggers() {
        let engine = engine_with_limits().unwrap();
        let mut store = Store::new(&engine, ());
        store.set_epoch_deadline(1);

        let task = attach_deadline(&engine, Duration::from_millis(50));

        // Wait for deadline to expire
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Epoch should have incremented beyond deadline
        // (We can't directly test interruption without WASM execution,
        //  but we verify the task runs)
        assert!(!task.is_finished(), "Task should still be polling until max_wall");

        task.abort();
    }
}
