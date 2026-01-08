#!/bin/bash

# Usage: ./scripts/storage_stats.sh [env_file]
# Default: ../coordinator/.env.testnet
#
# Note: If DATABASE_URL uses Docker hostname (e.g., postgres:5432),
# replace with localhost:5432 for local access.

ENV_FILE="${1:-../coordinator/.env.testnet}"

if [ ! -f "$ENV_FILE" ]; then
    echo "Error: $ENV_FILE not found"
    exit 1
fi

# Extract DATABASE_URL from env file
DATABASE_URL=$(grep -E "^DATABASE_URL=" "$ENV_FILE" | cut -d'=' -f2-)

# Replace Docker hostname 'postgres' with 'localhost' for local access
DATABASE_URL=$(echo "$DATABASE_URL" | sed 's/@postgres:/@localhost:/')

if [ -z "$DATABASE_URL" ]; then
    echo "Error: DATABASE_URL not found in $ENV_FILE"
    exit 1
fi

echo "=== Storage Stats ==="
echo "Env: $ENV_FILE"
echo ""

echo "--- Overall Stats ---"
psql "$DATABASE_URL" -c "
SELECT
    COUNT(DISTINCT project_uuid) as projects,
    COALESCE(SUM(total_bytes), 0) as total_bytes,
    COALESCE(SUM(key_count), 0) as total_keys,
    pg_size_pretty(COALESCE(SUM(total_bytes), 0)::bigint) as human_size
FROM storage_usage
WHERE project_uuid IS NOT NULL;
"

echo ""
echo "--- Top 10 Projects by Size ---"
psql "$DATABASE_URL" -c "
SELECT
    project_uuid,
    SUM(total_bytes) as bytes,
    pg_size_pretty(SUM(total_bytes)::bigint) as size,
    SUM(key_count) as keys
FROM storage_usage
WHERE project_uuid IS NOT NULL
GROUP BY project_uuid
ORDER BY bytes DESC
LIMIT 10;
"

echo ""
echo "--- Storage Data Rows ---"
psql "$DATABASE_URL" -c "
SELECT
    COUNT(*) as total_rows,
    COUNT(DISTINCT project_uuid) as unique_projects,
    COUNT(DISTINCT account_id) as unique_accounts
FROM storage_data;
"
