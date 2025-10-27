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

# Step 1: Build Rust binary
echo "═══════════════════════════════════════════════════════════"
echo "Step 1: Building Rust binary (release mode)..."
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

# Step 2: Build Docker image
echo "═══════════════════════════════════════════════════════════"
echo "Step 2: Building Docker image for $NETWORK..."
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

# Step 3: Start/restart coordinator
echo "═══════════════════════════════════════════════════════════"
echo "Step 3: Starting coordinator for $NETWORK..."
echo "═══════════════════════════════════════════════════════════"
$DOCKER_COMPOSE -f "$COMPOSE_FILE" up -d

echo ""
echo "✅ Coordinator started successfully!"
echo ""
echo "Network: $NETWORK"
if [ "$NETWORK" = "testnet" ]; then
    echo "PostgreSQL: localhost:5432"
    echo "Redis: localhost:6379"
    echo "Coordinator API: http://localhost:8080"
else
    echo "PostgreSQL: localhost:5433"
    echo "Redis: localhost:6380"
    echo "Coordinator API: http://localhost:8180"
fi
echo ""
echo "Check logs: $DOCKER_COMPOSE -f $COMPOSE_FILE logs -f"
echo "Stop: $DOCKER_COMPOSE -f $COMPOSE_FILE down"
