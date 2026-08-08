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

use crate::arrange::{LinkClass, classify_link};
use crate::miter::Occupancy;

/// Which identity obligation SEW-EDGES imposes on an edge, dispatched by its
/// [`Occupancy`] boundary count (spec §8.5 line 385: "identity
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

/// The four occupancy bits in **cyclic quadrant order** `[A_L, B_L, A_R, B_R]`.
///
/// The order is forced by the manifold constraint: the canonical clean miter occupies
/// `{A_L, B_R}` and must classify as [`LinkClass::Boundary`] (it is the paired shell edge),
/// as must its L/R mirror `{A_R, B_L}`. That pins an **alternating-flank** cycle — same-flank
/// opposite sides sit diagonally, so `{A_L, A_R}` (a genuine cross-flank pinch) lands in
/// opposite quadrants and rejects, while `{A_L, B_R}` occupies adjacent quadrants and passes.
/// (Interleaving `B_L`/`B_R` within the alternation is free: positions 1 and 3 share the same
/// neighbour set `{0, 2}`, so both interleavings classify identically for every input.)
/// Reading `frame` here is unnecessary — an L↔R frame flip reverses the cycle, preserving the
/// cyclic run count and hence the class, so the row is frame-invariant by construction.
fn quadrant_mask(occ: Occupancy) -> [bool; 4] {
    [occ.a_l, occ.b_l, occ.a_r, occ.b_r]
}

/// The SEW-EDGES **quadrant test** for one edge: its [`Occupancy`] → one cyclic occupied
/// interval ([`LinkClass::Boundary`]) / all four ([`LinkClass::Interior`]) / none
/// ([`LinkClass::Exterior`]) / two opposite quadrants ([`LinkClass::Pinch`], which SEW rejects).
///
/// This mints no new decision procedure: the four bits in cyclic quadrant order
/// (in cyclic quadrant order) are fed to the already-Kani-proven [`classify_link`]. Soundness — that
/// this reproduces the independent boundary-count reference for every one of the sixteen bit
/// patterns — is the ★ (`occupancy_row_sound` in `proof.rs`).
///
/// ```
/// use certify_core::miter::Occupancy;
/// use certify_core::sew::occupancy_row;
/// use certify_core::arrange::LinkClass;
///
/// // Canonical clean miter: {A_L, B_R} occupied — a paired shell edge.
/// let clean = Occupancy { a_l: true, a_r: false, b_l: false, b_r: true, frame: false };
/// assert_eq!(occupancy_row(clean), LinkClass::Boundary);
///
/// // Both sides of one flank occupied, neither of the other: an opposite-quadrant pinch.
/// let pinch = Occupancy { a_l: true, a_r: true, b_l: false, b_r: false, frame: false };
/// assert_eq!(occupancy_row(pinch), LinkClass::Pinch);
/// ```
pub fn occupancy_row(occ: Occupancy) -> LinkClass {
    classify_link(&quadrant_mask(occ))
}

/// The SEW-EDGES **identity dispatch**: which equality the sewing checker must discharge for
/// this edge, keyed by its [`Occupancy`] boundary count (how many flanks change material across
/// it). See [`IdentityMode`] for what each arm obligates.
///
/// ```
/// use certify_core::miter::Occupancy;
/// use certify_core::sew::{identity_mode, IdentityMode};
///
/// // Two boundaries (both flanks flip): PAIR-IDENTICAL — the clean miter's whole domain.
/// let clean = Occupancy { a_l: true, a_r: false, b_l: false, b_r: true, frame: false };
/// assert_eq!(identity_mode(clean), IdentityMode::PairIdentical);
///
/// // No boundary (neither flank flips): provenance only, no edge-pair identity.
/// let none = Occupancy { a_l: false, a_r: false, b_l: false, b_r: false, frame: false };
/// assert_eq!(identity_mode(none), IdentityMode::Provenance);
/// ```
pub fn identity_mode(occ: Occupancy) -> IdentityMode {
    let boundaries = u8::from(occ.a_l != occ.a_r) + u8::from(occ.b_l != occ.b_r);
    match boundaries {
        2 => IdentityMode::PairIdentical,
        1 => IdentityMode::OutputSourceIdentical,
        _ => IdentityMode::Provenance,
    }
}
