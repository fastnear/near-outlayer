#!/usr/bin/env bash
#
# Who may call what, with which credential — the whole matrix, on one stack.
#
# There is ONE paying credential now: a payment key, presented as
# `X-Payment-Key`. `wk_` authenticates a wallet to `/wallet/v1/*` and names it;
# it does not pay. Agent payment keys — keyless keys resolved from a `wk_` —
# were removed on 2026-08-21 because nothing was left that needed them.
#
# That leaves three ways a call can arrive and two kinds of project, and this
# runs all six. The trial is the interesting row: it is a real payment key with
# a scope, so it reaches a connector and is REFUSED on a plain project — free
# execution exists for trying connectors, not for running arbitrary code.
#
#             │ connector project      │ ordinary WASI project
#   ──────────┼────────────────────────┼───────────────────────────
#   no key    │ 401 missing_payment_key│ 401 missing_payment_key
#   trial key │ runs                   │ 403 project_not_allowed
#   paid key  │ runs                   │ runs
#
# What each probe pins:
#   K1  no credential is refused on BOTH kinds of project, with the same answer.
#       A route that answered differently would tell a stranger which projects
#       exist
#   K2  a `wk_` is not a payment credential. It authenticates fine against
#       `/wallet/v1/*` in the same run, so this is not "the key is broken" — it
#       is the second interface being gone, and the test says which
#   K3  a trial key reaches a connector
#   K4  the same trial key is refused on an ordinary project, by SCOPE and not
#       by balance — `project_not_allowed`, not `insufficient_balance`
#   K5  a funded payment key reaches a connector
#   K6  and the same key reaches an ordinary project
#
# K2 is the one that would rot silently: restoring `wk_` as a payer is a small,
# well-meaning change, and every other probe here would still pass.
#
# Money: claims this account's one trial (a real claim — it cannot be undone or
# repeated) and creates one funded payment key. Connector calls cost their
# operation price; `ping` is priced at zero, which is what this uses.
#
# Requires: $PARENT with a keychain credential, `outlayer` logged in as $PARENT,
# and the wallet's `wk_` — the script claims and creates through it.
#
# Run (spends real testnet NEAR and one trial claim):
#   PARENT=you.testnet ./tests/call_credentials_e2e.sh --apply

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APPLY=false
[[ "${1:-}" == "--apply" ]] && APPLY=true

NETWORK="${NETWORK:-testnet}"
COORDINATOR_URL="${COORDINATOR_URL:-https://testnet-api.outlayer.ai}"
CONNECTOR="${CONNECTOR:-connectors.outlayer.testnet/connector-probe}"
# An ordinary project: outside the connector namespace, so a trial's scope does
# not reach it. Which module it is does not matter — the refusal happens before
# anything runs.
ORDINARY="${ORDINARY:-zavodil.testnet/wallet-probe}"
PARENT="${PARENT:-}"
DEPOSIT_USDC="${DEPOSIT_USDC:-0.30}"

PASS=0; FAILED=0; FAILED_NAMES=()
log()  { printf '\n\033[36m▶ %s\033[0m\n' "$*" >&2; }
note() { printf '\033[35m• %s\033[0m\n' "$*" >&2; }
warn() { printf '\033[33m⚠ %s\033[0m\n' "$*" >&2; }
pass() { printf '\033[32m✓ %s\033[0m\n' "$*" >&2; PASS=$((PASS+1)); }
fail() { printf '\033[31m✗ %s\033[0m\n' "$*" >&2; FAILED=$((FAILED+1)); FAILED_NAMES+=("$*"); }

if [[ "$APPLY" != true ]]; then
  sed -n '3,45p' "$0" >&2
  echo "  Pass --apply to run." >&2
  exit 0
fi

[[ -n "$PARENT" ]] || { echo "USAGE: PARENT=you.testnet $0 --apply" >&2; exit 1; }
for tool in jq curl outlayer cargo; do
  command -v "$tool" >/dev/null || { echo "✗ missing $tool" >&2; exit 1; }
done
CREDS_DIR="$HOME/.near-credentials/$NETWORK"
[[ -f "$CREDS_DIR/$PARENT.json" ]] || { echo "✗ creds missing: $CREDS_DIR/$PARENT.json" >&2; exit 1; }
# Without this, `outlayer` follows ~/.outlayer/default-network — mainnet on this
# machine — and the check below compares against the wrong network's account.
export OUTLAYER_NETWORK="$NETWORK"
WHOAMI=$(outlayer whoami 2>/dev/null | awk -F': *' '/^Account:/{print $2; exit}')
[[ "$WHOAMI" == "$PARENT" ]] || { echo "✗ outlayer logged in as '$WHOAMI', not '$PARENT'" >&2; exit 1; }
PARENT_PRIVKEY=$(jq -r '.private_key' "$CREDS_DIR/$PARENT.json")

RECOVERY_BIN="$SCRIPT_DIR/../scripts/customer-recovery/target/release/customer-recovery"
log "Building customer-recovery (sign-bearer-near)"
(cd "$SCRIPT_DIR/../scripts/customer-recovery" && cargo build --release --quiet) \
  || { echo "✗ customer-recovery build failed" >&2; exit 1; }

SEED="callcred-$(date +%s)-$$"
WK="Authorization: Bearer near:$("$RECOVERY_BIN" sign-bearer-near \
      --private-key "$PARENT_PRIVKEY" --account-id "$PARENT" --seed "$SEED")"

# call <project> [auth-header] — echoes "<http> <body>". An empty (or absent)
# header means "send none", which is K1.
#
# Two branches rather than an array of flags: bash 3.2 is what `bash` is on
# macOS, and expanding an EMPTY array under `set -u` there aborts the script
# with `args[@]: unbound variable`. The one probe that passes no header is K1 —
# so the credential-less case would take the whole run down before its first
# request, and a syntax check would not see it, because the line is valid.
call() {
  local project=$1 header=${2:-} out http
  out=$(mktemp -t callcred.XXXXXX)
  if [[ -n "$header" ]]; then
    http=$(curl -sS -m 300 -o "$out" -w '%{http_code}' -X POST "$COORDINATOR_URL/call/$project" \
      -H "$header" -H 'Content-Type: application/json' -d '{"input":{"operation":"ping"}}')
  else
    http=$(curl -sS -m 300 -o "$out" -w '%{http_code}' -X POST "$COORDINATOR_URL/call/$project" \
      -H 'Content-Type: application/json' -d '{"input":{"operation":"ping"}}')
  fi
  printf '%s %s\n' "$http" "$(tr -d '\n' < "$out")"
  rm -f "$out"
}
code_of() { jq -r '.error // empty' <<<"$1" 2>/dev/null; }

# ── K1: no credential, on both kinds of project ──────────────────────────────
log "K1 no credential at all"
for project in "$CONNECTOR" "$ORDINARY"; do
  R=$(call "$project" ""); HTTP=${R%% *}; BODY=${R#* }
  if [[ "$HTTP" == "401" ]]; then
    pass "K1 $project refused without a credential (401)"
  else
    fail "K1 $project answered $HTTP without a credential — a call must be paid for before it runs"
  fi
done

# ── K2: a wk_ is not a payment credential ────────────────────────────────────
#
# Proven in two halves, and the second is what makes it a test rather than an
# observation: the same `wk_` is shown to authenticate a wallet route in the
# same run. Without that, a broken token would produce this result too.
log "K2 a wk_ authenticates a wallet but pays for nothing"
R=$(curl -sS -m 30 -o /dev/null -w '%{http_code}' "$COORDINATOR_URL/wallet/v1/address?chain=near" -H "$WK")
if [[ "$R" == "200" ]]; then
  pass "K2 the wk_ is live: /wallet/v1/address answers 200"
else
  fail "K2 the wk_ does not authenticate at all ($R) — the rest of K2 would prove nothing"
fi

R=$(call "$CONNECTOR" "$WK"); HTTP=${R%% *}; BODY=${R#* }
if [[ "$HTTP" == "401" ]]; then
  pass "K2 and it buys nothing: the connector call is refused (401)"
else
  fail "K2 a wk_ paid for a call ($HTTP) — the removed second credential is back"
fi

# ── the two keys ─────────────────────────────────────────────────────────────
log "Claiming the trial and creating a funded key"
R=$(curl -sS -m 60 -X POST "$COORDINATOR_URL/trial-key" -H "$WK")
TRIAL=$(jq -r '.payment_key // empty' <<<"$R")
TRIAL_SCOPE=$(jq -r '(.project_ids // []) | join(",")' <<<"$R")
if [[ -n "$TRIAL" ]]; then
  note "trial claimed, scope: ${TRIAL_SCOPE:-<none>}"
else
  warn "trial not claimed: $(jq -r '.error // .message // .' <<<"$R" | head -c 160)"
fi

R=$(curl -sS -m 90 -X POST "$COORDINATOR_URL/wallet/v1/create-payment-key" \
      -H "$WK" -H 'Content-Type: application/json' \
      -d "$(jq -nc --arg d "$DEPOSIT_USDC" '{initial_deposit_usdc:$d}')")
PAID=$(jq -r '.payment_key // empty' <<<"$R")
if [[ -n "$PAID" ]]; then
  note "payment key created, nonce $(jq -r '.nonce' <<<"$R")"
else
  warn "payment key not created: $(jq -r '.error // .message // .' <<<"$R" | head -c 160)"
fi

# ── K3 / K4: the trial reaches connectors and nothing else ───────────────────
if [[ -z "$TRIAL" ]]; then
  note "K3/K4 SKIPPED — no trial key (already claimed on this account?)"
else
  log "K3 trial key on a connector"
  R=$(call "$CONNECTOR" "X-Payment-Key: $TRIAL"); HTTP=${R%% *}; BODY=${R#* }
  if [[ "$HTTP" == 2?? ]]; then
    pass "K3 the trial ran a connector call"
  else
    fail "K3 the trial was refused on a connector ($HTTP $(code_of "$BODY")): $(jq -r '.message // empty' <<<"$BODY" | head -c 140)"
  fi

  log "K4 the same trial key on an ordinary project"
  R=$(call "$ORDINARY" "X-Payment-Key: $TRIAL"); HTTP=${R%% *}; BODY=${R#* }
  CODE=$(code_of "$BODY")
  # The REASON matters as much as the refusal: a scope refusal says "this key
  # was never for that", while a balance refusal would say "top it up" and send
  # the caller down a road that does not exist.
  if [[ "$CODE" == "project_not_allowed" ]]; then
    pass "K4 refused by SCOPE — a trial buys connectors, not arbitrary code"
  elif [[ "$HTTP" == 2?? ]]; then
    fail "K4 the trial ran an ordinary project — free execution is not supposed to exist"
  else
    fail "K4 refused as '$CODE' rather than project_not_allowed: $(jq -r '.message // empty' <<<"$BODY" | head -c 140)"
  fi
fi

# ── K5 / K6: a funded key reaches both ───────────────────────────────────────
if [[ -z "$PAID" ]]; then
  note "K5/K6 SKIPPED — no payment key was created"
else
  log "K5 funded key on a connector"
  R=$(call "$CONNECTOR" "X-Payment-Key: $PAID"); HTTP=${R%% *}; BODY=${R#* }
  [[ "$HTTP" == 2?? ]] \
    && pass "K5 the funded key ran a connector call" \
    || fail "K5 refused ($HTTP $(code_of "$BODY")): $(jq -r '.message // empty' <<<"$BODY" | head -c 140)"

  log "K6 funded key on an ordinary project"
  R=$(call "$ORDINARY" "X-Payment-Key: $PAID"); HTTP=${R%% *}; BODY=${R#* }
  CODE=$(code_of "$BODY")
  # `wallet-probe` refuses a run with no wallet attached, which is its own
  # contract and not a credential problem — the credential got that far.
  if [[ "$HTTP" == 2?? ]]; then
    pass "K6 the funded key ran an ordinary project"
  elif grep -qi "wallet is not available" <<<"$BODY"; then
    pass "K6 the credential was accepted; the module refused for its own reason (no wallet attached)"
  else
    fail "K6 refused as '$CODE': $(jq -r '.message // empty' <<<"$BODY" | head -c 160)"
  fi
fi

# ── verdict ──────────────────────────────────────────────────────────────────
log "call credentials — $PASS passed, $FAILED failed"
if (( FAILED > 0 )); then
  for n in "${FAILED_NAMES[@]}"; do printf '  \033[31m✗ %s\033[0m\n' "$n" >&2; done
  exit 1
fi
exit 0
