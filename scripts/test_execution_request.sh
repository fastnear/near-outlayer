#!/bin/bash

# Test script to call request_execution on offchainvm.testnet contract
# This will trigger the full execution flow

CONTRACT_ID="outlayer.testnet"
CALLER_ACCOUNT="outlayer.testnet"  # Using owner account for testing
PAYMENT="0.1"  # 0.1 NEAR deposit

echo "Calling request_execution on $CONTRACT_ID..."
echo "Repo: https://github.com/zavodil/random-ark"
echo "Commit: main"
echo ""

near contract call-function as-transaction $CONTRACT_ID request_execution json-args \
'{
  "code_source": {
    "repo": "https://github.com/zavodil/random-ark",
    "commit": "main",
    "build_target": "wasm32-wasip1"
  },
  "input_data": "{\"min\": 100, \"max\": 5000}",
  "resource_limits": {
    "max_instructions": 10000000000,
    "max_memory_mb": 128,
    "max_execution_seconds": 60
  }
}' \
prepaid-gas '300.0 Tgas' \
attached-deposit "$PAYMENT NEAR" \
sign-as $CALLER_ACCOUNT \
network-config testnet \
sign-with-keychain \
send

echo ""
echo "Transaction sent! Check events with:"
echo "near contract view-function $CONTRACT_ID get_last_request json-args '{}' network-config testnet now"
