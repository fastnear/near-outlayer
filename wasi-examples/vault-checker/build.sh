#!/bin/bash
set -e

cd "$(dirname "$0")"

WASM_FILE="target/wasm32-wasip2/release/vault-checker.wasm"
MAX_SIZE=$((2 * 1024 * 1024))

echo "Building vault-checker (wasm32-wasip2)..."

rustup target add wasm32-wasip2 2>/dev/null || true

cargo build --target wasm32-wasip2 --release

SIZE=$(stat -f%z "$WASM_FILE" 2>/dev/null || stat -c%s "$WASM_FILE" 2>/dev/null)
SIZE_KB=$(echo "scale=0; $SIZE / 1024" | bc)
SIZE_MB=$(echo "scale=2; $SIZE / 1024 / 1024" | bc)

echo
echo "WASM:   $WASM_FILE"
echo "Size:   ${SIZE_KB} KB (${SIZE_MB} MB)"

HASH=$(shasum -a 256 "$WASM_FILE" | cut -d' ' -f1)
echo "SHA256: $HASH"

if [ "$SIZE" -gt "$MAX_SIZE" ]; then
    echo
    echo "WARNING: exceeds 2MB limit for FastFS upload."
fi
