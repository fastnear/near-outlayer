#!/usr/bin/env bash
#
# Put the Mercury connector's prices on chain, on TESTNET.
#
# Same shape as `set_connector_prices_testnet.sh` (which owns the probe's
# numbers and the runbook's arithmetic); kept separate so the probe's price
# table — every number of which a test asserts — is never edited by accident
# while pricing a real connector.
#
# Until this has run, every call to the connector is REFUSED — an unpriced
# project is not a free one.
#
# Prerequisites, in order:
#   1. the contract is at storage version 8 (`get_storage_version`);
#   2. `connectors.outlayer.testnet/mercury` exists — `set_project_pricing`
#      refuses a project nobody registered.
#
# Usage:
#   CONTRACT=outlayer.testnet OWNER=owner.outlayer.testnet ./scripts/set_mercury_prices_testnet.sh
#
set -euo pipefail

CONTRACT="${CONTRACT:?set CONTRACT to the testnet contract account}"
OWNER="${OWNER:?set OWNER to the contract owner account}"
NETWORK_ID="${NETWORK_ID:-testnet}"
NAMESPACE="connectors.outlayer.testnet"

# Our own connector: the author is us, the share is zero, the whole fee stays
# with the platform. `author_account_id` is still required by the contract.
AUTHOR="${MERCURY_AUTHOR:-$NAMESPACE}"

near_call() {
  near call "$CONTRACT" "$1" "$2" --accountId "$OWNER" --networkId "$NETWORK_ID"
}

# Prices in minimal stablecoin units (6 decimals): 1000 = $0.001, 10000 = $0.01.
#
#   status            free   — health check and the token keep-alive; an agent
#                              is meant to call it often, and it moves nothing
#   reads             $0.001 — accounts, recipients, transactions, payment
#                              status, invoice listings, the rendered invoice
#   writes            $0.01  — add_recipient, pay_invoice, send_invoice,
#                              cancel_invoice: each is a bank-side act
#
# These are placeholders for the testnet run; the mainnet numbers are decided
# at publication, not here.
near_call set_project_pricing "$(cat <<EOF
{
  "project_id": "$NAMESPACE/mercury",
  "pricing": {
    "author_account_id": "$AUTHOR",
    "operations": [
      {"operation": "status",         "price_usd": "0",     "developer_share_bp": 0},
      {"operation": "accounts",       "price_usd": "1000",  "developer_share_bp": 0},
      {"operation": "recipients",     "price_usd": "1000",  "developer_share_bp": 0},
      {"operation": "transactions",   "price_usd": "1000",  "developer_share_bp": 0},
      {"operation": "payment_status", "price_usd": "1000",  "developer_share_bp": 0},
      {"operation": "create_invoice", "price_usd": "1000",  "developer_share_bp": 0},
      {"operation": "customers",      "price_usd": "1000",  "developer_share_bp": 0},
      {"operation": "invoices",       "price_usd": "1000",  "developer_share_bp": 0},
      {"operation": "invoice_status", "price_usd": "1000",  "developer_share_bp": 0},
      {"operation": "add_recipient",  "price_usd": "10000", "developer_share_bp": 0},
      {"operation": "pay_invoice",    "price_usd": "10000", "developer_share_bp": 0},
      {"operation": "send_invoice",   "price_usd": "10000", "developer_share_bp": 0},
      {"operation": "cancel_invoice", "price_usd": "10000", "developer_share_bp": 0}
    ]
  }
}
EOF
)"

echo
echo "Reading it back:"
near view "$CONTRACT" get_project_pricing \
  "{\"project_id\": \"$NAMESPACE/mercury\"}" --networkId "$NETWORK_ID"

echo
echo "The dearest operation, for budgeting (expect 10000):"
near view "$CONTRACT" get_project_max_price \
  "{\"project_id\": \"$NAMESPACE/mercury\"}" --networkId "$NETWORK_ID"

echo
echo "Now tell the coordinator, or it refuses the connector for up to a minute:"
echo
echo "  curl -X POST \$API/admin/connector-prices/refresh -H \"Authorization: Bearer \$ADMIN_TOKEN\""
