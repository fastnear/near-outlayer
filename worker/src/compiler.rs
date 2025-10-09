use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use tracing::{info, warn};

use crate::api_client::{ApiClient, CodeSource};
use crate::config::Config;

/// Compiler for building GitHub repositories into WASM binaries
pub struct Compiler {
    api_client: ApiClient,
    config: Config,
}

impl Compiler {
    /// Create a new compiler instance
    pub fn new(api_client: ApiClient, config: Config) -> Result<Self> {
        Ok(Self {
            api_client,
            config,
        })
    }

    /// Compile a GitHub repository to WASM
    ///
    /// This function:
    /// 1. Checks if WASM already exists in cache
    /// 2. If not, acquires a distributed lock
    /// 3. Compiles the WASM (placeholder for MVP)
    /// 4. Uploads the result to the coordinator
    /// 5. Releases the lock
    ///
    /// # Arguments
    /// * `code_source` - GitHub repository information
    ///
    /// # Returns
    /// * `Ok(checksum)` - SHA256 checksum of compiled WASM
    /// * `Err(_)` - Compilation failed
    pub async fn compile(&self, code_source: &CodeSource) -> Result<String> {
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
            return Ok(checksum);
        }

        // Try to acquire distributed lock to prevent duplicate compilations
        let lock_key = format!("compile:{}:{}", repo, commit);
        let acquired = self
            .api_client
            .acquire_lock(
                lock_key.clone(),
                self.config.worker_id.clone(),
                self.config.compile_timeout_seconds + 60, // Add buffer to TTL
            )
            .await?;

        if !acquired {
            // Another worker is compiling, wait and check again
            info!("Another worker is compiling {}, waiting...", repo);
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

            // Check if compilation completed
            if self.api_client.wasm_exists(&checksum).await? {
                info!("WASM compilation completed by another worker");
                return Ok(checksum);
            }

            anyhow::bail!("Failed to acquire compilation lock and WASM not available");
        }

        info!("Acquired compilation lock for {}", repo);

        // For MVP: Create a dummy WASM binary
        // TODO: Implement real Docker-based compilation
        let wasm_bytes = self.compile_dummy_wasm(repo, commit)?;

        // Upload to coordinator
        info!("Uploading compiled WASM to coordinator");
        let upload_result = self.api_client
            .upload_wasm(
                checksum.clone(),
                repo.to_string(),
                commit.to_string(),
                wasm_bytes,
            )
            .await;

        // Always release the lock, even on error
        if let Err(e) = self.api_client.release_lock(&lock_key).await {
            warn!("Failed to release compilation lock: {}", e);
        }

        // Check upload result after releasing lock
        upload_result.context("Failed to upload WASM")?;

        Ok(checksum)
    }

    /// Create a dummy WASM binary for MVP testing
    /// TODO: Replace with real Docker-based compilation
    fn compile_dummy_wasm(&self, repo: &str, _commit: &str) -> Result<Vec<u8>> {
        // Special case: if it's our test-wasm repo, load the real compiled WASM
        if repo == "https://github.com/near-offshore/test-wasm"
            || repo == "https://github.com/zavodil/random-ark" {
            // Try to load from local test-wasm build (multiple possible locations)
            let possible_paths = vec![
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .unwrap()
                    .join("test-wasm/target/wasm32-wasip1/release/test_wasm.wasm"),
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .unwrap()
                    .join("test-wasm/target/wasm32-wasi/release/test_wasm.wasm"),
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .unwrap()
                    .join("test-wasm/target/wasm32-unknown-unknown/release/test_wasm.wasm"),
            ];

            for test_wasm_path in possible_paths {
                if test_wasm_path.exists() {
                    info!("Loading pre-compiled test WASM from: {}", test_wasm_path.display());
                    return std::fs::read(&test_wasm_path)
                        .context("Failed to read test WASM file");
                }
            }

            warn!("Test WASM not found in any expected location, using minimal WASM");
        }

        // Fallback: minimal valid WASM module that does nothing
        // Real implementation would compile from GitHub
        let wasm = vec![
            0x00, 0x61, 0x73, 0x6d, // \0asm - magic number
            0x01, 0x00, 0x00, 0x00, // version 1
        ];
        Ok(wasm)
    }

    /// Compute deterministic checksum for a code source
    fn compute_checksum(&self, repo: &str, commit: &str, build_target: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(repo.as_bytes());
        hasher.update(commit.as_bytes());
        hasher.update(build_target.as_bytes());
        hex::encode(hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checksum_computation() {
        let config = create_test_config();
        let api_client = ApiClient::new(
            "http://localhost:8080".to_string(),
            "test-token".to_string(),
        )
        .unwrap();
        let compiler = Compiler::new(api_client, config).unwrap();

        let checksum1 = compiler.compute_checksum(
            "https://github.com/near/test",
            "abc123",
            "wasm32-wasi",
        );
        let checksum2 = compiler.compute_checksum(
            "https://github.com/near/test",
            "abc123",
            "wasm32-wasi",
        );
        let checksum3 = compiler.compute_checksum(
            "https://github.com/near/test",
            "def456",
            "wasm32-wasi",
        );

        assert_eq!(checksum1, checksum2);
        assert_ne!(checksum1, checksum3);
    }

    fn create_test_config() -> Config {
        use near_crypto::InMemorySigner;

        Config {
            api_base_url: "http://localhost:8080".to_string(),
            api_auth_token: "test-token".to_string(),
            near_rpc_url: "https://rpc.testnet.near.org".to_string(),
            neardata_api_url: "https://testnet.neardata.xyz/v0/block".to_string(),
            fastnear_api_url: "https://test.api.fastnear.com/status".to_string(),
            start_block_height: 0,
            offchainvm_contract_id: "offchainvm.testnet".parse().unwrap(),
            operator_account_id: "worker.testnet".parse().unwrap(),
            operator_signer: InMemorySigner::from_secret_key(
                "worker.testnet".parse().unwrap(),
                "ed25519:3D4YudUahN1nawWvHfEKBGpmJLfbCTbvdXDJKqfLhQ98XewyWK4tEDWvmAYPZqcgz7qfkCEHyWD15m8JVVWJ3LXD".parse().unwrap(),
            ),
            worker_id: "test-worker".to_string(),
            enable_event_monitor: false,
            poll_timeout_seconds: 60,
            scan_interval_seconds: 1,
            docker_image: "rust:1.75".to_string(),
            compile_timeout_seconds: 300,
            compile_memory_limit_mb: 2048,
            compile_cpu_limit: 2.0,
            default_max_instructions: 10_000_000_000,
            default_max_memory_mb: 128,
            default_max_execution_seconds: 60,
        }
    }
}
