//! Proptest strategies for generating test data
//!
//! These strategies generate diverse, edge-case-rich inputs for property testing.

use super::*;
use proptest::prelude::*;
use std::collections::HashSet;

/// Strategy: Generate valid NEAR account IDs
pub fn arb_account_id() -> impl Strategy<Value = AccountId> {
    proptest::string::string_regex("[a-z0-9\\-]{2,48}\\.near").unwrap()
}

/// Strategy: Generate function arguments (0-1024 bytes)
pub fn arb_args() -> impl Strategy<Value = FunctionArgs> {
    prop::collection::vec(any::<u8>(), 0..1024).prop_map(FunctionArgs)
}

/// Strategy: Generate receipt IDs (32 random bytes)
pub fn arb_receipt_id() -> impl Strategy<Value = ReceiptId> {
    prop::array::uniform32(any::<u8>()).prop_map(ReceiptId)
}

/// Strategy: Generate valid sealed states (with correct BLAKE3 hash)
pub fn arb_sealed_state() -> impl Strategy<Value = SealedState> {
    prop::collection::vec(any::<u8>(), 0..4096).prop_map(|data| SealedState {
        integrity_hash: hash32(&data),
        data,
    })
}

/// Strategy: Generate method names (common + random)
pub fn arb_method_name() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("do_something_and_callback".to_string()),
        Just("do_something_else".to_string()),
        proptest::string::string_regex("[a-z_]{5,30}").unwrap()
    ]
}

/// Strategy: Generate gas amounts (1 to 300 Tgas)
pub fn arb_gas() -> impl Strategy<Value = Gas> {
    1u64..300_000_000_000_000u64
}

/// Strategy: Generate complete receipts
pub fn arb_receipt() -> impl Strategy<Value = Receipt> {
    (
        arb_receipt_id(),
        arb_account_id(),
        arb_account_id(),
        arb_method_name(),
        arb_args(),
        arb_gas(),
    )
        .prop_map(
            |(id, signer_id, receiver_id, method_name, args, gas_attached)| Receipt {
                id,
                signer_id,
                receiver_id,
                method_name,
                args,
                gas_attached,
            },
        )
}

/// Standard access key constraints for testing
pub fn allowed_constraints() -> AccessKeyConstraints {
    AccessKeyConstraints {
        allowed_receiver: "callback.near".to_string(),
        allowed_methods: HashSet::from_iter(
            [
                "on_something_done".to_string(),
                "on_failure".to_string(),
            ]
            .into_iter(),
        ),
        gas_allowance: 300_000_000_000_000, // 300 Tgas
    }
}

/// Create mock TEE with standard constraints
pub fn mock_tee() -> MockOutlayerTEE {
    MockOutlayerTEE {
        constraints: allowed_constraints(),
    }
}
