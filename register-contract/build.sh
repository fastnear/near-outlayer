#!/bin/bash
set -e

echo "Building register-contract..."

# Build with LLVM for WASM target (required for ring crate)
# --no-wasmopt: cargo-near's wasm-opt has bulk-memory validation issues
CC=/Users/alice/.local/opt/llvm/bin/clang \
AR=/Users/alice/.local/opt/llvm/bin/llvm-ar \
cargo near build non-reproducible-wasm --no-wasmopt

# Create res directory if not exists
mkdir -p res

# Copy WASM file (without wasm-opt post-processing)
cp target/near/register_contract.wasm res/register_contract.wasm

# Show file size
ls -lh res/register_contract.wasm

echo "✅ Build complete: res/register_contract.wasm"
echo "Note: Built without wasm-opt due to bulk-memory operations in dcap-qvl/ring dependencies"

# deploy {"owner_id": "owner.outlayer.testnet", "init_worker_account": "init-worker.outlayer.testnet"}
