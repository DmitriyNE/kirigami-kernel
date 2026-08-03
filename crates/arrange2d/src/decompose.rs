//! Canonical x-monotone decomposition. Split
//! every circle/arc into simple x-monotone pieces at its exact x-extremal points
//! (`cx ± √r²`); the axis-aligned tag chart makes the half-angle pole the x-min
//! extremal, so extremal splitting subsumes pole splitting; no `Edge` spans more
//! than one simple arc; winding stays provenance on the source. Corpus:
//! `cx_full_circle_edge`.
//!
//! Which x-extrema a CCW arc crosses is decided *without* materialising angles or
//! escalating past a single radical: both extrema sit on `y = cy`, so every
//! extremum-vs-endpoint orientation test collapses to a sign of `(y − cy)` (one
//! `Surd` sign) plus an exact `Surd` x-comparison — the whole enumeration is
//! branch tables over the two endpoints' halves.

use core::cmp::Ordering;
use geom::content::{ArcPiece, Circle, Curve, CurveId, Edge, Half, Orient, Point2, Winding};
use lattice::{Backend, Rat, Surd};

/// The two x-extremal points `L = (cx − √r², cy)` and `R = (cx + √r², cy)`; their
/// coordinates share the single radical `d = r²`.
pub(crate) fn extrema<B: Backend>(c: &Circle<B>) -> (Point2<B>, Point2<B>) {
    let y = Surd::from_rat(c.cy.clone());
    let l = Point2 {
        x: Surd::new(c.cx.clone(), Rat::from_i128(-1), c.r2.clone()),
        y: y.clone(),
    };
    let r = Point2 {
        x: Surd::new(c.cx.clone(), Rat::from_i128(1), c.r2.clone()),
        y,
    };
    (l, r)
}

/// A point's half relative to the centre line `y = cy`: `Greater` = upper,
/// `Less` = lower, `Equal` = an x-extremum.
fn half_ord<B: Backend>(p: &Point2<B>, cy: &Rat<B>) -> Ordering {
    p.y.cmp(&Surd::from_rat(cy.clone()))
}

/// Is `p` strictly right of centre (`x > cx`)? For an x-extremum this tells R from L.
fn is_right<B: Backend>(p: &Point2<B>, cx: &Rat<B>) -> bool {
    p.x.cmp(&Surd::from_rat(cx.clone())) == Ordering::Greater
}

/// The half a monotone piece whose CCW-start is `p` lies on: upper if `p` is above
/// centre or is R (CCW leaves R upward), lower if below or is L.
fn piece_half<B: Backend>(p: &Point2<B>, c: &Circle<B>) -> Half {
    match half_ord(p, &c.cy) {
        Ordering::Greater => Half::Upper,
        Ordering::Less => Half::Lower,
        Ordering::Equal if is_right(p, &c.cx) => Half::Upper,
        Ordering::Equal => Half::Lower,
    }
}

/// The x-extrema strictly interior to the CCW arc `a → b` (`a != b`, both on `c`),
/// in CCW order from `a`. A CCW arc crosses 0, 1, or 2 of `{L, R}`.
fn crossed_extrema<B: Backend>(
    c: &Circle<B>,
    a: &Point2<B>,
    b: &Point2<B>,
    l: &Point2<B>,
    r: &Point2<B>,
) -> Vec<Point2<B>> {
    use Ordering::{Equal, Greater, Less};
    let (ha, hb) = (half_ord(a, &c.cy), half_ord(b, &c.cy));
    let (lc, rc) = (|| (*l).clone(), || (*r).clone());
    match (ha, hb) {
        // both upper (CCW ⇒ x decreasing): stays upper iff b is ahead (b.x < a.x),
        // else wraps the long way across both extrema.
        (Greater, Greater) if b.x.cmp(&a.x) == Less => vec![],
        (Greater, Greater) => vec![lc(), rc()],
        // both lower (CCW ⇒ x increasing): stays lower iff b.x > a.x.
        (Less, Less) if b.x.cmp(&a.x) == Greater => vec![],
        (Less, Less) => vec![rc(), lc()],
        // opposite halves: exactly the one extremum between them.
        (Greater, Less) => vec![lc()],
        (Less, Greater) => vec![rc()],
        // one endpoint is itself an extremum.
        (Greater, Equal) if is_right(b, &c.cx) => vec![lc()], // → R: cross L first
        (Greater, Equal) => vec![],                           // → L directly
        (Less, Equal) if is_right(b, &c.cx) => vec![],        // → R directly
        (Less, Equal) => vec![rc()],                          // → L: cross R first
        (Equal, Greater) if is_right(a, &c.cx) => vec![],     // R → upper directly
        (Equal, Greater) => vec![rc()],                       // L → cross R
        (Equal, Less) if is_right(a, &c.cx) => vec![lc()],    // R → cross L
        (Equal, Less) => vec![],                              // L → lower directly
        // both endpoints extrema: a single semicircle, nothing interior.
        (Equal, Equal) => vec![],
    }
}

/// Build one x-monotone [`ArcPiece`] spanning the CCW step `p → q` (no extremum
/// strictly between). Endpoints are stored left-to-right (x-sorted); the CCW-start
/// `p` fixes the half.
fn arc_piece<B: Backend>(
    c: &Circle<B>,
    p: &Point2<B>,
    q: &Point2<B>,
    winding: &Winding<B>,
    source: CurveId,
) -> ArcPiece<B> {
    let half = piece_half(p, c);
    let (left, right) = if p.x.cmp(&q.x) == Ordering::Greater {
        (q, p)
    } else {
        (p, q)
    };
    ArcPiece {
        circle: (*c).clone(),
        half,
        x_lo: left.x.clone(),
        x_hi: right.x.clone(),
        start: (*left).clone(),
        end: (*right).clone(),
        winding: (*winding).clone(),
        source,
    }
}

/// Canonical decomposition of one input [`Curve`] into simple x-monotone [`Edge`]s
/// (spec-pending-v0.25 §1): a segment passes through; a whole circle splits into
/// its upper and lower semicircles; a proper arc splits at each x-extremum inside
/// its span. Winding is carried as provenance, never DCEL multiplicity.
pub fn decompose<B: Backend>(curve: &Curve<B>) -> Vec<Edge<B>> {
    match curve {
        Curve::Seg(seg) => vec![Edge::Seg(Box::new((*seg).clone()))],
        Curve::Circle {
            circle,
            orient,
            source,
        } => {
            let (l, r) = extrema(circle);
            let winding = Winding {
                orient: *orient,
                source_span: None,
            };
            // R → L (CCW) is the upper semicircle; L → R the lower.
            let upper = arc_piece(circle, &r, &l, &winding, *source);
            let lower = arc_piece(circle, &l, &r, &winding, *source);
            vec![Edge::Arc(Box::new(upper)), Edge::Arc(Box::new(lower))]
        }
        Curve::Arc {
            circle,
            start,
            end,
            orient,
            source,
        } => {
            // Normalise to CCW; a CW arc is the reversed CCW arc.
            let (a, b) = match orient {
                Orient::Ccw => (start, end),
                Orient::Cw => (end, start),
            };
            debug_assert!(a != b, "decompose: degenerate arc (start == end)");
            if a == b {
                return vec![];
            }
            let (l, r) = extrema(circle);
            let winding = Winding {
                orient: *orient,
                source_span: Some(((*start).clone(), (*end).clone())),
            };
            let breaks = crossed_extrema(circle, a, b, &l, &r);
            // Chain A → breaks → B; one monotone piece per consecutive pair.
            let mut chain: Vec<&Point2<B>> = Vec::with_capacity(breaks.len() + 2);
            chain.push(a);
            chain.extend(breaks.iter());
            chain.push(b);
            chain
                .windows(2)
                .map(|w| Edge::Arc(Box::new(arc_piece(circle, w[0], w[1], &winding, *source))))
                .collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testgen::{on_circle, on_circle_pt};
    use geom::content::{Line, SegPiece};
    use lattice::Bignum;

    type Q = Rat<Bignum>;
    type P = Point2<Bignum>;

    fn circle(cx: i128, cy: i128, r2: i128) -> Circle<Bignum> {
        Circle {
            cx: Q::from_i128(cx),
            cy: Q::from_i128(cy),
            r2: Q::from_i128(r2),
        }
    }
    fn rp(x: i128, y: i128) -> P {
        Point2::from_rat(Q::from_i128(x), Q::from_i128(y))
    }
    fn arc(c: &Circle<Bignum>, a: P, b: P, orient: Orient) -> Curve<Bignum> {
        Curve::Arc {
            circle: c.clone(),
            start: a,
            end: b,
            orient,
            source: CurveId(0),
        }
    }

    fn arcs(edges: &[Edge<Bignum>]) -> Vec<&ArcPiece<Bignum>> {
        edges
            .iter()
            .map(|e| match e {
                Edge::Arc(a) => a.as_ref(),
                Edge::Seg(_) => panic!("expected only arc pieces"),
            })
            .collect()
    }

    /// Corpus `cx_full_circle_edge`: a whole circle → exactly two x-monotone
    /// pieces (upper + lower), neither spanning the whole circle.
    #[test]
    fn cx_full_circle_edge() {
        let c = circle(0, 0, 1);
        let edges = decompose(&Curve::Circle {
            circle: c.clone(),
            orient: Orient::Ccw,
            source: CurveId(7),
        });
        assert_eq!(edges.len(), 2);
        let ps = arcs(&edges);
        // one upper, one lower
        let halves: Vec<Half> = ps.iter().map(|a| a.half).collect();
        assert!(halves.contains(&Half::Upper) && halves.contains(&Half::Lower));
        for a in &ps {
            assert!(
                a.x_lo < a.x_hi,
                "piece must be x-monotone, not a full circle"
            );
            assert_eq!(
                a.x_lo,
                Surd::new(Q::from_i128(0), Q::from_i128(-1), Q::from_i128(1))
            );
            assert_eq!(
                a.x_hi,
                Surd::new(Q::from_i128(0), Q::from_i128(1), Q::from_i128(1))
            );
            assert!(on_circle(&c, &a.start) && on_circle(&c, &a.end));
            assert_eq!(a.source, CurveId(7));
        }
    }

    /// Irrational-radicand circle: extrema are genuine `±√2` surds.
    #[test]
    fn full_circle_surd_extrema() {
        let c = circle(0, 0, 2);
        let edges = decompose(&Curve::Circle {
            circle: c.clone(),
            orient: Orient::Ccw,
            source: CurveId(0),
        });
        let ps = arcs(&edges);
        for a in &ps {
            // x_lo = −√2, x_hi = +√2
            assert_eq!(
                a.x_lo,
                Surd::new(Q::from_i128(0), Q::from_i128(-1), Q::from_i128(2))
            );
            assert_eq!(
                a.x_hi,
                Surd::new(Q::from_i128(0), Q::from_i128(1), Q::from_i128(2))
            );
            assert!(on_circle(&c, &a.start));
        }
    }

    /// r² = 25 ⇒ r = 5, extrema at (±5, 0); (3,4),(−3,4),(3,−4) are exact.
    fn c5() -> Circle<Bignum> {
        circle(0, 0, 25)
    }

    #[test]
    fn arc_zero_crossings_stays_upper() {
        // CCW (3,4) → (−3,4): x decreasing across the top, no extremum inside.
        let c = c5();
        let edges = decompose(&arc(&c, rp(3, 4), rp(-3, 4), Orient::Ccw));
        assert_eq!(edges.len(), 1);
        let a = arcs(&edges);
        assert_eq!(a[0].half, Half::Upper);
        assert_eq!(a[0].x_lo, Surd::from_rat(Q::from_i128(-3)));
        assert_eq!(a[0].x_hi, Surd::from_rat(Q::from_i128(3)));
    }

    #[test]
    fn arc_one_crossing_upper_to_lower() {
        // CCW (3,4) → (3,−4) the long way, crossing L at (−5,0).
        let c = c5();
        let edges = decompose(&arc(&c, rp(3, 4), rp(3, -4), Orient::Ccw));
        assert_eq!(edges.len(), 2);
        let a = arcs(&edges);
        assert_eq!(a[0].half, Half::Upper);
        assert_eq!(a[1].half, Half::Lower);
        // shared breakpoint is L = (−5, 0)
        assert_eq!(a[0].x_lo, Surd::from_rat(Q::from_i128(-5)));
        assert_eq!(a[1].x_lo, Surd::from_rat(Q::from_i128(-5)));
    }

    #[test]
    fn arc_two_crossings_wraps() {
        // CCW (−3,4) → (3,4) the long way (b.x > a.x, both upper): crosses L then R.
        let c = c5();
        let edges = decompose(&arc(&c, rp(-3, 4), rp(3, 4), Orient::Ccw));
        assert_eq!(edges.len(), 3);
        let a = arcs(&edges);
        assert_eq!(
            a.iter().map(|p| p.half).collect::<Vec<_>>(),
            vec![Half::Upper, Half::Lower, Half::Upper]
        );
        // middle piece is the full lower semicircle x ∈ [−5, 5]
        assert_eq!(a[1].x_lo, Surd::from_rat(Q::from_i128(-5)));
        assert_eq!(a[1].x_hi, Surd::from_rat(Q::from_i128(5)));
    }

    #[test]
    fn semicircle_endpoints_at_extrema() {
        let c = c5();
        // L → R CCW is the lower semicircle.
        let el = decompose(&arc(&c, rp(-5, 0), rp(5, 0), Orient::Ccw));
        let lower = arcs(&el);
        assert_eq!(lower.len(), 1);
        assert_eq!(lower[0].half, Half::Lower);
        // R → L CCW is the upper semicircle.
        let eu = decompose(&arc(&c, rp(5, 0), rp(-5, 0), Orient::Ccw));
        let upper = arcs(&eu);
        assert_eq!(upper.len(), 1);
        assert_eq!(upper[0].half, Half::Upper);
    }

    #[test]
    fn cw_is_reversed_ccw() {
        // CW (−3,4) → (3,4) is the short arc over the top: 1 upper piece,
        // unlike the CCW long way (3 pieces).
        let c = c5();
        let edges = decompose(&arc(&c, rp(-3, 4), rp(3, 4), Orient::Cw));
        assert_eq!(edges.len(), 1);
        assert_eq!(arcs(&edges)[0].half, Half::Upper);
    }

    #[test]
    fn segment_passes_through() {
        let seg = SegPiece {
            line: Line {
                a: Q::from_i128(0),
                b: Q::from_i128(1),
                c: Q::from_i128(0),
            },
            start: rp(-1, 0),
            end: rp(1, 0),
            orient: Orient::Ccw,
            source: CurveId(3),
        };
        let edges = decompose(&Curve::Seg(seg.clone()));
        assert_eq!(edges.len(), 1);
        match &edges[0] {
            Edge::Seg(s) => {
                assert_eq!(s.start, seg.start);
                assert_eq!(s.end, seg.end);
                assert_eq!(s.source, CurveId(3));
            }
            Edge::Arc(_) => panic!("segment must stay a segment"),
        }
    }

    // --- properties ---

    use proptest::prelude::*;

    fn points_eq(p: &P, q: &P) -> bool {
        p == q
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(96))]

        /// Over random proper arcs (exact rational endpoints on a rational-radius
        /// circle): every piece is a monotone arc on one half with endpoints
        /// exactly on the circle, and the pieces chain back into the source —
        /// consecutive pieces share exactly one boundary point, and that shared
        /// point is an x-extremum.
        #[test]
        fn arc_pieces_monotone_on_circle_and_reassemble(
            cx in -4i128..=4, cy in -4i128..=4, r in 1i128..=6,
            t1n in -6i128..=6, t1d in 1i128..=6,
            t2n in -6i128..=6, t2d in 1i128..=6,
            cw in any::<bool>(),
        ) {
            let (cx, cy, r) = (Q::from_i128(cx), Q::from_i128(cy), Q::from_i128(r));
            let a = on_circle_pt(&cx, &cy, &r, t1n, t1d);
            let b = on_circle_pt(&cx, &cy, &r, t2n, t2d);
            prop_assume!(!points_eq(&a, &b)); // proper arc
            let c = Circle { cx: cx.clone(), cy: cy.clone(), r2: r.mul(&r) };
            let orient = if cw { Orient::Cw } else { Orient::Ccw };
            let edges = decompose(&arc(&c, a.clone(), b.clone(), orient));
            let ps = arcs(&edges);
            prop_assert!(!ps.is_empty());

            for p in &ps {
                prop_assert!(p.x_lo < p.x_hi, "x-monotone, non-degenerate");
                prop_assert!(on_circle(&c, &p.start) && on_circle(&c, &p.end));
            }
            // consecutive pieces share exactly one boundary, an x-extremum
            let (l, r_ex) = extrema(&c);
            for w in ps.windows(2) {
                let u = [w[0].start.clone(), w[0].end.clone()];
                let v = [w[1].start.clone(), w[1].end.clone()];
                let mut shared: Vec<P> = Vec::new();
                for x in &u {
                    if v.iter().any(|y| x == y) {
                        shared.push(x.clone());
                    }
                }
                prop_assert_eq!(shared.len(), 1);
                let s = &shared[0];
                prop_assert!(points_eq(s, &l) || points_eq(s, &r_ex),
                    "internal breakpoints are x-extrema");
            }
            // the two unshared ends are exactly A and B
            let first = [ps[0].start.clone(), ps[0].end.clone()];
            let last = [ps[ps.len()-1].start.clone(), ps[ps.len()-1].end.clone()];
            prop_assert!(first.contains(&a) || last.contains(&a));
            prop_assert!(first.contains(&b) || last.contains(&b));
        }
    }
}
