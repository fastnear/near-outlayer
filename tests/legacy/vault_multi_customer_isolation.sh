#!/bin/bash
# Phase 10 Scenario 5: Multi-customer isolation.
#
# Two distinct customers, each with their own vault and wallet API
# key bound to that vault, must derive DIFFERENT NEAR addresses for
# the SAME canonical wallet operation (because the per-vault master
# is HMAC-keyed differently). The API key is the binding — a
# client-supplied `X-Customer-Vault: <other_vault>` header must be
# IGNORED (auth-driven vault scope, not request-driven).
#
# This proves the customer-isolation invariant at the HTTP layer:
#   keystore.derive_keypair(customer = A, seed = "wallet:..:near")
#         ≠ keystore.derive_keypair(customer = B, seed = "wallet:..:near")
#
# Why two vaults under the same PARENT works: dashboard's vault
# subaccount names are arbitrary (new7, new8, …), each lands at a
# distinct account id and produces its own per-vault master via
# MPC CKD with a distinct derivation_path.
#
# Required env:
#   PARENT         logged-in NEAR account (parent for BOTH vaults)
#   VAULT_A        sub-account name under PARENT for customer A (default: iso-a-<ts>)
#   VAULT_B        sub-account name under PARENT for customer B (default: iso-b-<ts>)
#
# Run:
#   PARENT=zavodil2.testnet ./tests/vault_multi_customer_isolation.sh --apply

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APPLY=false
[[ "${1:-}" == "--apply" ]] && APPLY=true

NETWORK="${NETWORK:-testnet}"
PARENT="${PARENT:-}"
COORDINATOR_URL="${COORDINATOR_URL:-https://testnet-api.outlayer.fastnear.com}"
TS="$(date +%s)"
VAULT_A_NAME="${VAULT_A:-iso-a-$TS}"
VAULT_B_NAME="${VAULT_B:-iso-b-$TS}"
VAULT_A_ID="$VAULT_A_NAME.$PARENT"
VAULT_B_ID="$VAULT_B_NAME.$PARENT"

log()  { printf '\n\033[36m▶ %s\033[0m\n' "$*" >&2; }
warn() { printf '\033[33m⚠ %s\033[0m\n' "$*" >&2; }
pass() { printf '\033[32m✓ %s\033[0m\n' "$*" >&2; }
fail() { printf '\033[31m✗ %s\033[0m\n' "$*" >&2; exit 1; }

[[ -n "$PARENT" ]] || fail "PARENT env required"
for tool in jq curl outlayer; do
  command -v "$tool" >/dev/null || fail "tool '$tool' missing"
done
if [[ "$APPLY" != true ]]; then
  warn "Dry-run; pass --apply to deploy two vaults + run isolation checks."
  exit 0
fi

LOGGED_IN=$(outlayer whoami 2>/dev/null | awk -F': *' '/^Account:/{print $2; exit}')
[[ "$LOGGED_IN" == "$PARENT" ]] || \
  fail "logged in as '$LOGGED_IN', not '$PARENT'"
pass "logged in as $PARENT on $NETWORK"

deploy_vault() {
  local name=$1
  local fqdn=$2
  log "Deploying $fqdn (exit-window 60s)"
  outlayer vault init --name "$name" --exit-window 60s >&2 || \
    fail "vault init $fqdn failed"
}

mint_api_key() {
  local vault=$1
  local resp
  resp=$(curl -sS -X POST "$COORDINATOR_URL/register" \
    -H 'Content-Type: application/json' \
    -d "{\"vault_id\":\"$vault\"}")
  echo "$resp"
}

# ─── 1. Deploy two vaults under the same parent ─────────────────────

deploy_vault "$VAULT_A_NAME" "$VAULT_A_ID"
deploy_vault "$VAULT_B_NAME" "$VAULT_B_ID"

# ─── 2. Mint a wallet API key per vault ─────────────────────────────

log "Minting wallet API key bound to $VAULT_A_ID"
REG_A=$(mint_api_key "$VAULT_A_ID")
echo "$REG_A" | jq . >&2
API_KEY_A=$(echo "$REG_A" | jq -r '.api_key')
WALLET_A=$(echo "$REG_A" | jq -r '.wallet_id')
ADDR_A=$(echo "$REG_A" | jq -r '.near_account_id')
[[ -n "$API_KEY_A" && "$API_KEY_A" != "null" ]] || \
  fail "/register did not return api_key for A: $REG_A"

log "Minting wallet API key bound to $VAULT_B_ID"
REG_B=$(mint_api_key "$VAULT_B_ID")
echo "$REG_B" | jq . >&2
API_KEY_B=$(echo "$REG_B" | jq -r '.api_key')
WALLET_B=$(echo "$REG_B" | jq -r '.wallet_id')
ADDR_B=$(echo "$REG_B" | jq -r '.near_account_id')
[[ -n "$API_KEY_B" && "$API_KEY_B" != "null" ]] || \
  fail "/register did not return api_key for B: $REG_B"

pass "customer A: wallet=$WALLET_A addr=$ADDR_A"
pass "customer B: wallet=$WALLET_B addr=$ADDR_B"

# ─── 3. Addresses must differ ───────────────────────────────────────

# Different per-vault masters → different
# HMAC-SHA256(master_X, "wallet:{wallet_id_X}:near") → different
# ed25519 secret → different NEAR implicit address.
if [[ "$ADDR_A" == "$ADDR_B" ]]; then
  fail "ISOLATION BROKEN: both vaults derived the same NEAR address $ADDR_A. \
keystore is using the same master for distinct vault scopes."
fi
pass "addresses differ — vault scope is honored end-to-end"

# ─── 4. Sign-message under each key returns distinct signatures ─────

sign_with_key() {
  local key=$1
  curl -sS -X POST "$COORDINATOR_URL/wallet/v1/sign-message" \
    -H "Authorization: Bearer $key" \
    -H 'Content-Type: application/json' \
    -d '{"message":"isolation-check","recipient":"verifier.testnet","nonce_base64":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="}'
}

log "Signing under API key A"
SIG_A=$(sign_with_key "$API_KEY_A")
echo "$SIG_A" | jq . >&2
PUB_A=$(echo "$SIG_A" | jq -r '.public_key // empty')
[[ -n "$PUB_A" && "$PUB_A" != "null" ]] || fail "no signature from A"

log "Signing under API key B"
SIG_B=$(sign_with_key "$API_KEY_B")
echo "$SIG_B" | jq . >&2
PUB_B=$(echo "$SIG_B" | jq -r '.public_key // empty')
[[ -n "$PUB_B" && "$PUB_B" != "null" ]] || fail "no signature from B"

if [[ "$PUB_A" == "$PUB_B" ]]; then
  fail "ISOLATION BROKEN: both API keys signed with the same public key $PUB_A"
fi
pass "signing public keys differ ($PUB_A vs $PUB_B)"

# ─── 5. X-Customer-Vault header is IGNORED (binding is API-key driven)

# Try using API key A but pass `X-Customer-Vault: <vault B>`. The
# coordinator must resolve vault scope from the API key's DB binding,
# NOT from the request header. So the address returned must equal
# ADDR_A — proving the header is decorative / ignored at this
# endpoint.

log "X-Customer-Vault override attempt: API key A with header pointing at vault B"
ADDR_PROBE=$(curl -sS -G "$COORDINATOR_URL/wallet/v1/address" \
  --data-urlencode "chain=near" \
  -H "Authorization: Bearer $API_KEY_A" \
  -H "X-Customer-Vault: $VAULT_B_ID" \
  | jq -r '.address // empty')
echo "/wallet/v1/address with header override returned: $ADDR_PROBE" >&2
if [[ "$ADDR_PROBE" != "$ADDR_A" ]]; then
  fail "HEADER NOT IGNORED: API key A returned $ADDR_PROBE (vault B's master?) instead of $ADDR_A. \
Auth-bound vault scope is leaking to request-driven override."
fi
pass "X-Customer-Vault header ignored when authenticated — binding is API-key driven"

echo
pass "ALL ISOLATION CHECKS PASSED. Vault scope is properly auth-bound."
warn "Cleanup (optional): delete $VAULT_A_ID and $VAULT_B_ID to reclaim NEAR."
