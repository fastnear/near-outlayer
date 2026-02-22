//! Wallet host functions for WASM components
//!
//! Provides wallet operations (get-id, get-address, withdraw, etc.)
//! when X-Wallet-Id is present in the execution request.

pub mod host_functions;

pub use host_functions::{WalletHostState, add_wallet_to_linker};
