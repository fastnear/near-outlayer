#!/bin/bash
# Test GitHub WASM compilation with Docker

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "🧪 Compilation Test - GitHub to WASM"
echo "====================================="
echo ""

# Check if Docker is running
echo "🔍 Checking prerequisites..."
if ! docker info > /dev/null 2>&1; then
    echo "❌ Error: Docker is not running!"
    echo "   Please start Docker and try again."
    exit 1
fi
echo "✓ Docker is running"
echo ""

# Run the compilation integration test
echo "📦 Testing compilation:"
echo "  Repository: https://github.com/zavodil/random-ark"
echo "  Commit: 6491b317afa33534b56cebe9957844e16ac720e8"
echo "  Target: wasm32-wasi"
echo ""

cd "$PROJECT_ROOT/worker"
cargo test test_real_github_compilation -- --ignored --nocapture

echo ""
echo "✅ Compilation test passed!"
