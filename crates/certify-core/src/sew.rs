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

use core::cmp::Ordering;

use lattice::{Backend, Bignum, Rat};

use crate::arrange::{LinkClass, classify_link};
use crate::miter::{Occupancy, OrderSign, cross2, dot2, sub2};
use crate::verdict::Verdict;

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

/// Exact point-set identity of two paired edges plus the order sign — the **PAIR-IDENTICAL**
/// obligation (spec §8.5). The two edges must trace the *same* segment, and `eps` must be the
/// orientation relating them: [`Preserving`](OrderSign::Preserving) when they run the same way
/// (`a_start = b_start ∧ a_end = b_end`), [`Reversing`](OrderSign::Reversing) when opposed
/// (`a_start = b_end ∧ a_end = b_start`).
///
/// On the MITER branch the ledger stores the single coincident edge — MITER-FIT already proved
/// the point-identity — so SEW replays this as a cheap citation. It does the real work on the
/// D24 ledge stratum (D24-STAGE2-EQUALITY), where the two paired edges are distinct records.
///
/// ```
/// use certify_core::sew::pair_identical_ok;
/// use certify_core::miter::OrderSign;
/// use lattice::{Bignum, Rat};
///
/// let p = |x: i128, y: i128| (Rat::<Bignum>::from_i128(x), Rat::from_i128(y));
/// // The same segment traced in opposite directions ⇒ Reversing (not Preserving).
/// assert!(pair_identical_ok(&p(0, 0), &p(4, 0), &p(4, 0), &p(0, 0), OrderSign::Reversing));
/// assert!(!pair_identical_ok(&p(0, 0), &p(4, 0), &p(4, 0), &p(0, 0), OrderSign::Preserving));
/// ```
pub fn pair_identical_ok<B: Backend>(
    a_start: &(Rat<B>, Rat<B>),
    a_end: &(Rat<B>, Rat<B>),
    b_start: &(Rat<B>, Rat<B>),
    b_end: &(Rat<B>, Rat<B>),
    eps: OrderSign,
) -> bool {
    match eps {
        OrderSign::Preserving => a_start == b_start && a_end == b_end,
        OrderSign::Reversing => a_start == b_end && a_end == b_start,
    }
}

/// The **OUTPUT-SOURCE-IDENTICAL** obligation (spec §8.5) for a one-boundary edge: the emitted
/// output edge lies on the **same carrier** as its arrangement source, its extent is
/// **contained** in the source's (the arrangement legitimately splits a source edge), and its
/// `sense` matches the source half-edge direction. Totality — every source covered, no invented
/// edge — is CAP-OUT's boundary bijection, cited there, never re-derived here.
///
/// ```
/// use certify_core::sew::output_source_identical_ok;
/// use certify_core::miter::OrderSign;
/// use lattice::{Bignum, Rat};
///
/// let p = |x: i128, y: i128| (Rat::<Bignum>::from_i128(x), Rat::from_i128(y));
/// // Output (1,0)→(3,0) is the middle third of source (0,0)→(4,0), same direction.
/// assert!(output_source_identical_ok(&p(1, 0), &p(3, 0), &p(0, 0), &p(4, 0), OrderSign::Preserving));
/// // Poking past the source's end fails containment.
/// assert!(!output_source_identical_ok(&p(1, 0), &p(5, 0), &p(0, 0), &p(4, 0), OrderSign::Preserving));
/// ```
pub fn output_source_identical_ok<B: Backend>(
    out_start: &(Rat<B>, Rat<B>),
    out_end: &(Rat<B>, Rat<B>),
    src_start: &(Rat<B>, Rat<B>),
    src_end: &(Rat<B>, Rat<B>),
    sense: OrderSign,
) -> bool {
    let d = sub2(src_end, src_start);
    let dd = dot2(&d, &d);
    if dd.is_zero() {
        return false; // degenerate source carrier — no direction to contain against
    }
    // Same carrier: both output endpoints lie on the source line.
    if !cross2(&d, &sub2(out_start, src_start)).is_zero()
        || !cross2(&d, &sub2(out_end, src_start)).is_zero()
    {
        return false;
    }
    // Interval containment: 0 ≤ (p − src_start)·d ≤ d·d for each output endpoint.
    for p in [out_start, out_end] {
        let t = dot2(&sub2(p, src_start), &d);
        if t.sign() < 0 || t.cmp(&dd) == Ordering::Greater {
            return false;
        }
    }
    // Sense: the output direction agrees with (Preserving) or opposes (Reversing) the source.
    let along = dot2(&sub2(out_end, out_start), &d).sign();
    match sense {
        OrderSign::Preserving => along > 0,
        OrderSign::Reversing => along < 0,
    }
}

/// The **MITER-REGION-IDENTITY** side consistency (spec §8.5): a boundary-boundary miter edge is
/// a watertight shell edge only when the two flanks' material lies on **opposite** transverse
/// sides (`A_L ≠ B_L`) — the flanks close the solid across Π. A same-side occupancy is a folded,
/// non-manifold configuration that the quadrant test alone (reading it as one interval) would
/// pass; this rejects it.
///
/// Deriving each flank's occupied side from its stored boundary orientation composed with `ε_φ`
/// and the packet frame bit is the orientation-authority ladder deferred with the SEW-LINK jet
/// machinery (spec §8.5); on the transverse straight-crease slice the load-bearing content is
/// exactly this opposite-sides condition.
pub fn miter_opposite_sides_ok(occ: Occupancy) -> bool {
    occ.is_boundary_boundary() && occ.a_l != occ.b_l
}

/// The typed provenance class of a SEW-EDGES record (spec §8.5, "typed exact counts"):
/// a **cap-to-flank** incidence (a cap-region output edge abutting a flank — the LEDGE branch's
/// OUTPUT-SOURCE-IDENTICAL stratum) or a **flank-to-flank** incidence (two flanks paired across
/// the crease — the MITER branch's PAIR-IDENTICAL stratum).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EdgeProvenance {
    /// A cap output edge abutting a flank (LEDGE branch).
    CapToFlank,
    /// Two flanks paired across the crease (MITER branch).
    FlankToFlank,
}

/// The identity evidence a [`EdgeRecord`] carries, one variant per [`IdentityMode`] — the
/// occupancy's boundary count selects which variant the record must supply.
#[derive(Clone, Debug)]
pub enum EdgeIdentity<B: Backend = Bignum> {
    /// Two boundaries: the two paired edges (checked by [`pair_identical_ok`]) and their order sign.
    PairIdentical {
        /// Paired edge A's start.
        a_start: (Rat<B>, Rat<B>),
        /// Paired edge A's end.
        a_end: (Rat<B>, Rat<B>),
        /// Paired edge B's start.
        b_start: (Rat<B>, Rat<B>),
        /// Paired edge B's end.
        b_end: (Rat<B>, Rat<B>),
        /// The order sign relating the two.
        eps: OrderSign,
    },
    /// One boundary: the emitted output edge, its arrangement source, and the source-relative
    /// sense (checked by [`output_source_identical_ok`]).
    OutputSourceIdentical {
        /// Emitted output edge start.
        out_start: (Rat<B>, Rat<B>),
        /// Emitted output edge end.
        out_end: (Rat<B>, Rat<B>),
        /// Arrangement source edge start.
        src_start: (Rat<B>, Rat<B>),
        /// Arrangement source edge end.
        src_end: (Rat<B>, Rat<B>),
        /// The output's sense relative to the source half-edge.
        sense: OrderSign,
    },
    /// Zero boundaries: no edge-pair identity to discharge — a non-emitting internal edge, which
    /// SEW-EDGES rejects if it appears as a shell record (spec: zero boundaries ⇒ zero output).
    Provenance,
}

/// One SEW-EDGES edge record: its transverse [`Occupancy`], its typed [`EdgeProvenance`], and
/// the [`EdgeIdentity`] evidence its occupancy's [`IdentityMode`] demands. Built by the `sew`
/// searcher's two constructors (MITER-REGION-IDENTITY, ARRANGEMENT-BITS) and consumed by
/// [`sew_edges`].
#[derive(Clone, Debug)]
pub struct EdgeRecord<B: Backend = Bignum> {
    /// The four transverse occupancy bits (+ frame bit).
    pub occupancy: Occupancy,
    /// The typed provenance class.
    pub provenance: EdgeProvenance,
    /// The identity evidence for the occupancy's mode.
    pub identity: EdgeIdentity<B>,
}

/// The source-side incidence counts SEW-EDGES checks the emitted records against, **both
/// directions** (spec §8.5): no declared incidence without a record (completeness) and no record
/// without a declared incidence (soundness). The clean-miter branch declares
/// `{cap_to_flank: 0, flank_to_flank: <ledger edges>}`; an empty or internal joint declares
/// `{0, 0}`, forcing zero records.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SewCounts {
    /// Declared cap-to-flank incidences.
    pub cap_to_flank: usize,
    /// Declared flank-to-flank incidences.
    pub flank_to_flank: usize,
}

/// The SEW-EDGES verdict evidence: the verified typed counts (equal to the declared source-side
/// counts, both directions).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SewEdges {
    /// Verified cap-to-flank record count.
    pub cap_to_flank: usize,
    /// Verified flank-to-flank record count.
    pub flank_to_flank: usize,
}

/// Why SEW-EDGES refused the record set (spec §8.5). Every per-edge fault is located by record
/// index `at`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SewEdgesFault {
    /// The record's occupancy is an opposite-quadrant pinch — a non-manifold transverse link.
    Pinch {
        /// The offending record index.
        at: usize,
    },
    /// A record was emitted for a non-boundary edge (interior or exterior occupancy): a shell
    /// edge must be one proper cyclic interval, and internal/empty edges emit zero records.
    NonBoundaryRecord {
        /// The offending record index.
        at: usize,
    },
    /// The identity variant supplied does not match the occupancy's [`IdentityMode`].
    ModeMismatch {
        /// The offending record index.
        at: usize,
    },
    /// The identity obligation failed: point-set identity / order sign (PAIR-IDENTICAL), or
    /// same-carrier / containment / sense (OUTPUT-SOURCE-IDENTICAL), or the miter opposite-sides
    /// consistency.
    IdentityFailed {
        /// The offending record index.
        at: usize,
    },
    /// The typed provenance disagrees with the identity mode (a flank-to-flank record that is not
    /// PAIR-IDENTICAL, or a cap-to-flank record that is not OUTPUT-SOURCE-IDENTICAL).
    ProvenanceMismatch {
        /// The offending record index.
        at: usize,
    },
    /// The emitted typed counts do not equal the declared source-side counts — some direction of
    /// the reverse equality `{records} = {cap-to-flank} ⊔ {flank-to-flank}` fails.
    CountMismatch {
        /// Emitted cap-to-flank count.
        cap_to_flank: usize,
        /// Emitted flank-to-flank count.
        flank_to_flank: usize,
        /// Declared cap-to-flank count.
        expected_cap_to_flank: usize,
        /// Declared flank-to-flank count.
        expected_flank_to_flank: usize,
    },
}

/// SEW-EDGES (spec §8.5): audit the emitted edge records as a set. Each record passes the
/// quadrant test (no pinch, and a genuine boundary — [`occupancy_row`]), carries and discharges
/// the identity evidence its [`IdentityMode`] demands, and is typed consistently with its
/// [`EdgeProvenance`]; then the typed counts must equal the declared source-side counts **both
/// directions** — the reverse equality `{records} = {cap-to-flank} ⊔ {flank-to-flank}`, with
/// empty and internal joints forced to zero records. Total: `Verified` / `Refuted`, never
/// `Unresolved`.
///
/// ```
/// use certify_core::sew::{sew_edges, EdgeIdentity, EdgeProvenance, EdgeRecord, SewCounts};
/// use certify_core::miter::{Occupancy, OrderSign};
/// use certify_core::Verdict;
/// use lattice::{Bignum, Rat};
///
/// let p = |x: i128, y: i128| (Rat::<Bignum>::from_i128(x), Rat::from_i128(y));
/// // One clean-miter shell edge: opposite-side boundary-boundary, flank-to-flank, coincident pair.
/// let rec = EdgeRecord {
///     occupancy: Occupancy { a_l: true, a_r: false, b_l: false, b_r: true, frame: false },
///     provenance: EdgeProvenance::FlankToFlank,
///     identity: EdgeIdentity::PairIdentical {
///         a_start: p(0, 0), a_end: p(4, 0), b_start: p(0, 0), b_end: p(4, 0),
///         eps: OrderSign::Preserving,
///     },
/// };
/// let v = sew_edges(&[rec], SewCounts { cap_to_flank: 0, flank_to_flank: 1 });
/// assert!(matches!(v, Verdict::Verified(_)));
/// ```
pub fn sew_edges<B: Backend>(
    records: &[EdgeRecord<B>],
    expected: SewCounts,
) -> Verdict<SewEdges, SewEdgesFault, ()> {
    let mut cap = 0usize;
    let mut flank = 0usize;
    for (at, rec) in records.iter().enumerate() {
        // Quadrant test: reject a pinch; a shell record must be a genuine boundary edge, so an
        // interior/exterior occupancy (a zero-boundary internal edge) may not carry a record.
        match occupancy_row(rec.occupancy) {
            LinkClass::Pinch => return Verdict::Refuted(SewEdgesFault::Pinch { at }),
            LinkClass::Boundary => {}
            LinkClass::Interior | LinkClass::Exterior => {
                return Verdict::Refuted(SewEdgesFault::NonBoundaryRecord { at });
            }
        }
        // Identity mode ⟺ evidence variant, discharge the obligation, and type the provenance.
        match (&rec.identity, identity_mode(rec.occupancy)) {
            (
                EdgeIdentity::PairIdentical {
                    a_start,
                    a_end,
                    b_start,
                    b_end,
                    eps,
                },
                IdentityMode::PairIdentical,
            ) => {
                if !pair_identical_ok(a_start, a_end, b_start, b_end, *eps)
                    || !miter_opposite_sides_ok(rec.occupancy)
                {
                    return Verdict::Refuted(SewEdgesFault::IdentityFailed { at });
                }
                if rec.provenance != EdgeProvenance::FlankToFlank {
                    return Verdict::Refuted(SewEdgesFault::ProvenanceMismatch { at });
                }
                flank += 1;
            }
            (
                EdgeIdentity::OutputSourceIdentical {
                    out_start,
                    out_end,
                    src_start,
                    src_end,
                    sense,
                },
                IdentityMode::OutputSourceIdentical,
            ) => {
                if !output_source_identical_ok(out_start, out_end, src_start, src_end, *sense) {
                    return Verdict::Refuted(SewEdgesFault::IdentityFailed { at });
                }
                if rec.provenance != EdgeProvenance::CapToFlank {
                    return Verdict::Refuted(SewEdgesFault::ProvenanceMismatch { at });
                }
                cap += 1;
            }
            _ => return Verdict::Refuted(SewEdgesFault::ModeMismatch { at }),
        }
    }
    // Typed counts, both directions. Each record is exactly one class, so `cap + flank =
    // records.len()` is structural — the reverse equality {records} = {cap} ⊔ {flank}; matching
    // the declared counts then closes both no-omission and no-extra.
    if cap != expected.cap_to_flank || flank != expected.flank_to_flank {
        return Verdict::Refuted(SewEdgesFault::CountMismatch {
            cap_to_flank: cap,
            flank_to_flank: flank,
            expected_cap_to_flank: expected.cap_to_flank,
            expected_flank_to_flank: expected.flank_to_flank,
        });
    }
    Verdict::Verified(SewEdges {
        cap_to_flank: cap,
        flank_to_flank: flank,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: i128, y: i128) -> (Rat<Bignum>, Rat<Bignum>) {
        (Rat::from_i128(x), Rat::from_i128(y))
    }

    // A clean-miter shell edge: opposite-side boundary-boundary, alternating flanks.
    fn clean_miter_occ() -> Occupancy {
        Occupancy {
            a_l: true,
            a_r: false,
            b_l: false,
            b_r: true,
            frame: false,
        }
    }

    #[test]
    fn pair_identical_preserving_and_reversing() {
        // Same segment, same direction.
        assert!(pair_identical_ok(
            &p(0, 0),
            &p(4, 0),
            &p(0, 0),
            &p(4, 0),
            OrderSign::Preserving
        ));
        // Same segment, opposite direction ⇒ only Reversing accepts.
        assert!(pair_identical_ok(
            &p(0, 0),
            &p(4, 0),
            &p(4, 0),
            &p(0, 0),
            OrderSign::Reversing
        ));
        assert!(!pair_identical_ok(
            &p(0, 0),
            &p(4, 0),
            &p(4, 0),
            &p(0, 0),
            OrderSign::Preserving
        ));
        // Different segment: neither sign accepts.
        assert!(!pair_identical_ok(
            &p(0, 0),
            &p(4, 0),
            &p(0, 1),
            &p(4, 1),
            OrderSign::Preserving
        ));
    }

    #[test]
    fn output_source_containment_and_carrier() {
        // Middle third of the source, same direction.
        assert!(output_source_identical_ok(
            &p(1, 0),
            &p(3, 0),
            &p(0, 0),
            &p(4, 0),
            OrderSign::Preserving
        ));
        // Reversed output on the same carrier ⇒ Reversing.
        assert!(output_source_identical_ok(
            &p(3, 0),
            &p(1, 0),
            &p(0, 0),
            &p(4, 0),
            OrderSign::Reversing
        ));
        // Off the carrier line.
        assert!(!output_source_identical_ok(
            &p(1, 1),
            &p(3, 0),
            &p(0, 0),
            &p(4, 0),
            OrderSign::Preserving
        ));
        // Past the source end (t > d·d).
        assert!(!output_source_identical_ok(
            &p(1, 0),
            &p(5, 0),
            &p(0, 0),
            &p(4, 0),
            OrderSign::Preserving
        ));
        // Before the source start (t < 0).
        assert!(!output_source_identical_ok(
            &p(-1, 0),
            &p(3, 0),
            &p(0, 0),
            &p(4, 0),
            OrderSign::Preserving
        ));
        // Degenerate source carrier.
        assert!(!output_source_identical_ok(
            &p(1, 0),
            &p(1, 0),
            &p(2, 2),
            &p(2, 2),
            OrderSign::Preserving
        ));
        // Wrong sense on a contained collinear output.
        assert!(!output_source_identical_ok(
            &p(1, 0),
            &p(3, 0),
            &p(0, 0),
            &p(4, 0),
            OrderSign::Reversing
        ));
    }

    #[test]
    fn opposite_sides_gate() {
        assert!(miter_opposite_sides_ok(clean_miter_occ()));
        // Same-side flanks (A_L == B_L) — a folded, non-manifold config.
        assert!(!miter_opposite_sides_ok(Occupancy {
            a_l: true,
            a_r: false,
            b_l: true,
            b_r: false,
            frame: false,
        }));
        // Not boundary-boundary at all (A_L == A_R).
        assert!(!miter_opposite_sides_ok(Occupancy {
            a_l: true,
            a_r: true,
            b_l: false,
            b_r: true,
            frame: false,
        }));
    }

    fn miter_record() -> EdgeRecord {
        EdgeRecord {
            occupancy: clean_miter_occ(),
            provenance: EdgeProvenance::FlankToFlank,
            identity: EdgeIdentity::PairIdentical {
                a_start: p(0, 0),
                a_end: p(4, 0),
                b_start: p(0, 0),
                b_end: p(4, 0),
                eps: OrderSign::Preserving,
            },
        }
    }

    #[test]
    fn sew_edges_accepts_clean_miter() {
        let v = sew_edges(
            &[miter_record()],
            SewCounts {
                cap_to_flank: 0,
                flank_to_flank: 1,
            },
        );
        assert_eq!(
            v,
            Verdict::Verified(SewEdges {
                cap_to_flank: 0,
                flank_to_flank: 1,
            })
        );
    }

    #[test]
    fn sew_edges_accepts_ledge_output_source() {
        // One boundary ⇒ OutputSourceIdentical, cap-to-flank.
        let rec = EdgeRecord {
            occupancy: Occupancy {
                a_l: true,
                a_r: false,
                b_l: false,
                b_r: false,
                frame: false,
            },
            provenance: EdgeProvenance::CapToFlank,
            identity: EdgeIdentity::OutputSourceIdentical {
                out_start: p(1, 0),
                out_end: p(3, 0),
                src_start: p(0, 0),
                src_end: p(4, 0),
                sense: OrderSign::Preserving,
            },
        };
        let v = sew_edges(
            &[rec],
            SewCounts {
                cap_to_flank: 1,
                flank_to_flank: 0,
            },
        );
        assert!(matches!(v, Verdict::Verified(_)));
    }

    #[test]
    fn sew_edges_rejects_pinch() {
        // Opposite quadrants of the same flank ⇒ Pinch.
        let rec = EdgeRecord {
            occupancy: Occupancy {
                a_l: true,
                a_r: true,
                b_l: false,
                b_r: false,
                frame: false,
            },
            provenance: EdgeProvenance::FlankToFlank,
            identity: EdgeIdentity::<Bignum>::Provenance,
        };
        assert_eq!(
            sew_edges(
                &[rec],
                SewCounts {
                    cap_to_flank: 0,
                    flank_to_flank: 0
                }
            ),
            Verdict::Refuted(SewEdgesFault::Pinch { at: 0 })
        );
    }

    #[test]
    fn sew_edges_rejects_non_boundary_record() {
        // All four occupied ⇒ Interior, no shell record allowed.
        let rec = EdgeRecord {
            occupancy: Occupancy {
                a_l: true,
                a_r: true,
                b_l: true,
                b_r: true,
                frame: false,
            },
            provenance: EdgeProvenance::FlankToFlank,
            identity: EdgeIdentity::<Bignum>::Provenance,
        };
        assert_eq!(
            sew_edges(
                &[rec],
                SewCounts {
                    cap_to_flank: 0,
                    flank_to_flank: 0
                }
            ),
            Verdict::Refuted(SewEdgesFault::NonBoundaryRecord { at: 0 })
        );
    }

    #[test]
    fn sew_edges_rejects_mode_mismatch() {
        // Two boundaries want PairIdentical; supply OutputSourceIdentical.
        let rec = EdgeRecord {
            occupancy: clean_miter_occ(),
            provenance: EdgeProvenance::FlankToFlank,
            identity: EdgeIdentity::OutputSourceIdentical {
                out_start: p(1, 0),
                out_end: p(3, 0),
                src_start: p(0, 0),
                src_end: p(4, 0),
                sense: OrderSign::Preserving,
            },
        };
        assert_eq!(
            sew_edges(
                &[rec],
                SewCounts {
                    cap_to_flank: 0,
                    flank_to_flank: 0
                }
            ),
            Verdict::Refuted(SewEdgesFault::ModeMismatch { at: 0 })
        );
    }

    #[test]
    fn sew_edges_rejects_identity_failure_same_side() {
        // Two boundaries, PairIdentical point-identity holds, but same-side flanks.
        let rec = EdgeRecord {
            occupancy: Occupancy {
                a_l: true,
                a_r: false,
                b_l: true,
                b_r: false,
                frame: false,
            },
            provenance: EdgeProvenance::FlankToFlank,
            identity: EdgeIdentity::PairIdentical {
                a_start: p(0, 0),
                a_end: p(4, 0),
                b_start: p(0, 0),
                b_end: p(4, 0),
                eps: OrderSign::Preserving,
            },
        };
        assert_eq!(
            sew_edges(
                &[rec],
                SewCounts {
                    cap_to_flank: 0,
                    flank_to_flank: 0
                }
            ),
            Verdict::Refuted(SewEdgesFault::IdentityFailed { at: 0 })
        );
    }

    #[test]
    fn sew_edges_rejects_provenance_mismatch() {
        // Clean miter is flank-to-flank; declaring cap-to-flank is a type error.
        let mut rec = miter_record();
        rec.provenance = EdgeProvenance::CapToFlank;
        assert_eq!(
            sew_edges(
                &[rec],
                SewCounts {
                    cap_to_flank: 0,
                    flank_to_flank: 0
                }
            ),
            Verdict::Refuted(SewEdgesFault::ProvenanceMismatch { at: 0 })
        );
    }

    #[test]
    fn sew_edges_rejects_count_mismatch() {
        // Valid record, but declared counts disagree with the emitted set.
        assert_eq!(
            sew_edges(
                &[miter_record()],
                SewCounts {
                    cap_to_flank: 0,
                    flank_to_flank: 2
                }
            ),
            Verdict::Refuted(SewEdgesFault::CountMismatch {
                cap_to_flank: 0,
                flank_to_flank: 1,
                expected_cap_to_flank: 0,
                expected_flank_to_flank: 2,
            })
        );
    }

    #[test]
    fn sew_edges_empty_joint_zero_records() {
        // Empty/internal ⇒ zero incidence ∧ zero records.
        let v = sew_edges::<Bignum>(
            &[],
            SewCounts {
                cap_to_flank: 0,
                flank_to_flank: 0,
            },
        );
        assert_eq!(
            v,
            Verdict::Verified(SewEdges {
                cap_to_flank: 0,
                flank_to_flank: 0,
            })
        );
    }

    #[test]
    fn identity_mode_by_boundary_count() {
        assert_eq!(
            identity_mode(clean_miter_occ()),
            IdentityMode::PairIdentical
        );
        assert_eq!(
            identity_mode(Occupancy {
                a_l: true,
                a_r: false,
                b_l: false,
                b_r: false,
                frame: false
            }),
            IdentityMode::OutputSourceIdentical
        );
        assert_eq!(
            identity_mode(Occupancy {
                a_l: true,
                a_r: true,
                b_l: false,
                b_r: false,
                frame: false
            }),
            IdentityMode::Provenance
        );
    }
}
