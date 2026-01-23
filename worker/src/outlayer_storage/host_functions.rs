//! Storage host functions for WASM components
//!
//! Implements the `near:storage/api` WIT interface.

use anyhow::Result;
use tracing::debug;
use wasmtime::component::Linker;

use super::client::{StorageClient, StorageConfig};

// Generate bindings from WIT (storage is now separate package near:storage)
wasmtime::component::bindgen!({
    path: "wit",
    world: "near:storage/storage-host",
});

/// Host state for storage functions
pub struct StorageHostState {
    client: StorageClient,
}

impl StorageHostState {
    /// Create new storage host state from config
    pub fn new(config: StorageConfig) -> Result<Self> {
        let client = StorageClient::new(config)?;
        Ok(Self { client })
    }

    /// Create new storage host state from existing client
    pub fn from_client(client: StorageClient) -> Self {
        Self { client }
    }
}

impl near::storage::api::Host for StorageHostState {
    fn set(&mut self, key: String, value: Vec<u8>) -> String {
        debug!("storage::set key={}, value_len={}", key, value.len());
        match self.client.set(&key, &value) {
            Ok(()) => String::new(),
            Err(e) => e.to_string(),
        }
    }

    fn get(&mut self, key: String) -> (Vec<u8>, String) {
        debug!("storage::get key={}", key);
        match self.client.get(&key) {
            Ok(Some(value)) => (value, String::new()),
            Ok(None) => (Vec::new(), String::new()),
            Err(e) => (Vec::new(), e.to_string()),
        }
    }

    fn has(&mut self, key: String) -> bool {
        debug!("storage::has key={}", key);
        self.client.has(&key).unwrap_or(false)
    }

    fn delete(&mut self, key: String) -> bool {
        debug!("storage::delete key={}", key);
        self.client.delete(&key).unwrap_or(false)
    }

    fn list_keys(&mut self, prefix: String) -> (String, String) {
        debug!("storage::list_keys prefix={}", prefix);
        match self.client.list_keys(&prefix) {
            Ok(keys) => (keys, String::new()),
            Err(e) => (String::from("[]"), e.to_string()),
        }
    }

    fn set_worker(&mut self, key: String, value: Vec<u8>, is_encrypted: Option<bool>) -> String {
        let encrypted = is_encrypted.unwrap_or(true);
        debug!("storage::set_worker key={}, value_len={}, is_encrypted={}", key, value.len(), encrypted);
        match self.client.set_worker(&key, &value, encrypted) {
            Ok(()) => String::new(),
            Err(e) => e.to_string(),
        }
    }

    fn get_worker(&mut self, key: String, project: Option<String>) -> (Vec<u8>, String) {
        debug!("storage::get_worker key={}, project={:?}", key, project);
        match self.client.get_worker(&key, project.as_deref()) {
            Ok(Some(value)) => (value, String::new()),
            Ok(None) => (Vec::new(), String::new()),
            Err(e) => (Vec::new(), e.to_string()),
        }
    }

    fn get_by_version(&mut self, key: String, wasm_hash: String) -> (Vec<u8>, String) {
        debug!("storage::get_by_version key={}, wasm_hash={}", key, wasm_hash);
        match self.client.get_by_version(&key, &wasm_hash) {
            Ok(Some(value)) => (value, String::new()),
            Ok(None) => (Vec::new(), String::new()),
            Err(e) => (Vec::new(), e.to_string()),
        }
    }

    fn clear_all(&mut self) -> String {
        debug!("storage::clear_all");
        match self.client.clear_all() {
            Ok(()) => String::new(),
            Err(e) => e.to_string(),
        }
    }

    fn clear_version(&mut self, wasm_hash: String) -> String {
        debug!("storage::clear_version wasm_hash={}", wasm_hash);
        match self.client.clear_version(&wasm_hash) {
            Ok(()) => String::new(),
            Err(e) => e.to_string(),
        }
    }

    // ==================== Conditional Write Operations ====================

    fn set_if_absent(&mut self, key: String, value: Vec<u8>) -> (bool, String) {
        debug!("storage::set_if_absent key={}, value_len={}", key, value.len());
        match self.client.set_if_absent(&key, &value) {
            Ok(inserted) => (inserted, String::new()),
            Err(e) => (false, e.to_string()),
        }
    }

    fn set_if_equals(&mut self, key: String, expected: Vec<u8>, new_value: Vec<u8>) -> (bool, Vec<u8>, String) {
        debug!("storage::set_if_equals key={}, expected_len={}, new_len={}", key, expected.len(), new_value.len());
        match self.client.set_if_equals(&key, &expected, &new_value) {
            Ok((success, current)) => (success, current.unwrap_or_default(), String::new()),
            Err(e) => (false, Vec::new(), e.to_string()),
        }
    }

    fn increment(&mut self, key: String, delta: i64) -> (i64, String) {
        debug!("storage::increment key={}, delta={}", key, delta);
        match self.client.increment(&key, delta) {
            Ok(new_value) => (new_value, String::new()),
            Err(e) => (0, e.to_string()),
        }
    }

    fn decrement(&mut self, key: String, delta: i64) -> (i64, String) {
        debug!("storage::decrement key={}, delta={}", key, delta);
        match self.client.decrement(&key, delta) {
            Ok(new_value) => (new_value, String::new()),
            Err(e) => (0, e.to_string()),
        }
    }
}

/// Add storage host functions to a wasmtime component linker
pub fn add_storage_to_linker<T: Send + 'static>(
    linker: &mut Linker<T>,
    get_state: impl Fn(&mut T) -> &mut StorageHostState + Send + Sync + Copy + 'static,
) -> Result<()> {
    near::storage::api::add_to_linker(linker, get_state)
}
