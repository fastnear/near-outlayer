#!/bin/bash
# ============================================================================
# Gasless Wallet E2E Test — Deposit, Swap, Withdraw (Mainnet)
#
# Flow:
#   0. Check initial balances (NEAR account + intents) and wallet address
#   1. Storage deposit for token (idempotent)
#   2. Deposit tokens into intents (ON-CHAIN — requires NEAR for gas)
#   3. Verify intents balance increased
#   4. Swap with insufficient balance — expect rejection
#   5. Swap quote (read-only)
#   6. Swap via 1Click intents (GASLESS)
#   7. Withdraw dry-run — storage check against nonexistent account
#   8. Withdraw to own wallet (GASLESS)
#   9. Final balance comparison
#
# Note: Step 2 (deposit) is an ON-CHAIN transaction and requires NEAR for gas.
#       Steps 6 and 8 (swap, withdraw) are GASLESS — no NEAR required.
#
# Usage:
#   # Full run:
#   API_KEY=wk_... ./tests/gasless_e2e.sh
#
#   # Resume from step 3:
#   API_KEY=wk_... START_STEP=3 ./tests/gasless_e2e.sh
#
# Environment:
#   API_KEY        — wallet API key with USDC balance (required)
#   API_URL        — default https://api.outlayer.fastnear.com
#   TOKEN          — token contract ID (default: USDC on NEAR mainnet)
#   SWAP_TOKEN_OUT — token to swap to (default: nep141:wrap.near)
#   DEPOSIT_AMOUNT — amount to deposit into intents (default: 100000 = 0.10 USDC, 6 decimals)
#   SWAP_AMOUNT    — amount to swap (default: 50000 = 0.05 USDC)
#   WITHDRAW_AMOUNT— amount to withdraw back (default: 10000 = 0.01 USDC)
#   START_STEP     — step to start from (default: 0)
# ============================================================================

set -euo pipefail

API_URL="${API_URL:-https://api.outlayer.fastnear.com}"
# USDC on NEAR mainnet
TOKEN="${TOKEN:-17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1}"
SWAP_TOKEN_OUT="${SWAP_TOKEN_OUT:-nep141:wrap.near}"
DEPOSIT_AMOUNT="${DEPOSIT_AMOUNT:-100000}"
SWAP_AMOUNT="${SWAP_AMOUNT:-50000}"
WITHDRAW_AMOUNT="${WITHDRAW_AMOUNT:-10000}"
START_STEP="${START_STEP:-0}"

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
NC='\033[0m'

# ============================================================================
# Validation
# ============================================================================

if [ -z "${API_KEY:-}" ]; then
    echo -e "${RED}Error: API_KEY must be set${NC}"
    echo ""
    echo "Usage:"
    echo "  API_KEY=wk_... ./tests/gasless_e2e.sh"
    echo ""
    echo "Resume from step N:"
    echo "  API_KEY=wk_... START_STEP=3 ./tests/gasless_e2e.sh"
    echo ""
    echo "The wallet needs:"
    echo "  - USDC balance (at least $DEPOSIT_AMOUNT units = $(echo "scale=6; $DEPOSIT_AMOUNT / 1000000" | bc) USDC)"
    echo "  - NEAR for gas (step 2 deposit is on-chain)"
    echo "  - Steps 6 (swap) and 8 (withdraw) are gasless"
    exit 1
fi

echo -e "${CYAN}=== Gasless Wallet E2E Test ===${NC}"
echo -e "API:             $API_URL"
echo -e "Token:           $TOKEN"
echo -e "Deposit:         $DEPOSIT_AMOUNT ($(echo "scale=6; $DEPOSIT_AMOUNT / 1000000" | bc) USDC)"
echo -e "Swap:            $SWAP_AMOUNT ($(echo "scale=6; $SWAP_AMOUNT / 1000000" | bc) USDC)"
echo -e "Withdraw:        $WITHDRAW_AMOUNT ($(echo "scale=6; $WITHDRAW_AMOUNT / 1000000" | bc) USDC)"
echo -e "Swap token out:  $SWAP_TOKEN_OUT"
if [ "$START_STEP" -gt 0 ]; then
    echo -e "Starting from:   step $START_STEP"
fi
echo ""

# ============================================================================
# Helpers
# ============================================================================

PASS=0
FAIL=0
WALLET_ADDRESS=""
INTENTS_BALANCE_BEFORE=""

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

assert_gte() {
    local label="$1" actual="$2" minimum="$3"
    if [ "$actual" -ge "$minimum" ] 2>/dev/null; then
        echo -e "  ${GREEN}✓${NC} $label = $actual (>= $minimum)"
        PASS=$((PASS + 1))
    else
        echo -e "  ${RED}✗${NC} $label = $actual (expected >= $minimum)"
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

api_post() { api_call POST "$API_KEY" "$1" "$2"; }
api_get() { api_call GET "$API_KEY" "$1"; }

# Raw curl that captures HTTP code + body without exiting on error
api_raw() {
    local method="$1" path="$2" body="${3:-}"
    local tmpfile
    tmpfile=$(mktemp)
    local http_code
    if [ -n "$body" ]; then
        http_code=$(curl -s -o "$tmpfile" -w "%{http_code}" -X "$method" \
            -H "Content-Type: application/json" \
            -H "Authorization: Bearer $API_KEY" \
            -d "$body" \
            "${API_URL}${path}")
    else
        http_code=$(curl -s -o "$tmpfile" -w "%{http_code}" \
            -H "Authorization: Bearer $API_KEY" \
            "${API_URL}${path}")
    fi
    local response
    response=$(cat "$tmpfile")
    rm -f "$tmpfile"
    echo "$http_code"
    echo "$response"
}

# ============================================================================
# Pre-flight: fetch wallet address (needed by multiple steps)
# ============================================================================

ADDRESS_RESULT=$(api_get "/wallet/v1/address?chain=near")
WALLET_ADDRESS=$(echo "$ADDRESS_RESULT" | jq -r '.address // empty')
if [ -z "$WALLET_ADDRESS" ] || [ "$WALLET_ADDRESS" = "null" ]; then
    echo -e "${RED}Error: failed to get wallet address. Is API_KEY valid?${NC}"
    exit 1
fi

# ============================================================================
# Step 0: Check initial balances + preflight validation
# ============================================================================

if [ "$START_STEP" -le 0 ]; then
echo -e "${CYAN}[0] Checking balances...${NC}"

# Wallet address
echo -e "  Wallet address:     ${YELLOW}$WALLET_ADDRESS${NC}"

# NEAR (native) — needed for gas in steps 1-2
NEAR_NATIVE=$(api_get "/wallet/v1/balance?chain=near" | jq -r '.balance // "0"')
echo -e "  Native NEAR:        $NEAR_NATIVE yoctoNEAR"
# Convert to human readable (24 decimals). >0.01 NEAR is enough for storage + deposit
NEAR_ENOUGH=$(echo "$NEAR_NATIVE > 10000000000000000000000" | bc 2>/dev/null || echo 0)

# USDC on NEAR account — needed for step 2 (deposit into intents)
NEAR_USDC=$(api_get "/wallet/v1/balance?token=$TOKEN" | jq -r '.balance // "0"')
echo -e "  NEAR account USDC:  $NEAR_USDC (need $DEPOSIT_AMOUNT for deposit)"

# USDC in intents — starting point for steps 4-8
INTENTS_BALANCE_BEFORE=$(api_get "/wallet/v1/balance?token=$TOKEN&source=intents" | jq -r '.balance // "0"')
echo -e "  Intents USDC:       $INTENTS_BALANCE_BEFORE"
echo ""

# ── Preflight checks ─────────────────────────────────────────────────────
PREFLIGHT_OK=true

if [ "$NEAR_ENOUGH" != "1" ]; then
    echo -e "  ${RED}✗ Not enough NEAR for gas${NC}"
    echo -e "    Steps 1 (storage-deposit) and 2 (intents/deposit) are on-chain and need NEAR."
    echo -e "    Send at least ${YELLOW}0.05 NEAR${NC} to ${YELLOW}$WALLET_ADDRESS${NC}"
    PREFLIGHT_OK=false
fi

if [ "$NEAR_USDC" -lt "$DEPOSIT_AMOUNT" ] 2>/dev/null; then
    echo -e "  ${RED}✗ Not enough USDC on NEAR account${NC}"
    echo -e "    Step 2 deposits $DEPOSIT_AMOUNT into intents, but wallet has $NEAR_USDC."
    echo -e "    Send at least ${YELLOW}$(echo "scale=6; $DEPOSIT_AMOUNT / 1000000" | bc) USDC${NC} to ${YELLOW}$WALLET_ADDRESS${NC}"
    echo -e "    Token contract: $TOKEN"
    PREFLIGHT_OK=false
fi

if [ "$PREFLIGHT_OK" = "false" ]; then
    echo ""
    echo -e "${YELLOW}Tip: if you already have tokens in intents, skip deposit steps:${NC}"
    echo -e "  API_KEY=$API_KEY START_STEP=3 ./tests/gasless_e2e.sh"
    echo ""
    # If starting from step 3+, we can skip on-chain steps — only check intents balance
    if [ "$START_STEP" -ge 3 ]; then
        echo -e "${YELLOW}START_STEP=$START_STEP — skipping on-chain preflight checks${NC}"
    else
        exit 1
    fi
fi

# Check intents balance if starting from step 3+ (swap/withdraw need tokens there)
if [ "$START_STEP" -ge 3 ]; then
    NEEDED_INTENTS=$((SWAP_AMOUNT + WITHDRAW_AMOUNT))
    if [ "$INTENTS_BALANCE_BEFORE" -lt "$NEEDED_INTENTS" ] 2>/dev/null; then
        echo -e "  ${RED}✗ Not enough USDC in intents balance${NC}"
        echo -e "    Steps 6 (swap) and 8 (withdraw) need $NEEDED_INTENTS total, but intents has $INTENTS_BALANCE_BEFORE."
        echo -e "    Run from step 0 to deposit first, or deposit manually via:"
        echo -e "    curl -X POST -H 'Authorization: Bearer \$API_KEY' -H 'Content-Type: application/json' \\"
        echo -e "      -d '{\"token\":\"$TOKEN\",\"amount\":\"$NEEDED_INTENTS\"}' \\"
        echo -e "      $API_URL/wallet/v1/intents/deposit"
        exit 1
    fi
fi

echo -e "  ${GREEN}✓ Preflight OK${NC}"
echo ""
else
echo -e "${CYAN}[0] Skipped (balances not checked)${NC}"
echo ""
fi

# ============================================================================
# Step 1: Storage deposit (idempotent)
# ============================================================================

if [ "$START_STEP" -le 1 ]; then
echo -e "${CYAN}[1] Storage deposit for token...${NC}"

STORAGE_RESULT=$(api_post "/wallet/v1/storage-deposit" \
    "{\"token\":\"$TOKEN\"}") || { echo -e "  ${RED}✗ storage-deposit failed${NC}"; FAIL=$((FAIL + 1)); }

STORAGE_STATUS=$(echo "$STORAGE_RESULT" | jq -r '.status // empty')
ALREADY_REG=$(echo "$STORAGE_RESULT" | jq -r '.already_registered // empty')

assert_eq "status" "$STORAGE_STATUS" "success"

# already_registered must be a boolean (true or false)
if [ "$ALREADY_REG" = "true" ] || [ "$ALREADY_REG" = "false" ]; then
    echo -e "  ${GREEN}✓${NC} already_registered = $ALREADY_REG"
    PASS=$((PASS + 1))
else
    echo -e "  ${RED}✗${NC} already_registered: expected boolean, got '$ALREADY_REG'"
    FAIL=$((FAIL + 1))
fi

# If not already registered, tx_hash should be present
if [ "$ALREADY_REG" = "false" ]; then
    STORAGE_TX=$(echo "$STORAGE_RESULT" | jq -r '.tx_hash // empty')
    assert_not_empty "tx_hash" "$STORAGE_TX"
fi
echo ""
else
echo -e "${CYAN}[1] Skipped${NC}"
echo ""
fi

# ============================================================================
# Step 2: Deposit tokens into intents (ON-CHAIN — requires NEAR gas)
# ============================================================================

if [ "$START_STEP" -le 2 ]; then
echo -e "${CYAN}[2] Depositing $DEPOSIT_AMOUNT into intents (on-chain tx)...${NC}"

DEPOSIT_RESULT=$(api_post "/wallet/v1/intents/deposit" \
    "{\"token\":\"$TOKEN\",\"amount\":\"$DEPOSIT_AMOUNT\"}") || { echo -e "  ${RED}✗ deposit failed${NC}"; FAIL=$((FAIL + 1)); }

DEPOSIT_STATUS=$(echo "$DEPOSIT_RESULT" | jq -r '.status // empty')
DEPOSIT_TX=$(echo "$DEPOSIT_RESULT" | jq -r '.tx_hash // empty')

assert_eq "status" "$DEPOSIT_STATUS" "success"
assert_not_empty "tx_hash" "$DEPOSIT_TX"
echo ""
else
echo -e "${CYAN}[2] Skipped${NC}"
echo ""
fi

# ============================================================================
# Step 3: Verify intents balance increased
# ============================================================================

if [ "$START_STEP" -le 3 ]; then
echo -e "${CYAN}[3] Verifying intents balance after deposit...${NC}"

INTENTS_BALANCE_AFTER_DEPOSIT=$(api_get "/wallet/v1/balance?token=$TOKEN&source=intents" | jq -r '.balance // "0"')
echo -e "  Intents USDC after deposit: $INTENTS_BALANCE_AFTER_DEPOSIT"

if [ -n "$INTENTS_BALANCE_BEFORE" ]; then
    EXPECTED_MIN=$(( INTENTS_BALANCE_BEFORE + DEPOSIT_AMOUNT ))
    echo -e "  Expected minimum: $EXPECTED_MIN (before $INTENTS_BALANCE_BEFORE + deposit $DEPOSIT_AMOUNT)"
    assert_gte "intents_balance" "$INTENTS_BALANCE_AFTER_DEPOSIT" "$EXPECTED_MIN"
else
    # No before snapshot — just check it's at least DEPOSIT_AMOUNT
    assert_gte "intents_balance" "$INTENTS_BALANCE_AFTER_DEPOSIT" "$DEPOSIT_AMOUNT"
fi
echo ""
else
echo -e "${CYAN}[3] Skipped${NC}"
echo ""
fi

# ============================================================================
# Step 4: Swap with insufficient balance — expect rejection
# ============================================================================

if [ "$START_STEP" -le 4 ]; then
echo -e "${CYAN}[4] Swap with insufficient balance (expect error)...${NC}"

# Use raw curl — api_call would exit on HTTP error
RAW_OUTPUT=$(api_raw POST "/wallet/v1/intents/swap" \
    "{\"token_in\":\"nep141:$TOKEN\",\"token_out\":\"$SWAP_TOKEN_OUT\",\"amount_in\":\"999999999999999\"}")

HTTP_CODE=$(echo "$RAW_OUTPUT" | head -1)
BODY=$(echo "$RAW_OUTPUT" | tail -n +2)

if [ "$HTTP_CODE" -ge 400 ]; then
    echo -e "  ${GREEN}✓${NC} Rejected with HTTP $HTTP_CODE"
    PASS=$((PASS + 1))
else
    # Check if the body contains an error message
    ERROR_MSG=$(echo "$BODY" | jq -r '.error // .message // empty')
    if echo "$ERROR_MSG" | grep -qi "insufficient"; then
        echo -e "  ${GREEN}✓${NC} Rejected: $ERROR_MSG"
        PASS=$((PASS + 1))
    else
        echo -e "  ${RED}✗${NC} Expected HTTP 400+ or 'Insufficient' error, got HTTP $HTTP_CODE: $BODY"
        FAIL=$((FAIL + 1))
    fi
fi
echo ""
else
echo -e "${CYAN}[4] Skipped${NC}"
echo ""
fi

# ============================================================================
# Step 5: Swap quote (read-only)
# ============================================================================

if [ "$START_STEP" -le 5 ]; then
echo -e "${CYAN}[5] Getting swap quote...${NC}"

QUOTE_RESULT=$(api_post "/wallet/v1/intents/swap/quote" \
    "{\"token_in\":\"nep141:$TOKEN\",\"token_out\":\"$SWAP_TOKEN_OUT\",\"amount_in\":\"$SWAP_AMOUNT\"}") || { echo -e "  ${RED}✗ quote failed${NC}"; FAIL=$((FAIL + 1)); }

QUOTE_AMOUNT_OUT=$(echo "$QUOTE_RESULT" | jq -r '.amount_out // empty')
QUOTE_DEPOSIT_ADDR=$(echo "$QUOTE_RESULT" | jq -r '.deposit_address // empty')

assert_not_empty "amount_out" "$QUOTE_AMOUNT_OUT"
assert_not_empty "deposit_address" "$QUOTE_DEPOSIT_ADDR"
echo -e "  Swap $SWAP_AMOUNT USDC -> $QUOTE_AMOUNT_OUT ($SWAP_TOKEN_OUT)"
echo ""
else
echo -e "${CYAN}[5] Skipped${NC}"
echo ""
fi

# ============================================================================
# Step 6: Swap via intents (GASLESS)
# ============================================================================

if [ "$START_STEP" -le 6 ]; then
echo -e "${CYAN}[6] Swapping $SWAP_AMOUNT USDC -> $SWAP_TOKEN_OUT (gasless)...${NC}"

SWAP_RESULT=$(api_post "/wallet/v1/intents/swap" \
    "{\"token_in\":\"nep141:$TOKEN\",\"token_out\":\"$SWAP_TOKEN_OUT\",\"amount_in\":\"$SWAP_AMOUNT\"}") || { echo -e "  ${RED}✗ swap failed${NC}"; FAIL=$((FAIL + 1)); }

SWAP_STATUS=$(echo "$SWAP_RESULT" | jq -r '.status // empty')
SWAP_REQUEST_ID=$(echo "$SWAP_RESULT" | jq -r '.request_id // empty')

# Accept both "success" and "processing" (1Click may still be settling)
if [ "$SWAP_STATUS" = "success" ] || [ "$SWAP_STATUS" = "processing" ]; then
    echo -e "  ${GREEN}✓${NC} status = $SWAP_STATUS"
    PASS=$((PASS + 1))
else
    echo -e "  ${RED}✗${NC} status: expected 'success' or 'processing', got '$SWAP_STATUS'"
    FAIL=$((FAIL + 1))
fi
assert_not_empty "request_id" "$SWAP_REQUEST_ID"
echo ""
else
echo -e "${CYAN}[6] Skipped${NC}"
echo ""
fi

# ============================================================================
# Step 7: Withdraw dry-run (storage check)
# ============================================================================

if [ "$START_STEP" -le 7 ]; then
echo -e "${CYAN}[7] Withdraw dry-run to nonexistent account...${NC}"

# The endpoint might return 200 with would_succeed=false, or 400 — handle both
RAW_OUTPUT=$(api_raw POST "/wallet/v1/intents/withdraw/dry-run" \
    "{\"chain\":\"near\",\"to\":\"nonexistent-account.near\",\"amount\":\"1\",\"token\":\"$TOKEN\"}")

HTTP_CODE=$(echo "$RAW_OUTPUT" | head -1)
BODY=$(echo "$RAW_OUTPUT" | tail -n +2)

if [ "$HTTP_CODE" -lt 400 ]; then
    # 200 response — check would_succeed field
    WOULD_SUCCEED=$(echo "$BODY" | jq -r '.would_succeed // empty')
    REASON=$(echo "$BODY" | jq -r '.reason // empty')

    if [ "$WOULD_SUCCEED" = "false" ]; then
        echo -e "  ${GREEN}✓${NC} would_succeed = false (reason: $REASON)"
        PASS=$((PASS + 1))
    else
        echo -e "  ${RED}✗${NC} Expected would_succeed=false for nonexistent account, got: $BODY"
        FAIL=$((FAIL + 1))
    fi
else
    # 400+ response — acceptable if it explains why
    ERROR_MSG=$(echo "$BODY" | jq -r '.error // .message // empty')
    if [ -n "$ERROR_MSG" ]; then
        echo -e "  ${GREEN}✓${NC} Rejected (HTTP $HTTP_CODE): $ERROR_MSG"
        PASS=$((PASS + 1))
    else
        echo -e "  ${GREEN}✓${NC} Rejected (HTTP $HTTP_CODE)"
        PASS=$((PASS + 1))
    fi
fi
echo ""
else
echo -e "${CYAN}[7] Skipped${NC}"
echo ""
fi

# ============================================================================
# Step 8: Withdraw to own wallet (GASLESS)
# ============================================================================

if [ "$START_STEP" -le 8 ]; then
echo -e "${CYAN}[8] Withdrawing $WITHDRAW_AMOUNT to own wallet (gasless)...${NC}"

if [ -z "$WALLET_ADDRESS" ]; then
    echo -e "  ${RED}✗${NC} No wallet address available — run from step 0"
    FAIL=$((FAIL + 1))
else
    echo -e "  Destination: $WALLET_ADDRESS"

    WITHDRAW_RESULT=$(api_post "/wallet/v1/intents/withdraw" \
        "{\"chain\":\"near\",\"to\":\"$WALLET_ADDRESS\",\"amount\":\"$WITHDRAW_AMOUNT\",\"token\":\"$TOKEN\"}") || { echo -e "  ${RED}✗ withdraw failed${NC}"; FAIL=$((FAIL + 1)); }

    WITHDRAW_STATUS=$(echo "$WITHDRAW_RESULT" | jq -r '.status // empty')
    WITHDRAW_REQUEST_ID=$(echo "$WITHDRAW_RESULT" | jq -r '.request_id // empty')

    assert_eq "status" "$WITHDRAW_STATUS" "success"
    assert_not_empty "request_id" "$WITHDRAW_REQUEST_ID"
fi
echo ""
else
echo -e "${CYAN}[8] Skipped${NC}"
echo ""
fi

# ============================================================================
# Step 9: Final balance check
# ============================================================================

if [ "$START_STEP" -le 9 ]; then
echo -e "${CYAN}[9] Final balance comparison...${NC}"

INTENTS_BALANCE_FINAL=$(api_get "/wallet/v1/balance?token=$TOKEN&source=intents" | jq -r '.balance // "0"')

if [ -n "$INTENTS_BALANCE_BEFORE" ]; then
    echo -e "  Intents USDC before:  $INTENTS_BALANCE_BEFORE"
    echo -e "  Intents USDC after:   $INTENTS_BALANCE_FINAL"
    DIFF=$((INTENTS_BALANCE_FINAL - INTENTS_BALANCE_BEFORE))
    NET_EXPECTED=$((DEPOSIT_AMOUNT - SWAP_AMOUNT - WITHDRAW_AMOUNT))
    echo -e "  Net change:           $DIFF (expected approx $NET_EXPECTED)"
    echo -e "  ${YELLOW}Note: swap output was in $SWAP_TOKEN_OUT — USDC balance reflects deposit minus swap minus withdraw${NC}"
else
    echo -e "  Intents USDC: $INTENTS_BALANCE_FINAL (no before snapshot — skipped step 0)"
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
