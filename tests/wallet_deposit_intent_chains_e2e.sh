#!/bin/bash
# ============================================================================
# Wallet E2E — /wallet/v1/deposit-intent chain matrix  (issue #25 Bug A)
#
# Regression test for the bug Kirill reported:
#   - `chain=near` was rejected outright;
#   - the `{source_asset, destination_asset}` shape always returned Solana
#     base58 deposit addresses regardless of source chain.
#
# Read-only: registers a wallet, calls `/wallet/v1/deposit-intent` once per
# source chain, asserts the returned `deposit_address` has the correct
# address shape for that chain. No funds move, no on-chain tx.
#
# Usage:
#   ./tests/wallet_deposit_intent_chains_e2e.sh
#
# Environment:
#   COORDINATOR_URL  — default https://api.outlayer.fastnear.com
#   WALLET_API_KEY   — reuse existing key (optional; new wallet registered if absent)
#   CHAINS           — comma-separated list (default: all six below)
#   DEST_ASSET       — destination intents asset (default: USDC NEP-141 on NEAR)
#   VERBOSE          — set to 1 for full response bodies
# ============================================================================
set -euo pipefail

COORDINATOR_URL="${COORDINATOR_URL:-https://api.outlayer.fastnear.com}"
CHAINS="${CHAINS:-near,ethereum,base,arbitrum,solana,bitcoin}"
DEST_ASSET="${DEST_ASSET:-nep141:17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1}"
VERBOSE="${VERBOSE:-0}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

# ─── Source asset (USDC variant) per chain, sourced from the 1Click catalog ──
source_asset_for_chain() {
    case "$1" in
        near)     echo "nep141:17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1" ;;
        ethereum) echo "nep141:eth-0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48.omft.near" ;;
        base)     echo "nep141:base-0x833589fcd6edb6e08f4c7c32d4f71b54bda02913.omft.near" ;;
        arbitrum) echo "nep141:arb-0xaf88d065e77c8cc2239327c5edb3a432268e5831.omft.near" ;;
        solana)   echo "nep141:sol-EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v.omft.near" ;;
        bitcoin)  echo "nep141:btc.omft.near" ;;  # BTC — no USDC on Bitcoin
        *)        echo "" ;;
    esac
}

# ─── Expected address shape per chain ────────────────────────────────────────
expected_format_label() {
    case "$1" in
        near)              echo "64-char hex (NEAR implicit)" ;;
        ethereum|base|arbitrum) echo "0x + 40 hex (EVM)" ;;
        solana)            echo "base58, 32-44 chars (Solana)" ;;
        bitcoin)           echo "bc1…/1…/3… (Bitcoin)" ;;
        *)                 echo "?" ;;
    esac
}

# Validate a `deposit_address` string against the source chain's expected shape.
# Returns 0 on match, 1 on mismatch.
address_matches_chain() {
    local chain="$1"; local addr="$2"
    case "$chain" in
        near)
            [[ "$addr" =~ ^[0-9a-f]{64}$ ]] ;;
        ethereum|base|arbitrum)
            [[ "$addr" =~ ^0x[0-9a-fA-F]{40}$ ]] ;;
        solana)
            # base58: 32-44 chars, no 0/O/I/l, alphanumeric
            [[ ${#addr} -ge 32 && ${#addr} -le 44 && "$addr" =~ ^[1-9A-HJ-NP-Za-km-z]+$ ]] ;;
        bitcoin)
            [[ "$addr" =~ ^bc1 || "$addr" =~ ^[13] ]] ;;
        *) return 1 ;;
    esac
}

# ─── Coordinator health ──────────────────────────────────────────────────────
echo ""
echo -e "${CYAN}=================================================${NC}"
echo -e "${CYAN} /wallet/v1/deposit-intent chain matrix         ${NC}"
echo -e "${CYAN} Target: ${COORDINATOR_URL}${NC}"
echo -e "${CYAN}=================================================${NC}"

if ! curl -s --max-time 10 "${COORDINATOR_URL}/health" > /dev/null 2>&1; then
    echo -e "${RED}ERROR: Coordinator not reachable at ${COORDINATOR_URL}${NC}"
    exit 1
fi

# ─── Register / reuse wallet ─────────────────────────────────────────────────
if [ -z "${WALLET_API_KEY:-}" ]; then
    echo "Registering fresh wallet…"
    REG=$(curl -s -X POST "${COORDINATOR_URL}/register")
    WALLET_API_KEY=$(echo "$REG" | jq -r '.api_key')
    NEAR_ACCT=$(echo "$REG" | jq -r '.near_account_id // .address // empty')
    if [ -z "$WALLET_API_KEY" ] || [ "$WALLET_API_KEY" = "null" ]; then
        echo -e "${RED}FAIL: /register did not return api_key${NC}"
        echo "$REG" | jq '.' 2>/dev/null || echo "$REG"
        exit 1
    fi
    echo -e "  Wallet: ${GREEN}${NEAR_ACCT:-(unknown)}${NC}"
    echo -e "  API key: ${YELLOW}${WALLET_API_KEY:0:16}…${NC}"
else
    echo "Reusing WALLET_API_KEY: ${WALLET_API_KEY:0:16}…"
fi
echo ""

# ─── Per-chain test ──────────────────────────────────────────────────────────
PASS=0
FAIL=0
FAILED_CHAINS=()

IFS=',' read -ra CHAIN_ARR <<< "$CHAINS"
for chain in "${CHAIN_ARR[@]}"; do
    chain="${chain// /}"
    src=$(source_asset_for_chain "$chain")
    if [ -z "$src" ]; then
        echo -e "[${chain}] ${RED}FAIL${NC} — unknown chain (no source asset in test catalog)"
        FAIL=$((FAIL+1))
        FAILED_CHAINS+=("$chain")
        continue
    fi
    expected=$(expected_format_label "$chain")
    body=$(jq -n --arg src "$src" --arg dst "$DEST_ASSET" \
        '{source_asset: $src, destination_asset: $dst, amount: "5000000"}')
    RESP=$(curl -s -w "\n%{http_code}" -X POST \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer ${WALLET_API_KEY}" \
        -d "$body" \
        "${COORDINATOR_URL}/wallet/v1/deposit-intent")
    RESP_BODY=$(echo "$RESP" | sed '$d')
    RESP_CODE=$(echo "$RESP" | tail -1)
    if [ "$VERBOSE" = "1" ]; then
        echo "  [${chain}] HTTP ${RESP_CODE}: $(echo "$RESP_BODY" | jq -c '.' 2>/dev/null || echo "$RESP_BODY")"
    fi
    if [ "$RESP_CODE" != "200" ]; then
        echo -e "[${chain}] ${RED}FAIL${NC} — HTTP ${RESP_CODE}  source=${src}"
        echo "    body: $(echo "$RESP_BODY" | jq -c '.' 2>/dev/null || echo "$RESP_BODY")"
        FAIL=$((FAIL+1))
        FAILED_CHAINS+=("$chain")
        continue
    fi
    addr=$(echo "$RESP_BODY" | jq -r '.deposit_address // empty')
    if [ -z "$addr" ]; then
        echo -e "[${chain}] ${RED}FAIL${NC} — no deposit_address in response"
        echo "    body: $(echo "$RESP_BODY" | jq -c '.')"
        FAIL=$((FAIL+1))
        FAILED_CHAINS+=("$chain")
        continue
    fi
    if address_matches_chain "$chain" "$addr"; then
        echo -e "[${chain}] ${GREEN}PASS${NC} — ${addr}  (matches ${expected})"
        PASS=$((PASS+1))
    else
        echo -e "[${chain}] ${RED}FAIL${NC} — ${addr}"
        echo -e "    expected: ${expected}"
        echo -e "    source:   ${src}"
        FAIL=$((FAIL+1))
        FAILED_CHAINS+=("$chain")
    fi
done

# ─── NEAR-refund derivation smoke ────────────────────────────────────────────
# The main loop above never specifies `refund_address`, so it implicitly
# exercises the default-derivation path (coordinator derives the wallet's
# own implicit address as the refund target). That covers the happy case
# but doesn't prove what gets derived for `chain=near` specifically — for
# EVM/Solana sources the derivation is `keystore_chain` → secp256k1/ed25519
# address, but for NEAR the answer should be the wallet's own NEAR implicit
# account. This sub-suite locks that down:
#
#   1. explicit refund_address (valid NEAR named account) → should accept
#      and 1Click should record it verbatim
#   2. explicit refund_address (valid NEAR implicit hex) → same
#   3. omitted refund_address with chain=near → derivation must succeed
#      without 500
#
# The response doesn't echo back the refund_address, so we can't directly
# assert what 1Click recorded. The signal is "request succeeded" — the
# failure mode this catches is the coordinator panicking or 400-ing on
# the NEAR derivation path. Recovery from a real bridge-refund event
# would need a separate test that forces a bridge failure (not feasible
# without 1Click cooperation).
echo ""
echo -e "${CYAN}=================================================${NC}"
echo -e "${CYAN} NEAR refund-derivation smoke (chain=near)${NC}"
echo -e "${CYAN}=================================================${NC}"

REFUND_PASS=0
REFUND_FAIL=0
REFUND_FAILED_CASES=()

# Helper — POST /wallet/v1/deposit-intent with arbitrary extra fields,
# return HTTP code in $RC and body in $RB.
deposit_intent_call() {
    local extra_json="$1"  # additional top-level keys, e.g. '"refund_address":"x.near"'
    local body
    body=$(jq -nc \
        --arg src "$(source_asset_for_chain near)" \
        --arg dst "$DEST_ASSET" \
        --argjson extra "{${extra_json}}" \
        '{source_asset: $src, destination_asset: $dst, amount: "5000000"} + $extra')
    local resp
    resp=$(curl -s -w "\n%{http_code}" -X POST \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer ${WALLET_API_KEY}" \
        -d "$body" \
        "${COORDINATOR_URL}/wallet/v1/deposit-intent")
    RB=$(echo "$resp" | sed '$d')
    RC=$(echo "$resp" | tail -1)
}

refund_case() {
    local label="$1"
    local extra="$2"
    deposit_intent_call "$extra"
    if [ "$RC" = "200" ] && [ -n "$(echo "$RB" | jq -r '.deposit_address // empty')" ]; then
        echo -e "  [${label}] ${GREEN}PASS${NC} — HTTP 200, deposit_address present"
        REFUND_PASS=$((REFUND_PASS+1))
    else
        echo -e "  [${label}] ${RED}FAIL${NC} — HTTP ${RC}"
        echo "    body: $(echo "$RB" | jq -c '.' 2>/dev/null || echo "$RB")"
        REFUND_FAIL=$((REFUND_FAIL+1))
        REFUND_FAILED_CASES+=("$label")
    fi
}

refund_case "named NEAR refund_address" '"refund_address":"zavodil.near"'
refund_case "implicit-hex NEAR refund_address" '"refund_address":"950c134ec86a21a8525d16d1dbae79258b923cabdaa8d32da284d931f74bdcb2"'
refund_case "default derivation (no refund_address)" ''

# ─── Summary ─────────────────────────────────────────────────────────────────
TOTAL_PASS=$((PASS + REFUND_PASS))
TOTAL_FAIL=$((FAIL + REFUND_FAIL))
echo ""
echo -e "${CYAN}=================================================${NC}"
if [ "$TOTAL_FAIL" -eq 0 ]; then
    echo -e "${GREEN} ALL PASSED  (chains ${PASS}/${PASS}, refund-derivation ${REFUND_PASS}/${REFUND_PASS})${NC}"
    echo -e "${CYAN}=================================================${NC}"
    exit 0
else
    echo -e "${RED} ${TOTAL_FAIL} FAILED  (${TOTAL_PASS} passed)${NC}"
    if [ "$FAIL" -ne 0 ]; then
        echo -e "${RED} failing chains: ${FAILED_CHAINS[*]}${NC}"
    fi
    if [ "$REFUND_FAIL" -ne 0 ]; then
        echo -e "${RED} failing refund cases: ${REFUND_FAILED_CASES[*]}${NC}"
    fi
    echo -e "${CYAN}=================================================${NC}"
    echo ""
    echo "This is issue #25 Bug A: /wallet/v1/deposit-intent should return"
    echo "a deposit address on the chain matching source_asset, and the"
    echo "NEAR-refund derivation path must succeed without 500. See"
    echo "https://github.com/fastnear/near-outlayer/issues/25 for the report."
    exit 1
fi
