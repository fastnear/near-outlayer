//! Property Test: Deterministic Execution
//!
//! **Property 1**: Given identical inputs (same sealed state + same receipt),
//! the TEE must produce identical outputs every single time.
//!
//! This test runs 512+ randomized scenarios and verifies bit-for-bit replay.

use outlayer_verification_suite::strategies::*;
use outlayer_verification_suite::*;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 512,
        max_shrink_iters: 1000,
        .. ProptestConfig::default()
    })]

    #[test]
    fn deterministic_replay(
        state in arb_sealed_state(),
        receipt in arb_receipt()
    ) {
        let tee = mock_tee();

        // First execution
        let r1 = tee.execute(state.clone(), receipt.clone());

        // Second execution (identical inputs)
        let r2 = tee.execute(state.clone(), receipt.clone());

        // CRITICAL: Results must be identical (bit-for-bit)
        prop_assert_eq!(
            r1,
            r2,
            "Non-deterministic TEE execution observed! \
             Same inputs produced different outputs."
        );
    }
}
