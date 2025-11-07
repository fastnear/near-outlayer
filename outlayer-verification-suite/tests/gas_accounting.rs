//! Property Test: Gas Accounting
//!
//! **Property**: Gas threshold enforcement must be deterministic and safe.
//!
//! Based on nearcore/runtime/src/config.rs gas accounting patterns:
//! - Insufficient gas → GasExhausted error
//! - Sufficient gas → execution proceeds
//! - No arithmetic overflows
//!
//! This mirrors nearcore's prepaid gas validation.

use outlayer_verification_suite::strategies::*;
use outlayer_verification_suite::*;
use proptest::prelude::*;

/// Compute minimum required gas for a receipt
/// (Mirrors the cost model in MockOutlayerTEE)
fn min_required_gas(args_len: usize) -> u64 {
    (args_len + 32) as u64
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 512,
        max_shrink_iters: 1000,
        .. ProptestConfig::default()
    })]

    /// If gas_attached < min_required_gas, execution must fail
    #[test]
    fn insufficient_gas_rejected(
        state in arb_sealed_state(),
        mut receipt in arb_receipt()
    ) {
        let tee = mock_tee();

        // Calculate minimum gas needed
        let need = min_required_gas(receipt.args.0.len());

        // Attach insufficient gas (need - 1, but at least 1)
        let insufficient = need.saturating_sub(1).max(1);
        receipt.gas_attached = insufficient;

        // Execute with insufficient gas
        let res = tee.execute(state, receipt);

        // CRITICAL: Must reject with GasExhausted
        prop_assert_eq!(
            res,
            Err(EnclaveError::GasExhausted),
            "TEE allowed execution with insufficient gas! \
             Need: {}, Attached: {}",
            need,
            insufficient
        );
    }

    /// If gas_attached >= min_required_gas, execution should not fail for gas reasons
    #[test]
    fn sufficient_gas_allows_progress(
        state in arb_sealed_state(),
        mut receipt in arb_receipt()
    ) {
        let tee = mock_tee();

        // Calculate minimum gas needed
        let need = min_required_gas(receipt.args.0.len());

        // Attach exactly the minimum gas
        receipt.gas_attached = need;

        // Execute
        let res = tee.execute(state, receipt);

        // Should either succeed or fail for non-gas reasons
        prop_assert!(
            res.is_ok() || matches!(res, Err(EnclaveError::CapabilityViolation) | Err(EnclaveError::IntegrityError)),
            "Execution should not fail for gas when gas >= required. \
             Got: {:?}",
            res
        );
    }

    /// Zero gas is always rejected
    #[test]
    fn zero_gas_rejected(
        state in arb_sealed_state(),
        mut receipt in arb_receipt()
    ) {
        let tee = mock_tee();

        // Force zero gas
        receipt.gas_attached = 0;

        let res = tee.execute(state, receipt);

        // Must reject (either GasExhausted or validation error)
        prop_assert!(
            res.is_err(),
            "TEE allowed execution with zero gas!"
        );
    }
}
