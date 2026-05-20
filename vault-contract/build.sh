#!/bin/bash
set -e

echo "Building vault-contract (non-reproducible)..."

cargo near build non-reproducible-wasm --no-abi

mkdir -p res
cp target/near/vault_contract.wasm res/vault_contract.wasm

ls -lh res/vault_contract.wasm

# Compute and print sha256 (NEAR contract code hash uses sha256 over the WASM bytes)
HASH_HEX=$(shasum -a 256 res/vault_contract.wasm | awk '{print $1}')
echo "✅ Build complete: res/vault_contract.wasm"
echo "   sha256 (hex):    $HASH_HEX"
echo "   NOTE: This is a non-reproducible build. Use ./build-docker.sh to obtain"
echo "         the canonical hash for keystore-dao approved_vault_code_hashes."
