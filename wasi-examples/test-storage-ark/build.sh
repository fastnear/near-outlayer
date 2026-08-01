#!/bin/bash
set -e

echo "Building test-storage-ark for wasm32-wasip2..."

cargo build --target wasm32-wasip2 --release

echo "Build complete!"
echo "WASM file: target/wasm32-wasip2/release/test-storage-ark.wasm"
echo ""
echo "File size:"
ls -lh target/wasm32-wasip2/release/test-storage-ark.wasm
