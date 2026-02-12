//! VRF host functions for WASM components
//!
//! Provides verifiable random output using Ed25519 deterministic signatures.
//! Request_id is auto-prepended to user seed — WASM cannot manipulate it.

pub mod host_functions;

pub use host_functions::{VrfHostState, add_vrf_to_linker};
