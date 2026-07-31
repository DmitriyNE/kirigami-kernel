# Fast-path arithmetic (gcd, reduce)

The `lattice` L0 fast-path `gcd`/`reduce` correctness that **Lean owns** per the
gcd tool-fit decision: Kani keeps the gcd-free bridge + panic-freedom; Lean proves
the parts that are CBMC-intractable / full-range. Both are Aeneas-lifted models
proven against a mathematical spec with the validated `loop.spec_decr_nat` + `step`
template (the same idiom as `Refine.lean`'s `sign_variations`).

## gcd

- **Checker** — `small::gcd_u128` · **Lean** — `certify-check/CertifyCheck/GcdReduce.lean`

### Statement
The `u128` Euclidean loop `gcd_u128 a b` computes `Nat.gcd a b`.

### Citation
None — proven directly (`Nat.gcd_rec`, `Nat.gcd_comm`).

### Checked at runtime
None. This *is* the correctness the loop needs: Kani owns the gcd-free bridge +
panic-freedom; Lean owns the 128-bit loop, which is CBMC-intractable.

### Lean
The Aeneas-lifted `gcd_u128` proven `= Nat.gcd` via `loop.spec_decr_nat`
(measure = `b`, invariant `gcd a b = gcd a₀ b₀`) + `step`.

- **Axioms** — `#print axioms gcd_u128_spec` = `[propext, Classical.choice, Quot.sound]`.

## reduce

- **Checker** — `small::SmallRat::reduce` · **Lean** — `certify-check/CertifyCheck/Reduce.lean`

### Statement
`reduce num den` returns the canonical reduced form of `num/den`.

### Citation
None — proven directly (`Nat.coprime_div_gcd_div_gcd`).

### Checked at runtime
None (Lean owns full-range reduce correctness).

### Lean
Over the full `i128` range, `reduce_spec` proves that whenever `reduce` returns
`some sr`:

    0 < sr.den  ∧  gcd(|sr.num|, sr.den) = 1  ∧  sr.num · den = num · sr.den

(positive denominator, coprime, equal rational). Depends on faithful hand-written
Std-gap models (`Lattice/FunsExternal.lean`: `unsigned_abs`, `TryFrom<u128> for
i128`, the `?`-operator glue) — the model's only TCB surface beyond
Aeneas/Lean/Mathlib (see `../spike-extraction-report.md` §"Phase 5").

- **Axioms** — `#print axioms reduce_spec` = `[propext, Classical.choice, Quot.sound]`.
