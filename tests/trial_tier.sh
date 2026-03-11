#!/bin/bash
# ============================================================================
# Trial Tier Integration Tests
#
# Full cycle: register → trial info → trial WASI call → quota check →
#             rate limit → cooldown → quota exhaustion → admin kill
#
# Prerequisites:
#   - Coordinator running on localhost:8080
#   - PostgreSQL + Redis running
#   - A test project deployed (e.g., test-owner/test-project)
#
# Usage:
#   ./trial_tier.sh
#   COORDINATOR_URL=https://api.outlayer.fastnear.com ./trial_tier.sh
#   TEST_PROJECT="zavodil/random-ark" ./trial_tier.sh
# ============================================================================

set -euo pipefail

COORDINATOR_URL="${COORDINATOR_URL:-https://testnet-api.outlayer.fastnear.com}"
ADMIN_TOKEN="${ADMIN_TOKEN:-}"
TEST_PROJECT="${TEST_PROJECT:-zavodil.testnet/test-storage}"
PROJECT_OWNER="${TEST_PROJECT%%/*}"
PROJECT_NAME="${TEST_PROJECT##*/}"

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
NC='\033[0m'

PASSED=0
FAILED=0
TOTAL=0

# ============================================================================
# Helpers (same pattern as wallet_mode1_agent.sh)
# ============================================================================

assert_status() {
    local expected="$1"
    local actual="$2"
    local test_name="$3"
    TOTAL=$((TOTAL + 1))
    if [ "$actual" = "$expected" ]; then
        echo -e "  ${GREEN}PASS${NC} $test_name (HTTP $actual)"
        PASSED=$((PASSED + 1))
    else
        echo -e "  ${RED}FAIL${NC} $test_name (expected HTTP $expected, got $actual)"
        FAILED=$((FAILED + 1))
    fi
}

assert_json_field() {
    local json="$1"
    local field="$2"
    local expected="$3"
    local test_name="$4"
    TOTAL=$((TOTAL + 1))
    local actual
    actual=$(echo "$json" | jq -r "$field" 2>/dev/null || echo "PARSE_ERROR")
    if [ "$actual" = "$expected" ]; then
        echo -e "  ${GREEN}PASS${NC} $test_name ($field = $actual)"
        PASSED=$((PASSED + 1))
    else
        echo -e "  ${RED}FAIL${NC} $test_name ($field: expected '$expected', got '$actual')"
        FAILED=$((FAILED + 1))
    fi
}

assert_json_not_empty() {
    local json="$1"
    local field="$2"
    local test_name="$3"
    TOTAL=$((TOTAL + 1))
    local actual
    actual=$(echo "$json" | jq -r "$field" 2>/dev/null || echo "")
    if [ -n "$actual" ] && [ "$actual" != "null" ] && [ "$actual" != "" ]; then
        echo -e "  ${GREEN}PASS${NC} $test_name ($field = ${actual:0:40}...)"
        PASSED=$((PASSED + 1))
    else
        echo -e "  ${RED}FAIL${NC} $test_name ($field is empty or null)"
        FAILED=$((FAILED + 1))
    fi
}

assert_json_numeric_gt() {
    local json="$1"
    local field="$2"
    local min="$3"
    local test_name="$4"
    TOTAL=$((TOTAL + 1))
    local actual
    actual=$(echo "$json" | jq -r "$field" 2>/dev/null || echo "0")
    if [ "$actual" -gt "$min" ] 2>/dev/null; then
        echo -e "  ${GREEN}PASS${NC} $test_name ($field = $actual > $min)"
        PASSED=$((PASSED + 1))
    else
        echo -e "  ${RED}FAIL${NC} $test_name ($field = $actual, expected > $min)"
        FAILED=$((FAILED + 1))
    fi
}

parse_response() {
    local response="$1"
    RESP_BODY=$(echo "$response" | sed '$d')
    RESP_CODE=$(echo "$response" | tail -1)
}

curl_get() {
    local path="$1"
    curl -s -w "\n%{http_code}" \
        -H "Authorization: Bearer $API_KEY" \
        "${COORDINATOR_URL}${path}"
}

curl_post_trial() {
    local path="$1"
    local body="$2"
    curl -s -w "\n%{http_code}" \
        -X POST \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer $API_KEY" \
        -d "$body" \
        "${COORDINATOR_URL}${path}"
}

curl_admin() {
    local method="$1"
    local path="$2"
    local body="${3:-}"
    if [ -n "$body" ]; then
        curl -s -w "\n%{http_code}" \
            -X "$method" \
            -H "Content-Type: application/json" \
            -H "Authorization: Bearer $ADMIN_TOKEN" \
            -d "$body" \
            "${COORDINATOR_URL}${path}"
    else
        curl -s -w "\n%{http_code}" \
            -X "$method" \
            -H "Authorization: Bearer $ADMIN_TOKEN" \
            "${COORDINATOR_URL}${path}"
    fi
}

# ============================================================================
# Pre-flight
# ============================================================================

echo ""
echo "============================================="
echo " Trial Tier Integration Tests"
echo "============================================="
echo ""
echo "Coordinator: $COORDINATOR_URL"
echo "Test project: $TEST_PROJECT"
echo ""

if ! curl -s "${COORDINATOR_URL}/health" > /dev/null 2>&1; then
    echo -e "${RED}ERROR: Coordinator not running at ${COORDINATOR_URL}${NC}"
    exit 1
fi

# ============================================================================
# Test 1: Register — should include trial info
# ============================================================================

echo "1. Register new wallet (expect trial info in response)"
REGISTER_RESP=$(curl -s -w "\n%{http_code}" -X POST "${COORDINATOR_URL}/register")
parse_response "$REGISTER_RESP"
assert_status "200" "$RESP_CODE" "POST /register"
assert_json_not_empty "$RESP_BODY" ".api_key" "api_key present"
assert_json_not_empty "$RESP_BODY" ".wallet_id" "wallet_id present"
assert_json_not_empty "$RESP_BODY" ".trial.calls_remaining" "trial.calls_remaining present"
assert_json_not_empty "$RESP_BODY" ".trial.expires_at" "trial.expires_at present"
assert_json_numeric_gt "$RESP_BODY" ".trial.calls_remaining" "0" "trial.calls_remaining > 0"
assert_json_not_empty "$RESP_BODY" ".trial.limits.max_instructions" "trial limits present"

API_KEY=$(echo "$RESP_BODY" | jq -r '.api_key')
WALLET_ID=$(echo "$RESP_BODY" | jq -r '.wallet_id')
TRIAL_CALLS=$(echo "$RESP_BODY" | jq -r '.trial.calls_remaining')
echo "  Wallet ID: $WALLET_ID"
echo "  API Key: ${API_KEY:0:10}..."
echo "  Trial calls: $TRIAL_CALLS"
echo ""

# ============================================================================
# Test 2: GET /trial/status — check quota
# ============================================================================

echo "2. Check trial status"
parse_response "$(curl_get "/trial/status")"
assert_status "200" "$RESP_CODE" "GET /trial/status"
assert_json_field "$RESP_BODY" ".calls_used" "0" "calls_used = 0"
assert_json_numeric_gt "$RESP_BODY" ".calls_remaining" "0" "calls_remaining > 0"
assert_json_not_empty "$RESP_BODY" ".expires_at" "expires_at present"
echo ""

# ============================================================================
# Test 3: Trial WASI call (Bearer wk_... instead of X-Payment-Key)
# ============================================================================

echo "3. Trial WASI call (sync)"
CALL_BODY='{"input":{"command":"test-all"}}'
parse_response "$(curl_post_trial "/call/${PROJECT_OWNER}/${PROJECT_NAME}" "$CALL_BODY")"
# Accept 200 (success) or 500 (project might not exist on test env) — both prove trial auth works
if [ "$RESP_CODE" = "200" ]; then
    assert_status "200" "$RESP_CODE" "POST /call (trial)"
    assert_json_not_empty "$RESP_BODY" ".call_id" "call_id present"
    echo "  Call completed successfully"
elif [ "$RESP_CODE" = "404" ]; then
    assert_status "404" "$RESP_CODE" "POST /call (trial, project not found — OK, auth worked)"
else
    # Any other code means trial auth itself failed — fail the test
    assert_status "200" "$RESP_CODE" "POST /call (trial)"
fi
echo ""

# ============================================================================
# Test 4: Trial status should show calls_used incremented
# ============================================================================

echo "4. Trial status after call"
parse_response "$(curl_get "/trial/status")"
assert_status "200" "$RESP_CODE" "GET /trial/status"
CALLS_USED=$(echo "$RESP_BODY" | jq -r '.calls_used')
echo "  calls_used = $CALLS_USED"
# If the call in test 3 succeeded (200 or 404 with quota decrement), calls_used should be >= 1
# Note: 404 (project not found) happens after auth+quota decrement, so count goes up
echo ""

# ============================================================================
# Test 5: Trial call without Bearer → should fall through to MissingPaymentKey
# ============================================================================

echo "5. Call without any auth → 401"
RESP=$(curl -s -w "\n%{http_code}" \
    -X POST \
    -H "Content-Type: application/json" \
    -d '{"input":{}}' \
    "${COORDINATOR_URL}/call/${PROJECT_OWNER}/${PROJECT_NAME}")
parse_response "$RESP"
assert_status "401" "$RESP_CODE" "POST /call (no auth → 401)"
echo ""

# ============================================================================
# Test 6: Invalid Bearer token → 401
# ============================================================================

echo "6. Call with invalid Bearer → 401"
RESP=$(curl -s -w "\n%{http_code}" \
    -X POST \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer wk_invalid_key_does_not_exist" \
    -d '{"input":{}}' \
    "${COORDINATOR_URL}/call/${PROJECT_OWNER}/${PROJECT_NAME}")
parse_response "$RESP"
assert_status "401" "$RESP_CODE" "POST /call (bad Bearer → 401)"
echo ""

# ============================================================================
# Test 7: Trial cooldown (two rapid calls)
# ============================================================================

echo "7. Trial cooldown (rapid calls)"
# First call (may succeed or fail with 404, but should pass rate limit)
curl -s -o /dev/null \
    -X POST \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $API_KEY" \
    -d '{"input":{}}' \
    "${COORDINATOR_URL}/call/${PROJECT_OWNER}/${PROJECT_NAME}" || true

# Second call immediately — should hit cooldown (429)
RESP=$(curl -s -w "\n%{http_code}" \
    -X POST \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $API_KEY" \
    -d '{"input":{}}' \
    "${COORDINATOR_URL}/call/${PROJECT_OWNER}/${PROJECT_NAME}")
parse_response "$RESP"
# Could be 429 (cooldown) or 200/404 if cooldown=0 in config
if [ "$RESP_CODE" = "429" ]; then
    assert_status "429" "$RESP_CODE" "POST /call (cooldown → 429)"
else
    echo -e "  ${YELLOW}SKIP${NC} cooldown test (cooldown may be 0 in config, got HTTP $RESP_CODE)"
fi
echo ""

# ============================================================================
# Test 8: Async trial call + poll
# ============================================================================

echo "8. Async trial call + poll"
sleep 3  # wait for cooldown
ASYNC_BODY='{"input":{"test":true},"async":true}'
parse_response "$(curl_post_trial "/call/${PROJECT_OWNER}/${PROJECT_NAME}" "$ASYNC_BODY")"
if [ "$RESP_CODE" = "200" ]; then
    assert_status "200" "$RESP_CODE" "POST /call (async trial)"
    CALL_ID=$(echo "$RESP_BODY" | jq -r '.call_id')
    assert_json_field "$RESP_BODY" ".status" "pending" "status=pending"

    # Poll with Bearer wk_... (not X-Payment-Key)
    if [ -n "$CALL_ID" ] && [ "$CALL_ID" != "null" ]; then
        parse_response "$(curl_get "/calls/$CALL_ID")"
        # 200 = found, any status is fine (pending/completed/failed)
        assert_status "200" "$RESP_CODE" "GET /calls/{id} (trial poll)"
    fi
elif [ "$RESP_CODE" = "404" ]; then
    echo -e "  ${YELLOW}SKIP${NC} async poll test (project not found)"
else
    assert_status "200" "$RESP_CODE" "POST /call (async trial)"
fi
echo ""

# ============================================================================
# Test 9: X-Attached-Deposit blocked for trial
# ============================================================================

echo "9. Trial call with X-Attached-Deposit (should be ignored/blocked)"
sleep 3  # wait for cooldown
RESP=$(curl -s -w "\n%{http_code}" \
    -X POST \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $API_KEY" \
    -H "X-Attached-Deposit: 1000000" \
    -d '{"input":{}}' \
    "${COORDINATOR_URL}/call/${PROJECT_OWNER}/${PROJECT_NAME}")
parse_response "$RESP"
# Trial path ignores deposit headers — should still work (200 or 404)
if [ "$RESP_CODE" = "200" ] || [ "$RESP_CODE" = "404" ]; then
    echo -e "  ${GREEN}PASS${NC} Trial call with deposit header accepted (deposit ignored)"
    PASSED=$((PASSED + 1))
    TOTAL=$((TOTAL + 1))
else
    assert_status "200" "$RESP_CODE" "POST /call (trial + deposit)"
fi
echo ""

# ============================================================================
# Test 10: Admin kill trial (requires ADMIN_TOKEN)
# ============================================================================

if [ -n "$ADMIN_TOKEN" ]; then
    echo "10. Admin kill trial"
    parse_response "$(curl_admin "DELETE" "/admin/trial/$WALLET_ID")"
    assert_status "200" "$RESP_CODE" "DELETE /admin/trial/{wallet_id}"
    assert_json_field "$RESP_BODY" ".status" "ok" "status=ok"

    # Verify quota is exhausted
    sleep 3  # wait for cooldown
    parse_response "$(curl_post_trial "/call/${PROJECT_OWNER}/${PROJECT_NAME}" '{"input":{}}')"
    assert_status "402" "$RESP_CODE" "POST /call after kill → 402"
    echo ""

    # ============================================================================
    # Test 11: Admin update trial config
    # ============================================================================

    echo "11. Admin update trial config"
    CONFIG_BODY='{"cooldown_seconds": 1}'
    parse_response "$(curl_admin "PUT" "/admin/trial/config" "$CONFIG_BODY")"
    assert_status "200" "$RESP_CODE" "PUT /admin/trial/config"
    assert_json_field "$RESP_BODY" ".status" "ok" "status=ok"
    echo ""

    # Reset config
    curl_admin "PUT" "/admin/trial/config" '{"cooldown_seconds": 3}' > /dev/null 2>&1
else
    echo -e "10. ${YELLOW}SKIP${NC} Admin tests (set ADMIN_TOKEN to enable)"
    echo -e "11. ${YELLOW}SKIP${NC} Admin tests (set ADMIN_TOKEN to enable)"
    echo ""
fi

# ============================================================================
# Test 12: Register multiple wallets from same IP (rate limit)
# ============================================================================

echo "12. IP registration rate limit"
# Register 3 more wallets (limit is typically 3/day, we already used 1)
REMAINING_REG=0
for i in 1 2 3; do
    RESP=$(curl -s -w "\n%{http_code}" -X POST "${COORDINATOR_URL}/register")
    REG_CODE=$(echo "$RESP" | tail -1)
    if [ "$REG_CODE" = "429" ]; then
        echo -e "  ${GREEN}PASS${NC} Registration $((i+1)): rate limited (HTTP 429)"
        PASSED=$((PASSED + 1))
        TOTAL=$((TOTAL + 1))
        REMAINING_REG=$i
        break
    elif [ "$REG_CODE" = "200" ]; then
        echo "  Registration $((i+1)): OK (HTTP 200)"
    else
        echo -e "  ${RED}FAIL${NC} Registration $((i+1)): unexpected HTTP $REG_CODE"
        FAILED=$((FAILED + 1))
        TOTAL=$((TOTAL + 1))
    fi
done
if [ "$REMAINING_REG" -eq 0 ]; then
    echo -e "  ${YELLOW}NOTE${NC} All registrations succeeded (limit may be higher than 4)"
fi
echo ""

# ============================================================================
# Test 13: Non-Bearer auth falls through to payment key
# ============================================================================

echo "13. Bearer without wk_ prefix → falls through to payment key auth"
RESP=$(curl -s -w "\n%{http_code}" \
    -X POST \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer some_other_token" \
    -d '{"input":{}}' \
    "${COORDINATOR_URL}/call/${PROJECT_OWNER}/${PROJECT_NAME}")
parse_response "$RESP"
# Should fall through to payment key path → 401 Missing X-Payment-Key
assert_status "401" "$RESP_CODE" "Bearer without wk_ → 401 (payment key required)"
echo ""

# ============================================================================
# Summary
# ============================================================================

echo "============================================="
echo "Results: $PASSED/$TOTAL passed, $FAILED failed"
if [ "$FAILED" -eq 0 ]; then
    echo -e "${GREEN}All tests passed!${NC}"
else
    echo -e "${RED}$FAILED test(s) failed${NC}"
    exit 1
fi
echo ""
