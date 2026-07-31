//! Per-edge interval membership (M3a Phase 3), checked on **both** edges *before*
//! any classification; non-members are discarded with no vertex and no record.
//! Winding-aware, but after canonical decomposition it collapses to an x-range +
//! half test (the graph-of-a-function property). This is what discards the
//! pole-adjacent phantom. Corpus: `cx_tangent_outside_arc`.
//!
//! Precondition: the point is already on the edge's *carrier* (it is a
//! carrier ∩ carrier solution). Membership therefore tests only the bounded
//! extent — the carrier is not re-checked here.

use core::cmp::Ordering;
use geom::content::{ArcPiece, Edge, Half, Point2, SegPiece};
use lattice::{Backend, Surd};

/// Is `v` within `[min(a, b), max(a, b)]` (inclusive)? Exact, radical-safe `Surd`
/// comparison.
fn between<B: Backend>(v: &Surd<B>, a: &Surd<B>, b: &Surd<B>) -> bool {
    let (lo, hi) = if a.cmp(b) == Ordering::Greater {
        (b, a)
    } else {
        (a, b)
    };
    v.cmp(lo) != Ordering::Less && v.cmp(hi) != Ordering::Greater
}

/// Is a carrier point `p` on the x-monotone arc piece `arc`? Given `p` on the
/// circle, this is `p.x ∈ [x_lo, x_hi]` **and** the half matches — the whole
/// membership-before-classification win (a pole-tangency at `cx − √r²` below
/// `x_lo` is discarded here, before any tangency identity runs). An x-extremum
/// (`y = cy`) is the shared endpoint of both halves, so it is a member of either
/// half whenever its x is in range.
pub fn on_arc<B: Backend>(p: &Point2<B>, arc: &ArcPiece<B>) -> bool {
    if p.x.cmp(&arc.x_lo) == Ordering::Less || p.x.cmp(&arc.x_hi) == Ordering::Greater {
        return false;
    }
    match p.y.cmp(&Surd::from_rat(arc.circle.cy.clone())) {
        Ordering::Equal => true, // x-extremum: shared boundary endpoint
        Ordering::Greater => arc.half == Half::Upper,
        Ordering::Less => arc.half == Half::Lower,
    }
}

/// Is a carrier point `p` on the segment `seg`? Given `p` on the line, this is an
/// exact coordinate-range test: the x-range for a non-vertical segment, the
/// y-range for a vertical one (which the lexicographic (x, then y) sweep order
/// still resolves).
pub fn on_seg<B: Backend>(p: &Point2<B>, seg: &SegPiece<B>) -> bool {
    let (s, e) = (&seg.start, &seg.end);
    if s.x != e.x {
        between(&p.x, &s.x, &e.x)
    } else {
        between(&p.y, &s.y, &e.y)
    }
}

/// Membership of a carrier point on either edge kind (both must pass before the
/// spine classifies the point).
pub fn on_edge<B: Backend>(p: &Point2<B>, edge: &Edge<B>) -> bool {
    match edge {
        Edge::Seg(s) => on_seg(p, s),
        Edge::Arc(a) => on_arc(p, a),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decompose::decompose;
    use crate::testgen::{on_circle_pt, rigid, rigid_circle, rigid_pt};
    use geom::content::{Circle, Curve, CurveId, Line, Orient, Winding};
    use lattice::{Bignum, Rat};

    type Q = Rat<Bignum>;
    type P = Point2<Bignum>;

    fn rp(x: i128, y: i128) -> P {
        Point2::from_rat(Q::from_i128(x), Q::from_i128(y))
    }
    fn rq(n: i128, d: i128) -> Q {
        Q::new(n, d)
    }
    fn circle(cx: i128, cy: i128, r2: i128) -> Circle<Bignum> {
        Circle {
            cx: Q::from_i128(cx),
            cy: Q::from_i128(cy),
            r2: Q::from_i128(r2),
        }
    }
    fn seg(sx: i128, sy: i128, ex: i128, ey: i128) -> SegPiece<Bignum> {
        SegPiece {
            // carrier not consulted by membership
            line: Line {
                a: Q::from_i128(0),
                b: Q::from_i128(0),
                c: Q::from_i128(0),
            },
            start: rp(sx, sy),
            end: rp(ex, ey),
            orient: Orient::Ccw,
            source: CurveId(0),
        }
    }

    /// Corpus `cx_tangent_outside_arc`: a tangency at the pole `(−1, 0)` of the
    /// unit circle, on an arc piece whose x-range `[−1/2, 1/2]` excludes it —
    /// membership discards it, so no phantom vertex reaches classification.
    #[test]
    fn cx_tangent_outside_arc() {
        let arc = ArcPiece {
            circle: circle(0, 0, 1),
            half: Half::Upper,
            x_lo: Surd::from_rat(rq(-1, 2)),
            x_hi: Surd::from_rat(rq(1, 2)),
            start: Point2::from_rat(rq(-1, 2), Q::from_i128(0)),
            end: Point2::from_rat(rq(1, 2), Q::from_i128(0)),
            winding: Winding {
                orient: Orient::Ccw,
                source_span: None,
            },
            source: CurveId(0),
        };
        let pole = rp(-1, 0); // the tangency point, x = −1 < x_lo = −1/2
        assert!(!on_arc(&pole, &arc), "pole below x_lo must be discarded");
    }

    #[test]
    fn arc_membership_x_and_half() {
        // Upper half of the unit circle, full extent [−1, 1].
        let c = circle(0, 0, 1);
        let arc = ArcPiece {
            circle: c,
            half: Half::Upper,
            x_lo: Surd::new(Q::from_i128(0), Q::from_i128(-1), Q::from_i128(1)), // −1
            x_hi: Surd::new(Q::from_i128(0), Q::from_i128(1), Q::from_i128(1)),  // +1
            start: rp(-1, 0),
            end: rp(1, 0),
            winding: Winding {
                orient: Orient::Ccw,
                source_span: None,
            },
            source: CurveId(0),
        };
        assert!(on_arc(&rp(0, 1), &arc)); // top point, upper
        assert!(!on_arc(&rp(0, -1), &arc)); // bottom point, wrong half
        assert!(on_arc(&rp(-1, 0), &arc)); // left extremum, shared endpoint
        assert!(on_arc(&rp(1, 0), &arc)); // right extremum, shared endpoint
        assert!(on_arc(&Point2::from_rat(rq(3, 5), rq(4, 5)), &arc)); // (3/5,4/5) upper
    }

    #[test]
    fn seg_membership_extent() {
        let s = seg(0, 0, 4, 0); // horizontal
        assert!(on_seg(&rp(0, 0), &s)); // endpoint
        assert!(on_seg(&rp(4, 0), &s)); // endpoint
        assert!(on_seg(&rp(2, 0), &s)); // midpoint
        assert!(!on_seg(&rp(5, 0), &s)); // beyond
        assert!(!on_seg(&rp(-1, 0), &s)); // before
    }

    #[test]
    fn seg_membership_vertical() {
        let s = seg(1, -2, 1, 3); // vertical: x ≡ 1, decide by y
        assert!(on_seg(&rp(1, 0), &s));
        assert!(on_seg(&rp(1, -2), &s));
        assert!(on_seg(&rp(1, 3), &s));
        assert!(!on_seg(&rp(1, 4), &s)); // beyond in y
    }

    #[test]
    fn on_edge_dispatches() {
        let s = seg(0, 0, 2, 0);
        assert!(on_edge(&rp(1, 0), &Edge::Seg(Box::new(s))));
    }

    // --- properties ---

    use proptest::prelude::*;

    /// Is `p` on any of the pieces (i.e. on the decomposed curve as a point set)?
    fn member_of_any(p: &P, edges: &[Edge<Bignum>]) -> bool {
        edges.iter().any(|e| on_edge(p, e))
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// Segment membership is invariant under a rational rigid motion: a point
        /// `P = S + t·(E−S)` on the segment's line is a member iff `t ∈ [0, 1]`,
        /// and that verdict is preserved when the whole configuration is moved.
        #[test]
        fn seg_membership_rigid_invariant(
            sx in -5i128..=5, sy in -5i128..=5, ex in -5i128..=5, ey in -5i128..=5,
            tn in -4i128..=8, td in 1i128..=6,
            u in -3i128..=3, v in -3i128..=3,
            mtx in -4i128..=4, mty in -4i128..=4,
        ) {
            prop_assume!(u != 0 || v != 0);
            prop_assume!(sx != ex || sy != ey); // proper segment
            let s = seg(sx, sy, ex, ey);
            // P = S + t·(E−S), on the line.
            let t = Q::new(tn, td);
            let px = Q::from_i128(sx).add(&t.mul(&Q::from_i128(ex - sx)));
            let py = Q::from_i128(sy).add(&t.mul(&Q::from_i128(ey - sy)));
            let p = Point2::from_rat(px, py);
            let in_extent = t.sign() >= 0 && t.cmp(&Q::from_i128(1)) != Ordering::Greater;
            prop_assert_eq!(on_seg(&p, &s), in_extent);

            let m = rigid(u, v, mtx, mty);
            let s2 = SegPiece {
                start: rigid_pt(&s.start, &m),
                end: rigid_pt(&s.end, &m),
                ..s.clone()
            };
            let p2 = rigid_pt(&p, &m);
            prop_assert_eq!(on_seg(&p2, &s2), in_extent);
        }

        /// Arc membership is consistent with decomposition: for a full circle,
        /// every point on the circle is a member of exactly the piece(s) whose
        /// half it matches (both, at an extremum). Points are generated exactly
        /// on the circle by the rational stereographic parametrisation.
        #[test]
        fn arc_membership_matches_decompose(
            cx in -3i128..=3, cy in -3i128..=3, r in 1i128..=6,
            tn in -6i128..=6, td in 1i128..=6,
        ) {
            let (cx, cy, r) = (Q::from_i128(cx), Q::from_i128(cy), Q::from_i128(r));
            let c = Circle { cx: cx.clone(), cy: cy.clone(), r2: r.mul(&r) };
            let edges = decompose(&Curve::Circle {
                circle: c,
                orient: Orient::Ccw,
                source: CurveId(0),
            });
            let (tn, td) = (Q::from_i128(tn), Q::from_i128(td));
            let denom = td.mul(&td).add(&tn.mul(&tn));
            let qx = cx.add(&r.mul(&td.mul(&td).sub(&tn.mul(&tn))).div(&denom));
            let qy = cy.add(&r.mul(&Q::from_i128(2).mul(&tn).mul(&td)).div(&denom));
            let q = Point2::from_rat(qx, qy.clone());
            let above = qy.cmp(&cy); // Greater = upper, Less = lower, Equal = extremum
            for e in &edges {
                if let Edge::Arc(a) = e {
                    let expect = !matches!(
                        (a.half, above),
                        (Half::Upper, Ordering::Less) | (Half::Lower, Ordering::Greater)
                    );
                    prop_assert_eq!(on_arc(&q, a), expect);
                }
            }
        }

        /// End-to-end rigid-motion invariance of the *pipeline*: rotating (and
        /// translating) an input arc and a point, then decomposing, does not change
        /// whether the point is on the arc. A single decomposed piece is not
        /// rotation-covariant (its x-monotone chart is normatively axis-aligned),
        /// but `decompose ∘ membership` is — "on the arc" is a frame-independent
        /// point-set fact. This is the invariant that would actually bite if it
        /// failed, so it is checked directly.
        #[test]
        fn membership_pipeline_rigid_invariant(
            cx in -3i128..=3, cy in -3i128..=3, r in 1i128..=5,
            e1 in -5i128..=5, e1d in 1i128..=5,
            e2 in -5i128..=5, e2d in 1i128..=5,
            pt in -5i128..=5, ptd in 1i128..=5,
            u in -3i128..=3, v in -3i128..=3,
            mtx in -4i128..=4, mty in -4i128..=4,
            cw in any::<bool>(),
        ) {
            prop_assume!(u != 0 || v != 0);
            let (cx, cy, r) = (Q::from_i128(cx), Q::from_i128(cy), Q::from_i128(r));
            let on_c = |tn: i128, td: i128| on_circle_pt(&cx, &cy, &r, tn, td);
            let (a, b, p) = (on_c(e1, e1d), on_c(e2, e2d), on_c(pt, ptd));
            prop_assume!(a != b); // proper arc
            let orient = if cw { Orient::Cw } else { Orient::Ccw };
            let c = Circle { cx: cx.clone(), cy: cy.clone(), r2: r.mul(&r) };

            let m0 = member_of_any(&p, &decompose(&Curve::Arc {
                circle: c.clone(), start: a.clone(), end: b.clone(), orient, source: CurveId(0),
            }));

            // rigid image: move the centre (r² preserved) and every point.
            let m = rigid(u, v, mtx, mty);
            let c2 = rigid_circle(&c, &m);
            let rp2 = |q: &P| rigid_pt(q, &m);
            let m1 = member_of_any(&rp2(&p), &decompose(&Curve::Arc {
                circle: c2, start: rp2(&a), end: rp2(&b), orient, source: CurveId(0),
            }));

            prop_assert_eq!(m0, m1);
        }

        /// Arc membership is invariant under a rational TRANSLATION (which
        /// preserves the axis-aligned chart; a single decomposed piece is not
        /// rotation-covariant — see `membership_pipeline_rigid_invariant` for the
        /// end-to-end invariant that is).
        #[test]
        fn arc_membership_translation_invariant(
            cx in -3i128..=3, cy in -3i128..=3, r in 1i128..=5,
            tn in -5i128..=5, td in 1i128..=5,
            dx in -4i128..=4, dy in -4i128..=4,
        ) {
            let (cx, cy, r) = (Q::from_i128(cx), Q::from_i128(cy), Q::from_i128(r));
            let build = |cxx: Q, cyy: Q| {
                let c = Circle { cx: cxx, cy: cyy, r2: r.mul(&r) };
                decompose(&Curve::Circle { circle: c, orient: Orient::Ccw, source: CurveId(0) })
            };
            let (tn, td) = (Q::from_i128(tn), Q::from_i128(td));
            let denom = td.mul(&td).add(&tn.mul(&tn));
            let qx = cx.add(&r.mul(&td.mul(&td).sub(&tn.mul(&tn))).div(&denom));
            let qy = cy.add(&r.mul(&Q::from_i128(2).mul(&tn).mul(&td)).div(&denom));
            let (ddx, ddy) = (Q::from_i128(dx), Q::from_i128(dy));

            let e0 = build(cx.clone(), cy.clone());
            let q0 = Point2::from_rat(qx.clone(), qy.clone());
            let e1 = build(cx.add(&ddx), cy.add(&ddy));
            let q1 = Point2::from_rat(qx.add(&ddx), qy.add(&ddy));
            for (a, b) in e0.iter().zip(e1.iter()) {
                if let (Edge::Arc(a), Edge::Arc(b)) = (a, b) {
                    prop_assert_eq!(on_arc(&q0, a), on_arc(&q1, b));
                }
            }
        }
    }
}
