#!/bin/bash
# Быстрый скрипт для запуска теста WASM execution

set -e

echo "🔨 Building get-random example..."
cd ../wasi-examples/get-random
cargo build --release --target wasm32-wasip1

echo ""
echo "✅ get-random example built successfully"
echo ""

cd ../worker

echo "🧪 Running WASM execution test..."
echo ""
cargo test test_wasm_execution -- --nocapture

echo ""
echo "✅ Test completed!"
