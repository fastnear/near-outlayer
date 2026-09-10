#!/bin/bash
set -e

cd "$(dirname "$0")"

WASM_FILE="target/wasm32-wasip2/release/subkey-probe.wasm"
WORKER_WIT="../../worker/wit/deps/wallet.wit"
MAX_SIZE=$((2 * 1024 * 1024))  # 2MB in bytes

# The WIT here is a COPY of the worker's. A drifted copy compiles against an
# interface the host no longer provides and fails at runtime as a missing
# import, so the drift is reported here and never silently fixed.
if [ -f "$WORKER_WIT" ]; then
    if ! diff -q "$WORKER_WIT" wit/wallet.wit >/dev/null; then
        echo "ERROR: wit/wallet.wit has drifted from $WORKER_WIT"
        diff "$WORKER_WIT" wit/wallet.wit || true
        echo "Copy the worker's version over if the interface changed:"
        echo "  cp $WORKER_WIT wit/wallet.wit"
        exit 1
    fi
    echo "WIT: matches the worker's"
else
    echo "WIT: worker copy not found at $WORKER_WIT — skipping the drift check"
fi

echo "Building WASI module (wasm32-wasip2)..."
rustup target add wasm32-wasip2 2>/dev/null || true
cargo build --target wasm32-wasip2 --release
echo ""

SIZE=$(stat -f%z "$WASM_FILE" 2>/dev/null || stat -c%s "$WASM_FILE" 2>/dev/null)
echo "WASM module: $WASM_FILE"
echo "Size: $((SIZE / 1024)) KB"

# The wallet interface must actually be imported — a build that dropped it
# would run and test nothing.
if command -v wasm-tools >/dev/null 2>&1; then
    if ! wasm-tools component wit "$WASM_FILE" 2>/dev/null | grep -q "outlayer:wallet"; then
        echo "ERROR: $WASM_FILE does not import outlayer:wallet"
        exit 1
    fi
    echo "OK: outlayer:wallet is imported"
fi

# The connector manifest must be in the artefact: it carries `connector_id`,
# which is what makes sub-keys available to this module at all.
if ! grep -qa 'outlayer.manifest' "$WASM_FILE"; then
    echo "ERROR: outlayer.manifest custom section is missing from $WASM_FILE"
    echo "See wasi-examples/CONNECTOR_MANIFEST.md"
    exit 1
fi
echo "Manifest section: present"

HASH=$(shasum -a 256 "$WASM_FILE" | cut -d' ' -f1)
echo "SHA256: $HASH"

if [ "$SIZE" -gt "$MAX_SIZE" ]; then
    echo "ERROR: Size exceeds the 2MB FastFS limit"
    exit 1
fi
echo "OK: Size is within 2MB limit for FastFS"
