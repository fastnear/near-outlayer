#!/usr/bin/env bash
#
# Put the connector prices on chain, on TESTNET.
#
# These are the numbers `outlayer-coordinator/docs/TESTNET_RUNBOOK.md` checks
# against, and this script is their only home outside the chain: the coordinator
# keeps no price table of its own, so what is written here is what gets charged.
#
# Until this has run, every connector call is REFUSED — an unpriced project is
# not a free one. That is the safe direction, and it is also why this is not
# optional after a fresh deploy.
#
# Prerequisites, in order:
#   1. the contract is deployed and `migrate()` has run — `get_storage_version`
#      must answer "8", because `project_pricing` is a v8 field;
#   2. the project exists. `set_project_pricing` refuses a project nobody
#      registered: a price on an unregistered name would start applying the day
#      somebody else took that name.
#
# Usage:
#   CONTRACT=outlayer.testnet OWNER=you.testnet ./scripts/set_connector_prices_testnet.sh
#
set -euo pipefail

CONTRACT="${CONTRACT:?set CONTRACT to the testnet contract account}"
OWNER="${OWNER:?set OWNER to the contract owner account}"
NETWORK_ID="${NETWORK_ID:-testnet}"

# Every connector is deployed by us under this account. The project id is
# derived — `{namespace}/{id}` — so there is nothing to look up and no alias.
NAMESPACE="connectors.outlayer.testnet"

# Who is credited the author's share. NOT derivable from the project's owner,
# which is us: deriving it would pay us the author's cut and leave the author
# with nothing, with every total still adding up.
PROBE_AUTHOR="${PROBE_AUTHOR:-zavodil.testnet}"

near_call() {
  near call "$CONTRACT" "$1" "$2" --accountId "$OWNER" --networkId "$NETWORK_ID"
}

# ---------------------------------------------------------------------------
# connector-probe — the testnet stand-in, and the only connector that exercises
# the payout at all.
#
# Our own connectors sit at a zero share (there the author is us), so nothing we
# ship would otherwise run the split. The six operations below cover every price
# the table can hold (free, a cent, a cent and a half) against every share it
# can hold (none, a third, most, all) — including the combination that is the
# commonest in production and was the last one uncovered: PRICED with a ZERO
# share, which is every connector we own ourselves.
#
#   ping               free, share 0     — a share of nothing is nothing, and no
#                                          earnings row should appear at all
#   whoami             10000, share 0    — PRICED, and the whole of it ours.
#                                          This is the shape of every connector
#                                          WE own, and so the commonest case in
#                                          production; without a row like it,
#                                          nothing would exercise "the author is
#                                          us, keep all of it" against a fee that
#                                          actually exists
#   secret             10000, 70%        — 7000, the ordinary case
#   burn               10000, 33.33%     — 3333, and per OPERATION: a single
#                                          share per project would print 7000
#                                          here too and prove nothing
#   vrf                10000, share 0    — priced like `whoami`, because what it
#                                          proves is about the ALPHA and not
#                                          about money; a share would only add
#                                          noise to the earnings rows the refund
#                                          row below is read against
#   refund             10000, 70%        — a share that is NOT zero, deliberately.
#                                          The refund is subtracted from the
#                                          AUTHOR's earnings, so with a zero
#                                          share there would be nothing for it to
#                                          come out of and the ledger row could
#                                          not tell a working refund from a
#                                          dropped one
#   fetch              15000, 100%       — the whole fee to the author
#   forbidden_fetch    15000, 33.33%     — 4999.5 floored to 4999. The ONLY
#                                          combination on the probe that
#                                          produces a fraction, and the floor is
#                                          not decoration: the reporting sum
#                                          divides in NUMERIC, so without a
#                                          per-row floor it comes back
#                                          fractional, fails to parse as an
#                                          integer and reads as ZERO — the whole
#                                          of the authors' money reported as
#                                          ours
#
# `unpriced` is deliberately ABSENT. An operation with no row must be refused
# before anything runs; the probe implements no such operation either, so if a
# call for it ever reaches the guest, the fail-closed lookup has a hole and the
# runbook says so.
#
# `forbidden_fetch` carries the rounding case on purpose: it RUNS — the guest's
# outbound request is refused inside the allowlist and it reports the error — so
# by the rule in CONNECTORS.md a module that ran and returned an error is
# charged and its author is paid. One row, two things checked.
# ---------------------------------------------------------------------------
near_call set_project_pricing "$(cat <<EOF
{
  "project_id": "$NAMESPACE/connector-probe",
  "pricing": {
    "author_account_id": "$PROBE_AUTHOR",
    "operations": [
      {"operation": "ping",            "price_usd": "0",     "developer_share_bp": 0},
      {"operation": "env",             "price_usd": "0",     "developer_share_bp": 0},
      {"operation": "whoami",          "price_usd": "10000", "developer_share_bp": 0},
      {"operation": "secret",          "price_usd": "10000", "developer_share_bp": 7000},
      {"operation": "burn",            "price_usd": "10000", "developer_share_bp": 3333},
      {"operation": "fetch",           "price_usd": "15000", "developer_share_bp": 10000},
      {"operation": "forbidden_fetch", "price_usd": "15000", "developer_share_bp": 3333},
      {"operation": "vrf",             "price_usd": "10000", "developer_share_bp": 0},
      {"operation": "refund",          "price_usd": "10000", "developer_share_bp": 7000}
    ]
  }
}
EOF
)"

# ---------------------------------------------------------------------------
# near-email is MAINNET-ONLY and is deliberately not priced here.
#
# Its addresses are derived from `.near` accounts, so nothing is deployed at
# `connectors.outlayer.testnet/near-email`; the registry says so too
# (`networks()` returns mainnet alone). Pricing a project that does not exist
# would be refused by the contract anyway — which is the check working, not a
# problem to route around.
#
# The mainnet equivalent, for when that deploy happens:
#
#   near call $CONTRACT set_project_pricing '{
#     "project_id": "connectors.outlayer.near/near-email",
#     "pricing": {
#       "author_account_id": "zavodil.near",
#       "operations": [
#         {"operation": "send",                 "price_usd": "10000", "developer_share_bp": 7000},
#         {"operation": "send_with_attachment", "price_usd": "15000", "developer_share_bp": 7000},
#         {"operation": "list",                 "price_usd": "0",     "developer_share_bp": 0},
#         {"operation": "read",                 "price_usd": "0",     "developer_share_bp": 0}
#       ]
#     }
#   }' --accountId $OWNER --networkId mainnet
# ---------------------------------------------------------------------------

echo
echo "Reading it back:"
near view "$CONTRACT" get_project_pricing \
  "{\"project_id\": \"$NAMESPACE/connector-probe\"}" --networkId "$NETWORK_ID"

# The DEAREST operation — what to budget for, not what to attach. The contract
# reads the `operation` field out of the request and requires exactly that
# operation's price, so a `ping` costs nothing and a `fetch` costs 15000.
echo
echo "The dearest operation, for budgeting (expect 15000):"
near view "$CONTRACT" get_project_max_price \
  "{\"project_id\": \"$NAMESPACE/connector-probe\"}" --networkId "$NETWORK_ID"

echo
echo "Done. Now tell the coordinator, or it bills the OLD prices for up to a"
echo "minute and refuses a newly priced connector entirely:"
echo
echo "  curl -X POST \$API/admin/connector-prices/refresh -H \"Authorization: Bearer \$ADMIN_TOKEN\""
