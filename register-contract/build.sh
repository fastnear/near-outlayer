#!/bin/bash
set -e

echo "Building register-contract..."

# Build with LLVM for WASM target (required for ring crate)
# --no-abi: Don't embed ABI into the contract (prevents deserialization errors)
# --no-wasmopt: cargo-near's wasm-opt has bulk-memory validation issues
CC=/Users/alice/.local/opt/llvm/bin/clang \
AR=/Users/alice/.local/opt/llvm/bin/llvm-ar \
cargo near build non-reproducible-wasm --no-abi --no-wasmopt

# Create res directory if not exists
mkdir -p res

# Copy WASM file (without wasm-opt post-processing)
cp target/near/register_contract.wasm res/register_contract.wasm

# Show file size
ls -lh res/register_contract.wasm

echo "✅ Build complete: res/register_contract.wasm"
echo "Note: Built without wasm-opt due to bulk-memory operations in dcap-qvl/ring dependencies"

# near contract deploy worker.outlayer.testnet use-file target/near/register_contract.wasm with-init-call new json-args '{"owner_id": "owner.outlayer.testnet", "init_worker_account": "init-worker.outlayer.testnet"}' prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' network-config testnet sign-with-keychain send
# near contract deploy worker.outlayer.testnet use-file res/register_contract.wasm without-init-call network-config testnet sign-with-keychain send
near contract call-function as-transaction worker.outlayer.testnet migrate json-args '{"outlayer_contract_id":"outlayer.testnet"}' prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' sign-as worker.outlayer.testnet network-config testnet sign-with-keychain send

# mainnet
# near contract deploy worker.outlayer.near use-file worker-contract.wasm with-init-call new json-args '{"owner_id": "owner.outlayer.near", "init_worker_account": "init-worker.outlayer.near"}' prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' network-config mainnet sign-with-keychain send

# Add approved measurements (use scripts/deploy_phala.sh to extract all 5 measurements)
# near contract call-function as-transaction worker.outlayer.testnet add_approved_measurements json-args '{"measurements":{"mrtd":"...","rtmr0":"...","rtmr1":"...","rtmr2":"...","rtmr3":"..."}, "clear_others": true}' prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' sign-as owner.outlayer.testnet network-config testnet sign-with-legacy-keychain send