#!/bin/bash
# ============================================================================
# Wallet E2E Test — NEAR deposit + /call + Intents withdraw
#
# Full flow:
#   1. setup    — register wallet, get implicit address
#   2. fund     — send NEAR to implicit address (manual step via near CLI)
#   3. policy   — set policy via dashboard (limits, whitelist, multisig)
#   4. call     — test /wallet/v1/call: wrap NEAR via wrap.near near_deposit
#   5. withdraw — test /wallet/v1/intents/withdraw: unwrap via intents ft_withdraw
#
# Usage:
#   # Step 1: Register wallet + get address
#   ./tests/wallet_intents_e2e.sh setup
#
#   # Step 2: Fund with NEAR (manual — use near CLI)
#   near send YOUR_ACCOUNT <implicit_address> 0.1 --networkId testnet
#
#   # Step 3 (optional): Set policy via dashboard
#   #   https://outlayer.fastnear.com/dashboard/wallet/manage
#
#   # Step 4: Test /call endpoint — wrap NEAR on implicit account
#   WALLET_API_KEY=wk_... ./tests/wallet_intents_e2e.sh call
#
#   # Step 5: Test intents withdraw (mainnet only — needs solver-relay)
#   WALLET_API_KEY=wk_... WITHDRAW_TO=receiver.near ./tests/wallet_intents_e2e.sh withdraw
#
# Environment:
#   COORDINATOR_URL  — default http://localhost:8080
#   WALLET_API_KEY   — API key from setup
#   NETWORK          — "testnet" or "mainnet" (default: testnet)
#   WRAP_CONTRACT    — wrap contract (default: wrap.testnet or wrap.near)
#   CALL_DEPOSIT     — NEAR to wrap in yocto (default: 10000000000000000000000 = 0.01 NEAR)
#   WITHDRAW_TO      — receiver account for withdraw
#   WITHDRAW_TOKEN   — token to withdraw (default: wrap contract)
#   WITHDRAW_AMOUNT  — amount in yocto (default: 1000000000000000000000 = 0.001 wNEAR)
# ============================================================================

set -euo pipefail

COORDINATOR_URL="${COORDINATOR_URL:-http://localhost:8080}"
NETWORK="${NETWORK:-testnet}"
YOUR_ACCOUNT="${YOUR_ACCOUNT:-YOUR_ACCOUNT.near}"

if [ "$NETWORK" = "mainnet" ]; then
    WRAP_CONTRACT="${WRAP_CONTRACT:-wrap.near}"
else
    WRAP_CONTRACT="${WRAP_CONTRACT:-wrap.testnet}"
fi

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
NC='\033[0m'

# ============================================================================
# Helpers
# ============================================================================

curl_get() {
    local path="$1"
    curl -s -w "\n%{http_code}" \
        -H "Authorization: Bearer $WALLET_API_KEY" \
        "${COORDINATOR_URL}${path}"
}

curl_post() {
    local path="$1"
    local body="$2"
    local idem_key="${3:-}"
    local extra_headers=()
    if [ -n "$idem_key" ]; then
        extra_headers+=(-H "X-Idempotency-Key: $idem_key")
    fi
    curl -s -w "\n%{http_code}" \
        -X POST \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer $WALLET_API_KEY" \
        ${extra_headers[@]+"${extra_headers[@]}"} \
        -d "$body" \
        "${COORDINATOR_URL}${path}"
}

parse_response() {
    local response="$1"
    RESP_BODY=$(echo "$response" | sed '$d')
    RESP_CODE=$(echo "$response" | tail -1)
}

check_coordinator() {
    if ! curl -s "${COORDINATOR_URL}/health" > /dev/null 2>&1; then
        echo -e "${RED}ERROR: Coordinator not running at ${COORDINATOR_URL}${NC}"
        exit 1
    fi
}

# ============================================================================
# Command: setup
# ============================================================================

cmd_setup() {
    echo ""
    echo -e "${CYAN}=============================================${NC}"
    echo -e "${CYAN} Wallet E2E — Setup (network: $NETWORK)${NC}"
    echo -e "${CYAN}=============================================${NC}"
    echo ""

    check_coordinator

    if [ -n "${WALLET_API_KEY:-}" ]; then
        echo "Using existing API key: ${WALLET_API_KEY:0:10}..."
        echo ""
    else
        echo "Registering new wallet..."
        RESPONSE=$(curl -s -w "\n%{http_code}" -X POST "${COORDINATOR_URL}/register")
        parse_response "$RESPONSE"

        if [ "$RESP_CODE" != "200" ]; then
            echo -e "${RED}FAIL: register returned HTTP $RESP_CODE${NC}"
            echo "$RESP_BODY"
            exit 1
        fi

        WALLET_API_KEY=$(echo "$RESP_BODY" | jq -r '.api_key')
        WALLET_ID=$(echo "$RESP_BODY" | jq -r '.wallet_id')
        NEAR_ADDRESS=$(echo "$RESP_BODY" | jq -r '.near_account_id')

        echo -e "Wallet ID: ${GREEN}$WALLET_ID${NC}"
        echo -e "API Key:   ${YELLOW}$WALLET_API_KEY${NC}"
        echo ""
    fi

    # Get NEAR address (if reusing existing key)
    if [ -z "${NEAR_ADDRESS:-}" ]; then
        RESPONSE=$(curl_get "/wallet/v1/address?chain=near")
        parse_response "$RESPONSE"

        if [ "$RESP_CODE" != "200" ]; then
            echo -e "${RED}Failed to get wallet address (HTTP $RESP_CODE):${NC}"
            echo "$RESP_BODY"
            exit 1
        fi

        NEAR_ADDRESS=$(echo "$RESP_BODY" | jq -r '.address')
    fi

    echo -e "NEAR address (implicit): ${GREEN}$NEAR_ADDRESS${NC}"
    echo ""
    echo "============================================="
    echo " Next steps:"
    echo ""
    echo " 1. Send NEAR to the implicit address (for gas + wrapping):"
    echo ""
    echo "    near send ${YOUR_ACCOUNT} $NEAR_ADDRESS 0.1 --networkId $NETWORK"
    echo ""
    echo " 2. (Optional) Set a policy via dashboard:"
    echo ""
    echo "    https://outlayer.fastnear.com/wallet?key=$WALLET_API_KEY"
    echo ""
    echo " 3. Get wallet address:"
    echo ""
    echo "    curl -s -H 'Authorization: Bearer $WALLET_API_KEY' '${COORDINATOR_URL}/wallet/v1/address?chain=near' | jq ."
    echo ""
    echo " 4. Call contract (wrap NEAR via $WRAP_CONTRACT near_deposit):"
    echo ""
    echo "    curl -s -X POST -H 'Content-Type: application/json' -H 'Authorization: Bearer $WALLET_API_KEY' \\"
    echo "      -d '{\"receiver_id\":\"$WRAP_CONTRACT\",\"method_name\":\"near_deposit\",\"args\":{},\"deposit\":\"10000000000000000000000\"}' \\"
    echo "      '${COORDINATOR_URL}/wallet/v1/call' | jq ."
    echo ""
    echo " 5. Check available tokens:"
    echo ""
    echo "    curl -s -H 'Authorization: Bearer $WALLET_API_KEY' '${COORDINATOR_URL}/wallet/v1/tokens' | jq ."
    echo ""
    echo " 6. Withdraw via intents (dry-run):"
    echo ""
    echo "    curl -s -X POST -H 'Content-Type: application/json' -H 'Authorization: Bearer $WALLET_API_KEY' \\"
    echo "      -d '{\"to\":\"${YOUR_ACCOUNT}\",\"amount\":\"1000000000000000000000\",\"token\":\"$WRAP_CONTRACT\",\"chain\":\"near\"}' \\"
    echo "      '${COORDINATOR_URL}/wallet/v1/intents/withdraw/dry-run' | jq ."
    echo ""
    echo " 7. Withdraw via intents (real):"
    echo ""
    echo "    curl -s -X POST -H 'Content-Type: application/json' -H 'Authorization: Bearer $WALLET_API_KEY' \\"
    echo "      -d '{\"to\":\"${YOUR_ACCOUNT}\",\"amount\":\"1000000000000000000000\",\"token\":\"$WRAP_CONTRACT\",\"chain\":\"near\"}' \\"
    echo "      '${COORDINATOR_URL}/wallet/v1/intents/withdraw' | jq ."
    echo ""
    echo " 8. Check request status (replace REQUEST_ID):"
    echo ""
    echo "    curl -s -H 'Authorization: Bearer $WALLET_API_KEY' '${COORDINATOR_URL}/wallet/v1/requests/REQUEST_ID' | jq ."
    echo ""
    echo " 9. Get audit log:"
    echo ""
    echo "    curl -s -H 'Authorization: Bearer $WALLET_API_KEY' '${COORDINATOR_URL}/wallet/v1/audit?limit=50' | jq ."
    echo "============================================="
    echo ""
}

# ============================================================================
# Command: call — test /wallet/v1/call (wrap NEAR via near_deposit)
# ============================================================================

cmd_call() {
    echo ""
    echo -e "${CYAN}=============================================${NC}"
    echo -e "${CYAN} Wallet E2E — Call Test (network: $NETWORK)${NC}"
    echo -e "${CYAN}=============================================${NC}"
    echo ""

    check_coordinator

    WALLET_API_KEY="${WALLET_API_KEY:?Set WALLET_API_KEY=wk_... (from setup output)}"
    CALL_DEPOSIT="${CALL_DEPOSIT:-10000000000000000000000}"  # 0.01 NEAR

    echo "API Key: ${WALLET_API_KEY:0:10}..."
    echo "Wrap contract: $WRAP_CONTRACT"
    echo "Deposit: $CALL_DEPOSIT yoctoNEAR"
    echo ""

    # 1. Get wallet address + check balance
    echo -e "${CYAN}[1/4] Getting wallet address...${NC}"
    RESPONSE=$(curl_get "/wallet/v1/address?chain=near")
    parse_response "$RESPONSE"

    if [ "$RESP_CODE" != "200" ]; then
        echo -e "${RED}FAIL: get address returned HTTP $RESP_CODE${NC}"
        echo "$RESP_BODY"
        exit 1
    fi

    NEAR_ADDRESS=$(echo "$RESP_BODY" | jq -r '.address')
    echo -e "  ${GREEN}OK${NC} Address: $NEAR_ADDRESS"

    # 2. Check NEAR balance via RPC
    echo -e "${CYAN}[2/4] Checking NEAR balance...${NC}"
    if [ "$NETWORK" = "mainnet" ]; then
        RPC_URL="https://rpc.mainnet.near.org"
    else
        RPC_URL="https://rpc.testnet.near.org"
    fi

    BALANCE_RESP=$(curl -s "$RPC_URL" \
        -H 'Content-Type: application/json' \
        -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"query\",\"params\":{\"request_type\":\"view_account\",\"finality\":\"final\",\"account_id\":\"$NEAR_ADDRESS\"}}")

    BALANCE=$(echo "$BALANCE_RESP" | jq -r '.result.amount // "0"')
    if [ "$BALANCE" = "0" ] || echo "$BALANCE_RESP" | jq -e '.error' > /dev/null 2>&1; then
        echo -e "  ${RED}Account not found or zero balance${NC}"
        echo "  Fund it first:"
        echo "    near send ${YOUR_ACCOUNT} $NEAR_ADDRESS 0.1 --networkId $NETWORK"
        exit 1
    fi

    # Convert yoctoNEAR to NEAR for display (rough: drop last 24 digits)
    BALANCE_NEAR=$(echo "scale=4; $BALANCE / 1000000000000000000000000" | bc 2>/dev/null || echo "$BALANCE yocto")
    echo -e "  ${GREEN}OK${NC} Balance: $BALANCE_NEAR NEAR"

    # 3. Call wrap.near near_deposit (wraps NEAR into wNEAR)
    echo -e "${CYAN}[3/4] Calling $WRAP_CONTRACT.near_deposit (wrapping NEAR)...${NC}"
    echo "  Method: near_deposit"
    echo "  Deposit: $CALL_DEPOSIT yoctoNEAR"
    echo ""

    IDEM_KEY="call-wrap-$(date +%s)-$$"
    BODY=$(jq -n \
        --arg receiver "$WRAP_CONTRACT" \
        --arg deposit "$CALL_DEPOSIT" \
        '{
            receiver_id: $receiver,
            method_name: "near_deposit",
            args: {},
            deposit: $deposit
        }')

    RESPONSE=$(curl_post "/wallet/v1/call" "$BODY" "$IDEM_KEY")
    parse_response "$RESPONSE"

    echo "  HTTP: $RESP_CODE"
    echo "  Response:"
    echo "$RESP_BODY" | jq '.'
    echo ""

    if [ "$RESP_CODE" != "200" ]; then
        echo -e "${RED}FAIL: /call returned HTTP $RESP_CODE${NC}"
        echo ""
        echo "Possible reasons:"
        echo "  - Implicit account not funded (run 'setup' + send NEAR)"
        echo "  - Insufficient balance for deposit + gas"
        echo "  - Keystore signing error"
        exit 1
    fi

    REQUEST_ID=$(echo "$RESP_BODY" | jq -r '.request_id')
    STATUS=$(echo "$RESP_BODY" | jq -r '.status')
    TX_HASH=$(echo "$RESP_BODY" | jq -r '.tx_hash // "none"')

    echo -e "  Request ID: $REQUEST_ID"
    echo -e "  Status: ${GREEN}$STATUS${NC}"
    if [ "$TX_HASH" != "none" ] && [ "$TX_HASH" != "null" ]; then
        if [ "$NETWORK" = "mainnet" ]; then
            echo -e "  Explorer: ${CYAN}https://nearblocks.io/txns/$TX_HASH${NC}"
        else
            echo -e "  Explorer: ${CYAN}https://testnet.nearblocks.io/txns/$TX_HASH${NC}"
        fi
    fi

    # 4. Verify wNEAR balance on wrap contract
    echo ""
    echo -e "${CYAN}[4/4] Checking wNEAR balance on $WRAP_CONTRACT...${NC}"
    WNEAR_RESP=$(curl -s "$RPC_URL" \
        -H 'Content-Type: application/json' \
        -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"query\",\"params\":{\"request_type\":\"call_function\",\"finality\":\"final\",\"account_id\":\"$WRAP_CONTRACT\",\"method_name\":\"ft_balance_of\",\"args_base64\":\"$(echo -n "{\"account_id\":\"$NEAR_ADDRESS\"}" | base64)\"}}")

    WNEAR_RAW=$(echo "$WNEAR_RESP" | jq -r '.result.result // []' | jq -r 'implode' 2>/dev/null || echo "error")
    WNEAR_BALANCE=$(echo "$WNEAR_RAW" | tr -d '"' 2>/dev/null || echo "0")

    if [ -n "$WNEAR_BALANCE" ] && [ "$WNEAR_BALANCE" != "error" ] && [ "$WNEAR_BALANCE" != "0" ]; then
        WNEAR_NEAR=$(echo "scale=4; $WNEAR_BALANCE / 1000000000000000000000000" | bc 2>/dev/null || echo "$WNEAR_BALANCE yocto")
        echo -e "  ${GREEN}OK${NC} wNEAR balance: $WNEAR_NEAR"
    else
        echo -e "  ${YELLOW}WARN${NC} Could not read wNEAR balance (may need storage deposit first)"
    fi

    echo ""
    echo "============================================="
    if [ "$STATUS" = "success" ]; then
        echo -e "${GREEN} CALL SUCCEEDED — NEAR wrapped into wNEAR${NC}"
        echo ""
        echo " Now you can test intents withdraw:"
        echo "   WALLET_API_KEY=$WALLET_API_KEY WITHDRAW_TO=${YOUR_ACCOUNT} ./tests/wallet_intents_e2e.sh withdraw"
    elif [ "$STATUS" = "pending_approval" ]; then
        echo -e "${YELLOW} CALL REQUIRES APPROVAL (check /wallet/v1/pending_approvals)${NC}"
    else
        echo -e "${YELLOW} CALL STATUS: $STATUS${NC}"
    fi
    echo "============================================="
    echo ""
}

# ============================================================================
# Command: withdraw — intents ft_withdraw
# ============================================================================

cmd_withdraw() {
    echo ""
    echo -e "${CYAN}=============================================${NC}"
    echo -e "${CYAN} Wallet E2E — Withdraw Test (network: $NETWORK)${NC}"
    echo -e "${CYAN}=============================================${NC}"
    echo ""

    check_coordinator

    WALLET_API_KEY="${WALLET_API_KEY:?Set WALLET_API_KEY=wk_... (from setup output)}"
    WITHDRAW_TO="${WITHDRAW_TO:?Set WITHDRAW_TO=receiver_account.near}"
    WITHDRAW_TOKEN="${WITHDRAW_TOKEN:-$WRAP_CONTRACT}"
    WITHDRAW_AMOUNT="${WITHDRAW_AMOUNT:-1000000000000000000000}"

    echo "API Key: ${WALLET_API_KEY:0:10}..."
    echo ""

    # 1. Get wallet address
    echo -e "${CYAN}[1/5] Getting wallet address...${NC}"
    RESPONSE=$(curl_get "/wallet/v1/address?chain=near")
    parse_response "$RESPONSE"

    if [ "$RESP_CODE" != "200" ]; then
        echo -e "${RED}FAIL: get address returned HTTP $RESP_CODE${NC}"
        echo "$RESP_BODY"
        exit 1
    fi

    NEAR_ADDRESS=$(echo "$RESP_BODY" | jq -r '.address')
    echo -e "  ${GREEN}OK${NC} Address: $NEAR_ADDRESS"

    # 2. Check tokens
    echo -e "${CYAN}[2/5] Checking available tokens...${NC}"
    RESPONSE=$(curl_get "/wallet/v1/tokens")
    parse_response "$RESPONSE"

    if [ "$RESP_CODE" != "200" ]; then
        echo -e "${RED}FAIL: tokens returned HTTP $RESP_CODE${NC}"
        exit 1
    fi

    TOKEN_COUNT=$(echo "$RESP_BODY" | jq -r '.tokens | length')
    echo -e "  ${GREEN}OK${NC} $TOKEN_COUNT tokens available"

    # 3. Dry-run withdraw
    echo -e "${CYAN}[3/5] Dry-run withdraw...${NC}"
    BODY="{\"to\":\"$WITHDRAW_TO\",\"amount\":\"$WITHDRAW_AMOUNT\",\"token\":\"$WITHDRAW_TOKEN\",\"chain\":\"near\"}"
    RESPONSE=$(curl_post "/wallet/v1/intents/withdraw/dry-run" "$BODY" "dry-run-$(date +%s)")
    parse_response "$RESPONSE"

    echo -e "  HTTP $RESP_CODE"
    echo "  $(echo "$RESP_BODY" | jq -c '.')"

    # 4. Real withdraw
    echo -e "${CYAN}[4/5] Executing real withdraw...${NC}"
    echo "  To: $WITHDRAW_TO"
    echo "  Token: $WITHDRAW_TOKEN"
    echo "  Amount: $WITHDRAW_AMOUNT"
    echo "  (this may take up to 30 seconds — polling settlement)..."
    echo ""

    IDEM_KEY="intents-e2e-$(date +%s)-$$"
    RESPONSE=$(curl_post "/wallet/v1/intents/withdraw" "$BODY" "$IDEM_KEY")
    parse_response "$RESPONSE"

    echo "  HTTP: $RESP_CODE"
    echo "  Response:"
    echo "$RESP_BODY" | jq '.'
    echo ""

    if [ "$RESP_CODE" != "200" ]; then
        echo -e "${RED}FAIL: withdraw returned HTTP $RESP_CODE${NC}"
        echo ""
        echo "Possible reasons:"
        echo "  - Wallet has no tokens in intents.near (deposit wNEAR first)"
        echo "  - Solver-relay rejected the intent"
        echo "  - Keystore signing error"
        exit 1
    fi

    REQUEST_ID=$(echo "$RESP_BODY" | jq -r '.request_id')
    STATUS=$(echo "$RESP_BODY" | jq -r '.status')
    echo -e "  Request ID: $REQUEST_ID"
    echo -e "  Status: ${GREEN}$STATUS${NC}"

    # 5. Check request status
    echo -e "${CYAN}[5/5] Checking request status...${NC}"
    RESPONSE=$(curl_get "/wallet/v1/requests/$REQUEST_ID")
    parse_response "$RESPONSE"

    if [ "$RESP_CODE" = "200" ]; then
        FINAL_STATUS=$(echo "$RESP_BODY" | jq -r '.status')
        echo -e "  ${GREEN}OK${NC} Status: $FINAL_STATUS"
        echo "  Full response:"
        echo "$RESP_BODY" | jq '.'
    else
        echo -e "  ${YELLOW}WARN${NC} Could not fetch request status (HTTP $RESP_CODE)"
    fi

    echo ""
    echo "============================================="
    if [ "$STATUS" = "success" ]; then
        echo -e "${GREEN} WITHDRAW SETTLED SUCCESSFULLY${NC}"
    elif [ "$STATUS" = "processing" ]; then
        echo -e "${YELLOW} WITHDRAW PUBLISHED (still processing — may settle later)${NC}"
    else
        echo -e "${YELLOW} WITHDRAW STATUS: $STATUS${NC}"
    fi
    echo "============================================="
    echo ""
}

# ============================================================================
# Command: status — check existing request
# ============================================================================

cmd_status() {
    REQUEST_ID="${1:?Usage: $0 status <request_id>}"

    check_coordinator
    WALLET_API_KEY="${WALLET_API_KEY:?Set WALLET_API_KEY=wk_...}"

    echo ""
    echo -e "${CYAN}Checking request status: $REQUEST_ID${NC}"

    RESPONSE=$(curl_get "/wallet/v1/requests/$REQUEST_ID")
    parse_response "$RESPONSE"

    if [ "$RESP_CODE" = "200" ]; then
        echo "$RESP_BODY" | jq '.'
    else
        echo -e "${RED}HTTP $RESP_CODE${NC}"
        echo "$RESP_BODY"
    fi
}

# ============================================================================
# Main
# ============================================================================

COMMAND="${1:-help}"
shift || true

case "$COMMAND" in
    setup)
        cmd_setup
        ;;
    call)
        cmd_call
        ;;
    withdraw)
        cmd_withdraw
        ;;
    status)
        cmd_status "$@"
        ;;
    *)
        echo "Usage: $0 <command>"
        echo ""
        echo "Commands:"
        echo "  setup     — register wallet, show address + funding instructions"
        echo "  call      — wrap NEAR via /call (near_deposit on wrap contract)"
        echo "  withdraw  — unwrap wNEAR via intents /withdraw"
        echo "  status    — check request status by ID"
        echo ""
        echo "Full flow:  setup → fund via near CLI → [policy] → call → withdraw"
        echo ""
        echo "Environment:"
        echo "  WALLET_API_KEY   — reuse wallet key across runs (from setup output)"
        echo "  NETWORK          — testnet (default) or mainnet"
        echo "  COORDINATOR_URL  — default http://localhost:8080"
        echo "  CALL_DEPOSIT     — NEAR to wrap in yocto (default: 0.01 NEAR)"
        echo "  WITHDRAW_TO      — receiver NEAR account"
        echo "  WITHDRAW_TOKEN   — token ID (default: wrap contract)"
        echo "  WITHDRAW_AMOUNT  — amount in yocto (default: 0.001 wNEAR)"
        ;;
esac
