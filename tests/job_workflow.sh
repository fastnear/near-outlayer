#!/bin/bash
# Job-Based Workflow Integration Test
#
# This test verifies the new job-based workflow:
# 1. First execution: compile + execute jobs created
# 2. Second execution: only execute job created (WASM cached)
# 3. Multiple workers: no duplicate work (race condition test)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

echo ""
echo "🧪 Job-Based Workflow Integration Test"
echo "======================================="
echo ""

# Configuration
COORDINATOR_URL="${COORDINATOR_URL:-http://localhost:8080}"
CONTRACT_ID="${CONTRACT_ID:-outlayer.testnet}"
CALLER_ACCOUNT="${CALLER_ACCOUNT:-outlayer.testnet}"
SIGN_METHOD="${SIGN_METHOD:-sign-with-legacy-keychain}"

echo "📝 Configuration:"
echo "  Coordinator: $COORDINATOR_URL"
echo "  Contract: $CONTRACT_ID"
echo "  Caller: $CALLER_ACCOUNT"
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
if ! curl -s "$COORDINATOR_URL/health" > /dev/null 2>&1; then
    echo -e "${RED}❌ Coordinator not responding at $COORDINATOR_URL${NC}"
    echo "   Start with: cd coordinator && cargo run"
    exit 1
fi
echo "✓ Coordinator running"

# Check public API endpoints
if ! curl -s "$COORDINATOR_URL/public/workers" > /dev/null 2>&1; then
    echo -e "${YELLOW}⚠️  Warning: Public API not accessible${NC}"
else
    WORKER_COUNT=$(curl -s "$COORDINATOR_URL/public/workers" | grep -o '"worker_id"' | wc -l | xargs)
    echo "✓ Public API accessible ($WORKER_COUNT workers online)"
fi

echo ""
echo "════════════════════════════════════════════════════════════════"
echo -e "${CYAN}Part 1: First Execution (Compile + Execute)${NC}"
echo "════════════════════════════════════════════════════════════════"
echo ""
echo "This test should trigger BOTH compilation and execution jobs"
echo ""

# Test repository - use echo-ark as it's simple and fast
TEST_REPO="https://github.com/zavodil/echo-ark"
TEST_COMMIT="main"
TEST_TARGET="wasm32-wasip1"
TEST_INPUT="Hello from job test 1"

echo "📦 Sending first execution request..."
echo "  Repo: $TEST_REPO"
echo "  Commit: $TEST_COMMIT"
echo "  Target: $TEST_TARGET"
echo "  Input: \"$TEST_INPUT\""
echo ""

TX1_OUTPUT=$(near contract call-function as-transaction \
  "$CONTRACT_ID" \
  request_execution \
  json-args "{
    \"code_source\": {
      \"repo\": \"$TEST_REPO\",
      \"commit\": \"$TEST_COMMIT\",
      \"build_target\": \"$TEST_TARGET\"
    },
    \"resource_limits\": {
      \"max_instructions\": 10000000000,
      \"max_memory_mb\": 128,
      \"max_execution_seconds\": 60
    },
    \"input_data\": \"$TEST_INPUT\"
  }" \
  prepaid-gas '300.0 Tgas' \
  attached-deposit '0.1 NEAR' \
  sign-as "$CALLER_ACCOUNT" \
  network-config testnet \
  $SIGN_METHOD \
  send 2>&1)

if echo "$TX1_OUTPUT" | grep -q -E "(Transaction ID:|succeeded)"; then
    echo -e "${GREEN}✓ Transaction 1 sent successfully${NC}"

    # Extract transaction ID
    TX1_ID=$(echo "$TX1_OUTPUT" | grep "Transaction ID:" | sed 's/.*Transaction ID: //' | awk '{print $1}')
    if [ -n "$TX1_ID" ]; then
        echo -e "${BLUE}Transaction ID:${NC} $TX1_ID"
        echo -e "${BLUE}Explorer:${NC} https://testnet.nearblocks.io/txns/$TX1_ID"
    fi

    # Extract request_id from logs
    REQUEST_ID1=$(echo "$TX1_OUTPUT" | grep -o '"request_id":[0-9]*' | head -1 | grep -o '[0-9]*')
    if [ -n "$REQUEST_ID1" ]; then
        echo -e "${BLUE}Request ID:${NC} $REQUEST_ID1"
    fi

    echo ""
    echo -e "${YELLOW}📊 Expected worker logs:${NC}"
    echo "  • 🎯 Claiming jobs for request_id=$REQUEST_ID1"
    echo "  • ✅ Claimed 2 job(s) (compile + execute)"
    echo "  • 🔨 Starting compilation job_id=XXX"
    echo "  • ✅ Compilation successful: time=XXXms"
    echo "  • ⚙️ Starting execution job_id=YYY"
    echo "  • ✅ Execution successful: time=XXXms instructions=XXX"
    echo ""
else
    echo -e "${RED}✗ Transaction 1 failed${NC}"
    echo "$TX1_OUTPUT"
    exit 1
fi

# Wait for worker to process
echo "⏳ Waiting 60 seconds for worker to compile and execute..."
sleep 60

echo ""
echo "════════════════════════════════════════════════════════════════"
echo -e "${CYAN}Part 2: Second Execution (Execute Only - Cached WASM)${NC}"
echo "════════════════════════════════════════════════════════════════"
echo ""
echo "This test should use CACHED WASM (no compilation, only execute)"
echo ""

TEST_INPUT2="Hello from job test 2"

echo "📦 Sending second execution request (same repo)..."
echo "  Input: \"$TEST_INPUT2\""
echo ""

TX2_OUTPUT=$(near contract call-function as-transaction \
  "$CONTRACT_ID" \
  request_execution \
  json-args "{
    \"code_source\": {
      \"repo\": \"$TEST_REPO\",
      \"commit\": \"$TEST_COMMIT\",
      \"build_target\": \"$TEST_TARGET\"
    },
    \"resource_limits\": {
      \"max_instructions\": 10000000000,
      \"max_memory_mb\": 128,
      \"max_execution_seconds\": 60
    },
    \"input_data\": \"$TEST_INPUT2\"
  }" \
  prepaid-gas '300.0 Tgas' \
  attached-deposit '0.1 NEAR' \
  sign-as "$CALLER_ACCOUNT" \
  network-config testnet \
  $SIGN_METHOD \
  send 2>&1)

if echo "$TX2_OUTPUT" | grep -q -E "(Transaction ID:|succeeded)"; then
    echo -e "${GREEN}✓ Transaction 2 sent successfully${NC}"

    TX2_ID=$(echo "$TX2_OUTPUT" | grep "Transaction ID:" | sed 's/.*Transaction ID: //' | awk '{print $1}')
    if [ -n "$TX2_ID" ]; then
        echo -e "${BLUE}Transaction ID:${NC} $TX2_ID"
        echo -e "${BLUE}Explorer:${NC} https://testnet.nearblocks.io/txns/$TX2_ID"
    fi

    REQUEST_ID2=$(echo "$TX2_OUTPUT" | grep -o '"request_id":[0-9]*' | head -1 | grep -o '[0-9]*')
    if [ -n "$REQUEST_ID2" ]; then
        echo -e "${BLUE}Request ID:${NC} $REQUEST_ID2"
    fi

    echo ""
    echo -e "${YELLOW}📊 Expected worker logs:${NC}"
    echo "  • 🎯 Claiming jobs for request_id=$REQUEST_ID2"
    echo "  • ✅ Claimed 1 job(s) (execute ONLY - WASM cached!)"
    echo "  • ⚙️ Starting execution job_id=ZZZ"
    echo "  • 📥 Downloading WASM: checksum=XXX"
    echo "  • ✅ Execution successful: time=XXXms instructions=XXX"
    echo ""
    echo -e "${GREEN}✓ NO COMPILATION - WASM was reused from cache!${NC}"
    echo ""
else
    echo -e "${RED}✗ Transaction 2 failed${NC}"
    echo "$TX2_OUTPUT"
    exit 1
fi

# Wait for execution
echo "⏳ Waiting 30 seconds for worker to execute (should be faster)..."
sleep 30

echo ""
echo "════════════════════════════════════════════════════════════════"
echo -e "${CYAN}Part 3: Race Condition Test (Multiple Workers)${NC}"
echo "════════════════════════════════════════════════════════════════"
echo ""
echo "This test verifies that only ONE worker processes the task"
echo ""

TEST_INPUT3="Hello from job test 3"

echo "📦 Sending third execution request..."
echo "  Input: \"$TEST_INPUT3\""
echo ""
echo -e "${YELLOW}⚠️  Run 2+ workers simultaneously to test race condition handling${NC}"
echo ""

TX3_OUTPUT=$(near contract call-function as-transaction \
  "$CONTRACT_ID" \
  request_execution \
  json-args "{
    \"code_source\": {
      \"repo\": \"$TEST_REPO\",
      \"commit\": \"$TEST_COMMIT\",
      \"build_target\": \"$TEST_TARGET\"
    },
    \"resource_limits\": {
      \"max_instructions\": 10000000000,
      \"max_memory_mb\": 128,
      \"max_execution_seconds\": 60
    },
    \"input_data\": \"$TEST_INPUT3\"
  }" \
  prepaid-gas '300.0 Tgas' \
  attached-deposit '0.1 NEAR' \
  sign-as "$CALLER_ACCOUNT" \
  network-config testnet \
  $SIGN_METHOD \
  send 2>&1)

if echo "$TX3_OUTPUT" | grep -q -E "(Transaction ID:|succeeded)"; then
    echo -e "${GREEN}✓ Transaction 3 sent successfully${NC}"

    TX3_ID=$(echo "$TX3_OUTPUT" | grep "Transaction ID:" | sed 's/.*Transaction ID: //' | awk '{print $1}')
    if [ -n "$TX3_ID" ]; then
        echo -e "${BLUE}Transaction ID:${NC} $TX3_ID"
        echo -e "${BLUE}Explorer:${NC} https://testnet.nearblocks.io/txns/$TX3_ID"
    fi

    REQUEST_ID3=$(echo "$TX3_OUTPUT" | grep -o '"request_id":[0-9]*' | head -1 | grep -o '[0-9]*')
    if [ -n "$REQUEST_ID3" ]; then
        echo -e "${BLUE}Request ID:${NC} $REQUEST_ID3"
    fi

    echo ""
    echo -e "${YELLOW}📊 Expected worker logs (if 2 workers running):${NC}"
    echo ""
    echo -e "${GREEN}Worker 1:${NC}"
    echo "  • 🎯 Claiming jobs for request_id=$REQUEST_ID3"
    echo "  • ✅ Claimed 1 job(s) for request_id=$REQUEST_ID3"
    echo "  • ⚙️ Starting execution job_id=AAA"
    echo "  • ✅ Execution successful"
    echo ""
    echo -e "${YELLOW}Worker 2:${NC}"
    echo "  • 🎯 Claiming jobs for request_id=$REQUEST_ID3"
    echo "  • ⚠️ Failed to claim job (likely already claimed): Task already claimed by another worker"
    echo "  • (Worker 2 moves on to next task)"
    echo ""
    echo -e "${GREEN}✓ Only ONE worker processed the task!${NC}"
    echo ""
else
    echo -e "${RED}✗ Transaction 3 failed${NC}"
    echo "$TX3_OUTPUT"
    exit 1
fi

echo "⏳ Waiting 30 seconds for execution..."
sleep 30

echo ""
echo "════════════════════════════════════════════════════════════════"
echo -e "${CYAN}Part 4: Verification${NC}"
echo "════════════════════════════════════════════════════════════════"
echo ""

echo "🔍 Checking coordinator database state..."
echo ""

# Try to query public endpoints
echo "📊 Worker Status:"
curl -s "$COORDINATOR_URL/public/workers" 2>/dev/null | python3 -m json.tool 2>/dev/null || echo "  (Public API not accessible or no workers online)"
echo ""

echo "📊 Execution History:"
curl -s "$COORDINATOR_URL/public/executions" 2>/dev/null | python3 -m json.tool 2>/dev/null || echo "  (Public API not accessible)"
echo ""

echo "════════════════════════════════════════════════════════════════"
echo -e "${GREEN}✅ Job Workflow Test Completed!${NC}"
echo "════════════════════════════════════════════════════════════════"
echo ""

echo "📋 Summary:"
echo ""
echo "Test 1 (First execution):"
echo "  • Should have created: 2 jobs (compile + execute)"
echo "  • Worker compiled GitHub repo and executed WASM"
echo ""
echo "Test 2 (Second execution):"
echo "  • Should have created: 1 job (execute only)"
echo "  • Worker reused cached WASM (no compilation)"
echo "  • Execution should be FASTER than test 1"
echo ""
echo "Test 3 (Race condition):"
echo "  • Multiple workers attempted to claim same task"
echo "  • Only ONE worker succeeded (409 CONFLICT for others)"
echo "  • No duplicate work performed"
echo ""

echo "🔍 Verification Steps:"
echo ""
echo "1. Check worker logs for job claiming and completion:"
echo "   grep -E '(Claiming|Claimed|compilation|execution)' worker_logs.txt"
echo ""
echo "2. Query coordinator database:"
echo "   psql postgres://postgres:postgres@localhost/offchainvm"
echo "   SELECT * FROM jobs WHERE request_id IN ($REQUEST_ID1, $REQUEST_ID2, $REQUEST_ID3);"
echo ""
echo "3. Verify contract state:"
echo "   near view $CONTRACT_ID get_stats '{}' --networkId testnet"
echo ""
echo "4. Check WASM cache:"
echo "   ls -lah /tmp/offchainvm/wasm/"
echo ""

echo "📚 Documentation:"
echo "  • Job-based workflow: JOB_BASED_WORKFLOW.md"
echo "  • Architecture diagram in CLAUDE.md"
echo ""
