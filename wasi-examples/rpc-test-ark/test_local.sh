#!/bin/bash
# Test rpc-test-ark locally with OutLayer worker
#
# Prerequisites:
# 1. Coordinator running: cd coordinator && cargo run
# 2. Worker running with RPC_PROXY_ENABLED=true: cd worker && cargo run
#
# Usage:
#   ./test_local.sh [test_name] [account_id]
#
# Examples:
#   ./test_local.sh                    # Run all tests with default account
#   ./test_local.sh view_account       # Run only view_account test
#   ./test_local.sh all alice.testnet  # Run all tests for alice.testnet

set -e

# Configuration
COORDINATOR_URL="${COORDINATOR_URL:-http://localhost:8080}"
AUTH_TOKEN="${AUTH_TOKEN:-test-worker-token-123}"
WASM_FILE="$(dirname $0)/target/wasm32-wasip2/release/rpc-test-ark.wasm"

# Input parameters
TEST_NAME="${1:-all}"
ACCOUNT_ID="${2:-outlayer.testnet}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "=========================================="
echo "  RPC Test Ark - Local Test Runner"
echo "=========================================="
echo ""

# Check if WASM exists
if [ ! -f "$WASM_FILE" ]; then
    echo -e "${YELLOW}Building WASM...${NC}"
    cd "$(dirname $0)"
    cargo build --target wasm32-wasip2 --release
fi

# Calculate checksum
CHECKSUM=$(shasum -a 256 "$WASM_FILE" | cut -d' ' -f1)
echo "WASM checksum: $CHECKSUM"
echo "WASM size: $(ls -lh "$WASM_FILE" | awk '{print $5}')"
echo ""

# Check if coordinator is running
echo "Checking coordinator at $COORDINATOR_URL..."
if ! curl -s "$COORDINATOR_URL/health" > /dev/null 2>&1; then
    echo -e "${RED}ERROR: Coordinator not running at $COORDINATOR_URL${NC}"
    echo "Start it with: cd coordinator && cargo run"
    exit 1
fi
echo -e "${GREEN}Coordinator is running${NC}"
echo ""

# Upload WASM
echo "Uploading WASM to coordinator..."
UPLOAD_RESPONSE=$(curl -s -X POST "$COORDINATOR_URL/wasm/upload" \
    -H "Authorization: Bearer $AUTH_TOKEN" \
    -F "file=@$WASM_FILE" \
    -F "checksum=$CHECKSUM" 2>&1)

if echo "$UPLOAD_RESPONSE" | grep -q "error"; then
    echo -e "${YELLOW}Upload response: $UPLOAD_RESPONSE${NC}"
    echo "(This may be OK if WASM already exists)"
else
    echo -e "${GREEN}WASM uploaded successfully${NC}"
fi
echo ""

# Create input JSON
INPUT_JSON=$(cat <<EOF
{
  "test": "$TEST_NAME",
  "account_id": "$ACCOUNT_ID",
  "contract_id": "wrap.testnet",
  "method_name": "ft_metadata",
  "args_json": "{}"
}
EOF
)

echo "Test configuration:"
echo "  - Test: $TEST_NAME"
echo "  - Account: $ACCOUNT_ID"
echo ""

# Create execution task
echo "Creating execution task..."
REQUEST_ID=$RANDOM

TASK_RESPONSE=$(curl -s -X POST "$COORDINATOR_URL/tasks/create" \
    -H "Authorization: Bearer $AUTH_TOKEN" \
    -H "Content-Type: application/json" \
    -d "{
        \"request_id\": $REQUEST_ID,
        \"wasm_checksum\": \"$CHECKSUM\",
        \"input_data\": $(echo "$INPUT_JSON" | jq -c . | jq -Rs .),
        \"build_target\": \"wasm32-wasip2\",
        \"resource_limits\": {
            \"max_instructions\": 10000000000,
            \"max_memory_mb\": 128,
            \"max_execution_seconds\": 60
        }
    }")

echo "Task created: $TASK_RESPONSE"
echo ""

# Extract task ID if available
TASK_ID=$(echo "$TASK_RESPONSE" | jq -r '.task_id // .id // empty' 2>/dev/null)

if [ -n "$TASK_ID" ]; then
    echo "Waiting for worker to process task $TASK_ID..."
    echo "(Check worker logs for execution output)"
    echo ""

    # Poll for result
    for i in {1..30}; do
        sleep 2
        RESULT=$(curl -s "$COORDINATOR_URL/tasks/$TASK_ID" \
            -H "Authorization: Bearer $AUTH_TOKEN" 2>/dev/null)

        STATUS=$(echo "$RESULT" | jq -r '.status // empty' 2>/dev/null)

        if [ "$STATUS" = "completed" ]; then
            echo -e "${GREEN}Task completed!${NC}"
            echo ""
            echo "Result:"
            echo "$RESULT" | jq '.output // .result // .'
            exit 0
        elif [ "$STATUS" = "failed" ]; then
            echo -e "${RED}Task failed!${NC}"
            echo "$RESULT" | jq '.'
            exit 1
        fi

        echo "  ... waiting ($i/30)"
    done

    echo -e "${YELLOW}Timeout waiting for task completion${NC}"
    echo "Check worker logs manually"
else
    echo -e "${YELLOW}Could not extract task ID from response${NC}"
    echo "Check worker logs for execution output"
fi

echo ""
echo "=========================================="
echo "  Manual verification commands:"
echo "=========================================="
echo ""
echo "# Check worker logs:"
echo "  tail -f worker.log | grep -E '(RPC|rpc-test)'"
echo ""
echo "# Check task status:"
echo "  curl -s $COORDINATOR_URL/tasks/status -H 'Authorization: Bearer $AUTH_TOKEN'"
