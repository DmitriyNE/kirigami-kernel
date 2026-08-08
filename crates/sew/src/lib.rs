#![forbid(unsafe_code)]
//! `sew` — the sewing layer (shell tier; M5).
//!
//! EDGE-OCCUPANCY construction (four bits + frame bit; both constructors,
//! ARRANGEMENT-BITS and MITER-REGION-IDENTITY), the identity dispatch table,
//! MITER-EDGE-LEDGER + MITER-OUT (EDGE-REG/EMB/EDGE-EDGE, CYCLE, coverage,
//! vertex quotient), the quadrant classifier, and SEW-LINK (embedded spherical
//! link, FACE-GERM branch index, invariant-jet ties). Constructions here;
//! their checkers are in `certify_core::sew`.

use arrange2d::boolean::CellLabeling;
use certify_core::miter::{MiterLedger, Occupancy, OrderSign};
use certify_core::sew::{EdgeIdentity, EdgeProvenance, EdgeRecord};
use lattice::Backend;

/// **MITER-REGION-IDENTITY** constructor: project a [`MiterLedger`]'s edges into SEW-EDGES
/// records for [`sew_edges`](certify_core::sew::sew_edges). Each ledger edge is a single
/// coincident flank-to-flank pairing — MITER-FIT already proved the two flanks trace the same
/// segment and minted `ε_φ` — so the record replays that as a [`PairIdentical`] identity, with
/// both paired edges set to the stored coincident segment and the ledger's minted order sign.
///
/// This does no geometry: it *reads* the occupancy the miter ledger already carries per edge
/// (spec §8.5, "sides from stored boundary orientations"). SEW-EDGES then re-derives the
/// opposite-sides consistency and the point identity as an audit.
///
/// [`PairIdentical`]: certify_core::sew::EdgeIdentity::PairIdentical
///
/// ```
/// use sew::records_from_miter_ledger;
/// use certify_core::miter::{LedgerEdge, MiterLedger, Occupancy, OrderSign};
/// use certify_core::sew::{sew_edges, SewCounts};
/// use certify_core::Verdict;
/// use lattice::{Bignum, Rat};
///
/// let p = |x: i128, y: i128| (Rat::<Bignum>::from_i128(x), Rat::from_i128(y));
/// let ledger = MiterLedger {
///     edges: vec![LedgerEdge {
///         start: p(0, 0),
///         end: p(4, 0),
///         eps_phi: OrderSign::Preserving,
///         occupancy: Occupancy { a_l: true, a_r: false, b_l: false, b_r: true, frame: false },
///     }],
/// };
/// let records = records_from_miter_ledger(&ledger);
/// let v = sew_edges(&records, SewCounts { cap_to_flank: 0, flank_to_flank: 1 });
/// assert!(matches!(v, Verdict::Verified(_)));
/// ```
pub fn records_from_miter_ledger<B: Backend>(ledger: &MiterLedger<B>) -> Vec<EdgeRecord<B>> {
    ledger
        .edges
        .iter()
        .map(|e| {
            // Flank A is the stored directed edge; flank B runs along the same coincident segment
            // with the orientation MITER-FIT minted into ε_φ — Preserving keeps A's direction,
            // Reversing flips it. `pair_identical_ok` then re-audits point identity ∘ ε_φ.
            let (b_start, b_end) = match e.eps_phi {
                OrderSign::Preserving => (e.start.clone(), e.end.clone()),
                OrderSign::Reversing => (e.end.clone(), e.start.clone()),
            };
            EdgeRecord {
                occupancy: e.occupancy,
                provenance: EdgeProvenance::FlankToFlank,
                identity: EdgeIdentity::PairIdentical {
                    a_start: e.start.clone(),
                    a_end: e.end.clone(),
                    b_start,
                    b_end,
                    eps: e.eps_phi,
                },
            }
        })
        .collect()
}

/// **ARRANGEMENT-BITS** constructor: read the four occupancy bits of a LEDGE-branch separating
/// edge directly from a [`CellLabeling`] — "a projection of the §6 cell labels, four lookups, no
/// computation" (spec §8.5). Edge `edge_k` indexes `labeling.adj`, whose entry names the two
/// incident cycles `(cyc_l, cyc_r, _, _)`; each cycle's `(A, B)` membership pair is
/// `labeling.labels[cyc]`. The `frame` bit is supplied by the caller (the boundary orientation
/// authority), not recomputed here.
///
/// Returns `None` if `edge_k` or either named cycle is out of range — a malformed labeling, which
/// the caller must not turn into a record.
///
/// ```
/// use sew::arrangement_bits;
/// use arrange2d::boolean::CellLabeling;
/// use certify_core::miter::Occupancy;
///
/// // Two cycles: cycle 0 is in operand A only, cycle 1 in operand B only. Edge 0 separates them.
/// let labeling = CellLabeling {
///     n_cycles: 2,
///     labels: vec![(true, false), (false, true)],
///     adj: vec![(0, 1, false, false)],
///     seed: 0,
///     cocycle_ok: true,
/// };
/// let occ = arrangement_bits(&labeling, 0, false).unwrap();
/// assert_eq!(occ, Occupancy { a_l: true, a_r: false, b_l: false, b_r: true, frame: false });
/// ```
pub fn arrangement_bits(labeling: &CellLabeling, edge_k: usize, frame: bool) -> Option<Occupancy> {
    let (cyc_l, cyc_r, _flip_a, _flip_b) = *labeling.adj.get(edge_k)?;
    let (a_l, b_l) = *labeling.labels.get(cyc_l)?;
    let (a_r, b_r) = *labeling.labels.get(cyc_r)?;
    Some(Occupancy {
        a_l,
        a_r,
        b_l,
        b_r,
        frame,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use certify_core::Verdict;
    use certify_core::miter::LedgerEdge;
    use certify_core::sew::{SewCounts, SewEdges, sew_edges};
    use lattice::{Bignum, Rat};

    fn p(x: i128, y: i128) -> (Rat<Bignum>, Rat<Bignum>) {
        (Rat::from_i128(x), Rat::from_i128(y))
    }

    #[test]
    fn miter_ledger_records_pass_sew_edges() {
        let ledger = MiterLedger {
            edges: vec![
                LedgerEdge {
                    start: p(0, 0),
                    end: p(4, 0),
                    eps_phi: OrderSign::Preserving,
                    occupancy: Occupancy {
                        a_l: true,
                        a_r: false,
                        b_l: false,
                        b_r: true,
                        frame: false,
                    },
                },
                LedgerEdge {
                    start: p(0, 2),
                    end: p(4, 2),
                    eps_phi: OrderSign::Reversing,
                    occupancy: Occupancy {
                        a_l: false,
                        a_r: true,
                        b_l: true,
                        b_r: false,
                        frame: true,
                    },
                },
            ],
        };
        let records = records_from_miter_ledger(&ledger);
        assert_eq!(records.len(), 2);
        let v = sew_edges(
            &records,
            SewCounts {
                cap_to_flank: 0,
                flank_to_flank: 2,
            },
        );
        assert_eq!(
            v,
            Verdict::Verified(SewEdges {
                cap_to_flank: 0,
                flank_to_flank: 2,
            })
        );
    }

    #[test]
    fn arrangement_bits_reads_four_labels() {
        let labeling = CellLabeling {
            n_cycles: 2,
            labels: vec![(true, false), (false, true)],
            adj: vec![(0, 1, false, false)],
            seed: 0,
            cocycle_ok: true,
        };
        assert_eq!(
            arrangement_bits(&labeling, 0, true),
            Some(Occupancy {
                a_l: true,
                a_r: false,
                b_l: false,
                b_r: true,
                frame: true,
            })
        );
    }

    #[test]
    fn arrangement_bits_out_of_range_is_none() {
        let labeling = CellLabeling {
            n_cycles: 1,
            labels: vec![(true, false)],
            adj: vec![],
            seed: 0,
            cocycle_ok: true,
        };
        // No such edge.
        assert_eq!(arrangement_bits(&labeling, 0, false), None);
        // Edge naming a cycle out of range.
        let bad = CellLabeling {
            n_cycles: 1,
            labels: vec![(true, false)],
            adj: vec![(0, 5, false, false)],
            seed: 0,
            cocycle_ok: true,
        };
        assert_eq!(arrangement_bits(&bad, 0, false), None);
    }
}
