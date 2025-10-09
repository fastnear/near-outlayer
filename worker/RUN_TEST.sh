#!/bin/bash
# Быстрый скрипт для запуска теста WASM execution

set -e

echo "🔨 Building test-wasm..."
cd ../test-wasm
cargo build --release --target wasm32-unknown-unknown

echo ""
echo "✅ test-wasm built successfully"
echo ""

cd ../worker

echo "🧪 Running WASM execution test..."
echo ""
cargo test test_wasm_execution -- --nocapture

echo ""
echo "✅ Test completed!"
