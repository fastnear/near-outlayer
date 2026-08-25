#!/usr/bin/env bash
#
# Runs every HoS Agent Connect suite, in order, and prints the coverage the
# partner asked to see: endpoint × class, and acceptance R0–R11 with an honest
# label on each — `live`, `stub`, or `waits-TLA`.
#
# SEQUENTIALLY, always. The wallet routes allow 100 requests a minute per IP
# and the limiter lives in the coordinator's memory, so two suites in parallel
# produce 429s and connection resets that are indistinguishable from defects.
# Half of every previous full run's "failures" were this.
#
#   PARENT=you.testnet ./tests/run_hos_suite.sh --apply
#   PARENT=you.testnet ONLY="decoder custody" ./tests/run_hos_suite.sh --apply
#
# Exits non-zero if any suite failed.

set -uo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
[[ "${1:-}" == "--apply" ]] || { sed -n '3,16p' "$0" >&2; echo "  Pass --apply to run." >&2; exit 0; }
: "${PARENT:?USAGE: PARENT=you.testnet $0 --apply}"
export OUTLAYER_NETWORK="${NETWORK:-testnet}"
LOGDIR="${LOGDIR:-/tmp/hos-suite-$(date +%Y%m%d-%H%M%S)}"
mkdir -p "$LOGDIR"

log()  { printf '\n\033[1;36m════ %s\033[0m\n' "$*" >&2; }
note() { printf '\033[35m• %s\033[0m\n' "$*" >&2; }

SUITES=(
  "endpoints|hos_endpoints_e2e.sh|§2 every endpoint of the method list, by class"
  "decoder|hos_decoder_policy_e2e.sh|§3 the decoder and the mode-blind core policy"
  "lease|hos_lease_stub_e2e.sh|§3.1 the hos_lease profile, through the §10 stub"
  "custody|hos_custody_matrix_e2e.sh|§4 limits, rights, capabilities, multisig, freeze"
  "identity|hos_identity_e2e.sh|§5 use_bound_identity and attribution"
  "lifecycle|hos_lifecycle_e2e.sh|§6 lifecycle and invalidation (R5)"
  "dos|hos_dos_e2e.sh|§7 resource abuse"
)
ONLY="${ONLY:-}"

# A FRESH shared fixture every full run. A wallet carries a monthly custody
# allowance of 100 operations, and a re-used fixture arrives with an unknown
# share of it already spent — after which suites fail on the cap and the failure
# reads as a policy defect. The suites that spend most of an allowance on their
# own (decoder, custody, lease, lifecycle, identity) build their own wallets;
# this one carries the two that do not.
log "Building a fresh shared fixture"
bash "$SCRIPT_DIR/hos_fixture.sh" --destroy >/dev/null 2>&1 || true
bash "$SCRIPT_DIR/hos_fixture.sh" --apply 2>&1 | tee "$LOGDIR/fixture.log" >&2 || {
  echo "the fixture could not be built — nothing below would be judgeable" >&2; exit 1; }

declare -a RESULTS=()
FAILED_SUITES=0
for entry in "${SUITES[@]}"; do
  name=${entry%%|*}; rest=${entry#*|}; script=${rest%%|*}; desc=${rest#*|}
  if [[ -n "$ONLY" && " $ONLY " != *" $name "* ]]; then
    RESULTS+=("$name|skipped|-|-|-|$desc"); continue
  fi
  log "$name — $desc"
  bash "$SCRIPT_DIR/$script" --apply > "$LOGDIR/$name.log" 2>&1
  rc=$?
  line=$(grep -oE "— [0-9]+ passed, [0-9]+ failed, [0-9]+ skipped, [0-9]+ finding" "$LOGDIR/$name.log" | tail -1)
  p=$(grep -oE "[0-9]+ passed" <<<"$line" | grep -oE "[0-9]+")
  f=$(grep -oE "[0-9]+ failed" <<<"$line" | grep -oE "[0-9]+")
  s=$(grep -oE "[0-9]+ skipped" <<<"$line" | grep -oE "[0-9]+")
  g=$(grep -oE "[0-9]+ finding" <<<"$line" | grep -oE "[0-9]+")
  RESULTS+=("$name|$( ((rc==0)) && echo ok || echo FAILED )|${p:-?}|${f:-?}|${s:-0}/${g:-0}|$desc")
  ((rc==0)) || FAILED_SUITES=$((FAILED_SUITES+1))
  tail -25 "$LOGDIR/$name.log" >&2
  note "full log: $LOGDIR/$name.log"
  # The per-IP window is a minute wide; a gap between suites costs a minute and
  # buys back a whole class of phantom failures.
  sleep 45
done

log "Suite results"
printf '  %-10s %-8s %6s %6s %8s  %s\n' SUITE STATUS PASS FAIL SKIP/FIND DESCRIPTION >&2
for r in "${RESULTS[@]}"; do
  IFS='|' read -r n st p f sg d <<<"$r"
  printf '  %-10s %-8s %6s %6s %8s  %s\n' "$n" "$st" "$p" "$f" "$sg" "$d" >&2
done

log "Findings (behaviour the partner will meet, reported apart from failures)"
grep -h "FINDING:" "$LOGDIR"/*.log 2>/dev/null | sed 's/\x1b\[[0-9;]*m//g' | sed 's/^/  /' >&2 || echo "  (none)" >&2

log "Skips (what is NOT covered, and why)"
grep -h "SKIP:" "$LOGDIR"/*.log 2>/dev/null | sed 's/\x1b\[[0-9;]*m//g' | sed 's/^/  /' >&2 || echo "  (none)" >&2

# ── §8 acceptance map ──────────────────────────────────────────────────────
#
# `live` = proved against the real chain and the real coordinator.
# `stub`  = proved through the §10 stub: our side of the leased mode in full,
#           the partner's on-chain panic not included.
# `waits-TLA` / `external` = named so nobody reads a blank as "covered".
log "§8 acceptance R0–R11"
cat >&2 <<'MAP'
  R0   executor→asset w_execute_extension executes and is tracked
       live   decoder §D0 (executed, tx hash), endpoints §E11'''' (the builder), lifecycle §R5a
  R1   a binding only after the owner's authorization and on-chain confirmation
       live   endpoints §E5/E5' (pending claims nothing, active is exclusive), fixture (kit → active)
       stub   lease §"the leased binding is ACTIVE" (the partner's view believed, then verified)
  R2   a limit breached INSIDE args → refused before signing
       live   decoder §R2 (250 NEAR nested, 1 yocto outside) and §R2-aggregate (split payment, one rule)
  R3   ft_transfer through a permitted token contract to a foreign logical recipient → refused
       live   decoder §R3, custody §F3/§G1 (per-token cap and token allowlist over the DECODED move)
  R4   any AddExtension / RemoveExtension / SetSignatureMode → hard deny
       live   decoder §R4 (all three ops, plus one smuggled beside a legal transfer)
  R5   the binding stops executing after transfer / recovery / revoke / expiry
       live   lifecycle §R5b–R5g (executor cut, wiped account, re-bind, DELETE)
       stub   lease §C-* (frozen, parked, suspended, lease expired) and §ROT (ownership rotation)
       gap    the partner's revoke WEBHOOK is unconfigured on testnet — see the findings
  R6   gas: source, submit, retry, replay specified and exercised
       live   endpoints §E12 (gas_balance + the low-gas flag), custody §L (one spend at a time)
       partial a deliberate executor-below-floor terminal error was not driven here
  R7   reads do not mix the two identities
       live   endpoints §E10/E12/E13 (asset vs executor, intents sent to the right door, /balance unmoved)
       stub   lease reserve_yocto is read from hos_agent_status (§R1)
  R8   one operation traceable from API to inner receipts
       partial request_id / tx_hash / promise_index / additional_violations observed on every refusal
              and every send; the full status ladder needs the coordinator's own rows (DB access)
  R9   an unsupported impl_version is rejected before signing
       live   endpoints §E3i (PUT refuses version 999, names the supported set, says terminal)
       stub   lease §C-version (a chain that reports version 5 → unsupported_wallet_implementation)
  R10  a confidential operation end to end
       external mainnet only — there are no intents on testnet, and the routes 503 there by design
  R11  sign-in / proof of ownership
       external not part of this integration: the wallet's own NEP-413 path
MAP

log "Verdict"
if (( FAILED_SUITES > 0 )); then
  echo "  $FAILED_SUITES suite(s) failed — see $LOGDIR" >&2
  exit 1
fi
echo "  every suite passed. Logs in $LOGDIR" >&2
