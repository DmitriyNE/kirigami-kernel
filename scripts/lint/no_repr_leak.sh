#!/usr/bin/env bash
# Algebra encapsulation (docs/algebra-trust.md): the two-tier Fast(i128)/Slow(bignum)
# representation of lattice::Int / lattice::Rat MUST stay private to lattice::rat. Every
# consumer uses only the arithmetic API (add/sub/mul/div/cmp/sign/…), never a match on
# Fast/Slow — because any code that observes the representation cannot be lifted to the
# Int=ℤ / Rat=ℚ model soundly. lattice::rat (the arithmetic) and lattice::proof (its Kani
# fast≡slow equivalence proofs) are the only places allowed to name the variants.
set -eu
root="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
hits="$(grep -rnE --include='*.rs' '\b(Int|Rat)::(Fast|Slow)\b' "$root/crates" 2>/dev/null \
  | grep -vE '/lattice/src/(rat|proof)\.rs:' || true)"
if [ -n "$hits" ]; then
  printf 'no-repr-leak: FAIL — the Int/Rat Fast/Slow representation leaked outside lattice::rat:\n%s\n' "$hits"
  exit 1
fi
echo "no-repr-leak: OK (Int/Rat Fast/Slow confined to lattice::rat)."
