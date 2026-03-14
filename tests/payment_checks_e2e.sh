#!/bin/bash
# ============================================================================
# Payment Checks E2E Test — Agent-to-Agent Payments (Mainnet)
#
# Flow:
#   1. Agent A creates a payment check (e.g. 0.01 USDC = 10000 units)
#   2. Agent B partial claims 3000 units
#   3. Verify status = partially_claimed
#   4. Agent A partial reclaims 3000 units
#   5. Agent B claims the remaining 4000 units
#   6. Verify final status = claimed
#   7. Verify receiver balance increased
#   8. Verify double-claim fails
#   9. Check appears in sender's list
#
# Usage:
#   # Full run:
#   SENDER_API_KEY=wk_... RECEIVER_API_KEY=wk_... ./tests/payment_checks_e2e.sh
#
#   # Resume from step 2 with existing check:
#   SENDER_API_KEY=wk_... RECEIVER_API_KEY=wk_... \
#     START_STEP=2 CHECK_ID=... CHECK_KEY=... ./tests/payment_checks_e2e.sh
#
# Environment:
#   SENDER_API_KEY    — API key for Agent A (check creator)
#   RECEIVER_API_KEY  — API key for Agent B (check claimer)
#   API_URL           — default https://api.outlayer.fastnear.com
#   TOKEN             — token contract ID (default: USDC on NEAR mainnet)
#   TOTAL_AMOUNT      — total check amount in smallest units (default: 10000 = 0.01 USDC)
#   CLAIM_1           — first partial claim amount (default: 3000)
#   RECLAIM_1         — sender reclaim amount (default: 3000)
#   START_STEP        — step to start from (default: 0). Steps 2+ require CHECK_ID and CHECK_KEY
#   CHECK_ID          — existing check ID (required when START_STEP >= 2)
#   CHECK_KEY         — existing check key (required when START_STEP >= 2)
# ============================================================================

set -euo pipefail

API_URL="${API_URL:-https://api.outlayer.fastnear.com}"
# USDC on NEAR mainnet
TOKEN="${TOKEN:-17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1}"
TOTAL_AMOUNT="${TOTAL_AMOUNT:-10000}"
CLAIM_1="${CLAIM_1:-3000}"
RECLAIM_1="${RECLAIM_1:-3000}"
START_STEP="${START_STEP:-0}"

# Derived: remaining after partial claim + partial reclaim
CLAIM_2=$(( TOTAL_AMOUNT - CLAIM_1 - RECLAIM_1 ))

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
NC='\033[0m'

# ============================================================================
# Validation
# ============================================================================

if [ -z "${SENDER_API_KEY:-}" ] || [ -z "${RECEIVER_API_KEY:-}" ]; then
    echo -e "${RED}Error: SENDER_API_KEY and RECEIVER_API_KEY must be set${NC}"
    echo ""
    echo "Usage:"
    echo "  SENDER_API_KEY=wk_... RECEIVER_API_KEY=wk_... ./tests/payment_checks_e2e.sh"
    echo ""
    echo "Resume from step N:"
    echo "  START_STEP=2 CHECK_ID=... CHECK_KEY=... SENDER_API_KEY=wk_... RECEIVER_API_KEY=wk_... ./tests/payment_checks_e2e.sh"
    echo ""
    echo "Both wallets need USDC balance (at least $TOTAL_AMOUNT units = $(echo "scale=6; $TOTAL_AMOUNT / 1000000" | bc) USDC for sender)."
    echo "All operations are gasless — no NEAR required."
    exit 1
fi

if [ "$CLAIM_2" -le 0 ]; then
    echo -e "${RED}Error: CLAIM_1 ($CLAIM_1) + RECLAIM_1 ($RECLAIM_1) >= TOTAL_AMOUNT ($TOTAL_AMOUNT)${NC}"
    echo "Remaining for second claim must be > 0."
    exit 1
fi

if [ "$START_STEP" -ge 2 ]; then
    if [ -z "${CHECK_ID:-}" ] || [ -z "${CHECK_KEY:-}" ]; then
        echo -e "${RED}Error: CHECK_ID and CHECK_KEY required when START_STEP >= 2${NC}"
        echo ""
        echo "Find your checks:  curl -s -H 'Authorization: Bearer \$SENDER_API_KEY' '$API_URL/wallet/v1/payment-check/list?limit=10' | jq ."
        exit 1
    fi
fi

echo -e "${CYAN}=== Payment Checks E2E Test ===${NC}"
echo -e "API:           $API_URL"
echo -e "Token:         $TOKEN"
echo -e "Total:         $TOTAL_AMOUNT"
echo -e "Partial claim: $CLAIM_1 → reclaim: $RECLAIM_1 → final claim: $CLAIM_2"
if [ "$START_STEP" -gt 0 ]; then
    echo -e "Starting from:  step $START_STEP"
    [ -n "${CHECK_ID:-}" ] && echo -e "Check ID:       ${YELLOW}$CHECK_ID${NC}"
    [ -n "${CHECK_KEY:-}" ] && echo -e "Check key:      ${YELLOW}${CHECK_KEY:0:20}...${NC}"
fi
echo ""

# ============================================================================
# Helpers
# ============================================================================

PASS=0
FAIL=0
CHECK_ID="${CHECK_ID:-}"
CHECK_KEY="${CHECK_KEY:-}"

assert_eq() {
    local label="$1" actual="$2" expected="$3"
    if [ "$actual" = "$expected" ]; then
        echo -e "  ${GREEN}✓${NC} $label = $actual"
        PASS=$((PASS + 1))
    else
        echo -e "  ${RED}✗${NC} $label: expected '$expected', got '$actual'"
        FAIL=$((FAIL + 1))
    fi
}

assert_not_empty() {
    local label="$1" actual="$2"
    if [ -n "$actual" ] && [ "$actual" != "null" ]; then
        echo -e "  ${GREEN}✓${NC} $label = $actual"
        PASS=$((PASS + 1))
    else
        echo -e "  ${RED}✗${NC} $label is empty/null"
        FAIL=$((FAIL + 1))
    fi
}

api_call() {
    local method="$1" api_key="$2" path="$3" body="${4:-}"
    local tmpfile
    tmpfile=$(mktemp)
    local http_code
    if [ -n "$body" ]; then
        http_code=$(curl -s -o "$tmpfile" -w "%{http_code}" -X "$method" \
            -H "Content-Type: application/json" \
            -H "Authorization: Bearer $api_key" \
            -d "$body" \
            "${API_URL}${path}")
    else
        http_code=$(curl -s -o "$tmpfile" -w "%{http_code}" \
            -H "Authorization: Bearer $api_key" \
            "${API_URL}${path}")
    fi
    local response
    response=$(cat "$tmpfile")
    rm -f "$tmpfile"
    if [ "$http_code" -ge 400 ]; then
        echo -e "  ${RED}HTTP $http_code${NC}: $response" >&2
        echo ""
        return 1
    fi
    echo "$response"
}

sender_post() { api_call POST "$SENDER_API_KEY" "$1" "$2"; }
sender_get() { api_call GET "$SENDER_API_KEY" "$1"; }
receiver_post() { api_call POST "$RECEIVER_API_KEY" "$1" "$2"; }
receiver_get() { api_call GET "$RECEIVER_API_KEY" "$1"; }

# ============================================================================
# Step 0: Check balances
# ============================================================================

if [ "$START_STEP" -le 0 ]; then
echo -e "${CYAN}[0] Checking balances...${NC}"

SENDER_BALANCE=$(sender_get "/wallet/v1/balance?token=$TOKEN&source=intents" | jq -r '.balance // "0"')
echo -e "  Sender intents USDC: $SENDER_BALANCE (need $TOTAL_AMOUNT)"

RECEIVER_BALANCE_BEFORE=$(receiver_get "/wallet/v1/balance?token=$TOKEN&source=intents" | jq -r '.balance // "0"')
echo -e "  Receiver intents USDC before: $RECEIVER_BALANCE_BEFORE"
echo ""
fi

# ============================================================================
# Step 1: Agent A creates a payment check
# ============================================================================

if [ "$START_STEP" -le 1 ]; then
echo -e "${CYAN}[1] Creating payment check ($TOTAL_AMOUNT units, 1h expiry)...${NC}"

CREATE_RESULT=$(sender_post "/wallet/v1/payment-check/create" \
    "{\"token\":\"$TOKEN\",\"amount\":\"$TOTAL_AMOUNT\",\"memo\":\"E2E test payment\",\"expires_in\":3600}") || { echo -e "  ${RED}✗ create failed${NC}"; FAIL=$((FAIL + 1)); }

CHECK_ID=$(echo "$CREATE_RESULT" | jq -r '.check_id')
CHECK_KEY=$(echo "$CREATE_RESULT" | jq -r '.check_key')
CREATE_AMOUNT=$(echo "$CREATE_RESULT" | jq -r '.amount')

assert_not_empty "check_id" "$CHECK_ID"
assert_not_empty "check_key" "$CHECK_KEY"
assert_eq "amount" "$CREATE_AMOUNT" "$TOTAL_AMOUNT"
echo -e "  check_id: ${YELLOW}$CHECK_ID${NC}"
echo -e "  check_key: ${YELLOW}${CHECK_KEY:0:20}...${NC}"
echo ""
else
echo -e "${CYAN}[1] Skipped (using existing check)${NC}"
echo ""
fi

# ============================================================================
# Step 2: Agent B partial claims (CLAIM_1 units)
# ============================================================================

if [ "$START_STEP" -le 2 ]; then
echo -e "${CYAN}[2] Agent B partial claim ($CLAIM_1 units)...${NC}"

CLAIM1_RESULT=$(receiver_post "/wallet/v1/payment-check/claim" \
    "{\"check_key\":\"$CHECK_KEY\",\"amount\":\"$CLAIM_1\"}") || { echo -e "  ${RED}✗ claim failed${NC}"; FAIL=$((FAIL + 1)); }

CLAIM1_CLAIMED=$(echo "$CLAIM1_RESULT" | jq -r '.amount_claimed')
CLAIM1_REMAINING=$(echo "$CLAIM1_RESULT" | jq -r '.remaining')

assert_not_empty "amount_claimed" "$CLAIM1_CLAIMED"
assert_eq "amount_claimed" "$CLAIM1_CLAIMED" "$CLAIM_1"

EXPECTED_REMAINING_1=$(( TOTAL_AMOUNT - CLAIM_1 ))
assert_eq "remaining" "$CLAIM1_REMAINING" "$EXPECTED_REMAINING_1"
echo ""
else
echo -e "${CYAN}[2] Skipped${NC}"
echo ""
fi

# ============================================================================
# Step 3: Verify status shows partially_claimed
# ============================================================================

if [ "$START_STEP" -le 3 ]; then
echo -e "${CYAN}[3] Checking status (expect partially_claimed)...${NC}"

STATUS1_RESULT=$(sender_get "/wallet/v1/payment-check/status?check_id=$CHECK_ID")
STATUS1_STATUS=$(echo "$STATUS1_RESULT" | jq -r '.status')
STATUS1_CLAIMED_AMT=$(echo "$STATUS1_RESULT" | jq -r '.claimed_amount')

assert_eq "status" "$STATUS1_STATUS" "partially_claimed"
assert_eq "claimed_amount" "$STATUS1_CLAIMED_AMT" "$CLAIM_1"
echo ""
else
echo -e "${CYAN}[3] Skipped${NC}"
echo ""
fi

# ============================================================================
# Step 4: Agent A partial reclaims (RECLAIM_1 units)
# ============================================================================

if [ "$START_STEP" -le 4 ]; then
echo -e "${CYAN}[4] Agent A partial reclaim ($RECLAIM_1 units)...${NC}"

RECLAIM_RESULT=$(sender_post "/wallet/v1/payment-check/reclaim" \
    "{\"check_id\":\"$CHECK_ID\",\"amount\":\"$RECLAIM_1\"}") || { echo -e "  ${RED}✗ reclaim failed${NC}"; FAIL=$((FAIL + 1)); }

RECLAIM_AMOUNT=$(echo "$RECLAIM_RESULT" | jq -r '.amount_reclaimed')
RECLAIM_REMAINING=$(echo "$RECLAIM_RESULT" | jq -r '.remaining')

assert_not_empty "amount_reclaimed" "$RECLAIM_AMOUNT"
assert_eq "amount_reclaimed" "$RECLAIM_AMOUNT" "$RECLAIM_1"
assert_eq "remaining" "$RECLAIM_REMAINING" "$CLAIM_2"
echo ""
else
echo -e "${CYAN}[4] Skipped${NC}"
echo ""
fi

# ============================================================================
# Step 5: Agent B claims the remaining (CLAIM_2 units)
# ============================================================================

if [ "$START_STEP" -le 5 ]; then
echo -e "${CYAN}[5] Agent B claims remaining ($CLAIM_2 units)...${NC}"

CLAIM2_RESULT=$(receiver_post "/wallet/v1/payment-check/claim" \
    "{\"check_key\":\"$CHECK_KEY\"}") || { echo -e "  ${RED}✗ claim failed${NC}"; FAIL=$((FAIL + 1)); }

CLAIM2_CLAIMED=$(echo "$CLAIM2_RESULT" | jq -r '.amount_claimed')
CLAIM2_REMAINING=$(echo "$CLAIM2_RESULT" | jq -r '.remaining')

assert_not_empty "amount_claimed" "$CLAIM2_CLAIMED"
assert_eq "amount_claimed" "$CLAIM2_CLAIMED" "$CLAIM_2"
assert_eq "remaining" "$CLAIM2_REMAINING" "0"
echo ""
else
echo -e "${CYAN}[5] Skipped${NC}"
echo ""
fi

# ============================================================================
# Step 6: Verify final status = claimed
# ============================================================================

if [ "$START_STEP" -le 6 ]; then
echo -e "${CYAN}[6] Final status check...${NC}"

STATUS2_RESULT=$(sender_get "/wallet/v1/payment-check/status?check_id=$CHECK_ID")
STATUS2_STATUS=$(echo "$STATUS2_RESULT" | jq -r '.status')
STATUS2_CLAIMED_AMT=$(echo "$STATUS2_RESULT" | jq -r '.claimed_amount')

TOTAL_CLAIMED=$(( CLAIM_1 + CLAIM_2 ))
assert_eq "status" "$STATUS2_STATUS" "claimed"
assert_eq "claimed_amount" "$STATUS2_CLAIMED_AMT" "$TOTAL_CLAIMED"
echo ""
else
echo -e "${CYAN}[6] Skipped${NC}"
echo ""
fi

# ============================================================================
# Step 7: Verify receiver intents balance increased
# ============================================================================

if [ "$START_STEP" -le 7 ]; then
echo -e "${CYAN}[7] Verifying receiver balance change...${NC}"

RECEIVER_BALANCE_AFTER=$(receiver_get "/wallet/v1/balance?token=$TOKEN&source=intents" | jq -r '.balance // "0"')
# If we skipped step 0, we don't have a "before" value — just show current
if [ -n "${RECEIVER_BALANCE_BEFORE:-}" ]; then
    EXPECTED_INCREASE=$(( CLAIM_1 + CLAIM_2 ))
    ACTUAL_INCREASE=$(( RECEIVER_BALANCE_AFTER - RECEIVER_BALANCE_BEFORE ))

    echo -e "  Receiver intents USDC before: $RECEIVER_BALANCE_BEFORE"
    echo -e "  Receiver intents USDC after:  $RECEIVER_BALANCE_AFTER"
    echo -e "  Expected increase: $EXPECTED_INCREASE"

    if [ "$ACTUAL_INCREASE" -ge "$EXPECTED_INCREASE" ]; then
        echo -e "  ${GREEN}✓${NC} Balance increased by $ACTUAL_INCREASE (>= expected $EXPECTED_INCREASE)"
        PASS=$((PASS + 1))
    else
        echo -e "  ${RED}✗${NC} Balance increased by $ACTUAL_INCREASE (expected >= $EXPECTED_INCREASE)"
        FAIL=$((FAIL + 1))
    fi
else
    echo -e "  Receiver intents USDC: $RECEIVER_BALANCE_AFTER (no before snapshot — skipped step 0)"
fi
echo ""
fi

# ============================================================================
# Step 8: Verify claim on empty check fails
# ============================================================================

if [ "$START_STEP" -le 8 ]; then
echo -e "${CYAN}[8] Verify double-claim fails...${NC}"

DOUBLE_CLAIM_HTTP=$(curl -s -o /dev/null -w "%{http_code}" -X POST \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $RECEIVER_API_KEY" \
    -d "{\"check_key\":\"$CHECK_KEY\"}" \
    "${API_URL}/wallet/v1/payment-check/claim")

if [ "$DOUBLE_CLAIM_HTTP" -ge 400 ]; then
    echo -e "  ${GREEN}✓${NC} Double claim rejected (HTTP $DOUBLE_CLAIM_HTTP)"
    PASS=$((PASS + 1))
else
    echo -e "  ${RED}✗${NC} Double claim should fail, got HTTP $DOUBLE_CLAIM_HTTP"
    FAIL=$((FAIL + 1))
fi
echo ""
fi

# ============================================================================
# Step 9: Check appears in list
# ============================================================================

if [ "$START_STEP" -le 9 ]; then
echo -e "${CYAN}[9] Check appears in sender's list...${NC}"

LIST_RESULT=$(sender_get "/wallet/v1/payment-check/list?limit=10")
FOUND=$(echo "$LIST_RESULT" | jq -r ".checks[] | select(.check_id == \"$CHECK_ID\") | .check_id")

if [ "$FOUND" = "$CHECK_ID" ]; then
    echo -e "  ${GREEN}✓${NC} Found $CHECK_ID in list"
    PASS=$((PASS + 1))
else
    echo -e "  ${RED}✗${NC} Check $CHECK_ID not found in list"
    FAIL=$((FAIL + 1))
fi
echo ""
fi

# ============================================================================
# Summary
# ============================================================================

TOTAL=$((PASS + FAIL))
echo -e "${CYAN}=== Results ===${NC}"
echo -e "  ${GREEN}Passed: $PASS${NC}"
if [ "$FAIL" -gt 0 ]; then
    echo -e "  ${RED}Failed: $FAIL${NC}"
else
    echo -e "  Failed: 0"
fi
echo -e "  Total:  $TOTAL"
echo ""

if [ "$FAIL" -gt 0 ]; then
    echo -e "${RED}FAILED${NC}"
    exit 1
else
    echo -e "${GREEN}ALL PASSED${NC}"
fi
