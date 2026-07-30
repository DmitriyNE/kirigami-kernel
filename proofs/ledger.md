# Proof ledger (stub)

One entry per theorem the checkers rely on. Fill as `certify-core` grows.

Format: **theorem · statement · citation (Mathlib lemma or literature) · hypotheses checked at runtime · hypotheses structural · Lean status**

- **Sturm variation** · #distinct real roots in (a,b] = V(a)−V(b) for a Sturm chain of `p` · [BPR "Algorithms in Real Algebraic Geometry" Thm 2.50; Mathlib `Polynomial.sturm_*` where present] · **runtime** (`sturm::SturmChain::verify_chain`, `crates/lattice/src/sturm.rs`): p₀ ∝₊ p, p₁ ∝₊ p′, each pₖ₊₁ a positive-rational multiple of −(pₖ₋₁ mod pₖ) (fraction-free positive-proportionality `lead(u)·v = lead(v)·u` + matching-sign leads), strictly descending degrees, terminating (pₙ₋₂ mod pₙ₋₁ = 0 ⇒ the tail is gcd(p,p′)) — PRS-agnostic · **structural**: exact ℚ arithmetic only (distinct-root counting needs no squarefree hypothesis; the chain ends at gcd(p,p′)) · **Lean**: TODO(spike)
- Resultant ⇔ common root · Res(f,g)=0 ⇔ deg-preserving common factor over the field · [Mathlib: `Polynomial.resultant`, developing] · Sylvester matrix identity on the instance · leading-coeff nonvanishing · TODO
- Sylvester criterion (strict) · all leading principal minors > 0 ⇔ SPD · [Mathlib: `Matrix.posDef_iff_*`] · the evaluated minors are the stated minors · symmetry of the form · TODO
- (add as needed)
