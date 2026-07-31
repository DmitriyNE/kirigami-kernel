#!/usr/bin/env bash
#
# Regenerate the Aeneas-lifted Lean model of `crates/lattice` from the Rust source.
#
# This is the automated "crossing" from the Rust shore to the deductive shore:
# charon (rustc → LLBC) + aeneas (LLBC → pure Lean).  The generated
# `certify-check/Lattice/{Funs,Types}.lean` are COMMITTED ARTIFACTS WE PROVE
# ABOUT; CI's `extraction-drift` workflow regenerates and `git diff`s them, so a
# model that has drifted from the Rust (or from a bumped Charon/Aeneas pin) fails
# the build.  Output is byte-deterministic at our pins, so the drift check is a
# plain `git diff --exit-code` (no normaliser).
#
# `Lattice/FunsExternal.lean` is NOT regenerated — it is the hand-written,
# trusted model of the `core`-library items Aeneas's Std lib does not cover (the
# lift's only hand-written TCB surface).  We DO refresh the generated
# `FunsExternal_Template.lean` (into `certify-check/extract/`, un-built) so
# `scripts/check-externals.sh` can verify every externalised item still has a
# faithful model.
#
# Run via `nix run .#extract`, or `scripts/extract.sh` inside
# `nix develop .#extraction` (needs `charon` + `aeneas` on PATH).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel 2>/dev/null || echo "$SCRIPT_DIR/..")"
ROOT="$(cd "$ROOT" && pwd)"
cd "$ROOT"

# darwin: lake/git-in-lake (and, defensively, charon/aeneas) break under the
# C-toolchain's DEVELOPER_DIR/SDKROOT — see docs/spike-extraction-report.md §3.
unset DEVELOPER_DIR SDKROOT 2>/dev/null || true

command -v charon >/dev/null || { echo "error: charon not on PATH — run inside 'nix develop .#extraction' or via 'nix run .#extract'" >&2; exit 127; }
command -v aeneas >/dev/null || { echo "error: aeneas not on PATH — run inside 'nix develop .#extraction' or via 'nix run .#extract'" >&2; exit 127; }

MANIFEST="$ROOT/certify-check/extract/lattice.startfrom"
CRATE_DIR="$ROOT/crates/lattice"
LEAN_DIR="$ROOT/certify-check/Lattice"
EXTRACT_DIR="$ROOT/certify-check/extract"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# Build the `--start-from` argument list from the manifest (skip blanks/comments).
START_ARGS=()
while IFS= read -r line; do
  line="${line%%#*}"
  line="$(echo "$line" | xargs || true)"
  [ -z "$line" ] && continue
  START_ARGS+=(--start-from "$line")
done < "$MANIFEST"
[ "${#START_ARGS[@]}" -gt 0 ] || { echo "error: no start-from entries in $MANIFEST" >&2; exit 1; }

echo "extract: charon (crates/lattice, $(( ${#START_ARGS[@]} / 2 )) start-from entries) …"
( cd "$CRATE_DIR" && charon cargo --preset=aeneas "${START_ARGS[@]}" --dest "$WORK" )

echo "extract: aeneas → Lean …"
aeneas -backend lean "$WORK/lattice.llbc" -dest "$WORK" -split-files

# Generated model (proven about) → Lattice/. NOT FunsExternal.lean (hand-written).
cp "$WORK/Funs.lean"  "$LEAN_DIR/Funs.lean"
cp "$WORK/Types.lean" "$LEAN_DIR/Types.lean"
# Generated externals template (never built) → extract/, for the coverage check.
cp "$WORK/FunsExternal_Template.lean" "$EXTRACT_DIR/FunsExternal_Template.lean"

echo "extract: wrote $LEAN_DIR/{Funs,Types}.lean + $EXTRACT_DIR/FunsExternal_Template.lean"
