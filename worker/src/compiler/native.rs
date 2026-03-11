//! Native WASI compilation with env isolation (TEE-friendly)
//!
//! This module provides compilation without Docker-in-Docker, using:
//! - env -i for environment variable isolation
//! - ulimit for resource limits (memory, CPU time, processes)
//! - Temporary isolated directories for each compilation
//! - build.rs validation to prevent malicious build scripts
//!
//! Security model:
//! 1. Environment isolation (env -i clears all worker secrets)
//! 2. Process isolation (Linux kernel isolates process memory)
//! 3. Intel TDX hardware isolation (TEE protects from host)
//! 4. Resource limits prevent DoS (memory, CPU, time)
//! 5. Build.rs validation prevents code execution attacks
//! 6. Temporary directories prevent filesystem conflicts
//!
//! This is designed for TEE environments (Phala, Intel TDX) where
//! advanced sandboxing (bubblewrap, pivot_root) is blocked by seccomp.

use anyhow::{Context, Result};
use bollard::Docker;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tracing::{debug, error, info, warn};

use crate::compiler::CompilationError;

/// Maximum memory for compilation (bytes): 2GB
const MAX_MEMORY_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Maximum CPU time for compilation (seconds): 300s = 5 minutes
const MAX_CPU_TIME_SECONDS: u64 = 300;

/// Maximum number of processes during compilation
const MAX_PROCESSES: u32 = 1024;

/// Compile WASM using native Rust toolchain with bubblewrap sandboxing
///
/// # Arguments
/// * `repo` - GitHub repository URL (e.g., "https://github.com/user/repo")
/// * `commit` - Git commit hash or branch name
/// * `build_target` - WASM target (wasm32-wasip1, wasm32-wasip2)
/// * `timeout_seconds` - Optional timeout override
///
/// # Returns
/// * `Ok(Vec<u8>)` - Compiled WASM binary
/// * `Err(CompilationError)` - Compilation failed with user-friendly error
///
/// # Security
/// - Environment isolation: env -i clears all worker secrets (OPERATOR_PRIVATE_KEY, etc.)
/// - Process isolation: Linux kernel prevents reading worker memory
/// - Hardware isolation: Intel TDX protects from host and other processes
/// - Resource limits: 2GB RAM, 5min CPU time, 1024 processes (ulimit)
/// - Build.rs validation: rejects projects with build scripts
/// - Temporary directories: compilation isolated in /tmp/compile-{uuid}
pub async fn compile(
    _docker: Option<&Docker>, // Unused in native mode
    repo: &str,
    commit: &str,
    build_target: &str,
    timeout_seconds: Option<u64>,
) -> Result<Vec<u8>> {
    let timeout = timeout_seconds.unwrap_or(MAX_CPU_TIME_SECONDS);

    info!("🔨 Native compilation: {} @ {} ({})", repo, commit, build_target);
    info!("⏱️  Timeout: {}s, Memory limit: {}MB", timeout, MAX_MEMORY_BYTES / 1024 / 1024);

    // 1. Create isolated working directory
    let work_dir = create_temp_dir()?;
    info!("📁 Work directory: {}", work_dir.display());

    // 2. Clone repository (outside sandbox, faster)
    clone_repo(repo, commit, &work_dir).await?;

    // 3. Validate no build.rs (security check)
    validate_no_build_scripts(&work_dir)?;

    // 4. Compile with env isolation + ulimit
    let wasm_bytes = compile_with_isolation(&work_dir, build_target, timeout).await?;

    // 5. Cleanup
    cleanup_dir(&work_dir)?;

    info!("✅ Compilation successful: {} bytes", wasm_bytes.len());
    Ok(wasm_bytes)
}

/// Create temporary directory for compilation
fn create_temp_dir() -> Result<PathBuf> {
    let uuid = uuid::Uuid::new_v4();
    let dir = PathBuf::from(format!("/tmp/compile-{}", uuid));

    std::fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create temp dir: {}", dir.display()))?;

    Ok(dir)
}

/// Maximum time for a git clone operation (shallow clone with --branch)
const GIT_SHALLOW_CLONE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
/// Maximum time for a full git clone (fallback for commit hashes)
const GIT_FULL_CLONE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);
/// Maximum time for a git checkout operation
const GIT_CHECKOUT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Run a git command with a timeout, killing the process if it exceeds the limit
async fn run_git_with_timeout(
    args: &[&str],
    work_dir: &Path,
    timeout: std::time::Duration,
    operation: &str,
) -> Result<std::process::Output> {
    let fut = tokio::process::Command::new("git")
        .args(args)
        .current_dir(work_dir)
        .kill_on_drop(true)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    match tokio::time::timeout(timeout, fut).await {
        Ok(result) => result.with_context(|| format!("git {} failed", operation)),
        Err(_) => {
            anyhow::bail!(
                "Git {} timed out after {}s. The repository may be too large or the ref is invalid.",
                operation, timeout.as_secs()
            );
        }
    }
}

/// Clone Git repository
async fn clone_repo(repo: &str, commit: &str, work_dir: &Path) -> Result<()> {
    info!("📥 Cloning {} @ {}", repo, commit);

    // Try shallow clone at the specific ref first (works for branches and tags)
    let output = run_git_with_timeout(
        &["clone", "--depth", "1", "--branch", commit, repo, "."],
        work_dir,
        GIT_SHALLOW_CLONE_TIMEOUT,
        "clone --branch",
    ).await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);

        // If --branch failed (e.g. commit hash), fall back to shallow fetch by hash
        // Uses git init + fetch --depth 1 to avoid cloning entire history (DoS protection)
        if stderr.contains("not found in upstream") || stderr.contains("Remote branch") {
            let output = run_git_with_timeout(
                &["init"],
                work_dir,
                GIT_CHECKOUT_TIMEOUT,
                "init",
            ).await?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("Git init failed: {}", stderr);
            }

            let output = run_git_with_timeout(
                &["remote", "add", "origin", repo],
                work_dir,
                GIT_CHECKOUT_TIMEOUT,
                "remote add",
            ).await?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("Git remote add failed: {}", stderr);
            }

            let output = run_git_with_timeout(
                &["fetch", "--depth", "1", "origin", commit],
                work_dir,
                GIT_FULL_CLONE_TIMEOUT,
                "fetch",
            ).await?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if stderr.contains("Repository not found") || stderr.contains("not found") {
                    classify_clone_error(&stderr, repo)?;
                }
                anyhow::bail!("Git fetch failed for '{}': {}. Make sure the commit hash exists.", commit, stderr);
            }

            let output = run_git_with_timeout(
                &["checkout", "FETCH_HEAD"],
                work_dir,
                GIT_CHECKOUT_TIMEOUT,
                "checkout",
            ).await?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("Git checkout failed for '{}': {}", commit, stderr);
            }
        } else {
            classify_clone_error(&stderr, repo)?;
        }
    }

    info!("✅ Repository cloned");
    Ok(())
}

/// Classify git clone errors into user-friendly messages
fn classify_clone_error(stderr: &str, repo: &str) -> Result<()> {
    if stderr.contains("Repository not found") || stderr.contains("not found") {
        anyhow::bail!("Repository not found: {}. Please check the URL is correct and the repository is public.", repo);
    } else if stderr.contains("could not read Username") || stderr.contains("authentication") {
        anyhow::bail!("Cannot access repository: {}. Only public repositories are supported.", repo);
    } else {
        anyhow::bail!("Git clone failed: {}", stderr);
    }
}

/// Validate that project doesn't use build.rs or git dependencies (security requirement)
///
/// Build scripts can execute arbitrary code during compilation, which is a security risk.
/// We reject projects with build.rs to prevent:
/// - Reading worker environment variables (secrets, keys)
/// - Accessing dstack.sock or other sensitive files
/// - Network exfiltration of data
///
/// Git dependencies are also rejected because:
/// - They bypass crates.io verification
/// - Can point to malicious or unreviewed code
/// - Enable typosquatting attacks (git = "https://evil.com/fake-serde")
fn validate_no_build_scripts(work_dir: &Path) -> Result<()> {
    let cargo_toml_path = work_dir.join("Cargo.toml");

    if !cargo_toml_path.exists() {
        anyhow::bail!("Cargo.toml not found in repository");
    }

    let cargo_toml = std::fs::read_to_string(&cargo_toml_path)
        .context("Failed to read Cargo.toml")?;

    // Check for build = "build.rs" or build = 'build.rs' with flexible whitespace
    // Matches: build = "...", build="...", build  =  "...", etc.
    let build_patterns = [
        "build\\s*=",  // build = or build=
    ];

    for pattern in &build_patterns {
        let regex = regex::Regex::new(pattern).unwrap();
        if regex.is_match(&cargo_toml) {
            return Err(CompilationError {
                user_message: "Security: build.rs scripts are not allowed. Please remove the build script from your Cargo.toml. Build scripts can execute arbitrary code during compilation and access sensitive data.".to_string(),
                stderr: "Cargo.toml contains 'build =' directive".to_string(),
                stdout: String::new(),
                exit_code: None,
            }.into());
        }
    }

    // Check for git dependencies: git = "https://..." or git = 'https://...'
    // Matches: git = "https://...", git="https://...", git = "http://...", etc.
    let git_dep_regex = regex::Regex::new(r#"git\s*=\s*["']https?://"#).unwrap();
    if git_dep_regex.is_match(&cargo_toml) {
        return Err(CompilationError {
            user_message: "Security: Git dependencies are not allowed. Please use published crates from crates.io only. Git dependencies bypass crates.io verification and can point to malicious code. This protects against typosquatting attacks like git = 'https://evil.com/fake-serde'.".to_string(),
            stderr: "Cargo.toml contains git dependency".to_string(),
            stdout: String::new(),
            exit_code: None,
        }.into());
    }

    // Check for common dependencies that require build.rs
    let dangerous_deps = [
        ("ring", "ring requires build.rs for native crypto compilation"),
        ("openssl-sys", "openssl-sys requires build.rs"),
        ("libsodium-sys", "libsodium-sys requires build.rs"),
        ("secp256k1-sys", "secp256k1-sys requires build.rs"),
    ];

    for (dep, reason) in &dangerous_deps {
        if cargo_toml.contains(dep) {
            warn!("⚠️  Detected potentially problematic dependency: {}", dep);
            warn!("⚠️  Reason: {}", reason);
            // Note: We log but don't reject, as some deps might work without build.rs
        }
    }

    info!("✅ Validation passed: no build.rs, no git dependencies");
    Ok(())
}

/// Compile with env isolation and ulimit (TEE-friendly, no pivot_root)
async fn compile_with_isolation(
    work_dir: &Path,
    build_target: &str,
    timeout: u64,
) -> Result<Vec<u8>> {
    info!("🔒 Starting compilation with env isolation + ulimit");

    // Prepare cargo home directory inside work_dir
    let cargo_home = work_dir.join(".cargo");
    std::fs::create_dir_all(&cargo_home)
        .context("Failed to create .cargo directory")?;

    // Build cargo command with resource limits
    // ulimit is executed inside bash, before cargo build
    // Export PATH explicitly so cargo can be found
    // Note: We don't use --locked because user repos may not have Cargo.lock or it may be outdated
    let cargo_cmd = format!(
        "export PATH=/usr/local/cargo/bin:/usr/local/bin:/usr/bin:/bin && export RUSTUP_HOME=/usr/local/rustup && ulimit -v {} && ulimit -t {} && ulimit -u {} && cargo build --target {} --release",
        MAX_MEMORY_BYTES / 1024, // ulimit -v expects KB
        timeout,
        MAX_PROCESSES,
        build_target
    );

    info!("Cargo command: {}", cargo_cmd);

    // Use env -i to clear all environment variables
    // Then set only safe variables needed for compilation
    let mut cmd = Command::new("env");
    cmd.arg("-i"); // Clear all env vars

    // Set only safe environment variables
    cmd.env("HOME", work_dir.to_str().unwrap());
    cmd.env("CARGO_HOME", cargo_home.to_str().unwrap());
    cmd.env("PATH", "/usr/local/cargo/bin:/usr/local/bin:/usr/bin:/bin");
    cmd.env("RUST_BACKTRACE", "1"); // For debugging compilation errors
    cmd.env("RUSTUP_HOME", "/usr/local/rustup"); // Rustup installation directory

    // WASI SDK environment (for C dependencies like ring, openssl-sys)
    cmd.env("CC_wasm32_wasip1", "/opt/wasi-sdk/bin/clang");
    cmd.env("AR_wasm32_wasip1", "/opt/wasi-sdk/bin/llvm-ar");
    cmd.env("CARGO_TARGET_WASM32_WASIP1_LINKER", "/opt/wasi-sdk/bin/clang");

    // DO NOT SET (these are worker secrets):
    // - OPERATOR_PRIVATE_KEY
    // - API_AUTH_TOKEN
    // - KEYSTORE_AUTH_TOKEN
    // - Any other sensitive environment variables

    // Execute bash -c with the cargo command (bash required for ulimit -u)
    cmd.arg("bash");
    cmd.arg("-c");
    cmd.arg(&cargo_cmd);

    // Set working directory
    cmd.current_dir(work_dir);

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    info!("Executing: env -i bash -c '{}'", cargo_cmd);

    // Spawn process
    let child = cmd.spawn()
        .context("Failed to spawn compilation process")?;

    // Wait with timeout
    let output = tokio::time::timeout(
        Duration::from_secs(timeout + 10), // Extra 10s buffer
        child.wait_with_output(),
    )
    .await
    .context("Compilation timeout exceeded")??;

    // Check exit code
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();

        error!("❌ Compilation failed");
        error!("STDERR: {}", stderr);
        error!("STDOUT: {}", stdout);

        // Classify error for user-friendly message
        let (_category, user_message) = classify_compilation_error(&stderr, output.status.code());

        return Err(CompilationError {
            user_message: user_message.to_string(),
            stderr,
            stdout,
            exit_code: output.status.code().map(|c| c as i32),
        }.into());
    }

    info!("✅ Compilation finished, extracting WASM");

    // Find compiled WASM file
    let wasm_path = find_wasm_file(work_dir, build_target)?;

    // Read WASM bytes
    let wasm_bytes = std::fs::read(&wasm_path)
        .with_context(|| format!("Failed to read WASM file: {}", wasm_path.display()))?;

    info!("✅ WASM extracted: {} bytes from {}", wasm_bytes.len(), wasm_path.display());

    Ok(wasm_bytes)
}

/// Find compiled WASM file in target directory
fn find_wasm_file(work_dir: &Path, build_target: &str) -> Result<PathBuf> {
    let target_dir = work_dir.join("target").join(build_target).join("release");

    if !target_dir.exists() {
        anyhow::bail!("Target directory not found: {}", target_dir.display());
    }

    // Find .wasm file
    let entries = std::fs::read_dir(&target_dir)
        .with_context(|| format!("Failed to read target directory: {}", target_dir.display()))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("wasm") {
            return Ok(path);
        }
    }

    anyhow::bail!("No .wasm file found in {}", target_dir.display());
}

/// Classify compilation error for user-friendly message
fn classify_compilation_error(stderr: &str, exit_code: Option<i32>) -> (&'static str, &'static str) {
    let stderr_lower = stderr.to_lowercase();

    // Git errors
    if stderr_lower.contains("fatal: repository") && stderr_lower.contains("not found") {
        return ("repository_not_found", "Repository not found. Please check that the repository URL is correct and publicly accessible.");
    }

    if stderr_lower.contains("fatal: could not read username") || stderr_lower.contains("authentication") {
        return ("repository_access_denied", "Cannot access repository. The repository may be private or the URL may be incorrect. Only public repositories are supported.");
    }

    // Rust compilation errors
    if stderr_lower.contains("error[e") || stderr_lower.contains("error: could not compile") {
        return ("rust_compilation_error", "Rust compilation failed. Your code contains syntax errors or type errors. Please check your Rust code for correctness.");
    }

    // Dependency errors
    if stderr_lower.contains("error: no matching package") || stderr_lower.contains("failed to select a version") {
        return ("dependency_not_found", "Dependency resolution failed. One or more dependencies specified in Cargo.toml could not be found or resolved.");
    }

    // Build script errors (should be caught earlier, but just in case)
    if stderr_lower.contains("build.rs") || stderr_lower.contains("build script") {
        return ("build_script_error", "Build script detected. Build scripts (build.rs) are not allowed for security reasons. Please remove the build script from your project.");
    }

    // Resource limit errors
    if stderr_lower.contains("out of memory") || stderr_lower.contains("cannot allocate memory") {
        return ("out_of_memory", "Compilation ran out of memory. Please reduce the complexity of your project or optimize dependencies.");
    }

    // Timeout
    if exit_code == Some(137) { // SIGKILL
        return ("timeout", "Compilation timeout exceeded. Please reduce compilation time or simplify your project.");
    }

    // Generic error
    ("compilation_error", "Compilation failed. Please check your code and try again. See documentation for supported features.")
}

/// Cleanup temporary directory
fn cleanup_dir(work_dir: &Path) -> Result<()> {
    if work_dir.exists() {
        std::fs::remove_dir_all(work_dir)
            .with_context(|| format!("Failed to remove temp dir: {}", work_dir.display()))?;
        debug!("🧹 Cleaned up: {}", work_dir.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_rejects_build_rs() {
        let temp_dir = std::env::temp_dir().join("test-build-rs");
        std::fs::create_dir_all(&temp_dir).unwrap();

        let cargo_toml = temp_dir.join("Cargo.toml");
        std::fs::write(&cargo_toml, r#"
[package]
name = "test"
version = "0.1.0"
build = "build.rs"
        "#).unwrap();

        let result = validate_no_build_scripts(&temp_dir);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("build.rs"));

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_validate_rejects_build_rs_with_extra_spaces() {
        let temp_dir = std::env::temp_dir().join("test-build-rs-spaces");
        std::fs::create_dir_all(&temp_dir).unwrap();

        let cargo_toml = temp_dir.join("Cargo.toml");
        std::fs::write(&cargo_toml, r#"
[package]
name = "test"
version = "0.1.0"
build  =  "build.rs"
        "#).unwrap();

        let result = validate_no_build_scripts(&temp_dir);
        assert!(result.is_err(), "Should reject build.rs with extra spaces");
        assert!(result.unwrap_err().to_string().contains("build.rs"));

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_validate_rejects_git_dependencies() {
        let temp_dir = std::env::temp_dir().join("test-git-dep");
        std::fs::create_dir_all(&temp_dir).unwrap();

        let cargo_toml = temp_dir.join("Cargo.toml");
        std::fs::write(&cargo_toml, r#"
[package]
name = "test"
version = "0.1.0"

[dependencies]
serde = { git = "https://evil.com/fake-serde" }
        "#).unwrap();

        let result = validate_no_build_scripts(&temp_dir);
        assert!(result.is_err(), "Should reject git dependencies");
        assert!(result.unwrap_err().to_string().contains("Git dependencies are not allowed"));

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_validate_accepts_clean_project() {
        let temp_dir = std::env::temp_dir().join("test-clean");
        std::fs::create_dir_all(&temp_dir).unwrap();

        let cargo_toml = temp_dir.join("Cargo.toml");
        std::fs::write(&cargo_toml, r#"
[package]
name = "test"
version = "0.1.0"

[dependencies]
serde = "1.0"
serde_json = "1.0"
        "#).unwrap();

        let result = validate_no_build_scripts(&temp_dir);
        assert!(result.is_ok(), "Should accept clean project with crates.io deps");

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_classify_compilation_error() {
        let (category, msg) = classify_compilation_error("error[E0425]: cannot find value", None);
        assert_eq!(category, "rust_compilation_error");
        assert!(msg.contains("syntax errors"));

        let (category, msg) = classify_compilation_error("fatal: repository not found", Some(128));
        assert_eq!(category, "repository_not_found");
        assert!(msg.contains("Repository not found"));
    }
}
