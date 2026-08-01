#!/bin/bash
# Unit tests for WASM execution

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "🧪 Unit Tests - WASM Execution"
echo "==============================="
echo ""

# Build test WASM modules
echo "🔨 Building test WASM modules..."
echo ""

# Build get-random (WASI P1)
echo "📦 Building random-ark (WASI P1)..."
cd "$PROJECT_ROOT/wasi-examples/random-ark"
cargo build --release --target wasm32-wasip1 --quiet
echo "✓ random-ark built successfully"

# Build ai-example (WASI P2) if needed
if [ -d "$PROJECT_ROOT/wasi-examples/ai-example" ]; then
    echo "📦 Building ai-example (WASI P2)..."
    cd "$PROJECT_ROOT/wasi-examples/ai-example"
    cargo build --release --target wasm32-wasip2 --quiet
    echo "✓ ai-example built successfully"
fi

echo ""

# Run worker unit tests
echo "🧪 Running worker unit tests..."
echo ""
cd "$PROJECT_ROOT/worker"
cargo test --quiet

echo ""
echo "✅ All unit tests passed!"
