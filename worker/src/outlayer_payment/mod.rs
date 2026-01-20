//! Payment host functions for WASM components
//!
//! Allows WASI modules to refund part of attached_usd back to caller.

pub mod host_functions;

pub use host_functions::{PaymentHostState, add_payment_to_linker};
