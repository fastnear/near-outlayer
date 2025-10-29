#!/bin/bash

# Usage: ./run_coordinator.sh [testnet|mainnet] [--no-cache]
# Default: mainnet
# --no-cache: Force rebuild without cache

NETWORK=${1:-mainnet}
NO_CACHE=""

# Parse arguments
for arg in "$@"; do
    if [ "$arg" = "--no-cache" ]; then
        NO_CACHE="--no-cache"
    fi
done

if [ "$NETWORK" != "testnet" ] && [ "$NETWORK" != "mainnet" ]; then
    echo "Error: Invalid network. Use 'testnet' or 'mainnet'"
    exit 1
fi

COMPOSE_FILE="docker-compose.$NETWORK.yml"

if [ ! -f "$COMPOSE_FILE" ]; then
    echo "Error: $COMPOSE_FILE not found"
    exit 1
fi

# Detect docker-compose command (old vs new)
if command -v docker-compose &> /dev/null; then
    DOCKER_COMPOSE="docker-compose"
else
    DOCKER_COMPOSE="docker compose"
fi

# Determine script directory and load environment
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Load root .env file for Docker variables
ROOT_ENV="$PROJECT_ROOT/.env"
if [ ! -f "$ROOT_ENV" ]; then
    echo "Error: Root environment file not found: $ROOT_ENV"
    exit 1
fi

# Source the root .env file
set -a
source "$ROOT_ENV"
set +a

# Check coordinator .env file exists (for docker-compose to use)
if [ "$NETWORK" = "testnet" ]; then
    ENV_FILE="$PROJECT_ROOT/coordinator/.env.testnet"
else
    ENV_FILE="$PROJECT_ROOT/coordinator/.env"
fi

if [ ! -f "$ENV_FILE" ]; then
    echo "Error: Environment file not found: $ENV_FILE"
    exit 1
fi

# Set network-specific variables from root .env
if [ "$NETWORK" = "testnet" ]; then
    POSTGRES_PORT=${POSTGRES_EXTERNAL_PORT_TESTNET:-5432}
    POSTGRES_PASSWORD=${POSTGRES_PASSWORD_TESTNET:-postgres}
    POSTGRES_DB=${POSTGRES_DB_TESTNET:-offchainvm}
else
    POSTGRES_PORT=${POSTGRES_EXTERNAL_PORT_MAINNET:-5433}
    POSTGRES_PASSWORD=${POSTGRES_PASSWORD_MAINNET:-postgres}
    POSTGRES_DB=${POSTGRES_DB_MAINNET:-outlayer}
fi

# Step 1: Update SQLx cache (if SQL queries changed)
echo "═══════════════════════════════════════════════════════════"
echo "Step 1: Updating SQLx query cache..."
echo "═══════════════════════════════════════════════════════════"
cd coordinator
if ! DATABASE_URL="postgres://postgres:${POSTGRES_PASSWORD}@localhost:${POSTGRES_PORT}/${POSTGRES_DB}" cargo sqlx prepare; then
    echo ""
    echo "⚠️  Warning: SQLx prepare failed (database might be offline)"
    echo "Continuing with offline mode..."
fi
cd ..
echo ""
echo "✅ SQLx cache updated"
echo ""

# Step 2: Build Rust binary
echo "═══════════════════════════════════════════════════════════"
echo "Step 2: Building Rust binary (release mode)..."
echo "═══════════════════════════════════════════════════════════"
cd coordinator
if ! env SQLX_OFFLINE=true cargo build --release --bin offchainvm-coordinator; then
    echo ""
    echo "❌ Error: Rust build failed!"
    exit 1
fi
cd ..
echo ""
echo "✅ Rust binary built successfully"
echo ""

# Step 3: Build Docker image
echo "═══════════════════════════════════════════════════════════"
echo "Step 3: Building Docker image for $NETWORK..."
echo "═══════════════════════════════════════════════════════════"
if [ -n "$NO_CACHE" ]; then
    echo "(Using --no-cache flag)"
fi
if ! $DOCKER_COMPOSE -f "$COMPOSE_FILE" build $NO_CACHE coordinator; then
    echo ""
    echo "❌ Error: Docker build failed!"
    exit 1
fi
echo ""
echo "✅ Docker image built successfully"
echo ""

# Step 4: Start/restart coordinator
echo "═══════════════════════════════════════════════════════════"
echo "Step 4: Starting coordinator for $NETWORK..."
echo "═══════════════════════════════════════════════════════════"
$DOCKER_COMPOSE -f "$COMPOSE_FILE" up -d

echo ""
echo "✅ Coordinator started successfully!"
echo ""
echo "Network: $NETWORK"
if [ "$NETWORK" = "testnet" ]; then
    echo "PostgreSQL: localhost:${POSTGRES_EXTERNAL_PORT_TESTNET:-5432} (DB: ${POSTGRES_DB_TESTNET:-offchainvm})"
    echo "Redis: localhost:${REDIS_EXTERNAL_PORT_TESTNET:-6379}"
    echo "Coordinator API: http://localhost:${COORDINATOR_EXTERNAL_PORT_TESTNET:-8080}"
else
    echo "PostgreSQL: localhost:${POSTGRES_EXTERNAL_PORT_MAINNET:-5433} (DB: ${POSTGRES_DB_MAINNET:-outlayer})"
    echo "Redis: localhost:${REDIS_EXTERNAL_PORT_MAINNET:-6380}"
    echo "Coordinator API: http://localhost:${COORDINATOR_EXTERNAL_PORT_MAINNET:-8180}"
fi
echo ""
echo "Check logs: $DOCKER_COMPOSE -f $COMPOSE_FILE logs -f"
echo "Stop: $DOCKER_COMPOSE -f $COMPOSE_FILE down"
