//! WASI Preview 2 (P2) compiler
//!
//! Compiles Rust code to wasm32-wasip2 target using:
//! - cargo build --release --target wasm32-wasip2
//! - wasm-tools strip for optimization

use anyhow::Result;
use bollard::Docker;
use tracing::info;

use super::docker;

/// Compile WASM for WASI Preview 2 target
///
/// This function:
/// 1. Clones the repository
/// 2. Installs wasm32-wasip2 target
/// 3. Builds with cargo (produces CLI component)
/// 4. Strips debug info with wasm-tools
///
/// # Arguments
/// * `docker` - Docker client
/// * `container_id` - Container ID where compilation happens
/// * `build_target` - Build target (wasm32-wasip2)
///
/// # Returns
/// * `Ok(Vec<u8>)` - Compiled and optimized WASM component bytes
/// * `Err(_)` - Compilation failed
pub async fn compile(docker: &Docker, container_id: &str, build_target: &str) -> Result<Vec<u8>> {
    info!("Compiling WASI Preview 2 component: target={}", build_target);
    let start_time = std::time::Instant::now();

    // Compilation script for WASI P2
    let compile_script = format!(
        r#"
set -ex
cd /workspace

# Track compilation time
START_TIME=$(date +%s)

# Setup Rust environment
if [ -f /usr/local/cargo/env ]; then
    . /usr/local/cargo/env
elif [ -f $HOME/.cargo/env ]; then
    . $HOME/.cargo/env
fi

# Add WASM target for WASI Preview 2
echo "ℹ️  Using wasm32-wasip2 (WASI Preview 2)"
rustup target add wasm32-wasip2
echo "🔧 TARGET: wasm32-wasip2"

# Clone repository
git clone $REPO repo
cd repo
git checkout $COMMIT

# Build WASM component with cargo
cargo build --release --target wasm32-wasip2
WASM_FILE=$(find target/wasm32-wasip2/release -maxdepth 1 -name "*.wasm" -type f | head -1)

# Find compiled WASM
if [ -z "$WASM_FILE" ]; then
    echo "❌ ERROR: No WASM file found!"
    find target/wasm32-wasip2/release -type f
    exit 1
fi

echo "📦 Original WASM component: $WASM_FILE"
ls -lah "$WASM_FILE"

# Copy to output
mkdir -p /workspace/output
cp "$WASM_FILE" /workspace/output/output.wasm

# Optimize WASM P2 component
ORIGINAL_SIZE=$(stat -c%s /workspace/output/output.wasm 2>/dev/null || stat -f%z /workspace/output/output.wasm)

# WASI Preview 2: already a proper CLI component from cargo
# Just strip debug info and optimize
if command -v wasm-tools &> /dev/null; then
    echo "🔧 Optimizing WASI P2 CLI component..."

    # Strip debug information from component
    wasm-tools strip /workspace/output/output.wasm -o /workspace/output/output_optimized.wasm
    mv /workspace/output/output_optimized.wasm /workspace/output/output.wasm

    OPTIMIZED_SIZE=$(stat -c%s /workspace/output/output.wasm 2>/dev/null || stat -f%z /workspace/output/output.wasm)
    SAVED=$((ORIGINAL_SIZE - OPTIMIZED_SIZE))
    PERCENT=$((SAVED * 100 / ORIGINAL_SIZE))
    echo "✅ Component optimization complete: $ORIGINAL_SIZE bytes → $OPTIMIZED_SIZE bytes (saved $SAVED bytes / $PERCENT%)"
else
    echo "ℹ️  wasm-tools not available - skipping WASI Preview 2 component optimization"
    echo "   Component size: $ORIGINAL_SIZE bytes"
fi

ls -lah /workspace/output/output.wasm

# Calculate total compilation time
END_TIME=$(date +%s)
COMPILE_TIME=$((END_TIME - START_TIME))
echo "⏱️  Total compilation time: $COMPILE_TIME seconds"
"#
    );

    // Execute compilation in container
    docker::exec_in_container(docker, container_id, &compile_script).await?;

    // Extract WASM file from container
    let wasm_bytes = docker::extract_wasm(docker, container_id, "/workspace/output/output.wasm").await?;

    let elapsed = start_time.elapsed();
    info!(
        "✅ WASI P2 compilation successful: Component size={} bytes, total_time={:.2}s",
        wasm_bytes.len(),
        elapsed.as_secs_f64()
    );

    Ok(wasm_bytes)
}
