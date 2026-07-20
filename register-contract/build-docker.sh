#!/bin/bash
set -e

# cargo generate-lockfile

echo "Building register-contract in Docker (following MPC pattern)..."

# Use the same Docker image as MPC contract
# rust >= 1.93 is required by near-sdk 5.29 (post-quantum ml-dsa-65 PublicKey support)
DOCKER_IMAGE="sourcescan/cargo-near:0.22.0-rust-1.97.1"

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
