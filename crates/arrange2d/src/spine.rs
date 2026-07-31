//! The event-spine driver (M3a Phase 4) — spec §6 steps 1–4, branch priority
//! most-degenerate-first: (1) CARRIER-COINCIDENT first (its result is deferred to
//! the stage-2 1D coincidence lattice, slice 3c — the seam is here); (2) else
//! solve carrier ∩ carrier; (3) interval membership on both edges before any
//! classification; (4) classify the survivors. The untrusted searcher entry
//! (`arrange_events`), returning a `certify_core::Verdict` of the emitted
//! `EventSet` + a replayable [`super::witness`].

use certify_core::{MarginSq, Verdict};
use core::convert::Infallible;
use geom::content::{CurveId, Edge, Point2};
use lattice::{Backend, Rat};

use crate::carrier::{self, Intersections};
use crate::classify::{det_at, kind_of};
use crate::event::{EventSet, Incidence};
use crate::membership::on_edge;
use crate::predicates;
use crate::witness::{PairWitness, SpineBranch, TouchWitness, Witness};

/// The searcher entry's verdict: `Verified((events, witness))` at degree ≤ 2
/// (always — every predicate is total), `Unresolved(margin)` reserved for the L3
/// escalation, and `Refuted` [`Infallible`] (the searcher computes, never refutes).
pub type ArrangeVerdict<B> = Verdict<(EventSet<B>, Witness<B>), Infallible, MarginSq<Rat<B>>>;

/// The source curve id an edge came from.
fn source_of<B: Backend>(edge: &Edge<B>) -> CurveId {
    match edge {
        Edge::Seg(s) => s.source,
        Edge::Arc(a) => a.source,
    }
}

/// Step 1: do the two edges share a carrier? Lines by the three-minor COINCIDENT;
/// circles by equal centre ∧ r²; a line and a circle never do.
fn carrier_coincident<B: Backend>(a: &Edge<B>, b: &Edge<B>) -> bool {
    match (a, b) {
        (Edge::Seg(x), Edge::Seg(y)) => predicates::coincident(&x.line, &y.line),
        (Edge::Arc(x), Edge::Arc(y)) => predicates::circles_coincident(&x.circle, &y.circle),
        _ => false,
    }
}

/// Step 2: solve carrier ∩ carrier, dispatched by edge kind.
fn carrier_intersect<B: Backend>(a: &Edge<B>, b: &Edge<B>) -> Intersections<B> {
    match (a, b) {
        (Edge::Seg(x), Edge::Seg(y)) => carrier::line_line(&x.line, &y.line),
        (Edge::Seg(x), Edge::Arc(y)) => carrier::line_circle(&x.line, &y.circle),
        (Edge::Arc(x), Edge::Seg(y)) => carrier::line_circle(&y.line, &x.circle),
        (Edge::Arc(x), Edge::Arc(y)) => carrier::circle_circle(&x.circle, &y.circle),
    }
}

/// The result of the four-step spine on one edge pair.
struct PairResult<B: Backend> {
    incidences: Vec<(Point2<B>, Incidence)>,
    witness: PairWitness<B>,
}

/// The event spine (spec §6 steps 1–4) on one pair, most-degenerate-first.
fn arrange_pair<B: Backend>(
    sources: (CurveId, CurveId),
    a: &Edge<B>,
    b: &Edge<B>,
) -> PairResult<B> {
    // (1) CARRIER-COINCIDENT first — the seam to the stage-2 1D lattice (3c).
    // Coincident circles satisfy the internal-tangency identity, so testing
    // coincidence before tangency is what stops the mislabel.
    if carrier_coincident(a, b) {
        return PairResult {
            incidences: Vec::new(),
            witness: PairWitness {
                sources,
                branch: SpineBranch::CarrierCoincident,
                touches: Vec::new(),
            },
        };
    }
    // (2) carrier ∩ carrier. SharedCarrier cannot occur here — it is exactly the
    // coincidence caught in step 1.
    let pts: Vec<Point2<B>> = match carrier_intersect(a, b) {
        Intersections::Empty | Intersections::SharedCarrier => Vec::new(),
        Intersections::One(p) => vec![p],
        Intersections::Two(p, q) => vec![p, q],
    };
    // (3) membership on BOTH edges, then (4) classify the survivors.
    let mut incidences = Vec::new();
    let mut touches = Vec::new();
    for p in pts {
        if on_edge(&p, a) && on_edge(&p, b) {
            let d = det_at(&p, a, b);
            let kind = kind_of(&d);
            incidences.push((p.clone(), Incidence { kind, sources }));
            touches.push(TouchWitness { point: p, det: d });
        }
    }
    let branch = if touches.is_empty() {
        SpineBranch::NoIntersection
    } else {
        SpineBranch::Touches
    };
    PairResult {
        incidences,
        witness: PairWitness {
            sources,
            branch,
            touches,
        },
    }
}

/// The untrusted searcher entry: the touch-vertex [`EventSet`] of an edge list
/// plus the replayable [`Witness`]. All pairs, most-degenerate-first, with ℓ=0
/// vertex dedup. Always `Verified` at degree ≤ 2 (every predicate is total);
/// `Unresolved(margin)` is reserved for the L3 escalation, and `Refuted` is
/// [`Infallible`] — the searcher computes, it never refutes (that is the M3e
/// checker's role).
pub fn arrange_events<B: Backend>(edges: &[Edge<B>]) -> ArrangeVerdict<B> {
    let mut set = EventSet::new();
    let mut wit = Witness::new();
    for (i, ea) in edges.iter().enumerate() {
        for eb in &edges[i + 1..] {
            let sources = (source_of(ea), source_of(eb));
            let res = arrange_pair(sources, ea, eb);
            for (p, inc) in res.incidences {
                set.insert(p, inc);
            }
            wit.pairs.push(res.witness);
        }
    }
    Verdict::Verified((set, wit))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::TouchKind;
    use geom::content::{ArcPiece, Circle, Half, Line, Orient, SegPiece, Winding};
    use lattice::{Bignum, Surd};

    type Q = Rat<Bignum>;
    type P = Point2<Bignum>;

    fn rp(x: i128, y: i128) -> P {
        Point2::from_rat(Q::from_i128(x), Q::from_i128(y))
    }

    /// A full-line segment carrier `a·x + b·y + c = 0` with a wide extent so the
    /// membership gate always passes (we test the classifier here, not extent).
    fn line_edge(a: i128, b: i128, c: i128, src: u32) -> Edge<Bignum> {
        Edge::Seg(Box::new(SegPiece {
            line: Line {
                a: Q::from_i128(a),
                b: Q::from_i128(b),
                c: Q::from_i128(c),
            },
            start: rp(-100, -100),
            end: rp(100, 100),
            orient: Orient::Ccw,
            source: CurveId(src),
        }))
    }

    /// A full circle as one wide arc piece (extent [−r, r], upper), extent chosen
    /// so membership passes for the tested points.
    fn circle_edge(cx: i128, cy: i128, r2: i128, half: Half, src: u32) -> Edge<Bignum> {
        let c = Circle {
            cx: Q::from_i128(cx),
            cy: Q::from_i128(cy),
            r2: Q::from_i128(r2),
        };
        Edge::Arc(Box::new(ArcPiece {
            circle: c,
            half,
            x_lo: Surd::new(Q::from_i128(cx), Q::from_i128(-1), Q::from_i128(r2)),
            x_hi: Surd::new(Q::from_i128(cx), Q::from_i128(1), Q::from_i128(r2)),
            start: rp(cx, cy),
            end: rp(cx, cy),
            winding: Winding {
                orient: Orient::Ccw,
                source_span: None,
            },
            source: CurveId(src),
        }))
    }

    fn verified(v: ArrangeVerdict<Bignum>) -> (EventSet<Bignum>, Witness<Bignum>) {
        match v {
            Verdict::Verified(x) => x,
            _ => panic!("degree-≤2 arrangement is always Verified"),
        }
    }

    /// Corpus `cx_coincident_vs_tangent_circles`: coincident circles → shared
    /// carrier, zero events (COINCIDENT wins over the internal-tangency identity
    /// they also satisfy); non-coincident internally-tangent circles → one Tangent.
    #[test]
    fn cx_coincident_vs_tangent_circles() {
        // coincident: same centre and r².
        let coincident = [
            circle_edge(0, 0, 4, Half::Upper, 0),
            circle_edge(0, 0, 4, Half::Lower, 1),
        ];
        let (set, _) = verified(arrange_events(&coincident));
        assert_eq!(set.len(), 0, "coincident carriers emit no events");

        // internally tangent: centres 1 apart, r = 2 and r = 1, touch at (2, 0).
        let tangent = [
            circle_edge(0, 0, 4, Half::Upper, 0), // r = 2
            circle_edge(1, 0, 1, Half::Upper, 1), // r = 1
        ];
        let (set, _) = verified(arrange_events(&tangent));
        assert_eq!(set.len(), 1);
        assert_eq!(set.vertices[0].incidences[0].kind, TouchKind::Tangent);
    }

    /// Corpus `cx_antipodal_arcs`: two arcs on the SAME circle share a carrier →
    /// shared carrier, zero events, zero merged edges (the refutation of one-stage
    /// merging; the 1D overlap decision is deferred to 3c).
    #[test]
    fn cx_antipodal_arcs() {
        let arcs = [
            circle_edge(0, 0, 1, Half::Upper, 0),
            circle_edge(0, 0, 1, Half::Lower, 0), // same circle, same source
        ];
        let (set, wit) = verified(arrange_events(&arcs));
        assert_eq!(set.len(), 0);
        assert_eq!(wit.pairs[0].branch, SpineBranch::CarrierCoincident);
    }

    #[test]
    fn two_lines_transverse() {
        // x-axis and y-axis cross transversely at the origin.
        let (set, _) = verified(arrange_events(&[
            line_edge(0, 1, 0, 0), // y = 0
            line_edge(1, 0, 0, 1), // x = 0
        ]));
        assert_eq!(set.len(), 1);
        assert_eq!(set.vertices[0].point, rp(0, 0));
        assert!(matches!(
            set.vertices[0].incidences[0].kind,
            TouchKind::Transverse { .. }
        ));
    }

    #[test]
    fn line_circle_secant_two_transverse() {
        // y = 0 secant through the unit-√2 circle: two transverse crossings.
        let (set, _) = verified(arrange_events(&[
            line_edge(0, 1, 0, 0),
            circle_edge(0, 0, 2, Half::Upper, 1),
        ]));
        assert_eq!(set.len(), 2);
        for v in &set.vertices {
            assert!(matches!(v.incidences[0].kind, TouchKind::Transverse { .. }));
        }
    }

    #[test]
    fn line_circle_tangent() {
        // y = 1 tangent to the unit circle at (0, 1): one Tangent event.
        let (set, _) = verified(arrange_events(&[
            line_edge(0, 1, -1, 0),
            circle_edge(0, 0, 1, Half::Upper, 1),
        ]));
        assert_eq!(set.len(), 1);
        assert_eq!(set.vertices[0].incidences[0].kind, TouchKind::Tangent);
    }

    #[test]
    fn concurrency_dedups_to_one_vertex() {
        // Three distinct lines through the origin ⇒ one vertex, three incidences.
        let (set, _) = verified(arrange_events(&[
            line_edge(0, 1, 0, 0),  // y = 0
            line_edge(1, 0, 0, 1),  // x = 0
            line_edge(1, -1, 0, 2), // y = x
        ]));
        assert_eq!(set.len(), 1, "one vertex (ℓ=0 identity)");
        assert_eq!(set.vertices[0].point, rp(0, 0));
        assert_eq!(set.vertices[0].incidences.len(), 3, "three incidences");
    }

    // --- property: classification is frame-independent ---

    use crate::classify::{det_at, kind_of};
    use proptest::prelude::*;

    /// A rational rigid motion `p ↦ R·p + t`.
    struct Rigid {
        co: Q,
        si: Q,
        tx: Q,
        ty: Q,
    }
    fn rigid(u: i128, v: i128, tx: i128, ty: i128) -> Rigid {
        let den = u * u + v * v;
        Rigid {
            co: Q::new(u * u - v * v, den),
            si: Q::new(2 * u * v, den),
            tx: Q::from_i128(tx),
            ty: Q::from_i128(ty),
        }
    }
    /// `k1·a ± k2·b + t` for rational `k1,k2,t` and (rational) surds a,b.
    fn surd_lin(
        k1: &Q,
        a: &Surd<Bignum>,
        k2: &Q,
        b: &Surd<Bignum>,
        minus: bool,
        t: &Q,
    ) -> Surd<Bignum> {
        let (t1, t2) = (a.scale(k1), b.scale(k2));
        let s = if minus {
            t1.sub(&t2).unwrap_surd()
        } else {
            t1.add(&t2).unwrap_surd()
        };
        s.add(&Surd::from_rat(t.clone())).unwrap_surd()
    }
    fn rigid_pt(p: &P, m: &Rigid) -> P {
        Point2 {
            x: surd_lin(&m.co, &p.x, &m.si, &p.y, true, &m.tx),
            y: surd_lin(&m.si, &p.x, &m.co, &p.y, false, &m.ty),
        }
    }
    /// A line `a·x+b·y+c=0` under `p ↦ R·p + t`: normal rotates (`n' = R·n`),
    /// `c' = c − n'·t`. Wide extent so the classifier (not membership) is tested.
    fn rigid_line_edge(a: &Q, b: &Q, c: &Q, m: &Rigid, src: u32) -> Edge<Bignum> {
        let na = m.co.mul(a).sub(&m.si.mul(b));
        let nb = m.si.mul(a).add(&m.co.mul(b));
        let nc = c.sub(&na.mul(&m.tx).add(&nb.mul(&m.ty)));
        Edge::Seg(Box::new(SegPiece {
            line: Line {
                a: na,
                b: nb,
                c: nc,
            },
            start: rp(-1000, -1000),
            end: rp(1000, 1000),
            orient: Orient::Ccw,
            source: CurveId(src),
        }))
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// The transverse/tangent classification and its sidedness `det_sign` are
        /// invariant under a rational rigid motion: `det(R·u, R·v) = det(R)·det(u,v)`
        /// and `det R = 1`, so the determinant — hence the whole decision — is
        /// preserved. Checked on two random non-parallel lines at their crossing.
        #[test]
        fn classification_rigid_invariant(
            a1 in -6i128..=6, b1 in -6i128..=6, c1 in -6i128..=6,
            a2 in -6i128..=6, b2 in -6i128..=6, c2 in -6i128..=6,
            u in -3i128..=3, v in -3i128..=3,
            mtx in -4i128..=4, mty in -4i128..=4,
        ) {
            prop_assume!(u != 0 || v != 0);
            let (la, lb) = (
                (Q::from_i128(a1), Q::from_i128(b1), Q::from_i128(c1)),
                (Q::from_i128(a2), Q::from_i128(b2), Q::from_i128(c2)),
            );
            // non-parallel ⇒ a unique crossing
            let ea = line_edge(a1, b1, c1, 0);
            let eb = line_edge(a2, b2, c2, 1);
            let p = match carrier::line_line(
                &Line { a: la.0.clone(), b: la.1.clone(), c: la.2.clone() },
                &Line { a: lb.0.clone(), b: lb.1.clone(), c: lb.2.clone() },
            ) {
                Intersections::One(p) => p,
                _ => return Ok(()), // parallel / coincident: no crossing to classify
            };
            let kind0 = kind_of(&det_at(&p, &ea, &eb));

            let m = rigid(u, v, mtx, mty);
            let ea2 = rigid_line_edge(&la.0, &la.1, &la.2, &m, 0);
            let eb2 = rigid_line_edge(&lb.0, &lb.1, &lb.2, &m, 1);
            let p2 = rigid_pt(&p, &m);
            let kind1 = kind_of(&det_at(&p2, &ea2, &eb2));

            prop_assert_eq!(kind0, kind1);
            let is_transverse = matches!(kind0, TouchKind::Transverse { .. }); // lines cross
            prop_assert!(is_transverse);
        }
    }
}
