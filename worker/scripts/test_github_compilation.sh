#!/bin/bash
# Test script for GitHub compilation
# This script tests the real Docker-based WASM compilation from GitHub

set -e

echo "🧪 Testing GitHub WASM Compilation"
echo "=================================="
echo ""

# Check if Docker is running
if ! docker info > /dev/null 2>&1; then
    echo "❌ Error: Docker is not running!"
    echo "Please start Docker and try again."
    exit 1
fi

echo "✅ Docker is running"
echo ""

# Run the ignored integration test
echo "Running compilation test for:"
echo "  Repository: https://github.com/zavodil/random-ark"
echo "  Commit: 6491b317afa33534b56cebe9957844e16ac720e8"
echo "  Target: wasm32-wasi"
echo ""

cd "$(dirname "$0")/.."

cargo test test_real_github_compilation -- --ignored --nocapture

echo ""
echo "✅ Test completed successfully!"
