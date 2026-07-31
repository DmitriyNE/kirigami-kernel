//! The stage-2 1D coincidence lattice (M3 slice 3c). When two edges share a
//! CARRIER (coincident lines / arcs on one circle — the M3a spine's
//! `CarrierCoincident` branch), an exact 1D domain arrangement on that carrier
//! decides the normative outcome lattice (spec §6): `disjoint` ⇒ nothing;
//! `touch-at-point` ⇒ a vertex; `partial overlap` ⇒ one merged edge (both
//! operands) + the residual sub-edges; `containment`/`equality` as the degenerate
//! cases of that same form. The emitted primitives feed 3d's DCEL. Only
//! **distinct-source** pairs are coincidences — two pieces of one decomposed curve
//! are structure, not overlap.
//!
//! The angular question is handled in the §8.3 tag chart ([`crate::azimuth`]): a
//! decomposed arc piece is x-monotone on one half (winding 0 structurally), so on
//! a shared circle the tag order coincides with the x order per half, and the
//! per-half x-interval overlap *is* the tag-interval overlap of the two
//! non-wrapping pieces; cross-half pieces meet only at the shared x-extrema.

use core::cmp::Ordering;

use crate::decompose::extrema;
use crate::event::{CoincEdge, Operand};
use crate::witness::CoincOutcome;
use geom::content::{ArcPiece, CurveId, Edge, Point2, SegPiece};
use lattice::{Backend, Surd};

/// The 1D coincidence decision for one shared-carrier pair.
pub struct Coincidence<B: Backend> {
    /// Touch-at-point vertices (become `EventSet` vertices).
    pub touches: Vec<Point2<B>>,
    /// The merged + residual coincidence sub-edges.
    pub edges: Vec<CoincEdge<B>>,
    /// The replayable outcome class.
    pub outcome: CoincOutcome,
}

fn empty<B: Backend>(outcome: CoincOutcome) -> Coincidence<B> {
    Coincidence {
        touches: Vec::new(),
        edges: Vec::new(),
        outcome,
    }
}

/// Decide the 1D coincidence of a shared-carrier pair `(a, b)` from sources
/// `sources`. Same-source pairs (one decomposed curve) are skipped; a line and a
/// circle never share a carrier.
pub fn coincide<B: Backend>(
    sources: (CurveId, CurveId),
    a: &Edge<B>,
    b: &Edge<B>,
) -> Coincidence<B> {
    if sources.0 == sources.1 {
        return empty(CoincOutcome::SameSource);
    }
    match (a, b) {
        (Edge::Seg(sa), Edge::Seg(sb)) => seg_coincide(sources, sa, sb),
        (Edge::Arc(aa), Edge::Arc(ab)) => arc_coincide(sources, aa, ab),
        _ => empty(CoincOutcome::Disjoint), // a line and a circle share no carrier
    }
}

// A 1D endpoint: its position on the shared domain + the point it is.
struct End<B: Backend> {
    coord: Surd<B>,
    pt: Point2<B>,
}

fn order2<B: Backend>(x: End<B>, y: End<B>) -> (End<B>, End<B>) {
    if x.coord.cmp(&y.coord) == Ordering::Greater {
        (y, x)
    } else {
        (x, y)
    }
}

/// A tagged output span `(lo point, hi point, operand)` from the 1D overlap.
type Span<B> = (Point2<B>, Point2<B>, Operand);

/// The unified 1D overlap of two coord-sorted intervals `a=[a0,a1]`, `b=[b0,b1]`:
/// `overlap = [max(a0,b0), min(a1,b1)]`. Returns the touch points, the tagged
/// spans (`(lo, hi, operand)`, all endpoints drawn from the four inputs), and the
/// outcome class — covering disjoint / touch / equality / containment / partial.
fn overlap_1d<B: Backend>(
    a0: &End<B>,
    a1: &End<B>,
    b0: &End<B>,
    b1: &End<B>,
) -> (Vec<Point2<B>>, Vec<Span<B>>, CoincOutcome) {
    let lo = if a0.coord.cmp(&b0.coord) == Ordering::Less {
        b0
    } else {
        a0
    };
    let hi = if a1.coord.cmp(&b1.coord) == Ordering::Greater {
        b1
    } else {
        a1
    };
    match lo.coord.cmp(&hi.coord) {
        Ordering::Greater => (Vec::new(), Vec::new(), CoincOutcome::Disjoint),
        Ordering::Equal => (
            vec![lo.pt.clone()],
            Vec::new(),
            CoincOutcome::Touch { touches: 1 },
        ),
        Ordering::Less => {
            let mut spans = vec![(lo.pt.clone(), hi.pt.clone(), Operand::Both)];
            if a0.coord.cmp(&lo.coord) == Ordering::Less {
                spans.push((a0.pt.clone(), lo.pt.clone(), Operand::First));
            }
            if hi.coord.cmp(&a1.coord) == Ordering::Less {
                spans.push((hi.pt.clone(), a1.pt.clone(), Operand::First));
            }
            if b0.coord.cmp(&lo.coord) == Ordering::Less {
                spans.push((b0.pt.clone(), lo.pt.clone(), Operand::Second));
            }
            if hi.coord.cmp(&b1.coord) == Ordering::Less {
                spans.push((hi.pt.clone(), b1.pt.clone(), Operand::Second));
            }
            let residuals = spans.len() - 1;
            (
                Vec::new(),
                spans,
                CoincOutcome::Overlap {
                    touches: 0,
                    merged: 1,
                    residuals,
                },
            )
        }
    }
}

fn src_of(op: Operand, sources: (CurveId, CurveId)) -> CurveId {
    match op {
        Operand::Second => sources.1,
        _ => sources.0, // Both is provenance-shared; residual First is source 0
    }
}

fn seg_coincide<B: Backend>(
    sources: (CurveId, CurveId),
    a: &SegPiece<B>,
    b: &SegPiece<B>,
) -> Coincidence<B> {
    // A shared line is parametrized by x, unless vertical (b = 0), then by y.
    let vertical = a.line.b.is_zero();
    let end = |p: &Point2<B>| End {
        coord: if vertical { p.y.clone() } else { p.x.clone() },
        pt: p.clone(),
    };
    let (a0, a1) = order2(end(&a.start), end(&a.end));
    let (b0, b1) = order2(end(&b.start), end(&b.end));
    let (touches, spans, outcome) = overlap_1d(&a0, &a1, &b0, &b1);
    let edges = spans
        .into_iter()
        .map(|(lo, hi, op)| CoincEdge {
            edge: Edge::Seg(Box::new(SegPiece {
                line: a.line.clone(),
                start: lo,
                end: hi,
                orient: a.orient,
                source: src_of(op, sources),
            })),
            operand: op,
            sources,
        })
        .collect();
    Coincidence {
        touches,
        edges,
        outcome,
    }
}

/// Does the arc piece reach `pt` as one of its endpoints?
fn has_endpoint<B: Backend>(a: &ArcPiece<B>, pt: &Point2<B>) -> bool {
    &a.start == pt || &a.end == pt
}

fn arc_coincide<B: Backend>(
    sources: (CurveId, CurveId),
    a: &ArcPiece<B>,
    b: &ArcPiece<B>,
) -> Coincidence<B> {
    if a.half == b.half {
        // same half: x-monotone graphs, so overlap in the x-interval [x_lo, x_hi].
        let a0 = End {
            coord: a.x_lo.clone(),
            pt: a.start.clone(),
        };
        let a1 = End {
            coord: a.x_hi.clone(),
            pt: a.end.clone(),
        };
        let b0 = End {
            coord: b.x_lo.clone(),
            pt: b.start.clone(),
        };
        let b1 = End {
            coord: b.x_hi.clone(),
            pt: b.end.clone(),
        };
        let (touches, spans, outcome) = overlap_1d(&a0, &a1, &b0, &b1);
        let edges = spans
            .into_iter()
            .map(|(lo, hi, op)| CoincEdge {
                edge: Edge::Arc(Box::new(ArcPiece {
                    circle: a.circle.clone(),
                    half: a.half,
                    x_lo: lo.x.clone(),
                    x_hi: hi.x.clone(),
                    start: lo,
                    end: hi,
                    winding: a.winding.clone(),
                    source: src_of(op, sources),
                })),
                operand: op,
                sources,
            })
            .collect();
        Coincidence {
            touches,
            edges,
            outcome,
        }
    } else {
        // cross-half: upper and lower pieces meet only at shared x-extrema (y = cy).
        let (l, r) = extrema(&a.circle);
        let mut touches = Vec::new();
        if has_endpoint(a, &l) && has_endpoint(b, &l) {
            touches.push(l);
        }
        if has_endpoint(a, &r) && has_endpoint(b, &r) {
            touches.push(r);
        }
        let outcome = if touches.is_empty() {
            CoincOutcome::Disjoint
        } else {
            CoincOutcome::Touch {
                touches: touches.len(),
            }
        };
        Coincidence {
            touches,
            edges: Vec::new(),
            outcome,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geom::content::{ArcPiece, Circle, Half, Line, Orient, SegPiece, Winding};
    use lattice::{Bignum, Rat};

    type Q = Rat<Bignum>;
    type P = Point2<Bignum>;

    fn rp(x: i128, y: i128) -> P {
        Point2::from_rat(Q::from_i128(x), Q::from_i128(y))
    }
    fn ci(a: u32, b: u32) -> (CurveId, CurveId) {
        (CurveId(a), CurveId(b))
    }

    /// A segment on the shared line `y = 0`, from `x0` to `x1`.
    fn seg(x0: i128, x1: i128, src: u32) -> Edge<Bignum> {
        Edge::Seg(Box::new(SegPiece {
            line: Line {
                a: Q::from_i128(0),
                b: Q::from_i128(1),
                c: Q::from_i128(0),
            },
            start: rp(x0, 0),
            end: rp(x1, 0),
            orient: Orient::Ccw,
            source: CurveId(src),
        }))
    }

    fn c25() -> Circle<Bignum> {
        Circle {
            cx: Q::from_i128(0),
            cy: Q::from_i128(0),
            r2: Q::from_i128(25),
        }
    }
    fn xsurd(a: i128, b: i128) -> Surd<Bignum> {
        Surd::new(Q::from_i128(a), Q::from_i128(b), Q::from_i128(25)) // a + b·√25 = a + 5b
    }
    /// An arc piece on `c25` and `half`, from the left endpoint `(lx,ly)` to the
    /// right `(rx,ry)` (both exact on the circle); `x_lo/x_hi` are exact Surds.
    fn arc(half: Half, lx: i128, ly: i128, rx: i128, ry: i128, src: u32) -> Edge<Bignum> {
        // x_lo/x_hi: if an endpoint is an extremum (y=0) use ±√25, else the rational x.
        let sx = |x: i128, y: i128| {
            if y == 0 && x < 0 {
                xsurd(0, -1) // L = −√25
            } else if y == 0 && x > 0 {
                xsurd(0, 1) // R = +√25
            } else {
                Surd::from_rat(Q::from_i128(x))
            }
        };
        Edge::Arc(Box::new(ArcPiece {
            circle: c25(),
            half,
            x_lo: sx(lx, ly),
            x_hi: sx(rx, ry),
            start: rp(lx, ly),
            end: rp(rx, ry),
            winding: Winding {
                orient: Orient::Ccw,
                source_span: None,
            },
            source: CurveId(src),
        }))
    }

    fn merged_count(c: &Coincidence<Bignum>) -> usize {
        c.edges
            .iter()
            .filter(|e| e.operand == Operand::Both)
            .count()
    }
    fn residual_count(c: &Coincidence<Bignum>) -> usize {
        c.edges
            .iter()
            .filter(|e| e.operand != Operand::Both)
            .count()
    }

    // --- segment outcome lattice (all five) ---

    #[test]
    fn seg_disjoint() {
        let c = coincide(ci(0, 1), &seg(0, 2, 0), &seg(4, 6, 1));
        assert_eq!(c.outcome, CoincOutcome::Disjoint);
        assert!(c.touches.is_empty() && c.edges.is_empty());
    }

    #[test]
    fn seg_touch() {
        let c = coincide(ci(0, 1), &seg(0, 2, 0), &seg(2, 4, 1));
        assert_eq!(c.outcome, CoincOutcome::Touch { touches: 1 });
        assert_eq!(c.touches, vec![rp(2, 0)]);
        assert!(c.edges.is_empty());
    }

    #[test]
    fn seg_partial() {
        // [0,4] ∩ [2,6] = merged [2,4] + residual [0,2] (First) + [4,6] (Second).
        let c = coincide(ci(0, 1), &seg(0, 4, 0), &seg(2, 6, 1));
        assert!(matches!(
            c.outcome,
            CoincOutcome::Overlap {
                merged: 1,
                residuals: 2,
                ..
            }
        ));
        assert_eq!(merged_count(&c), 1);
        assert_eq!(residual_count(&c), 2);
        // one First residual, one Second residual
        assert_eq!(
            c.edges
                .iter()
                .filter(|e| e.operand == Operand::First)
                .count(),
            1
        );
        assert_eq!(
            c.edges
                .iter()
                .filter(|e| e.operand == Operand::Second)
                .count(),
            1
        );
    }

    #[test]
    fn seg_containment() {
        // [0,6] ⊇ [2,4]: merged [2,4] + two residuals, both First (the container).
        let c = coincide(ci(0, 1), &seg(0, 6, 0), &seg(2, 4, 1));
        assert!(matches!(
            c.outcome,
            CoincOutcome::Overlap {
                merged: 1,
                residuals: 2,
                ..
            }
        ));
        assert_eq!(
            c.edges
                .iter()
                .filter(|e| e.operand == Operand::First)
                .count(),
            2
        );
        assert_eq!(
            c.edges
                .iter()
                .filter(|e| e.operand == Operand::Second)
                .count(),
            0
        );
    }

    #[test]
    fn seg_equality() {
        let c = coincide(ci(0, 1), &seg(0, 4, 0), &seg(0, 4, 1));
        assert!(matches!(
            c.outcome,
            CoincOutcome::Overlap {
                merged: 1,
                residuals: 0,
                ..
            }
        ));
        assert_eq!(merged_count(&c), 1);
        assert_eq!(residual_count(&c), 0);
    }

    #[test]
    fn same_source_skipped() {
        // two pieces of one decomposed curve — not a coincidence
        let c = coincide(ci(0, 0), &seg(0, 4, 0), &seg(2, 6, 0));
        assert_eq!(c.outcome, CoincOutcome::SameSource);
        assert!(c.touches.is_empty() && c.edges.is_empty());
    }

    // --- arc outcome lattice ---

    #[test]
    fn arc_same_half_containment() {
        // upper semicircle [L,R] ⊇ upper sub-arc [(−3,4),(3,4)]: merged + 2 residuals (First).
        let big = arc(Half::Upper, -5, 0, 5, 0, 0);
        let small = arc(Half::Upper, -3, 4, 3, 4, 1);
        let c = coincide(ci(0, 1), &big, &small);
        assert!(matches!(
            c.outcome,
            CoincOutcome::Overlap {
                merged: 1,
                residuals: 2,
                ..
            }
        ));
        assert_eq!(
            c.edges
                .iter()
                .filter(|e| e.operand == Operand::First)
                .count(),
            2
        );
    }

    #[test]
    fn arc_same_half_equality() {
        let a = arc(Half::Upper, -3, 4, 3, 4, 0);
        let b = arc(Half::Upper, -3, 4, 3, 4, 1);
        let c = coincide(ci(0, 1), &a, &b);
        assert!(matches!(
            c.outcome,
            CoincOutcome::Overlap {
                merged: 1,
                residuals: 0,
                ..
            }
        ));
    }

    #[test]
    fn arc_cross_half_touch_at_extrema() {
        // upper + lower semicircles (distinct source) meet at both extrema L, R.
        let up = arc(Half::Upper, -5, 0, 5, 0, 0);
        let lo = arc(Half::Lower, -5, 0, 5, 0, 1);
        let c = coincide(ci(0, 1), &up, &lo);
        assert_eq!(c.outcome, CoincOutcome::Touch { touches: 2 });
        assert!(c.touches.contains(&rp(-5, 0)) && c.touches.contains(&rp(5, 0)));
        assert!(c.edges.is_empty());
    }

    #[test]
    fn arc_cross_half_no_shared_extrema_disjoint() {
        // an upper sub-arc (no extremum endpoint) vs a lower semicircle: no shared points.
        let up = arc(Half::Upper, -3, 4, 3, 4, 0);
        let lo = arc(Half::Lower, -5, 0, 5, 0, 1);
        let c = coincide(ci(0, 1), &up, &lo);
        assert_eq!(c.outcome, CoincOutcome::Disjoint);
    }

    // --- property: reassembly ---

    use crate::testgen::{on_line, rigid, rigid_pt};
    use proptest::prelude::*;

    /// Extract a segment coincidence edge's x-interval [lo, hi].
    fn seg_x_range(e: &Edge<Bignum>) -> (Surd<Bignum>, Surd<Bignum>) {
        match e {
            Edge::Seg(s) => {
                if s.start.x.cmp(&s.end.x) == Ordering::Greater {
                    (s.end.x.clone(), s.start.x.clone())
                } else {
                    (s.start.x.clone(), s.end.x.clone())
                }
            }
            Edge::Arc(_) => unreachable!(),
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// Over two random overlapping segments on y=0: the emitted merged +
        /// residual spans tile the union [min, max] exactly — contiguous, no gaps
        /// or double cover — and each span's endpoints lie on the shared line. The
        /// outcome is invariant under a rational rigid motion.
        #[test]
        fn seg_reassembly_and_rigid_invariant(
            a0 in -8i128..=8, a1 in -8i128..=8, b0 in -8i128..=8, b1 in -8i128..=8,
            u in -3i128..=3, v in -3i128..=3, tx in -4i128..=4, ty in -4i128..=4,
        ) {
            prop_assume!(a0 != a1 && b0 != b1);
            prop_assume!(u != 0 || v != 0);
            let (sa0, sa1) = (a0.min(a1), a0.max(a1));
            let (sb0, sb1) = (b0.min(b1), b0.max(b1));
            // require a genuine overlap so an outcome with edges is produced
            prop_assume!(sa0.max(sb0) < sa1.min(sb1));
            let c = coincide(ci(0, 1), &seg(sa0, sa1, 0), &seg(sb0, sb1, 1));

            // tiling: sort the spans by lo; they must be contiguous and cover [min, max].
            let mut ranges: Vec<(Surd<Bignum>, Surd<Bignum>)> =
                c.edges.iter().map(|ce| seg_x_range(&ce.edge)).collect();
            ranges.sort_by(|x, y| x.0.cmp(&y.0));
            let union_lo = Surd::from_rat(Q::from_i128(sa0.min(sb0)));
            let union_hi = Surd::from_rat(Q::from_i128(sa1.max(sb1)));
            prop_assert_eq!(ranges.first().unwrap().0.clone(), union_lo);
            prop_assert_eq!(ranges.last().unwrap().1.clone(), union_hi);
            for w in ranges.windows(2) {
                prop_assert_eq!(w[0].1.clone(), w[1].0.clone()); // contiguous, no gap/overlap
            }
            // every span endpoint is on the shared line
            let line = Line { a: Q::from_i128(0), b: Q::from_i128(1), c: Q::from_i128(0) };
            for e in &c.edges {
                if let Edge::Seg(s) = &e.edge {
                    prop_assert!(on_line(&line, &s.start) && on_line(&line, &s.end));
                }
            }

            // rigid-motion invariance of the outcome class
            let m = rigid(u, v, tx, ty);
            let rseg = |x0: i128, x1: i128, src: u32| {
                let (p0, p1) = (rigid_pt(&rp(x0, 0), &m), rigid_pt(&rp(x1, 0), &m));
                // the moved line through p0,p1
                Edge::Seg(Box::new(SegPiece {
                    line: crate::testgen::rigid_line(
                        &Line { a: Q::from_i128(0), b: Q::from_i128(1), c: Q::from_i128(0) }, &m),
                    start: p0, end: p1, orient: Orient::Ccw, source: CurveId(src),
                }))
            };
            let c2 = coincide(ci(0, 1), &rseg(sa0, sa1, 0), &rseg(sb0, sb1, 1));
            prop_assert_eq!(c.outcome, c2.outcome);
        }
    }
}
