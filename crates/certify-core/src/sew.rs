//! Pure sewing checkers.
//!
//! The EDGE-OCCUPANCY four-bit `(A_L, A_R, B_L, B_R)` + frame-bit → row
//! classifier (Kani-exhaustive, ≤ 6 bits), the quadrant test (one cyclic
//! interval / all four / none; opposite quadrants ⇒ pinch, reject), the
//! mode-indexed identity dispatch (PAIR-IDENTICAL / OUTPUT-SOURCE-IDENTICAL),
//! EDGE-EMB / EDGE-EDGE verdict logic, and SEW-LINK comparison over V_∂.
//! Implemented at M5. The sewing construction lives in the `sew` crate.
//!
//! **`ε_φ` and EDGE-OCCUPANCY are minted upstream, at M4.** The order sign of the
//! monotone correspondence — the order sign, never the derivative sign — is minted by
//! [`crate::miter::eps_phi`] as part of MITER-EDGE-LEDGER, and the four-bit occupancy is
//! materialized there as [`crate::miter::Occupancy`]. SEW **consumes** both (it does not
//! re-mint them): the spec lists `ε_φ` under both the M4 ledger row and this M5 sewing row,
//! and the tiebreaker is ownership — M4 mints, M5 reads.
//!
//! # Two axes of the edge layer
//!
//! SEW-EDGES decides two independent things per edge. The **quadrant test** — one cyclic
//! interval / all four / none, with opposite quadrants a rejected pinch — is the four
//! occupancy bits arranged in cyclic order and fed to the *reused*, already-Kani-proven
//! [`crate::arrange::classify_link`]; its outcome is a [`crate::arrange::LinkClass`], so this
//! module mints no parallel row enum. The **identity obligation** is dispatched by how many
//! flanks change material across the edge (the boundary count) — [`IdentityMode`].

/// Which identity obligation SEW-EDGES imposes on an edge, dispatched by its
/// [`Occupancy`](crate::miter::Occupancy) boundary count (spec §8.5 line 385: "identity
/// obligations dispatched by occupancy"). This selects *which* equality the sewing checker
/// must discharge; the discharge itself lands with the [`crate::miter`]/`arrange2d`
/// provenance at M5.2.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IdentityMode {
    /// **Two boundaries** (`A_L ≠ A_R ∧ B_L ≠ B_R` —
    /// [`is_boundary_boundary`](crate::miter::Occupancy::is_boundary_boundary)):
    /// **PAIR-IDENTICAL** — point-set identity of the two paired edges + `ε_φ` (the clean
    /// miter's whole domain lives here). Mints D24-STAGE2-EQUALITY / MITER-BRANCH-IDENTITY.
    PairIdentical,
    /// **One boundary** (material changes across the edge on exactly one flank):
    /// **OUTPUT-SOURCE-IDENTICAL** — same carrier ∧ interval *containment* (the arrangement
    /// legitimately splits a source) ∧ `ε` vs the source half-edge sense. Mints
    /// ARRANGEMENT-PROVENANCE, a re-verification of the stored back-reference.
    OutputSourceIdentical,
    /// **Zero boundaries** (no material change on either flank): provenance + the
    /// zero-output assertions, **no edge-pair identity** (demanding one is uninhabitable —
    /// the ledge's default case, before topology enters).
    Provenance,
}
