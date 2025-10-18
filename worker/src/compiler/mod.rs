//! WASM compiler with support for multiple build targets
//!
//! This module provides compilation from GitHub repositories to WASM for different targets:
//! - WASI Preview 2 (P2): Modern component model with HTTP support (wasm32-wasip2)
//! - WASI Preview 1 (P1): Standard WASI modules (wasm32-wasip1, wasm32-wasi)
//!
//! ## Adding New Build Targets
//!
//! To add support for a new build target (e.g., wasm32-unknown-unknown):
//!
//! 1. Create a new module file: `src/compiler/wasm32_unknown.rs`
//! 2. Implement the compilation function with signature:
//!    ```rust,ignore
//!    pub async fn compile(
//!        docker: &Docker,
//!        container_id: &str,
//!        repo: &str,
//!        commit: &str,
//!        build_target: &str,
//!    ) -> Result<Vec<u8>>
//!    ```
//! 3. Add module declaration: `mod wasm32_unknown;`
//! 4. Add match arm in `compile_in_container()` to call your compiler
//! 5. Add unit tests
//!
//! ## Architecture
//!
//! The compiler selects the appropriate compilation strategy based on build_target:
//! 1. wasm32-wasip2 → P2 compiler (cargo build + wasm-tools)
//! 2. wasm32-wasip1/wasm32-wasi → P1 compiler (cargo build + wasm-opt)
//! 3. Unknown target → return error with helpful message

use anyhow::{Context, Result};
use bollard::Docker;
use sha2::{Digest, Sha256};
use tracing::{info, warn};

use crate::api_client::{ApiClient, CodeSource};
use crate::config::Config;

mod docker;
mod wasm32_wasip1;
mod wasm32_wasip2;

/// Lock TTL for compilation in seconds (5 minutes)
/// This prevents stale locks from blocking compilation forever
const COMPILATION_LOCK_TTL_SECONDS: u64 = 300;

/// Compiler for building GitHub repositories into WASM binaries
pub struct Compiler {
    api_client: ApiClient,
    config: Config,
    docker: Docker,
}

impl Compiler {
    /// Create a new compiler instance
    pub fn new(api_client: ApiClient, config: Config) -> Result<Self> {
        let docker = Docker::connect_with_socket_defaults()
            .context("Failed to connect to Docker")?;

        Ok(Self {
            api_client,
            config,
            docker,
        })
    }

    /// Compile a GitHub repository to WASM
    ///
    /// This function:
    /// 1. Checks if WASM already exists in cache
    /// 2. If not, acquires a distributed lock
    /// 3. Compiles the WASM using target-specific compiler
    /// 4. Uploads the result to the coordinator
    /// 5. Releases the lock
    ///
    /// # Arguments
    /// * `code_source` - GitHub repository information
    ///
    /// # Returns
    /// * `Ok(checksum)` - SHA256 checksum of compiled WASM
    /// * `Err(_)` - Compilation failed
    /// Compile WASM and return checksum + bytes (does NOT upload to coordinator)
    /// Use this for job-based workflow where upload happens after execution
    ///
    /// # Arguments
    /// * `code_source` - GitHub repo, commit, and build target
    /// * `timeout_seconds` - Optional timeout for compilation (kills process if exceeded)
    pub async fn compile_local(
        &self,
        code_source: &CodeSource,
        timeout_seconds: Option<u64>,
    ) -> Result<(String, Vec<u8>)> {
        let CodeSource::GitHub {
            repo,
            commit,
            build_target,
        } = code_source;

        // Generate checksum for this specific compilation
        let checksum = self.compute_checksum(repo, commit, build_target);

        // Check if WASM already exists
        if self.api_client.wasm_exists(&checksum).await? {
            info!("WASM already exists in cache: {}", checksum);
            // Download and return it
            let wasm_bytes = self.api_client.download_wasm(&checksum).await?;
            return Ok((checksum, wasm_bytes));
        }

        // Try to acquire distributed lock to prevent duplicate compilations
        let lock_key = format!("compile:{}:{}", repo, commit);
        let acquired = self
            .api_client
            .acquire_lock(
                lock_key.clone(),
                self.config.worker_id.clone(),
                COMPILATION_LOCK_TTL_SECONDS,
            )
            .await?;

        if !acquired {
            // Another worker is compiling, wait and check again
            info!("Another worker is compiling {}, waiting...", repo);
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

            // Check if compilation completed
            if self.api_client.wasm_exists(&checksum).await? {
                info!("WASM compilation completed by another worker");
                let wasm_bytes = self.api_client.download_wasm(&checksum).await?;
                return Ok((checksum, wasm_bytes));
            }

            anyhow::bail!("Failed to acquire compilation lock and WASM not available");
        }

        info!("Acquired compilation lock for {}", repo);

        // Compile WASM from GitHub repository with timeout
        let wasm_bytes = if let Some(timeout) = timeout_seconds {
            info!("⏱️  Compiling with timeout: {}s", timeout);
            tokio::time::timeout(
                std::time::Duration::from_secs(timeout),
                self.compile_from_github(repo, commit, build_target)
            )
            .await
            .map_err(|_| anyhow::anyhow!("Compilation timeout exceeded: {}s", timeout))??
        } else {
            self.compile_from_github(repo, commit, build_target).await?
        };

        // Release lock
        if let Err(e) = self.api_client.release_lock(&lock_key).await {
            warn!("Failed to release lock {}: {}", lock_key, e);
        }

        info!("✅ WASM compilation complete: {} ({} bytes)", checksum, wasm_bytes.len());
        Ok((checksum, wasm_bytes))
    }

    /// OLD METHOD - kept for backward compatibility
    /// Compile WASM and upload to coordinator immediately
    pub async fn compile(&self, code_source: &CodeSource) -> Result<String> {
        let (checksum, wasm_bytes) = self.compile_local(code_source, None).await?;

        // Upload to coordinator
        info!("Uploading compiled WASM to coordinator");
        self.api_client
            .upload_wasm(checksum.clone(), code_source.repo().to_string(), code_source.commit().to_string(), wasm_bytes)
            .await?;

        info!("✅ WASM compilation and upload complete: {}", checksum);
        Ok(checksum)
    }

    /// Compile WASM from GitHub repository using Docker
    async fn compile_from_github(&self, repo: &str, commit: &str, build_target: &str) -> Result<Vec<u8>> {
        info!("Compiling {} @ {} for target {}", repo, commit, build_target);

        // Validate and normalize build target
        let normalized_target = self.validate_build_target(build_target)?;

        info!("Using build target: {}", normalized_target);

        // Create unique container name
        let container_name = format!("offchainvm-compile-{}", uuid::Uuid::new_v4());

        // Ensure Docker image is available
        docker::ensure_image(&self.docker, &self.config.docker_image).await?;

        // Create container
        let container_id = docker::create_container(
            &self.docker,
            &container_name,
            &self.config,
            repo,
            commit,
            &normalized_target,
        )
        .await?;

        // Execute compilation using target-specific compiler
        let result = self.compile_in_container(&container_id, &normalized_target).await;

        // Always cleanup container
        if let Err(e) = docker::cleanup_container(&self.docker, &container_id).await {
            warn!("Failed to cleanup container {}: {}", container_id, e);
        }

        // Return result
        result
    }

    /// Compile WASM in container using target-specific compiler
    async fn compile_in_container(&self, container_id: &str, build_target: &str) -> Result<Vec<u8>> {
        info!("Executing compilation in container {} for target {}", container_id, build_target);

        // Select compiler based on build target
        match build_target {
            "wasm32-wasip2" => {
                wasm32_wasip2::compile(&self.docker, container_id, build_target).await
            }
            "wasm32-wasip1" | "wasm32-wasi" => {
                wasm32_wasip1::compile(&self.docker, container_id, build_target).await
            }
            _ => {
                anyhow::bail!(
                    "Unsupported build target: {}\n\
                     Supported targets:\n\
                     - wasm32-wasip2 (WASI Preview 2)\n\
                     - wasm32-wasip1, wasm32-wasi (WASI Preview 1)\n\
                     \n\
                     To add support for a new target, see module documentation.",
                    build_target
                )
            }
        }
    }

    /// Validate and normalize build target
    fn validate_build_target(&self, target: &str) -> Result<String> {
        match target {
            "wasm32-wasip1" | "wasm32-wasi" | "wasm32-wasip2" => Ok(target.to_string()),
            _ => anyhow::bail!(
                "Unsupported build target: {}. Supported: wasm32-wasip1, wasm32-wasi, wasm32-wasip2",
                target
            ),
        }
    }

    /// Compute checksum for a specific compilation
    fn compute_checksum(&self, repo: &str, commit: &str, build_target: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(repo.as_bytes());
        hasher.update(b":");
        hasher.update(commit.as_bytes());
        hasher.update(b":");
        hasher.update(build_target.as_bytes());
        let result = hasher.finalize();
        hex::encode(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checksum_computation() {
        // Test checksum computation without needing full Compiler struct
        let mut hasher = Sha256::new();
        hasher.update(b"https://github.com/user/repo:");
        hasher.update(b"abc123:");
        hasher.update(b"wasm32-wasip1");
        let checksum1 = hex::encode(hasher.finalize());

        let mut hasher = Sha256::new();
        hasher.update(b"https://github.com/user/repo:");
        hasher.update(b"abc123:");
        hasher.update(b"wasm32-wasip2");
        let checksum2 = hex::encode(hasher.finalize());

        // Different targets should produce different checksums
        assert_ne!(checksum1, checksum2);

        // Same inputs should produce same checksum
        let mut hasher = Sha256::new();
        hasher.update(b"https://github.com/user/repo:");
        hasher.update(b"abc123:");
        hasher.update(b"wasm32-wasip1");
        let checksum3 = hex::encode(hasher.finalize());
        assert_eq!(checksum1, checksum3);
    }
}
