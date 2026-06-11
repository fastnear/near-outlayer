#!/usr/bin/env bash
#
# Bump the committed keystore-worker Cargo.lock to the current `shared-tee-helpers`
# main HEAD.
#
# WHY THIS EXISTS:
#   keystore-worker COMMITS its Cargo.lock (reproducible TEE builds), so the lock
#   PINS the `shared-tee-helpers` git rev — docker/CI do NOT auto-pull a newer main.
#   The local `.cargo/config.toml` `[patch]` (path = ../../shared-tee-helpers) makes
#   any normal `cargo` run rewrite the lock entry to a SOURCELESS/path form, dropping
#   the `source = "git+...#rev"` line. Committing that polluted lock makes CI (which
#   has no patch) resolve a STALE crate and fail to compile.
#
# WHAT THIS DOES:
#   Temporarily disables the path patch, re-resolves `shared-tee-helpers` from git
#   (so the lock records the real `source = "git+...#<HEAD rev>"`), restores the
#   patch, and verifies the result. Run it whenever the crate's main has moved and
#   the keystore needs the new code.
#
# AFTER RUNNING:
#   Commit ONLY keystore-worker/Cargo.lock. Do NOT run a patched `cargo build` before
#   committing — it would re-pollute the lock back to the sourceless form.
#
set -euo pipefail

# Operate from the keystore-worker root (this script's directory), regardless of cwd.
cd "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

CONFIG=".cargo/config.toml"
BACKUP=".cargo/config.toml.bak"
CRATE="shared-tee-helpers"

if [[ ! -f "$CONFIG" ]]; then
  echo "error: $CONFIG not found — run this from inside the keystore-worker checkout." >&2
  exit 1
fi

# Always restore the patch config, even if cargo fails or the script is interrupted.
restore_patch() {
  if [[ -f "$BACKUP" ]]; then
    mv -f "$BACKUP" "$CONFIG"
    echo "Restored $CONFIG (local path patch re-enabled)."
  fi
}
trap restore_patch EXIT

echo "Disabling local path patch ($CONFIG -> $BACKUP)..."
mv -f "$CONFIG" "$BACKUP"

echo "Re-resolving $CRATE from git (branch = main HEAD)..."
cargo update -p "$CRATE"

# restore_patch runs here via the EXIT trap.
trap - EXIT
restore_patch

# Verify the lock entry is now a git source (NOT sourceless/path). The `source` line
# appears immediately after the `name`/`version` lines for the crate's lock entry.
SRC_LINE="$(grep -A2 "name = \"$CRATE\"" Cargo.lock | grep '^source = "git+' || true)"
if [[ -z "$SRC_LINE" ]]; then
  echo "error: Cargo.lock entry for $CRATE has no git source — the lock is still" >&2
  echo "       polluted (sourceless/path). Did the patch get re-applied mid-run?"   >&2
  exit 1
fi

echo
echo "OK — $CRATE is now pinned to:"
echo "  ${SRC_LINE#source = }"
echo
echo "Next: commit ONLY keystore-worker/Cargo.lock and push."
echo "Do NOT run a patched 'cargo build' before committing (it re-pollutes the lock)."
echo "Optional CI sanity check (uses the git dep, like CI):"
echo "  mv $CONFIG $BACKUP && cargo check --release; mv $BACKUP $CONFIG"
