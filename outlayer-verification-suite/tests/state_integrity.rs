//! Property Test: Sealed State Integrity
//!
//! **Property 4**: The TEE must detect any tampering with sealed state.
//! If the state hash doesn't match the data, execution must fail with IntegrityError.
//!
//! This test runs 256+ randomized tampering attempts.

use outlayer_verification_suite::strategies::*;
use outlayer_verification_suite::*;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        max_shrink_iters: 1000,
        .. ProptestConfig::default()
    })]

    #[test]
    fn tamper_proof_state(
        mut state in arb_sealed_state(),
        receipt in arb_receipt(),
        idx in 0usize..4096
    ) {
        let tee = mock_tee();

        // Skip if state is empty or index out of bounds
        if state.data.is_empty() || idx >= state.data.len() {
            return Ok(());
        }

        // Tamper: flip one bit at random position
        state.data[idx] ^= 0x01;

        // Execute with tampered state
        let res = tee.execute(state, receipt);

        // CRITICAL: Must detect tampering and reject
        prop_assert_eq!(
            res,
            Err(EnclaveError::IntegrityError),
            "TEE failed to detect state tampering at byte {}! \
             BLAKE3 hash mismatch should have been caught.",
            idx
        );
    }
}
