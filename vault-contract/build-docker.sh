#!/bin/bash
set -e

echo "Building vault-contract reproducibly in Docker..."

# Same image as keystore-dao-contract / register-contract for cross-contract
# build hash consistency.
DOCKER_IMAGE="sourcescan/cargo-near:0.17.0-rust-1.86.0"

docker run --rm \
  -v "$(pwd)":/contract \
  -w /contract \
  "$DOCKER_IMAGE" \
  cargo near build non-reproducible-wasm --locked --no-abi

mkdir -p res
cp target/near/vault_contract.wasm res/vault_contract.wasm

ls -lh res/vault_contract.wasm

HASH_HEX=$(shasum -a 256 res/vault_contract.wasm | awk '{print $1}')
echo "✅ Build complete: res/vault_contract.wasm"
echo "   Built in:        $DOCKER_IMAGE"
echo "   sha256 (hex):    $HASH_HEX"
echo
echo "This sha256 is the value to add to keystore-dao approved_vault_code_hashes"
echo "(in NEAR cli — wrap in base58 with the Base58CryptoHash type)."
