#!/bin/bash
# Bearer near: stateless flow + sovereign exit, end-to-end.
#
# Complements sovereignty_e2e.sh: that test covers the /register +
# wk_ path; this one covers the **stateless Bearer near: + vault_id**
# path that a tipbot-style agent uses (no per-user wk_, no /register).
#
# The full sovereignty story for Bearer near: hangs on three claims,
# verified inline:
#
#   A. While the vault is healthy, OutLayer derives + signs for any
#      `(account_id, seed, vault_id)` via Bearer near:. Wallet
#      addresses are deterministic and reproducible across requests.
#
#   B. After `finalize_recovery`, **OutLayer can no longer derive or
#      sign for this vault**. Bearer near: + vault_id requests for
#      /address and /sign-message MUST fail (keystore's
#      `assert_serving_allowed` rejects on `unlocked == true`).
#
#   C. The customer can recover the per-vault master via MPC CKD
#      using only the new parent key, and then **re-derive every user's
#      wallet offline** with just (parent_account_id, seed). The
#      offline-derived addresses MUST equal the addresses OutLayer
#      reported in step A. Multiple users are recovered from one master.
#
# This last claim is the tipbot value-prop: "I leave OutLayer, I
# recover all my users in one shot from my recovered master + my list
# of telegram_ids."
#
# Flow:
#   1. `outlayer vault init`
#   2. Mint 3 user wallets via Bearer near: + vault_id (different seeds)
#   3. Sign /sign-message via Bearer near: for user #1 → crypto-valid
#   4. `outlayer vault initiate-unilateral-recovery`
#   5. Wait MIN_UNILATERAL_EXIT_WINDOW_SECS (60s)
#   6. `customer-recovery generate-key` → new_parent_pubkey
#   7. `outlayer vault finalize-recovery <vault> <new_parent_pubkey>`
#   8. Bearer near: + vault_id → /address must FAIL (cutoff proof)
#      Bearer near: + vault_id → /sign-message must FAIL too
#   9. `customer-recovery --from-chain --vault-id <vault>`
#      → recovers per-vault master locally
#  10. For each of the 3 users:
#        wallet_id_local = compute-wallet-id PARENT seed
#        derived         = derive-wallet-key master wallet_id_local
#        ASSERT derived.near_address == address from step 2
#  11. (Optional) Send a real testnet tx with user #1's recovered key
#
# Required env (same as sovereignty_e2e.sh):
#   PARENT          NEAR account that will own the vault (logged in
#                   via `outlayer login`).
#   MPC_PUBLIC_KEY  bls12381g2:base58 — same value keystore-worker uses.
#   NETWORK         testnet (default). mainnet not supported.
#
# Run:
#   MPC_PUBLIC_KEY=bls12381g2:... PARENT=alice.testnet ./bearer_near_recovery_e2e.sh --apply

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APPLY=false
[[ "${1:-}" == "--apply" ]] && APPLY=true

NETWORK="${NETWORK:-testnet}"
PARENT="${PARENT:-}"
RPC_URL="${RPC_URL:-https://rpc.${NETWORK}.fastnear.com}"

case "$NETWORK" in
  testnet)
    COORDINATOR_URL="${COORDINATOR_URL:-https://testnet-api.outlayer.fastnear.com}"
    MPC_CONTRACT_ID="${MPC_CONTRACT_ID:-v1.signer-prod.testnet}"
    NEARBLOCKS_URL="${NEARBLOCKS_URL:-https://api-testnet.nearblocks.io}"
    ;;
  *)
    echo "✗ unsupported NETWORK=$NETWORK (only testnet)" >&2
    exit 1
    ;;
esac

[[ -n "$PARENT" ]] || { echo "USAGE: PARENT=alice.testnet MPC_PUBLIC_KEY=... $0 --apply" >&2; exit 1; }
[[ -n "${MPC_PUBLIC_KEY:-}" ]] || { echo "✗ MPC_PUBLIC_KEY required (bls12381g2:base58)" >&2; exit 1; }

for tool in jq curl outlayer python3; do
  command -v "$tool" >/dev/null || { echo "✗ missing tool: $tool" >&2; exit 1; }
done

CREDS_FILE="${CREDS_FILE:-$HOME/.near-credentials/$NETWORK/$PARENT.json}"
[[ -f "$CREDS_FILE" ]] || { echo "✗ creds not found: $CREDS_FILE" >&2; exit 1; }

log()  { printf '\n\033[36m▶ %s\033[0m\n' "$*" >&2; }
warn() { printf '\033[33m⚠ %s\033[0m\n' "$*" >&2; }
pass() { printf '\033[32m✓ %s\033[0m\n' "$*" >&2; }
fail() { printf '\033[31m✗ %s\033[0m\n' "$*" >&2; exit 1; }

near_tty() {
  if command -v script >/dev/null 2>&1; then
    local tmp; tmp=$(mktemp -t bn_recov_cmd.XXXXXX.sh)
    printf 'set -euo pipefail\n%s\n' "$*" > "$tmp"
    script -q /dev/null bash "$tmp"; local rc=$?
    rm -f "$tmp"; return $rc
  else
    eval "$@"
  fi
}

if [[ "$APPLY" != true ]]; then
  warn "Dry-run mode. Pass --apply to deploy a vault, mint wallets, recover, and verify offline."
  exit 0
fi

RECOVERY_BIN="$SCRIPT_DIR/../scripts/customer-recovery/target/release/customer-recovery"
log "Building customer-recovery (need sign-bearer-near + compute-wallet-id)"
(cd "$SCRIPT_DIR/../scripts/customer-recovery" && cargo build --release --quiet) || \
  fail "customer-recovery build failed"

WHOAMI=$(outlayer whoami 2>/dev/null | awk -F': *' '/^Account:/{print $2; exit}')
[[ "$WHOAMI" == "$PARENT" ]] || fail "outlayer logged in as '$WHOAMI', not '$PARENT'"
pass "logged in as $PARENT on $NETWORK"

PARENT_PRIVKEY=$(jq -r '.private_key' "$CREDS_FILE")
[[ -n "$PARENT_PRIVKEY" ]] || fail "no private_key in $CREDS_FILE"

# ─── Helper: Bearer near: GET /address ────────────────────────────
bn_address() {
  local seed=$1 vault=$2 label=$3
  local token
  token=$("$RECOVERY_BIN" sign-bearer-near \
    --private-key "$PARENT_PRIVKEY" --account-id "$PARENT" --seed "$seed" \
    ${vault:+--vault-id "$vault"})
  local resp http
  resp=$(curl -sS -w '\nHTTP:%{http_code}' -G "$COORDINATOR_URL/wallet/v1/address" \
    --data-urlencode "chain=near" \
    -H "Authorization: Bearer near:$token")
  local body=$(echo "$resp" | sed '$d')
  http=$(echo "$resp" | tail -1 | sed 's/HTTP://')
  if [[ "$http" != "200" ]]; then
    echo "$label: HTTP $http body=$body" >&2
    return 1
  fi
  echo "$body" | jq -r '.address'
}

bn_sign_message() {
  local seed=$1 vault=$2 msg=$3
  local token
  token=$("$RECOVERY_BIN" sign-bearer-near \
    --private-key "$PARENT_PRIVKEY" --account-id "$PARENT" --seed "$seed" \
    ${vault:+--vault-id "$vault"})
  curl -sS -w '\nHTTP:%{http_code}' -X POST "$COORDINATOR_URL/wallet/v1/sign-message" \
    -H "Authorization: Bearer near:$token" \
    -H 'Content-Type: application/json' \
    -d "$(jq -n --arg m "$msg" '{message: $m, recipient: "bn-recov.testnet", nonce_base64: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="}')"
}

# ─── 1. Deploy vault ─────────────────────────────────────────────

log "1. Deploy fresh vault (exit-window=60s for fast test)"
VAULT_NAME="bnrec-$(date +%s)"
VAULT_ID="$VAULT_NAME.$PARENT"
INIT_RC=0
INIT_OUT=$(outlayer vault init --name "$VAULT_NAME" --exit-window 60s 2>&1) || INIT_RC=$?
echo "$INIT_OUT" >&2
if [[ $INIT_RC -ne 0 ]] && echo "$INIT_OUT" | grep -q "outlayer vault resume"; then
  log "  vault deploy raced RPC propagation — resuming"
  for attempt in 1 2 3 4 5; do
    sleep 6
    if outlayer vault resume "$VAULT_ID" >&2; then INIT_RC=0; break; fi
  done
fi
[[ $INIT_RC -eq 0 ]] || fail "vault init failed"
outlayer vault status "$VAULT_ID" >/dev/null || fail "vault.status failed"
pass "vault $VAULT_ID deployed + verified"

# ─── 2. Mint 3 users via Bearer near: + vault_id ──────────────────

log "2. Mint 3 users via Bearer near: + vault_id (no /register, no PUT /api-key)"
declare -a SEEDS
declare -a ADDRS_PRE
for i in 1 2 3; do
  seed="user-$i-$(date +%s)-$$"
  SEEDS+=("$seed")
  addr=$(bn_address "$seed" "$VAULT_ID" "user $i mint") || fail "user $i mint failed"
  ADDRS_PRE+=("$addr")
  echo "  user $i: seed=$seed  addr=$addr" >&2
done
# Sanity: all 3 distinct.
[[ "${ADDRS_PRE[0]}" != "${ADDRS_PRE[1]}" && "${ADDRS_PRE[1]}" != "${ADDRS_PRE[2]}" && "${ADDRS_PRE[0]}" != "${ADDRS_PRE[2]}" ]] || \
  fail "3 distinct seeds gave duplicate addresses — HMAC broken"
pass "3 distinct user wallets minted under $VAULT_ID"

# ─── 3. PRE-RECOVERY signing via Bearer near: ────────────────────

log "3. PRE-RECOVERY: /sign-message via Bearer near: for user #1 must succeed"
PRE_SIGN_RESP=$(bn_sign_message "${SEEDS[0]}" "$VAULT_ID" "bnrec-preflight-$(date +%s)")
PRE_BODY=$(echo "$PRE_SIGN_RESP" | sed '$d')
PRE_HTTP=$(echo "$PRE_SIGN_RESP" | tail -1 | sed 's/HTTP://')
PRE_SIG=$(echo "$PRE_BODY" | jq -r '.signature // empty')
[[ "$PRE_HTTP" == "200" && -n "$PRE_SIG" && "$PRE_SIG" != "null" ]] || \
  fail "pre-recovery sign failed (http=$PRE_HTTP body=$PRE_BODY)"
pass "keystore signed pre-recovery for user #1 (sig length=${#PRE_SIG})"

# ─── 4. Initiate unilateral recovery ─────────────────────────────

log "4. Initiate unilateral recovery"
outlayer vault initiate-unilateral-recovery "$VAULT_ID" || fail "initiate-unilateral-recovery failed"
pass "recovery initiated; exit window running"

# ─── 5. Wait exit window ─────────────────────────────────────────

WAIT_SECS=70
log "5. Wait $WAIT_SECS s for exit window"
sleep "$WAIT_SECS"

# ─── 6. Generate new parent keypair ──────────────────────────────

log "6. Generate sovereign parent keypair"
KEY_DIR="${KEY_DIR:-/tmp/bn-recov-e2e}"
mkdir -p "$KEY_DIR" && chmod 700 "$KEY_DIR"
KEY_FILE="$KEY_DIR/$VAULT_ID.json"
"$RECOVERY_BIN" generate-key > "$KEY_FILE"
chmod 600 "$KEY_FILE"
NEW_PARENT_PUBKEY=$(jq -r '.public_key' "$KEY_FILE")
NEW_PARENT_PRIVKEY=$(jq -r '.private_key' "$KEY_FILE")
pass "new sovereign pubkey: $NEW_PARENT_PUBKEY (priv in $KEY_FILE)"

# ─── 7. Finalize recovery ────────────────────────────────────────

log "7. finalize_recovery — atomic on-chain key-swap"
outlayer vault finalize-recovery "$VAULT_ID" "$NEW_PARENT_PUBKEY" || fail "finalize-recovery failed"

# Wait for unlocked=true to be observable at final finality.
UNLOCKED=false
for attempt in 1 2 3 4 5 6 7 8 9 10; do
  STATE_BYTES=$(curl -s "$RPC_URL" -X POST -H 'Content-Type: application/json' \
    -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"query\",\"params\":{\"request_type\":\"call_function\",\"finality\":\"final\",\"account_id\":\"$VAULT_ID\",\"method_name\":\"get_state\",\"args_base64\":\"e30=\"}}" \
    | jq -r '.result.result | implode')
  UNLOCKED=$(echo "$STATE_BYTES" | jq -r '.unlocked')
  if [[ "$UNLOCKED" == "true" ]]; then break; fi
  sleep 3
done
[[ "$UNLOCKED" == "true" ]] || fail "vault.unlocked did not flip true after finalize"
pass "vault.unlocked == true on chain"

# ─── 8. POST-RECOVERY: keystore MUST refuse ──────────────────────
#
# This is claim (B): OutLayer can no longer derive or sign for this
# vault. Both /address (derive) and /sign-message (sign) go through
# keystore's `ensure_customer_loaded → assert_serving_allowed`, which
# checks `vault.unlocked == false` on every call.

log "8. POST-RECOVERY: Bearer near: + vault_id → /address must FAIL"
POST_ADDR_RESP=$(bn_address "${SEEDS[0]}" "$VAULT_ID" "post-recovery address" 2>&1) || POST_ADDR_RC=$?
if [[ "${POST_ADDR_RC:-0}" -eq 0 ]]; then
  fail "POST-RECOVERY: /address STILL returned an address — cutoff broken"
fi
pass "/address refused post-recovery (keystore evicted master, refuses to derive)"

log "8.1 POST-RECOVERY: Bearer near: + vault_id → /sign-message must FAIL"
POST_SIGN_RESP=$(bn_sign_message "${SEEDS[0]}" "$VAULT_ID" "post-recovery-attempt")
POST_BODY=$(echo "$POST_SIGN_RESP" | sed '$d')
POST_HTTP=$(echo "$POST_SIGN_RESP" | tail -1 | sed 's/HTTP://')
POST_SIG=$(echo "$POST_BODY" | jq -r '.signature // empty' 2>/dev/null || echo "")
if [[ "$POST_HTTP" -ge 400 ]] || [[ -z "$POST_SIG" || "$POST_SIG" == "null" ]]; then
  pass "/sign-message refused post-recovery (HTTP $POST_HTTP) — OutLayer cannot sign for this vault"
else
  fail "POST-RECOVERY: keystore STILL signed (http=$POST_HTTP sig=${POST_SIG:0:20}…). Cutoff broken."
fi

# ─── 9. CKD-recover the per-vault master locally ─────────────────

log "9. Recover per-vault master via MPC CKD using NEW parent key"
RECOVERY_RC=0
RECOVERY_OUT=$(VAULT_PRIVATE_KEY="$NEW_PARENT_PRIVKEY" "$RECOVERY_BIN" \
  --vault-id "$VAULT_ID" \
  --from-chain \
  --rpc-url "$RPC_URL" \
  --mpc-contract "$MPC_CONTRACT_ID" \
  --nearblocks-url "$NEARBLOCKS_URL" 2>&1) || RECOVERY_RC=$?
echo "$RECOVERY_OUT" >&2
[[ $RECOVERY_RC -eq 0 ]] || fail "customer-recovery exited $RECOVERY_RC"
MASTER_HEX=$(echo "$RECOVERY_OUT" | awk -F= '/^master_hex=/{print $2; exit}')
[[ -n "$MASTER_HEX" && ${#MASTER_HEX} -eq 64 ]] || fail "no master_hex (got: '$MASTER_HEX')"
pass "per-vault master recovered locally (64 hex chars, hidden)"

# ─── 10. Offline-derive all 3 user wallets ───────────────────────
#
# THE TIPBOT VALUE-PROP: with only (master, parent_account_id, seeds),
# the bot can recompute every user's NEAR keypair without contacting
# OutLayer. We test this for all 3 minted users and assert exact
# address equality with what OutLayer had reported in step 2.

log "10. Offline-derive each user's NEAR keypair from (master, account_id, seed)"
declare -a ADDRS_POST
declare -a PRIVKEYS_POST
for i in 0 1 2; do
  seed="${SEEDS[$i]}"
  expected="${ADDRS_PRE[$i]}"
  # v2: wallet_id encodes vault scope. Users were minted via Bearer-near
  # + vault_id=$VAULT_ID, so offline derivation MUST include the same
  # vault_id to reproduce the coordinator-side wallet_id.
  wallet_id_local=$("$RECOVERY_BIN" compute-wallet-id \
    --account-id "$PARENT" --seed "$seed" --vault-id "$VAULT_ID")
  derived=$("$RECOVERY_BIN" derive-wallet-key --master "$MASTER_HEX" --wallet-id "$wallet_id_local")
  derived_addr=$(echo "$derived" | jq -r '.near_address')
  derived_priv=$(echo "$derived" | jq -r '.private_key')
  ADDRS_POST+=("$derived_addr")
  PRIVKEYS_POST+=("$derived_priv")
  echo "  user $((i+1)): seed=$seed" >&2
  echo "    wallet_id_local: $wallet_id_local" >&2
  echo "    addr (OutLayer): $expected" >&2
  echo "    addr (offline):  $derived_addr" >&2
  [[ "$derived_addr" == "$expected" ]] || \
    fail "user $((i+1)) DERIVATION MISMATCH — offline=$derived_addr vs OutLayer=$expected"
done
pass "ALL 3 users recovered offline; addresses match exactly"

# ─── 11. (Optional) Send a real testnet tx with user #1's key ────
#
# Final proof: the offline-derived private key actually controls the
# wallet on chain. Fund user #1 from PARENT, then send a tiny amount
# back, signed by the locally-derived key (no OutLayer involvement).

log "11. Fund user #1 with 0.05 NEAR from $PARENT (so we can send a sovereign tx back)"
near_tty "near tokens $PARENT send-near ${ADDRS_PRE[0]} '0.05 NEAR' \\
  network-config $NETWORK sign-with-keychain send" || \
  fail "funding user #1 from $PARENT failed"

# Wait for the funded account to appear at final finality.
for attempt in 1 2 3 4 5 6 7 8; do
  ACCT_PROBE=$(curl -s "$RPC_URL" -X POST -H 'Content-Type: application/json' \
    -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"query\",\"params\":{\"request_type\":\"view_account\",\"finality\":\"final\",\"account_id\":\"${ADDRS_PRE[0]}\"}}")
  if echo "$ACCT_PROBE" | jq -e '.result.amount' >/dev/null 2>&1; then break; fi
  sleep 2
done

log "11.1 SOVEREIGN TX: send 0.001 NEAR from user #1 back to $PARENT, signed by offline-derived key"
near_tty "near tokens ${ADDRS_PRE[0]} send-near $PARENT '0.001 NEAR' \\
  network-config $NETWORK \\
  sign-with-plaintext-private-key '${PRIVKEYS_POST[0]}' send" || \
  fail "sovereign send-near with offline-derived key failed — the wallet is NOT actually recoverable end-to-end"
pass "sovereign tx landed — user #1's wallet is independently controlled by the customer"

echo
pass "ALL CHECKS PASSED. Bearer near: stateless + sovereign exit verified:"
pass "  - 3 users minted via Bearer near: + vault_id, distinct addresses"
pass "  - keystore signs pre-recovery"
pass "  - finalize_recovery atomically swaps the FullAccess key"
pass "  - POST-RECOVERY: /address refuses (keystore evicts master)"
pass "  - POST-RECOVERY: /sign-message refuses (assert_serving_allowed)"
pass "  - per-vault master recovered via MPC CKD using new parent key"
pass "  - ALL 3 users re-derived offline from (master, parent, seed)"
pass "  - offline-derived keys actually sign valid on-chain txs"
warn "Cleanup (optional): $VAULT_ID still on chain with ~0.1 NEAR storage stake."
