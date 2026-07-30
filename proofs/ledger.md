# Proof ledger (stub)

One entry per theorem the checkers rely on. Fill as `certify-core` grows.

Format: **theorem · statement · citation (Mathlib lemma or literature) · hypotheses checked at runtime · hypotheses structural · Lean status**

- Sturm variation · #real roots in (a,b] = V(a)−V(b) for a Sturm chain · [Mathlib: `Polynomial.sturm_*` if present, else BPR "Algorithms in Real Algebraic Geometry"] · chain identities p_{i+1}=−(p_{i−1} mod p_i) verified by exact division on the instance · squarefree-ness / gcd structure · TODO(spike)
- Resultant ⇔ common root · Res(f,g)=0 ⇔ deg-preserving common factor over the field · [Mathlib: `Polynomial.resultant`, developing] · Sylvester matrix identity on the instance · leading-coeff nonvanishing · TODO
- Sylvester criterion (strict) · all leading principal minors > 0 ⇔ SPD · [Mathlib: `Matrix.posDef_iff_*`] · the evaluated minors are the stated minors · symmetry of the form · TODO
- (add as needed)
