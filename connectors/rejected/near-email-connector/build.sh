#!/bin/bash
set -e

cd "$(dirname "$0")"

WASM_FILE="target/wasm32-wasip2/release/near-email-connector.wasm"
MAX_SIZE=$((2 * 1024 * 1024))  # 2MB in bytes

echo "Building WASI module (wasm32-wasip2)..."
rustup target add wasm32-wasip2 2>/dev/null || true
cargo build --target wasm32-wasip2 --release
echo ""

SIZE=$(stat -f%z "$WASM_FILE" 2>/dev/null || stat -c%s "$WASM_FILE" 2>/dev/null)
echo "WASM module: $WASM_FILE"
echo "Size: $((SIZE / 1024)) KB"

# The manifest must be in the artefact: it carries the connector id, the one
# host this module may reach and the author secret it needs. A build that
# dropped it would publish with NO allowlist and, being a connector, would be
# refused all outbound network at runtime.
if ! grep -qa 'outlayer.manifest' "$WASM_FILE"; then
    echo "ERROR: outlayer.manifest custom section is missing from $WASM_FILE"
    echo "See wasi-examples/CONNECTOR_MANIFEST.md"
    exit 1
fi
echo "Manifest section: present"

# The manifest's operations must be exactly the ones the code dispatches on,
# and the price script checks itself against the manifest — so the three cannot
# drift apart silently. An operation priced but not implemented is money for
# nothing; one implemented but unpriced is refused before it runs.
python3 - manifest.json src/main.rs <<'PY'
import json, re, sys
m = json.load(open(sys.argv[1]))
src = open(sys.argv[2]).read()
declared = set(m["operations"])
advertised = set(re.findall(r'^\s*"([a-z_]+)",$', re.search(r'const OPERATIONS.*?\];', src, re.S).group(0), re.M))
dispatched = set(re.findall(r'^\s*"([a-z_]+)" => ', re.search(r'fn run\(.*?\n\}\n', src, re.S).group(0), re.M))
bad = []
if declared != advertised:
    bad.append(f"manifest {sorted(declared)} vs OPERATIONS {sorted(advertised)}")
if declared != dispatched:
    bad.append(f"manifest {sorted(declared)} vs dispatched {sorted(dispatched)}")
WINDOWS = {"day", "week", "month"}; APPLIES = {"everyone", "unpaid", "covered"}
for limit in m.get("limits", []):
    if limit.get("window") not in WINDOWS:
        bad.append(f"window {limit.get('window')!r}")
    if limit.get("applies", "everyone") not in APPLIES:
        bad.append(f"applies {limit.get('applies')!r}")
    # A limit may scope an operation (`send:external`), and the operation half
    # must exist.
    if limit.get("operation", "").split(":")[0] not in declared:
        bad.append(f"limit on unknown operation {limit.get('operation')!r}")
if m.get("author_secrets", {}).get("profile", "") == "":
    bad.append("author_secrets.profile is empty: the master key would have nowhere to come from")
if bad:
    print("ERROR:"); [print("  -", b) for b in bad]; sys.exit(1)
print(f"Manifest: {len(declared)} operations agree with the code; limits and author_secrets are well formed")
PY

HASH=$(shasum -a 256 "$WASM_FILE" | cut -d' ' -f1)
echo "SHA256: $HASH"

if [ "$SIZE" -gt "$MAX_SIZE" ]; then
    echo "ERROR: Size exceeds the 2MB FastFS limit"
    exit 1
fi
echo "OK: Size is within 2MB limit for FastFS"
