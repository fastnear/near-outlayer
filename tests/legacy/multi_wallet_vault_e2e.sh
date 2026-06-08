#!/bin/bash
# End-to-end test for the "N wallets per vault" symmetry.
#
# Verifies migration 20260511000001 + the dropped UNIQUE constraint:
# a single sovereign vault can back arbitrarily many independent
# custody wallets, each with its own wk_ / wallet_id / derived
# address, and a sub-agent minted under one of those wallets still
# inherits the vault binding.
#
# Prerequisites:
#   * Coordinator running with the post-migration code (no
#     `already bound to a wallet` 23505 handler; partial UNIQUE
#     dropped).
#   * A vault account that's already verified on chain
#     (`is_vault_verified == true` on keystore-DAO). Easiest: run
#     `outlayer vault init --name <foo>` first, then pass that
#     sub-account name here. The wk_ from that init is NOT used by
#     this script — we mint fresh ones from /register.
#
# Run:
#   COORDINATOR_URL=https://testnet-api.outlayer.fastnear.com \
#   VAULT_ID=vault-multi.alice.testnet \
#       ./tests/multi_wallet_vault_e2e.sh --apply
#
# Defaults to dry-run.

set -euo pipefail

APPLY=false
[[ "${1:-}" == "--apply" ]] && APPLY=true

COORDINATOR_URL="${COORDINATOR_URL:-https://testnet-api.outlayer.fastnear.com}"
VAULT_ID="${VAULT_ID:-}"
N_WALLETS="${N_WALLETS:-3}"
SUB_AGENT_SEED="${SUB_AGENT_SEED:-multi-wallet-e2e-sub-1}"

log()   { printf '\n\033[36m▶ %s\033[0m\n' "$*"; }
warn()  { printf '\033[33m⚠ %s\033[0m\n' "$*"; }
fail()  { printf '\033[31m✗ %s\033[0m\n' "$*"; exit 1; }
pass()  { printf '\033[32m✓ %s\033[0m\n' "$*"; }

run() {
  if [[ "$APPLY" == true ]]; then
    log "$ $*"
    eval "$@"
  else
    printf '\033[90m  (dry-run) $ %s\033[0m\n' "$*"
  fi
}

if [[ -z "$VAULT_ID" ]]; then
  fail "VAULT_ID not set. Example:
    VAULT_ID=vault.alice.testnet ./tests/multi_wallet_vault_e2e.sh --apply"
fi

if [[ "$APPLY" != true ]]; then
  warn "Dry-run mode. Use --apply to actually hit the coordinator."
fi

# ─── 1. Mint N independent wallets bound to the same vault ─────────

declare -a WK_KEYS=()
declare -a ADDRS=()
declare -a WALLET_IDS=()

for i in $(seq 1 "$N_WALLETS"); do
  log "1.$i: POST /register with vault_id=$VAULT_ID (wallet #$i)"
  if [[ "$APPLY" == true ]]; then
    RESP=$(curl -sS -X POST "$COORDINATOR_URL/register" \
      -H "Content-Type: application/json" \
      -d "{\"vault_id\": \"$VAULT_ID\"}")
    echo "$RESP" | python3 -m json.tool

    WK=$(echo "$RESP" | python3 -c "import json,sys; print(json.load(sys.stdin).get('api_key',''))")
    WID=$(echo "$RESP" | python3 -c "import json,sys; print(json.load(sys.stdin).get('wallet_id',''))")
    ADDR=$(echo "$RESP" | python3 -c "import json,sys; print(json.load(sys.stdin).get('near_account_id',''))")

    if [[ -z "$WK" || "$WK" != wk_* ]]; then
      fail "register #$i returned no api_key: $RESP"
    fi
    WK_KEYS+=("$WK")
    WALLET_IDS+=("$WID")
    ADDRS+=("$ADDR")
  else
    printf '\033[90m  (dry-run) curl -sS -X POST %s/register -d {"vault_id":"%s"}\033[0m\n' "$COORDINATOR_URL" "$VAULT_ID"
  fi
done

# ─── 2. All wallet_ids and addresses must be distinct ──────────────

if [[ "$APPLY" == true ]]; then
  log "2: assert all wallet_ids are distinct"
  unique_wid=$(printf '%s\n' "${WALLET_IDS[@]}" | sort -u | wc -l | tr -d ' ')
  if [[ "$unique_wid" != "$N_WALLETS" ]]; then
    fail "expected $N_WALLETS distinct wallet_ids, got $unique_wid"
  fi
  pass "$N_WALLETS distinct wallet_ids"

  log "3: assert all near_account_ids are distinct"
  unique_addr=$(printf '%s\n' "${ADDRS[@]}" | sort -u | wc -l | tr -d ' ')
  if [[ "$unique_addr" != "$N_WALLETS" ]]; then
    fail "expected $N_WALLETS distinct addresses, got $unique_addr"
  fi
  pass "$N_WALLETS distinct addresses (per-wallet derivation under same vault master)"
fi

# ─── 4. Each wk_ → GET /address → must include vault_id ─────────────

for i in $(seq 1 "$N_WALLETS"); do
  idx=$((i - 1))
  WK="${WK_KEYS[$idx]:-<wk_$i>}"
  log "4.$i: GET /wallet/v1/address with WK #$i — expect vault_id=$VAULT_ID"
  if [[ "$APPLY" == true ]]; then
    A=$(curl -sS -H "Authorization: Bearer $WK" \
        "$COORDINATOR_URL/wallet/v1/address?chain=near")
    echo "$A" | python3 -m json.tool
    GOT_VAULT=$(echo "$A" | python3 -c "import json,sys; print(json.load(sys.stdin).get('vault_id',''))")
    if [[ "$GOT_VAULT" != "$VAULT_ID" ]]; then
      fail "wallet #$i's address response missing vault_id (got '$GOT_VAULT', want '$VAULT_ID')"
    fi
  else
    printf '\033[90m  (dry-run) curl -H "Authorization: Bearer wk_..." %s/wallet/v1/address?chain=near\033[0m\n' "$COORDINATOR_URL"
  fi
done
[[ "$APPLY" == true ]] && pass "all $N_WALLETS wallets show vault_id=$VAULT_ID"

# ─── 5. Sub-agent under wallet #1 inherits the vault binding ───────

log "5: PUT /wallet/v1/api-key from wallet #1 — create sub-agent"
if [[ "$APPLY" == true ]]; then
  PARENT_KEY="${WK_KEYS[0]}"
  # Recipe from agent-custody skill:
  #   sub_key   = wk_ + sha256("<seed>:0:<parent_key>")
  #   key_hash  = sha256(sub_key)
  SUB_KEY="wk_$(printf '%s:0:%s' "$SUB_AGENT_SEED" "$PARENT_KEY" | shasum -a 256 | awk '{print $1}')"
  KEY_HASH=$(printf '%s' "$SUB_KEY" | shasum -a 256 | awk '{print $1}')

  RESP=$(curl -sS -X PUT "$COORDINATOR_URL/wallet/v1/api-key" \
    -H "Authorization: Bearer $PARENT_KEY" \
    -H "Content-Type: application/json" \
    -d "{\"seed\":\"$SUB_AGENT_SEED\",\"key_hash\":\"$KEY_HASH\"}")
  echo "$RESP" | python3 -m json.tool

  log "5.1: GET /address with sub-agent — vault_id must still be $VAULT_ID"
  SUB_ADDR=$(curl -sS -H "Authorization: Bearer $SUB_KEY" \
    "$COORDINATOR_URL/wallet/v1/address?chain=near")
  echo "$SUB_ADDR" | python3 -m json.tool
  GOT_VAULT=$(echo "$SUB_ADDR" | python3 -c "import json,sys; print(json.load(sys.stdin).get('vault_id',''))")
  if [[ "$GOT_VAULT" != "$VAULT_ID" ]]; then
    fail "sub-agent address response missing vault_id (got '$GOT_VAULT', want '$VAULT_ID')"
  fi
  pass "sub-agent inherits vault_id=$VAULT_ID"

  log "5.2: sub-agent address ≠ parent #1 address"
  GOT_SUB_ADDR=$(echo "$SUB_ADDR" | python3 -c "import json,sys; print(json.load(sys.stdin).get('address',''))")
  if [[ "$GOT_SUB_ADDR" == "${ADDRS[0]}" ]]; then
    fail "sub-agent address equals parent #1 — derivation didn't fan out"
  fi
  pass "sub-agent address differs from parent (own wallet_id salt under vault master)"
else
  printf '\033[90m  (dry-run) PUT /wallet/v1/api-key with parent wk_ + seed=%s\033[0m\n' "$SUB_AGENT_SEED"
fi

# ─── 6. Cross-vault isolation (optional — needs second vault) ──────

VAULT_ID_B="${VAULT_ID_B:-}"
if [[ -n "$VAULT_ID_B" ]]; then
  log "6: cross-vault isolation — VAULT_ID_B=$VAULT_ID_B"
  if [[ "$APPLY" == true ]]; then
    RESP_B=$(curl -sS -X POST "$COORDINATOR_URL/register" \
      -H "Content-Type: application/json" \
      -d "{\"vault_id\": \"$VAULT_ID_B\"}")
    WK_B=$(echo "$RESP_B" | python3 -c "import json,sys; print(json.load(sys.stdin).get('api_key',''))")
    ADDR_B=$(curl -sS -H "Authorization: Bearer $WK_B" \
      "$COORDINATOR_URL/wallet/v1/address?chain=near")
    GOT_VAULT_B=$(echo "$ADDR_B" | python3 -c "import json,sys; print(json.load(sys.stdin).get('vault_id',''))")
    GOT_ADDR_B=$(echo "$ADDR_B" | python3 -c "import json,sys; print(json.load(sys.stdin).get('address',''))")

    log "6.1: wallet under VAULT_ID_B must report vault_id=$VAULT_ID_B"
    [[ "$GOT_VAULT_B" == "$VAULT_ID_B" ]] || \
      fail "expected vault_id=$VAULT_ID_B, got '$GOT_VAULT_B'"
    pass "B's wallet correctly bound to $VAULT_ID_B"

    log "6.2: B's address must differ from every wallet under $VAULT_ID"
    for a in "${ADDRS[@]}"; do
      [[ "$GOT_ADDR_B" != "$a" ]] || \
        fail "cross-vault collision: B's address $GOT_ADDR_B matches a wallet from $VAULT_ID"
    done
    pass "B's address ($GOT_ADDR_B) is distinct from all $N_WALLETS wallets under $VAULT_ID"

    log "6.3: spoofed X-Customer-Vault header (B claiming to be A) must be IGNORED"
    SPOOF=$(curl -sS -H "Authorization: Bearer $WK_B" \
      -H "X-Customer-Vault: $VAULT_ID" \
      "$COORDINATOR_URL/wallet/v1/address?chain=near")
    SPOOF_VAULT=$(echo "$SPOOF" | python3 -c "import json,sys; print(json.load(sys.stdin).get('vault_id',''))")
    SPOOF_ADDR=$(echo "$SPOOF" | python3 -c "import json,sys; print(json.load(sys.stdin).get('address',''))")
    [[ "$SPOOF_VAULT" == "$VAULT_ID_B" ]] || \
      fail "header spoof was honored! got vault_id=$SPOOF_VAULT, expected $VAULT_ID_B"
    [[ "$SPOOF_ADDR" == "$GOT_ADDR_B" ]] || \
      fail "header spoof changed derived address ($SPOOF_ADDR vs expected $GOT_ADDR_B)"
    pass "X-Customer-Vault spoof correctly ignored — binding is from DB, not header"
  else
    printf '\033[90m  (dry-run) mint + verify wallet under VAULT_ID_B=%s\033[0m\n' "$VAULT_ID_B"
  fi
else
  warn "VAULT_ID_B not set — skipping cross-vault isolation scenario. \
Set VAULT_ID_B=<other-vault-id> to run it."
fi

# ─── 7. Vault-vs-default isolation ─────────────────────────────────
#
# Same user can have a vault-bound wallet AND a default-master wallet
# at the same time. The default one must NOT carry a vault_id and
# must derive to a different address (different master root).

log "7: vault-vs-default isolation — mint a default-master wallet"
if [[ "$APPLY" == true ]]; then
  RESP_D=$(curl -sS -X POST "$COORDINATOR_URL/register" \
    -H "Content-Type: application/json" \
    -d '{}')
  WK_D=$(echo "$RESP_D" | python3 -c "import json,sys; print(json.load(sys.stdin).get('api_key',''))")
  ADDR_D_RESP=$(curl -sS -H "Authorization: Bearer $WK_D" \
    "$COORDINATOR_URL/wallet/v1/address?chain=near")
  DEFAULT_VAULT=$(echo "$ADDR_D_RESP" | python3 -c "import json,sys; print(repr(json.load(sys.stdin).get('vault_id')))")
  DEFAULT_ADDR=$(echo "$ADDR_D_RESP" | python3 -c "import json,sys; print(json.load(sys.stdin).get('address',''))")

  log "7.1: default-master wallet must have NO vault_id"
  [[ "$DEFAULT_VAULT" == "None" || "$DEFAULT_VAULT" == "null" ]] || \
    fail "default-master wallet leaked a vault_id: $DEFAULT_VAULT"
  pass "default wallet has no vault_id (key derives from OutLayer shared master)"

  log "7.2: default address must differ from all vault-bound addresses"
  for a in "${ADDRS[@]}"; do
    [[ "$DEFAULT_ADDR" != "$a" ]] || \
      fail "default address $DEFAULT_ADDR collides with vault-bound wallet $a — masters not isolated"
  done
  pass "default address ($DEFAULT_ADDR) distinct from every vault wallet — masters isolated"
else
  printf '\033[90m  (dry-run) mint default-master wallet via POST /register {}\033[0m\n'
fi

# ─── 8. Summary ────────────────────────────────────────────────────

if [[ "$APPLY" == true ]]; then
  log "Summary"
  echo "  vault_id: $VAULT_ID"
  for i in $(seq 1 "$N_WALLETS"); do
    idx=$((i - 1))
    echo "  wallet #$i: wallet_id=${WALLET_IDS[$idx]}  addr=${ADDRS[$idx]}"
  done
  pass "Isolation + N-wallets-per-vault + sub-agent inheritance — all checks green"
fi
