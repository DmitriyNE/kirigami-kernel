/-
  Resultant common-factor checker — the §7 runtime-checked-hypothesis deliverable
  for `resultant::verify_common_factor` (`crates/lattice/src/resultant.rs`).

  `verify_common_factor(f, g, h)` is a *certifying* check:
      `h.degree() ≥ 1  ∧  f.rem(h).is_zero()  ∧  g.rem(h).is_zero()`
  — it does not recompute the resultant, it *exhibits* a common factor `h` and
  checks (by exact ℚ arithmetic) that `h` has positive degree and divides both
  `f` and `g` (`vv-guide §0`, runtime-checked hypotheses; `docs/proofs/ledger.md`).

  Unlike the Sturm checker, the deep theorem this relies on —
  `Res(f, g) = 0 ⇔ f, g share a positive-degree factor` — is **now in Mathlib**
  (`Polynomial.resultant_eq_zero_iff`, over a field). So this checker's soundness
  is proven **outright, with no cited axiom** (footprint = `[propext,
  Classical.choice, Quot.sound]`). This is the resultant analogue of
  `SturmChecker.lean`, but stronger: the citation gap is closed.
-/
import Mathlib

namespace CertifyCheck

open Polynomial

/-- **The conditions `verify_common_factor` checks**, formalized over `ℚ[X]`: the
    exhibited `h` has degree ≥ 1 and divides both `f` and `g`
    (`f.rem(h).is_zero()` ⟺ `h ∣ f` in the Euclidean domain `ℚ[X]`). Writing this
    down *is* the formalization of the certificate (`vv-guide §4`). -/
structure IsCommonFactorWitness (f g h : Polynomial ℚ) : Prop where
  deg   : 1 ≤ h.natDegree
  dvd_f : h ∣ f
  dvd_g : h ∣ g

/-- **Soundness (direct).** A verified witness means `f` and `g` are not coprime —
    they genuinely share a positive-degree factor. (A common divisor of positive
    degree is not a unit, so `f`, `g` cannot be coprime.) -/
theorem verify_common_factor_not_coprime (f g h : Polynomial ℚ)
    (H : IsCommonFactorWitness f g h) : ¬ IsCoprime f g := by
  intro hcop
  have hu : IsUnit h := hcop.isUnit_of_dvd' H.dvd_f H.dvd_g
  have hd : h.natDegree = 0 := natDegree_eq_zero_of_isUnit hu
  have := H.deg
  omega

/-- **Interface corollary** the kernel relies on: a verified common factor
    certifies `resultant f g = 0` — via Mathlib's `resultant_eq_zero_iff`, no
    axiom. The `f ≠ 0 ∨ g ≠ 0` side condition is honest and required:
    `verify_common_factor` does *not* guard the degenerate `f = g = 0` (every `h`
    "divides" `0`), and there `resultant 0 0 ≠ 0`; in the geometric use (curve
    equations / derivatives) `f`, `g` are nonzero. -/
theorem verify_common_factor_sound (f g h : Polynomial ℚ)
    (H : IsCommonFactorWitness f g h) (hfg : f ≠ 0 ∨ g ≠ 0) :
    resultant f g = 0 := by
  rw [resultant_eq_zero_iff]
  exact ⟨hfg, verify_common_factor_not_coprime f g h H⟩

end CertifyCheck
