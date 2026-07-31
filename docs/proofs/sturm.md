# Sturm variation

- **Checker** — `sturm::SturmChain::verify_chain` (`crates/lattice/src/sturm.rs`)
- **Lean** — `certify-check/CertifyCheck/SturmChecker.lean` (Lean/Mathlib v4.31.0)

### Statement

The number of **distinct real roots** of `p` in the half-open interval `(a, b]`
equals `V(a) − V(b)`, where `V(x)` is the sign-variation count of a Sturm chain of
`p` evaluated at `x`.

### Citation

BPR, *Algorithms in Real Algebraic Geometry*, Thm 2.50; Eberl, Isabelle/AFP
`Sturm_Sequences`. Mathlib (v4.31.0) has Descartes' rule of signs
(`Polynomial.signVariations` / `RuleOfSigns`) but **not** Sturm's theorem — this is
the coverage gap the §7 spike recorded.

### Checked at runtime

The chain identities, by exact ℚ arithmetic on the given sequence `cs`:

- `c₀ ∝₊ p`, `c₁ ∝₊ p′`;
- each `cₖ₊₁` a positive-rational multiple of `−(cₖ₋₁ mod cₖ)` — fraction-free
  positive-proportionality `lead(u)·v = lead(v)·u` + matching-sign leads;
- strictly descending degrees;
- terminating: `cₙ₋₂ mod cₙ₋₁ = 0 ⇒` the tail is `gcd(p, p′)`.

PRS-agnostic (checks positive-multiple, not exact equality — so any polynomial
remainder sequence passes).

### Structural

Exact ℚ arithmetic only. Distinct-root counting needs no squarefree hypothesis
(the chain ends at `gcd(p, p′)`).

### Lean

The checked conditions are formalized as `IsSturmChainData` over `ℚ[X]`; the
variation count `variationAt` reuses `signVariations` (proven correct in
`SignVariations.lean`, axiom-clean), so the counter the Rust computes and the one
the theorem is stated over are the same object. The theorem itself is the **single
cited axiom** `sturm_root_count`; `verify_chain_sound` is the interface corollary.

- **Axioms** — `#print axioms verify_chain_sound` =
  `[propext, sturm_root_count, Classical.choice, Quot.sound]`.
