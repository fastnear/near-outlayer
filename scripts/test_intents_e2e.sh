#!/bin/bash
# =============================================================================
# E2E Test: Full Intents Cycle
#
# Flow:
#   1. Register wallet (or reuse existing)
#   2. Get NEAR address, wait for 0.1 NEAR funding
#   3. Wrap 0.09 NEAR → wNEAR  (/wallet/v1/call)
#   4. Swap wNEAR → USDT       (/wallet/v1/intents/swap)
#   5. Send USDT to user       (/wallet/v1/call ft_transfer)
#
# Endpoints tested:
#   POST /register
#   GET  /wallet/v1/address
#   GET  /wallet/v1/balance
#   POST /wallet/v1/call           (wrap, storage_deposit, ft_transfer)
#   POST /wallet/v1/intents/swap
#   GET  /wallet/v1/requests/:id
#
# Usage:
#   ./scripts/test_intents_e2e.sh
#   API_KEY=wk_xxx USER_ACCOUNT=you.near ./scripts/test_intents_e2e.sh
#   API_KEY=wk_xxx START_STEP=4 ./scripts/test_intents_e2e.sh   # resume
#   BASE_URL=http://localhost:8080 ./scripts/test_intents_e2e.sh # local
# =============================================================================

set -euo pipefail

# ---- Configuration ----
BASE_URL="${BASE_URL:-https://api.outlayer.fastnear.com}"
API_KEY="${API_KEY:-}"
USER_ACCOUNT="${USER_ACCOUNT:-}"
START_STEP="${START_STEP:-1}"
WRAP_AMOUNT="${WRAP_AMOUNT:-50000000000000000000000}"  # 0.05 NEAR (leaves ~0.05 for gas)
USDT_CONTRACT="usdt.tether-token.near"
WRAP_CONTRACT="wrap.near"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log()  { echo -e "${BLUE}[INFO]${NC} $*"; }
ok()   { echo -e "${GREEN}[ OK ]${NC} $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
fail() { echo -e "${RED}[FAIL]${NC} $*" >&2; exit 1; }

command -v curl >/dev/null || fail "curl is required"
command -v jq >/dev/null || fail "jq is required"

# ---- HTTP helpers ----
_BODY_FILE=$(mktemp)
_STATUS_FILE=$(mktemp)
trap "rm -f $_BODY_FILE $_STATUS_FILE" EXIT

curl_check() {
    curl -s -o "$_BODY_FILE" -w '%{http_code}' "$@" > "$_STATUS_FILE"
}
http_status() { cat "$_STATUS_FILE"; }
http_body()   { cat "$_BODY_FILE"; }

wallet_get() {
    local path="$1"
    curl_check "$BASE_URL$path" -H "Authorization: Bearer $API_KEY"
    local status; status=$(http_status)
    local response; response=$(http_body)
    if [ "$status" -ge 400 ]; then
        fail "GET $path → HTTP $status: $response"
    fi
    echo "$response"
}

wallet_post() {
    local path="$1"
    local body="$2"
    local idem_key="${3:-$(uuidgen 2>/dev/null || cat /proc/sys/kernel/random/uuid 2>/dev/null || echo $RANDOM)}"
    curl_check "$BASE_URL$path" \
        -H "Authorization: Bearer $API_KEY" \
        -H "Content-Type: application/json" \
        -H "X-Idempotency-Key: $idem_key" \
        -d "$body"
    local status; status=$(http_status)
    local response; response=$(http_body)
    if [ "$status" -ge 400 ]; then
        fail "POST $path → HTTP $status: $response"
    fi
    echo "$response"
}

echo ""
echo "============================================================"
echo "  Intents E2E Test"
echo "  Base: $BASE_URL"
echo "  Flow: register → fund → wrap → swap → withdraw USDT"
echo "============================================================"
echo ""

# =============================================================================
# Step 1: Register or reuse wallet
# =============================================================================
if [ -z "$API_KEY" ]; then
    log "Step 1: Registering new wallet..."
    curl_check -X POST "$BASE_URL/register"
    if [ "$(http_status)" -ge 400 ]; then
        fail "POST /register → HTTP $(http_status): $(http_body)"
    fi
    REGISTER_RESPONSE=$(http_body)
    echo "$REGISTER_RESPONSE" | jq .

    API_KEY=$(echo "$REGISTER_RESPONSE" | jq -r '.api_key')
    [ -z "$API_KEY" ] || [ "$API_KEY" = "null" ] && fail "Registration failed"

    ok "API key: $API_KEY"
    echo ""
else
    log "Step 1: Reusing wallet API_KEY=${API_KEY:0:12}..."
fi

# =============================================================================
# Step 2: Get address + check balance (always runs)
# =============================================================================
log "Step 2: Getting NEAR address..."
ADDRESS_RESPONSE=$(wallet_get "/wallet/v1/address?chain=near")
WALLET_ADDRESS=$(echo "$ADDRESS_RESPONSE" | jq -r '.address')
[ -z "$WALLET_ADDRESS" ] || [ "$WALLET_ADDRESS" = "null" ] && fail "No address"
ok "Address: $WALLET_ADDRESS"

log "Checking NEAR balance..."
BALANCE_RESPONSE=$(wallet_get "/wallet/v1/balance?chain=near")
NEAR_BALANCE=$(echo "$BALANCE_RESPONSE" | jq -r '.balance // "0"')
ok "Balance: $NEAR_BALANCE yoctoNEAR"

# Need at least 0.1 NEAR (skip check when resuming from later step)
if [ "$START_STEP" -le 2 ]; then
    MIN_BALANCE="100000000000000000000000"
    if python3 -c "exit(0 if int('${NEAR_BALANCE}') < int('${MIN_BALANCE}') else 1)" 2>/dev/null; then
        echo ""
        warn "Need at least 0.1 NEAR. Send to: $WALLET_ADDRESS"
        echo ""
        echo "  Fund link:"
        echo "    https://outlayer.fastnear.com/wallet/fund?to=${WALLET_ADDRESS}&amount=0.1&token=near"
        echo ""
        echo "  Or near-cli:"
        echo "    near send <your-account>.near $WALLET_ADDRESS 0.1"
        echo ""
        read -p "Press Enter after funding (or Ctrl+C to abort)..."
        echo ""
        log "Re-checking..."
        BALANCE_RESPONSE=$(wallet_get "/wallet/v1/balance?chain=near")
        NEAR_BALANCE=$(echo "$BALANCE_RESPONSE" | jq -r '.balance // "0"')
        ok "Balance: $NEAR_BALANCE yoctoNEAR"
    fi
fi

[ "$START_STEP" -gt 2 ] && log "Resuming from step $START_STEP..."

# =============================================================================
# Step 3: Wrap NEAR → wNEAR
# =============================================================================
if [ "$START_STEP" -le 3 ]; then
log "Step 3: Wrapping NEAR → wNEAR ($WRAP_AMOUNT yocto)..."

WRAP_RESPONSE=$(wallet_post "/wallet/v1/call" "{
    \"receiver_id\": \"$WRAP_CONTRACT\",
    \"method_name\": \"near_deposit\",
    \"args\": {},
    \"gas\": \"30000000000000\",
    \"deposit\": \"$WRAP_AMOUNT\"
}")
echo "$WRAP_RESPONSE" | jq .

WRAP_STATUS=$(echo "$WRAP_RESPONSE" | jq -r '.status')
WRAP_TX=$(echo "$WRAP_RESPONSE" | jq -r '.tx_hash // "none"')
[ "$WRAP_STATUS" = "failed" ] && fail "Wrap failed! tx=$WRAP_TX"
ok "Wrapped. tx=$WRAP_TX"

sleep 2
log "Checking wNEAR balance..."
WNEAR_BALANCE=$(wallet_get "/wallet/v1/balance?chain=near&token=$WRAP_CONTRACT")
echo "$WNEAR_BALANCE" | jq .
fi

# =============================================================================
# Step 4: Swap wNEAR → USDT via Intents
# =============================================================================
if [ "$START_STEP" -le 4 ]; then
log "Step 4: Swapping wNEAR → USDT via /intents/swap..."

# Query actual wNEAR balance to swap (wrap may deduct storage fees)
WNEAR_BALANCE=$(wallet_get "/wallet/v1/balance?chain=near&token=$WRAP_CONTRACT" | jq -r '.balance // "0"')
[ "$WNEAR_BALANCE" = "0" ] && fail "No wNEAR balance to swap"
SWAP_AMOUNT_IN="$WNEAR_BALANCE"

echo "  token_in:  nep141:$WRAP_CONTRACT"
echo "  token_out: nep141:$USDT_CONTRACT"
echo "  amount:    $SWAP_AMOUNT_IN"
echo ""

SWAP_RESPONSE=$(wallet_post "/wallet/v1/intents/swap" "{
    \"token_in\": \"nep141:$WRAP_CONTRACT\",
    \"token_out\": \"nep141:$USDT_CONTRACT\",
    \"amount_in\": \"$SWAP_AMOUNT_IN\"
}")
echo "$SWAP_RESPONSE" | jq .

SWAP_STATUS=$(echo "$SWAP_RESPONSE" | jq -r '.status // "unknown"')
SWAP_AMOUNT_OUT=$(echo "$SWAP_RESPONSE" | jq -r '.amount_out // "0"')
SWAP_REQUEST_ID=$(echo "$SWAP_RESPONSE" | jq -r '.request_id // "none"')

if [ "$SWAP_STATUS" = "success" ]; then
    ok "Swap done! Got $SWAP_AMOUNT_OUT USDT"
elif [ "$SWAP_STATUS" = "processing" ]; then
    warn "Swap processing — check /wallet/v1/requests/$SWAP_REQUEST_ID"
else
    fail "Swap failed: $(echo "$SWAP_RESPONSE" | jq -c .)"
fi
fi

# =============================================================================
# Step 5: Send USDT to user via ft_transfer
# =============================================================================
if [ "$START_STEP" -le 5 ]; then

# Query actual USDT balance on the wallet account
sleep 2
USDT_AMOUNT=$(wallet_get "/wallet/v1/balance?chain=near&token=$USDT_CONTRACT" | jq -r '.balance // "0"')
log "Step 5: USDT balance: $USDT_AMOUNT (6 decimals)"

if [ -z "$USER_ACCOUNT" ]; then
    echo ""
    warn "No USER_ACCOUNT set — skipping USDT transfer."
    echo "  To send USDT, rerun with:"
    echo "  USER_ACCOUNT=you.near API_KEY=$API_KEY START_STEP=5 $0"
    echo ""
elif [ "$USDT_AMOUNT" = "0" ] || [ -z "$USDT_AMOUNT" ]; then
    warn "USDT balance is 0 — skipping transfer"
else
    log "Sending $USDT_AMOUNT USDT to $USER_ACCOUNT..."
    TRANSFER_RESPONSE=$(wallet_post "/wallet/v1/call" "{
        \"receiver_id\": \"$USDT_CONTRACT\",
        \"method_name\": \"ft_transfer\",
        \"args\": {\"receiver_id\": \"$USER_ACCOUNT\", \"amount\": \"$USDT_AMOUNT\"},
        \"gas\": \"30000000000000\",
        \"deposit\": \"1\"
    }")
    echo "$TRANSFER_RESPONSE" | jq .

    TRANSFER_STATUS=$(echo "$TRANSFER_RESPONSE" | jq -r '.status')
    TRANSFER_TX=$(echo "$TRANSFER_RESPONSE" | jq -r '.tx_hash // "none"')
    [ "$TRANSFER_STATUS" = "failed" ] && fail "ft_transfer failed! tx=$TRANSFER_TX"
    ok "USDT sent to $USER_ACCOUNT! tx=$TRANSFER_TX"
fi
fi

# =============================================================================
# Summary
# =============================================================================
echo ""
echo "============================================================"
echo -e "  ${GREEN}E2E Test Complete${NC}"
echo "============================================================"
echo "  Wallet:    $WALLET_ADDRESS"
echo "  API key:   ${API_KEY:0:12}..."
echo "  Swap:      ${SWAP_STATUS:-skipped} (${SWAP_AMOUNT_OUT:-0} USDT)"
[ -n "${TRANSFER_TX:-}" ] && echo "  Transfer:  $USDT_AMOUNT USDT → $USER_ACCOUNT (tx=$TRANSFER_TX)"
echo "============================================================"
echo ""
echo "  Resume: API_KEY=$API_KEY START_STEP=4 $0"
echo "  Audit:  curl -s '$BASE_URL/wallet/v1/audit?limit=10' -H 'Authorization: Bearer $API_KEY' | jq ."
echo ""
