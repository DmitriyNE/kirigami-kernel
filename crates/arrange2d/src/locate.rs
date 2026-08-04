//! Exact point-location by horizontal ray-casting.
//!
//! The region rebuild and CAP-OUT assembly need to answer "is this point inside the
//! region?" and "how many A- / B-curves enclose this point?" exactly, over lines and
//! circles. This module is the primitive layer: where a horizontal ray `y = y0`
//! crosses an [`Edge`], and the even-odd crossing parity that follows.
//!
//! **The arc crossing lives in a new radical.** A ray `y = y0` (rational) meets the
//! circle `(x−cx)² + (y0−cy)² = r2` at `x = cx ± √disc`, `disc = r2 − (y0−cy)²` — a
//! [`Surd`] in the radical `d = disc`, which is *not* the vertex radical `Δ`, so the
//! `try_surd` carrier idiom does not apply here. We compare these crossings with
//! the rational query `x` (and the arc's `[x_lo, x_hi]`) through the cross-radical
//! [`Surd::cmp`], which is exact for differing radicals.
//!
//! **Genericity precondition (documented, satisfiable).** The parity count is exact
//! when the ray height `y0` avoids every circle centre `cy` and every arrangement
//! vertex `y` — the standard ray-casting non-degeneracy. At `y0 = cy` a ray grazes
//! the two shared x-extrema of a circle's upper/lower pieces (a double-count trap);
//! at a vertex height it grazes a shared endpoint. Both are a finite set, so a
//! generic rational `y0` always exists — and the seeder *chooses* its query
//! points, so it simply picks generic ones. This module therefore skips the
//! `y0 = cy` row (and, for segments, uses the straddle rule that counts a shared
//! vertex once); callers must respect the precondition.

use core::cmp::Ordering;
use geom::content::{ArcPiece, Edge, Half, SegPiece};
use lattice::{Backend, Rat, Surd};

/// Where the horizontal ray `y = y0` crosses the arc piece `arc`: the x-coordinates
/// `cx ± √disc` (`disc = r2 − (y0−cy)²`) that fall on the piece's half and within
/// `[x_lo, x_hi]`. A full upper/lower half is crossed **twice** (the graph is not
/// y-monotone). Returns empty when the ray misses the circle, grazes an apex
/// (`disc = 0`), or runs along the `y0 = cy` extremum row (a grazing endpoint row —
/// excluded by the genericity precondition; see the module doc).
pub fn ray_x_arc<B: Backend>(y0: &Rat<B>, arc: &ArcPiece<B>) -> Vec<Surd<B>> {
    let dy = y0.sub(&arc.circle.cy);
    // The ray must strike the interior of this half strictly: an Upper piece
    // (y ≥ cy) only at y0 > cy, a Lower piece only at y0 < cy.
    match (arc.half, dy.sign()) {
        (Half::Upper, s) if s <= 0 => return Vec::new(),
        (Half::Lower, s) if s >= 0 => return Vec::new(),
        _ => {}
    }
    let disc = arc.circle.r2.sub(&dy.mul(&dy));
    if disc.sign() <= 0 {
        return Vec::new(); // no real crossing, or a grazing apex tangency
    }
    let cx = arc.circle.cx.clone();
    let cands = [
        Surd::new(cx.clone(), Rat::from_i128(1), disc.clone()), // cx + √disc
        Surd::new(cx, Rat::from_i128(-1), disc),                // cx − √disc
    ];
    cands
        .into_iter()
        .filter(|x| in_closed_range(x, &arc.x_lo, &arc.x_hi))
        .collect()
}

/// Where the horizontal ray `y = y0` crosses the segment `seg`, as a **rational** x
/// (the carrier line and `y0` are both rational). Uses the even-odd **straddle
/// rule** — the segment is crossed iff exactly one endpoint is strictly above `y0`
/// — so a vertex shared by two segments is counted once and a horizontal segment
/// (which never straddles) contributes nothing.
pub fn ray_x_seg<B: Backend>(y0: &Rat<B>, seg: &SegPiece<B>) -> Option<Rat<B>> {
    let s = Surd::from_rat(y0.clone());
    let start_above = seg.start.y.cmp(&s) == Ordering::Greater;
    let end_above = seg.end.y.cmp(&s) == Ordering::Greater;
    if start_above == end_above {
        return None; // does not straddle y0 (covers the horizontal case)
    }
    // Straddling ⇒ non-horizontal ⇒ a ≠ 0; solve a·x + b·y0 + c = 0 for x.
    debug_assert!(
        seg.line.a.sign() != 0,
        "straddling segment must be non-horizontal"
    );
    let numer = seg.line.b.mul(y0).add(&seg.line.c); // b·y0 + c
    Some(numer.neg().div(&seg.line.a)) // −(b·y0 + c) / a
}

/// Even-odd point-in-region test: is `(qx, qy)` enclosed by the boundary `edges`?
/// Casts the rightward ray from `(qx, qy)` and returns whether the number of
/// crossings strictly to the right (`x > qx`) is odd. `qy` must satisfy the module's
/// genericity precondition. Pass the subset of edges bounding one operand to read
/// that operand's enclosure bit.
pub fn winding_parity<B: Backend>(qx: &Rat<B>, qy: &Rat<B>, edges: &[Edge<B>]) -> bool {
    let qxs = Surd::from_rat(qx.clone());
    let mut n = 0usize;
    for e in edges {
        match e {
            Edge::Seg(s) => {
                if let Some(x) = ray_x_seg(qy, s) {
                    if x.cmp(qx) == Ordering::Greater {
                        n += 1;
                    }
                }
            }
            Edge::Arc(a) => {
                for x in ray_x_arc(qy, a) {
                    if qxs.cmp(&x) == Ordering::Less {
                        n += 1;
                    }
                }
            }
        }
    }
    n % 2 == 1
}

/// Inclusive `min(a, b) ≤ v ≤ max(a, b)`, cross-radical-safe. (The arc-range filter;
/// `membership::between` is the same idea but private to that module.)
fn in_closed_range<B: Backend>(v: &Surd<B>, a: &Surd<B>, b: &Surd<B>) -> bool {
    let (lo, hi) = if a.cmp(b) == Ordering::Greater {
        (b, a)
    } else {
        (a, b)
    };
    v.cmp(lo) != Ordering::Less && v.cmp(hi) != Ordering::Greater
}

/// **Strict** `min(a, b) < v < max(a, b)`, cross-radical-safe — the open-interval
/// companion of the inclusive test, for "strictly interior" checks (3e seeding).
pub fn strict_between<B: Backend>(v: &Surd<B>, a: &Surd<B>, b: &Surd<B>) -> bool {
    let (lo, hi) = if a.cmp(b) == Ordering::Greater {
        (b, a)
    } else {
        (a, b)
    };
    v.cmp(lo) == Ordering::Greater && v.cmp(hi) == Ordering::Less
}

/// A rational strictly greater than the surd `s`, found by doubling a bracket. Uses
/// only the public cross-radical [`Surd::cmp`] — no access to `s`'s internals.
pub fn rational_above<B: Backend>(s: &Surd<B>) -> Rat<B> {
    let mut step = Rat::from_i128(1);
    let mut b = Rat::from_i128(0);
    while s.cmp(&Surd::from_rat(b.clone())) != Ordering::Less {
        b = b.add(&step);
        step = step.add(&step);
    }
    b
}

/// A rational strictly less than the surd `s` (companion of [`rational_above`]).
pub fn rational_below<B: Backend>(s: &Surd<B>) -> Rat<B> {
    let mut step = Rat::from_i128(1);
    let mut a = Rat::from_i128(0);
    while s.cmp(&Surd::from_rat(a.clone())) != Ordering::Greater {
        a = a.sub(&step);
        step = step.add(&step);
    }
    a
}

/// A rational strictly between the surds `lo < hi` — used to pick generic scanline
/// heights and interior sample x's for the slab decomposition. Brackets `[a, b]`
/// with `a < lo`, `b > hi` (via
/// [`rational_below`]/[`rational_above`]) then bisects until the rational midpoint
/// lands in the open gap `(lo, hi)`; terminates because the gap has positive width
/// and the bracket halves each step. Requires `lo < hi`.
pub fn rational_between<B: Backend>(lo: &Surd<B>, hi: &Surd<B>) -> Rat<B> {
    debug_assert!(
        lo.cmp(hi) == Ordering::Less,
        "rational_between needs lo < hi"
    );
    let mut a = rational_below(lo);
    let mut b = rational_above(hi);
    let two = Rat::from_i128(2);
    loop {
        let m = a.add(&b).div(&two);
        let ms = Surd::from_rat(m.clone());
        if lo.cmp(&ms) != Ordering::Less {
            a = m; // m ≤ lo
        } else if hi.cmp(&ms) != Ordering::Greater {
            b = m; // m ≥ hi
        } else {
            return m; // lo < m < hi
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decompose::decompose;
    use crate::testgen::{rigid, rigid_circle, rigid_pt};
    use geom::content::{Circle, Curve, CurveId, Line, Orient, Point2, SegPiece};
    use lattice::{Bignum, Rat};

    type Q = Rat<Bignum>;

    fn q(n: i128, d: i128) -> Q {
        Q::new(n, d)
    }
    fn qi(n: i128) -> Q {
        Q::from_i128(n)
    }

    /// A whole circle decomposed into its two x-monotone half-arcs.
    fn disk(cx: i128, cy: i128, r2: i128, src: u32) -> Vec<Edge<Bignum>> {
        decompose(&Curve::Circle {
            circle: Circle {
                cx: qi(cx),
                cy: qi(cy),
                r2: qi(r2),
            },
            orient: Orient::Ccw,
            source: CurveId(src),
        })
    }

    // --- ray ∩ arc ---

    #[test]
    fn ray_crosses_upper_half_twice() {
        // Unit circle at origin; ray at y = 1/2 (generic, ≠ cy = 0).
        let edges = disk(0, 0, 1, 0);
        let upper: &ArcPiece<Bignum> = edges
            .iter()
            .find_map(|e| match e {
                Edge::Arc(a) if a.half == Half::Upper => Some(a.as_ref()),
                _ => None,
            })
            .unwrap();
        let xs = ray_x_arc(&q(1, 2), upper);
        assert_eq!(xs.len(), 2, "a horizontal ray meets the upper half twice");
        // x = ±√(1 − 1/4) = ±√(3)/2 = ±√(3/4).
        let expect = Surd::new(qi(0), qi(1), q(3, 4));
        assert!(xs.contains(&expect));
        assert!(xs.contains(&expect.neg()));
    }

    #[test]
    fn ray_misses_wrong_half_and_apex() {
        let edges = disk(0, 0, 1, 0);
        let lower: &ArcPiece<Bignum> = edges
            .iter()
            .find_map(|e| match e {
                Edge::Arc(a) if a.half == Half::Lower => Some(a.as_ref()),
                _ => None,
            })
            .unwrap();
        // y = 1/2 is on the upper side → the lower piece is not met.
        assert!(ray_x_arc(&q(1, 2), lower).is_empty());
        let upper: &ArcPiece<Bignum> = edges
            .iter()
            .find_map(|e| match e {
                Edge::Arc(a) if a.half == Half::Upper => Some(a.as_ref()),
                _ => None,
            })
            .unwrap();
        // y = 1 is the apex (disc = 0) → grazing, not counted.
        assert!(ray_x_arc(&qi(1), upper).is_empty());
        // y = 2 is outside the circle entirely.
        assert!(ray_x_arc(&qi(2), upper).is_empty());
    }

    // --- ray ∩ seg ---

    fn seg(sx: i128, sy: i128, ex: i128, ey: i128, a: i128, b: i128, c: i128) -> SegPiece<Bignum> {
        SegPiece {
            line: Line {
                a: qi(a),
                b: qi(b),
                c: qi(c),
            },
            start: Point2::from_rat(qi(sx), qi(sy)),
            end: Point2::from_rat(qi(ex), qi(ey)),
            orient: Orient::Ccw,
            source: CurveId(0),
        }
    }

    #[test]
    fn ray_crosses_slanted_segment() {
        // Segment from (0,0) to (2,2) on the line x − y = 0 (a=1, b=−1, c=0).
        let s = seg(0, 0, 2, 2, 1, -1, 0);
        // y = 1 straddles ⇒ x = 1.
        assert_eq!(ray_x_seg(&qi(1), &s), Some(qi(1)));
        // y = 3 is above both endpoints ⇒ no crossing.
        assert_eq!(ray_x_seg(&qi(3), &s), None);
    }

    #[test]
    fn ray_skips_horizontal_segment() {
        // Horizontal segment on y = 1 (a=0, b=1, c=−1): never straddles.
        let s = seg(0, 1, 4, 1, 0, 1, -1);
        assert_eq!(ray_x_seg(&qi(1), &s), None);
        assert_eq!(ray_x_seg(&qi(0), &s), None);
    }

    // --- point-in-region parity ---

    #[test]
    fn point_in_disk() {
        let d = disk(0, 0, 4, 0); // radius-2 disk
        // generic y = 1/3 (≠ cy = 0)
        assert!(
            winding_parity(&qi(0), &q(1, 3), &d),
            "centre-ish point is inside"
        );
        assert!(winding_parity(&qi(1), &q(1, 3), &d), "(1,1/3) inside");
        assert!(
            !winding_parity(&qi(3), &q(1, 3), &d),
            "(3,1/3) outside to the right"
        );
        assert!(
            !winding_parity(&qi(-3), &q(1, 3), &d),
            "(-3,1/3) outside to the left"
        );
    }

    #[test]
    fn point_in_annulus() {
        // Outer r=3 (r2=9), inner r=1 (r2=1), concentric at origin.
        let mut edges = disk(0, 0, 9, 0);
        edges.extend(disk(0, 0, 1, 1));
        let yy = q(1, 7); // generic
        // (0,1/7): inside the inner hole ⇒ NOT in the annulus (2 outer + 2 inner
        // crossings to consider; to the right: 1 outer + 1 inner = even ⇒ outside).
        assert!(
            !winding_parity(&qi(0), &yy, &edges),
            "hole centre is outside the annulus"
        );
        // (2,1/7): between inner and outer ⇒ inside the annulus (1 outer crossing right).
        assert!(
            winding_parity(&qi(2), &yy, &edges),
            "annulus body is inside"
        );
        // (4,1/7): outside the outer ⇒ outside.
        assert!(
            !winding_parity(&qi(4), &yy, &edges),
            "beyond the outer is outside"
        );
    }

    // --- rational-between primitives ---

    #[test]
    fn rational_strictly_between_surds() {
        let r2 = Surd::<Bignum>::new(qi(0), qi(1), qi(2)); // √2 ≈ 1.414
        let r3 = Surd::<Bignum>::new(qi(0), qi(1), qi(3)); // √3 ≈ 1.732
        let m = rational_between(&r2, &r3);
        assert_eq!(r2.cmp(&Surd::from_rat(m.clone())), Ordering::Less);
        assert_eq!(r3.cmp(&Surd::from_rat(m.clone())), Ordering::Greater);

        // Above / below a single surd.
        let a = rational_above(&r2);
        let b = rational_below(&r2);
        assert_eq!(r2.cmp(&Surd::from_rat(a)), Ordering::Less);
        assert_eq!(r2.cmp(&Surd::from_rat(b)), Ordering::Greater);
    }

    #[test]
    fn rational_between_close_surds() {
        // Two nearby irrationals: √(200) and √(201) (≈14.142 vs 14.177).
        let lo = Surd::<Bignum>::new(qi(0), qi(1), qi(200));
        let hi = Surd::<Bignum>::new(qi(0), qi(1), qi(201));
        let m = rational_between(&lo, &hi);
        assert_eq!(lo.cmp(&Surd::from_rat(m.clone())), Ordering::Less);
        assert_eq!(hi.cmp(&Surd::from_rat(m)), Ordering::Greater);
    }

    // --- properties ---

    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(96))]

        /// Point-in-disk membership is invariant under a rational rigid motion:
        /// whether a rational point is inside a disk is a frame-independent fact,
        /// and `decompose ∘ winding_parity` preserves it. The query height is kept
        /// generic (≠ cy) on both sides by construction.
        #[test]
        fn point_in_disk_rigid_invariant(
            cx in -3i128..=3, cy in -3i128..=3, r in 1i128..=5,
            px in -8i128..=8, py in -8i128..=8, pyd in 2i128..=6,
            u in -3i128..=3, v in -3i128..=3,
            mtx in -4i128..=4, mty in -4i128..=4,
        ) {
            prop_assume!(u != 0 || v != 0);
            let c = Circle { cx: qi(cx), cy: qi(cy), r2: qi(r * r) };
            // Query point with a generic, non-integer y (denominator ≥ 2 ⇒ never
            // equal to the integer centre cy), well away from grazing rows.
            let qxr = qi(px);
            let qyr = q(py, pyd);
            prop_assume!(qyr.cmp(&qi(cy)) != Ordering::Equal);
            let edges = decompose(&Curve::Circle {
                circle: c.clone(), orient: Orient::Ccw, source: CurveId(0),
            });
            let inside0 = winding_parity(&qxr, &qyr, &edges);

            // Cross-check against the exact algebraic membership |p − c|² < r².
            let dx = qxr.sub(&qi(cx));
            let dy = qyr.sub(&qi(cy));
            let d2 = dx.mul(&dx).add(&dy.mul(&dy));
            // Skip points exactly on the circle — winding_parity is defined only off
            // the boundary (the query-point genericity companion of the ray genericity).
            prop_assume!(d2.cmp(&c.r2) != Ordering::Equal);
            let truly_inside = d2.cmp(&c.r2) == Ordering::Less;
            prop_assert_eq!(inside0, truly_inside, "parity must match |p−c|² < r²");

            // Rigid image: |p − c|² and thus membership are preserved.
            let m = rigid(u, v, mtx, mty);
            let c2 = rigid_circle(&c, &m);
            let p = Point2::from_rat(qxr.clone(), qyr.clone());
            let p2 = rigid_pt(&p, &m);
            // The moved query keeps a generic height as long as it avoids the moved
            // centre; skip the measure-zero coincidence rather than mis-count it.
            prop_assume!(p2.y != Surd::from_rat(c2.cy.clone()));
            // p2 has Surd coords in general (rotation); membership by the exact
            // predicate is the invariant we assert (winding needs a rational y).
            let ddx = p2.x.sub(&Surd::from_rat(c2.cx.clone())).try_surd().unwrap();
            let ddy = p2.y.sub(&Surd::from_rat(c2.cy.clone())).try_surd().unwrap();
            let d2b = ddx.mul(&ddx).try_surd().unwrap().add(&ddy.mul(&ddy).try_surd().unwrap()).try_surd().unwrap();
            let inside1 = d2b.cmp(&Surd::from_rat(c2.r2.clone())) == Ordering::Less;
            prop_assert_eq!(inside0, inside1, "membership is rigid-invariant");
        }

        /// Parity is independent of the (generic) ray height: for a point strictly
        /// inside / outside a disk, every generic y gives the same verdict.
        #[test]
        fn parity_independent_of_generic_y(
            r in 2i128..=6,
            px in -9i128..=9,
            yn in -9i128..=9, yd in 2i128..=7,
        ) {
            let c = Circle { cx: qi(0), cy: qi(0), r2: qi(r * r) };
            let edges = decompose(&Curve::Circle {
                circle: c.clone(), orient: Orient::Ccw, source: CurveId(0),
            });
            let qxr = qi(px);
            let qyr = q(yn, yd);
            prop_assume!(qyr.sign() != 0); // generic: ≠ cy = 0
            // Must also be within the circle's y-band for a meaningful horizontal
            // chord; otherwise "inside" is trivially false and still consistent.
            let dx = qxr.clone();
            let d2 = dx.mul(&dx).add(&qyr.mul(&qyr));
            prop_assume!(d2.cmp(&c.r2) != Ordering::Equal); // skip on-boundary points
            let inside = winding_parity(&qxr, &qyr, &edges);
            let truly_inside = d2.cmp(&c.r2) == Ordering::Less;
            prop_assert_eq!(inside, truly_inside);
        }
    }
}
