#!/usr/bin/env bash
# Invariant 1: no floats in certified paths. The pure tier (lattice,
# certify-core) must contain no f32/f64 — a float that reaches a predicate is a
# bug. Floats are permitted only in `export` behind the `diagnostics` feature
# (plots/viewers). This is the mechanical guard; widen the scoped scan into the
# shell crates' certified functions as they gain them.
set -eu
root="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
hits="$(grep -rnE '\bf(32|64)\b' \
  "$root/crates/lattice/src" \
  "$root/crates/certify-core/src" 2>/dev/null || true)"
if [ -n "$hits" ]; then
  printf 'no-float-certified: FAIL — floats in the pure tier (invariant 1):\n%s\n' "$hits"
  exit 1
fi
echo "no-float-certified: OK (pure tier: lattice, certify-core)."
