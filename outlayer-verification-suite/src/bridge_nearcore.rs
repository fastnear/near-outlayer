//! Nearcore Oracle Bridge
//!
//! This module provides a bridge to the real NEAR Protocol runtime for differential testing.
//!
//! ## Purpose
//!
//! By comparing our mock TEE against nearcore's actual runtime (`apply()`), we can:
//! - Verify our implementation matches NEAR Protocol's behavior
//! - Catch divergence early
//! - Use battle-tested production code as ground truth
//!
//! ## Implementation Tasks
//!
//! TODO (engineer): Wire this to `nearcore/runtime/src/lib.rs::apply()`
//!
//! 1. Create in-memory trie for state storage
//! 2. Map `Receipt` → `near_primitives::runtime::FunctionCall`
//! 3. Call `apply()` with test config
//! 4. Map outcomes → `ExecutionOutput`
//! 5. Handle gas accounting via `near-parameters`
//!
//! ## Example Usage
//!
//! ```rust,ignore
//! let oracle = NearcoreOracle::new(test_config());
//! let result = oracle.execute(state, receipt)?;
//! ```

use crate::*;

#[derive(Clone, Debug)]
pub struct NearcoreOracleConfig {
    // TODO: Add fields from nearcore runtime config:
    // - protocol_version
    // - gas_config (from near-parameters)
    // - wasm_config
    // - account_creation_config
}

impl Default for NearcoreOracleConfig {
    fn default() -> Self {
        Self {
            // TODO: Initialize with test defaults from nearcore
        }
    }
}

#[derive(Clone, Debug)]
pub struct NearcoreOracle {
    pub cfg: NearcoreOracleConfig,
}

impl NearcoreOracle {
    pub fn new(cfg: NearcoreOracleConfig) -> Self {
        Self { cfg }
    }

    /// Execute receipt using nearcore runtime
    ///
    /// TODO (engineer): Implement by calling nearcore::runtime::apply()
    ///
    /// Steps:
    /// 1. Create in-memory Trie
    /// 2. Build ApplyState with test config
    /// 3. Convert Receipt → near_primitives::runtime::receipt_manager::Receipt
    /// 4. Call apply() with single FunctionCall action
    /// 5. Extract outcomes (gas used, return data, errors)
    /// 6. Map to ExecutionOutput
    ///
    /// Reference: nearcore/runtime/src/tests/apply.rs for test patterns
    pub fn execute(
        &self,
        state: SealedState,
        receipt: Receipt,
    ) -> Result<ExecutionOutput, EnclaveError> {
        // TEMPORARY: Delegate to mock until wired
        // Remove this once nearcore integration complete
        let tee = MockOutlayerTEE {
            constraints: crate::strategies::allowed_constraints(),
        };
        tee.execute(state, receipt)

        // TODO: Replace with:
        // let trie = create_test_trie();
        // let apply_state = build_apply_state(&self.cfg);
        // let actions = vec![Action::FunctionCall(FunctionCallAction {
        //     method_name: receipt.method_name,
        //     args: receipt.args.0,
        //     gas: receipt.gas_attached,
        //     deposit: 0,
        // })];
        // let result = nearcore::runtime::apply(
        //     &trie,
        //     &apply_state,
        //     &actions,
        //     &receipt.signer_id,
        // )?;
        // map_nearcore_result_to_output(result)
    }
}

/// TODO: Helper to create in-memory trie
// fn create_test_trie() -> Trie { ... }

/// TODO: Helper to build ApplyState from config
// fn build_apply_state(cfg: &NearcoreOracleConfig) -> ApplyState { ... }

/// TODO: Helper to map nearcore results to our ExecutionOutput
// fn map_nearcore_result_to_output(result: ApplyResult) -> Result<ExecutionOutput, EnclaveError> { ... }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oracle_config_default() {
        let cfg = NearcoreOracleConfig::default();
        let oracle = NearcoreOracle::new(cfg);
        // TODO: Add assertions when config populated
    }
}
