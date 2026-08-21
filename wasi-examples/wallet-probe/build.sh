#!/bin/bash
set -e

cd "$(dirname "$0")"

WASM_FILE="target/wasm32-wasip2/release/wallet-probe.wasm"
WORKER_WIT="../../worker/wit/deps/wallet.wit"
MAX_SIZE=$((2 * 1024 * 1024))  # 2MB in bytes

# The WIT here is a COPY of the worker's, and a copy that has drifted is worse
# than no copy: the bindings would compile against an interface the host no
# longer provides, and the failure would arrive at runtime as a missing import.
#
# Reported rather than fixed. Which side is right is a decision — the worker may
# have gained a function this module has no business calling — and a build
# script silently overwriting either one would make that decision quietly.
if [ -f "$WORKER_WIT" ]; then
    if ! diff -q "$WORKER_WIT" wit/wallet.wit >/dev/null; then
        echo "ERROR: wit/wallet.wit has drifted from $WORKER_WIT"
        echo ""
        diff "$WORKER_WIT" wit/wallet.wit || true
        echo ""
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
SIZE_KB=$(echo "scale=0; $SIZE / 1024" | bc)

echo "WASM module: $WASM_FILE"
echo "Size: ${SIZE_KB} KB"

# The manifest must be in the artefact: it declares `"network": []`, and for an
# ordinary project an empty list is ENFORCED while a missing key is not. A
# linker that dropped the static (a missing `#[used]`) would turn an enforced
# "talks to nobody" into an unenforced nothing, silently.
if ! grep -qa 'outlayer.manifest' "$WASM_FILE"; then
    echo ""
    echo "ERROR: outlayer.manifest custom section is missing from $WASM_FILE"
    echo "See wasi-examples/CONNECTOR_MANIFEST.md"
    exit 1
fi
echo "Manifest section: present"

# The import this module exists for. Without it the worker would run it happily
# and every wallet call inside would be a link error — so its absence is a
# broken build, not a smaller one.
if command -v wasm-tools >/dev/null 2>&1; then
    if wasm-tools component wit "$WASM_FILE" 2>/dev/null | grep -q "outlayer:wallet"; then
        echo "Wallet import: present"
    else
        echo ""
        echo "ERROR: $WASM_FILE does not import outlayer:wallet/api"
        echo "The whole point of this module is that import; a build without it tests nothing."
        exit 1
    fi
else
    echo "Wallet import: NOT CHECKED (install wasm-tools to verify)"
fi

HASH=$(shasum -a 256 "$WASM_FILE" | cut -d' ' -f1)
echo "SHA256: $HASH"

if [ "$SIZE" -gt "$MAX_SIZE" ]; then
    echo ""
    echo "WARNING: File size exceeds the 2MB limit for FastFS upload"
else
    echo "OK: Size is within 2MB limit for FastFS"
fi
