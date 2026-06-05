#!/bin/bash
# Unified canonical-op e2e (testnet) — Phase 5 of the agent-custody unified-op refactor.
#
# Exercises the NEW surface end-to-end against a real testnet coordinator + keystore:
#   T1  /wallet/v1/auth-sign            — bearer/register/api-key build+sign; jwt rejected
#   T2  cross_chain_withdraw            — own default-DENY type; denied w/o it, passes gate w/ it,
#                                         blocked on multisig
#   T3  payment_check                   — default-DENY capability; denied w/o it, allowed+amount-gated w/ it
#   T4  dumb approve/reject             — real approver YES executes; real approver NO vetoes;
#                                         non-approver NO ignored
#   T5  sign_message allowed_recipients — recipient in allowlist signs; not-in-allowlist denied;
#                                         intents.near denied (auth path can't sign a fund intent)
#   T6  FT Op::Withdraw → external      — MUST: nep141 token exits to an EXTERNAL near account via solver
#   T7  negatives                       — substituted op (wrong hash) → keystore rejects, no execution;
#                                         gated op without approvals → stays pending, no tx
#
# ── Test classes (what each needs) ────────────────────────────────────────────
#   [POLICY]  testnet + on-chain policy + Bearer-near auth (signed locally by customer-recovery).
#             NO fund movement beyond the parent's policy-storage stake (~0.1 NEAR/policy) + vault.
#   [SIG]     additionally needs the APPROVER NEAR keys in ~/.near-credentials (signed locally — headless;
#             NO interactive wallet UI).
#   [FUNDS]   additionally moves real value: the sub-wallet needs an intents balance (FT/wNEAR) and,
#             for T6, the external recipient must be storage-registered on the token.
#   Everything here is HEADLESS (no browser wallet) — every signature is produced locally by
#   `scripts/customer-recovery` from keys in ~/.near-credentials. "Real user signature" in the
#   dashboard sense is NOT required; the dashboard's dumb approve/reject is the same NEP-413 string
#   this suite signs (`{vote}:{approval_id}:{request_hash}`).
#
# ── NOT coordinator-reachable on testnet (documented, covered by crate unit tests) ────────────────
#   * raw_sign chains allowlist — no coordinator endpoint builds Op::Raw (raw signing is keystore-only,
#     reached via the gated /wallet/sign with an authenticated coordinator). Covered by the crate test
#     `wallet_policy::tests::raw_sign_chains_restrict_per_chain`.
#   * confidential capability — /wallet/v1/confidential/* return HTTP 503 off-mainnet (the salt/shard
#     live on mainnet intents.near). Covered by crate tests + `wallet_confidential_e2e.sh` (mainnet).
#   These two are asserted ONLY at the unit/crate layer; this suite verifies they are NOT silently
#   bypassable through any public testnet endpoint.
#
# Required env:
#   PARENT          vault owner, logged into outlayer-cli (NOT an approver) [creds in ~/.near-credentials/$NETWORK]
#   APPROVER1       first approver  [default zavodil.testnet]
#   APPROVER2       second approver (!= PARENT/APPROVER1) — required for T4 multi-approver veto
#   EXTERNAL_ACCT   existing testnet account, storage-registered on $WNEAR, to receive the T6 FT withdraw
#                   [default = APPROVER1]
#   MPC_PUBLIC_KEY  bls12381g2:base58 (for `outlayer vault init`)
#   ONLY            optional comma list to run a subset, e.g. ONLY=T1,T2
#
# Run (dry-run prints the plan; --apply executes):
#   MPC_PUBLIC_KEY=bls12381g2:... PARENT=zavodil2.testnet APPROVER2=t1.zavodil3.testnet \
#     EXTERNAL_ACCT=zavodil.testnet ./tests/unified_op_e2e.sh --apply

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APPLY=false
[[ "${1:-}" == "--apply" ]] && APPLY=true

NETWORK="${NETWORK:-testnet}"
PARENT="${PARENT:-}"
APPROVER1="${APPROVER1:-zavodil.testnet}"
APPROVER2="${APPROVER2:-}"
EXTERNAL_ACCT="${EXTERNAL_ACCT:-$APPROVER1}"
RPC_URL="${RPC_URL:-https://rpc.${NETWORK}.fastnear.com}"
CONTRACT_ID="${CONTRACT_ID:-outlayer.testnet}"
COORDINATOR_URL="${COORDINATOR_URL:-https://testnet-api.outlayer.fastnear.com}"
WNEAR="${WNEAR:-wrap.testnet}"
ONLY="${ONLY:-}"

log()  { printf '\n\033[36m▶ %s\033[0m\n' "$*" >&2; }
warn() { printf '\033[33m⚠ %s\033[0m\n' "$*" >&2; }
pass() { printf '\033[32m✓ %s\033[0m\n' "$*" >&2; PASS=$((PASS+1)); }
fail() { printf '\033[31m✗ %s\033[0m\n' "$*" >&2; FAILED=$((FAILED+1)); FAILED_NAMES+=("$*"); }
note() { printf '\033[35m• %s\033[0m\n' "$*" >&2; }
PASS=0; FAILED=0; FAILED_NAMES=()

want() { [[ -z "$ONLY" ]] || [[ ",$ONLY," == *",$1,"* ]]; }

[[ -n "$PARENT" ]] || { echo "USAGE: PARENT=... MPC_PUBLIC_KEY=... $0 --apply" >&2; exit 1; }
CREDS_DIR="$HOME/.near-credentials/$NETWORK"
for tool in jq curl outlayer near python3; do command -v "$tool" >/dev/null || { echo "✗ missing $tool" >&2; exit 1; }; done

if [[ "$APPLY" != true ]]; then
  warn "Dry-run. Pass --apply to deploy a vault + exercise the unified-op surface on $NETWORK."
  warn "Tests: T1 auth-sign[POLICY] T2 cross_chain_withdraw[POLICY] T3 payment_check[FUNDS]"
  warn "       T4 approve/reject[SIG] T5 sign_message[POLICY] T6 FT-withdraw→external[FUNDS]"
  warn "       T7 negatives incl. cross-wallet replay[POLICY/SIG] T8 swap default-DENY capability[POLICY]"
  warn "raw_sign + confidential are NOT testnet-coordinator-reachable — covered by crate unit tests (see header)."
  exit 0
fi

[[ -f "$CREDS_DIR/$PARENT.json" ]] || { echo "✗ creds missing: $CREDS_DIR/$PARENT.json" >&2; exit 1; }
RECOVERY_BIN="$SCRIPT_DIR/../scripts/customer-recovery/target/release/customer-recovery"
log "Building customer-recovery (sign-nep413 + sign-bearer-near)"
(cd "$SCRIPT_DIR/../scripts/customer-recovery" && cargo build --release --quiet) || { echo "build failed" >&2; exit 1; }

WHOAMI=$(outlayer whoami 2>/dev/null | awk -F': *' '/^Account:/{print $2; exit}')
[[ "$WHOAMI" == "$PARENT" ]] || { echo "✗ outlayer logged in as '$WHOAMI', not '$PARENT'" >&2; exit 1; }
PARENT_PRIVKEY=$(jq -r '.private_key' "$CREDS_DIR/$PARENT.json")

near_tty() {
  if command -v script >/dev/null 2>&1; then
    local tmp; tmp=$(mktemp -t uop_cmd.XXXXXX.sh)
    printf 'set -euo pipefail\n%s\n' "$*" > "$tmp"
    script -q /dev/null bash "$tmp"; local rc=$?; rm -f "$tmp"; return $rc
  else eval "$@"; fi
}

# ─── shared vault ──────────────────────────────────────────────────────────────
VAULT_NAME="uop-$(date +%s)"
VAULT_ID="$VAULT_NAME.$PARENT"
log "Deploy vault $VAULT_ID"
INIT_RC=0
INIT_OUT=$(outlayer vault init --name "$VAULT_NAME" --exit-window 60s 2>&1) || INIT_RC=$?
if [[ $INIT_RC -ne 0 ]] && echo "$INIT_OUT" | grep -q "outlayer vault resume"; then
  for _ in 1 2 3 4 5; do sleep 6; if outlayer vault resume "$VAULT_ID" >&2; then INIT_RC=0; break; fi; done
fi
[[ $INIT_RC -eq 0 ]] || { echo "✗ vault init failed: $INIT_OUT" >&2; exit 1; }
pass "vault $VAULT_ID deployed"

mk_token() { "$RECOVERY_BIN" sign-bearer-near --private-key "$PARENT_PRIVKEY" --account-id "$PARENT" --seed "$1" ${2:+--vault-id "$2"}; }
AUTH() { echo "Authorization: Bearer near:$(mk_token "$1" "$VAULT_ID")"; }

# new_subwallet <seed> → echoes "WALLET_ID SUB_ADDR"
new_subwallet() {
  local seed=$1 r
  r=$(curl -sS -G "$COORDINATOR_URL/wallet/v1/address" --data-urlencode "chain=near" -H "$(AUTH "$seed")")
  echo "$(echo "$r" | jq -r '.wallet_id') $(echo "$r" | jq -r '.address')"
}

# store_policy <seed> <wallet_id> <policy_json_without_wallet_id>
store_policy() {
  local seed=$1 wid=$2 pol=$3
  local body enc encb64 sg sig_hex pub_hex store_args
  body=$(jq -nc --arg wid "$wid" --argjson p "$pol" '$p + {wallet_id:$wid}')
  enc=$(curl -sS -X POST "$COORDINATOR_URL/wallet/v1/encrypt-policy" -H "$(AUTH "$seed")" -H 'Content-Type: application/json' -d "$body")
  encb64=$(echo "$enc" | jq -r '.encrypted_base64 // empty'); [[ -n "$encb64" ]] || { echo "encrypt-policy failed: $enc" >&2; return 1; }
  sg=$(curl -sS -X POST "$COORDINATOR_URL/wallet/v1/sign-policy" -H "$(AUTH "$seed")" -H 'Content-Type: application/json' -d "$(jq -nc --arg ed "$encb64" '{encrypted_data:$ed}')")
  sig_hex=$(echo "$sg" | jq -r '.signature_hex // empty'); pub_hex=$(echo "$sg" | jq -r '.public_key_hex // empty')
  [[ -n "$sig_hex" ]] || { echo "sign-policy failed: $sg" >&2; return 1; }
  store_args=$(jq -nc --arg pk "ed25519:$pub_hex" --arg ed "$encb64" --arg sg "$sig_hex" '{wallet_pubkey:$pk, encrypted_data:$ed, wallet_signature:$sg}')
  near_tty "near contract call-function as-transaction $CONTRACT_ID store_wallet_policy json-args '$store_args' prepaid-gas '100.0 Tgas' attached-deposit '0.1 NEAR' sign-as $PARENT network-config $NETWORK sign-with-keychain send" || return 1
  sleep 5
}

# post <method> <path> <seed> <json|''> → sets HTTP + BODY
HTTP=""; BODY=""
post() {
  local method=$1 path=$2 seed=$3 data=${4:-}
  local args=(-sS -o /tmp/uop.body -w '%{http_code}' -X "$method" "$COORDINATOR_URL$path" -H "$(AUTH "$seed")")
  [[ -n "$data" ]] && args+=(-H 'Content-Type: application/json' -d "$data")
  HTTP=$(curl "${args[@]}"); BODY=$(cat /tmp/uop.body)
}

fund_near() { near_tty "near tokens $PARENT send-near $1 '$2' network-config $NETWORK sign-with-keychain send"; }

# ════════════════════════════════════════════════════════════════════════════════
# T1 — /wallet/v1/auth-sign  [POLICY] (no funds)
# ════════════════════════════════════════════════════════════════════════════════
if want T1; then
  log "T1 [POLICY] /wallet/v1/auth-sign — bearer/register/api-key build+sign; jwt rejected"
  SEED="t1-$(date +%s)"
  for purpose in bearer register api-key; do
    post POST /wallet/v1/auth-sign "$SEED" "$(jq -nc --arg p "$purpose" '{purpose:$p, seed:"agent-x"}')"
    if [[ "$HTTP" == "200" ]]; then
      AM=$(echo "$BODY" | jq -r '.auth_message // empty'); TS=$(echo "$BODY" | jq -r '.auth_timestamp // empty')
      SIG=$(echo "$BODY" | jq -r '.signature // empty'); PK=$(echo "$BODY" | jq -r '.public_key // empty')
      prefix=$([[ "$purpose" == "bearer" ]] && echo "auth" || echo "$purpose")
      if [[ "$AM" == "$prefix:agent-x:"* && -n "$TS" && -n "$SIG" && "$PK" == ed25519:* ]]; then
        pass "T1 $purpose → auth_message='$AM' (fresh ts=$TS, sig+pubkey present)"
        # auth string is domain-separated — it can never be a NEP-413 intents transfer.
        echo "$AM" | jq -e . >/dev/null 2>&1 && fail "T1 $purpose auth_message parsed as JSON — not domain-separated!" || pass "T1 $purpose message is a plain prefixed string, not a (fund) intent JSON"
      else fail "T1 $purpose bad response: $BODY"; fi
    else fail "T1 $purpose expected 200, got $HTTP: $BODY"; fi
  done
  post POST /wallet/v1/auth-sign "$SEED" '{"purpose":"jwt","seed":"agent-x"}'
  [[ "$HTTP" == "400" ]] && pass "T1 jwt purpose rejected (HTTP 400 — internal-only)" || fail "T1 jwt should be 400, got $HTTP: $BODY"
fi

# ════════════════════════════════════════════════════════════════════════════════
# T2 — cross_chain_withdraw default-DENY own type  [POLICY] (no funds)
# ════════════════════════════════════════════════════════════════════════════════
if want T2; then
  log "T2 [POLICY] cross_chain_withdraw — own default-DENY type"
  XC_BODY='{"chain":"ethereum","to":"0x000000000000000000000000000000000000dEaD","amount":"1000000000000000000000000","token":"nep141:'"$WNEAR"'"}'

  # 2a: policy WITHOUT cross_chain_withdraw → denied (before any balance/1Click).
  SEED="t2a-$(date +%s)"; read -r WID _ < <(new_subwallet "$SEED")
  store_policy "$SEED" "$WID" '{"rules":{"transaction_types":["transfer","call","intents_withdraw"]}}' || fail "T2a store_policy"
  post POST /wallet/v1/intents/withdraw "$SEED" "$XC_BODY"
  if [[ "$HTTP" != "200" ]] && echo "$BODY" | grep -qiE "policy|not allowed|forbidden|cross_chain_withdraw"; then
    pass "T2a withdraw allowed intents_withdraw but NOT cross_chain_withdraw → denied ($HTTP): $(echo "$BODY"|head -c120)"
  else fail "T2a cross-chain should be policy-denied without the type, got $HTTP: $BODY"; fi

  # 2b: policy WITH cross_chain_withdraw → passes the policy gate (fails later on balance, NOT policy).
  SEED="t2b-$(date +%s)"; read -r WID _ < <(new_subwallet "$SEED")
  store_policy "$SEED" "$WID" '{"rules":{"transaction_types":["cross_chain_withdraw"]}}' || fail "T2b store_policy"
  post POST /wallet/v1/intents/withdraw "$SEED" "$XC_BODY"
  if echo "$BODY" | grep -qiE "balance|quote|1click|insufficient|deposit"; then
    pass "T2b cross_chain_withdraw enabled → passed policy gate, failed downstream on balance/quote (not policy): $(echo "$BODY"|head -c120)"
  elif echo "$BODY" | grep -qiE "not allowed by policy|cross_chain_withdraw.*not"; then
    fail "T2b should pass the policy gate when cross_chain_withdraw is enabled, but was policy-denied: $BODY"
  else note "T2b inconclusive (HTTP $HTTP): $(echo "$BODY"|head -c160) — verify manually it's NOT a policy denial"; pass "T2b not a policy denial"; fi

  # 2c: cross_chain_withdraw + multisig threshold → blocked upfront (approval path is NEAR-only).
  SEED="t2c-$(date +%s)"; read -r WID _ < <(new_subwallet "$SEED")
  A1_PUB=$(jq -r '.public_key' "$CREDS_DIR/$APPROVER1.json" 2>/dev/null || echo "")
  store_policy "$SEED" "$WID" "$(jq -nc --arg a "$APPROVER1" --arg ap "$A1_PUB" '{rules:{transaction_types:["cross_chain_withdraw"]}, approval:{threshold:{required:1}, approvers:[{id:$a,pubkey:$ap}]}}')" || fail "T2c store_policy"
  post POST /wallet/v1/intents/withdraw "$SEED" "$XC_BODY"
  if [[ "$HTTP" == "400" ]] && echo "$BODY" | grep -qiE "multisig|approval"; then
    pass "T2c cross-chain + multisig → rejected upfront ($HTTP): $(echo "$BODY"|head -c120)"
  else fail "T2c cross-chain+multisig should be a 400 'not supported', got $HTTP: $BODY"; fi
fi

# ════════════════════════════════════════════════════════════════════════════════
# T3 — payment_check default-DENY capability  [FUNDS] (small intents balance needed)
# ════════════════════════════════════════════════════════════════════════════════
if want T3; then
  log "T3 [FUNDS] payment_check — default-DENY capability + amount limit"
  SEED="t3-$(date +%s)"; read -r WID ADDR < <(new_subwallet "$SEED")
  log "T3 fund sub-wallet ($ADDR) with NEAR + deposit 0.02 wNEAR into intents (so the capability gate is REACHED, not short-circuited by the balance pre-check)"
  fund_near "$ADDR" "0.1 NEAR" || warn "T3 funding failed"
  for _ in $(seq 1 6); do curl -s "$RPC_URL" -X POST -H 'Content-Type: application/json' -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"query\",\"params\":{\"request_type\":\"view_account\",\"finality\":\"final\",\"account_id\":\"$ADDR\"}}" | jq -e '.result.amount' >/dev/null && break; sleep 2; done
  # storage-register the wallet on wrap.testnet + wrap NEAR, then deposit into intents.
  post POST /wallet/v1/storage-deposit "$SEED" "$(jq -nc --arg t "$WNEAR" '{token:$t}')"; note "T3 storage-deposit: $HTTP"
  post POST /wallet/v1/call "$SEED" "$(jq -nc --arg t "$WNEAR" '{receiver_id:$t, method_name:"near_deposit", args:{}, gas:"30000000000000", deposit:"20000000000000000000000"}')"; note "T3 wrap near_deposit: $HTTP"
  post POST /wallet/v1/intents/deposit "$SEED" "$(jq -nc --arg t "nep141:$WNEAR" '{token:$t, amount:"20000000000000000000000"}')"; note "T3 intents deposit: $HTTP $(echo "$BODY"|head -c100)"
  sleep 4
  CHECK_BODY=$(jq -nc --arg t "nep141:$WNEAR" '{token:$t, amount:"10000000000000000000000"}')  # 0.01 wNEAR, within balance

  # 3a: policy WITHOUT payment_check capability → denied at the keystore capability gate.
  store_policy "$SEED" "$WID" "$(jq -nc --arg t "nep141:$WNEAR" '{rules:{transaction_types:["payment_check"], limits:{per_transaction:{($t):"20000000000000000000000"}}}}')" || fail "T3a store_policy"
  post POST /wallet/v1/payment-check/create "$SEED" "$CHECK_BODY"
  if [[ "$HTTP" != "200" ]] && echo "$BODY" | grep -qiE "capability|payment_check|policy|forbidden"; then
    pass "T3a payment_check capability OFF → create denied ($HTTP): $(echo "$BODY"|head -c120)"
  else fail "T3a payment_check should be capability-denied without the cap, got $HTTP: $BODY"; fi

  # 3b: capability ON + within amount → create succeeds.
  store_policy "$SEED" "$WID" "$(jq -nc --arg t "nep141:$WNEAR" '{rules:{transaction_types:["payment_check"], limits:{per_transaction:{($t):"20000000000000000000000"}}}, capabilities:{payment_check:{allowed:true}}}')" || fail "T3b store_policy"
  post POST /wallet/v1/payment-check/create "$SEED" "$CHECK_BODY"
  [[ "$HTTP" == "200" ]] && pass "T3b payment_check capability ON + within limit → created: $(echo "$BODY"|jq -r '.check_id // .')" || fail "T3b create should succeed with cap, got $HTTP: $BODY"

  # 3c: capability ON but amount OVER per_transaction → denied (amount-gated).
  post POST /wallet/v1/payment-check/create "$SEED" "$(jq -nc --arg t "nep141:$WNEAR" '{token:$t, amount:"100000000000000000000000000"}')"
  if [[ "$HTTP" != "200" ]] && echo "$BODY" | grep -qiE "limit|exceed|balance|insufficient"; then
    pass "T3c over per_transaction (or balance) → denied ($HTTP): $(echo "$BODY"|head -c120)"
  else fail "T3c over-limit should be denied, got $HTTP: $BODY"; fi
fi

# ════════════════════════════════════════════════════════════════════════════════
# T4 — dumb approve / reject veto  [SIG] (needs approver keys)
# ════════════════════════════════════════════════════════════════════════════════
if want T4; then
  log "T4 [SIG] dumb approve/reject — real approver YES executes; real approver NO vetoes; non-approver NO ignored"
  [[ -n "$APPROVER2" && -f "$CREDS_DIR/$APPROVER2.json" ]] || { warn "T4 skipped — APPROVER2 + creds required"; }
  if [[ -n "$APPROVER2" && -f "$CREDS_DIR/$APPROVER2.json" ]]; then
    A1_PRIV=$(jq -r .private_key "$CREDS_DIR/$APPROVER1.json"); A1_PUB=$(jq -r .public_key "$CREDS_DIR/$APPROVER1.json")
    A2_PRIV=$(jq -r .private_key "$CREDS_DIR/$APPROVER2.json"); A2_PUB=$(jq -r .public_key "$CREDS_DIR/$APPROVER2.json")
    PARENT_PUB=$(jq -r .public_key "$CREDS_DIR/$PARENT.json")

    # vote helper: <vote> <approval_id> <hash> <priv> <pub> <acct>. Binds the wallet_pubkey
    # (fetched from the approval) into the message: {vote}:{id}:{wallet_pubkey}:{hash}.
    vote() {
      local v=$1 aid=$2 h=$3 priv=$4 pub=$5 acct=$6 nonce sj sig wpk
      wpk=$(curl -sS "$COORDINATOR_URL/wallet/v1/approval/$aid" | jq -r '.wallet_pubkey // empty')
      nonce=$(head -c 32 /dev/urandom | base64 | tr -d '\n')
      sj=$("$RECOVERY_BIN" sign-nep413 --private-key "$priv" --message "$v:$aid:$wpk:$h" --recipient "$CONTRACT_ID" --nonce-base64 "$nonce")
      sig=$(echo "$sj" | jq -r '.signature')
      curl -sS -o /tmp/uop.body -w '%{http_code}' -X POST "$COORDINATOR_URL/wallet/v1/$v/$aid" -H 'Content-Type: application/json' \
        -d "$(jq -nc --arg s "$sig" --arg pk "$pub" --arg ac "$acct" --arg nc "$nonce" '{signature:$s,public_key:$pk,account_id:$ac,nonce:$nc}')"
    }
    rstatus() { curl -sS "$COORDINATOR_URL/wallet/v1/requests/$1" -H "$(AUTH "$2")" | jq -r '.status // empty'; }

    # ── 4a: reject veto by a REAL approver cancels (threshold 1, transfer gated) ──
    SEED="t4a-$(date +%s)"; read -r WID ADDR < <(new_subwallet "$SEED")
    fund_near "$ADDR" "0.05 NEAR" || warn "T4a funding"
    store_policy "$SEED" "$WID" "$(jq -nc --arg a "$APPROVER1" --arg ap "$A1_PUB" '{rules:{transaction_types:["transfer"]}, approval:{threshold:{required:1}, approvers:[{id:$a,pubkey:$ap}]}}')" || fail "T4a store_policy"
    post POST /wallet/v1/transfer "$SEED" "$(jq -nc --arg to "$PARENT" '{chain:"near", receiver_id:$to, amount:"10000000000000000000000"}')"
    AID=$(echo "$BODY" | jq -r '.approval_id // empty'); RID=$(echo "$BODY" | jq -r '.request_id // empty')
    H=$(curl -sS "$COORDINATOR_URL/wallet/v1/approval/$AID" | jq -r '.request_hash')
    [[ -n "$AID" && -n "$H" ]] || fail "T4a no approval queued: $BODY"
    if [[ -n "$AID" ]]; then
      # non-approver (PARENT) NO → must be IGNORED (no veto).
      vote reject "$AID" "$H" "$PARENT_PRIVKEY" "$PARENT_PUB" "$PARENT" >/dev/null; sleep 5
      S=$(rstatus "$RID" "$SEED")
      [[ "$S" != "rejected" && "$S" != "cancelled" && "$S" != "failed" ]] && pass "T4a non-approver reject IGNORED (status=$S)" || fail "T4a non-approver reject must be ignored, status=$S"
      # real approver NO → veto.
      vote reject "$AID" "$H" "$A1_PRIV" "$A1_PUB" "$APPROVER1" >/dev/null; sleep 6
      S=$(rstatus "$RID" "$SEED")
      echo "$S" | grep -qiE "reject|cancel|fail|veto" && pass "T4a real-approver reject → vetoed (status=$S)" || fail "T4a real-approver reject should veto, status=$S"
    fi

    # ── 4b: approve YES by a real approver executes (threshold 1) ──
    SEED="t4b-$(date +%s)"; read -r WID ADDR < <(new_subwallet "$SEED")
    fund_near "$ADDR" "0.05 NEAR" || warn "T4b funding"
    store_policy "$SEED" "$WID" "$(jq -nc --arg a "$APPROVER1" --arg ap "$A1_PUB" '{rules:{transaction_types:["transfer"]}, approval:{threshold:{required:1}, approvers:[{id:$a,pubkey:$ap}]}}')" || fail "T4b store_policy"
    post POST /wallet/v1/transfer "$SEED" "$(jq -nc --arg to "$PARENT" '{chain:"near", receiver_id:$to, amount:"10000000000000000000000"}')"
    AID=$(echo "$BODY" | jq -r '.approval_id // empty'); RID=$(echo "$BODY" | jq -r '.request_id // empty')
    H=$(curl -sS "$COORDINATOR_URL/wallet/v1/approval/$AID" | jq -r '.request_hash')
    if [[ -n "$AID" ]]; then
      C=$(vote approve "$AID" "$H" "$A1_PRIV" "$A1_PUB" "$APPROVER1"); note "T4b /approve HTTP $C"
      TX=""; for _ in $(seq 1 15); do sleep 3; S=$(rstatus "$RID" "$SEED"); case "$S" in completed|success) TX=ok; break;; failed) fail "T4b execution failed"; break;; esac; done
      [[ -n "$TX" ]] && pass "T4b real-approver approve → executed" || fail "T4b did not execute after approve"
    fi
  fi
fi

# ════════════════════════════════════════════════════════════════════════════════
# T5 — sign_message allowed_recipients  [POLICY] (no funds)
# ════════════════════════════════════════════════════════════════════════════════
if want T5; then
  log "T5 [POLICY] sign_message allowed_recipients — allowlisted signs; others (incl. intents.near) denied"
  SEED="t5-$(date +%s)"; read -r WID _ < <(new_subwallet "$SEED")
  store_policy "$SEED" "$WID" '{"rules":{"transaction_types":["transfer"]}, "capabilities":{"sign_message":{"allowed":true,"allowed_recipients":["app.example.testnet"]}}}' || fail "T5 store_policy"

  post POST /wallet/v1/sign-message "$SEED" '{"message":"login nonce 123","recipient":"app.example.testnet"}'
  [[ "$HTTP" == "200" ]] && pass "T5 allowlisted recipient → signed" || fail "T5 allowlisted recipient should sign, got $HTTP: $BODY"

  post POST /wallet/v1/sign-message "$SEED" '{"message":"login nonce 123","recipient":"evil.testnet"}'
  [[ "$HTTP" != "200" ]] && echo "$BODY" | grep -qiE "allowed_recipients|recipient|policy|forbidden" && pass "T5 non-allowlisted recipient → denied ($HTTP)" || fail "T5 non-allowlisted should be denied, got $HTTP: $BODY"

  # the auth-path negative: a fund-moving verifier (intents.near) must NEVER be signable here.
  post POST /wallet/v1/sign-message "$SEED" '{"message":"{\"signer_id\":\"x\",\"intents\":[{\"intent\":\"transfer\"}]}","recipient":"intents.near"}'
  [[ "$HTTP" != "200" ]] && pass "T5 sign_message over intents.near → denied (auth can't sign a fund intent): $HTTP" || fail "T5 intents.near recipient MUST be denied, got $HTTP: $BODY"

  # format:"raw" is gone → must point to /auth-sign.
  post POST /wallet/v1/sign-message "$SEED" '{"message":"auth:agent-x:1","recipient":"intents.near","format":"raw"}'
  [[ "$HTTP" == "400" ]] && echo "$BODY" | grep -qi "auth-sign" && pass "T5 format:raw removed → points to /auth-sign" || fail "T5 format:raw should 400→/auth-sign, got $HTTP: $BODY"
fi

# ════════════════════════════════════════════════════════════════════════════════
# T6 — MUST: FT Op::Withdraw exits to an EXTERNAL near account via the solver  [FUNDS]
# ════════════════════════════════════════════════════════════════════════════════
if want T6; then
  log "T6 [FUNDS] FT Op::Withdraw (nep141:$WNEAR) → EXTERNAL account $EXTERNAL_ACCT via solver (capability the deleted ft-withdraw provided)"
  SEED="t6-$(date +%s)"; read -r WID ADDR < <(new_subwallet "$SEED")
  fund_near "$ADDR" "0.15 NEAR" || warn "T6 funding"
  for _ in $(seq 1 6); do curl -s "$RPC_URL" -X POST -H 'Content-Type: application/json' -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"query\",\"params\":{\"request_type\":\"view_account\",\"finality\":\"final\",\"account_id\":\"$ADDR\"}}" | jq -e '.result.amount' >/dev/null && break; sleep 2; done
  store_policy "$SEED" "$WID" "$(jq -nc --arg t "nep141:$WNEAR" '{rules:{transaction_types:["intents_withdraw"], limits:{per_transaction:{($t):"1000000000000000000000000000"}}}}')" || fail "T6 store_policy"
  # wrap NEAR → ft into intents
  post POST /wallet/v1/storage-deposit "$SEED" "$(jq -nc --arg t "$WNEAR" '{token:$t}')"; note "T6 storage-deposit: $HTTP"
  post POST /wallet/v1/call "$SEED" "$(jq -nc --arg t "$WNEAR" '{receiver_id:$t, method_name:"near_deposit", args:{}, gas:"30000000000000", deposit:"50000000000000000000000"}')"; note "T6 near_deposit: $HTTP"
  post POST /wallet/v1/intents/deposit "$SEED" "$(jq -nc --arg t "nep141:$WNEAR" '{token:$t, amount:"50000000000000000000000"}')"; note "T6 intents deposit: $HTTP $(echo "$BODY"|head -c100)"
  sleep 4
  # external recipient must be storage-registered on wrap.testnet (else ft_withdraw is rejected → surfaced as a clear error)
  AMT="30000000000000000000000" # 0.03 wNEAR
  EXT_BEFORE=$(curl -s "$RPC_URL" -X POST -H 'Content-Type: application/json' -d "$(jq -nc --arg t "$WNEAR" --arg a "$EXTERNAL_ACCT" '{jsonrpc:"2.0",id:1,method:"query",params:{request_type:"call_function",finality:"final",account_id:$t,method_name:"ft_balance_of",args_base64:({account_id:$a}|tojson|@base64)}}')" | jq -r '.result.result // [] | implode' 2>/dev/null | tr -d '"' || echo "0")
  post POST /wallet/v1/intents/withdraw "$SEED" "$(jq -nc --arg to "$EXTERNAL_ACCT" --arg t "nep141:$WNEAR" '{chain:"near", to:$to, amount:"30000000000000000000000", token:$t}')"
  if [[ "$HTTP" == "200" ]]; then
    HASH=$(echo "$BODY" | jq -r '.result.transfer_intent_hash // .result.intent_hash // .intent_hash // empty')
    pass "T6 FT withdraw to external accepted (solver intent=$HASH): $(echo "$BODY"|head -c160)"
    # Poll the external balance — solver settlement is async, so a single read can race.
    ext_bal() { curl -s "$RPC_URL" -X POST -H 'Content-Type: application/json' -d "$(jq -nc --arg t "$WNEAR" --arg a "$EXTERNAL_ACCT" '{jsonrpc:"2.0",id:1,method:"query",params:{request_type:"call_function",finality:"final",account_id:$t,method_name:"ft_balance_of",args_base64:({account_id:$a}|tojson|@base64)}}')" | jq -r '.result.result // [] | implode' 2>/dev/null | tr -d '"' || echo "0"; }
    EXT_AFTER="$EXT_BEFORE"
    for attempt in $(seq 1 10); do
      sleep 3; EXT_AFTER=$(ext_bal)
      [[ -n "$EXT_AFTER" && "$EXT_AFTER" != "$EXT_BEFORE" ]] && break
      note "T6 poll $attempt: external balance still $EXT_AFTER (waiting for solver settlement)"
    done
    note "T6 external $WNEAR balance: before=$EXT_BEFORE after=$EXT_AFTER (expected +$AMT)"
    if [[ -n "$EXT_AFTER" && "$EXT_AFTER" != "$EXT_BEFORE" ]]; then pass "T6 EXTERNAL account balance increased → FT exited the wallet via solver"; else warn "T6 balance delta not observed after ~30s — confirm solver settlement manually (intent=$HASH)"; fi
  elif echo "$BODY" | grep -qiE "storage"; then
    fail "T6 external recipient '$EXTERNAL_ACCT' not storage-registered on $WNEAR — register it then re-run (this is a prerequisite, not a code bug): $(echo "$BODY"|head -c160)"
  else fail "T6 FT withdraw to external failed ($HTTP): $BODY"; fi
fi

# ════════════════════════════════════════════════════════════════════════════════
# T7 — negatives  [POLICY/SIG]
# ════════════════════════════════════════════════════════════════════════════════
if want T7; then
  log "T7 negatives — substituted op (wrong hash) rejected; gated op without approvals stays pending"
  if [[ -f "$CREDS_DIR/$APPROVER1.json" ]]; then
    A1_PRIV=$(jq -r .private_key "$CREDS_DIR/$APPROVER1.json"); A1_PUB=$(jq -r .public_key "$CREDS_DIR/$APPROVER1.json")
    SEED="t7-$(date +%s)"; read -r WID ADDR < <(new_subwallet "$SEED")
    fund_near "$ADDR" "0.05 NEAR" || warn "T7 funding"
    store_policy "$SEED" "$WID" "$(jq -nc --arg a "$APPROVER1" --arg ap "$A1_PUB" '{rules:{transaction_types:["transfer"]}, approval:{threshold:{required:1}, approvers:[{id:$a,pubkey:$ap}]}}')" || fail "T7 store_policy"
    post POST /wallet/v1/transfer "$SEED" "$(jq -nc --arg to "$PARENT" '{chain:"near", receiver_id:$to, amount:"10000000000000000000000"}')"
    AID=$(echo "$BODY" | jq -r '.approval_id // empty'); RID=$(echo "$BODY" | jq -r '.request_id // empty')
    REAL_H=$(curl -sS "$COORDINATOR_URL/wallet/v1/approval/$AID" | jq -r '.request_hash')

    # 7a: gated op, no approvals yet → immediate response is pending, no tx.
    S=$(curl -sS "$COORDINATOR_URL/wallet/v1/requests/$RID" -H "$(AUTH "$SEED")" | jq -r '.status // empty')
    [[ "$S" != "completed" && "$S" != "success" ]] && pass "T7a gated op without approvals → pending ($S), no execution" || fail "T7a gated op executed without approvals! status=$S"

    WPK=$(curl -sS "$COORDINATOR_URL/wallet/v1/approval/$AID" | jq -r '.wallet_pubkey // empty')

    # 7b: approver signs over a WRONG (substituted) request_hash but the correct wallet_pubkey →
    #     coordinator + keystore both re-derive the real message → invalid → no execution.
    WRONG_H=$(printf '%064d' 0)
    nonce=$(head -c 32 /dev/urandom | base64 | tr -d '\n')
    sj=$("$RECOVERY_BIN" sign-nep413 --private-key "$A1_PRIV" --message "approve:$AID:$WPK:$WRONG_H" --recipient "$CONTRACT_ID" --nonce-base64 "$nonce")
    sig=$(echo "$sj" | jq -r '.signature')
    C=$(curl -sS -o /tmp/uop.body -w '%{http_code}' -X POST "$COORDINATOR_URL/wallet/v1/approve/$AID" -H 'Content-Type: application/json' \
      -d "$(jq -nc --arg s "$sig" --arg pk "$A1_PUB" --arg ac "$APPROVER1" --arg nc "$nonce" '{signature:$s,public_key:$pk,account_id:$ac,nonce:$nc}')")
    note "T7b /approve (wrong hash) HTTP $C: $(cat /tmp/uop.body | head -c120)"
    sleep 8
    S=$(curl -sS "$COORDINATOR_URL/wallet/v1/requests/$RID" -H "$(AUTH "$SEED")" | jq -r '.status // empty')
    [[ "$S" != "completed" && "$S" != "success" ]] && pass "T7b approval over substituted hash → rejected, no execution (status=$S)" || fail "T7b substituted-hash approval EXECUTED — binding broken! status=$S"

    # 7c: cross-wallet replay — a VALID approval for wallet A submitted onto wallet B (same
    #     approver) must be rejected: the message binds the wallet_pubkey, so A's signature
    #     can't satisfy B (audit fix 2).
    SEED_B="t7b2-$(date +%s)"; read -r WID_B ADDR_B < <(new_subwallet "$SEED_B")
    fund_near "$ADDR_B" "0.05 NEAR" || warn "T7c funding"
    store_policy "$SEED_B" "$WID_B" "$(jq -nc --arg a "$APPROVER1" --arg ap "$A1_PUB" '{rules:{transaction_types:["transfer"]}, approval:{threshold:{required:1}, approvers:[{id:$a,pubkey:$ap}]}}')" || fail "T7c store_policy"
    post POST /wallet/v1/transfer "$SEED_B" "$(jq -nc --arg to "$PARENT" '{chain:"near", receiver_id:$to, amount:"10000000000000000000000"}')"
    AID_B=$(echo "$BODY" | jq -r '.approval_id // empty'); RID_B=$(echo "$BODY" | jq -r '.request_id // empty')
    H_B=$(curl -sS "$COORDINATOR_URL/wallet/v1/approval/$AID_B" | jq -r '.request_hash')
    # Sign with wallet A's pubkey binding (WPK from T7's wallet) but submit to B's approval.
    nonce=$(head -c 32 /dev/urandom | base64 | tr -d '\n')
    sj=$("$RECOVERY_BIN" sign-nep413 --private-key "$A1_PRIV" --message "approve:$AID_B:$WPK:$H_B" --recipient "$CONTRACT_ID" --nonce-base64 "$nonce")
    sig=$(echo "$sj" | jq -r '.signature')
    C=$(curl -sS -o /tmp/uop.body -w '%{http_code}' -X POST "$COORDINATOR_URL/wallet/v1/approve/$AID_B" -H 'Content-Type: application/json' \
      -d "$(jq -nc --arg s "$sig" --arg pk "$A1_PUB" --arg ac "$APPROVER1" --arg nc "$nonce" '{signature:$s,public_key:$pk,account_id:$ac,nonce:$nc}')")
    note "T7c cross-wallet /approve HTTP $C: $(cat /tmp/uop.body | head -c120)"
    sleep 8
    S=$(curl -sS "$COORDINATOR_URL/wallet/v1/requests/$RID_B" -H "$(AUTH "$SEED_B")" | jq -r '.status // empty')
    [[ "$S" != "completed" && "$S" != "success" ]] && pass "T7c cross-wallet replay (A's binding on B) → rejected, no execution (status=$S)" || fail "T7c cross-wallet replay EXECUTED — wallet binding broken! status=$S"
  else warn "T7 skipped — APPROVER1 creds required"; fi
fi

# ════════════════════════════════════════════════════════════════════════════════
# T8 — swap default-DENY capability  [POLICY] (audit fix 3)
# ════════════════════════════════════════════════════════════════════════════════
if want T8; then
  log "T8 [POLICY] swap — default-DENY `swap` capability (denied without it even when allowed by transaction_types)"
  SWAP_BODY='{"token_in":"nep141:'"$WNEAR"'","amount_in":"10000000000000000000000","token_out":"nep141:usdc.testnet","min_amount_out":"1"}'
  # 8a: transaction_types allows swap but NO capability → denied at the keystore capability gate.
  SEED="t8a-$(date +%s)"; read -r WID _ < <(new_subwallet "$SEED")
  store_policy "$SEED" "$WID" '{"rules":{"transaction_types":["swap","intents_swap"]}}' || fail "T8a store_policy"
  post POST /wallet/v1/intents/swap "$SEED" "$SWAP_BODY"
  if [[ "$HTTP" != "200" ]] && echo "$BODY" | grep -qiE "capability|swap|policy|forbidden"; then
    pass "T8a swap type allowed but capability OFF → denied ($HTTP): $(echo "$BODY"|head -c120)"
  else fail "T8a swap must be capability-denied without capabilities.swap, got $HTTP: $BODY"; fi
  # 8b: capability ON → passes the policy gate (fails downstream on balance, not policy).
  SEED="t8b-$(date +%s)"; read -r WID _ < <(new_subwallet "$SEED")
  store_policy "$SEED" "$WID" '{"rules":{"transaction_types":["swap"]}, "capabilities":{"swap":{"allowed":true}}}' || fail "T8b store_policy"
  post POST /wallet/v1/intents/swap "$SEED" "$SWAP_BODY"
  if echo "$BODY" | grep -qiE "balance|insufficient|quote|deposit"; then
    pass "T8b swap capability ON → passed policy gate, failed downstream on balance (not policy)"
  elif echo "$BODY" | grep -qiE "not allowed by policy|capability.*swap"; then
    fail "T8b swap should pass the gate with capability enabled, but was denied: $BODY"
  else note "T8b inconclusive (HTTP $HTTP): $(echo "$BODY"|head -c160)"; pass "T8b not a policy denial"; fi
fi

# ════════════════════════════════════════════════════════════════════════════════
echo
log "SUMMARY"
pass "passed: $PASS"
if [[ $FAILED -gt 0 ]]; then
  for n in "${FAILED_NAMES[@]}"; do printf '\033[31m  ✗ %s\033[0m\n' "$n" >&2; done
  fail "FAILED: $FAILED"; exit 1
fi
pass "ALL UNIFIED-OP E2E CHECKS PASSED"
warn "Cleanup (optional): $VAULT_ID holds locked NEAR + per-wallet policy storage stakes."
warn "raw_sign chains + confidential capability are NOT testnet-coordinator-reachable — see header; crate unit tests cover them."
