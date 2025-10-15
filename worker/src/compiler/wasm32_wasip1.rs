//! WASI Preview 1 (P1) compiler
//!
//! Compiles Rust code to wasm32-wasip1 or wasm32-wasi targets using:
//! - cargo build --release --target wasm32-wasip1
//! - wasm-opt for size optimization

use anyhow::Result;
use bollard::Docker;
use tracing::info;

use super::docker;

/// Compile WASM for WASI Preview 1 target
///
/// This function:
/// 1. Clones the repository
/// 2. Installs wasm32-wasip1 (or wasm32-wasi) target
/// 3. Builds with cargo
/// 4. Optimizes with wasm-opt (-Oz, strip DWARF)
///
/// # Arguments
/// * `docker` - Docker client
/// * `container_id` - Container ID where compilation happens
/// * `build_target` - Build target (wasm32-wasip1 or wasm32-wasi)
///
/// # Returns
/// * `Ok(Vec<u8>)` - Compiled and optimized WASM bytes
/// * `Err(_)` - Compilation failed
pub async fn compile(docker: &Docker, container_id: &str, build_target: &str) -> Result<Vec<u8>> {
    info!("Compiling WASI Preview 1 module: target={}", build_target);
    let start_time = std::time::Instant::now();

    // Compilation script for WASI P1
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
fi
rustup target add $TARGET_TO_ADD
echo "🔧 TARGET_TO_ADD: $TARGET_TO_ADD"

# Clone repository
git clone $REPO repo
cd repo
git checkout $COMMIT

# Build WASM with size optimizations
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

# Optimize WASM module with wasm-opt
ORIGINAL_SIZE=$(stat -c%s /workspace/output/output.wasm 2>/dev/null || stat -f%z /workspace/output/output.wasm)

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
    # --enable-sign-ext: enable sign extension operations
    # --enable-bulk-memory: enable bulk memory operations
    wasm-opt -Oz \
        --strip-dwarf \
        --strip-producers \
        --enable-sign-ext \
        --enable-bulk-memory \
        /workspace/output/output.wasm \
        -o /workspace/output/output_optimized.wasm

    mv /workspace/output/output_optimized.wasm /workspace/output/output.wasm
    OPTIMIZED_SIZE=$(stat -c%s /workspace/output/output.wasm 2>/dev/null || stat -f%z /workspace/output/output.wasm)
    SAVED=$((ORIGINAL_SIZE - OPTIMIZED_SIZE))
    PERCENT=$((SAVED * 100 / ORIGINAL_SIZE))
    echo "✅ Module optimization complete: $ORIGINAL_SIZE bytes → $OPTIMIZED_SIZE bytes (saved $SAVED bytes / $PERCENT%)"
else
    echo "⚠️  wasm-opt not available, skipping module optimization"
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
        "✅ WASI P1 compilation successful: WASM size={} bytes, total_time={:.2}s",
        wasm_bytes.len(),
        elapsed.as_secs_f64()
    );

    Ok(wasm_bytes)
}
