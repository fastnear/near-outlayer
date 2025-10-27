#!/bin/bash

# Usage: ./run_keystore.sh [testnet|mainnet]
# Default: mainnet

NETWORK=${1:-mainnet}

if [ "$NETWORK" != "testnet" ] && [ "$NETWORK" != "mainnet" ]; then
    echo "Error: Invalid network. Use 'testnet' or 'mainnet'"
    exit 1
fi

# Determine env file
if [ "$NETWORK" = "testnet" ]; then
    ENV_FILE=".env.testnet"
else
    ENV_FILE=".env"
fi

if [ ! -f "keystore-worker/$ENV_FILE" ]; then
    echo "Error: keystore-worker/$ENV_FILE not found"
    echo "Please create it from .env.example"
    exit 1
fi

echo "Starting keystore worker for $NETWORK..."
echo "Env file: $ENV_FILE"
echo ""

cd keystore-worker
set -a
source "$ENV_FILE"
set +a
cargo run --release
