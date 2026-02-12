#!/bin/bash
set -e

echo "Building payment-keys-with-intents for WASI..."
cargo build --release --target wasm32-wasip2

echo "Build complete: target/wasm32-wasip2/release/payment-keys-with-intents.wasm"
