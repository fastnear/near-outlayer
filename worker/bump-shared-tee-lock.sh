#!/usr/bin/env bash
#
# Bump this component's committed Cargo.lock onto the current `shared-tee-helpers`
# main HEAD. The file is component-agnostic — it acts on the directory it lives in,
# so an identical copy sits in both `worker/` and `keystore-worker/`.
#
# WHY THIS EXISTS:
#   Both components COMMIT their Cargo.lock (reproducible TEE builds), so the lock
#   PINS the `shared-tee-helpers` git rev and docker/CI do NOT pull a newer main on
#   their own. This script moves that pin.
#
# WHY THERE IS NO PATCH JUGGLING HERE:
#   A local `[patch]` (path = ../../shared-tee-helpers) rewrites the lock entry to a
#   SOURCELESS/path form on every cargo run — including the `cargo metadata` a running
#   rust-analyzer issues continuously, which reverts a freshly bumped lock within
#   seconds. So the patch is kept OPT-IN, parked as `.cargo/config.toml.local` and
#   moved into place only while the crate itself is being edited:
#
#     work on the crate locally:  mv .cargo/config.toml.local .cargo/config.toml
#     back to the pinned rev:     mv .cargo/config.toml .cargo/config.toml.local
#
set -euo pipefail

# Operate from this component's root (this script's directory), regardless of cwd.
cd "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

COMPONENT="$(basename "$PWD")"
CONFIG=".cargo/config.toml"
CRATE="shared-tee-helpers"

# Refuse to run under an active path patch: the lock written here would be sourceless,
# and the next cargo run would revert it anyway. Fail loudly instead of pretending.
if [[ -f "$CONFIG" ]] && grep -q '^\[patch\.' "$CONFIG"; then
  echo "error: $CONFIG has an active [patch] — the lock cannot be pinned while it is" >&2
  echo "       in place. Park it first:  mv $CONFIG $CONFIG.local"                    >&2
  exit 1
fi

echo "Re-resolving $CRATE from git (branch = main HEAD)..."

# Two steps, and the order matters — they cover the two states the lock can be in.
#
#   cargo fetch      HEALS a sourceless entry left by an earlier patched run.
#                    `cargo update -p` cannot: with no patch the path package does
#                    not exist, so the spec matches nothing and cargo exits with
#                    `package ID specification ... did not match any packages`.
#
#   cargo update -p  MOVES an entry that already carries a git source onto current
#                    main HEAD. fetch alone keeps the old rev, since an existing pin
#                    still satisfies `branch = "main"`.
cargo fetch
cargo update -p "$CRATE"

# Verify the entry is a git source. The `source` line follows `name`/`version`.
SRC_LINE="$(grep -A2 "^name = \"$CRATE\"" Cargo.lock | grep '^source = "git+' || true)"
if [[ -z "$SRC_LINE" ]]; then
  echo "error: Cargo.lock entry for $CRATE has no git source — the lock is still"  >&2
  echo "       sourceless. Is a [patch] active somewhere up the directory tree?"   >&2
  exit 1
fi

echo
echo "OK — $CRATE is now pinned to:"
echo "  ${SRC_LINE#source = }"
echo
echo "Next: commit $COMPONENT/Cargo.lock."
