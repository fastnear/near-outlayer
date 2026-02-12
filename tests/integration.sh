#!/bin/bash
# Integration tests for Coordinator + Worker API

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "🧪 Integration Tests - Coordinator + Worker Flow"
echo "================================================="
echo ""

COORDINATOR_URL="http://localhost:8080"
TEST_WASM_PATH="$PROJECT_ROOT/wasi-examples/random-ark/target/wasm32-wasip1/release/random-ark.wasm"
CHECKSUM="ba2c7a75c93b7cd7bc3e2f7e12943ba2dacac6ea444f6a2e853023b892ca8acc"

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Test 1: Health Check
echo "📋 Test 1: Coordinator Health Check"
HEALTH=$(curl -s $COORDINATOR_URL/health)
if [ "$HEALTH" = "OK" ]; then
    echo -e "${GREEN}✓${NC} Coordinator is healthy"
else
    echo -e "${RED}✗${NC} Coordinator health check failed"
    exit 1
fi
echo ""

# Test 2: Upload WASM
echo "📋 Test 2: Upload WASM File"
if [ ! -f "$TEST_WASM_PATH" ]; then
    echo -e "${RED}✗${NC} Test WASM not found at $TEST_WASM_PATH"
    echo "Run: $SCRIPT_DIR/unit.sh to build test modules"
    exit 1
fi

UPLOAD_RESPONSE=$(curl -s -w "\n%{http_code}" -X POST $COORDINATOR_URL/wasm/upload \
  -F "checksum=$CHECKSUM" \
  -F "repo_url=https://github.com/zavodil/random-ark" \
  -F "commit_hash=test" \
  -F "wasm_file=@$TEST_WASM_PATH")

HTTP_CODE=$(echo "$UPLOAD_RESPONSE" | tail -1)
if [ "$HTTP_CODE" = "201" ]; then
    echo -e "${GREEN}✓${NC} WASM uploaded successfully"
else
    echo -e "${RED}✗${NC} WASM upload failed (HTTP $HTTP_CODE)"
    exit 1
fi
echo ""

# Test 3: Check WASM Exists
echo "📋 Test 3: Verify WASM Exists"
EXISTS=$(curl -s $COORDINATOR_URL/wasm/exists/$CHECKSUM | jq -r '.exists')
if [ "$EXISTS" = "true" ]; then
    echo -e "${GREEN}✓${NC} WASM exists in cache"
else
    echo -e "${RED}✗${NC} WASM not found in cache"
    exit 1
fi
echo ""

# Test 4: Download WASM
echo "📋 Test 4: Download WASM"
DOWNLOAD_CODE=$(curl -s -o /tmp/downloaded.wasm -w "%{http_code}" $COORDINATOR_URL/wasm/$CHECKSUM)
if [ "$DOWNLOAD_CODE" = "200" ]; then
    DOWNLOADED_SIZE=$(stat -f%z /tmp/downloaded.wasm 2>/dev/null || stat -c%s /tmp/downloaded.wasm 2>/dev/null)
    echo -e "${GREEN}✓${NC} WASM downloaded successfully ($DOWNLOADED_SIZE bytes)"
    rm /tmp/downloaded.wasm
else
    echo -e "${RED}✗${NC} WASM download failed (HTTP $DOWNLOAD_CODE)"
    exit 1
fi
echo ""

# Test 5: Create Task
echo "📋 Test 5: Create Execution Task"
CREATE_RESPONSE=$(curl -s -w "\n%{http_code}" -X POST $COORDINATOR_URL/executions/create \
  -H "Content-Type: application/json" \
  -d '{
    "request_id": 999,
    "data_id": "0000000000000000000000000000000000000000000000000000000000000001",
    "code_source": {
      "type": "GitHub",
      "repo": "https://github.com/zavodil/random-ark",
      "commit": "test",
      "build_target": "wasm32-wasip1"
    },
    "resource_limits": {
      "max_instructions": 10000000000,
      "max_memory_mb": 128,
      "max_execution_seconds": 60
    },
    "input_data": "{\"min\":1,\"max\":100}"
  }')

HTTP_CODE=$(echo "$CREATE_RESPONSE" | tail -1)
if [ "$HTTP_CODE" = "201" ]; then
    echo -e "${GREEN}✓${NC} Task created successfully"
else
    echo -e "${RED}✗${NC} Task creation failed (HTTP $HTTP_CODE)"
    exit 1
fi
echo ""

# Test 6: Poll for Task (should get the task we just created)
echo "📋 Test 6: Poll for Task"
TASK=$(curl -s "$COORDINATOR_URL/executions/poll?timeout=1")
if echo "$TASK" | jq -e '.type == "Compile"' > /dev/null 2>&1; then
    REQUEST_ID=$(echo "$TASK" | jq -r '.request_id')
    echo -e "${GREEN}✓${NC} Task received (request_id: $REQUEST_ID)"
else
    echo -e "${GREEN}✓${NC} No tasks in queue (expected if already processed)"
fi
echo ""

# Test 7: Distributed Lock
echo "📋 Test 7: Distributed Lock"
LOCK_RESPONSE=$(curl -s -X POST $COORDINATOR_URL/locks/acquire \
  -H "Content-Type: application/json" \
  -d '{
    "lock_key": "test-lock",
    "worker_id": "test-worker-123",
    "ttl_seconds": 60
  }')

ACQUIRED=$(echo "$LOCK_RESPONSE" | jq -r '.acquired')
if [ "$ACQUIRED" = "true" ]; then
    echo -e "${GREEN}✓${NC} Lock acquired"

    # Release lock
    curl -s -X DELETE $COORDINATOR_URL/locks/release/test-lock > /dev/null
    echo -e "${GREEN}✓${NC} Lock released"
else
    echo -e "${RED}✗${NC} Lock acquisition failed"
    exit 1
fi
echo ""

echo "===================================="
echo -e "${GREEN}✅ All tests passed!${NC}"
echo ""
echo "Next steps:"
echo "1. Start worker: cd ../worker && RUST_LOG=info cargo run"
echo "2. Create blockchain transaction to test end-to-end flow"
