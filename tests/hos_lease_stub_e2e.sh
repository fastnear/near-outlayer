#!/usr/bin/env bash
#
# §3.1 + §6(leased) of the HoS test plan — the `hos_lease` profile end to end,
# against the stub contract from §10.
#
# Everything below runs through the real coordinator, the real pre-flight and
# the real chain. What is simulated is only the PARTNER'S ANSWER: the stub
# serves `hos_agent_status` (and `nft_item_info`) and a test sets it to
# whatever state the case is about — no grant, an expired grant, a frozen
# account, a lease that ran out, a version we have no decoder for.
#
# The boundary, stated so nobody has to infer it: this proves that OUR side
# agrees with the shape and the rules of their view, in the order their
# contract checks them. It does NOT prove their contract panics at the same
# rung — only a leased account can, and that is the one line of acceptance
# that waits for their TLA. The golden vectors in the crate
# (`hos_contract_vectors`, tag `valhalla-2026-08`) already pin the agreement of
# the RULES; this pins the agreement of the FLOW.
#
# Two things the suite is careful about:
#   * the observation cache is 5 s, so every status change is followed by a
#     wait — otherwise a case would be judged against the previous state and
#     the suite would be measuring its own impatience;
#   * the wallet policy is deliberately empty of address/token/limit rules, so
#     that anything refused here is refused by the LEASED PROFILE and not by
#     the ordinary custody rules that §4 already covers.
#
#   PARENT=you.testnet ./tests/hos_lease_stub_e2e.sh --apply
#   KEEP=1 leaves the stub account alive for a re-run.

source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib/hos_common.sh"

[[ "${1:-}" == "--apply" ]] || { sed -n '3,29p' "$0" >&2; echo "  Pass --apply to run." >&2; exit 0; }
hos_require

STUB_WASM="$REPO_ROOT/tests/hos-status-stub/target/near/hos_status_stub.wasm"
[[ -f "$STUB_WASM" ]] || { echo "✗ build the stub first: (cd tests/hos-status-stub && cargo near build non-reproducible-wasm)" >&2; exit 1; }

WL="${WL:-zavodil2.testnet}"                       # a granted receiver
OUTSIDER="${OUTSIDER:-outsider-nobody.testnet}"    # never in a grant
TOKEN="${TOKEN:-usdc.fakes.testnet}"               # granted fungible
OTHER_TOKEN="${OTHER_TOKEN:-dai.fakes.testnet}"    # never granted
COLL="${COLL:-nft.fakes.testnet}"                  # granted collection
OTHER_COLL="${OTHER_COLL:-other-nft.fakes.testnet}"
# The registry this leased account's own name lives in. Deliberately NOT the
# granted collection: the own-collection guard sits at rung 4 and would answer
# before the item fence at rung 10, so sharing them would hide G7.
OWN_COLL="${OWN_COLL:-own-registry.fakes.testnet}"
FUTURE_NS="4000000000000000000"                    # year 2096
PAST_NS="1000000000000000000"                      # year 2001

SEED_S="hos-stub-$(date +%s)-$$"
read -r WID_S EXEC_S < <(wallet_address "$SEED_S")
[[ -n "$WID_S" ]] || { echo "✗ could not mint the wallet" >&2; exit 1; }
STUB="hos-stub-$(openssl rand -hex 3).$PARENT"
note "wallet $WID_S / executor $EXEC_S / stub $STUB"

cleanup() {
  local rc=$?
  api "$SEED_S" DELETE /wallet/v1/binding >/dev/null 2>&1 || true
  if [[ -z "${KEEP:-}" ]] && account_exists "$STUB"; then
    note "cleaning up $STUB"; delete_account "$STUB"
  fi
  return $rc
}
trap cleanup EXIT

# ── the stub ───────────────────────────────────────────────────────────────
log "Deploying the stub to $STUB"
create_subaccount "$STUB" 3 || { echo "✗ $STUB never appeared" >&2; exit 1; }
near_tty "near contract deploy $STUB use-file $STUB_WASM \
  with-init-call new json-args '{}' prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' \
  network-config $NETWORK sign-with-keychain send" >/dev/null 2>&1 \
  || { echo "✗ the stub did not deploy" >&2; exit 1; }
pass "the stub is deployed and initialised"

# set_status <json> — reconfigure the partner's answer, then outwait the 5 s
# observation cache so the NEXT call is judged against the new state.
set_status() {
  near_tty "near contract call-function as-transaction $STUB set_status \
    json-args '$(jq -nc --arg s "$1" '{status_json:$s}')' prepaid-gas '30.0 Tgas' \
    attached-deposit '0 NEAR' sign-as $STUB network-config $NETWORK sign-with-keychain send" >/dev/null 2>&1 \
    || { warn "set_status did not land"; return 1; }
  sleep 7
}
set_item_info() {
  near_tty "near contract call-function as-transaction $STUB set_item_info \
    json-args '$(jq -nc --arg s "$1" '{item_info_json:$s}')' prepaid-gas '30.0 Tgas' \
    attached-deposit '0 NEAR' sign-as $STUB network-config $NETWORK sign-with-keychain send" >/dev/null 2>&1 \
    || { warn "set_item_info did not land"; return 1; }
  sleep 7
}

# The healthy answer every case starts from, with the grant it varies.
status_json() { # <grant-json-or-null> [state] [frozen] [lease_ns] [reserve] [impl]
  jq -nc --argjson g "$1" --arg st "${2:-Active}" --arg fr "${3:-Unfrozen}" \
     --arg lu "${4:-$FUTURE_NS}" --arg rv "${5:-0}" --argjson iv "${6:-6}" \
     '{extension_enabled:true, grant:$g, state:$st, frozen:$fr,
       lease_until_ns:$lu, reserve_yocto:$rv, impl_version:$iv}'
}
GRANT_OK=$(jq -nc --arg w "$WL" --arg t "$TOKEN" --arg c "$COLL" --arg e "$FUTURE_NS" \
  '{receivers:[$w], budget_yocto:"1000000000000000000000000", spent_yocto:"0",
    tokens:{($t):{budget:"1000", spent:"0"}}, items:{($c):["1"]}, expires_at:$e}')

# `rotation_seq` as a STRING, because that is what the chain sends: the partner
# types it as near-sdk `U64`, which serializes to a decimal string. A stub that
# answers with a JSON number tests a wire form nothing produces — and a coordinator
# that could only read the number form would pass every case here while leaving
# every real leased binding stuck `pending`.
set_item_info "$(jq -nc --arg c "$OWN_COLL" '{rotation_seq:"1", collection_id:$c}')" || true

log "Binding the wallet to the stub as kind=hos_lease, impl_version=6"
api "$SEED_S" PUT /wallet/v1/binding \
  "$(jq -nc --arg a "$STUB" --arg o "$PARENT" '{asset_account_id:$a, owner_account_id:$o, kind:"hos_lease", impl_version:6}')" >/dev/null
assert_status "the leased binding was accepted" 200

set_status "$(status_json "$GRANT_OK")" || true
fund_account "$EXEC_S" 0.25
# Empty of custody rules on purpose: whatever refuses below is the leased
# profile speaking, not the address/limit engine §4 already covers.
store_policy "$SEED_S" "$WID_S" '{"rules":{"addresses":{"mode":"none","list":[]}}}' \
  || { echo "✗ policy not stored" >&2; exit 1; }

ST=""
for _ in 1 2 3 4 5 6 7 8; do
  api "$SEED_S" GET /wallet/v1/binding >/dev/null
  ST=$(jq -r '.binding_status // ""' <<<"$BODY"); [[ "$ST" == "active" ]] && break; sleep 4
done
if [[ "$ST" == "active" ]]; then
  pass "the leased binding is ACTIVE — the coordinator read the partner's view and believed it"
  assert_json "impl_version is echoed" '.impl_version' 6
  assert_json "the decoder version it maps to is stated" '.decoder_version' 1
else
  fail "the leased binding never went active ('$ST'): $(msg_of)"
  verdict "§3.1 hos_lease via stub"; exit 1
fi

send() { log "$1"; call_ext "$SEED_S" "$STUB" "$2" >/dev/null; }

nft_env() { # <collection> <recipient> <token_id> [approval_id]
  local extra=""; [[ -n "${4:-}" ]] && extra=",\"approval_id\":$4"
  local args; args=$(printf '{"receiver_id":"%s","token_id":"%s"%s}' "$2" "$3" "$extra")
  jq -nc --arg c "$1" --arg a "$(printf '%s' "$args" | base64 | tr -d '\n')" \
    '{request:{external:[{receiver_id:$c, actions:[{action:"function_call",payload:{function_name:"nft_transfer",args:$a,deposit:"1",gas:"30000000000000"}}]}]}}'
}
ft_env() { # <token> <recipient> <amount> [deposit] [extra-arg-json]
  local args; args=$(printf '{"receiver_id":"%s","amount":"%s"%s}' "$2" "$3" "${5:-}")
  jq -nc --arg t "$1" --arg a "$(printf '%s' "$args" | base64 | tr -d '\n')" --arg d "${4:-1}" \
    '{request:{external:[{receiver_id:$t, actions:[{action:"function_call",payload:{function_name:"ft_transfer",args:$a,deposit:$d,gas:"30000000000000"}}]}]}}'
}

# ── G0 the door: a granted spend is NOT refused ────────────────────────────
#
# The stub has no `w_execute_extension`, so the transaction fails on chain
# afterwards. That is expected and is not what is being measured: the question
# here is whether OUR pre-flight lets a legal request through, and a refusal
# would arrive as a 403 before anything was signed.
send "G0 control — a plain transfer to a granted receiver, inside every budget" "$(ext_transfer "$WL" "1000000000000000000000")"
if [[ "$HTTP" == "403" ]]; then
  fail "G0 a fully granted spend was refused before signing: class '$(class_of)' — every refusal below is now unjudgeable"
else
  pass "G0 the pre-flight let a granted spend through (HTTP $HTTP) — the refusals below are refusals of something"
fi

# HERE, not at the end of the file, and that is not cosmetic: the cases below
# walk the binding through faults that END it — an expired lease, an ownership
# rotation — after which no call succeeds and a probe needing a HEALTHY lane
# cannot tell its own failure from the fixture's. Written at the tail first,
# this ran after the rotation and reported the lane shut when the lane had been
# shut on purpose two cases earlier.
# ── W. the partner's lifecycle webhook, and what it can honestly prove ─────
#
# §6 of the plan asks for one thing above all: their revoke stops the agent NOW,
# not when our 5 s observation cache happens to expire.
#
# That exact claim CANNOT be driven from outside, and the reason is worth
# writing down rather than rediscovering. The cache is written and read in ONE
# place — `preflight_extension_call` — so the only way to warm it is a call that
# also broadcasts a transaction. The write happens at the start of that request
# and the answer comes back seconds later, so by the time a test can send the
# NEXT call, most of the 5 s window is already spent. Measured here: the cached
# ALLOW was gone before the follow-up call could even start. A probe built on
# that race would pass or fail on network latency, and a green run would say
# nothing about the webhook.
#
# What IS judgeable, and what this probe pins:
#   W1  the endpoint re-reads the CHAIN rather than believing its caller: the
#       body says `frozen`, and the answer must carry the status the chain
#       actually reports, not the word in the request
#   W2  after the event, the lane is refused with the fault the chain reports
#
# Neither proves the invalidation beat the TTL. Both would fail if the webhook
# were a no-op that returned a canned answer, which is the failure this section
# exists to catch.
log "W the partner's lifecycle webhook re-reads the chain"
WH_SECRET="${BINDING_WEBHOOK_SECRET:-}"
if [[ -z "$WH_SECRET" ]]; then
  skip "W — set BINDING_WEBHOOK_SECRET (it lives in prod_configs/coordinator/.env.testnet) to judge the webhook"
elif ! set_status "$(status_json "$GRANT_OK" Active SelfFrozen)"; then
  skip "W — the stub would not report a frozen account, so there is nothing for the event to find"
else
  WH=$(curl -sS -m 30 -X POST "$COORDINATOR_URL/wallet/v1/binding/events" \
    -H "X-Binding-Webhook-Secret: $WH_SECRET" -H 'Content-Type: application/json' \
    -d "$(jq -nc --arg a "$STUB" '{asset_account_id:$a, event:"frozen"}')" 2>/dev/null)
  WH_STATUS=$(jq -r '.binding_status // ""' <<<"$WH")
  note "W the event answered: $WH"
  # `frozen` is reversible, so the binding is suspended rather than revoked —
  # and either way it must NOT still be `active`, which is what a webhook that
  # answered without looking would say.
  if [[ -n "$WH_STATUS" && "$WH_STATUS" != "active" ]]; then
    pass "W1 the event re-read the chain and reported '$WH_STATUS' — the body was a hint, the chain was the answer"
  else
    fail "W1 the event answered '$WH_STATUS' for an account the chain reports as frozen: either it believed the request or it did not look"
  fi

  send "W2 a spend right after the event" "$(ext_transfer "$WL" "1000000000000000000000")"
  assert_class "W2 the lane is refused with the fault the chain reports" "account_frozen"

  # RESTORE. Everything below reads the healthy grant this suite set up, and a
  # probe that leaves the stub frozen turns twenty-two later cases into
  # `account_frozen` — which is how this one was first written.
  set_status "$(status_json "$GRANT_OK")" \
    || fail "W the stub could not be returned to a healthy state — every case below is now judging a frozen account"
fi

# ── the grant ladder, in the contract's own order ──────────────────────────
send "G1 a receiver the grant never named (plain transfer)" "$(ext_transfer "$OUTSIDER" "1000000000000000000000")"
assert_class "G1" "receiver_not_granted"
assert_json "G1 names the promise it is about" '.promise_index' 0

send "G2 a receiver the grant never named, reached through a token call" "$(ft_env "$TOKEN" "$OUTSIDER" 5)"
assert_class "G2 (rung 8: only known once the arguments parsed)" "receiver_not_granted"

send "G3 a token the grant never named" "$(ft_env "$OTHER_TOKEN" "$WL" 5)"
assert_class "G3" "token_not_granted"

send "G4 a granted token, over its own budget" "$(ft_env "$TOKEN" "$WL" 5000)"
assert_class "G4" "token_budget_exceeded"

send "G5 native spending past the granted cap" "$(ext_transfer "$WL" "2000000000000000000000000")"
assert_class "G5" "grant_exhausted"
assert_json "G5 names the promise that breached it" '.promise_index' 0

send "G6 a collection the grant never named" "$(nft_env "$OTHER_COLL" "$WL" 1)"
assert_class "G6 — NOT item_not_granted: adding a token_id to a collection nobody granted is the fix that cannot work" "collection_not_granted"

send "G7 a granted collection, an item outside the fence" "$(nft_env "$COLL" "$WL" 9)"
assert_class "G7" "item_not_granted"

send "G8 the account's OWN collection" "$(nft_env "$OWN_COLL" "$WL" 1)"
assert_class "G8 — a grant can never move this account's own names" "own_collection_refused"

# ── the call FORM ──────────────────────────────────────────────────────────
send "F1 a granted spend that redirects refunds" "$(ext_transfer "$WL" "1000000000000000000000" "$OUTSIDER")"
assert_class "F1" "grant_shape_violation:refund_target_not_allowed"

MIXED=$(jq -nc --arg t "$TOKEN" --arg w "$WL" \
  --arg a "$(printf '{"receiver_id":"%s","amount":"5"}' "$WL" | base64 | tr -d '\n')" \
  '{request:{external:[{receiver_id:$t, actions:[
      {action:"function_call",payload:{function_name:"ft_transfer",args:$a,deposit:"1",gas:"30000000000000"}},
      {action:"transfer",payload:{amount:"1"}}]}]}}')
send "F2 a token call sharing its promise with another action" "$MIXED"
assert_class "F2" "grant_shape_violation:grant_call_must_stand_alone"

send "F3 a token call attaching more than the mandated yocto" "$(ft_env "$TOKEN" "$WL" 5 2)"
assert_class "F3" "grant_shape_violation:grant_call_deposit"

send "F4 an nft_transfer spending an approval" "$(nft_env "$OTHER_COLL" "$WL" 1 7)"
assert_class "F4 — approval_id (rung 7) answers before the collection lookup (rung 9)" "grant_shape_violation:grant_approval_not_allowed"

send "F5 a token call carrying an argument the contract cannot parse" "$(ft_env "$TOKEN" "$WL" 5 1 ',"note":"x"')"
assert_class "F5" "grant_shape_violation:grant_args_unreadable"

SD=$(jq -nc --arg t "$TOKEN" --arg a "$(printf '{"account_id":"%s"}' "$WL" | base64 | tr -d '\n')" \
  '{request:{external:[{receiver_id:$t, actions:[{action:"function_call",payload:{function_name:"storage_deposit",args:$a,deposit:"1",gas:"30000000000000"}}]}]}}')
send "F6 storage_deposit under a grant" "$SD"
assert_class "F6 — a grant covers ft_transfer and nft_transfer only" "grant_shape_violation:grant_method_not_allowed"

SI=$(jq -nc --arg w "$WL" \
  '{request:{external:[{receiver_id:$w, actions:[{action:"deterministic_state_init",payload:{code:"AA==",deposit:"1"}}]}]}}')
send "F7 deploying code under a grant" "$SI"
if [[ "$(class_of)" == "grant_shape_violation:grant_action_not_allowed" ]]; then
  pass "F7 — a grant never deploys code (class $(class_of))"
else
  assert_denied "F7 deploying code under a grant is refused" && note "  class: $(class_of), $(msg_of | head -c 140)"
fi

# ── multi-fault: the FIRST class is the one the contract would panic on ────
BOTH=$(jq -nc --arg o "$OUTSIDER" \
  '{request:{external:[{receiver_id:$o, actions:[{action:"transfer",payload:{amount:"9000000000000000000000000"}}]}]}}')
send "M1 an ungranted receiver AND a spend past the budget, in one promise" "$BOTH"
assert_class "M1 the receiver (rung 2) answers before the budget (rung 4)" "receiver_not_granted"
assert_json "M1 says how many further violations the request carries" '.additional_violations' 1

TWO=$(jq -nc --arg w "$WL" --arg o "$OUTSIDER" \
  '{request:{external:[
     {receiver_id:$w, actions:[{action:"transfer",payload:{amount:"1000000000000000000000"}}]},
     {receiver_id:$o, actions:[{action:"transfer",payload:{amount:"1000000000000000000000"}}]}]}}')
send "M2 a legal promise 0 and an illegal promise 1" "$TWO"
assert_class "M2" "receiver_not_granted"
assert_json "M2 blames promise 1, not the request as a whole" '.promise_index' 1

# ── legal shapes that must NOT be refused ─────────────────────────────────
MULTI=$(jq -nc --arg w "$WL" \
  '{request:{external:[{receiver_id:$w, actions:[
      {action:"transfer",payload:{amount:"1000000000000000000000"}},
      {action:"transfer",payload:{amount:"1000000000000000000000"}}]}]}}')
send "L1 several transfers in ONE promise — the contract accepts these" "$MULTI"
[[ "$HTTP" == "403" ]] \
  && fail "L1 refused a request the chain would have executed: class '$(class_of)' — $(msg_of | head -c 140)" \
  || pass "L1 not refused (HTTP $HTTP) — only a promise carrying a CALL must stand alone"

send "L2 a memo alongside the standard ft_transfer arguments" "$(ft_env "$TOKEN" "$WL" 5 1 ',"memo":"invoice 7"')"
[[ "$HTTP" == "403" ]] \
  && fail "L2 refused a legal memo: class '$(class_of)' — $(msg_of | head -c 140)" \
  || pass "L2 not refused (HTTP $HTTP) — memo is part of the standard"

# ── the door rules answer ALONE, before any promise ───────────────────────
if set_status "$(status_json null)"; then
  send "D1 no grant at all" "$(ext_transfer "$WL" "1000000000000000000000")"
  assert_class "D1" "grant_missing"
fi

GRANT_EXPIRED=$(jq -nc --arg w "$WL" --arg e "$PAST_NS" \
  '{receivers:[$w], budget_yocto:"1000000000000000000000000", spent_yocto:"0", tokens:{}, items:{}, expires_at:$e}')
if set_status "$(status_json "$GRANT_EXPIRED")"; then
  send "D2 an expired grant, with a request that ALSO breaks the form" "$(ext_transfer "$OUTSIDER" "1" "$OUTSIDER")"
  assert_class "D2 the expiry answers alone — the owner is not sent to fix the form of a request no grant covers" "grant_expired"
  assert_json "D2 no promise is blamed for a door rule" '.promise_index' ""
fi

GRANT_SPENT=$(jq -nc --arg w "$WL" --arg e "$FUTURE_NS" \
  '{receivers:[$w], budget_yocto:"1000", spent_yocto:"1000", tokens:{}, items:{}, expires_at:$e}')
if set_status "$(status_json "$GRANT_SPENT")"; then
  send "D3 a grant whose budget is already spent" "$(ext_transfer "$WL" "1000000000000000000000")"
  assert_class "D3" "grant_exhausted"
fi

# ── the reserve floor ─────────────────────────────────────────────────────
if set_status "$(status_json "$GRANT_OK" Active Unfrozen "$FUTURE_NS" "1000000000000000000000000000")"; then
  send "R1 a spend that would leave the account below its reserve floor" "$(ext_transfer "$WL" "1000000000000000000000")"
  assert_class "R1 — the floor tracks live storage and only the chain knows it" "insufficient_vs_reserve"
fi

# ── lifecycle faults (§6, leased half) ────────────────────────────────────
for spec in \
  "frozen|SelfFrozen|account_frozen|a frozen account" \
  "state|Parked|account_not_active|a parked account" \
  "state|Suspended|account_not_active|a suspended account"
do
  field=${spec%%|*}; rest=${spec#*|}; val=${rest%%|*}; rest=${rest#*|}; cls=${rest%%|*}; desc=${rest#*|}
  if [[ "$field" == "frozen" ]]; then S=$(status_json "$GRANT_OK" Active "$val"); else S=$(status_json "$GRANT_OK" "$val"); fi
  if set_status "$S"; then
    send "C-$val $desc" "$(ext_transfer "$WL" "1000000000000000000000")"
    assert_class "C-$val" "$cls"
    [[ "$(jq -r '.terminal' <<<"$BODY")" == "false" ]] \
      && pass "C-$val is marked reversible — the owner can lift it" \
      || note "  terminal=$(jq -r '.terminal' <<<"$BODY")"
  fi
done

if set_status "$(status_json "$GRANT_OK" Active Unfrozen "$PAST_NS")"; then
  send "C-lease a lease that has run out" "$(ext_transfer "$WL" "1000000000000000000000")"
  assert_class "C-lease" "lease_expired"
  assert_json "C-lease is terminal — the lane is over" '.terminal' true
fi

if set_status "$(status_json "$GRANT_OK" Active Unfrozen "$FUTURE_NS" 0 5)"; then
  send "C-version an implementation this build has no decoder for" "$(ext_transfer "$WL" "1000000000000000000000")"
  assert_class "C-version (K8/R9)" "unsupported_wallet_implementation"
fi

if set_status '{"extension_enabled":true,"grant":null,"state":"Active","frozen":"Unfrozen","lease_until_ns":"not a number","reserve_yocto":"0","impl_version":6}'; then
  send "C-malformed a lease_until_ns that is not a number" "$(ext_transfer "$WL" "1000000000000000000000")"
  assert_class "C-malformed — schema drift shows up as itself, not as a fake 'lease expired'" "chain_status_unreadable"
fi

if set_status "$(jq -nc --argjson g "$GRANT_OK" '{extension_enabled:false, grant:$g, state:"Active", frozen:"Unfrozen", lease_until_ns:"4000000000000000000", reserve_yocto:"0", impl_version:6}')"; then
  send "C-disabled the executor is no longer in the control set" "$(ext_transfer "$WL" "1000000000000000000000")"
  assert_class "C-disabled" "executor_not_in_control_set"
fi

if set_status '{}'; then
  send "C-empty a view that answers with nothing at all" "$(ext_transfer "$WL" "1000000000000000000000")"
  assert_denied "C-empty every default is the value that FAILS verification" "agent_connect_denied"
  note "  class: $(class_of)"
fi

# ── ownership rotation ends the lane ──────────────────────────────────────
if set_status "$(status_json "$GRANT_OK")"; then
  api "$SEED_S" GET /wallet/v1/binding >/dev/null
  note "status before rotation: $(jq -r '.binding_status' <<<"$BODY")"
  # The NUMBER spelling here on purpose. The rotation must be judged by VALUE,
  # not by the JSON type it arrived as: 1 (string) → 2 (number) is a rotation and
  # nothing else, and a reader that compared the raw forms would call it one when
  # the value had not moved, or miss it when it had.
  if set_item_info "$(jq -nc --arg c "$OWN_COLL" '{rotation_seq:2, collection_id:$c}')"; then
    api "$SEED_S" GET /wallet/v1/binding >/dev/null
    ST2=$(jq -r '.binding_status // ""' <<<"$BODY")
    if [[ "$ST2" == "revoked" || "$HTTP" == "404" ]]; then
      pass "ROT the account changed owners and the binding ended — the previous owner's authorization does not carry"
    else
      fail "ROT rotation_seq moved 1 → 2 and the binding is still '$ST2' (HTTP $HTTP)"
    fi
    send "ROT' a spend after the rotation" "$(ext_transfer "$WL" "1000000000000000000000")"
    assert_denied "ROT' refused after the rotation"
  fi
fi

verdict "§3.1 hos_lease via stub"
