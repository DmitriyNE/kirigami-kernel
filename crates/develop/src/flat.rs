//! Flat-pattern **2-D region assembly** over the exact [`arrange2d`] boolean kernel — G5:
//! cut an authored interior hole out of a developed flat outline.
//!
//! [`cut_hole`] takes a certified [`FlatOutline`](crate::unroll::FlatOutline) (G3) and a hole
//! polygon in flat coordinates, and returns a **certified flat region** with the hole. The
//! outline's `FlatBox` vertices are reduced to their rational [`center`](crate::cone::FlatBox::center)s
//! and lifted to exact [`Point2`]s, so the boolean is exact over ℚ — no float enters. (The
//! outline is only `ε`-faithful to the true development, per G3's bound; the hole is then placed
//! *exactly* on that rational polygon.)
//!
//! There is **no `Difference`** op in `arrange2d` (only `Xor`/`And`/`Or`). For a hole *strictly
//! interior* to the outline, `A △ B = A ∖ B`, so the cut is [`BoolOp::Xor`] with the outline as
//! operand `A` and the hole as `B` — the in-tree convention (`fixtures::gallery::square_with_hole`).
//! That the hole really was interior is not assumed: [`cut_hole`] **certifies the postcondition**
//! (exactly one face, exactly one hole, no self-touch pinch), so a hole that falls outside,
//! crosses, or is tangent to the boundary is [`Refuted`](certify_core::Verdict::Refuted), never a
//! wrong [`Verified`](certify_core::Verdict::Verified).

use crate::unroll::FlatOutline;
use arrange2d::boolean::{BoolOp, CapOutFault, OperandId, Region, ledge_dom_certified};
use certify_core::Verdict;
use geom::content::{CurveId, Edge, Line, Orient, Point2, SegPiece};
use lattice::{Backend, Bignum, Rat};

/// A certified flat pattern region with an interior hole: the exact [`arrange2d`] boolean of an
/// `ε`-faithful developed outline (`region.faces[0].outer`, CCW) with the authored hole
/// (`region.faces[0].holes[0]`, CW), tagged with the outline's development-fidelity bound.
pub struct HoledFlat<B: Backend = Bignum> {
    /// The certified boolean region — one [`Face`](arrange2d::boolean::Face): `outer` is the
    /// outline loop, `holes[0]` is the hole loop.
    pub region: Region<B>,
    /// The development-fidelity bound carried from the source [`FlatOutline`](crate::unroll::FlatOutline).
    pub eps: Rat<B>,
    /// The clearance the outline's DRC compared against (`ε < clearance/2`).
    pub clearance: Rat<B>,
}

/// Why the hole cut refused.
#[derive(Clone, Debug)]
pub enum FlatHoleFault {
    /// The outline or the hole has fewer than 3 vertices.
    DegenerateOutline,
    /// The exact [`arrange2d`] boolean refused (malformed input — e.g. a zero-length edge from
    /// two coincident outline vertices — or a kernel/constructor bug).
    Boolean(CapOutFault),
    /// The result is not exactly one face with exactly one hole, or the region self-touches — the
    /// hole was not strictly interior (it fell outside, crossed, or was tangent to the boundary).
    HoleNotInterior,
}

/// One polygon edge `(sx, sy) → (ex, ey)` as an exact [`Edge::Seg`], tagged with source `src`.
///
/// The directed line `a·x + b·y + c = 0` has normal `(a, b) = (−(ey−sy), ex−sx)` and
/// `c = −(a·sx + b·sy)`, which passes through *both* endpoints exactly (else `arrange2d`'s
/// CAP-IN pre-pass rejects the segment as off-carrier).
fn seg_edge<B: Backend>(sx: &Rat<B>, sy: &Rat<B>, ex: &Rat<B>, ey: &Rat<B>, src: u32) -> Edge<B> {
    let a = ey.sub(sy).neg(); // −(ey − sy)
    let b = ex.sub(sx); //  (ex − sx)
    let c = a.mul(sx).add(&b.mul(sy)).neg(); // −(a·sx + b·sy)
    Edge::Seg(Box::new(SegPiece {
        line: Line { a, b, c },
        start: Point2::from_rat(sx.clone(), sy.clone()),
        end: Point2::from_rat(ex.clone(), ey.clone()),
        orient: Orient::Ccw, // provenance only; the boolean recomputes winding
        source: CurveId(src),
    }))
}

/// Convert a [`FlatOutline`](crate::unroll::FlatOutline) into a closed loop of [`arrange2d`]
/// segment edges, tagged with source `src` — each vertex reduced to its rational
/// [`FlatBox`](crate::cone::FlatBox) center. The outline's vertices are already a closed loop with
/// no repeated first vertex, so this emits one edge per consecutive pair (wrapping the last back
/// to the first).
pub fn outline_to_edges<B: Backend>(outline: &FlatOutline<B>, src: u32) -> Vec<Edge<B>> {
    let n = outline.vertices.len();
    (0..n)
        .map(|i| {
            let (sx, sy) = outline.vertices[i].center();
            let (ex, ey) = outline.vertices[(i + 1) % n].center();
            seg_edge(&sx, &sy, &ex, &ey, src)
        })
        .collect()
}

/// Cut an interior `hole` polygon out of a developed `outline`, certified by the exact
/// [`arrange2d`] boolean (G5).
///
/// The outline is operand `A` (source `0`) and the hole operand `B` (source `1`); the cut is
/// [`BoolOp::Xor`] (which equals the set difference exactly when the hole is strictly interior).
/// The clean-interior postcondition — one face, one hole, no pinch — is **certified**, so a hole
/// that is not strictly inside is [`Refuted`](Verdict::Refuted). Both polygons are exact over ℚ
/// (the outline via its `FlatBox` centers), so no float enters.
///
/// Returns `Verified(`[`HoledFlat`]`)` on a clean interior cut, `Refuted(`[`FlatHoleFault`]`)`
/// for a degenerate/non-interior hole or a boolean refusal, or `Unresolved(())` (unreachable for
/// this entry — propagated defensively).
///
/// ```
/// use develop::flat::cut_hole;
/// use develop::unroll::FlatOutline;
/// use develop::cone::FlatBox;
/// use develop::interval::RatIv;
/// use certify_core::Verdict;
/// use lattice::{Bignum, Rat};
///
/// let q = |n: i128| Rat::<Bignum>::from_i128(n);
/// let pt = |x: i128, y: i128| FlatBox { x: RatIv::new(q(x), q(x)), y: RatIv::new(q(y), q(y)) };
/// // A 4×4 square outline minus a centered 2×2 square hole.
/// let outline = FlatOutline { vertices: vec![pt(0, 0), pt(4, 0), pt(4, 4), pt(0, 4)], eps: q(0), clearance: q(1) };
/// let hole = [[q(1), q(1)], [q(3), q(1)], [q(3), q(3)], [q(1), q(3)]];
/// assert!(matches!(cut_hole(&outline, &hole), Verdict::Verified(h) if h.region.faces[0].holes.len() == 1));
/// ```
pub fn cut_hole<B: Backend>(
    outline: &FlatOutline<B>,
    hole: &[[Rat<B>; 2]],
) -> Verdict<HoledFlat<B>, FlatHoleFault, ()> {
    if outline.vertices.len() < 3 || hole.len() < 3 {
        return Verdict::Refuted(FlatHoleFault::DegenerateOutline);
    }
    let mut edges = outline_to_edges(outline, 0);
    let m = hole.len();
    for (i, s) in hole.iter().enumerate() {
        let e = &hole[(i + 1) % m];
        edges.push(seg_edge(&s[0], &s[1], &e[0], &e[1], 1));
    }
    let operand_of = |c: CurveId| {
        if c.0 == 0 { OperandId::A } else { OperandId::B }
    };
    match ledge_dom_certified(&edges, &operand_of, BoolOp::Xor) {
        Verdict::Verified(cap) => {
            let (region, _v_boundary, pinches) = cap.into_parts();
            if pinches.is_empty() && region.faces.len() == 1 && region.faces[0].holes.len() == 1 {
                Verdict::Verified(HoledFlat {
                    region,
                    eps: outline.eps.clone(),
                    clearance: outline.clearance.clone(),
                })
            } else {
                Verdict::Refuted(FlatHoleFault::HoleNotInterior)
            }
        }
        Verdict::Refuted(f) => Verdict::Refuted(FlatHoleFault::Boolean(f)),
        Verdict::Unresolved(()) => Verdict::Unresolved(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cone::FlatBox;
    use crate::interval::RatIv;

    type Q = Rat<Bignum>;

    fn q(n: i128) -> Q {
        Q::from_i128(n)
    }
    fn pt(x: i128, y: i128) -> FlatBox<Bignum> {
        FlatBox {
            x: RatIv::new(q(x), q(x)),
            y: RatIv::new(q(y), q(y)),
        }
    }

    /// A 4×4 square outline minus a centered 2×2 square is one face with one 4-edge hole, and the
    /// outline's ε rides along on the result.
    #[test]
    fn synthetic_square_hole_is_clean() {
        let outline = FlatOutline {
            vertices: vec![pt(0, 0), pt(4, 0), pt(4, 4), pt(0, 4)],
            eps: q(7),
            clearance: q(1000),
        };
        let hole = [[q(1), q(1)], [q(3), q(1)], [q(3), q(3)], [q(1), q(3)]];
        match cut_hole(&outline, &hole) {
            Verdict::Verified(h) => {
                assert_eq!(h.region.faces.len(), 1, "one face");
                assert_eq!(h.region.faces[0].holes.len(), 1, "one hole");
                assert_eq!(
                    h.region.faces[0].holes[0].len(),
                    4,
                    "the square hole has 4 edges"
                );
                assert_eq!(
                    h.eps.cmp(&q(7)),
                    core::cmp::Ordering::Equal,
                    "ε carried from outline"
                );
            }
            _ => panic!("a strictly-interior square hole must certify"),
        }
    }

    /// A hole disjoint from the outline yields two faces under Xor — refused as not interior.
    #[test]
    fn hole_outside_is_refused() {
        let outline = FlatOutline {
            vertices: vec![pt(0, 0), pt(4, 0), pt(4, 4), pt(0, 4)],
            eps: q(0),
            clearance: q(1000),
        };
        let hole = [[q(6), q(6)], [q(8), q(6)], [q(8), q(8)], [q(6), q(8)]];
        assert!(matches!(
            cut_hole(&outline, &hole),
            Verdict::Refuted(FlatHoleFault::HoleNotInterior)
        ));
    }

    /// An outline with fewer than 3 vertices is refused before any boolean.
    #[test]
    fn degenerate_outline_is_refused() {
        let outline = FlatOutline {
            vertices: vec![pt(0, 0), pt(1, 0)],
            eps: q(0),
            clearance: q(1),
        };
        let hole = [[q(0), q(0)], [q(1), q(0)], [q(1), q(1)]];
        assert!(matches!(
            cut_hole(&outline, &hole),
            Verdict::Refuted(FlatHoleFault::DegenerateOutline)
        ));
    }

    /// The G3→G5 bridge: a *real* developed band outline (via `unroll_freeboundary`) is a valid,
    /// simple `arrange2d` operand — its `FlatBox`-center polygon certifies as one hole-free face.
    #[test]
    fn unrolled_band_bridges_to_arrange2d() {
        use crate::cone::{ConeDevelopment, DevConfig};
        use crate::unroll::unroll_freeboundary;
        use arrange2d::boolean::ledge_dom_certified;
        use fixtures::devices::cone;
        use lattice::{Interval, Poly, RatFunc};

        let dev = ConeDevelopment::new(&cone()).unwrap();
        let ratf = |c: i128| RatFunc::<Bignum>::from_poly(Poly::constant(q(c)));
        let sigma = Interval { lo: q(0), hi: q(1) };
        let outline = match unroll_freeboundary(
            &dev,
            &sigma,
            &ratf(-1),
            &ratf(-2),
            6,
            &DevConfig::tight(),
            &q(1000),
        ) {
            Verdict::Verified(o) => o,
            _ => panic!("the band must certify"),
        };
        let edges = outline_to_edges(&outline, 0);
        match ledge_dom_certified(&edges, &|_c: CurveId| OperandId::A, BoolOp::Or) {
            Verdict::Verified(cap) => {
                assert_eq!(
                    cap.region().faces.len(),
                    1,
                    "the developed band is one simple face"
                );
                assert!(
                    cap.region().faces[0].holes.is_empty(),
                    "a plain outline has no holes"
                );
            }
            _ => panic!("the developed outline must be a valid arrange2d operand"),
        }
    }
}
