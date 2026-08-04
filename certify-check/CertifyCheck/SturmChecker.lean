/-
  Sturm hypothesis-checker — the §7 spike's runtime-checked-hypothesis deliverable.

  `crates/lattice/src/sturm.rs::SturmChain::verify_chain` is a *certifying* check: it
  does not re-prove Sturm's theorem, it checks (by exact ℚ arithmetic) that a given
  polynomial sequence satisfies the *chain identities* that make the variation
  theorem apply, and then cites the theorem (`vv-guide §0`, runtime-checked
  hypotheses; `docs/proofs/ledger.md`).

  This file is that citation, formalized:

  * `IsSturmChainData p cs` — the exact conditions `verify_chain` checks.
  * `variationAt` — the sign-variation count `V(cs, x)`, wired to the very
    `signVariations` proven in `CertifyCheck.SignVariations` (so the counter the
    Rust uses and the counter the theorem is stated over are the same object).
  * `sturm_root_count` — **Sturm's theorem, cited as an axiom.** Mathlib has
    Descartes' rule of signs (`Polynomial.signVariations`, `RuleOfSigns`) but *not*
    Sturm's theorem; the complete formalization is Isabelle/AFP (Eberl). We record
    that gap (report §2, ledger) and cite BPR *Algorithms in Real Algebraic
    Geometry* Thm 2.50 / Eberl. The runtime checker discharges its hypotheses.
  * `verify_chain_sound` — the interface corollary the kernel relies on.
-/
import Mathlib
import CertifyCheck.SignVariations

namespace CertifyCheck

open Polynomial

/-- Sign of a rational as an `Int` in `{-1, 0, 1}` — matches `Rat::sign` in the
    lattice and feeds the shared `signVariations`. -/
def signInt (r : ℚ) : Int := if 0 < r then 1 else if r < 0 then -1 else 0

/-- `q` is a strictly-positive rational multiple of `r` (or both zero) — the
    PRS-agnostic relation `verify_chain`'s `pos_proportional` checks. -/
def PosMultiple (q r : Polynomial ℚ) : Prop := ∃ c : ℚ, 0 < c ∧ q = c • r

/-- **The conditions `verify_chain` checks**, formalized over `ℚ[X]`:
    `c₀ ∝₊ p`, `c₁ ∝₊ p'`, each `cₖ₊₂ ∝₊ -(cₖ % cₖ₊₁)`, and strictly descending
    degrees. (`ℚ[X]` is a Euclidean domain, so `%` is polynomial remainder.) This
    is the hand-written spec — writing it down *is* the formalization of the
    certificate, and is where spec ambiguities surface (`vv-guide §4`). -/
structure IsSturmChainData (p : Polynomial ℚ) (cs : List (Polynomial ℚ)) : Prop where
  nonempty   : cs ≠ []
  head       : PosMultiple cs[0]! p
  /-- A single-entry chain is only complete when `p` itself is a nonzero constant
      (`verify_chain`'s `c.len() == 1 ⇒ p.degree() == Some(0)`). Without this, `[p]`
      for a non-constant `p` passes vacuously yet has zero sign variations. -/
  single     : cs.length = 1 → p.degree = 0
  deriv      : 1 < cs.length → PosMultiple cs[1]! (derivative p)
  recurrence : ∀ i, i + 2 < cs.length → PosMultiple cs[i + 2]! (-(cs[i]! % cs[i + 1]!))
  descending : ∀ i, i + 1 < cs.length → (cs[i + 1]!).degree < (cs[i]!).degree
  /-- The chain is **complete**: the final division is exact, so the tail is
      `gcd(p, p')` (`verify_chain`'s `c[n-2] % c[n-1] == 0`). Without this, a truncated
      sequence passes every per-pair identity yet under-counts the roots. -/
  terminal   : 1 < cs.length → cs[cs.length - 2]! % cs[cs.length - 1]! = 0

/-- The number of sign variations `V(cs, x)` of the chain evaluated at `x`, using
    the *same* `signVariations` the Rust computes and we proved correct. -/
def variationAt (cs : List (Polynomial ℚ)) (x : ℚ) : Nat :=
  signVariations (cs.map (fun q => signInt (q.eval x)))

/-- The count of **distinct real roots** of `p` in the half-open `(a, b]`
    (base-change to `ℝ`; `toFinset` makes it distinct). What Sturm's theorem
    counts. -/
noncomputable def realRootsIn (p : Polynomial ℚ) (a b : ℚ) : Nat :=
  ((p.map (algebraMap ℚ ℝ)).roots.toFinset.filter
      (fun x => (a : ℝ) < x ∧ x ≤ (b : ℝ))).card

/--
**Sturm's theorem — CITED, not proven here.**

For a squarefree `p` whose sequence `cs` satisfies the Sturm chain identities, the
number of distinct real roots in `(a, b]` is `V(cs, a) − V(cs, b)`.

Mathlib (as of Lean v4.31.0 / Mathlib v4.31.0) does **not** contain Sturm's theorem
— only Descartes' rule of signs (`Mathlib.Algebra.Polynomial.RuleOfSigns`). The
complete formalization exists in Isabelle/AFP (Eberl, `Sturm_Sequences`). Per the
runtime-checked-hypothesis design (`vv-guide §0`), we cite the theorem
(BPR *Algorithms in Real Algebraic Geometry*, Thm 2.50; Eberl) and prove the
*reduction*: that `verify_chain` checks exactly `IsSturmChainData`, and that its
`V` is the `signVariations` proved in `CertifyCheck.SignVariations`. This axiom is
the single, labelled soundness assumption; it is recorded in `docs/proofs/ledger.md`.
-/
axiom sturm_root_count
    (p : Polynomial ℚ) (cs : List (Polynomial ℚ))
    (hchain : IsSturmChainData p cs) (hsqf : Squarefree p)
    {a b : ℚ} (hab : a < b) :
    realRootsIn p a b = variationAt cs a - variationAt cs b

/-- **Interface corollary** the kernel relies on: once the runtime check establishes
    `IsSturmChainData` (and `p` is squarefree), the root count in `(a, b]` is exactly
    the difference of the chain's sign-variation counts. This is what makes
    `verify_chain` a sound certificate. -/
theorem verify_chain_sound
    (p : Polynomial ℚ) (cs : List (Polynomial ℚ))
    (hchain : IsSturmChainData p cs) (hsqf : Squarefree p)
    {a b : ℚ} (hab : a < b) :
    realRootsIn p a b = variationAt cs a - variationAt cs b :=
  sturm_root_count p cs hchain hsqf hab

/-- **Regression witness for the completeness fix.** A bare single-element "chain" of a
    non-constant polynomial is rejected — the `single` field forces `p` constant. Without
    it, `IsSturmChainData (X²−1) [X²−1]` held vacuously; since a one-element chain has zero
    sign variations, `sturm_root_count` then claimed `X²−1` has no roots in any interval,
    an inconsistent axiom. (`verify_chain` always checked this — Rust `c.len()==1 ⇒
    p.degree()==Some(0)` — so only the Lean spec had drifted weaker.) -/
theorem not_isSturmChainData_singleton_nonconstant :
    ¬ IsSturmChainData (X ^ 2 - 1 : Polynomial ℚ) [X ^ 2 - 1] := by
  intro h
  have hdeg : (X ^ 2 - 1 : Polynomial ℚ).degree = 0 := h.single rfl
  rw [show (1 : Polynomial ℚ) = C 1 from (map_one C).symm,
      degree_X_pow_sub_C (by norm_num) (1 : ℚ)] at hdeg
  exact absurd hdeg (by decide)

end CertifyCheck
