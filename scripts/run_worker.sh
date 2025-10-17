#!/bin/bash
ENV_FILE=${1:-.env}

if [ ! -f "worker/$ENV_FILE" ]; then
    echo "Error: $ENV_FILE not found"
    exit 1
fi

cd worker
set -a
source $ENV_FILE
set +a
RUST_LOG=offchainvm_worker=debug cargo run --release