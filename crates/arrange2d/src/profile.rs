//! `Profile` — author a closed 2-D outline without hand-building arrangement edges.
//!
//! An extruded cutter's profile ([`Cutter::extrude`](../../author/part/enum.Cutter.html)) is a
//! `Vec<Edge>`, and `Edge` is a *post-decomposition* type: a circle arrives as its two x-monotone
//! arcs, each carrying `x_lo`/`x_hi` extrema and `start`/`end` points that must agree with the
//! circle exactly. Building that by hand is error-prone busywork, and it was being redone — nearly
//! identically — in three crates' tests.
//!
//! This is **sugar over machinery that already exists**: each constructor builds a
//! [`Curve`] and hands it to [`decompose`], which is the canonical, tested decomposition. Nothing
//! here re-derives an arc.
//!
//! ```
//! use arrange2d::profile::Profile;
//! use lattice::{Bignum, Rat};
//! type Q = Rat<Bignum>;
//!
//! // A square with a circular bite taken out of it — even-odd fill, so the disc is a hole.
//! let p = Profile::<Bignum>::new()
//!     .rect(Q::from_i128(0), Q::from_i128(0), Q::from_i128(2), Q::from_i128(1))
//!     .circle_r2(Q::from_i128(0), Q::from_i128(0), Q::new(1, 40));
//! assert_eq!(p.edges().len(), 4 + 2); // 4 segments, and a circle is two x-monotone arcs
//! ```
//!
//! **Radii are squared, and that is not a restriction.** `r²` may be any non-negative rational —
//! `1/40` is fine, though `√(1/40)` is irrational — because an arc's extreme points are
//! [`Surd`]s, `cx ± 1·√(r²)`, which represent that exactly. [`circle`](Profile::circle) is the
//! sugar for a rational radius, not the other way round.

use crate::decompose::decompose;
use geom::content::{Circle, Curve, CurveId, Edge, Line, Orient, Point2, SegPiece};
use lattice::{Backend, Bignum, Rat};

/// A closed 2-D outline, as the arrangement edges the cutter path consumes.
///
/// Shapes accumulate; the fill rule is the region's own **even-odd** parity, so a shape drawn
/// inside another is a hole with no decomposition and no ordering requirement. Constructors are
/// total — a degenerate outline (a repeated polygon vertex, a zero radius) is not rejected here
/// but surfaces downstream as a typed fault, matching the builder discipline used by `Part`.
pub struct Profile<B: Backend = Bignum> {
    edges: Vec<Edge<B>>,
    /// Next [`CurveId`], so each added shape is distinguishable in the arrangement.
    next: u32,
}

impl<B: Backend> Default for Profile<B> {
    fn default() -> Self {
        Self::new()
    }
}

impl<B: Backend> Profile<B> {
    /// An empty outline.
    pub fn new() -> Self {
        Profile {
            edges: Vec::new(),
            next: 0,
        }
    }

    /// The accumulated arrangement edges.
    pub fn edges(&self) -> &[Edge<B>] {
        &self.edges
    }

    /// Consume the profile into the edge list a cutter takes.
    pub fn into_edges(self) -> Vec<Edge<B>> {
        self.edges
    }

    /// Add every edge of `curve`'s canonical decomposition.
    fn push(mut self, curve: Curve<B>) -> Self {
        self.edges.extend(decompose(&curve));
        self.next += 1;
        self
    }

    /// Add the whole circle of **squared** radius `r2` about `(cx, cy)`.
    ///
    /// `r2` need not be a rational square: the two x-extrema are `cx ± √r2` as [`Surd`]s, exact
    /// for any non-negative rational.
    pub fn circle_r2(self, cx: Rat<B>, cy: Rat<B>, r2: Rat<B>) -> Self {
        let source = CurveId(self.next);
        self.push(Curve::Circle {
            circle: Circle { cx, cy, r2 },
            orient: Orient::Ccw,
            source,
        })
    }

    /// The circle of radius `r` — sugar for [`circle_r2`](Self::circle_r2) with `r²`.
    pub fn circle(self, cx: Rat<B>, cy: Rat<B>, r: Rat<B>) -> Self {
        let r2 = r.mul(&r);
        self.circle_r2(cx, cy, r2)
    }

    /// The counter-clockwise arc of the circle of **squared** radius `r2` about `(cx, cy)`, from
    /// `start` to `end` — both of which must lie on that circle.
    ///
    /// The companion to [`polyline`](Self::polyline): together they draw an outline that mixes
    /// straight runs with round ones — a keyhole, a slot with rounded ends — without reaching for
    /// [`edges_raw`](Self::edges_raw). Sugar, like the rest: the arc is handed to the same
    /// [`decompose`], which splits it at whichever x-extremum its span crosses.
    ///
    /// Endpoints are rational, so the two must satisfy `(x − cx)² + (y − cy)² = r²` exactly. That
    /// is not the restriction it sounds like — a Pythagorean split does it, as `r² = 1/100` with
    /// the chord at `y = cy ± 8/100` meeting the circle at `x = cx ± 6/100`.
    ///
    /// Total, like every constructor here: `start == end` decomposes to nothing (a whole circle is
    /// [`circle_r2`](Self::circle_r2)), and endpoints off the circle are emitted as drawn.
    pub fn arc(
        self,
        cx: Rat<B>,
        cy: Rat<B>,
        r2: Rat<B>,
        start: [Rat<B>; 2],
        end: [Rat<B>; 2],
    ) -> Self {
        let source = CurveId(self.next);
        let [sx, sy] = start;
        let [ex, ey] = end;
        self.push(Curve::Arc {
            circle: Circle { cx, cy, r2 },
            start: Point2::from_rat(sx, sy),
            end: Point2::from_rat(ex, ey),
            orient: Orient::Ccw,
            source,
        })
    }

    /// Add a closed polygon through `pts`, in order, closing the last point back to the first.
    ///
    /// Total, like every constructor here: a degenerate outline (fewer than three points, or a
    /// repeated vertex) is emitted as drawn and faults downstream — a builder that silently
    /// dropped the shape would be the worse failure, since nothing downstream could then tell
    /// that anything was asked for.
    pub fn polygon(self, pts: &[[Rat<B>; 2]]) -> Self {
        self.chain(pts, true)
    }

    /// Add the **open** chain through `pts` — every consecutive pair, and no closing edge.
    ///
    /// What [`polygon`](Self::polygon) is for a shape drawn entirely in straight lines, this is for
    /// the straight part of one that is not: the outline is closed by the [`arc`](Self::arc) whose
    /// two ends the chain runs between.
    pub fn polyline(self, pts: &[[Rat<B>; 2]]) -> Self {
        self.chain(pts, false)
    }

    /// The shared body of [`polygon`] and [`polyline`]: consecutive pairs of `pts`, plus the
    /// wrap-around pair when `closed`.
    fn chain(mut self, pts: &[[Rat<B>; 2]], closed: bool) -> Self {
        let n = pts.len();
        let last = if closed { n } else { n.saturating_sub(1) };
        let source = CurveId(self.next);
        for i in 0..last {
            let (s, e) = (&pts[i], &pts[(i + 1) % n]);
            let ((sx, sy), (ex, ey)) = ((&s[0], &s[1]), (&e[0], &e[1]));
            // The line through the two endpoints: `a·x + b·y + c = 0` with the standard
            // normal `(sy − ey, ex − sx)`, then `c` fixed so the start point satisfies it.
            let (a, b) = (sy.sub(ey), ex.sub(sx));
            let c = a.mul(sx).add(&b.mul(sy)).neg();
            self.edges.extend(decompose(&Curve::Seg(SegPiece {
                line: Line { a, b, c },
                start: Point2::from_rat(sx.clone(), sy.clone()),
                end: Point2::from_rat(ex.clone(), ey.clone()),
                orient: Orient::Ccw,
                source,
            })));
        }
        self.next += 1;
        self
    }

    /// The axis-aligned rectangle of half-width `hw` and half-height `hh` about `(cx, cy)`.
    pub fn rect(self, cx: Rat<B>, cy: Rat<B>, hw: Rat<B>, hh: Rat<B>) -> Self {
        let pts = [
            [cx.sub(&hw), cy.sub(&hh)],
            [cx.add(&hw), cy.sub(&hh)],
            [cx.add(&hw), cy.add(&hh)],
            [cx.sub(&hw), cy.add(&hh)],
        ];
        self.polygon(&pts)
    }

    /// Add already-decomposed edges — the escape hatch for an outline this vocabulary cannot
    /// draw (one produced by a boolean, say, whose endpoints are algebraic).
    pub fn edges_raw(mut self, edges: impl IntoIterator<Item = Edge<B>>) -> Self {
        self.edges.extend(edges);
        self.next += 1;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locate::winding_parity;

    type Q = Rat<Bignum>;

    /// Is `(x, y)` inside the profile, by the arrangement's own even-odd rule?
    fn inside(p: &Profile<Bignum>, x: Q, y: Q) -> bool {
        winding_parity(&x, &y, p.edges())
    }

    /// A circle decomposes into exactly the two x-monotone arcs, and its fill is right —
    /// with a squared radius that is **not** a rational square, which is the whole point.
    #[test]
    fn a_circle_of_irrational_radius_is_exact() {
        // r² = 1/40 → r ≈ 0.158, irrational.
        let p = Profile::<Bignum>::new().circle_r2(Q::from_i128(0), Q::new(11, 5), Q::new(1, 40));
        assert_eq!(p.edges().len(), 2, "a circle is two x-monotone arcs");
        assert!(inside(&p, Q::new(1, 100), Q::new(221, 100)), "near-centre");
        assert!(
            !inside(&p, Q::from_i128(2), Q::new(221, 100)),
            "far outside"
        );
    }

    /// `circle` is sugar: the same outline as `circle_r2` with `r²`.
    #[test]
    fn circle_is_circle_r2_of_the_squared_radius() {
        let by_r = Profile::<Bignum>::new().circle(Q::from_i128(0), Q::from_i128(0), Q::new(1, 5));
        let by_r2 =
            Profile::<Bignum>::new().circle_r2(Q::from_i128(0), Q::from_i128(0), Q::new(1, 25));
        assert_eq!(by_r.edges().len(), by_r2.edges().len());
        for (x, y) in [(1, 10), (3, 10), (0, 0)] {
            let (px, py) = (Q::new(x, 10), Q::new(y, 10));
            assert_eq!(
                inside(&by_r, px.clone(), py.clone()),
                inside(&by_r2, px, py),
                "the two spellings must fill identically"
            );
        }
    }

    /// A rectangle fills as drawn, and a shape inside another reads as a hole under even-odd —
    /// so a holed outline needs no decomposition and no ordering.
    #[test]
    fn even_odd_makes_an_inner_shape_a_hole() {
        let ring = Profile::<Bignum>::new()
            .rect(
                Q::from_i128(0),
                Q::from_i128(0),
                Q::from_i128(2),
                Q::from_i128(2),
            )
            .circle(Q::from_i128(0), Q::from_i128(0), Q::from_i128(1));
        assert_eq!(ring.edges().len(), 4 + 2);
        // Between the circle and the rectangle: material. Inside the circle: a hole.
        assert!(inside(&ring, Q::new(3, 2), Q::new(1, 7)), "in the ring");
        assert!(!inside(&ring, Q::new(1, 7), Q::new(1, 13)), "in the hole");
        assert!(!inside(&ring, Q::from_i128(3), Q::new(1, 7)), "outside");
    }

    /// **A keyhole** — the shape that needs [`arc`](Profile::arc) and
    /// [`polyline`](Profile::polyline) together, since neither a polygon nor a whole circle can
    /// draw it: a round head of radius `1/10` with a straight stem hanging off the chord at
    /// `y = −2/25`, where the sides `x = ±3/50` meet the circle exactly (`6² + 8² = 10²`).
    ///
    /// The fill is checked at the point that distinguishes a keyhole from the *union* of a disc
    /// and a rectangle drawn as two shapes: the **notch** beside the stem, which even-odd would
    /// have filled and the single traversed outline leaves empty.
    #[test]
    fn an_arc_and_a_polyline_close_one_keyhole_outline() {
        let (r2, hw, chord, foot) = (Q::new(1, 100), Q::new(3, 50), Q::new(2, 25), Q::new(1, 5));
        let p = Profile::<Bignum>::new()
            .arc(
                Q::from_i128(0),
                Q::from_i128(0),
                r2,
                [hw.clone(), chord.clone().neg()],
                [hw.clone().neg(), chord.clone().neg()],
            )
            .polyline(&[
                [hw.clone().neg(), chord.clone().neg()],
                [hw.clone().neg(), foot.clone().neg()],
                [hw.clone(), foot.neg()],
                [hw, chord.neg()],
            ]);
        // The major arc crosses both x-extrema, so it decomposes into three monotone pieces; the
        // open chain contributes its three sides and no closing edge.
        assert_eq!(p.edges().len(), 3 + 3);
        // Sampled off `y = 0` and off `y = −r`: the ray cast is exact but needs a generic row, and
        // the circle's centre and its bottom tangent are the two rows that are not.
        assert!(inside(&p, Q::from_i128(0), Q::new(1, 20)), "the head");
        assert!(inside(&p, Q::from_i128(0), Q::new(-3, 20)), "the stem");
        assert!(
            !inside(&p, Q::new(3, 40), Q::new(-9, 100)),
            "the notch beside the stem — outside the circle, outside the stem"
        );
        assert!(
            !inside(&p, Q::from_i128(0), Q::new(-3, 10)),
            "past the foot"
        );
    }

    /// Total, not validating: a degenerate outline is emitted as drawn, so the downstream typed
    /// fault can see it. Silently dropping it would leave nothing to refuse.
    #[test]
    fn a_degenerate_polygon_is_emitted_not_dropped() {
        let p = Profile::<Bignum>::new().polygon(&[
            [Q::from_i128(0), Q::from_i128(0)],
            [Q::from_i128(1), Q::from_i128(0)],
        ]);
        assert_eq!(
            p.edges().len(),
            2,
            "both directed sides, for the fault to find"
        );
    }
}
