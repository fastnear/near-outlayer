#!/bin/bash
set -e

cd $(dirname $0)
mkdir -p res/local

echo "Building contract..."

# Build the contract
cargo near build non-reproducible-wasm

cp target/near/outlayer_contract.wasm res/local/

# Check if build was successful
if [ $? -eq 0 ]; then
    echo "✅ Contract built successfully!"
    echo "WASM file location: res/local/outlayer_contract.wasm"
else
    echo "❌ Build failed!"
    exit 1
fi
