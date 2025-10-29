#!/bin/bash
# Integration test using real transactions from worker/test_transactions.txt
#
# This test sends real execution requests to the testnet contract
# and verifies that the worker processes them correctly.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo ""
echo "🧪 Transaction Integration Tests"
echo "================================="
echo ""

# Configuration
CONTRACT_ID="${CONTRACT_ID:-outlayer.testnet}"
CALLER_ACCOUNT="${CALLER_ACCOUNT:-outlayer.testnet}"
SIGN_METHOD="${SIGN_METHOD:-sign-with-legacy-keychain}"  # or sign-with-plaintext-private-key

echo "📝 Configuration:"
echo "  Contract: $CONTRACT_ID"
echo "  Caller: $CALLER_ACCOUNT"
echo "  Sign method: $SIGN_METHOD"
echo ""

# Check prerequisites
echo "🔍 Checking prerequisites..."

# Check NEAR CLI
if ! command -v near &> /dev/null; then
    echo -e "${RED}❌ NEAR CLI not found${NC}"
    echo "   Install with: npm install -g near-cli"
    exit 1
fi
echo "✓ NEAR CLI available"

# Check coordinator
if ! curl -s http://localhost:8080/health > /dev/null 2>&1; then
    echo -e "${YELLOW}⚠️  Warning: Coordinator not responding${NC}"
    echo "   Make sure coordinator is running: cd coordinator && cargo run"
    echo ""
fi

# Check worker configuration
if [ ! -f "$PROJECT_ROOT/worker/.env" ]; then
    echo -e "${YELLOW}⚠️  Warning: Worker .env not found${NC}"
    echo "   Copy from .env.example and configure"
    echo ""
fi

echo ""
echo -e "${BLUE}═══════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}Test 1/3: WASI P1 Execution (random-ark)${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════════════════${NC}"
echo ""

echo "📦 Sending execution request..."
echo "  Repo: https://github.com/zavodil/random-ark"
echo "  Target: wasm32-wasip1"
echo "  Input: {\"min\": 100, \"max\": 5000}"
echo ""

TX1_OUTPUT=$(near contract call-function as-transaction \
  "$CONTRACT_ID" \
  request_execution \
  json-args '{
    "code_source": {
      "repo": "https://github.com/zavodil/random-ark",
      "commit": "main",
      "build_target": "wasm32-wasip1"
    },
    "resource_limits": {
      "max_instructions": 10000000000,
      "max_memory_mb": 128,
      "max_execution_seconds": 60
    },
    "input_data": "{\"min\": 100, \"max\": 5000}"
  }' \
  prepaid-gas '300.0 Tgas' \
  attached-deposit '0.1 NEAR' \
  sign-as "$CALLER_ACCOUNT" \
  network-config testnet \
  $SIGN_METHOD \
  send 2>&1)

if echo "$TX1_OUTPUT" | grep -q -E "(Transaction ID:|succeeded)"; then
    echo -e "${GREEN}✓ Transaction 1 sent successfully${NC}"
    echo ""

    # Extract and display transaction ID
    TX_ID=$(echo "$TX1_OUTPUT" | grep "Transaction ID:" | sed 's/.*Transaction ID: //' | awk '{print $1}')
    if [ -n "$TX_ID" ]; then
        echo -e "${BLUE}Transaction ID:${NC} $TX_ID"
        echo -e "${BLUE}Explorer:${NC} https://testnet.nearblocks.io/txns/$TX_ID"
        echo ""
    fi

    # Display execution result if present
    echo -e "${BLUE}📋 Execution Result:${NC}"
    if echo "$TX1_OUTPUT" | grep -q "random_number"; then
        RESULT=$(echo "$TX1_OUTPUT" | grep "random_number" | tail -1)
        echo "  $RESULT"
    fi

    # Display logs
    echo ""
    echo -e "${BLUE}📊 Transaction Logs:${NC}"
    echo "$TX1_OUTPUT" | sed -n '/Function execution logs/,/Function execution return value/p' | grep -E "(Logs|execution_completed|random_number|Resources|Cost|Refund)" | sed 's/^/  /'
    echo ""

    echo "📍 Next steps:"
    echo "  • Worker: Watch for compilation and execution logs"
    echo "  • Contract: https://testnet.nearblocks.io/address/$CONTRACT_ID"
    echo ""
else
    echo -e "${RED}✗ Transaction 1 failed${NC}"
    echo "$TX1_OUTPUT"
    exit 1
fi

sleep 3

echo ""
echo -e "${BLUE}═══════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}Test 2/3: WASI P1 Execution (echo-ark)${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════════════════${NC}"
echo ""

echo "📦 Sending execution request..."
echo "  Repo: https://github.com/zavodil/echo-ark"
echo "  Target: wasm32-wasip1"
echo "  Input: Hello, world"
echo ""

TX2_OUTPUT=$(near contract call-function as-transaction \
  "$CONTRACT_ID" \
  request_execution \
  json-args '{
    "code_source": {
      "repo": "https://github.com/zavodil/echo-ark",
      "commit": "main",
      "build_target": "wasm32-wasip1"
    },
    "resource_limits": {
      "max_instructions": 10000000000,
      "max_memory_mb": 128,
      "max_execution_seconds": 60
    },
    "input_data": "Hello, world"
  }' \
  prepaid-gas '300.0 Tgas' \
  attached-deposit '0.1 NEAR' \
  sign-as "$CALLER_ACCOUNT" \
  network-config testnet \
  $SIGN_METHOD \
  send 2>&1)

if echo "$TX2_OUTPUT" | grep -q -E "(Transaction ID:|succeeded)"; then
    echo -e "${GREEN}✓ Transaction 1 sent successfully${NC}"
    echo ""

    # Extract and display transaction ID
    TX_ID=$(echo "$TX2_OUTPUT" | grep "Transaction ID:" | sed 's/.*Transaction ID: //' | awk '{print $1}')
    if [ -n "$TX_ID" ]; then
        echo -e "${BLUE}Transaction ID:${NC} $TX_ID"
        echo -e "${BLUE}Explorer:${NC} https://testnet.nearblocks.io/txns/$TX_ID"
        echo ""
    fi

    # Display execution result if present
    echo -e "${BLUE}📋 Execution Result:${NC}"
    if echo "$TX2_OUTPUT"; then
        RESULT=$(echo "$TX2_OUTPUT" | tail -1)
        echo "  $RESULT"
    fi

    # Display logs
    echo ""
    echo -e "${BLUE}📊 Transaction Logs:${NC}"
    echo "$TX2_OUTPUT" | sed -n '/Function execution logs/,/Function execution return value/p' | grep -E "(Logs|execution_completed|random_number|Resources|Cost|Refund)" | sed 's/^/  /'
    echo ""

    echo "📍 Next steps:"
    echo "  • Worker: Watch for compilation and execution logs"
    echo "  • Contract: https://testnet.nearblocks.io/address/$CONTRACT_ID"
    echo ""
else
    echo -e "${RED}✗ Transaction 2 failed${NC}"
    echo "$TX2_OUTPUT"
    exit 1
fi

sleep 3

echo ""
echo -e "${BLUE}═══════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}Test 3/3: WASI P2 Execution (ai-ark)${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════════════════${NC}"
echo ""

echo "📦 Sending execution request..."
echo "  Repo: https://github.com/zavodil/ai-ark"
echo "  Target: wasm32-wasip2"
echo "  Input: AI prompt with encrypted secrets"
echo ""

echo -e "${YELLOW}⚠️  Note: This test requires WASI P2, HTTP access, and keystore-worker${NC}"
echo "   Make sure keystore-worker is running on port 8081"
echo ""

set +e  # Don't exit on error
TX2_OUTPUT=$(near contract call-function as-transaction \
  "$CONTRACT_ID" \
  request_execution \
  json-args '{
    "code_source": {
      "repo": "https://github.com/zavodil/ai-ark",
      "commit": "main",
      "build_target": "wasm32-wasip2"
    },
    "resource_limits": {
      "max_instructions": 10000000000,
      "max_memory_mb": 128,
      "max_execution_seconds": 60
    },
    "input_data": "{\"prompt\":\"What could the NEAR OutLayer project do? Be short\",\"history\":[{\"role\":\"user\",\"content\":\"Tell me about NEAR\"},{\"role\":\"assistant\",\"content\":\"NEAR is a Layer 1 blockchain...\"}],\"model_name\":\"fireworks::accounts/fireworks/models/gpt-oss-120b\",\"openai_endpoint\":\"https://api.near.ai/v1/chat/completions\",\"max_tokens\":16384}",
    "secrets_ref": {
        "profile": "default",
        "account_id": "zavodil2.testnet"
    }
  }' \
  prepaid-gas '300.0 Tgas' \
  attached-deposit '0.1 NEAR' \
  sign-as "$CALLER_ACCOUNT" \
  network-config testnet \
  $SIGN_METHOD \
  send 2>&1)
TX2_EXIT_CODE=$?
set -e

if [ $TX2_EXIT_CODE -ne 0 ]; then
    echo -e "${RED}✗ Transaction 2 command failed (exit code: $TX2_EXIT_CODE)${NC}"
    echo ""
    echo "Error output:"
    echo "$TX2_OUTPUT"
    echo ""
elif echo "$TX2_OUTPUT" | grep -q -E "(Transaction ID:|succeeded)"; then
    echo -e "${GREEN}✓ Transaction 2 sent successfully${NC}"
    echo ""

    # Extract and display transaction ID
    TX_ID=$(echo "$TX2_OUTPUT" | grep "Transaction ID:" | sed 's/.*Transaction ID: //' | awk '{print $1}')
    if [ -n "$TX_ID" ]; then
        echo -e "${BLUE}Transaction ID:${NC} $TX_ID"
        echo -e "${BLUE}Explorer:${NC} https://testnet.nearblocks.io/txns/$TX_ID"
        echo ""
    fi

    # Display execution result if present
    echo -e "${BLUE}📋 Execution Result:${NC}"
    if echo "$TX2_OUTPUT" | grep -q "answer"; then
        RESULT=$(echo "$TX2_OUTPUT" | grep "answer" | tail -1)
        echo "  $RESULT"
    else
        echo "  (Waiting for worker to execute...)"
    fi

    # Display logs
    echo ""
    echo -e "${BLUE}📊 Transaction Logs:${NC}"
    echo "$TX2_OUTPUT" | sed -n '/Function execution logs/,/Function execution return value/p' | grep -E "(Logs|execution_completed|execution_requested|answer|Resources|Cost|Refund)" | sed 's/^/  /'
    echo ""

    echo "📍 Next steps:"
    echo "  • Worker: Watch for compilation and execution logs"
    echo "  • Contract: https://testnet.nearblocks.io/address/$CONTRACT_ID"
    echo ""
else
    echo -e "${RED}✗ Transaction 2 status unclear${NC}"
    echo ""
    echo "Full output:"
    echo "$TX2_OUTPUT"
    echo ""
fi

echo ""
echo "═══════════════════════════════════════════════════════════"
echo -e "${GREEN}✅ Transaction test completed!${NC}"
echo "═══════════════════════════════════════════════════════════"
echo ""

echo "📋 Next steps:"
echo "  1. Monitor worker logs for execution progress"
echo "  2. Check contract state:"
echo "     near contract call-function as-read-only $CONTRACT_ID get_stats json-args '{}' network-config testnet now"
echo "  3. Verify results in NEAR Explorer"
echo "  4. For WASI P2 with encrypted secrets: see worker/test_transactions.txt"
echo ""
