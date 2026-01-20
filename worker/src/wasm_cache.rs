//! WASM LRU Cache
//!
//! Local cache for compiled WASM files to avoid re-downloading from coordinator.
//! Uses LRU eviction by total size (configurable via WASM_CACHE_MAX_SIZE_MB).
//!
//! Security: Each cached file stores its expected checksum and verifies it
//! before returning, preventing cache corruption or tampering.

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;
use tracing::{debug, info, warn};

/// Cache entry metadata
#[derive(Debug, Clone)]
struct CacheEntry {
    /// Path to cached WASM file
    path: PathBuf,
    /// File size in bytes
    size: u64,
    /// Expected SHA256 checksum (stored for debugging, verified by recomputing)
    #[allow(dead_code)]
    expected_checksum: String,
    /// Last access time (for LRU eviction)
    last_used: Instant,
}

/// WASM LRU Cache with size-based eviction
pub struct WasmCache {
    /// Cache directory
    dir: PathBuf,
    /// Maximum cache size in bytes
    max_size_bytes: u64,
    /// Cache entries: checksum -> entry
    entries: HashMap<String, CacheEntry>,
    /// Current total size
    total_size: u64,
}

impl WasmCache {
    /// Create new cache with specified max size in MB
    ///
    /// # Arguments
    /// * `cache_dir` - Directory to store cached files
    /// * `max_size_mb` - Maximum cache size in megabytes
    pub fn new(cache_dir: PathBuf, max_size_mb: u64) -> Result<Self> {
        // Create cache directory if it doesn't exist
        fs::create_dir_all(&cache_dir)
            .with_context(|| format!("Failed to create cache directory: {:?}", cache_dir))?;

        let max_size_bytes = max_size_mb * 1024 * 1024;

        info!(
            "📦 WASM cache initialized: dir={:?}, max_size={}MB",
            cache_dir, max_size_mb
        );

        let mut cache = Self {
            dir: cache_dir,
            max_size_bytes,
            entries: HashMap::new(),
            total_size: 0,
        };

        // Load existing cache entries from disk
        cache.load_existing_entries()?;

        Ok(cache)
    }

    /// Load existing cache entries from disk on startup
    fn load_existing_entries(&mut self) -> Result<()> {
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(e) => {
                warn!("Failed to read cache directory: {}", e);
                return Ok(());
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();

            // Skip non-WASM files
            if path.extension().map(|e| e != "wasm").unwrap_or(true) {
                continue;
            }

            // Extract checksum from filename (format: {checksum}.wasm)
            let checksum = match path.file_stem().and_then(|s| s.to_str()) {
                Some(s) => s.to_string(),
                None => continue,
            };

            // Get file size
            let metadata = match fs::metadata(&path) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let size = metadata.len();

            // Verify file hash matches filename
            let actual_hash = match Self::compute_file_hash(&path) {
                Ok(h) => h,
                Err(e) => {
                    warn!("Failed to compute hash for {:?}: {}, removing", path, e);
                    let _ = fs::remove_file(&path);
                    continue;
                }
            };

            if actual_hash != checksum {
                warn!(
                    "Cache file hash mismatch: expected {}, got {}, removing",
                    checksum, actual_hash
                );
                let _ = fs::remove_file(&path);
                continue;
            }

            // Add to entries
            self.entries.insert(
                checksum.clone(),
                CacheEntry {
                    path,
                    size,
                    expected_checksum: checksum,
                    last_used: Instant::now(),
                },
            );
            self.total_size += size;
        }

        info!(
            "📦 Loaded {} cached WASM files ({}MB)",
            self.entries.len(),
            self.total_size / 1024 / 1024
        );

        // Evict if over limit
        self.evict_if_needed();

        Ok(())
    }

    /// Get WASM from cache if available and valid
    ///
    /// # Arguments
    /// * `checksum` - Expected SHA256 checksum
    ///
    /// # Returns
    /// * `Some(bytes)` - Cached WASM bytes (verified)
    /// * `None` - Cache miss or verification failed
    pub fn get(&mut self, checksum: &str) -> Option<Vec<u8>> {
        let entry = self.entries.get_mut(checksum)?;

        // Update last used time
        entry.last_used = Instant::now();

        // Read file
        let bytes = match fs::read(&entry.path) {
            Ok(b) => b,
            Err(e) => {
                warn!("Failed to read cached file {:?}: {}", entry.path, e);
                // Remove invalid entry
                let entry = self.entries.remove(checksum)?;
                self.total_size = self.total_size.saturating_sub(entry.size);
                let _ = fs::remove_file(&entry.path);
                return None;
            }
        };

        // Verify hash before returning (security check)
        let actual_hash = Self::compute_hash(&bytes);
        if actual_hash != checksum {
            warn!(
                "⚠️ Cache integrity check failed! Expected {}, got {}. File may be corrupted or tampered.",
                checksum, actual_hash
            );
            // Remove corrupted entry
            let entry = self.entries.remove(checksum)?;
            self.total_size = self.total_size.saturating_sub(entry.size);
            let _ = fs::remove_file(&entry.path);
            return None;
        }

        debug!("✅ WASM cache hit: {} ({}KB)", checksum, bytes.len() / 1024);
        Some(bytes)
    }

    /// Store WASM in cache
    ///
    /// # Arguments
    /// * `checksum` - Expected SHA256 checksum (will be verified)
    /// * `bytes` - WASM binary data
    ///
    /// # Returns
    /// * `Ok(())` - Successfully cached
    /// * `Err(_)` - Hash mismatch or write failed
    pub fn put(&mut self, checksum: &str, bytes: &[u8]) -> Result<()> {
        // Verify hash before storing
        let actual_hash = Self::compute_hash(bytes);
        if actual_hash != checksum {
            anyhow::bail!(
                "WASM hash mismatch: expected {}, got {}",
                checksum,
                actual_hash
            );
        }

        let size = bytes.len() as u64;

        // Skip if single file is larger than max cache size
        if size > self.max_size_bytes {
            warn!(
                "WASM file too large for cache: {}MB > {}MB max",
                size / 1024 / 1024,
                self.max_size_bytes / 1024 / 1024
            );
            return Ok(());
        }

        // Evict old entries until we have space
        while self.total_size + size > self.max_size_bytes && !self.entries.is_empty() {
            self.evict_oldest();
        }

        // Write to disk
        let path = self.dir.join(format!("{}.wasm", checksum));
        fs::write(&path, bytes)
            .with_context(|| format!("Failed to write cache file: {:?}", path))?;

        // Update entry
        if let Some(old_entry) = self.entries.remove(checksum) {
            self.total_size = self.total_size.saturating_sub(old_entry.size);
        }

        self.entries.insert(
            checksum.to_string(),
            CacheEntry {
                path,
                size,
                expected_checksum: checksum.to_string(),
                last_used: Instant::now(),
            },
        );
        self.total_size += size;

        debug!(
            "📦 Cached WASM: {} ({}KB, total: {}MB/{}MB)",
            checksum,
            size / 1024,
            self.total_size / 1024 / 1024,
            self.max_size_bytes / 1024 / 1024
        );

        Ok(())
    }

    /// Evict oldest entry (LRU)
    fn evict_oldest(&mut self) {
        let oldest = self
            .entries
            .iter()
            .min_by_key(|(_, e)| e.last_used)
            .map(|(k, _)| k.clone());

        if let Some(checksum) = oldest {
            if let Some(entry) = self.entries.remove(&checksum) {
                debug!(
                    "🗑️ Evicting cached WASM: {} ({}KB)",
                    checksum,
                    entry.size / 1024
                );
                self.total_size = self.total_size.saturating_sub(entry.size);
                let _ = fs::remove_file(&entry.path);
            }
        }
    }

    /// Evict entries if over size limit
    fn evict_if_needed(&mut self) {
        while self.total_size > self.max_size_bytes && !self.entries.is_empty() {
            self.evict_oldest();
        }
    }

    /// Compute SHA256 hash of bytes
    fn compute_hash(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("{:x}", hasher.finalize())
    }

    /// Compute SHA256 hash of file
    fn compute_file_hash(path: &PathBuf) -> Result<String> {
        let bytes = fs::read(path)?;
        Ok(Self::compute_hash(&bytes))
    }

    /// Get cache statistics
    #[allow(dead_code)]
    pub fn stats(&self) -> (usize, u64, u64) {
        (self.entries.len(), self.total_size, self.max_size_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_wasm() -> Vec<u8> {
        // Simple valid WASM module (magic number + version)
        vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]
    }

    #[test]
    fn test_cache_put_get() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = WasmCache::new(temp_dir.path().to_path_buf(), 10).unwrap();

        let wasm = create_test_wasm();
        let checksum = WasmCache::compute_hash(&wasm);

        // Put
        cache.put(&checksum, &wasm).unwrap();

        // Get - should hit
        let cached = cache.get(&checksum);
        assert!(cached.is_some());
        assert_eq!(cached.unwrap(), wasm);
    }

    #[test]
    fn test_cache_miss() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = WasmCache::new(temp_dir.path().to_path_buf(), 10).unwrap();

        // Get non-existent
        let cached = cache.get("nonexistent");
        assert!(cached.is_none());
    }

    #[test]
    fn test_hash_verification() {
        let temp_dir = TempDir::new().unwrap();
        let mut cache = WasmCache::new(temp_dir.path().to_path_buf(), 10).unwrap();

        let wasm = create_test_wasm();
        let correct_checksum = WasmCache::compute_hash(&wasm);

        // Try to put with wrong checksum
        let result = cache.put("wrong_checksum", &wasm);
        assert!(result.is_err());

        // Put with correct checksum
        cache.put(&correct_checksum, &wasm).unwrap();

        // Tamper with file
        let path = temp_dir.path().join(format!("{}.wasm", correct_checksum));
        fs::write(&path, b"tampered").unwrap();

        // Get should fail verification
        let cached = cache.get(&correct_checksum);
        assert!(cached.is_none());
    }

    #[test]
    fn test_lru_eviction() {
        let temp_dir = TempDir::new().unwrap();
        // Very small cache: 1KB
        let mut cache = WasmCache::new(temp_dir.path().to_path_buf(), 0).unwrap();
        cache.max_size_bytes = 1024; // Override for test

        // Create two WASMs
        let wasm1 = vec![0u8; 500];
        let wasm2 = vec![1u8; 500];
        let wasm3 = vec![2u8; 500];

        let checksum1 = WasmCache::compute_hash(&wasm1);
        let checksum2 = WasmCache::compute_hash(&wasm2);
        let checksum3 = WasmCache::compute_hash(&wasm3);

        // Put first two (fits in 1KB)
        cache.put(&checksum1, &wasm1).unwrap();
        cache.put(&checksum2, &wasm2).unwrap();

        // Access first one to make it more recent
        cache.get(&checksum1);

        // Put third - should evict second (least recently used)
        cache.put(&checksum3, &wasm3).unwrap();

        // First should still be there
        assert!(cache.get(&checksum1).is_some());
        // Second should be evicted
        assert!(cache.get(&checksum2).is_none());
        // Third should be there
        assert!(cache.get(&checksum3).is_some());
    }
}
