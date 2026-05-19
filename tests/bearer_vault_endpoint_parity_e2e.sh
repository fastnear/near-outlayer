#!/bin/bash
# bearer_vault_endpoint_parity_e2e.sh
#
# Regression coverage for the "resolve_wallet_pubkey ignores vault scope"
# bug class. Production repro: vault.deposit.tipbot.near minted with a
# Bearer-near + vault_id token; GET /address returned the correct
# vault-scoped address (deposit landed there on-chain), but GET /balance
# with the SAME token reported a default-master account_id and 0 balance.
#
# Root cause:
#   wallet_id = deterministic_wallet_id(account_id, seed)  // no vault
#   wallet_accounts.near_pubkey  — one column per wallet_id, IS NULL guard
#                                   on /address means first-write wins
#   resolve_wallet_pubkey(db, wallet_id) — DB lookup only, no re-derive
#
# Any path that ever wrote a default-master pubkey poisons every later
# vault-scoped lookup. The 16 endpoints that call resolve_wallet_pubkey
# all inherited this silent fork.
#
# Fix (server side, handlers.rs): resolve_wallet_pubkey_scoped re-derives
# via keystore when vault_id is present, only falling back to the DB
# cache for legitimate no-vault (default-master) Bearer-near and wk_
# flows where the cache is invariant.
#
# This test catches both the cache-poisoning manifestation and any
# future regression where a new endpoint forgets to thread vault scope.
#
# Three sections:
#   1. Endpoint parity — for a fresh seed under one vault, every
#      address-returning endpoint must report the same value.
#   2. Cache-poisoning regression — same seed used WITHOUT vault first
#      (writes default-master pubkey to DB), then WITH vault. All
#      vault-scoped endpoints must return the vault address, not the
#      poisoned cache.
#   3. Cross-vault isolation across endpoints — same seed under vault A
#      vs vault B, every endpoint honors the per-request scope.
#
# Requires:
#   PARENT            NEAR account that owns the vault
#   COORDINATOR_URL   default https://testnet-api.outlayer.fastnear.com
#   NETWORK           default testnet
#
# Run:
#   PARENT=zavodil2.testnet ./tests/bearer_vault_endpoint_parity_e2e.sh --apply

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APPLY=false
[[ "${1:-}" == "--apply" ]] && APPLY=true

NETWORK="${NETWORK:-testnet}"
PARENT="${PARENT:-}"
COORDINATOR_URL="${COORDINATOR_URL:-https://testnet-api.outlayer.fastnear.com}"
CREDS_FILE="${CREDS_FILE:-$HOME/.near-credentials/$NETWORK/$PARENT.json}"

log()  { printf '\n\033[36m▶ %s\033[0m\n' "$*" >&2; }
warn() { printf '\033[33m⚠ %s\033[0m\n' "$*" >&2; }
pass() { printf '\033[32m✓ %s\033[0m\n' "$*" >&2; }
fail() { printf '\033[31m✗ %s\033[0m\n' "$*" >&2; exit 1; }

[[ -n "$PARENT" ]] || fail "PARENT env required (e.g. PARENT=zavodil2.testnet)"
for tool in jq curl outlayer python3; do
  command -v "$tool" >/dev/null || fail "tool '$tool' missing"
done
[[ -f "$CREDS_FILE" ]] || fail "credentials file not found: $CREDS_FILE"

if [[ "$APPLY" != true ]]; then
  warn "Dry-run; pass --apply to deploy vaults and hit the coordinator."
  exit 0
fi

RECOVERY_BIN="$SCRIPT_DIR/../scripts/customer-recovery/target/release/customer-recovery"
log "Building customer-recovery (need sign-bearer-near)"
(cd "$SCRIPT_DIR/../scripts/customer-recovery" && cargo build --release --quiet) || \
  fail "customer-recovery build failed"
[[ -x "$RECOVERY_BIN" ]] || fail "binary not found: $RECOVERY_BIN"

LOGGED_IN=$(outlayer whoami 2>/dev/null | awk -F': *' '/^Account:/{print $2; exit}')
[[ "$LOGGED_IN" == "$PARENT" ]] || \
  fail "outlayer logged in as '$LOGGED_IN', not '$PARENT' — needed for vault init"
pass "logged in as $PARENT on $NETWORK"

PARENT_PRIVKEY=$(jq -r '.private_key // empty' "$CREDS_FILE")
PARENT_PUBKEY=$(jq -r '.public_key // empty' "$CREDS_FILE")
[[ -n "$PARENT_PRIVKEY" && "$PARENT_PRIVKEY" != "null" ]] || \
  fail "no .private_key in $CREDS_FILE"

# ─── Vaults ────────────────────────────────────────────────────────

VAULT_A_NAME="parity-$(date +%s)"
VAULT_A_ID="$VAULT_A_NAME.$PARENT"
log "Deploying VAULT_A=$VAULT_A_ID"
outlayer vault init --name "$VAULT_A_NAME" --exit-window 60s >&2 || \
  fail "vault init $VAULT_A_ID failed"

VAULT_B_NAME="parity-$(date +%s)-b"
VAULT_B_ID="$VAULT_B_NAME.$PARENT"
log "Deploying VAULT_B=$VAULT_B_ID (for cross-vault isolation)"
outlayer vault init --name "$VAULT_B_NAME" --exit-window 60s >&2 || \
  fail "vault init $VAULT_B_ID failed"

# ─── Helpers ───────────────────────────────────────────────────────

build_bearer_near() {
  # acct privkey seed vault_or_empty
  local acct=$1 privkey=$2 seed=$3 vault=$4 ts
  ts=$(date +%s)
  "$RECOVERY_BIN" sign-bearer-near \
    --private-key "$privkey" --account-id "$acct" --seed "$seed" \
    --timestamp "$ts" \
    ${vault:+--vault-id "$vault"} 2>/dev/null
}

token_for() {
  # label acct priv seed vault → echoes a Bearer-near token, fails loudly
  local label=$1 acct=$2 priv=$3 seed=$4 vault=$5
  local t
  t=$(build_bearer_near "$acct" "$priv" "$seed" "$vault")
  [[ -n "$t" ]] || fail "$label: build_bearer_near failed"
  printf '%s' "$t"
}

addr_via_address() {
  # token → echoes address
  local token=$1
  local resp
  resp=$(curl -sS -G "$COORDINATOR_URL/wallet/v1/address" \
    --data-urlencode "chain=near" \
    -H "Authorization: Bearer near:$token")
  local addr
  addr=$(echo "$resp" | jq -r '.address // empty')
  [[ -n "$addr" ]] || fail "no .address in /address response: $resp"
  printf '%s' "$addr"
}

acct_via_balance_intents() {
  # token → echoes /balance .account_id (intents path — the bug repro)
  local token=$1
  local resp
  resp=$(curl -sS -G "$COORDINATOR_URL/wallet/v1/balance" \
    --data-urlencode "token=nep141:usdt.tether-token.near" \
    --data-urlencode "source=intents" \
    -H "Authorization: Bearer near:$token")
  local acc
  acc=$(echo "$resp" | jq -r '.account_id // empty')
  [[ -n "$acc" ]] || fail "no .account_id in /balance(intents) response: $resp"
  printf '%s' "$acc"
}

acct_via_balance_native() {
  # token → echoes /balance .account_id (native NEAR path)
  local token=$1
  local resp
  resp=$(curl -sS -G "$COORDINATOR_URL/wallet/v1/balance" \
    --data-urlencode "chain=near" \
    -H "Authorization: Bearer near:$token")
  local acc
  acc=$(echo "$resp" | jq -r '.account_id // empty')
  [[ -n "$acc" ]] || fail "no .account_id in /balance(native) response: $resp"
  printf '%s' "$acc"
}

acct_via_sign_message() {
  # token → echoes /sign-message .account_id (separate code path; uses
  # auth.wallet_id with vault scope at signing time, not DB cache)
  local token=$1 msg
  msg="parity-$(date +%s%N)"
  local resp
  resp=$(curl -sS -X POST "$COORDINATOR_URL/wallet/v1/sign-message" \
    -H "Authorization: Bearer near:$token" \
    -H 'Content-Type: application/json' \
    -d "$(jq -n --arg m "$msg" '{message: $m, recipient: "parity-verifier.testnet", nonce_base64: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="}')")
  local acc
  acc=$(echo "$resp" | jq -r '.account_id // empty')
  [[ -n "$acc" ]] || fail "no .account_id in /sign-message response: $resp"
  printf '%s' "$acc"
}

# ─── Section 1: cross-endpoint parity ──────────────────────────────
#
# Fresh seed, fresh vault, fresh Bearer-near token. Every endpoint that
# exposes the wallet's NEAR account must report the same value as
# /address. /address derives directly via keystore so it's the ground
# truth; others go through resolve_wallet_pubkey_scoped and must agree.

log "1. Endpoint parity: same Bearer-near + vault_id → all endpoints same address"
SEED_PARITY="parity-$(date +%s)-$$"
TOK_PARITY=$(token_for "parity" "$PARENT" "$PARENT_PRIVKEY" "$SEED_PARITY" "$VAULT_A_ID")

A_ADDR=$(addr_via_address       "$TOK_PARITY")
A_BAL_I=$(acct_via_balance_intents "$TOK_PARITY")
A_BAL_N=$(acct_via_balance_native  "$TOK_PARITY")
A_SIGN=$(acct_via_sign_message    "$TOK_PARITY")

echo "  /address               : $A_ADDR" >&2
echo "  /balance (intents)     : $A_BAL_I" >&2
echo "  /balance (native NEAR) : $A_BAL_N" >&2
echo "  /sign-message          : $A_SIGN" >&2

[[ "$A_BAL_I" == "$A_ADDR" ]] || \
  fail "Section 1 PARITY BROKEN: /balance(intents) account_id '$A_BAL_I' ≠ /address '$A_ADDR' — resolve_wallet_pubkey ignores auth.vault_id"
[[ "$A_BAL_N" == "$A_ADDR" ]] || \
  fail "Section 1 PARITY BROKEN: /balance(native) account_id '$A_BAL_N' ≠ /address '$A_ADDR'"
[[ "$A_SIGN"  == "$A_ADDR" ]] || \
  fail "Section 1 PARITY BROKEN: /sign-message account_id '$A_SIGN' ≠ /address '$A_ADDR'"
pass "all 4 endpoints agree on the vault-scoped address: $A_ADDR"

# ─── Section 2: cache-poisoning regression ─────────────────────────
#
# Exact repro of the production bug. A bot's first call for a given
# (account, seed) happens to be a default-master /address — writes
# default-master pubkey to wallet_accounts.near_pubkey. Later the bot
# discovers it has a vault and starts sending vault-scoped tokens.
# Pre-fix: every endpoint except /address kept returning the poisoned
# default-master account_id, because the DB cache was first-write-wins
# and never re-derived.

log "2. Cache poisoning: default-master /address THEN vault /balance must still be vault-scoped"
SEED_POISON="poison-$(date +%s)-$$"

# Step 1: poison the cache by calling /address with NO vault_id.
TOK_NV=$(token_for "no vault" "$PARENT" "$PARENT_PRIVKEY" "$SEED_POISON" "")
ADDR_NV=$(addr_via_address "$TOK_NV")
echo "  default-master /address (cache write): $ADDR_NV" >&2

# Step 2: vault-scoped /address — proves the keystore derives differently.
TOK_VA=$(token_for "vault A" "$PARENT" "$PARENT_PRIVKEY" "$SEED_POISON" "$VAULT_A_ID")
ADDR_VA=$(addr_via_address "$TOK_VA")
echo "  vault-scoped /address (ground truth)  : $ADDR_VA" >&2
[[ "$ADDR_VA" != "$ADDR_NV" ]] || \
  fail "vault and default-master derived same address — per-vault HMAC broken"

# Step 3 (the bug): /balance under the vault token MUST return ADDR_VA,
# not the cached ADDR_NV from step 1.
BAL_VA_I=$(acct_via_balance_intents "$TOK_VA")
BAL_VA_N=$(acct_via_balance_native  "$TOK_VA")
SIGN_VA=$(acct_via_sign_message     "$TOK_VA")

echo "  /balance(intents) under vault token  : $BAL_VA_I" >&2
echo "  /balance(native)  under vault token  : $BAL_VA_N" >&2
echo "  /sign-message     under vault token  : $SIGN_VA" >&2

[[ "$BAL_VA_I" == "$ADDR_VA" ]] || \
  fail "BUG REPRO: /balance(intents) returned poisoned default-master '$BAL_VA_I' instead of vault-scoped '$ADDR_VA' — resolve_wallet_pubkey served stale DB cache"
[[ "$BAL_VA_N" == "$ADDR_VA" ]] || \
  fail "BUG REPRO: /balance(native) returned poisoned '$BAL_VA_N' instead of '$ADDR_VA'"
[[ "$SIGN_VA"  == "$ADDR_VA" ]] || \
  fail "BUG REPRO: /sign-message returned poisoned '$SIGN_VA' instead of '$ADDR_VA'"
pass "cache poisoning defeated: vault token gets vault-scoped account_id across all endpoints"

# Step 4: also verify the no-vault token still gets the default-master
# address (sanity that the fix didn't break the no-vault path).
BAL_NV_I=$(acct_via_balance_intents "$TOK_NV")
[[ "$BAL_NV_I" == "$ADDR_NV" ]] || \
  fail "no-vault path regressed: /balance(intents) '$BAL_NV_I' ≠ /address '$ADDR_NV'"
pass "no-vault path intact: /balance(intents) still reports default-master account_id"

# ─── Section 3: cross-vault isolation across endpoints ─────────────
#
# Same (account, seed), three scopes: vault A, vault B, default. Every
# endpoint must honor the per-request scope — not leak between vaults
# and not collapse to one cached value.

log "3. Cross-vault: same seed across vault A / vault B / default → each endpoint scope-aware"
SEED_XVAULT="xvault-$(date +%s)-$$"

TOK_A=$(token_for "scope A" "$PARENT" "$PARENT_PRIVKEY" "$SEED_XVAULT" "$VAULT_A_ID")
TOK_B=$(token_for "scope B" "$PARENT" "$PARENT_PRIVKEY" "$SEED_XVAULT" "$VAULT_B_ID")
TOK_D=$(token_for "default" "$PARENT" "$PARENT_PRIVKEY" "$SEED_XVAULT" "")

A_VA=$(addr_via_address "$TOK_A")
A_VB=$(addr_via_address "$TOK_B")
A_DM=$(addr_via_address "$TOK_D")
[[ "$A_VA" != "$A_VB" && "$A_VA" != "$A_DM" && "$A_VB" != "$A_DM" ]] || \
  fail "/address: three scopes collapsed to fewer than three addresses (A=$A_VA B=$A_VB D=$A_DM)"

# Each endpoint must mirror the same three-way isolation.
B_VA=$(acct_via_balance_intents "$TOK_A")
B_VB=$(acct_via_balance_intents "$TOK_B")
B_DM=$(acct_via_balance_intents "$TOK_D")
echo "  /balance(intents)  A=$B_VA  B=$B_VB  default=$B_DM" >&2
[[ "$B_VA" == "$A_VA" ]] || fail "/balance(intents) A mismatch: $B_VA vs /address $A_VA"
[[ "$B_VB" == "$A_VB" ]] || fail "/balance(intents) B mismatch: $B_VB vs /address $A_VB"
[[ "$B_DM" == "$A_DM" ]] || fail "/balance(intents) default mismatch: $B_DM vs /address $A_DM"

N_VA=$(acct_via_balance_native "$TOK_A")
N_VB=$(acct_via_balance_native "$TOK_B")
N_DM=$(acct_via_balance_native "$TOK_D")
echo "  /balance(native)   A=$N_VA  B=$N_VB  default=$N_DM" >&2
[[ "$N_VA" == "$A_VA" ]] || fail "/balance(native) A mismatch: $N_VA vs /address $A_VA"
[[ "$N_VB" == "$A_VB" ]] || fail "/balance(native) B mismatch: $N_VB vs /address $A_VB"
[[ "$N_DM" == "$A_DM" ]] || fail "/balance(native) default mismatch: $N_DM vs /address $A_DM"

S_VA=$(acct_via_sign_message "$TOK_A")
S_VB=$(acct_via_sign_message "$TOK_B")
S_DM=$(acct_via_sign_message "$TOK_D")
echo "  /sign-message      A=$S_VA  B=$S_VB  default=$S_DM" >&2
[[ "$S_VA" == "$A_VA" ]] || fail "/sign-message A mismatch: $S_VA vs /address $A_VA"
[[ "$S_VB" == "$A_VB" ]] || fail "/sign-message B mismatch: $S_VB vs /address $A_VB"
[[ "$S_DM" == "$A_DM" ]] || fail "/sign-message default mismatch: $S_DM vs /address $A_DM"

pass "cross-vault isolation holds across /address, /balance(intents), /balance(native), /sign-message"

echo "" >&2
pass "ALL PARITY CHECKS PASSED — resolve_wallet_pubkey honors auth.vault_id at every callsite tested"
warn "Cleanup (optional): $VAULT_A_ID and $VAULT_B_ID each have ~0.1 NEAR locked."
