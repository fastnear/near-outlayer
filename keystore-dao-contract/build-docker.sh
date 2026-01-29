#!/bin/bash
set -e

# cargo generate-lockfile

echo "Building keystore-dao-contract in Docker..."

# Use the same Docker image as MPC/register contracts
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
cp target/near/keystore_dao_contract.wasm res/keystore_dao_contract.wasm

# Show file size
ls -lh res/keystore_dao_contract.wasm

echo "✅ Build complete: res/keystore_dao_contract.wasm"
echo "Built in Docker: $DOCKER_IMAGE"

# Deploy and init example:
# near contract deploy dao.outlayer.testnet use-file res/keystore_dao_contract.wasm with-init-call new json-args '{"owner_id": "owner.outlayer.testnet", "init_account_id": "init-keystore.outlayer.testnet", "dao_members": ["zavodil.testnet"], "mpc_contract_id": "v1.signer-prod.testnet"}' prepaid-gas '30.0 Tgas' attached-deposit '0 NEAR' sign-as dao.outlayer.testnet network-config testnet sign-with-keychain send