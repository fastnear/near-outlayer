#!/usr/bin/env bash
#
# Put the Hyperliquid connector's prices on chain, on TESTNET.
#
# Same rules as `set_connector_prices_testnet.sh`: the contract must be at
# storage version 8 and the project published under the namespace first, and
# after this runs the coordinator must be told
# (`POST /admin/connector-prices/refresh`). The author is us, so the share is 0.
#
# Usage:
#   CONTRACT=outlayer.testnet OWNER=owner.outlayer.testnet ./scripts/set_hyperliquid_prices_testnet.sh
#
set -euo pipefail

CONTRACT="${CONTRACT:?set CONTRACT to the testnet contract account}"
OWNER="${OWNER:?set OWNER to the contract owner account}"
NETWORK_ID="${NETWORK_ID:-testnet}"
NAMESPACE="connectors.outlayer.testnet"
PROJECT="$NAMESPACE/hyperliquid"

# The priced operations must be exactly the manifest's.
python3 - "$0" "$(dirname "$0")/../connectors/hyperliquid-connector/manifest.json" <<'PY'
import json, re, sys
script = open(sys.argv[1]).read()
priced = set(re.findall(r'\{"operation": "([a-z_]+)"', script))
declared = set(json.load(open(sys.argv[2]))["operations"])
if priced != declared:
    print(f"ERROR: priced {sorted(priced)} vs manifest {sorted(declared)}"); sys.exit(1)
print("Manifest operations agree with the price list")
PY

# Reads are cheap, writes that sign are a cent; `status`/`address` are free so
# an agent can look before it pays.
near call "$CONTRACT" set_project_pricing "$(cat <<JSON
{
  "project_id": "$PROJECT",
  "pricing": {
    "author_account_id": "$NAMESPACE",
    "operations": [
      {"operation": "status",    "price_usd": "0",     "developer_share_bp": 0},
      {"operation": "address",   "price_usd": "0",     "developer_share_bp": 0},
      {"operation": "markets",   "price_usd": "1000",  "developer_share_bp": 0},
      {"operation": "leverage",  "price_usd": "1000",  "developer_share_bp": 0},
      {"operation": "order",     "price_usd": "10000", "developer_share_bp": 0},
      {"operation": "cancel",    "price_usd": "1000",  "developer_share_bp": 0},
      {"operation": "class_transfer", "price_usd": "1000", "developer_share_bp": 0},
      {"operation": "orders",    "price_usd": "1000",  "developer_share_bp": 0},
      {"operation": "positions", "price_usd": "1000",  "developer_share_bp": 0}
    ]
  }
}
JSON
)" --accountId "$OWNER" --networkId "$NETWORK_ID"

echo
near view "$CONTRACT" get_project_pricing "{\"project_id\": \"$PROJECT\"}" --networkId "$NETWORK_ID"
echo
echo "Now: curl -X POST \$API/admin/connector-prices/refresh -H \"Authorization: Bearer \$ADMIN_TOKEN\""
