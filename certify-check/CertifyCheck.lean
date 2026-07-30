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

import CertifyCheck.SignVariations
import CertifyCheck.SturmChecker
import CertifyCheck.LiftedAeneas
