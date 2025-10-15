use anyhow::{Context, Result};
use bollard::container::{Config as ContainerConfig, CreateContainerOptions, RemoveContainerOptions};
use bollard::exec::{CreateExecOptions, StartExecResults};
use bollard::image::CreateImageOptions;
use bollard::Docker;
use futures_util::stream::StreamExt;
use sha2::{Digest, Sha256};
use tracing::{debug, error, info, warn};

use crate::api_client::{ApiClient, CodeSource};
use crate::config::Config;

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
                COMPILATION_LOCK_TTL_SECONDS, // Lock expires after 5 minutes
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

        // Compile WASM from GitHub repository
        let wasm_bytes = self.compile_from_github(repo, commit, build_target).await?;

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
        // Note: Lock may already be expired (TTL=5min), which is OK
        if let Err(e) = self.api_client.release_lock(&lock_key).await {
            debug!("Could not release compilation lock (may have expired): {}", e);
        }

        // Check upload result after releasing lock
        upload_result.context("Failed to upload WASM")?;

        Ok(checksum)
    }

    /// Compile WASM from GitHub repository using Docker
    ///
    /// Currently supports: wasm32-wasi, wasm32-wasip1
    /// Future support planned for: wasm32-unknown-unknown, wasm32-wasip2
    async fn compile_from_github(&self, repo: &str, commit: &str, build_target: &str) -> Result<Vec<u8>> {
        info!("Compiling {} @ {} for target {}", repo, commit, build_target);

        // Validate and normalize build target
        let normalized_target = self.validate_build_target(build_target)?;

        info!("Using build target: {}", normalized_target);

        // Create unique container name
        let container_name = format!("offchainvm-compile-{}", uuid::Uuid::new_v4());

        // Ensure Docker image is available
        self.ensure_docker_image().await?;

        // Create container
        let container_id = self.create_compile_container(&container_name, repo, commit, &normalized_target).await?;

        // Execute compilation
        let result = self.execute_compilation(&container_id, &normalized_target).await;

        // Always cleanup container
        if let Err(e) = self.cleanup_container(&container_id).await {
            warn!("Failed to cleanup container {}: {}", container_id, e);
        }

        // Return result
        result
    }

    /// Ensure Docker image is available
    async fn ensure_docker_image(&self) -> Result<()> {
        let image = &self.config.docker_image;
        info!("Ensuring Docker image is available: {}", image);

        // Try to create/pull the image
        let mut stream = self.docker.create_image(
            Some(CreateImageOptions {
                from_image: image.clone(),
                ..Default::default()
            }),
            None,
            None,
        );

        while let Some(result) = stream.next().await {
            match result {
                Ok(info) => {
                    if let Some(status) = info.status {
                        debug!("Docker: {}", status);
                    }
                }
                Err(e) => {
                    error!("Failed to pull Docker image: {}", e);
                    return Err(e.into());
                }
            }
        }

        info!("Docker image ready: {}", image);
        Ok(())
    }

    /// Create a Docker container for compilation
    async fn create_compile_container(&self, name: &str, repo: &str, commit: &str, build_target: &str) -> Result<String> {
        info!("Creating Docker container: {}", name);

        let config = ContainerConfig {
            image: Some(self.config.docker_image.clone()),
            cmd: Some(vec!["sleep".to_string(), "600".to_string()]), // Keep alive for 10 minutes
            working_dir: Some("/workspace".to_string()),
            host_config: Some(bollard::models::HostConfig {
                // Note: Network is needed for installing rustup and cloning repo
                // We'll disable it after initial setup in the future
                network_mode: Some("bridge".to_string()),
                memory: Some((self.config.compile_memory_limit_mb as i64) * 1024 * 1024),
                nano_cpus: Some((self.config.compile_cpu_limit * 1_000_000_000.0) as i64),
                ..Default::default()
            }),
            env: Some(vec![
                format!("REPO={}", repo),
                format!("COMMIT={}", commit),
                format!("BUILD_TARGET={}", build_target),
            ]),
            ..Default::default()
        };

        let response = self.docker
            .create_container(Some(CreateContainerOptions { name: name.to_string(), ..Default::default() }), config)
            .await
            .context("Failed to create container")?;

        // Start container
        self.docker
            .start_container::<String>(&response.id, None)
            .await
            .context("Failed to start container")?;

        info!("Container created and started: {}", response.id);
        Ok(response.id)
    }

    /// Execute compilation inside container
    async fn execute_compilation(&self, container_id: &str, build_target: &str) -> Result<Vec<u8>> {
        info!("Executing compilation in container {}", container_id);
        let start_time = std::time::Instant::now();

        // Single command with proper shell environment handling
        let compile_script = format!(r#"
set -ex
cd /workspace

# Track compilation time
START_TIME=$(date +%s)

# Setup Rust environment (rust:1.75 image already has Rust)
if [ -f /usr/local/cargo/env ]; then
    . /usr/local/cargo/env
elif [ -f $HOME/.cargo/env ]; then
    . $HOME/.cargo/env
fi

# Add WASM target
# Note: wasm32-wasi will be deprecated in Rust 1.84 (Jan 2025)
# Prefer wasm32-wasip1 for forward compatibility
TARGET_TO_ADD={build_target}
if [ "{build_target}" = "wasm32-wasi" ]; then
    # Try wasip1 first (Rust 1.78+), fallback to wasi
    if rustup target list | grep -q wasm32-wasip1; then
        TARGET_TO_ADD="wasm32-wasip1"
        echo "ℹ️  Using wasm32-wasip1 (recommended for Rust 1.78+)"
    fi
elif [ "{build_target}" = "wasm32-wasip2" ]; then
    # WASI Preview 2 requires explicit target installation
    echo "ℹ️  Using wasm32-wasip2 (WASI Preview 2)"
fi
rustup target add $TARGET_TO_ADD
echo "🔧TARGET_TO_ADD: $TARGET_TO_ADD"

# Clone repository
git clone $REPO repo
cd repo
git checkout $COMMIT

# Build WASM with size optimizations
# Note: We rely on repository's Cargo.toml profile settings
# For wasm32-wasip2: use standard cargo build (works with wasi-http-client)
# For wasm32-wasi/wasip1: also use standard cargo build
cargo build --release --target $TARGET_TO_ADD
WASM_FILE=$(find target/$TARGET_TO_ADD/release -maxdepth 1 -name "*.wasm" -type f | head -1)

# Find compiled WASM
if [ -z "$WASM_FILE" ]; then
    echo "❌ ERROR: No WASM file found!"
    find target/$TARGET_TO_ADD/release -type f
    exit 1
fi

echo "📦 Original WASM: $WASM_FILE"
ls -lah "$WASM_FILE"

# Copy to output
mkdir -p /workspace/output
cp "$WASM_FILE" /workspace/output/output.wasm

# Optimize WASM based on target type
ORIGINAL_SIZE=$(stat -c%s /workspace/output/output.wasm)

if [ "$TARGET_TO_ADD" = "wasm32-wasip2" ]; then
    # WASI Preview 2: already a proper CLI component from cargo-component
    # Just strip debug info and optimize
    if command -v wasm-tools &> /dev/null; then
        echo "🔧 Optimizing WASI P2 CLI component..."

        # Strip debug information from component
        wasm-tools strip /workspace/output/output.wasm -o /workspace/output/output_optimized.wasm
        mv /workspace/output/output_optimized.wasm /workspace/output/output.wasm

        OPTIMIZED_SIZE=$(stat -c%s /workspace/output/output.wasm)
        SAVED=$((ORIGINAL_SIZE - OPTIMIZED_SIZE))
        PERCENT=$((SAVED * 100 / ORIGINAL_SIZE))
        echo "✅ Component optimization complete: $ORIGINAL_SIZE bytes → $OPTIMIZED_SIZE bytes (saved $SAVED bytes / $PERCENT%)"
    else
        echo "ℹ️  wasm-tools not available - skipping WASI Preview 2 component conversion"
        echo "   Module size: $ORIGINAL_SIZE bytes"
    fi
else
    # WASI Preview 1 - use wasm-opt for classic modules
    echo "🔧 Optimizing WASM module with wasm-opt..."

    # Install wasm-opt if not present (from binaryen package)
    if ! command -v wasm-opt &> /dev/null; then
        echo "📥 Installing wasm-opt..."
        apt-get update -qq && apt-get install -y -qq binaryen > /dev/null 2>&1 || true
    fi

    if command -v wasm-opt &> /dev/null; then
        # Apply optimizations as per cargo-wasi:
        # -Oz: optimize aggressively for size
        # --strip-dwarf: remove DWARF debug info
        # --strip-producers: remove producers section
        # --enable-sign-ext: enable sign extension operations (i32.extend8_s, i32.extend16_s)
        # --enable-bulk-memory: enable bulk memory operations (memory.copy, memory.fill)
        wasm-opt -Oz \
            --strip-dwarf \
            --strip-producers \
            --enable-sign-ext \
            --enable-bulk-memory \
            /workspace/output/output.wasm \
            -o /workspace/output/output_optimized.wasm

        mv /workspace/output/output_optimized.wasm /workspace/output/output.wasm
        OPTIMIZED_SIZE=$(stat -c%s /workspace/output/output.wasm)
        SAVED=$((ORIGINAL_SIZE - OPTIMIZED_SIZE))
        PERCENT=$((SAVED * 100 / ORIGINAL_SIZE))
        echo "✅ Module optimization complete: $ORIGINAL_SIZE bytes → $OPTIMIZED_SIZE bytes (saved $SAVED bytes / $PERCENT%)"
    else
        echo "⚠️  wasm-opt not available, skipping module optimization"
    fi
fi

ls -lah /workspace/output/output.wasm

# Calculate total compilation time
END_TIME=$(date +%s)
COMPILE_TIME=$((END_TIME - START_TIME))
echo "⏱️  Total compilation time: $COMPILE_TIME seconds"
"#);

        self.exec_in_container(container_id, &compile_script).await?;

        // Extract WASM file from container
        let wasm_bytes = self.extract_wasm_from_container(container_id, "/workspace/output/output.wasm").await?;

        let elapsed = start_time.elapsed();
        info!(
            "✅ Compilation successful: WASM size={} bytes, total_time={:.2}s",
            wasm_bytes.len(),
            elapsed.as_secs_f64()
        );
        Ok(wasm_bytes)
    }

    /// Execute command in container
    async fn exec_in_container(&self, container_id: &str, cmd: &str) -> Result<()> {
        debug!("Executing in container: {}", cmd);

        let exec = self.docker
            .create_exec(
                container_id,
                CreateExecOptions {
                    cmd: Some(vec!["sh", "-c", cmd]),
                    attach_stdout: Some(true),
                    attach_stderr: Some(true),
                    ..Default::default()
                },
            )
            .await
            .context("Failed to create exec")?;

        let mut output = match self.docker.start_exec(&exec.id, None).await? {
            StartExecResults::Attached { output, .. } => output,
            StartExecResults::Detached => {
                anyhow::bail!("Unexpected detached exec");
            }
        };

        let mut stdout_lines = Vec::new();
        let mut stderr_lines = Vec::new();

        while let Some(msg) = output.next().await {
            match msg {
                Ok(log) => {
                    use bollard::container::LogOutput;
                    match log {
                        LogOutput::StdOut { message } => {
                            let text = String::from_utf8_lossy(&message);
                            for line in text.lines() {
                                info!("📦 STDOUT: {}", line);
                                stdout_lines.push(line.to_string());
                            }
                        }
                        LogOutput::StdErr { message } => {
                            let text = String::from_utf8_lossy(&message);
                            for line in text.lines() {
                                warn!("📦 STDERR: {}", line);
                                stderr_lines.push(line.to_string());
                            }
                        }
                        _ => {}
                    }
                }
                Err(e) => {
                    error!("Error reading container output: {}", e);
                    return Err(e.into());
                }
            }
        }

        // Check exit code
        let inspect = self.docker.inspect_exec(&exec.id).await?;
        if let Some(exit_code) = inspect.exit_code {
            if exit_code != 0 {
                error!("❌ Container command failed with exit code: {}", exit_code);
                error!("Last stdout lines: {:?}", stdout_lines.iter().rev().take(5).collect::<Vec<_>>());
                error!("Last stderr lines: {:?}", stderr_lines.iter().rev().take(5).collect::<Vec<_>>());

                // Extract user-friendly error message from stderr
                let error_msg = Self::extract_compilation_error(&stderr_lines, &stdout_lines);
                anyhow::bail!("{}", error_msg);
            }
        }

        Ok(())
    }

    /// Extract WASM file from container using tar stream
    async fn extract_wasm_from_container(&self, container_id: &str, wasm_path: &str) -> Result<Vec<u8>> {
        info!("Extracting WASM from container: {}", wasm_path);

        use bollard::container::DownloadFromContainerOptions;
        use futures_util::TryStreamExt;
        use tar::Archive;
        use std::io::Read;

        // Download file as tar stream from container
        let mut tar_stream = self.docker.download_from_container(
            container_id,
            Some(DownloadFromContainerOptions {
                path: wasm_path.to_string(),
            }),
        );

        // Collect all chunks into a buffer
        let mut tar_data = Vec::new();
        while let Some(chunk) = tar_stream.try_next().await? {
            tar_data.extend_from_slice(&chunk);
        }

        // Parse tar archive
        let mut archive = Archive::new(&tar_data[..]);

        // Extract the WASM file
        for entry in archive.entries().context("Failed to read tar entries")? {
            let mut entry = entry.context("Failed to read tar entry")?;

            // Read the file content
            let mut wasm_bytes = Vec::new();
            entry.read_to_end(&mut wasm_bytes)
                .context("Failed to read WASM from tar")?;

            if !wasm_bytes.is_empty() {
                info!("Successfully extracted WASM ({} bytes)", wasm_bytes.len());
                return Ok(wasm_bytes);
            }
        }

        anyhow::bail!("WASM file not found in tar archive");
    }

    /// Cleanup container
    async fn cleanup_container(&self, container_id: &str) -> Result<()> {
        info!("Cleaning up container: {}", container_id);

        self.docker
            .remove_container(
                container_id,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await
            .context("Failed to remove container")?;

        Ok(())
    }

    /// Extract user-friendly error message from compilation output
    fn extract_compilation_error(stderr_lines: &[String], stdout_lines: &[String]) -> String {
        // Check for common git errors
        for line in stderr_lines.iter().chain(stdout_lines.iter()) {
            // Git clone errors
            if line.contains("fatal: repository") && line.contains("not found") {
                return "Repository not found or not accessible on GitHub".to_string();
            }
            if line.contains("fatal: could not read Username") {
                return "Repository requires authentication (private repo not supported)".to_string();
            }

            // Git checkout errors
            if line.contains("pathspec") && line.contains("did not match any file(s) known to git") {
                // Extract the commit hash from the error
                if let Some(hash_start) = line.find('\'') {
                    if let Some(hash_end) = line[hash_start + 1..].find('\'') {
                        let commit = &line[hash_start + 1..hash_start + 1 + hash_end];
                        return format!("Commit '{}' not found in repository", commit);
                    }
                }
                return "Commit hash not found in repository".to_string();
            }
            if line.contains("error: pathspec") && line.contains("did not match") {
                return "Invalid commit hash or branch name".to_string();
            }

            // Cargo build errors
            if line.contains("error: no matching package named") {
                return "Cargo.toml not found or invalid package configuration".to_string();
            }
            if line.contains("error: could not compile") {
                return "Rust compilation failed - check code for errors".to_string();
            }
            if line.contains("error[E") {
                // Rust compiler error, return first error line
                return format!("Rust compilation error: {}", line.trim());
            }
        }

        // No specific error found, check last stderr lines for any error indication
        for line in stderr_lines.iter().rev().take(3) {
            if line.contains("error:") || line.contains("fatal:") || line.contains("ERROR") {
                return line.trim().to_string();
            }
        }

        // Generic fallback
        "Compilation failed - see worker logs for details".to_string()
    }

    /// Validate and normalize build target
    ///
    /// Currently supports:
    /// - wasm32-wasi (primary, deprecated in Rust 1.84+)
    /// - wasm32-wasip1 (alias for wasi, recommended for Rust 1.78+)
    /// - wasm32-wasip2 (WASI Preview 2, requires WasmEdge or wasmtime)
    ///
    /// Future support planned:
    /// - wasm32-unknown-unknown (core WASM without WASI)
    fn validate_build_target(&self, target: &str) -> Result<String> {
        match target {
            // Currently supported targets
            "wasm32-wasi" => Ok("wasm32-wasi".to_string()),
            "wasm32-wasip1" => Ok("wasm32-wasi".to_string()), // Normalize to wasm32-wasi for backward compatibility
            "wasm32-wasip2" => Ok("wasm32-wasip2".to_string()), // WASI Preview 2 (requires WasmEdge/wasmtime)

            // Future targets (commented out for now)
            // "wasm32-unknown-unknown" => Ok("wasm32-unknown-unknown".to_string()),

            _ => {
                anyhow::bail!(
                    "Unsupported build target: '{}'. Currently supported: wasm32-wasi, wasm32-wasip1, wasm32-wasip2. \
                    Future support planned for: wasm32-unknown-unknown",
                    target
                )
            }
        }
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

    #[test]
    fn test_validate_build_target() {
        let config = create_test_config();
        let api_client = ApiClient::new(
            "http://localhost:8080".to_string(),
            "test-token".to_string(),
        )
        .unwrap();
        let compiler = Compiler::new(api_client, config).unwrap();

        // Valid targets
        assert_eq!(
            compiler.validate_build_target("wasm32-wasi").unwrap(),
            "wasm32-wasi"
        );
        assert_eq!(
            compiler.validate_build_target("wasm32-wasip1").unwrap(),
            "wasm32-wasi"
        ); // normalized

        // Invalid target
        assert!(compiler.validate_build_target("invalid-target").is_err());
    }

    #[tokio::test]
    #[ignore] // Run with: cargo test test_real_github_compilation -- --ignored --nocapture
    async fn test_real_github_compilation() {
        use sha2::{Digest, Sha256};

        // Initialize tracing for test output
        let _ = tracing_subscriber::fmt()
            .with_env_filter("info")
            .with_test_writer()
            .try_init();

        // This test requires Docker to be running
        let config = create_test_config();
        let api_client = ApiClient::new(
            "http://localhost:8080".to_string(),
            "test-token".to_string(),
        )
        .unwrap();
        let compiler = Compiler::new(api_client, config).unwrap();

        // Test with real repository
        let repo = "https://github.com/zavodil/random-ark";
        let commit = "6491b317afa33534b56cebe9957844e16ac720e8";
        let build_target = "wasm32-wasi";

        println!("Compiling {} @ {} for {}", repo, commit, build_target);

        let wasm_bytes = compiler
            .compile_from_github(repo, commit, build_target)
            .await
            .expect("Compilation failed");

        println!("Compiled WASM size: {} bytes", wasm_bytes.len());

        // Calculate checksum
        let mut hasher = Sha256::new();
        hasher.update(&wasm_bytes);
        let checksum = hex::encode(hasher.finalize());

        println!("Compiled WASM checksum: {}", checksum);

        // Compare with pre-compiled version if it exists
        let expected_wasm_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join("wasi-examples/random-ark/target/wasm32-wasip1/release/random-ark.wasm");

        if expected_wasm_path.exists() {
            let expected_bytes = std::fs::read(&expected_wasm_path)
                .expect("Failed to read expected WASM");

            let mut expected_hasher = Sha256::new();
            expected_hasher.update(&expected_bytes);
            let expected_checksum = hex::encode(expected_hasher.finalize());

            println!("Expected WASM checksum: {}", expected_checksum);
            println!("Expected WASM size: {} bytes", expected_bytes.len());

            // Note: Checksums might differ due to compilation environment differences
            // but we can compare sizes and validate structure
            assert!(wasm_bytes.len() > 0, "Compiled WASM should not be empty");
            assert!(
                wasm_bytes.starts_with(&[0x00, 0x61, 0x73, 0x6d]),
                "WASM should start with magic number"
            );

            println!(
                "✅ Compilation successful! Size difference: {} bytes",
                (wasm_bytes.len() as i64 - expected_bytes.len() as i64).abs()
            );
        } else {
            println!(
                "⚠️  Expected WASM not found at {:?}, skipping comparison",
                expected_wasm_path
            );
        }
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
            scan_interval_ms: 1000,
            docker_image: "rust:1.75".to_string(),
            compile_timeout_seconds: 300,
            compile_memory_limit_mb: 2048,
            compile_cpu_limit: 2.0,
            default_max_instructions: 10_000_000_000,
            default_max_memory_mb: 128,
            default_max_execution_seconds: 60,
            keystore_base_url: None,
            keystore_auth_token: None,
            tee_mode: "none".to_string()
        }
    }
}
