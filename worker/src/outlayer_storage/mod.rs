//! OutLayer Persistent Storage for WASM host functions
//!
//! This module provides storage host functions that allow WASM code
//! to persist data across executions. Storage is encrypted and stored
//! in the coordinator's PostgreSQL database.
//!
//! ## Architecture
//!
//! ```text
//! WASM Code (outlayer crate)
//!     │ extern "C" calls via WIT
//!     ▼
//! Host Functions (this module)
//!     │ calls StorageClient
//!     ▼
//! StorageClient (HTTP to Coordinator)
//!     │ encrypted data
//!     ▼
//! Coordinator API (/storage/*)
//!     │
//!     ▼
//! PostgreSQL (storage_data table)
//! ```
//!
//! ## Security
//!
//! - All data is encrypted before storage using `derive_key(master_key, project_uuid, account_id)`
//! - Keys are hashed (SHA256) for unique constraint without exposing plaintext
//! - `@worker` account_id for WASM-private storage (not accessible by users)
//! - wasm_hash stored with each write for version migration support

pub mod client;
pub mod host_functions;

pub use client::{StorageClient, StorageConfig};
pub use host_functions::{add_storage_to_linker, StorageHostState};
