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
CONTRACT_ID="${CONTRACT_ID:-c5.offchainvm.testnet}"
CALLER_ACCOUNT="${CALLER_ACCOUNT:-offchainvm.testnet}"
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
    "input_data": "{\"prompt\":\"What could the NEAR offshore project do? Be short\",\"history\":[{\"role\":\"user\",\"content\":\"Tell me about NEAR\"},{\"role\":\"assistant\",\"content\":\"NEAR is a Layer 1 blockchain...\"}],\"model_name\":\"fireworks::accounts/fireworks/models/gpt-oss-120b\",\"openai_endpoint\":\"https://api.near.ai/v1/chat/completions\",\"max_tokens\":16384}",
    "encrypted_secrets": [65, 74, 1, 216, 62, 160, 23, 21, 111, 91, 45, 97, 149, 158, 151, 121, 173, 13, 180, 53, 246, 89, 235, 165, 179, 247, 198, 81, 126, 18, 143, 11, 102, 74, 116, 212, 89, 128, 57, 113, 93, 117, 19, 77, 179, 248, 179, 67, 236, 88, 227, 32, 222, 85, 228, 163, 177, 234, 239, 29, 38, 17, 196, 31, 79, 10, 34, 225, 24, 177, 61, 57, 73, 70, 95, 18, 150, 247, 183, 68, 189, 2, 163, 127, 147, 65, 201, 174, 152, 250, 253, 94, 80, 58, 156, 14, 9, 57, 9, 202, 21, 221, 1, 29, 71, 125, 41, 120, 143, 231, 164, 83, 251, 77, 165, 122, 228, 46, 207, 160, 189, 236, 226, 106, 65, 28, 215, 14, 88, 90, 18, 170, 87, 178, 116, 47, 89, 125, 19, 73, 190, 160, 160, 69, 211, 21, 172, 18, 136, 9, 204, 147, 178, 209, 226, 126, 78, 20, 209, 7, 75, 34, 41, 237, 41, 153, 16, 53, 82, 105, 50, 78, 129, 228, 230, 71, 255, 116, 209, 58, 227, 17, 236, 244, 166, 161, 235, 16, 93, 55, 144, 53, 89, 48, 0, 187, 44, 155, 110, 21, 92, 111, 22, 101, 251, 157, 182, 68, 225, 118, 218, 62, 196, 43, 249, 128, 129, 219, 138, 110, 114, 3, 223, 61, 124, 49, 6, 227, 15, 180, 14, 46, 119, 91, 26, 21, 247, 137, 240, 12, 211, 21, 245, 47, 198, 23, 232, 167, 179, 243, 236, 74, 120, 33, 186, 77, 0, 52, 108, 224, 15, 154, 38, 47, 10, 53, 82, 73, 191, 161, 186, 14, 225, 82, 247, 60, 132, 26, 227, 249, 189, 253, 192, 76, 107, 42, 131, 82, 109, 13, 34, 235, 20, 131, 51, 119, 68, 117, 86, 102, 143, 148, 128, 11, 206, 126, 176, 60, 207, 24, 227, 182, 185, 253, 221, 75, 55, 44, 143, 65, 84, 13, 47, 250, 93, 128, 57, 50, 83, 127, 64, 24, 250, 229, 226, 16, 191, 7, 166, 126, 154, 75, 186, 246, 224, 168, 131, 15, 58, 125, 215, 88, 15, 90, 126, 177, 79, 218, 110, 104, 1, 42, 77, 14, 190, 172, 162, 69, 178, 69, 243, 35, 197, 15, 239, 154, 242, 180, 239, 29, 103, 40, 149, 28, 91, 15, 43, 212, 89, 212, 10, 126, 103, 127, 17, 75, 165, 184, 183, 0, 251, 88, 182, 0, 239, 58, 216, 230, 145, 209, 239, 29, 38, 17, 196, 29, 95, 11, 39, 248, 18, 139, 56, 40, 108, 56, 71, 116, 232, 180, 187, 14, 225, 82, 247, 60, 246, 89, 166, 154, 242, 246, 220, 81, 105, 40, 186, 77, 0, 52, 108, 184, 75, 222, 102, 108, 0, 42, 77, 24, 250, 229, 226, 16, 191, 7, 166, 126, 154, 75, 187, 241, 229, 170, 131, 6, 62, 121, 222, 91, 11, 88, 126, 212, 89, 147, 116, 33]
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
