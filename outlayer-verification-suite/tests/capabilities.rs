//! Property Test: Capability Scope Enforcement
//!
//! **Property 3**: The TEE must operate within its access key constraints.
//! No generated callback can violate {receiver, methods, gas_allowance}.
//!
//! This test runs 512+ randomized scenarios and verifies all callbacks are authorized.

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
    fn capability_scope_enforced(
        state in arb_sealed_state(),
        receipt in arb_receipt()
    ) {
        let tee = mock_tee();
        let policy = &tee.constraints;

        // Execute (may succeed or fail for various reasons)
        if let Ok(out) = tee.execute(state, receipt) {
            // For every callback generated, verify constraints
            for cb in out.callbacks {
                // 1. Receiver must match allowed_receiver
                prop_assert_eq!(
                    &cb.receiver_id,
                    &policy.allowed_receiver,
                    "TEE generated callback to disallowed receiver: {}",
                    cb.receiver_id
                );

                // 2. Method must be in allowed_methods
                prop_assert!(
                    policy.allowed_methods.contains(&cb.method_name),
                    "TEE generated callback with disallowed method: {}",
                    cb.method_name
                );

                // 3. Gas must not exceed allowance
                prop_assert!(
                    cb.gas_attached <= policy.gas_allowance,
                    "TEE generated callback with excess gas: {} > {}",
                    cb.gas_attached,
                    policy.gas_allowance
                );
            }
        }
        // If execution failed, that's fine - we only care about successful callbacks
    }
}
