#!/usr/bin/env bash
# Invariant 1: no floats in certified paths. The pure tier (lattice,
# certify-core) must contain no f32/f64 — a float that reaches a predicate is a
# bug. The arrange2d searcher's predicate path (all of its src except the
# test-only `testgen.rs` generators) is held to the same rule. Floats are
# permitted only in `export` behind the `diagnostics` feature (plots/viewers).
# This is the mechanical guard; widen the scan as the shell crates gain certified
# functions.
set -eu
root="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
hits="$(grep -rnE --exclude=testgen.rs '\bf(32|64)\b' \
  "$root/crates/lattice/src" \
  "$root/crates/certify-core/src" \
  "$root/crates/arrange2d/src" 2>/dev/null || true)"
if [ -n "$hits" ]; then
  printf 'no-float-certified: FAIL — floats in a certified path (invariant 1):\n%s\n' "$hits"
  exit 1
fi
echo "no-float-certified: OK (lattice, certify-core, arrange2d predicate path)."
