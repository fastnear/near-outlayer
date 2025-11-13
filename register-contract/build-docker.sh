#!/bin/bash
set -e

echo "Building register-contract in Docker (following MPC pattern)..."

# Use the same Docker image as MPC contract
DOCKER_IMAGE="sourcescan/cargo-near:0.17.0-rust-1.86.0"

# Run build in Docker container
docker run --rm \
  -v "$(pwd)":/contract \
  -w /contract \
  "$DOCKER_IMAGE" \
  cargo near build non-reproducible-wasm --locked --features abi --no-embed-abi

# Create res directory if not exists
mkdir -p res

# Copy WASM file
cp target/near/register_contract.wasm res/register_contract.wasm

# Show file size
ls -lh res/register_contract.wasm

echo "✅ Build complete: res/register_contract.wasm"
echo "Built in Docker: $DOCKER_IMAGE"

# deploy {"owner_id": "owner.outlayer.testnet", "init_worker_account": "init-worker.outlayer.testnet"}
