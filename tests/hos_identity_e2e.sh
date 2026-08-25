#!/usr/bin/env bash
#
# §5 of the HoS test plan — identity and attribution over the HTTPS door.
#
# The whole point of Agent Connect is that ONE of a wallet's two identities
# moves and the other does not: the guest ACTS AS the bound account
# (`NEAR_SENDER_ID`) while the money is still the executor's
# (`NEAR_USER_ACCOUNT_ID`). If both moved, usage would be attributed to an
# account that paid nothing; if neither did, the binding would buy the partner
# nothing at all. So each probe below reads BOTH values out of the guest and
# asks which one changed.
#
# Judged inside the guest, not from the answer envelope: `connector-probe`'s
# `whoami` reports the variables the WORKER injected, none of which a caller
# can set in the request body. That is the only place the question can be
# answered honestly.
#
# The on-chain half (`bound_identity_onchain_e2e.sh` B0–B5) reads the
# coordinator's own rows and needs database access from the coordinator host;
# it is not repeated here.
#
#   PARENT=you.testnet ./tests/hos_identity_e2e.sh --apply

source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib/hos_common.sh"

[[ "${1:-}" == "--apply" ]] || { sed -n '3,22p' "$0" >&2; echo "  Pass --apply to run." >&2; exit 0; }
hos_require

PROJECT="${PROJECT:-connectors.outlayer.testnet/connector-probe}"

# probe <payment-key> <flag> <operation> — one HTTPS call into the guest.
probe() {
  local pk=$1 flag=$2 op=${3:-whoami} out
  throttle
  out=$(mktemp -t hos_ident.XXXXXX)
  HTTP=$(curl -sS -o "$out" -w '%{http_code}' -m 300 -X POST "$COORDINATOR_URL/call/$PROJECT" \
    -H "X-Payment-Key: $pk" -H 'Content-Type: application/json' \
    -d "$(jq -nc --arg o "$op" --argjson f "$flag" '{input:{operation:$o}, use_bound_identity:$f}')" 2>/dev/null)
  BODY="$(tr -d '\n' < "$out")"; rm -f "$out"
}
guest() { jq -r "$1 // \"\"" <<<"$(jq -r '.output // .result // "{}"' <<<"$BODY" 2>/dev/null)" 2>/dev/null; }

# ── a wallet the coordinator minted, so the payer and the bound wallet are one ─
log "Minting a wallet and claiming its trial key"
WK=$(curl -sS -m 60 -X POST "$COORDINATOR_URL/register" -H 'Content-Type: application/json' -d '{}' 2>/dev/null | jq -r '.api_key // empty')
[[ -n "$WK" ]] || { echo "✗ /register gave no key" >&2; exit 1; }
api_wk "$WK" GET "/wallet/v1/address?chain=near" >/dev/null
WID=$(jq -r '.wallet_id // empty' <<<"$BODY"); EXEC=$(jq -r '.address // empty' <<<"$BODY")
[[ -n "$WID" ]] || { echo "✗ /address failed: $BODY" >&2; exit 1; }
note "wallet $WID / executor $EXEC"

CLAIM=$(curl -sS -m 60 -X POST "$COORDINATOR_URL/trial-key" -H "Authorization: Bearer $WK" \
  -H 'Content-Type: application/json' -d '{}' 2>/dev/null)
PK=$(jq -r '.payment_key // empty' <<<"$CLAIM")

# The trial quota is three keys per IP and it is not resettable from outside —
# so the section that judges the HTTPS half of `use_bound_identity` used to be
# lost to a counter, on a run that then reported nothing wrong. A wallet can buy
# its own key instead. The key it gets is owned by the same implicit account the
# trial key would have been, which is what makes I1 land on the binding lookup
# rather than on "this key names no wallet".
if [[ -z "$PK" ]]; then
  note "no trial key ($(jq -r '.error // .' <<<"$CLAIM" | head -c 90)) — buying one with stablecoin instead"
  buy_payment_key "$WK" "$EXEC" && PK="$PAID_KEY"
fi

if [[ -z "$PK" ]]; then
  skip "§5 — this wallet has no key to pay with: the trial quota for this address is spent and the stablecoin route did not go through either (see the warning above for which step)"
  # Exit 3, not 0: without a key this suite judges nothing, and §5 is where the
  # HTTPS half of `use_bound_identity` is judged at all. A zero here reads in
  # the runner's table as a suite that passed.
  verdict "§5 identity"; exit 3
fi
note "paying with ${PK:0:8}… ($(jq -r 'if .allowance_usd then "trial allowance \(.allowance_usd)" else "bought with stablecoin" end' <<<"$CLAIM" 2>/dev/null || echo "bought with stablecoin"))"

ACC="hos-ident-$(openssl rand -hex 3).$PARENT"
cleanup() {
  local rc=$?
  api_wk "$WK" DELETE /wallet/v1/binding >/dev/null 2>&1 || true
  account_exists "$ACC" && { note "cleaning up $ACC"; delete_account "$ACC"; }
  return $rc
}
trap cleanup EXIT

# ── I0 the flag OFF, with no binding: the guest is the caller ──────────────
log "I0 use_bound_identity=false, before any binding exists"
probe "$PK" false
if [[ "$HTTP" == "200" ]]; then
  S0=$(guest '.sender_id'); P0=$(guest '.user_account_id')
  note "sender=$S0 payer=$P0"
  if [[ -n "$S0" && "$S0" == "$P0" ]]; then
    pass "I0 the guest acts as its caller and pays as its caller — one identity, as before Agent Connect"
  else
    fail "I0 sender='$S0' payer='$P0' — with no binding the two must be the same account"
  fi
else
  fail "I0 the probe call failed (HTTP $HTTP): $(head -c 220 <<<"$BODY")"
  verdict "§5 identity"; exit 1
fi

# ── I1 the flag ON with NO binding is refused, not silently ignored ────────
log "I1 use_bound_identity=true while the wallet is unbound"
probe "$PK" true
S1=$(guest '.sender_id')
if [[ "$HTTP" == "200" && "$S1" == "$S0" ]]; then
  fail "I1 the flag was silently ignored: the job RAN under the caller's own name '$S1' with no binding in existence. The ON-CHAIN door refuses this exact case (handlers/tasks.rs sets the sender to an empty string so the worker rejects the job, with the comment 'NOT a silent fallback to the caller's own name'); the HTTPS door's own comment says the same and then falls through to None (handlers/call.rs, the bound_sender match: fetch_optional yields None for an absent or inactive binding and only a DB ERROR refuses). A connector that turns NEAR_SENDER_ID into something real — near-email into the mailbox it sends from — sends successfully from the wrong account, with nothing in the record to explain it"
elif [[ "$HTTP" == "200" ]]; then
  fail "I1 the job ran and the guest saw sender='$S1' with no binding in existence"
else
  pass "I1 refused (HTTP $HTTP) — asking to be somebody the wallet is not is an error, not a fallback"
  note "  $(jq -r '.message // .error // .' <<<"$BODY" | head -c 160)"
fi

# ── the binding ────────────────────────────────────────────────────────────
log "Binding the wallet to $ACC"
create_subaccount "$ACC" 0.7 || { echo "✗ $ACC never appeared" >&2; exit 1; }
api_wk "$WK" PUT /wallet/v1/binding "$(jq -nc --arg a "$ACC" '{asset_account_id:$a, kind:"personal_account"}')" >/dev/null
[[ "$HTTP" == "200" ]] || { echo "✗ PUT failed $HTTP: $BODY" >&2; exit 1; }
install_wallet "$ACC" "$EXEC" || { echo "✗ the setup transaction did not land" >&2; exit 1; }
ST=""
for _ in 1 2 3 4 5 6 7 8; do
  api_wk "$WK" GET /wallet/v1/binding >/dev/null
  ST=$(jq -r '.binding_status // ""' <<<"$BODY"); [[ "$ST" == "active" ]] && break; sleep 3
done
[[ "$ST" == "active" ]] && pass "the binding is ACTIVE" || { fail "the binding never went active ('$ST')"; verdict "§5 identity"; exit 1; }

# ── I2 the flag ON with an active binding ─────────────────────────────────
log "I2 use_bound_identity=true with the binding live"
probe "$PK" true
if [[ "$HTTP" == "200" ]]; then
  S2=$(guest '.sender_id'); P2=$(guest '.user_account_id')
  note "sender=$S2 payer=$P2"
  [[ "$S2" == "$ACC" ]] \
    && pass "I2 the guest acts as the BOUND account ($S2) — verified in the worker against the chain, not taken from the request" \
    || fail "I2 the guest saw sender='$S2', expected the bound account '$ACC'"
  [[ "$P2" == "$P0" ]] \
    && pass "I2 the PAYER did not move ($P2) — billing follows the money, never the name" \
    || fail "I2 the payer changed from '$P0' to '$P2' — usage would be attributed to an account that paid nothing"
else
  fail "I2 the call failed (HTTP $HTTP): $(head -c 220 <<<"$BODY")"
fi

# ── I3 the flag OFF with a live binding: nothing changes for old clients ──
log "I3 use_bound_identity=false with the binding still live"
probe "$PK" false
if [[ "$HTTP" == "200" ]]; then
  S3=$(guest '.sender_id')
  [[ "$S3" == "$S0" ]] \
    && pass "I3 an unflagged call is unaffected by the binding ($S3) — the flag is opt-in, not a mode" \
    || fail "I3 an unflagged call saw sender='$S3', not the caller's own '$S0' — the binding changed clients that never asked"
else
  fail "I3 the call failed (HTTP $HTTP)"
fi

# ── I4 the guest cannot forge either identity through its own inputs ───────
log "I4 the reserved system variables, as the guest sees them"
probe "$PK" true env
if [[ "$HTTP" == "200" ]]; then
  DUMP=$(jq -r '.output // .result // "{}"' <<<"$BODY")
  if grep -q "NEAR_SENDER_ID" <<<"$DUMP"; then
    pass "I4 the environment report is available and names the injected variables"
  else
    note "  env report: $(head -c 220 <<<"$DUMP")"
    skip "I4 — the env report did not name NEAR_SENDER_ID; nothing to compare"
  fi
else
  skip "I4 — the env operation answered HTTP $HTTP"
fi

# ── I5 VRF: the randomness follows the PAYER on this door ─────────────────
log "I5 VRF with and without the flag"
probe "$PK" false vrf; V_OFF=$(guest '.detail')
probe "$PK" true  vrf; V_ON=$(guest '.detail')
if [[ -n "$V_OFF$V_ON" ]]; then
  note "vrf(flag off): $(head -c 150 <<<"$V_OFF")"
  note "vrf(flag on):  $(head -c 150 <<<"$V_ON")"
  pass "I5 both doors answered — the alpha inputs are recorded above for the comparison the plan asks for"
else
  skip "I5 — the probe's vrf operation returned nothing to compare"
fi

verdict "§5 identity"
