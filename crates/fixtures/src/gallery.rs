//! Public demo shapes for the export/viewer gallery (demo-viewer plan, Phase 2).
//!
//! Each [`Shape`] is a two-operand boolean *input* — the decomposed arrangement [`Edge`]s
//! of both operands, plus the map from each source-curve id to the [`OperandId`] (`A`/`B`)
//! it bounds. The configurations are lifted verbatim from the `arrange2d` boolean test
//! corpus, so the viewer renders exactly the cases the kernel certifies.
//!
//! Feed a shape to [`arrange2d::boolean::ledge_dom_certified`] with any
//! [`arrange2d::boolean::BoolOp`]; the resulting `Region` is what the 2D (SVG) and 3D
//! (wrap-onto-cone) exporters draw. [`all`] returns the whole gallery in display order.
//!
//! ```
//! use arrange2d::boolean::{ledge_dom_certified, BoolOp};
//! use certify_core::Verdict;
//! use fixtures::gallery;
//!
//! let two_disks = gallery::two_disks();
//! let v = ledge_dom_certified(&two_disks.edges, &two_disks.operand_of, BoolOp::Or);
//! assert!(matches!(v, Verdict::Verified(_)));
//! ```

use arrange2d::boolean::OperandId;
use arrange2d::decompose::decompose;
use geom::content::{Circle, Curve, CurveId, Edge, Line, Orient, Point2, SegPiece};
use lattice::{Bignum, Rat};

type Q = Rat<Bignum>;

/// A demo configuration: two boolean operands as decomposed arrangement edges, plus the
/// rule mapping each source-curve id to the operand it bounds.
///
/// `operand_of` is a plain function pointer — every gallery map is a small rule over the
/// source id — and satisfies the `Fn(CurveId) -> OperandId` bound the boolean entries take.
pub struct Shape {
    /// Stable machine name (kebab-case) — the viewer's gallery key.
    pub name: &'static str,
    /// One-line description of the configuration and what `△`/`∩`/`∪` yield.
    pub blurb: &'static str,
    /// The decomposed input edges of both operands, tagged by source id.
    pub edges: Vec<Edge<Bignum>>,
    /// Source-curve id → operand (`A`/`B`).
    pub operand_of: fn(CurveId) -> OperandId,
}

// --- source → operand maps --------------------------------------------------------------

/// Source 0 → operand `A`, every other source → `B` (the two-curve default).
fn src0_ab(s: CurveId) -> OperandId {
    if s.0 == 0 { OperandId::A } else { OperandId::B }
}

/// Sources 0 and 1 → operand `A`, every other source → `B` (the two-in-A grouping used by
/// the three-circle degree-6 shape).
fn src01_ab(s: CurveId) -> OperandId {
    if s.0 <= 1 { OperandId::A } else { OperandId::B }
}

// --- operand builders -------------------------------------------------------------------

/// The two x-monotone arcs of a whole CCW circle `(cx, cy, r²)`, tagged `src`.
fn circle(cx: i128, cy: i128, r2: i128, src: u32) -> Vec<Edge<Bignum>> {
    decompose(&Curve::Circle {
        circle: Circle {
            cx: Q::from_i128(cx),
            cy: Q::from_i128(cy),
            r2: Q::from_i128(r2),
        },
        orient: Orient::Ccw,
        source: CurveId(src),
    })
}

/// A closed CCW polygon operand: the segment loop through `verts`, tagged `src`.
fn polygon(verts: &[(i128, i128)], src: u32) -> Vec<Edge<Bignum>> {
    let n = verts.len();
    (0..n)
        .map(|i| {
            let (sx, sy) = verts[i];
            let (ex, ey) = verts[(i + 1) % n];
            // Line through (sx,sy)→(ex,ey): normal (a,b) = (-(Δy), Δx), c = -(a·sx + b·sy).
            let (a, b) = (Q::from_i128(-(ey - sy)), Q::from_i128(ex - sx));
            let c = a
                .mul(&Q::from_i128(sx))
                .add(&b.mul(&Q::from_i128(sy)))
                .neg();
            Edge::Seg(Box::new(SegPiece {
                line: Line { a, b, c },
                start: Point2::from_rat(Q::from_i128(sx), Q::from_i128(sy)),
                end: Point2::from_rat(Q::from_i128(ex), Q::from_i128(ey)),
                orient: Orient::Ccw,
                source: CurveId(src),
            }))
        })
        .collect()
}

// --- the gallery ------------------------------------------------------------------------

/// Two overlapping disks `(0,0,25)` and `(8,0,25)` crossing transversally at `(4,±3)`:
/// `∪` is one face, `∩` the lens, `△` two lunes pinched at the crossings.
pub fn two_disks() -> Shape {
    let mut edges = circle(0, 0, 25, 0);
    edges.extend(circle(8, 0, 25, 1));
    Shape {
        name: "two-disks",
        blurb: "Two overlapping disks: ∪ one face, ∩ the lens, △ two lunes pinched at the crossings.",
        edges,
        operand_of: src0_ab,
    }
}

/// Concentric disks — outer `A = (0,0,9)`, inner `B = (0,0,1)`: `△` is the annulus (one
/// face with one hole), `∩` the inner disk, `∪` the outer disk.
pub fn annulus() -> Shape {
    let mut edges = circle(0, 0, 9, 0);
    edges.extend(circle(0, 0, 1, 1));
    Shape {
        name: "annulus",
        blurb: "Concentric disks (r²=9 ⊃ r²=1): △ the annulus (one hole), ∩ the inner disk, ∪ the outer.",
        edges,
        operand_of: src0_ab,
    }
}

/// Internally tangent disks `A = (0,0,4)` (r=2) and `B = (1,0,1)` (r=1) touching at
/// `(2,0)`: `△` (the crescent) pinches to a point at the tangency; `∪`/`∩` are smooth.
pub fn internal_tangency() -> Shape {
    let mut edges = circle(0, 0, 4, 0);
    edges.extend(circle(1, 0, 1, 1));
    Shape {
        name: "internal-tangency",
        blurb: "Internally tangent disks touching at (2,0): △ the crescent pinches there; ∪/∩ smooth.",
        edges,
        operand_of: src0_ab,
    }
}

/// Two overlapping 4×4 squares — `A = [0,4]²`, `B = [2,6]²`: `∪` one face, `∩` the 2×2
/// overlap, `△` two L-shapes pinched at the crossings. Line-bounded (segment) operands.
pub fn two_squares() -> Shape {
    let mut edges = polygon(&[(0, 0), (4, 0), (4, 4), (0, 4)], 0);
    edges.extend(polygon(&[(2, 2), (6, 2), (6, 6), (2, 6)], 1));
    Shape {
        name: "two-squares",
        blurb: "Two overlapping 4×4 squares: ∪ one face, ∩ the 2×2 overlap, △ two L-shapes pinched at the crossings.",
        edges,
        operand_of: src0_ab,
    }
}

/// Two 4×2 rectangles sharing a *partial* collinear edge overlap on `x ∈ [2,4]` —
/// `A = [0,4]×[0,2]`, `B = [2,6]×[0,2]`: `∪` one 6×2 rectangle, `∩` the 2×2 overlap,
/// `△` two disjoint 2×2 squares.
pub fn partial_overlap_rects() -> Shape {
    let mut edges = polygon(&[(0, 0), (4, 0), (4, 2), (0, 2)], 0);
    edges.extend(polygon(&[(2, 0), (6, 0), (6, 2), (2, 2)], 1));
    Shape {
        name: "partial-overlap-rects",
        blurb: "Two 4×2 rectangles with a partial collinear-edge overlap: ∪ one 6×2, ∩ the 2×2 overlap, △ two 2×2 squares.",
        edges,
        operand_of: src0_ab,
    }
}

/// A 6×6 square `A` with a radius-2 disk `B = (3,3,4)` fully inside it (mixed line+circle):
/// `∩` the inner disk, `∪` the square, `△` the square with a disk-shaped hole.
pub fn square_with_hole() -> Shape {
    let mut edges = polygon(&[(0, 0), (6, 0), (6, 6), (0, 6)], 0);
    edges.extend(circle(3, 3, 4, 1));
    Shape {
        name: "square-with-hole",
        blurb: "A 6×6 square around a radius-2 disk: ∩ the disk, ∪ the square, △ the square with a disk-shaped hole.",
        edges,
        operand_of: src0_ab,
    }
}

/// Three circles `(1,0,1)`, `(0,1,1)`, `(1,1,2)` all through the common point `(0,0)` — a
/// degree-6 arrangement vertex. Operand `A = {sources 0,1}`, `B = {source 2}`; the
/// certified entry Verifies for every op (CAP-OUT-LINK is proven up to ≤6 sectors).
pub fn degree6_vertex() -> Shape {
    let mut edges = circle(1, 0, 1, 0);
    edges.extend(circle(0, 1, 1, 1));
    edges.extend(circle(1, 1, 2, 2));
    Shape {
        name: "degree6-vertex",
        blurb: "Three circles meeting at (0,0), a degree-6 vertex: certifies for every op.",
        edges,
        operand_of: src01_ab,
    }
}

/// The whole gallery, in display order.
pub fn all() -> Vec<Shape> {
    vec![
        two_disks(),
        annulus(),
        internal_tangency(),
        two_squares(),
        partial_overlap_rects(),
        square_with_hole(),
        degree6_vertex(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrange2d::boolean::{BoolOp, ledge_dom_certified};
    use certify_core::Verdict;

    /// Every gallery shape certifies (CAP-OUT `Verified`) under all three boolean ops —
    /// the viewer only ever renders certified regions.
    #[test]
    fn every_shape_certifies_all_ops() {
        for shape in all() {
            for op in [BoolOp::Xor, BoolOp::And, BoolOp::Or] {
                assert!(
                    matches!(
                        ledge_dom_certified(&shape.edges, &shape.operand_of, op),
                        Verdict::Verified(_)
                    ),
                    "gallery shape `{}` must certify ({op:?})",
                    shape.name,
                );
            }
        }
    }

    /// Names are unique and kebab-case (the viewer uses them as stable keys).
    #[test]
    fn names_are_unique_kebab_case() {
        let shapes = all();
        let mut names: Vec<&str> = shapes.iter().map(|s| s.name).collect();
        let count = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), count, "gallery shape names must be unique");
        for name in names {
            assert!(
                !name.is_empty()
                    && name
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'),
                "name `{name}` must be kebab-case",
            );
        }
    }
}
