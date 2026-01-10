//! OutLayer SDK for WASM components
//!
//! This crate provides a high-level API for OutLayer off-chain WASM execution on NEAR.
//!
//! ## Features
//!
//! - **Storage**: Persistent encrypted storage across executions
//! - **Environment**: Access to execution context (signer, input/output)
//! - **Metadata**: Required metadata for project-based execution
//!
//! ## Requirements
//!
//! OutLayer SDK requires **wasm32-wasip2** target (WASI Preview 2 / Component Model).
//! WASI Preview 1 (wasm32-wasip1) is NOT supported for storage and RPC features.
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use outlayer::{metadata, storage, env};
//!
//! // Required for project-based execution
//! metadata! {
//!     project: "alice.near/my-app",
//!     version: "1.0.0",
//! }
//!
//! fn main() {
//!     // Get input from execution request
//!     let input = env::input();
//!
//!     // Use persistent storage
//!     storage::set("counter", b"42").unwrap();
//!     let value = storage::get("counter").unwrap();
//!
//!     // Return output
//!     env::output(b"result");
//! }
//! ```
//!
//! ## Compile-Time Target Check
//!
//! This crate will fail to compile if you target wasm32-wasip1:
//!
//! ```bash
//! # Correct - will work:
//! cargo build --target wasm32-wasip2 --release
//!
//! # Wrong - will fail to compile:
//! cargo build --target wasm32-wasip1 --release
//! ```

// Compile-time check: OutLayer SDK requires wasm32-wasip2
#[cfg(all(
    target_arch = "wasm32",
    target_os = "wasi",
    not(target_env = "p2")
))]
compile_error!(
    "OutLayer SDK requires wasm32-wasip2 target (WASI Preview 2). You are compiling with wasm32-wasip1 which does not support OutLayer host functions."
);

// Generate bindings from WIT
wit_bindgen::generate!({
    world: "outlayer-host",
    path: "wit",
    with: {
        "near:storage/api@0.1.0": generate,
    },
});

pub mod storage;
pub mod env;

// Re-export the metadata macro
pub use outlayer_macros::metadata;

/// Low-level access to generated WIT bindings
///
/// Most users should use the high-level `storage` and `env` modules instead.
pub mod raw {
    pub use super::near::rpc::api as rpc;
    pub use super::near::storage::api as storage;
}
