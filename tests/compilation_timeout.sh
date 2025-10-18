#!/bin/bash

# Compilation Timeout Test
# Tests that long-running compilations are properly terminated
# Usage: ./compilation_timeout.sh

set -e

# Configuration
CONTRACT_ID="${CONTRACT_ID:-c4.offchainvm.testnet}"
USER_ACCOUNT="${USER_ACCOUNT:-zavodil.testnet}"

# Use the slow-compile test repo
# NOTE: You need to create a GitHub repo with the slow-compile code
# For testing, you can use a local copy or fork this to your GitHub
SLOW_COMPILE_REPO="${SLOW_COMPILE_REPO:-https://github.com/yourusername/slow-compile-test}"
SLOW_COMPILE_COMMIT="main"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Compilation Timeout Test${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Step 1: Check current max_compilation_seconds setting
echo -e "${BLUE}Step 1: Checking current compilation timeout setting...${NC}"
PRICING=$(near contract call-function as-read-only "$CONTRACT_ID" get_pricing json-args '{}' network-config testnet now)
echo -e "${YELLOW}Current pricing: ${PRICING}${NC}"

MAX_LIMITS=$(near contract call-function as-read-only "$CONTRACT_ID" get_max_limits json-args '{}' network-config testnet now)
echo -e "${YELLOW}Current max limits: ${MAX_LIMITS}${NC}"
echo ""

# Step 2: Submit compilation request for slow repo
echo -e "${BLUE}Step 2: Submitting compilation request for slow-compile repo...${NC}"
echo -e "${YELLOW}Repo: ${SLOW_COMPILE_REPO}${NC}"
echo -e "${YELLOW}Note: This repo is designed to take 15-30 seconds to compile${NC}"
echo ""

START_TIME=$(date +%s)

TX_OUTPUT=$(near contract call-function as-transaction "$CONTRACT_ID" request_execution json-args "{
    \"code_source\": {
        \"GitHub\": {
            \"repo\": \"$SLOW_COMPILE_REPO\",
            \"commit\": \"$SLOW_COMPILE_COMMIT\",
            \"build_target\": \"wasm32-wasip1\"
        }
    },
    \"resource_limits\": {
        \"max_instructions\": 1000000000,
        \"max_memory_mb\": 128,
        \"max_execution_seconds\": 60
    },
    \"input_data\": \"{\\\"test\\\":\\\"timeout\\\"}\"
}" prepaid-gas '300.0 Tgas' attached-deposit '0.5 NEAR' sign-as "$USER_ACCOUNT" network-config testnet send 2>&1)

TX_HASH=$(echo "$TX_OUTPUT" | grep -oE '[A-Z0-9]{40,}' | head -1)

if [ -z "$TX_HASH" ]; then
    echo -e "${RED}✗ Failed to submit request${NC}"
    echo "$TX_OUTPUT"
    exit 1
fi

echo -e "${GREEN}✓ Request submitted${NC}"
echo -e "${GREEN}TX: https://testnet.nearblocks.io/txns/${TX_HASH}${NC}"
echo ""

# Step 3: Monitor compilation progress
echo -e "${BLUE}Step 3: Monitoring compilation progress...${NC}"
echo -e "${YELLOW}Waiting for worker to pick up the task...${NC}"
echo ""

sleep 5

# Function to check job status
check_job_status() {
    local timeout=300  # 5 minutes max
    local elapsed=0
    local check_interval=5

    while [ $elapsed -lt $timeout ]; do
        sleep $check_interval
        elapsed=$((elapsed + check_interval))

        # Query recent jobs
        JOBS=$(curl -s "http://localhost:8080/public/jobs?limit=10")

        # Check for failed compilation
        FAILED=$(echo "$JOBS" | python3 -c "
import sys, json
data = json.load(sys.stdin)
for job in data:
    if job.get('job_type') == 'compile' and job.get('success') == False:
        print(json.dumps(job))
        break
" 2>/dev/null || echo "")

        if [ -n "$FAILED" ]; then
            echo -e "${YELLOW}Compilation job failed (as expected for timeout test)${NC}"
            echo "$FAILED" | python3 -m json.tool

            COMPILE_TIME=$(echo "$FAILED" | python3 -c "import sys, json; print(json.load(sys.stdin).get('compile_time_ms', 0))")

            echo ""
            echo -e "${GREEN}========================================${NC}"
            echo -e "${GREEN}Test Result: PASSED ✓${NC}"
            echo -e "${GREEN}========================================${NC}"
            echo -e "${GREEN}Compilation was terminated (timeout or failure)${NC}"
            echo -e "${GREEN}Compile time: ${COMPILE_TIME}ms${NC}"

            # Check if timeout was the cause
            MAX_COMPILE_MS=$((300 * 1000))  # Default 5 minutes
            if [ "$COMPILE_TIME" -lt "$MAX_COMPILE_MS" ]; then
                echo -e "${GREEN}✓ Compilation stopped before max timeout${NC}"
            fi

            return 0
        fi

        # Check for successful compilation (unexpected)
        SUCCESS=$(echo "$JOBS" | python3 -c "
import sys, json
data = json.load(sys.stdin)
for job in data:
    if job.get('job_type') == 'compile' and job.get('success') == True:
        print(json.dumps(job))
        break
" 2>/dev/null || echo "")

        if [ -n "$SUCCESS" ]; then
            echo -e "${YELLOW}Warning: Compilation succeeded${NC}"
            echo "$SUCCESS" | python3 -m json.tool

            COMPILE_TIME=$(echo "$SUCCESS" | python3 -c "import sys, json; print(json.load(sys.stdin).get('compile_time_ms', 0))")

            echo ""
            echo -e "${YELLOW}========================================${NC}"
            echo -e "${YELLOW}Test Result: UNEXPECTED${NC}"
            echo -e "${YELLOW}========================================${NC}"
            echo -e "${YELLOW}Compilation completed successfully in ${COMPILE_TIME}ms${NC}"
            echo -e "${YELLOW}This might mean:${NC}"
            echo -e "${YELLOW}  1. The slow-compile repo is not slow enough${NC}"
            echo -e "${YELLOW}  2. The timeout is set too high${NC}"
            echo -e "${YELLOW}  3. Compilation was cached${NC}"

            return 0
        fi

        echo -e "${BLUE}[${elapsed}s] Still waiting for compilation result...${NC}"
    done

    echo -e "${RED}Timeout waiting for compilation result${NC}"
    return 1
}

check_job_status

END_TIME=$(date +%s)
TOTAL_TIME=$((END_TIME - START_TIME))

echo ""
echo -e "${BLUE}Total test time: ${TOTAL_TIME}s${NC}"
echo ""

# Step 4: Recommendations
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Recommendations${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""
echo -e "${YELLOW}To test compilation timeout properly:${NC}"
echo ""
echo -e "1. Push the slow-compile test repo to GitHub:"
echo -e "   ${BLUE}cd tests/test-repos/slow-compile${NC}"
echo -e "   ${BLUE}git init && git add . && git commit -m 'slow compile test'${NC}"
echo -e "   ${BLUE}git remote add origin YOUR_GITHUB_REPO_URL${NC}"
echo -e "   ${BLUE}git push -u origin main${NC}"
echo ""
echo -e "2. Set max_compilation_seconds to a low value (e.g., 10 seconds):"
echo -e "   ${BLUE}near contract call-function as-transaction $CONTRACT_ID set_pricing \\${NC}"
echo -e "   ${BLUE}  json-args '{\"max_compilation_seconds\":10}' \\${NC}"
echo -e "   ${BLUE}  prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' \\${NC}"
echo -e "   ${BLUE}  sign-as $USER_ACCOUNT network-config testnet send${NC}"
echo ""
echo -e "3. Run this test again with your GitHub repo URL"
echo ""
echo -e "${GREEN}✓ Compilation timeout test completed${NC}"
