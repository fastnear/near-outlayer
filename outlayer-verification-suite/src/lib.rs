#![forbid(unsafe_code)]
#![deny(warnings)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::dbg_macro)]

//! Outlayer Verification Suite
//!
//! Property-based testing harness for NEAR OutLayer TEE security properties.
//!
//! ## Properties Verified
//!
//! 1. **Determinism**: Same inputs → same outputs (bit-for-bit)
//! 2. **Capabilities**: No unauthorized operations escape
//! 3. **State Integrity**: Tampering always detected
//! 4. **Gas Accounting**: Insufficient gas → rejection
//!
//! ## Usage
//!
//! ```bash
//! # Run with mock TEE (fast)
//! cargo test
//!
//! # Run with nearcore oracle (validates against real NEAR runtime)
//! cargo test --features nearcore-oracle
//!
//! # Run with real wasmtime execution
//! cargo test --features engine-wasmtime
//! ```

use std::collections::HashSet;

pub type AccountId = String;
pub type Gas = u64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReceiptId(pub [u8; 32]);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionArgs(pub Vec<u8>);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SealedState {
    pub data: Vec<u8>,
    pub integrity_hash: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Receipt {
    pub id: ReceiptId,
    pub signer_id: AccountId,
    pub receiver_id: AccountId,
    pub method_name: String,
    pub args: FunctionArgs,
    pub gas_attached: Gas,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionOutput {
    pub new_state: SealedState,
    pub callbacks: Vec<Receipt>,
    pub logs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessKeyConstraints {
    pub allowed_receiver: AccountId,
    pub allowed_methods: HashSet<String>,
    pub gas_allowance: Gas,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EnclaveError {
    IntegrityError,
    CapabilityViolation,
    GasExhausted,
    ExecutionPanic(String),
}

#[derive(Clone, Debug)]
pub struct MockOutlayerTEE {
    pub constraints: AccessKeyConstraints,
}

impl MockOutlayerTEE {
    pub fn execute(
        &self,
        state: SealedState,
        receipt: Receipt,
    ) -> Result<ExecutionOutput, EnclaveError> {
        // 1) Sealed-state integrity check (BLAKE3-based)
        let expected = hash32(&state.data);
        if state.integrity_hash != expected {
            return Err(EnclaveError::IntegrityError);
        }

        // 2) Gas accounting (minimal deterministic cost model)
        // Cost = len(args) + 32 (baseline overhead)
        let exec_cost_units = (receipt.args.0.len() + 32) as u64;
        if receipt.gas_attached == 0 || exec_cost_units > receipt.gas_attached {
            return Err(EnclaveError::GasExhausted);
        }

        // 3) Deterministic state transition
        let mut new_data = state.data.clone();
        new_data.extend_from_slice(&(receipt.id.0));
        new_data.extend_from_slice(&(receipt.args.0));

        let mut callbacks = Vec::new();
        let mut logs = vec![format!(
            "Processed {:02x}{:02x}{:02x}{:02x}…",
            receipt.id.0[0], receipt.id.0[1], receipt.id.0[2], receipt.id.0[3]
        )];

        // 4) Capability-gated callback generation
        if receipt.method_name == "do_something_and_callback" {
            let cb_method = "on_something_done".to_string();
            let cb_gas: Gas = 5_000_000_000_000;

            // Enforce capability constraints
            if !self.constraints.allowed_methods.contains(&cb_method) {
                return Err(EnclaveError::CapabilityViolation);
            }
            if cb_gas > self.constraints.gas_allowance {
                return Err(EnclaveError::CapabilityViolation);
            }

            callbacks.push(Receipt {
                id: ReceiptId(gen_id(&new_data)),
                signer_id: receipt.receiver_id.clone(),
                receiver_id: self.constraints.allowed_receiver.clone(),
                method_name: cb_method,
                args: FunctionArgs(vec![1, 2, 3]),
                gas_attached: cb_gas,
            });
            logs.push("Emitted callback on_something_done".into());
        }

        // 5) Reseal state with BLAKE3
        let new_hash = hash32(&new_data);
        let new_state = SealedState {
            data: new_data,
            integrity_hash: new_hash,
        };

        Ok(ExecutionOutput {
            new_state,
            callbacks,
            logs,
        })
    }
}

/// BLAKE3 hash (deterministic, fast)
pub fn hash32(data: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    let h = blake3::hash(data);
    out.copy_from_slice(h.as_bytes());
    out
}

/// Domain-separated ID generation
pub fn gen_id(data: &[u8]) -> [u8; 32] {
    let mut tagged = b"outlayer:id:".to_vec();
    tagged.extend_from_slice(data);
    hash32(&tagged)
}

pub mod strategies;

#[cfg(feature = "nearcore-oracle")]
pub mod bridge_nearcore;
