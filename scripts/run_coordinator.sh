#!/bin/bash

# Usage: ./run_coordinator.sh [testnet|mainnet]
# Default: mainnet

NETWORK=${1:-mainnet}

if [ "$NETWORK" != "testnet" ] && [ "$NETWORK" != "mainnet" ]; then
    echo "Error: Invalid network. Use 'testnet' or 'mainnet'"
    exit 1
fi

COMPOSE_FILE="docker-compose.$NETWORK.yml"

if [ ! -f "$COMPOSE_FILE" ]; then
    echo "Error: $COMPOSE_FILE not found"
    exit 1
fi

echo "Starting coordinator for $NETWORK..."
docker-compose -f "$COMPOSE_FILE" up -d

echo ""
echo "Coordinator started!"
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
echo "Check logs: docker-compose -f $COMPOSE_FILE logs -f"
echo "Stop: docker-compose -f $COMPOSE_FILE down"
