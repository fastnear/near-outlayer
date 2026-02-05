#!/bin/bash
# Cleanup workers with stale heartbeats (>500 seconds ago)
# Usage: ADMIN_TOKEN=xxx COORDINATOR_URL=http://localhost:8080 ./cleanup-workers.sh

set -e

: "${ADMIN_TOKEN:?ADMIN_TOKEN environment variable is required}"
: "${COORDINATOR_URL:=http://localhost:8080}"

THRESHOLD_SECS=${THRESHOLD_SECS:-500}

echo "Fetching health from $COORDINATOR_URL..."

HEALTH=$(curl -s "$COORDINATOR_URL/health/detailed")

if [ -z "$HEALTH" ]; then
    echo "Error: Failed to fetch health data"
    exit 1
fi

# Extract workers with stale heartbeats using jq
STALE_WORKERS=$(echo "$HEALTH" | jq -r --argjson threshold "$THRESHOLD_SECS" '
    .checks.workers.details[]
    | select(.last_heartbeat_secs_ago > $threshold)
    | "\(.worker_id)\t\(.worker_name)\t\(.last_heartbeat_secs_ago)s"
')

if [ -z "$STALE_WORKERS" ]; then
    echo "No stale workers found (threshold: ${THRESHOLD_SECS}s)"
    exit 0
fi

echo "Found stale workers (heartbeat > ${THRESHOLD_SECS}s):"
echo "$STALE_WORKERS"
echo ""

# Extract just worker IDs
WORKER_IDS=$(echo "$HEALTH" | jq -r --argjson threshold "$THRESHOLD_SECS" '
    .checks.workers.details[]
    | select(.last_heartbeat_secs_ago > $threshold)
    | .worker_id
')

for WORKER_ID in $WORKER_IDS; do
    echo "Deleting worker: $WORKER_ID"
    HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" -X DELETE \
        -H "Authorization: Bearer $ADMIN_TOKEN" \
        "$COORDINATOR_URL/admin/workers/$WORKER_ID")

    if [ "$HTTP_CODE" = "200" ]; then
        echo "  Deleted successfully"
    else
        echo "  Failed (HTTP $HTTP_CODE)"
    fi
done

echo ""
echo "Cleanup complete"
