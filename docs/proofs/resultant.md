# Resultant ⇔ common root

- **Checker** — `resultant::verify_common_factor` (`crates/lattice/src/resultant.rs`)
- **Lean** — `certify-check/CertifyCheck/Resultant.lean` (Lean/Mathlib v4.31.0)

### Statement

`Res(f, g) = 0 ⇔ f, g` share a positive-degree factor over ℚ.

### Citation

BPR; Cox–Little–O'Shea. **Mathlib now proves it:**
`Polynomial.resultant_eq_zero_iff` — `resultant f g = 0 ↔ (f ≠ 0 ∨ g ≠ 0) ∧
¬IsCoprime f g`. The §7 spike flagged this as a Mathlib gap ("developing"); it has
since landed, so this obligation needs **no cited axiom** (unlike Sturm).

### Checked at runtime

The exhibited common factor `h` (deg ≥ 1) divides both `f` and `g` — the spec §5.3
"resultant-conditioned A-identity (divisibility check)". The resultant *value*
(numeric Euclidean over ℚ; bivariate Sylvester + fraction-free Bareiss) is
separately cross-checked differentially against the independent `Poly::gcd`
(`Res = 0 ⇔ deg gcd ≥ 1`).

### Structural

Leading-coefficient nonvanishing (degrees preserved). Honest side condition
`f ≠ 0 ∨ g ≠ 0`: the checker does not guard the degenerate `f = g = 0` (every `h`
"divides" `0`), and there `resultant 0 0 ≠ 0`; in the geometric use (curve
equations / derivatives) `f`, `g` are nonzero.

### Lean

The checked conditions are `IsCommonFactorWitness f g h`
(`1 ≤ h.natDegree ∧ h ∣ f ∧ h ∣ g`) over `ℚ[X]`.

- `verify_common_factor_not_coprime` — a verified witness ⟹ `¬IsCoprime f g`
  (direct: a positive-degree common divisor is not a unit).
- `verify_common_factor_sound` — ⟹ `resultant f g = 0`, via
  `resultant_eq_zero_iff`.

- **Axioms** — `#print axioms verify_common_factor_sound` =
  `[propext, Classical.choice, Quot.sound]` (no cited axiom).
