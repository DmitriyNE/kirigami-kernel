//! Pure 1D certificate checkers (absorbs the former `certify1d` crate).
//!
//! CLIP ladder — CLIP-W → CLIP-μ → common-zero isolation → CLIP-a | **CLIP-σ
//! signed disjunction** (soundness-critical; a four-corner |·| test is unsound)
//! | reject; REG-Q / REG-V / SLAB determinant forms; corner min/max evaluators
//! (declared min-or-max per the convexity rider); EDGE-REG three-way verdict
//! `{pass | fail | stall→pending}`; REPARAM as a pure `old-record → new-record`
//! function (never a truth-predicate). Implemented at M2.
