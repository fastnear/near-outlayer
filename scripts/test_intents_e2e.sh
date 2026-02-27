#!/bin/bash
# =============================================================================
# E2E Test: Custody Wallet → wNEAR → USDT via Intents → Send back
#
# Flow:
#   1. Register wallet (or use existing)
#   2. Get NEAR address
#   3. Fund wallet with NEAR (manual step)
#   4. Wrap NEAR → wNEAR
#   5. Register USDT storage (required for swap pre-flight)
#   6. Swap wNEAR → USDT (deposit → token_diff → settle → withdraw)
#   7. Check USDT balance
#   8. Send USDT to your account
#
# Usage:
#   ./test_intents_e2e.sh                          # Full flow from scratch
#   API_KEY=wk_xxx ./test_intents_e2e.sh           # Use existing wallet
#   BASE_URL=http://localhost:8080 ./test_intents_e2e.sh  # Local coordinator
#   API_KEY=wk_xxx START_STEP=4 ./test_intents_e2e.sh    # Resume from step 4
# =============================================================================

set -euo pipefail

# ---- Configuration ----
BASE_URL="${BASE_URL:-https://api.outlayer.fastnear.com}"
API_KEY="${API_KEY:-}"
USER_ACCOUNT="${USER_ACCOUNT:-}"      # Your NEAR account to receive USDT at the end
WRAP_AMOUNT="${WRAP_AMOUNT:-5000000000000000000000}"  # 0.005 NEAR in yoctoNEAR (keep rest for gas)
START_STEP="${START_STEP:-1}"       # Resume from this step (1-8)
USDT_CONTRACT="usdt.tether-token.near"
WRAP_CONTRACT="wrap.near"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log()  { echo -e "${BLUE}[INFO]${NC} $*"; }
ok()   { echo -e "${GREEN}[OK]${NC} $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
fail() { echo -e "${RED}[FAIL]${NC} $*" >&2; exit 1; }

# Check dependencies
command -v curl >/dev/null || fail "curl is required"
command -v jq >/dev/null || fail "jq is required"

# Helper: curl wrapper that shows errors instead of dying silently
# Writes response body to _BODY_FILE, HTTP status to _STATUS_FILE
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
    curl_check "$BASE_URL$path" \
        -H "Authorization: Bearer $API_KEY"
    local status; status=$(http_status)
    local response; response=$(http_body)
    if [ "$status" -ge 400 ]; then
        fail "GET $path returned HTTP $status: $response"
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
        fail "POST $path returned HTTP $status: $response"
    fi
    echo "$response"
}

# Non-fatal POST (for steps that might fail)
wallet_post_nofail() {
    local path="$1"
    local body="$2"
    local idem_key="${3:-$(uuidgen 2>/dev/null || cat /proc/sys/kernel/random/uuid 2>/dev/null || echo $RANDOM)}"
    curl_check "$BASE_URL$path" \
        -H "Authorization: Bearer $API_KEY" \
        -H "Content-Type: application/json" \
        -H "X-Idempotency-Key: $idem_key" \
        -d "$body"
    http_body
}

echo ""
echo "============================================================"
echo "  NEAR Intents E2E Test"
echo "  Base URL: $BASE_URL"
echo "============================================================"
echo ""

# =============================================================================
# Steps 1-2 always run (needed to resolve WALLET_ADDRESS)
# =============================================================================

# Step 1: Register or use existing wallet
if [ -z "$API_KEY" ]; then
    log "Step 1: Registering new wallet..."
    curl_check -X POST "$BASE_URL/register"
    if [ "$(http_status)" -ge 400 ]; then
        fail "POST /register returned HTTP $(http_status): $(http_body)"
    fi
    REGISTER_RESPONSE=$(http_body)
    echo "$REGISTER_RESPONSE" | jq .

    API_KEY=$(echo "$REGISTER_RESPONSE" | jq -r '.api_key')
    WALLET_ID=$(echo "$REGISTER_RESPONSE" | jq -r '.wallet_id')

    if [ -z "$API_KEY" ] || [ "$API_KEY" = "null" ]; then
        fail "Failed to register wallet"
    fi
    ok "Wallet registered: $WALLET_ID"
    ok "API key: $API_KEY"
    echo ""
    warn "SAVE THIS API KEY! You'll need it to continue."
    echo ""
else
    log "Step 1: Using existing wallet with API_KEY=${API_KEY:0:12}..."
fi

# Step 2: Get NEAR address (always needed)
log "Step 2: Getting wallet NEAR address..."
ADDRESS_RESPONSE=$(wallet_get "/wallet/v1/address?chain=near")
echo "$ADDRESS_RESPONSE" | jq .

WALLET_ADDRESS=$(echo "$ADDRESS_RESPONSE" | jq -r '.address')
if [ -z "$WALLET_ADDRESS" ] || [ "$WALLET_ADDRESS" = "null" ]; then
    fail "Could not get wallet address"
fi
ok "Wallet NEAR address: $WALLET_ADDRESS"

if [ "$START_STEP" -gt 1 ]; then
    log "Resuming from step $START_STEP..."
fi

# =============================================================================
# Step 3: Check NEAR balance (fund if needed)
# =============================================================================
if [ "$START_STEP" -le 3 ]; then
log "Step 3: Checking NEAR balance..."
BALANCE_RESPONSE=$(wallet_get "/wallet/v1/balance?chain=near")
echo "$BALANCE_RESPONSE" | jq .

NEAR_BALANCE=$(echo "$BALANCE_RESPONSE" | jq -r '.balance // "0"')
ok "NEAR balance: $NEAR_BALANCE yoctoNEAR"

# Check if balance is enough (at least 0.1 NEAR = 100000000000000000000000)
MIN_BALANCE="100000000000000000000000"
if python3 -c "exit(0 if int('${NEAR_BALANCE}') < int('${MIN_BALANCE}') else 1)" 2>/dev/null; then
    echo ""
    warn "Wallet has insufficient NEAR balance!"
    warn "Send at least 0.1 NEAR to: $WALLET_ADDRESS"
    echo ""
    echo "  Option 1 — Dashboard fund link (recommended):"
    echo "    https://outlayer.fastnear.com/wallet/fund?to=${WALLET_ADDRESS}&amount=0.1&token=near&msg=Fund%20E2E%20test%20wallet"
    echo ""
    echo "  Option 2 — near-cli:"
    echo "    near send <your-account>.near $WALLET_ADDRESS 0.1"
    echo ""
    read -p "Press Enter after funding the wallet (or Ctrl+C to abort)..."
    echo ""

    log "Re-checking balance..."
    BALANCE_RESPONSE=$(wallet_get "/wallet/v1/balance?chain=near")
    echo "$BALANCE_RESPONSE" | jq .
fi
fi

# =============================================================================
# Step 4: Wrap NEAR → wNEAR
# =============================================================================
if [ "$START_STEP" -le 4 ]; then
log "Step 4: Wrapping NEAR → wNEAR ($WRAP_AMOUNT yocto)..."
echo "  Calling wrap.near / near_deposit with deposit=$WRAP_AMOUNT"

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
if [ "$WRAP_STATUS" = "failed" ]; then
    fail "Wrap failed! tx=$WRAP_TX"
fi
ok "Wrapped NEAR → wNEAR. tx=$WRAP_TX"

# Check wNEAR balance
log "Checking wNEAR balance..."
WNEAR_BALANCE=$(wallet_get "/wallet/v1/balance?chain=near&token=$WRAP_CONTRACT")
echo "$WNEAR_BALANCE" | jq .
fi

# =============================================================================
# Step 5: Register storage for USDT on wallet account
# =============================================================================
if [ "$START_STEP" -le 5 ]; then
log "Step 5: Registering storage for USDT on wallet (required for swap)..."
echo "  Calling $USDT_CONTRACT / storage_deposit (0.00125 NEAR)"

STORAGE_RESPONSE=$(wallet_post "/wallet/v1/call" "{
    \"receiver_id\": \"$USDT_CONTRACT\",
    \"method_name\": \"storage_deposit\",
    \"args\": {\"account_id\": \"$WALLET_ADDRESS\", \"registration_only\": true},
    \"gas\": \"30000000000000\",
    \"deposit\": \"1250000000000000000000\"
}")
echo "$STORAGE_RESPONSE" | jq .
ok "USDT storage registered"
fi

# =============================================================================
# Step 6: Swap wNEAR → USDT
# =============================================================================
if [ "$START_STEP" -le 6 ]; then
log "Step 6: Swapping wNEAR → USDT via Intents..."
echo "  token_in:  nep141:$WRAP_CONTRACT"
echo "  token_out: nep141:$USDT_CONTRACT"
echo "  amount_in: $WRAP_AMOUNT"
echo ""
echo "  This will:"
echo "    1. Get 1Click quote"
echo "    2. Deposit wNEAR to intents.near"
echo "    3. mt_transfer to 1Click deposit address"
echo "    4. Poll 1Click status until settled"
echo ""

SWAP_RESPONSE=$(wallet_post "/wallet/v1/swap" "{
    \"token_in\": \"nep141:$WRAP_CONTRACT\",
    \"token_out\": \"nep141:$USDT_CONTRACT\",
    \"amount_in\": \"$WRAP_AMOUNT\"
}")
echo "$SWAP_RESPONSE" | jq .

SWAP_STATUS=$(echo "$SWAP_RESPONSE" | jq -r '.status // "unknown"')
SWAP_AMOUNT_OUT=$(echo "$SWAP_RESPONSE" | jq -r '.amount_out // "0"')
SWAP_INTENT=$(echo "$SWAP_RESPONSE" | jq -r '.intent_hash // "none"')
SWAP_REQUEST_ID=$(echo "$SWAP_RESPONSE" | jq -r '.request_id // "none"')

if [ "$SWAP_STATUS" = "success" ]; then
    ok "Swap succeeded!"
    ok "  Amount out: $SWAP_AMOUNT_OUT USDT (6 decimals)"
    ok "  Intent hash: $SWAP_INTENT"
elif [ "$SWAP_STATUS" = "processing" ]; then
    warn "Swap is still processing (intent may settle later)"
    warn "  Intent hash: $SWAP_INTENT"
    warn "  Check status: GET /wallet/v1/requests/$SWAP_REQUEST_ID"
else
    fail "Swap failed! Response: $(echo "$SWAP_RESPONSE" | jq -c .)"
fi
fi

# =============================================================================
# Step 7: Check USDT balance
# =============================================================================
if [ "$START_STEP" -le 7 ]; then
log "Step 7: Checking USDT balance..."
sleep 2  # Wait a moment for state to propagate
USDT_BALANCE=$(wallet_get "/wallet/v1/balance?chain=near&token=$USDT_CONTRACT")
echo "$USDT_BALANCE" | jq .

USDT_AMOUNT=$(echo "$USDT_BALANCE" | jq -r '.balance // "0"')
ok "USDT balance: $USDT_AMOUNT (6 decimals)"
fi

# =============================================================================
# Step 8: Send USDT to user's account
# =============================================================================
if [ -z "$USER_ACCOUNT" ]; then
    echo ""
    warn "No USER_ACCOUNT set. Skipping USDT transfer."
    echo "  To send USDT to your account, run:"
    echo "  USER_ACCOUNT=your-account.near API_KEY=$API_KEY $0"
    echo ""
    echo "  Or manually:"
    echo "  curl -X POST $BASE_URL/wallet/v1/call \\"
    echo "    -H 'Authorization: Bearer $API_KEY' \\"
    echo "    -H 'Content-Type: application/json' \\"
    echo "    -d '{\"receiver_id\": \"$USDT_CONTRACT\", \"method_name\": \"ft_transfer\","
    echo "         \"args\": {\"receiver_id\": \"YOUR_ACCOUNT.near\", \"amount\": \"$USDT_AMOUNT\"},"
    echo "         \"deposit\": \"1\"}'"
    echo ""
else
    if [ "$USDT_AMOUNT" = "0" ] || [ -z "$USDT_AMOUNT" ]; then
        warn "USDT balance is 0 — skipping transfer"
    else
    log "Step 8: Sending $USDT_AMOUNT USDT to $USER_ACCOUNT..."

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
    if [ "$TRANSFER_STATUS" = "failed" ]; then
        fail "USDT transfer failed! tx=$TRANSFER_TX"
    fi
    ok "USDT sent to $USER_ACCOUNT! tx=$TRANSFER_TX"
fi # balance > 0
fi # USER_ACCOUNT

# =============================================================================
# Summary
# =============================================================================
echo ""
echo "============================================================"
echo "  E2E Test Summary"
echo "============================================================"
echo "  Wallet address:  $WALLET_ADDRESS"
echo "  API key:         ${API_KEY:0:12}..."
echo "  Swap status:     ${SWAP_STATUS:-skipped}"
echo "  USDT received:   ${SWAP_AMOUNT_OUT:-n/a}"
if [ -n "$USER_ACCOUNT" ]; then
echo "  Sent to:         $USER_ACCOUNT"
fi
echo "============================================================"
echo ""
echo "  Resume:"
echo "    API_KEY=$API_KEY BASE_URL=$BASE_URL START_STEP=4 $0"
echo ""
echo "  Useful commands:"
echo "    Check balance: curl -s '$BASE_URL/wallet/v1/balance?chain=near&token=$USDT_CONTRACT' -H 'Authorization: Bearer $API_KEY' | jq ."
echo "    Check request: curl -s '$BASE_URL/wallet/v1/requests/${SWAP_REQUEST_ID:-none}' -H 'Authorization: Bearer $API_KEY' | jq ."
echo "    Audit log:     curl -s '$BASE_URL/wallet/v1/audit?limit=10' -H 'Authorization: Bearer $API_KEY' | jq ."
echo ""
