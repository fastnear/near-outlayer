//! Docker container operations for WASM compilation

use anyhow::{Context, Result};
use bollard::container::{Config as ContainerConfig, CreateContainerOptions, RemoveContainerOptions, DownloadFromContainerOptions};
use bollard::exec::{CreateExecOptions, StartExecResults};
use bollard::image::CreateImageOptions;
use bollard::Docker;
use futures_util::stream::StreamExt;
use futures_util::TryStreamExt;
use tracing::{debug, error, info, warn};

use crate::config::Config;

/// Ensure Docker image is available (pull if needed)
pub async fn ensure_image(docker: &Docker, image: &str) -> Result<()> {
    info!("Ensuring Docker image is available: {}", image);

    let mut stream = docker.create_image(
        Some(CreateImageOptions {
            from_image: image.to_string(),
            ..Default::default()
        }),
        None,
        None,
    );

    while let Some(result) = stream.next().await {
        match result {
            Ok(info_msg) => {
                if let Some(status) = info_msg.status {
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

/// Create and start a Docker container for compilation
pub async fn create_container(
    docker: &Docker,
    name: &str,
    config: &Config,
    repo: &str,
    commit: &str,
    build_target: &str,
) -> Result<String> {
    info!("Creating Docker container: {}", name);

    let container_config = ContainerConfig {
        image: Some(config.docker_image.clone()),
        cmd: Some(vec!["sleep".to_string(), "600".to_string()]),
        working_dir: Some("/workspace".to_string()),
        host_config: Some(bollard::models::HostConfig {
            network_mode: Some("bridge".to_string()),
            memory: Some((config.compile_memory_limit_mb as i64) * 1024 * 1024),
            nano_cpus: Some((config.compile_cpu_limit * 1_000_000_000.0) as i64),
            ..Default::default()
        }),
        env: Some(vec![
            format!("REPO={}", repo),
            format!("COMMIT={}", commit),
            format!("BUILD_TARGET={}", build_target),
        ]),
        ..Default::default()
    };

    let response = docker
        .create_container(
            Some(CreateContainerOptions {
                name: name.to_string(),
                ..Default::default()
            }),
            container_config,
        )
        .await
        .context("Failed to create container")?;

    docker
        .start_container::<String>(&response.id, None)
        .await
        .context("Failed to start container")?;

    info!("Container created and started: {}", response.id);
    Ok(response.id)
}

/// Execute a shell command in container
pub async fn exec_in_container(docker: &Docker, container_id: &str, cmd: &str) -> Result<()> {
    debug!("Executing in container: {}", cmd);

    let exec = docker
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

    let mut output = match docker.start_exec(&exec.id, None).await? {
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
                            info!("📦 [stdout] {}", line);
                            stdout_lines.push(line.to_string());
                        }
                    }
                    LogOutput::StdErr { message } => {
                        let text = String::from_utf8_lossy(&message);
                        for line in text.lines() {
                            warn!("📦 [stderr] {}", line);
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
    let inspect = docker.inspect_exec(&exec.id).await?;
    if let Some(exit_code) = inspect.exit_code {
        if exit_code != 0 {
            error!("❌ Container command failed with exit code: {}", exit_code);
            let error_msg = extract_compilation_error(&stderr_lines, &stdout_lines);
            anyhow::bail!("Command failed with exit code {}: {}", exit_code, error_msg);
        }
    }

    Ok(())
}

/// Extract WASM file from container
pub async fn extract_wasm(docker: &Docker, container_id: &str, wasm_path: &str) -> Result<Vec<u8>> {
    info!("Extracting WASM from container: {}", wasm_path);

    use tar::Archive;
    use std::io::Read;

    let mut tar_stream = docker.download_from_container(
        container_id,
        Some(DownloadFromContainerOptions {
            path: wasm_path.to_string(),
        }),
    );

    let mut tar_data = Vec::new();
    while let Some(chunk) = tar_stream.try_next().await? {
        tar_data.extend_from_slice(&chunk);
    }

    let mut archive = Archive::new(&tar_data[..]);

    for entry in archive.entries().context("Failed to read tar entries")? {
        let mut entry = entry.context("Failed to read tar entry")?;

        let mut wasm_bytes = Vec::new();
        entry
            .read_to_end(&mut wasm_bytes)
            .context("Failed to read WASM from tar")?;

        if !wasm_bytes.is_empty() {
            info!("✅ Extracted WASM: {} bytes", wasm_bytes.len());
            return Ok(wasm_bytes);
        }
    }

    anyhow::bail!("WASM file not found in container at {}", wasm_path);
}

/// Cleanup container (stop and remove)
pub async fn cleanup_container(docker: &Docker, container_id: &str) -> Result<()> {
    info!("Cleaning up container: {}", container_id);

    let _ = docker.stop_container(container_id, None).await;

    docker
        .remove_container(
            container_id,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await
        .context("Failed to remove container")?;

    info!("Container removed: {}", container_id);
    Ok(())
}

/// Extract meaningful error message from compilation output
fn extract_compilation_error(stderr_lines: &[String], stdout_lines: &[String]) -> String {
    let mut error_lines = Vec::new();

    for line in stderr_lines.iter().rev().take(50) {
        if line.contains("error:") || line.contains("error[E") {
            error_lines.insert(0, line.clone());
        } else if !error_lines.is_empty() && (line.starts_with("   ") || line.starts_with("  ")) {
            error_lines.insert(0, line.clone());
        }
    }

    if !error_lines.is_empty() {
        return error_lines.join("\n");
    }

    for line in stdout_lines.iter().rev().take(20) {
        if line.contains("ERROR") || line.contains("error:") {
            return line.clone();
        }
    }

    stderr_lines
        .iter()
        .rev()
        .take(10)
        .rev()
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
}
