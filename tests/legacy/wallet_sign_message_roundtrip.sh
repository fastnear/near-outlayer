#!/bin/bash
# Wallet sign-message roundtrip: cryptographic verification.
#
# Existing tests verify the keystore RETURNS a signature, but never
# actually verify the signature with the returned public key. This
# script closes that gap end-to-end:
#
#   1. Default-master path: `POST /register` (empty body) → wk_ +
#      pubkey. Sign a NEP-413 message via `/wallet/v1/sign-message`,
#      then ed25519-verify the returned signature against the
#      returned public key. Exit non-zero on mismatch.
#   2. Vault-bound path: deploy a fresh vault via `outlayer vault init`,
#      `POST /register {vault_id}`, sign-message, ed25519-verify.
#      Confirms the per-vault master produces valid signatures.
#   3. Cross-scope rejection: signature from the default-master wallet
#      MUST NOT verify against the vault-bound wallet's pubkey and
#      vice versa. Proves cryptographic isolation, not just distinct
#      strings.
#   4. Tamper detection: modifying any field (nonce, message,
#      recipient) MUST break verification.
#
# Requires:
#   PARENT          NEAR account that owns the vault (logged in via outlayer login)
#   COORDINATOR_URL default https://testnet-api.outlayer.fastnear.com
#
# Run:
#   PARENT=zavodil2.testnet ./tests/wallet_sign_message_roundtrip.sh --apply

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APPLY=false
[[ "${1:-}" == "--apply" ]] && APPLY=true

NETWORK="${NETWORK:-testnet}"
PARENT="${PARENT:-}"
COORDINATOR_URL="${COORDINATOR_URL:-https://testnet-api.outlayer.fastnear.com}"

log()  { printf '\n\033[36m▶ %s\033[0m\n' "$*" >&2; }
warn() { printf '\033[33m⚠ %s\033[0m\n' "$*" >&2; }
pass() { printf '\033[32m✓ %s\033[0m\n' "$*" >&2; }
fail() { printf '\033[31m✗ %s\033[0m\n' "$*" >&2; exit 1; }

[[ -n "$PARENT" ]] || fail "PARENT env required (e.g. PARENT=zavodil2.testnet)"
for tool in jq curl outlayer; do
  command -v "$tool" >/dev/null || fail "tool '$tool' missing"
done

if [[ "$APPLY" != true ]]; then
  warn "Dry-run; pass --apply to deploy + verify against testnet."
  exit 0
fi

RECOVERY_BIN="$SCRIPT_DIR/../scripts/customer-recovery/target/release/customer-recovery"
log "Building customer-recovery (need verify-sign-message subcommand)"
(cd "$SCRIPT_DIR/../scripts/customer-recovery" && cargo build --release --quiet) || \
  fail "customer-recovery build failed"
[[ -x "$RECOVERY_BIN" ]] || fail "binary not found: $RECOVERY_BIN"

LOGGED_IN=$(outlayer whoami 2>/dev/null | awk -F': *' '/^Account:/{print $2; exit}')
[[ "$LOGGED_IN" == "$PARENT" ]] || \
  fail "outlayer logged in as '$LOGGED_IN', not '$PARENT'"
pass "logged in as $PARENT on $NETWORK"

# ─── Helper: hit POST /wallet/v1/sign-message, validate signature ────

sign_and_verify() {
  local label=$1
  local api_key=$2
  local recipient=$3
  # NOTE: $4 (expected pubkey from /wallet/v1/address) is intentionally
  # NOT compared as a string. The address endpoint returns the pubkey in
  # hex (`ed25519:<64 hex>`) while sign-message returns base58 — same
  # bytes, different encoding. The crypto verify below is the actual
  # roundtrip proof.

  log "$label: POST /wallet/v1/sign-message"
  local msg="roundtrip-$(date +%s)-$$"
  local resp
  resp=$(curl -sS -X POST "$COORDINATOR_URL/wallet/v1/sign-message" \
    -H "Authorization: Bearer $api_key" \
    -H 'Content-Type: application/json' \
    -d "$(jq -n --arg msg "$msg" --arg rcp "$recipient" \
         '{message: $msg, recipient: $rcp, nonce_base64: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="}')")
  local sig pub nonce_b64 account
  sig=$(echo "$resp" | jq -r '.signature // empty')
  pub=$(echo "$resp" | jq -r '.public_key // empty')
  nonce_b64=$(echo "$resp" | jq -r '.nonce // empty')
  account=$(echo "$resp" | jq -r '.account_id // empty')
  [[ -n "$sig" && "$sig" != "null" ]] || fail "$label: no .signature in response: $resp"
  [[ -n "$pub" && "$pub" != "null" ]] || fail "$label: no .public_key in response"
  [[ -n "$nonce_b64" && "$nonce_b64" != "null" ]] || fail "$label: no .nonce in response"

  log "$label: ed25519-verify the returned signature"
  if "$RECOVERY_BIN" verify-sign-message \
       --pubkey "$pub" \
       --message "$msg" \
       --recipient "$recipient" \
       --nonce-base64 "$nonce_b64" \
       --signature "$sig" >/dev/null; then
    pass "$label: signature verifies under pubkey $pub"
  else
    fail "$label: signature did NOT verify — keystore returned crypto-invalid sig"
  fi

  # Echo what we got so cross-scope checks below can use it.
  printf '%s\n%s\n%s\n%s\n' "$sig" "$pub" "$nonce_b64" "$msg"
}

# ─── 1. Default-master path ────────────────────────────────────────

log "1. POST /register with empty body (legacy default-master)"
REG_A=$(curl -sS -X POST "$COORDINATOR_URL/register" \
  -H 'Content-Type: application/json' -d '{}')
echo "$REG_A" | jq . >&2
KEY_A=$(echo "$REG_A" | jq -r '.api_key')
ADDR_A=$(echo "$REG_A" | jq -r '.near_account_id')
[[ -n "$KEY_A" && "$KEY_A" != "null" ]] || fail "no api_key in /register response"

PUB_A=$(curl -sS -G "$COORDINATOR_URL/wallet/v1/address" --data-urlencode "chain=near" \
  -H "Authorization: Bearer $KEY_A" | jq -r '.public_key // empty')
[[ -n "$PUB_A" && "$PUB_A" != "null" ]] || fail "no public_key from /wallet/v1/address"
pass "default-master wallet: addr=$ADDR_A pubkey=$PUB_A"

# Capture sign-message outputs. Use the base58 pubkey returned by
# sign-message (PUB_A_SIG) for all crypto operations below — the hex
# form returned by /wallet/v1/address (PUB_A) is not consumable by
# the ed25519 verifier.
RESULT_A=$(sign_and_verify "DEFAULT" "$KEY_A" "verifier-default.testnet" "$PUB_A")
SIG_A=$(echo "$RESULT_A" | sed -n '1p')
PUB_A_SIG=$(echo "$RESULT_A" | sed -n '2p')
NONCE_A=$(echo "$RESULT_A" | sed -n '3p')
MSG_A=$(echo "$RESULT_A" | sed -n '4p')

# ─── 2. Vault-bound path ───────────────────────────────────────────

VAULT_NAME="sigrt-$(date +%s)"
VAULT_ID="$VAULT_NAME.$PARENT"
log "2. Deploying $VAULT_ID for vault-bound branch"
outlayer vault init --name "$VAULT_NAME" --exit-window 60s >&2 || \
  fail "vault init $VAULT_ID failed"

log "POST /register with vault_id=$VAULT_ID"
REG_B=$(curl -sS -X POST "$COORDINATOR_URL/register" \
  -H 'Content-Type: application/json' \
  -d "{\"vault_id\":\"$VAULT_ID\"}")
echo "$REG_B" | jq . >&2
KEY_B=$(echo "$REG_B" | jq -r '.api_key')
ADDR_B=$(echo "$REG_B" | jq -r '.near_account_id')
[[ -n "$KEY_B" && "$KEY_B" != "null" ]] || fail "no api_key for vault-bound /register"
[[ "$ADDR_A" != "$ADDR_B" ]] || fail "default and vault wallets share the same address — broken scope"
pass "vault-bound wallet: vault=$VAULT_ID addr=$ADDR_B"

PUB_B=$(curl -sS -G "$COORDINATOR_URL/wallet/v1/address" --data-urlencode "chain=near" \
  -H "Authorization: Bearer $KEY_B" | jq -r '.public_key // empty')
[[ -n "$PUB_B" && "$PUB_B" != "null" ]] || fail "no public_key for vault-bound wallet"
[[ "$PUB_A" != "$PUB_B" ]] || fail "default and vault pubkeys are identical — broken scope"

RESULT_B=$(sign_and_verify "VAULT" "$KEY_B" "verifier-vault.testnet" "$PUB_B")
SIG_B=$(echo "$RESULT_B" | sed -n '1p')
PUB_B_SIG=$(echo "$RESULT_B" | sed -n '2p')
NONCE_B=$(echo "$RESULT_B" | sed -n '3p')
MSG_B=$(echo "$RESULT_B" | sed -n '4p')

[[ "$PUB_A_SIG" != "$PUB_B_SIG" ]] || \
  fail "default and vault sign-message pubkeys identical — broken scope"

# ─── 3. Cross-scope rejection ──────────────────────────────────────
#
# A's signature must NOT verify against B's pubkey, and vice versa.
# This proves the keys are cryptographically isolated, not just two
# different strings the coordinator hands out for show.

log "3. Cross-scope: SIG_A against PUB_B (must fail)"
if "$RECOVERY_BIN" verify-sign-message \
     --pubkey "$PUB_B_SIG" --message "$MSG_A" \
     --recipient "verifier-default.testnet" \
     --nonce-base64 "$NONCE_A" --signature "$SIG_A" >/dev/null 2>&1; then
  fail "ISOLATION BROKEN: default-master signature verified against vault pubkey"
fi
pass "SIG_A rejected by PUB_B"

log "3. Cross-scope: SIG_B against PUB_A (must fail)"
if "$RECOVERY_BIN" verify-sign-message \
     --pubkey "$PUB_A_SIG" --message "$MSG_B" \
     --recipient "verifier-vault.testnet" \
     --nonce-base64 "$NONCE_B" --signature "$SIG_B" >/dev/null 2>&1; then
  fail "ISOLATION BROKEN: vault signature verified against default-master pubkey"
fi
pass "SIG_B rejected by PUB_A"

# ─── 4. Tamper detection ───────────────────────────────────────────

log "4. Tamper: flip one byte in nonce — must fail"
TAMPERED_NONCE=$(echo "$NONCE_A" | base64 -d 2>/dev/null | \
  python3 -c 'import sys; b=bytearray(sys.stdin.buffer.read()); b[0]^=1; sys.stdout.buffer.write(bytes(b))' \
  | base64 | tr -d '\n')
if "$RECOVERY_BIN" verify-sign-message \
     --pubkey "$PUB_A_SIG" --message "$MSG_A" \
     --recipient "verifier-default.testnet" \
     --nonce-base64 "$TAMPERED_NONCE" --signature "$SIG_A" >/dev/null 2>&1; then
  fail "TAMPER UNDETECTED: signature verified with mutated nonce"
fi
pass "tampered nonce rejected"

log "4. Tamper: change message — must fail"
if "$RECOVERY_BIN" verify-sign-message \
     --pubkey "$PUB_A_SIG" --message "${MSG_A}-mutated" \
     --recipient "verifier-default.testnet" \
     --nonce-base64 "$NONCE_A" --signature "$SIG_A" >/dev/null 2>&1; then
  fail "TAMPER UNDETECTED: signature verified with mutated message"
fi
pass "tampered message rejected"

log "4. Tamper: change recipient — must fail"
if "$RECOVERY_BIN" verify-sign-message \
     --pubkey "$PUB_A_SIG" --message "$MSG_A" \
     --recipient "verifier-mutated.testnet" \
     --nonce-base64 "$NONCE_A" --signature "$SIG_A" >/dev/null 2>&1; then
  fail "TAMPER UNDETECTED: signature verified with mutated recipient"
fi
pass "tampered recipient rejected"

echo
pass "ALL CHECKS PASSED. Keystore returns cryptographically-valid signatures"
pass "  for both default-master and vault-bound wallets, with proper"
pass "  cross-scope isolation and tamper detection."
warn "Cleanup (optional): $VAULT_ID has 0.1 NEAR locked in storage."
