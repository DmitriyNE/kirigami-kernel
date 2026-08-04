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
# check_pair <template> <provided> <label>: <provided> must be all faithful `def`/`abbrev`
# models (no `axiom`), and every `axiom` the template externalises must have a matching model.
# A crate that emits no such template (a pure lift) is skipped. Temp files + `comm` (not
# `mapfile`) so it runs on macOS bash 3.2 too.
check_pair() {
  local template="$1" provided="$2" label="$3"
  [ -f "$template" ] || return 0
  [ -f "$provided" ] || { echo "error: $provided missing (its template $template exists)" >&2; exit 1; }

  # 1) No `axiom` in the hand-written model — an axiom here would silently enter the certified
  #    proofs' footprint without being `sorryAx`.
  if grep -qE '^[[:space:]]*axiom[[:space:]]' "$provided"; then
    echo "::error title=Externalisation axiom ($label)::$provided must contain only faithful \`def\`/\`abbrev\` models, no \`axiom\`." >&2
    grep -nE '^[[:space:]]*axiom[[:space:]]' "$provided" >&2
    exit 1
  fi

  # 2) Every externalised item (template `axiom`) has a model.
  local needed prov
  needed="$(mktemp)"; prov="$(mktemp)"
  awk '
    /^axiom[ \t]*$/ { getline; gsub(/^[ \t]+/, ""); print $1; next }
    /^axiom[ \t]+/  { sub(/^axiom[ \t]+/, ""); print $1; next }
  ' "$template" | sort -u > "$needed"
  # Models may live in the crate's own file OR the shared `CommonExtern.lean` it imports.
  grep -hoE '^(def|abbrev) [A-Za-z0-9_.]+' "$provided" "$ROOT/certify-check/CommonExtern.lean" \
    | awk '{print $2}' | sort -u > "$prov"

  local missing
  missing="$(comm -23 "$needed" "$prov")"
  if [ -n "$missing" ]; then
    echo "::error title=Missing faithful model ($label)::$template externalises items with no \`def\`/\`abbrev\` in $provided — add a faithful model (not an axiom):" >&2
    echo "$missing" | sed 's/^/  /' >&2
    rm -f "$needed" "$prov"; exit 1
  fi
  echo "externals-coverage OK ($label): $(wc -l < "$needed" | tr -d ' ') item(s) modelled in $(basename "$provided")."
  rm -f "$needed" "$prov"
}

E="$ROOT/certify-check/extract"
check_pair "$E/lattice.FunsExternal_Template.lean"       "$ROOT/certify-check/Lattice/FunsExternal.lean"      "lattice funs"
check_pair "$E/certify_core.FunsExternal_Template.lean"  "$ROOT/certify-check/CertifyCore/FunsExternal.lean"  "certify-core funs"
check_pair "$E/certify_core.TypesExternal_Template.lean" "$ROOT/certify-check/CertifyCore/TypesExternal.lean" "certify-core types"
