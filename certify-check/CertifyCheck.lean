-- certify-check — the Rust→Lean extraction target (AGENT.md's "certify-check").
--
-- Hand-written Lean specs of the `certify-core` checkers live here alongside
-- the hax/Aeneas-lifted models; each checker's Lean spec IS the formalization
-- of its certificate definition (vv-guide §4).
--
-- §7 spike content:
--   * CertifyCheck.SignVariations — the sign-variation counter's spec + the
--     streaming-algorithm equivalence (core Lean, no Mathlib).
--   * CertifyCheck.SturmChecker    — the Sturm hypothesis-checker vs a cited
--     theorem (Mathlib `Polynomial`).
--   * CertifyCheck.LiftedAeneas    — the Aeneas-lifted `sign_variations` model.
--   * CertifyCheck.Refine          — proof that the lifted model computes the
--     mathematical `signVariations` (loop.spec_decr_nat + the `step` tactic).
--
-- Phase 5 (gated apply of the validated template — the gcd/reduce correctness
-- Lean owns per the gcd tool-fit decision):
--   * CertifyCheck.GcdReduce       — the fast-path `u128` Euclidean gcd loop
--     (CBMC-intractable) proven to compute `Nat.gcd`, same idiom as Refine.
--   * CertifyCheck.Reduce          — `SmallRat::reduce` proven to produce the
--     canonical reduced form of `num/den` (positive denominator, coprime,
--     equal rational), over the full `i128` range.
--   * CertifyCheck.Resultant       — `verify_common_factor` proven sound (a
--     verified common factor ⟹ `¬IsCoprime f g` ⟹ `resultant f g = 0`), with
--     NO cited axiom (Mathlib's `resultant_eq_zero_iff` closes the §7 gap).

import CertifyCheck.SignVariations
import CertifyCheck.SturmChecker
import CertifyCheck.LiftedAeneas
import CertifyCheck.Refine
import CertifyCheck.GcdReduce
import CertifyCheck.Reduce
import CertifyCheck.Resultant
import CertifyCheck.CapOut
