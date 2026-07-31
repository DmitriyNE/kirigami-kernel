//! Shared test-only V&V support for the arrangement (M3a Phase 5).
//!
//! One home for what the per-module test modules duplicated (rigid-motion +
//! exact-on-circle generators, the residual-zero point oracle), plus the two new
//! Phase-5 pieces: the `resultant_bivariate` **count** oracle (an intersection
//! cardinality computed by Sylvester + fraction-free Bareiss — sharing nothing
//! with `carrier.rs`'s discriminant), and the stratum-weighted input strategy with
//! the `ARRANGE_STRATUM_WEIGHT` degenerate-heavy knob. Compiled only under `test`.

#![allow(dead_code)]

use crate::carrier::{self, Intersections};
use crate::decompose::decompose;
use geom::content::{Circle, Curve, CurveId, Edge, Line, Orient, Point2, SegPiece};
use lattice::{Bignum, Poly, Rat, SturmChain, Surd, resultant_bivariate};
use proptest::prelude::*;
use proptest::strategy::{BoxedStrategy, Union};

pub(crate) type Q = Rat<Bignum>;
pub(crate) type P = Point2<Bignum>;

// ---------------------------------------------------------------------------
// basic constructors
// ---------------------------------------------------------------------------

pub(crate) fn q(n: i128) -> Q {
    Q::from_i128(n)
}
pub(crate) fn rp(x: i128, y: i128) -> P {
    Point2::from_rat(Q::from_i128(x), Q::from_i128(y))
}

// ---------------------------------------------------------------------------
// rigid motion — Pythagorean rational rotation + rational translation
// ---------------------------------------------------------------------------

/// A rational rigid motion `p ↦ R·p + t` with `cos = (u²−v²)/(u²+v²)`,
/// `sin = 2uv/(u²+v²)` (so `cos² + sin² = 1` exactly), `(u, v) ≠ (0, 0)`.
pub(crate) struct Rigid {
    pub co: Q,
    pub si: Q,
    pub tx: Q,
    pub ty: Q,
}

pub(crate) fn rigid(u: i128, v: i128, tx: i128, ty: i128) -> Rigid {
    let den = u * u + v * v;
    Rigid {
        co: Q::new(u * u - v * v, den),
        si: Q::new(2 * u * v, den),
        tx: Q::from_i128(tx),
        ty: Q::from_i128(ty),
    }
}

/// `k1·a ± k2·b + t` for rational `k1,k2,t` and surds a,b (same radical / rational).
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

pub(crate) fn rigid_pt(p: &P, m: &Rigid) -> P {
    // x' = co·x − si·y + tx ; y' = si·x + co·y + ty
    Point2 {
        x: surd_lin(&m.co, &p.x, &m.si, &p.y, true, &m.tx),
        y: surd_lin(&m.si, &p.x, &m.co, &p.y, false, &m.ty),
    }
}

/// A line under `p ↦ R·p + t`: the normal rotates (`n' = R·n`), `c' = c − n'·t`.
pub(crate) fn rigid_line(l: &Line<Bignum>, m: &Rigid) -> Line<Bignum> {
    let a = m.co.mul(&l.a).sub(&m.si.mul(&l.b));
    let b = m.si.mul(&l.a).add(&m.co.mul(&l.b));
    let c = l.c.sub(&a.mul(&m.tx).add(&b.mul(&m.ty)));
    Line { a, b, c }
}

/// A circle under the rigid motion: centre moves, `r²` is preserved.
pub(crate) fn rigid_circle(c: &Circle<Bignum>, m: &Rigid) -> Circle<Bignum> {
    let cx = m.co.mul(&c.cx).sub(&m.si.mul(&c.cy)).add(&m.tx);
    let cy = m.si.mul(&c.cx).add(&m.co.mul(&c.cy)).add(&m.ty);
    Circle {
        cx,
        cy,
        r2: c.r2.clone(),
    }
}

// ---------------------------------------------------------------------------
// lattice rescaling `p ↦ k·p` (the second metamorphic invariant)
// ---------------------------------------------------------------------------

/// Scale a point by a rational `k`.
pub(crate) fn scale_pt(p: &P, k: &Q) -> P {
    Point2 {
        x: p.x.scale(k),
        y: p.y.scale(k),
    }
}
/// The image of `a·x+b·y+c=0` under `p ↦ k·p` is `a·x+b·y+k·c=0`.
pub(crate) fn scale_line(l: &Line<Bignum>, k: &Q) -> Line<Bignum> {
    Line {
        a: l.a.clone(),
        b: l.b.clone(),
        c: l.c.mul(k),
    }
}
/// The image of a circle under `p ↦ k·p`: centre `k·C`, squared radius `k²·r²`.
pub(crate) fn scale_circle(c: &Circle<Bignum>, k: &Q) -> Circle<Bignum> {
    Circle {
        cx: c.cx.mul(k),
        cy: c.cy.mul(k),
        r2: c.r2.mul(k).mul(k),
    }
}

// ---------------------------------------------------------------------------
// exact rational point on a circle (stereographic parametrisation)
// ---------------------------------------------------------------------------

/// The exact rational point on the circle `(cx, cy, r²=r·r)` at parameter
/// `t = tn/td`: `(cx + r(td²−tn²)/D, cy + r·2·tn·td/D)`, `D = td²+tn²`.
pub(crate) fn on_circle_pt(cx: &Q, cy: &Q, r: &Q, tn: i128, td: i128) -> P {
    let (tn, td) = (Q::from_i128(tn), Q::from_i128(td));
    let denom = td.mul(&td).add(&tn.mul(&tn));
    let x = cx.add(&r.mul(&td.mul(&td).sub(&tn.mul(&tn))).div(&denom));
    let y = cy.add(&r.mul(&Q::from_i128(2).mul(&tn).mul(&td)).div(&denom));
    Point2::from_rat(x, y)
}

// ---------------------------------------------------------------------------
// residual-zero point oracle (a point lies exactly on a carrier)
// ---------------------------------------------------------------------------

pub(crate) fn on_line(l: &Line<Bignum>, p: &P) -> bool {
    p.x.scale(&l.a)
        .add(&p.y.scale(&l.b))
        .unwrap_surd()
        .add(&Surd::from_rat(l.c.clone()))
        .unwrap_surd()
        .sign()
        == 0
}
pub(crate) fn on_circle(c: &Circle<Bignum>, p: &P) -> bool {
    let dx = p.x.sub(&Surd::from_rat(c.cx.clone())).unwrap_surd();
    let dy = p.y.sub(&Surd::from_rat(c.cy.clone())).unwrap_surd();
    dx.square()
        .add(&dy.square())
        .unwrap_surd()
        .sub(&Surd::from_rat(c.r2.clone()))
        .unwrap_surd()
        .sign()
        == 0
}

// ---------------------------------------------------------------------------
// resultant_bivariate count oracle
// ---------------------------------------------------------------------------
//
// An intersection point of two carriers is a common (x, y) zero. `Res_x(f,g)(y₀)=0`
// iff f,g share an x-root at height y₀ — but that x-root may be complex, so a real
// root of the eliminant does not always mean a *real* intersection (two disjoint
// circles share a real x-projection with a complex y). The fix: eliminate the
// variable the LINE determines. For a line `a·x+b·y+c=0` with `a≠0`, `x` is a real
// function of `y`, so every real y-root of `Res_x` is a genuine real intersection;
// symmetrically eliminate `y` when `a=0`. Circle∩circle reduces to its (linear)
// radical line ∩ a circle, so the same rule applies. Then the eliminant's distinct
// real-root count (an independent Sylvester+Bareiss+Sturm computation) equals our
// carrier's cardinality.

fn carrier_card(i: &Intersections<Bignum>) -> usize {
    match i {
        Intersections::Empty | Intersections::SharedCarrier => 0,
        Intersections::One(_) => 1,
        Intersections::Two(..) => 2,
    }
}

// A carrier as x-coefficient list (low x-degree first), each coeff a y-polynomial,
// and vice versa — the shape `resultant_bivariate` eats.
fn line_x_in_y(l: &Line<Bignum>) -> Vec<Poly<Bignum>> {
    // a·x + (b·y + c): x⁰ = b·y+c, x¹ = a
    vec![
        Poly::from_coeffs(vec![l.c.clone(), l.b.clone()]),
        Poly::constant(l.a.clone()),
    ]
}
fn line_y_in_x(l: &Line<Bignum>) -> Vec<Poly<Bignum>> {
    // b·y + (a·x + c): y⁰ = a·x+c, y¹ = b
    vec![
        Poly::from_coeffs(vec![l.c.clone(), l.a.clone()]),
        Poly::constant(l.b.clone()),
    ]
}
fn circle_x_in_y(c: &Circle<Bignum>) -> Vec<Poly<Bignum>> {
    // x² − 2cx·x + (y² − 2cy·y + cx²+cy²−r²)
    let k = c.cx.mul(&c.cx).add(&c.cy.mul(&c.cy)).sub(&c.r2);
    vec![
        Poly::from_coeffs(vec![k, c.cy.mul(&q(-2)), q(1)]),
        Poly::constant(c.cx.mul(&q(-2))),
        Poly::constant(q(1)),
    ]
}
fn circle_y_in_x(c: &Circle<Bignum>) -> Vec<Poly<Bignum>> {
    let k = c.cx.mul(&c.cx).add(&c.cy.mul(&c.cy)).sub(&c.r2);
    vec![
        Poly::from_coeffs(vec![k, c.cx.mul(&q(-2)), q(1)]),
        Poly::constant(c.cy.mul(&q(-2))),
        Poly::constant(q(1)),
    ]
}

fn distinct_real_roots(elim: &Poly<Bignum>) -> usize {
    SturmChain::new(elim).count_all() as usize
}

/// The real-intersection count of a line/circle via the eliminant, eliminating
/// the variable the line determines so no complex intersection contributes.
fn count_via_resultant(l: &Line<Bignum>, c: &Circle<Bignum>) -> usize {
    let elim = if l.a.sign() != 0 {
        resultant_bivariate(&line_x_in_y(l), &circle_x_in_y(c)) // x determined by y
    } else {
        resultant_bivariate(&line_y_in_x(l), &circle_y_in_x(c)) // y determined by x
    };
    distinct_real_roots(&elim)
}

/// The resultant count oracle for a line/circle pair: the independent eliminant
/// root-count equals our carrier's cardinality.
pub(crate) fn resultant_agrees_line_circle(l: &Line<Bignum>, c: &Circle<Bignum>) -> bool {
    count_via_resultant(l, c) == carrier_card(&carrier::line_circle(l, c))
}

/// The resultant count oracle for two circles, via their radical line ∩ a circle.
/// Concentric circles have no radical line (`a=b=0`) — that is the `Empty` /
/// `SharedCarrier` case, validated elsewhere, so it is skipped here.
pub(crate) fn resultant_agrees_circle_circle(c1: &Circle<Bignum>, c2: &Circle<Bignum>) -> bool {
    let a = c2.cx.sub(&c1.cx).mul(&q(2));
    let b = c2.cy.sub(&c1.cy).mul(&q(2));
    if a.sign() == 0 && b.sign() == 0 {
        return true; // concentric
    }
    let p1 = c1.cx.mul(&c1.cx).add(&c1.cy.mul(&c1.cy)).sub(&c1.r2);
    let p2 = c2.cx.mul(&c2.cx).add(&c2.cy.mul(&c2.cy)).sub(&c2.r2);
    let radical = Line {
        a,
        b,
        c: p1.sub(&p2),
    };
    count_via_resultant(&radical, c1) == carrier_card(&carrier::circle_circle(c1, c2))
}

/// The image of a carrier result under `p ↦ k·p` (positive `k` preserves the
/// lexicographic sweep order, so the ordered comparison is exact).
fn scaled_result(i: &Intersections<Bignum>, k: &Q) -> Intersections<Bignum> {
    match i {
        Intersections::Empty => Intersections::Empty,
        Intersections::SharedCarrier => Intersections::SharedCarrier,
        Intersections::One(p) => Intersections::One(scale_pt(p, k)),
        Intersections::Two(p, q) => Intersections::Two(scale_pt(p, k), scale_pt(q, k)),
    }
}

// ---------------------------------------------------------------------------
// stratum-weighted input generation
// ---------------------------------------------------------------------------

/// An arrangement input carrier (a line, or a full circle) — pre-bounding.
#[derive(Clone, Debug)]
pub(crate) enum Carrier {
    Line(Line<Bignum>),
    Circle(Circle<Bignum>),
}

fn cl(a: i128, b: i128, c: i128) -> Carrier {
    Carrier::Line(Line {
        a: q(a),
        b: q(b),
        c: q(c),
    })
}
fn cc(cx: i128, cy: i128, r2: i128) -> Carrier {
    Carrier::Circle(Circle {
        cx: q(cx),
        cy: q(cy),
        r2: q(r2),
    })
}

/// Bound a line to a wide rational segment centred on a point of the line, along
/// its direction `(b, −a)` — wide enough (`±T·dir`) to contain every intersection
/// of small-coordinate inputs.
fn bound_line(l: &Line<Bignum>, src: CurveId) -> SegPiece<Bignum> {
    let t = q(64);
    let (px, py) = if l.b.sign() != 0 {
        (q(0), l.c.neg().div(&l.b)) // (0, −c/b)
    } else {
        (l.c.neg().div(&l.a), q(0)) // (−c/a, 0)
    };
    let (dx, dy) = (l.b.clone(), l.a.neg()); // direction (b, −a)
    SegPiece {
        line: l.clone(),
        start: Point2::from_rat(px.sub(&t.mul(&dx)), py.sub(&t.mul(&dy))),
        end: Point2::from_rat(px.add(&t.mul(&dx)), py.add(&t.mul(&dy))),
        orient: Orient::Ccw,
        source: src,
    }
}

/// Turn input carriers into the decomposed `Edge` list `arrange_events` consumes:
/// lines bounded to wide segments, circles decomposed to their two arcs.
pub(crate) fn to_edges(carriers: &[Carrier]) -> Vec<Edge<Bignum>> {
    let mut edges = Vec::new();
    for (i, cr) in carriers.iter().enumerate() {
        let src = CurveId(i as u32);
        match cr {
            Carrier::Line(l) => edges.extend(decompose(&Curve::Seg(bound_line(l, src)))),
            Carrier::Circle(c) => edges.extend(decompose(&Curve::Circle {
                circle: c.clone(),
                orient: Orient::Ccw,
                source: src,
            })),
        }
    }
    edges
}

/// Apply a rigid motion to every carrier (for the metamorphic invariant).
pub(crate) fn move_carriers(carriers: &[Carrier], m: &Rigid) -> Vec<Carrier> {
    carriers
        .iter()
        .map(|c| match c {
            Carrier::Line(l) => Carrier::Line(rigid_line(l, m)),
            Carrier::Circle(c) => Carrier::Circle(rigid_circle(c, m)),
        })
        .collect()
}

// The strata, built on-stratum by construction, exactly in-lattice.
fn s_two_lines() -> BoxedStrategy<Vec<Carrier>> {
    (
        -6i128..=6,
        -6i128..=6,
        -6i128..=6,
        -6i128..=6,
        -6i128..=6,
        -6i128..=6,
    )
        .prop_filter("non-parallel", |&(a1, b1, _, a2, b2, _)| {
            a1 * b2 - a2 * b1 != 0
        })
        .prop_map(|(a1, b1, c1, a2, b2, c2)| vec![cl(a1, b1, c1), cl(a2, b2, c2)])
        .boxed()
}
fn s_line_circle() -> BoxedStrategy<Vec<Carrier>> {
    (
        -6i128..=6,
        -6i128..=6,
        -8i128..=8,
        -5i128..=5,
        -5i128..=5,
        1i128..=30,
    )
        .prop_filter("real line", |&(a, b, ..)| a != 0 || b != 0)
        .prop_map(|(a, b, c, cx, cy, r2)| vec![cl(a, b, c), cc(cx, cy, r2)])
        .boxed()
}
fn s_two_circles() -> BoxedStrategy<Vec<Carrier>> {
    (
        -5i128..=5,
        -5i128..=5,
        1i128..=25,
        -5i128..=5,
        -5i128..=5,
        1i128..=25,
    )
        .prop_map(|(a, b, r1, c, d, r2)| vec![cc(a, b, r1), cc(c, d, r2)])
        .boxed()
}
fn s_concurrent_lines() -> BoxedStrategy<Vec<Carrier>> {
    // three distinct lines through the common point (px, py): c = −(a·px + b·py).
    (
        -5i128..=5,
        -5i128..=5,
        (-4i128..=4, -4i128..=4),
        (-4i128..=4, -4i128..=4),
        (-4i128..=4, -4i128..=4),
    )
        .prop_filter(
            "distinct non-degenerate normals",
            |&(_, _, (a1, b1), (a2, b2), (a3, b3))| {
                (a1 != 0 || b1 != 0)
                    && (a2 != 0 || b2 != 0)
                    && (a3 != 0 || b3 != 0)
                    && a1 * b2 - a2 * b1 != 0
                    && a1 * b3 - a3 * b1 != 0
                    && a2 * b3 - a3 * b2 != 0
            },
        )
        .prop_map(|(px, py, (a1, b1), (a2, b2), (a3, b3))| {
            let c = |a: i128, b: i128| -(a * px + b * py);
            vec![
                cl(a1, b1, c(a1, b1)),
                cl(a2, b2, c(a2, b2)),
                cl(a3, b3, c(a3, b3)),
            ]
        })
        .boxed()
}
fn s_coincident_lines() -> BoxedStrategy<Vec<Carrier>> {
    // l and a nonzero scalar multiple k·l — the SharedCarrier stratum.
    (-6i128..=6, -6i128..=6, -6i128..=6, 1i128..=5)
        .prop_filter("real line", |&(a, b, _, _)| a != 0 || b != 0)
        .prop_map(|(a, b, c, k)| vec![cl(a, b, c), cl(a * k, b * k, c * k)])
        .boxed()
}
fn s_coincident_circles() -> BoxedStrategy<Vec<Carrier>> {
    (-5i128..=5, -5i128..=5, 1i128..=25)
        .prop_map(|(cx, cy, r2)| vec![cc(cx, cy, r2), cc(cx, cy, r2)])
        .boxed()
}
fn s_tangent_circles() -> BoxedStrategy<Vec<Carrier>> {
    // exact tangency: centres r1+r2 apart (external) or |r1−r2| apart (internal),
    // along the x-axis, with rational radii — so d = r₁±r₂ is exactly rational.
    (-4i128..=4, -4i128..=4, 1i128..=5, 1i128..=5, any::<bool>())
        .prop_filter("distinct radii for internal", |&(_, _, r1, r2, ext)| {
            ext || r1 != r2
        })
        .prop_map(|(cx, cy, r1, r2, ext)| {
            let d = if ext { r1 + r2 } else { (r1 - r2).abs() };
            vec![cc(cx, cy, r1 * r1), cc(cx + d, cy, r2 * r2)]
        })
        .boxed()
}

/// The stratum-weighted input strategy. The degenerate strata (concurrent /
/// coincident / tangent) are over-sampled by `ARRANGE_STRATUM_WEIGHT` (default 1;
/// CI sets it high for the degenerate-heavy pass).
pub(crate) fn stratum() -> impl Strategy<Value = Vec<Carrier>> {
    let w = std::env::var("ARRANGE_STRATUM_WEIGHT")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(1)
        .max(1);
    Union::new_weighted(vec![
        (2, s_two_lines()),
        (2, s_line_circle()),
        (2, s_two_circles()),
        (w, s_concurrent_lines()),
        (w, s_coincident_lines()),
        (w, s_coincident_circles()),
        (w, s_tangent_circles()),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(a: i128, b: i128, c: i128) -> Line<Bignum> {
        Line {
            a: q(a),
            b: q(b),
            c: q(c),
        }
    }
    fn circle(cx: i128, cy: i128, r2: i128) -> Circle<Bignum> {
        Circle {
            cx: q(cx),
            cy: q(cy),
            r2: q(r2),
        }
    }

    // --- the resultant count oracle agrees on known configs ---

    #[test]
    fn resultant_oracle_units() {
        // secant (2 points), tangent (1), external (0), for line ∩ circle
        assert!(resultant_agrees_line_circle(
            &line(0, 1, 0),
            &circle(0, 0, 2)
        )); // y=0 secant
        assert!(resultant_agrees_line_circle(
            &line(1, 0, -1),
            &circle(0, 0, 2)
        )); // x=1 secant
        assert!(resultant_agrees_line_circle(
            &line(0, 1, -1),
            &circle(0, 0, 1)
        )); // tangent
        assert!(resultant_agrees_line_circle(
            &line(0, 1, -2),
            &circle(0, 0, 1)
        )); // external
        assert!(resultant_agrees_line_circle(
            &line(1, 2, -1),
            &circle(1, 1, 5)
        )); // skew secant
        // circle ∩ circle
        assert!(resultant_agrees_circle_circle(
            &circle(0, 0, 1),
            &circle(1, 0, 1)
        )); // 2 pts
        assert!(resultant_agrees_circle_circle(
            &circle(0, 0, 1),
            &circle(2, 0, 1)
        )); // tangent
        assert!(resultant_agrees_circle_circle(
            &circle(0, 0, 1),
            &circle(5, 0, 1)
        )); // disjoint
        assert!(resultant_agrees_circle_circle(
            &circle(0, 0, 4),
            &circle(0, 0, 9)
        )); // concentric
        assert!(resultant_agrees_circle_circle(
            &circle(0, 0, 4),
            &circle(0, 0, 4)
        )); // coincident (skipped)
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(96))]

        /// The independent resultant_bivariate count oracle agrees with carrier
        /// solving over random line/circle pairs.
        #[test]
        fn resultant_oracle_line_circle(
            a in -6i128..=6, b in -6i128..=6, c in -8i128..=8,
            cx in -5i128..=5, cy in -5i128..=5, r2 in 1i128..=30,
        ) {
            prop_assume!(a != 0 || b != 0);
            prop_assert!(resultant_agrees_line_circle(&line(a, b, c), &circle(cx, cy, r2)));
        }

        /// The resultant oracle agrees with carrier solving over random circle pairs.
        #[test]
        fn resultant_oracle_circle_circle(
            c1x in -5i128..=5, c1y in -5i128..=5, r1 in 1i128..=25,
            c2x in -5i128..=5, c2y in -5i128..=5, r2 in 1i128..=25,
        ) {
            prop_assert!(resultant_agrees_circle_circle(&circle(c1x, c1y, r1), &circle(c2x, c2y, r2)));
        }

        /// Lattice rescaling `p ↦ k·p` (k > 0): the carrier of the scaled inputs is
        /// the scaled carrier of the inputs — the second metamorphic invariant.
        #[test]
        fn rescaling_metamorphic_line_circle(
            a in -6i128..=6, b in -6i128..=6, c in -8i128..=8,
            cx in -5i128..=5, cy in -5i128..=5, r2 in 1i128..=30,
            kn in 1i128..=6, kd in 1i128..=6,
        ) {
            prop_assume!(a != 0 || b != 0);
            let (l, cc, k) = (line(a, b, c), circle(cx, cy, r2), Q::new(kn, kd));
            let scaled = carrier::line_circle(&scale_line(&l, &k), &scale_circle(&cc, &k));
            let expect = scaled_result(&carrier::line_circle(&l, &cc), &k);
            prop_assert_eq!(scaled, expect);
        }

        #[test]
        fn rescaling_metamorphic_circle_circle(
            c1x in -5i128..=5, c1y in -5i128..=5, r1 in 1i128..=25,
            c2x in -5i128..=5, c2y in -5i128..=5, r2 in 1i128..=25,
            kn in 1i128..=6, kd in 1i128..=6,
        ) {
            let (a, b, k) = (circle(c1x, c1y, r1), circle(c2x, c2y, r2), Q::new(kn, kd));
            let scaled = carrier::circle_circle(&scale_circle(&a, &k), &scale_circle(&b, &k));
            let expect = scaled_result(&carrier::circle_circle(&a, &b), &k);
            prop_assert_eq!(scaled, expect);
        }
    }

    // --- full-pipeline stratum-weighted smoke ---

    use crate::spine::arrange_events;
    use certify_core::Verdict;

    /// Touch-vertex count of an arrangement of carriers (always `Verified`).
    fn events(carriers: &[Carrier]) -> usize {
        match arrange_events(&to_edges(carriers)) {
            Verdict::Verified((set, _)) => set.len(),
            _ => unreachable!("degree-≤2 arrangement is always Verified"),
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(128))]

        /// Over stratum-weighted inputs (degenerate-heavy under
        /// `ARRANGE_STRATUM_WEIGHT`): the full pipeline `decompose → arrange_events`
        /// runs without panic; the independent resultant count oracle agrees on
        /// every carrier pair; and the touch-vertex count is invariant under a
        /// rational rigid motion.
        #[test]
        fn stratum_pipeline_invariants(carriers in stratum()) {
            let n = events(&carriers);
            for i in 0..carriers.len() {
                for j in (i + 1)..carriers.len() {
                    let ok = match (&carriers[i], &carriers[j]) {
                        (Carrier::Line(l), Carrier::Circle(c))
                        | (Carrier::Circle(c), Carrier::Line(l)) => resultant_agrees_line_circle(l, c),
                        (Carrier::Circle(a), Carrier::Circle(b)) => resultant_agrees_circle_circle(a, b),
                        (Carrier::Line(_), Carrier::Line(_)) => true,
                    };
                    prop_assert!(ok);
                }
            }
            let moved = move_carriers(&carriers, &rigid(2, 1, 3, -1));
            prop_assert_eq!(n, events(&moved));
        }
    }
}
