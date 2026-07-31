#!/usr/bin/env bash
#
# Guard the extraction TCB surface: every `core`-library item Aeneas externalises
# (i.e. cannot model, and emits as a hole in FunsExternal_Template.lean) must have
# a FAITHFUL hand-written model — a `def`, never an `axiom` — in
# `certify-check/Lattice/FunsExternal.lean`.
#
# Why: an `axiom` in FunsExternal.lean would compile and silently enter the
# certified theorems' axiom footprint WITHOUT being `sorryAx` (so the old
# grep-for-sorryAx audit would miss it).  This check + the whitelist axiom audit
# in CI together ensure no externalised `core` fn becomes a silent trust hole.
#
# Runs on the committed `certify-check/extract/FunsExternal_Template.lean`, which
# `scripts/extract.sh` regenerates; the drift workflow keeps it in sync with the
# Rust.  Pure bash + grep/awk — no toolchain needed.
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null \
  || { cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd; })"
TEMPLATE="$ROOT/certify-check/extract/FunsExternal_Template.lean"
PROVIDED="$ROOT/certify-check/Lattice/FunsExternal.lean"

[ -f "$TEMPLATE" ] || { echo "error: $TEMPLATE missing — run scripts/extract.sh first" >&2; exit 1; }
[ -f "$PROVIDED" ] || { echo "error: $PROVIDED missing" >&2; exit 1; }

# 1) FunsExternal.lean must be all faithful `def`s — no `axiom`.
if grep -qE '^[[:space:]]*axiom[[:space:]]' "$PROVIDED"; then
  echo "::error title=Externalisation axiom in FunsExternal.lean::FunsExternal.lean must contain only faithful \`def\` models, no \`axiom\` (an axiom here silently enters the certified proofs' footprint)." >&2
  grep -nE '^[[:space:]]*axiom[[:space:]]' "$PROVIDED" >&2
  exit 1
fi

# 2) Every core item Aeneas externalised (template `axiom`s) has a model.
#    (Uses temp files + `comm`, not `mapfile`, so it runs on macOS bash 3.2 too.)
needed="$(mktemp)"; provided="$(mktemp)"
trap 'rm -f "$needed" "$provided"' EXIT

awk '
  /^axiom[ \t]*$/ { getline; gsub(/^[ \t]+/, ""); print $1; next }
  /^axiom[ \t]+/  { sub(/^axiom[ \t]+/, ""); print $1; next }
' "$TEMPLATE" | sort -u > "$needed"

grep -oE '^def [A-Za-z0-9_.]+' "$PROVIDED" | awk '{print $2}' | sort -u > "$provided"

missing="$(comm -23 "$needed" "$provided")"
if [ -n "$missing" ]; then
  echo "::error title=Missing faithful model::Aeneas externalised these core items (certify-check/extract/FunsExternal_Template.lean) but Lattice/FunsExternal.lean has no \`def\` for them — add a faithful model (not an axiom):" >&2
  echo "$missing" | sed 's/^/  /' >&2
  exit 1
fi

echo "externals-coverage OK: all $(wc -l < "$needed" | tr -d ' ') externalised core items have faithful models in FunsExternal.lean."
