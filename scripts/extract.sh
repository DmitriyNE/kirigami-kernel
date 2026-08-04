#!/usr/bin/env bash
#
# Regenerate the Aeneas-lifted Lean models of the checker crates from Rust source.
#
# This is the automated "crossing" from the Rust shore to the deductive shore:
# charon (rustc → LLBC) + aeneas (LLBC → pure Lean).  The generated
# `certify-check/{Lattice,CertifyCore}/{Funs,Types}.lean` are COMMITTED ARTIFACTS WE
# PROVE ABOUT; CI's `extraction-drift` workflow regenerates and `git diff`s them, so a
# model that has drifted from the Rust (or from a bumped Charon/Aeneas pin) fails the
# build.  Output is byte-deterministic at our pins, so the drift check is a plain
# `git diff --exit-code` (no normaliser).
#
# `*/FunsExternal.lean` is NOT regenerated — it is the hand-written, trusted model of
# the `core`-library items Aeneas's Std lib does not cover (the lift's only hand-written
# TCB surface).  We DO refresh the generated `FunsExternal_Template.lean` (into
# `certify-check/extract/`, un-built) so `scripts/check-externals.sh` can verify every
# externalised item still has a faithful model.  A crate whose lift needs no externals
# (e.g. certify-core's pure slice checkers) emits no template — that is fine.
#
# Surfaces (crate → committed model):
#   crates/lattice      → certify-check/Lattice/       (sturm::sign_variations, small::*)
#   crates/certify-core → certify-check/CertifyCore/   (arrange:: link checkers, slice 3e)
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

EXTRACT_DIR="$ROOT/certify-check/extract"

# extract_crate <crate_dir> <manifest> <lean_dir> <llbc_stem>
#   <llbc_stem> is charon's output basename (the crate name with `-`→`_`).
extract_crate() {
  local crate_dir="$1" manifest="$2" lean_dir="$3" llbc_stem="$4"
  local work
  work="$(mktemp -d)"

  # Build the `--start-from` argument list from the manifest (skip blanks/comments).
  local start_args=()
  local line
  while IFS= read -r line; do
    line="${line%%#*}"
    line="$(echo "$line" | xargs || true)"
    [ -z "$line" ] && continue
    start_args+=(--start-from "$line")
  done < "$manifest"
  [ "${#start_args[@]}" -gt 0 ] || { echo "error: no start-from entries in $manifest" >&2; exit 1; }

  # Optional `<crate>.opaque` sibling manifest: charon `--opaque` name-matcher patterns held
  # opaque (their bodies not lifted) and bound to hand-written models in {Types,Funs}External.
  # certify-core holds `lattice::rat`/`backend` opaque → bound to Mathlib ℤ/ℚ (algebra-rehaul).
  local opaque_args=()
  local opaque_manifest="${manifest%.startfrom}.opaque"
  if [ -f "$opaque_manifest" ]; then
    while IFS= read -r line; do
      line="${line%%#*}"
      line="$(echo "$line" | xargs || true)"
      [ -z "$line" ] && continue
      opaque_args+=(--opaque "$line")
    done < "$opaque_manifest"
  fi

  echo "extract: charon ($crate_dir, $(( ${#start_args[@]} / 2 )) start-from, $(( ${#opaque_args[@]} / 2 )) opaque) …"
  ( cd "$crate_dir" && charon cargo --preset=aeneas "${start_args[@]}" "${opaque_args[@]}" --dest "$work" )

  echo "extract: aeneas → Lean ($llbc_stem) …"
  aeneas -backend lean "$work/$llbc_stem.llbc" -dest "$work" -split-files

  # Generated model (proven about) → <lean_dir>/. NOT {Funs,Types}External.lean (hand-written).
  cp "$work/Funs.lean"  "$lean_dir/Funs.lean"
  cp "$work/Types.lean" "$lean_dir/Types.lean"
  # Generated externals templates (never built) → extract/, PER-CRATE prefixed by the llbc
  # stem, for the coverage check (check-externals.sh). A crate may emit a Funs template
  # (core-library holes) and/or a Types template (opaque types — e.g. certify-core holds
  # `lattice::rat` opaque and binds it to ℚ, algebra-rehaul). A pure crate emits neither; a
  # stale prefixed template is removed so the committed set stays exactly what the lift needs.
  local wrote=""
  local kind tmpl dest
  for kind in Funs Types; do
    tmpl="$work/${kind}External_Template.lean"
    dest="$EXTRACT_DIR/${llbc_stem}.${kind}External_Template.lean"
    if [ -f "$tmpl" ]; then
      cp "$tmpl" "$dest"; wrote="$wrote ${llbc_stem}.${kind}External_Template.lean"
    else
      rm -f "$dest"
    fi
  done
  echo "extract: wrote $lean_dir/{Funs,Types}.lean${wrote:+ + extract/$wrote}"

  rm -rf "$work"
}

extract_crate "$ROOT/crates/lattice" \
  "$EXTRACT_DIR/lattice.startfrom" "$ROOT/certify-check/Lattice" "lattice"

extract_crate "$ROOT/crates/certify-core" \
  "$EXTRACT_DIR/certify-core.startfrom" "$ROOT/certify-check/CertifyCore" "certify_core"
