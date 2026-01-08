//! Persistent storage API for OutLayer WASM components
//!
//! Storage is encrypted and persisted across executions. For projects, storage
//! is shared across all versions (same encryption key derived from project UUID).
//!
//! ## Basic Usage
//!
//! ```rust,ignore
//! use outlayer::storage;
//!
//! // Store a value
//! storage::set("my-key", b"my-value")?;
//!
//! // Retrieve a value
//! if let Some(value) = storage::get("my-key")? {
//!     println!("Got: {:?}", value);
//! }
//!
//! // Check if key exists
//! if storage::has("my-key") {
//!     println!("Key exists!");
//! }
//!
//! // Delete a key
//! storage::delete("my-key");
//!
//! // List keys with prefix
//! let keys = storage::list_keys("prefix:")?;
//! ```
//!
//! ## Worker-Private Storage
//!
//! Worker-private storage is only accessible from within WASM code, not by the user.
//! Use this for internal state that shouldn't be exposed.
//!
//! ```rust,ignore
//! use outlayer::storage;
//!
//! // Store worker-private data
//! storage::set_worker("internal-state", b"secret")?;
//!
//! // Retrieve worker-private data
//! let state = storage::get_worker("internal-state")?;
//! ```
//!
//! ## Version Migration
//!
//! When upgrading your WASM, you can read data from a previous version:
//!
//! ```rust,ignore
//! use outlayer::storage;
//!
//! // Read from previous WASM version (by its SHA256 hash)
//! let old_data = storage::get_by_version("my-key", "abc123...")?;
//! ```

use crate::near::storage::api as raw;

/// Storage error
#[derive(Debug, Clone)]
pub struct StorageError(pub String);

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Storage error: {}", self.0)
    }
}

impl std::error::Error for StorageError {}

/// Result type for storage operations
pub type Result<T> = std::result::Result<T, StorageError>;

/// Store a value by key
///
/// # Arguments
/// * `key` - The key to store the value under
/// * `value` - The value to store (as bytes)
///
/// # Returns
/// * `Ok(())` - Value stored successfully
/// * `Err(StorageError)` - Storage operation failed
///
/// # Example
/// ```rust,ignore
/// storage::set("user:123", b"Alice")?;
/// storage::set("config", serde_json::to_vec(&config)?)?;
/// ```
pub fn set(key: &str, value: &[u8]) -> Result<()> {
    let error = raw::set(key, value);
    if error.is_empty() {
        Ok(())
    } else {
        Err(StorageError(error))
    }
}

/// Get a value by key
///
/// # Arguments
/// * `key` - The key to retrieve
///
/// # Returns
/// * `Ok(Some(bytes))` - Value found
/// * `Ok(None)` - Key doesn't exist
/// * `Err(StorageError)` - Storage operation failed
///
/// # Example
/// ```rust,ignore
/// if let Some(data) = storage::get("user:123")? {
///     let name = String::from_utf8(data)?;
///     println!("User name: {}", name);
/// }
/// ```
pub fn get(key: &str) -> Result<Option<Vec<u8>>> {
    let (data, error) = raw::get(key);
    if !error.is_empty() {
        return Err(StorageError(error));
    }
    if data.is_empty() {
        Ok(None)
    } else {
        Ok(Some(data))
    }
}

/// Check if a key exists
///
/// # Arguments
/// * `key` - The key to check
///
/// # Returns
/// * `true` - Key exists
/// * `false` - Key doesn't exist
///
/// # Example
/// ```rust,ignore
/// if storage::has("session:abc") {
///     // Session exists, continue
/// } else {
///     // Create new session
/// }
/// ```
pub fn has(key: &str) -> bool {
    raw::has(key)
}

/// Delete a key
///
/// # Arguments
/// * `key` - The key to delete
///
/// # Returns
/// * `true` - Key existed and was deleted
/// * `false` - Key didn't exist
///
/// # Example
/// ```rust,ignore
/// if storage::delete("session:abc") {
///     println!("Session deleted");
/// }
/// ```
pub fn delete(key: &str) -> bool {
    raw::delete(key)
}

/// List all keys with optional prefix filter
///
/// # Arguments
/// * `prefix` - Prefix to filter keys (empty string for all keys)
///
/// # Returns
/// * `Ok(Vec<String>)` - List of matching keys
/// * `Err(StorageError)` - Storage operation failed
///
/// # Example
/// ```rust,ignore
/// // List all keys
/// let all_keys = storage::list_keys("")?;
///
/// // List only user keys
/// let user_keys = storage::list_keys("user:")?;
/// for key in user_keys {
///     println!("Found user key: {}", key);
/// }
/// ```
pub fn list_keys(prefix: &str) -> Result<Vec<String>> {
    let (keys_json, error) = raw::list_keys(prefix);
    if !error.is_empty() {
        return Err(StorageError(error));
    }
    serde_json::from_str(&keys_json)
        .map_err(|e| StorageError(format!("Failed to parse keys list: {}", e)))
}

/// Store worker-private data
///
/// Worker-private storage is only accessible from within WASM code.
/// The user cannot read this data through the API.
///
/// # Arguments
/// * `key` - The key to store the value under
/// * `value` - The value to store (as bytes)
///
/// # Returns
/// * `Ok(())` - Value stored successfully
/// * `Err(StorageError)` - Storage operation failed
///
/// # Example
/// ```rust,ignore
/// // Store internal state that user shouldn't see
/// storage::set_worker("last_run_timestamp", &timestamp.to_le_bytes())?;
/// ```
pub fn set_worker(key: &str, value: &[u8]) -> Result<()> {
    let error = raw::set_worker(key, value);
    if error.is_empty() {
        Ok(())
    } else {
        Err(StorageError(error))
    }
}

/// Get worker-private data
///
/// # Arguments
/// * `key` - The key to retrieve
///
/// # Returns
/// * `Ok(Some(bytes))` - Value found
/// * `Ok(None)` - Key doesn't exist
/// * `Err(StorageError)` - Storage operation failed
///
/// # Example
/// ```rust,ignore
/// if let Some(data) = storage::get_worker("last_run_timestamp")? {
///     let timestamp = u64::from_le_bytes(data.try_into()?);
///     println!("Last run: {}", timestamp);
/// }
/// ```
pub fn get_worker(key: &str) -> Result<Option<Vec<u8>>> {
    let (data, error) = raw::get_worker(key);
    if !error.is_empty() {
        return Err(StorageError(error));
    }
    if data.is_empty() {
        Ok(None)
    } else {
        Ok(Some(data))
    }
}

/// Get data from a specific WASM version (for migration)
///
/// Use this when upgrading your WASM to read data written by a previous version.
///
/// # Arguments
/// * `key` - The key to retrieve
/// * `wasm_hash` - SHA256 hash of the previous WASM version
///
/// # Returns
/// * `Ok(Some(bytes))` - Value found
/// * `Ok(None)` - Key doesn't exist for that version
/// * `Err(StorageError)` - Storage operation failed
///
/// # Example
/// ```rust,ignore
/// // Migrate data from previous version
/// const OLD_VERSION_HASH: &str = "abc123...";
///
/// if let Some(old_data) = storage::get_by_version("config", OLD_VERSION_HASH)? {
///     // Transform old data format to new format
///     let new_data = migrate_config(old_data);
///     storage::set("config", &new_data)?;
/// }
/// ```
pub fn get_by_version(key: &str, wasm_hash: &str) -> Result<Option<Vec<u8>>> {
    let (data, error) = raw::get_by_version(key, wasm_hash);
    if !error.is_empty() {
        return Err(StorageError(error));
    }
    if data.is_empty() {
        Ok(None)
    } else {
        Ok(Some(data))
    }
}

/// Clear all storage for the current project/account
///
/// **WARNING**: This deletes ALL data. Use with caution!
///
/// # Returns
/// * `Ok(())` - Storage cleared successfully
/// * `Err(StorageError)` - Operation failed
///
/// # Example
/// ```rust,ignore
/// // Clear all storage (dangerous!)
/// storage::clear_all()?;
/// ```
pub fn clear_all() -> Result<()> {
    let error = raw::clear_all();
    if error.is_empty() {
        Ok(())
    } else {
        Err(StorageError(error))
    }
}

/// Clear storage written by a specific WASM version
///
/// Use this to clean up data from old versions after migration.
///
/// # Arguments
/// * `wasm_hash` - SHA256 hash of the WASM version to clear
///
/// # Returns
/// * `Ok(())` - Storage cleared successfully
/// * `Err(StorageError)` - Operation failed
///
/// # Example
/// ```rust,ignore
/// // After successful migration, clear old version's data
/// storage::clear_version("abc123...")?;
/// ```
pub fn clear_version(wasm_hash: &str) -> Result<()> {
    let error = raw::clear_version(wasm_hash);
    if error.is_empty() {
        Ok(())
    } else {
        Err(StorageError(error))
    }
}

// ==================== Convenience Functions ====================

/// Store a string value
///
/// Convenience wrapper around `set()` for string values.
///
/// # Example
/// ```rust,ignore
/// storage::set_string("name", "Alice")?;
/// ```
pub fn set_string(key: &str, value: &str) -> Result<()> {
    set(key, value.as_bytes())
}

/// Get a string value
///
/// Convenience wrapper around `get()` that returns a String.
///
/// # Returns
/// * `Ok(Some(String))` - Value found and valid UTF-8
/// * `Ok(None)` - Key doesn't exist
/// * `Err(StorageError)` - Storage error or invalid UTF-8
///
/// # Example
/// ```rust,ignore
/// if let Some(name) = storage::get_string("name")? {
///     println!("Hello, {}!", name);
/// }
/// ```
pub fn get_string(key: &str) -> Result<Option<String>> {
    match get(key)? {
        Some(data) => {
            String::from_utf8(data)
                .map(Some)
                .map_err(|e| StorageError(format!("Invalid UTF-8: {}", e)))
        }
        None => Ok(None),
    }
}

/// Store a JSON-serializable value
///
/// Serializes the value to JSON and stores it.
///
/// # Example
/// ```rust,ignore
/// #[derive(Serialize)]
/// struct Config {
///     max_retries: u32,
///     timeout_ms: u64,
/// }
///
/// let config = Config { max_retries: 3, timeout_ms: 5000 };
/// storage::set_json("config", &config)?;
/// ```
pub fn set_json<T: serde::Serialize>(key: &str, value: &T) -> Result<()> {
    let json = serde_json::to_vec(value)
        .map_err(|e| StorageError(format!("JSON serialization failed: {}", e)))?;
    set(key, &json)
}

/// Get a JSON-deserializable value
///
/// Retrieves the value and deserializes from JSON.
///
/// # Returns
/// * `Ok(Some(T))` - Value found and deserialized
/// * `Ok(None)` - Key doesn't exist
/// * `Err(StorageError)` - Storage error or JSON parse error
///
/// # Example
/// ```rust,ignore
/// #[derive(Deserialize)]
/// struct Config {
///     max_retries: u32,
///     timeout_ms: u64,
/// }
///
/// if let Some(config) = storage::get_json::<Config>("config")? {
///     println!("Max retries: {}", config.max_retries);
/// }
/// ```
pub fn get_json<T: serde::de::DeserializeOwned>(key: &str) -> Result<Option<T>> {
    match get(key)? {
        Some(data) => {
            serde_json::from_slice(&data)
                .map(Some)
                .map_err(|e| StorageError(format!("JSON deserialization failed: {}", e)))
        }
        None => Ok(None),
    }
}
