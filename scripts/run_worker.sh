#!/bin/bash

# Usage: ./run_worker.sh <env-file> [--verbose]
# Examples:
#   ./run_worker.sh .env.worker1
#   ./run_worker.sh .env.worker2 --verbose
#   ./run_worker.sh .env.testnet --verbose
#   ./run_worker.sh .env (mainnet)

ENV_FILE=${1:-.env}
VERBOSE=false

# Check if second argument is --verbose
if [ "$2" = "--verbose" ] || [ "$2" = "-v" ]; then
    VERBOSE=true
fi

if [ ! -f "worker/$ENV_FILE" ]; then
    echo "Error: worker/$ENV_FILE not found"
    echo ""
    echo "Usage: ./run_worker.sh <env-file> [--verbose]"
    echo "Examples:"
    echo "  ./run_worker.sh .env.worker1"
    echo "  ./run_worker.sh .env.worker2 --verbose"
    echo "  ./run_worker.sh .env.testnet --verbose"
    exit 1
fi

echo "Starting worker with $ENV_FILE..."
if [ "$VERBOSE" = true ]; then
    echo "Verbose logging enabled"
fi
echo ""

cd worker
set -a
source "$ENV_FILE"
set +a

if [ "$VERBOSE" = true ]; then
    RUST_LOG=offchainvm_worker=debug,offchainvm_worker::outlayer_rpc=info cargo run --release
else
    RUST_LOG=offchainvm_worker=info cargo run --release
fi