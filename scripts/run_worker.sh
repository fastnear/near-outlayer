#!/bin/bash

# Usage: ./run_worker.sh <env-file>
# Examples:
#   ./run_worker.sh .env.worker1
#   ./run_worker.sh .env.worker2
#   ./run_worker.sh .env.testnet
#   ./run_worker.sh .env (mainnet)

ENV_FILE=${1:-.env}

if [ ! -f "worker/$ENV_FILE" ]; then
    echo "Error: worker/$ENV_FILE not found"
    echo ""
    echo "Usage: ./run_worker.sh <env-file>"
    echo "Examples:"
    echo "  ./run_worker.sh .env.worker1"
    echo "  ./run_worker.sh .env.worker2"
    echo "  ./run_worker.sh .env.testnet"
    exit 1
fi

echo "Starting worker with $ENV_FILE..."
echo ""

cd worker
set -a
source "$ENV_FILE"
set +a
RUST_LOG=offchainvm_worker=debug cargo run --release