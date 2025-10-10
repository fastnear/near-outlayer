#!/bin/bash
# Clear WASM cache from coordinator and database
# This forces recompilation of all WASM files

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "🧹 Cleaning WASM cache..."
echo "=========================="
echo ""

# Get WASM cache directory from .env or use default
WASM_CACHE_DIR="/tmp/offchainvm/wasm"
if [ -f "$PROJECT_ROOT/coordinator/.env" ]; then
    ENV_DIR=$(grep "^WASM_CACHE_DIR=" "$PROJECT_ROOT/coordinator/.env" | cut -d'=' -f2)
    if [ -n "$ENV_DIR" ]; then
        WASM_CACHE_DIR="$ENV_DIR"
    fi
fi

echo "📁 WASM cache directory: $WASM_CACHE_DIR"
echo ""

# 1. Clear filesystem cache
if [ -d "$WASM_CACHE_DIR" ]; then
    FILE_COUNT=$(find "$WASM_CACHE_DIR" -name "*.wasm" -type f 2>/dev/null | wc -l | tr -d ' ')
    if [ "$FILE_COUNT" -gt 0 ]; then
        echo "🗑️  Found $FILE_COUNT WASM files in filesystem cache"
        rm -f "$WASM_CACHE_DIR"/*.wasm
        echo "✅ Deleted $FILE_COUNT WASM files from filesystem"
    else
        echo "✨ Filesystem cache is already empty"
    fi
else
    echo "📂 Creating cache directory: $WASM_CACHE_DIR"
    mkdir -p "$WASM_CACHE_DIR"
fi

echo ""

# 2. Clear database records
echo "🗄️  Clearing database cache records..."

# Get database URL from .env or use default
DATABASE_URL="postgres://postgres:postgres@localhost/offchainvm"
if [ -f "$PROJECT_ROOT/coordinator/.env" ]; then
    ENV_DB=$(grep "^DATABASE_URL=" "$PROJECT_ROOT/coordinator/.env" | cut -d'=' -f2)
    if [ -n "$ENV_DB" ]; then
        DATABASE_URL="$ENV_DB"
    fi
fi

# Replace docker service names with localhost if needed
DATABASE_URL=$(echo "$DATABASE_URL" | sed 's/@postgres/@localhost/g' | sed 's/@redis/@localhost/g')

echo "🔌 Database: $DATABASE_URL"

# Clear wasm_cache table
psql "$DATABASE_URL" -c "DELETE FROM wasm_cache;" 2>/dev/null && {
    echo "✅ Cleared wasm_cache table"
} || {
    echo "⚠️  Could not clear database (coordinator might not be running or DB not accessible)"
    echo "   You can manually run: psql $DATABASE_URL -c 'DELETE FROM wasm_cache;'"
}

echo ""

# 3. Clear Redis locks (optional but recommended)
echo "🔐 Clearing Redis compilation locks..."

REDIS_URL="redis://localhost:6379"
if [ -f "$PROJECT_ROOT/coordinator/.env" ]; then
    ENV_REDIS=$(grep "^REDIS_URL=" "$PROJECT_ROOT/coordinator/.env" | cut -d'=' -f2)
    if [ -n "$ENV_REDIS" ]; then
        REDIS_URL="$ENV_REDIS"
    fi
fi

REDIS_URL=$(echo "$REDIS_URL" | sed 's/redis:\/\/redis/redis:\/\/localhost/g')

# Extract host and port
REDIS_HOST=$(echo "$REDIS_URL" | sed 's|redis://||' | cut -d':' -f1)
REDIS_PORT=$(echo "$REDIS_URL" | cut -d':' -f2)

redis-cli -h "$REDIS_HOST" -p "$REDIS_PORT" --scan --pattern "compile:*" | xargs -r redis-cli -h "$REDIS_HOST" -p "$REDIS_PORT" DEL 2>/dev/null && {
    echo "✅ Cleared Redis compilation locks"
} || {
    echo "⚠️  Could not clear Redis locks (redis-cli might not be installed or Redis not accessible)"
    echo "   Locks will expire automatically after 5 minutes"
}

echo ""
echo "✅ WASM cache cleared successfully!"
echo ""
echo "📝 Next compilation will rebuild WASM from GitHub for each repo+commit pair"
