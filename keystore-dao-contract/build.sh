#!/bin/bash
set -e

echo "Building keystore-dao-contract..."

# Build with LLVM for WASM target (required for ring crate)
# --no-abi: Don't embed ABI into the contract (prevents deserialization errors)
# --no-wasmopt: cargo-near's wasm-opt has bulk-memory validation issues
CC=/Users/alice/.local/opt/llvm/bin/clang \
AR=/Users/alice/.local/opt/llvm/bin/llvm-ar \
cargo near build non-reproducible-wasm --no-abi --no-wasmopt

# Create res directory if not exists
mkdir -p res

# Copy WASM file (without wasm-opt post-processing)
cp target/near/keystore_dao_contract.wasm res/keystore_dao_contract.wasm

# Show file size
ls -lh res/keystore_dao_contract.wasm

echo "✅ Build complete: res/keystore_dao_contract.wasm"
echo "Note: Built without wasm-opt due to bulk-memory operations in dcap-qvl/ring dependencies"

# near account create-account fund-myself dao.outlayer.testnet '5 NEAR' autogenerate-new-keypair save-to-legacy-keychain sign-as outlayer.testnet network-config testnet sign-with-keychain send
# near contract deploy dao.outlayer.testnet use-file res/keystore_dao_contract.wasm without-init-call network-config testnet sign-with-keychain send
# near contract call-function as-transaction dao.outlayer.testnet new json-args '{"owner_id": "owner.outlayer.testnet", "init_account_id": "init-keystore.outlayer.testnet", "dao_members": ["zavodil.testnet"], "mpc_contract_id": "v1.signer-prod.testnet"}' prepaid-gas '30.0 Tgas' attached-deposit '0 NEAR' sign-as dao.outlayer.testnet network-config testnet sign-with-keychain send

# MAINNET
# near contract deploy dao.outlayer.near use-file res/keystore_dao_contract.wasm without-init-call network-config mainnet sign-with-keychain send
# near contract call-function as-transaction dao.outlayer.near new json-args '{"owner_id": "owner.outlayer.near", "init_account_id": "init-keystore.outlayer.near", "dao_members": ["zavodil.near"], "mpc_contract_id": "v1.signer"}' prepaid-gas '30.0 Tgas' attached-deposit '0 NEAR' sign-as dao.outlayer.near network-config mainnet sign-with-keychain send

# add collateral
# ./scripts/update_collateral.sh

# add rtmr3 from phala
# near contract call-function as-transaction dao.outlayer.testnet add_approved_rtmr3 json-args '{"rtmr3": "911f520e6cf959c314323931f7b8ce120964c969c1c8b3337828b9b1943969d9bb62c6b1f8e92162fe054a076fb5cfbb"}' prepaid-gas '30.0 Tgas' attached-deposit '0 NEAR' sign-as owner.outlayer.testnet network-config testnet sign-with-keychain send