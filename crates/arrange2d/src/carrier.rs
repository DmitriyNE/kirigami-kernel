//! Carrier ∩ carrier solving (M3a Phase 1). Line∩line → a rational point;
//! line∩circle & circle∩circle → a quadratic whose two roots are degree-≤2
//! `lattice::Surd`s sharing one radical `d = Δ` (`Δ<0` → none, `Δ=0` → tangency
//! identity, `Δ>0` → two points). At most two points, degree ≤ 2.

use crate::predicates;
use geom::content::{Circle, Line, Point2};
use lattice::{Backend, Bignum, Rat, Surd};

/// The intersection of two carriers — degree ≤ 2, so at most two points, plus the
/// shared-carrier degeneracy (identical line/circle carriers). A shared carrier's
/// 1D coincidence lattice is stage-2 (slice 3c), so M3a reports it and stops: the
/// spine emits **no** transverse/tangent events for a `SharedCarrier`.
///
/// `Two` is returned in the sweep order (lexicographic on [`Point2`]) so the
/// result is canonical regardless of the ± branch labelling.
#[derive(Clone, Debug)]
pub enum Intersections<B: Backend = Bignum> {
    /// Disjoint carriers (parallel-distinct lines, non-meeting line/circle, …).
    Empty,
    /// A single point — transverse line∩line, or a tangency (`Δ = 0`).
    One(Point2<B>),
    /// Two distinct points; their coordinates share the radical `d = Δ`.
    Two(Point2<B>, Point2<B>),
    /// Identical carriers (coincident lines / equal circles) — deferred to 3c.
    SharedCarrier,
}

// Manual PartialEq/Eq: the backend `B` is not itself `PartialEq` (see the
// `geom::content` primitives), so we compare structurally via `Point2`'s own
// (radical-safe) equality rather than deriving a `B: PartialEq` bound.
impl<B: Backend> PartialEq for Intersections<B> {
    fn eq(&self, o: &Self) -> bool {
        use Intersections::*;
        match (self, o) {
            (Empty, Empty) | (SharedCarrier, SharedCarrier) => true,
            (One(p), One(q)) => p == q,
            (Two(p1, p2), Two(q1, q2)) => p1 == q1 && p2 == q2,
            _ => false,
        }
    }
}
impl<B: Backend> Eq for Intersections<B> {}

/// Two points in sweep order.
fn ordered<B: Backend>(p: Point2<B>, q: Point2<B>) -> Intersections<B> {
    if p <= q {
        Intersections::Two(p, q)
    } else {
        Intersections::Two(q, p)
    }
}

/// Line ∩ line over ℚ: a single rational point (transverse), [`Empty`] (parallel
/// & distinct), or [`SharedCarrier`] (coincident lines).
///
/// [`Empty`]: Intersections::Empty
/// [`SharedCarrier`]: Intersections::SharedCarrier
pub fn line_line<B: Backend>(la: &Line<B>, lb: &Line<B>) -> Intersections<B> {
    // det = a_A·b_B − b_A·a_B  (the (a,b) minor); zero ⇔ parallel.
    let det = predicates::minor_ab(la, lb);
    if det.is_zero() {
        return if predicates::coincident(la, lb) {
            Intersections::SharedCarrier
        } else {
            Intersections::Empty
        };
    }
    // Cramer on { a·x + b·y = −c }:
    //   x = (b_A·c_B − b_B·c_A) / det,   y = (a_B·c_A − a_A·c_B) / det.
    let x = la.b.mul(&lb.c).sub(&lb.b.mul(&la.c)).div(&det);
    let y = lb.a.mul(&la.c).sub(&la.a.mul(&lb.c)).div(&det);
    Intersections::One(Point2::from_rat(x, y))
}

/// Line ∩ circle. With `s = a·cx + b·cy + c` (the scaled signed distance) and
/// `w = a² + b² > 0`, the discriminant is `Δ = r²·w − s²`:
/// `Δ < 0` → [`Empty`]; `Δ = 0` → the tangent point (rational foot of the
/// perpendicular); `Δ > 0` → two points `M ± (√Δ / w)·(b, −a)`, whose coordinates
/// are `Surd`s sharing the single radical `d = Δ`.
///
/// [`Empty`]: Intersections::Empty
pub fn line_circle<B: Backend>(l: &Line<B>, c: &Circle<B>) -> Intersections<B> {
    let w = l.a.mul(&l.a).add(&l.b.mul(&l.b)); // a² + b²
    debug_assert!(!w.is_zero(), "line_circle: degenerate line a = b = 0");
    if w.is_zero() {
        return Intersections::Empty;
    }
    let s = l.a.mul(&c.cx).add(&l.b.mul(&c.cy)).add(&l.c); // a·cx + b·cy + c
    let disc = c.r2.mul(&w).sub(&s.mul(&s)); // Δ = r²·w − s²

    // Foot of the perpendicular from the centre: M = C − (s/w)·(a, b) — rational.
    let mx = c.cx.sub(&s.mul(&l.a).div(&w));
    let my = c.cy.sub(&s.mul(&l.b).div(&w));

    match disc.sign() {
        -1 => Intersections::Empty,
        0 => Intersections::One(Point2::from_rat(mx, my)),
        _ => {
            // ± (√Δ / w)·(b, −a): x picks up +b/w·√Δ, y picks up −a/w·√Δ.
            let bx = l.b.div(&w);
            let ay = l.a.div(&w).neg();
            let plus = Point2 {
                x: Surd::new(mx.clone(), bx.clone(), disc.clone()),
                y: Surd::new(my.clone(), ay.clone(), disc.clone()),
            };
            let minus = Point2 {
                x: Surd::new(mx, bx.neg(), disc.clone()),
                y: Surd::new(my, ay.neg(), disc),
            };
            ordered(plus, minus)
        }
    }
}

/// Circle ∩ circle, reduced to the radical line ∩ either circle. The radical line
/// `C₁ − C₂` is `a = 2(c₂x − c₁x)`, `b = 2(c₂y − c₁y)`,
/// `c = (c₁x² + c₁y² − r₁²) − (c₂x² + c₂y² − r₂²)`. Concentric circles have a
/// degenerate radical line: [`SharedCarrier`] when `r₁² = r₂²` (coincident), else
/// [`Empty`].
///
/// [`Empty`]: Intersections::Empty
/// [`SharedCarrier`]: Intersections::SharedCarrier
pub fn circle_circle<B: Backend>(c1: &Circle<B>, c2: &Circle<B>) -> Intersections<B> {
    let two = Rat::from_i128(2);
    let a = two.mul(&c2.cx.sub(&c1.cx));
    let b = two.mul(&c2.cy.sub(&c1.cy));
    if a.is_zero() && b.is_zero() {
        // Concentric: same carrier iff equal r², otherwise no intersection.
        return if c1.r2 == c2.r2 {
            Intersections::SharedCarrier
        } else {
            Intersections::Empty
        };
    }
    // c = (c₁x² + c₁y² − r₁²) − (c₂x² + c₂y² − r₂²)
    let p1 = c1.cx.mul(&c1.cx).add(&c1.cy.mul(&c1.cy)).sub(&c1.r2);
    let p2 = c2.cx.mul(&c2.cx).add(&c2.cy.mul(&c2.cy)).sub(&c2.r2);
    let radical = Line {
        a,
        b,
        c: p1.sub(&p2),
    };
    line_circle(&radical, c1)
}

#[cfg(test)]
mod tests {
    use super::*;

    type Q = Rat<Bignum>;
    type P = Point2<Bignum>;

    fn line(a: i128, b: i128, c: i128) -> Line<Bignum> {
        Line {
            a: Q::from_i128(a),
            b: Q::from_i128(b),
            c: Q::from_i128(c),
        }
    }
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
    /// `(ax + bx·√d, ay + by·√d)`.
    fn sp(ax: Q, bx: Q, ay: Q, by: Q, d: Q) -> P {
        Point2 {
            x: Surd::new(ax, bx, d.clone()),
            y: Surd::new(ay, by, d),
        }
    }

    // --- exact self-contained oracle: a point lies on its carriers ---

    /// `a·x + b·y + c` as a `Surd` (rational coefficients, one shared radical).
    fn line_residual(l: &Line<Bignum>, p: &P) -> Surd<Bignum> {
        p.x.scale(&l.a)
            .add(&p.y.scale(&l.b))
            .unwrap_surd()
            .add(&Surd::from_rat(l.c.clone()))
            .unwrap_surd()
    }
    /// `(x − cx)² + (y − cy)² − r²` as a `Surd`.
    fn circle_residual(c: &Circle<Bignum>, p: &P) -> Surd<Bignum> {
        let dx = p.x.sub(&Surd::from_rat(c.cx.clone())).unwrap_surd();
        let dy = p.y.sub(&Surd::from_rat(c.cy.clone())).unwrap_surd();
        dx.square()
            .add(&dy.square())
            .unwrap_surd()
            .sub(&Surd::from_rat(c.r2.clone()))
            .unwrap_surd()
    }
    fn on_line(l: &Line<Bignum>, p: &P) -> bool {
        line_residual(l, p).sign() == 0
    }
    fn on_circle(c: &Circle<Bignum>, p: &P) -> bool {
        circle_residual(c, p).sign() == 0
    }

    // --- line ∩ line ---

    #[test]
    fn line_line_transverse() {
        // x-axis ∩ y-axis = origin.
        assert_eq!(
            line_line(&line(0, 1, 0), &line(1, 0, 0)),
            Intersections::One(rp(0, 0))
        );
    }

    #[test]
    fn line_line_transverse_offset() {
        // x = 2  ∩  y = 3  →  (2, 3).
        let got = line_line(&line(1, 0, -2), &line(0, 1, -3));
        assert_eq!(got, Intersections::One(rp(2, 3)));
    }

    #[test]
    fn line_line_parallel_distinct_empty() {
        assert_eq!(
            line_line(&line(0, 1, 0), &line(0, 1, -1)),
            Intersections::Empty
        );
    }

    #[test]
    fn line_line_coincident_shared() {
        assert_eq!(
            line_line(&line(1, 2, 3), &line(2, 4, 6)),
            Intersections::SharedCarrier
        );
    }

    // --- line ∩ circle ---

    #[test]
    fn line_circle_secant_surd() {
        // y = 0 through the unit-√2 circle at the origin ⇒ (±√2, 0).
        let l = line(0, 1, 0);
        let c = circle(0, 0, 2);
        let expect = ordered(
            sp(
                Q::from_i128(0),
                Q::from_i128(1),
                Q::from_i128(0),
                Q::from_i128(0),
                Q::from_i128(2),
            ),
            sp(
                Q::from_i128(0),
                Q::from_i128(-1),
                Q::from_i128(0),
                Q::from_i128(0),
                Q::from_i128(2),
            ),
        );
        assert_eq!(line_circle(&l, &c), expect);
    }

    #[test]
    fn line_circle_tangent_one() {
        // y = 1 tangent to the unit circle at the origin ⇒ (0, 1).
        assert_eq!(
            line_circle(&line(0, 1, -1), &circle(0, 0, 1)),
            Intersections::One(rp(0, 1))
        );
    }

    #[test]
    fn line_circle_external_empty() {
        // y = 2 misses the unit circle.
        assert_eq!(
            line_circle(&line(0, 1, -2), &circle(0, 0, 1)),
            Intersections::Empty
        );
    }

    #[test]
    fn line_circle_points_lie_on_both() {
        // A skew line + off-centre circle: the two Surd points satisfy both eqns.
        let l = line(1, 2, -1); // x + 2y = 1
        let c = circle(1, 1, 5);
        match line_circle(&l, &c) {
            Intersections::Two(p, q) => {
                assert!(on_line(&l, &p) && on_circle(&c, &p));
                assert!(on_line(&l, &q) && on_circle(&c, &q));
                assert!(p < q); // canonical order
            }
            other => panic!("expected two points, got {other:?}"),
        }
    }

    // --- circle ∩ circle ---

    #[test]
    fn circle_circle_two_points_lie_on_both() {
        // Unit circles at (0,0) and (1,0) meet at (1/2, ±√3/2).
        let c1 = circle(0, 0, 1);
        let c2 = circle(1, 0, 1);
        match circle_circle(&c1, &c2) {
            Intersections::Two(p, q) => {
                // Δ = 3 here, so y = ±(1/2)·√3 = ±√3/2.
                let half = Q::new(1, 2);
                let three = Q::from_i128(3);
                let hi = sp(
                    half.clone(),
                    Q::from_i128(0),
                    Q::from_i128(0),
                    half.clone(),
                    three.clone(),
                );
                let lo = sp(
                    half.clone(),
                    Q::from_i128(0),
                    Q::from_i128(0),
                    half.neg(),
                    three,
                );
                assert_eq!(Intersections::Two(p.clone(), q.clone()), ordered(hi, lo));
                assert!(on_circle(&c1, &p) && on_circle(&c2, &p));
                assert!(on_circle(&c1, &q) && on_circle(&c2, &q));
            }
            other => panic!("expected two points, got {other:?}"),
        }
    }

    #[test]
    fn circle_circle_external_tangent_one() {
        // Unit circles at (0,0) and (2,0) touch at (1, 0).
        assert_eq!(
            circle_circle(&circle(0, 0, 1), &circle(2, 0, 1)),
            Intersections::One(rp(1, 0))
        );
    }

    #[test]
    fn circle_circle_disjoint_empty() {
        assert_eq!(
            circle_circle(&circle(0, 0, 1), &circle(5, 0, 1)),
            Intersections::Empty
        );
    }

    #[test]
    fn circle_circle_concentric_shared_or_empty() {
        assert_eq!(
            circle_circle(&circle(0, 0, 4), &circle(0, 0, 4)),
            Intersections::SharedCarrier
        );
        assert_eq!(
            circle_circle(&circle(0, 0, 4), &circle(0, 0, 9)),
            Intersections::Empty
        );
    }

    // --- properties ---

    use proptest::prelude::*;

    /// A rational rotation `(cos, sin)` with `cos² + sin² = 1`, from the rational
    /// parametrisation of the unit circle: `cos = (u²−v²)/(u²+v²)`,
    /// `sin = 2uv/(u²+v²)` for integers `(u, v) ≠ (0, 0)`.
    fn rot(u: i128, v: i128) -> (Q, Q) {
        let den = u * u + v * v;
        (Q::new(u * u - v * v, den), Q::new(2 * u * v, den))
    }

    /// Apply the rigid motion `p ↦ R·p + t` to a line's `(a, b, c)`. For an
    /// orthogonal `R`, the normal rotates (`n' = R·n`) and `c' = c − n'·t`.
    fn rigid_line(l: &Line<Bignum>, co: &Q, si: &Q, tx: &Q, ty: &Q) -> Line<Bignum> {
        let a = co.mul(&l.a).sub(&si.mul(&l.b)); // co·a − si·b
        let b = si.mul(&l.a).add(&co.mul(&l.b)); // si·a + co·b
        let c = l.c.sub(&a.mul(tx).add(&b.mul(ty)));
        Line { a, b, c }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(48))]

        /// PARALLEL and COINCIDENT are exactly invariant under a rational rigid
        /// motion (the normal cross scales by `det R = 1`).
        #[test]
        fn predicates_rigid_motion_invariant(
            a1 in -6i128..=6, b1 in -6i128..=6, c1 in -6i128..=6,
            a2 in -6i128..=6, b2 in -6i128..=6, c2 in -6i128..=6,
            u in -4i128..=4, v in -4i128..=4,
            tx in -5i128..=5, ty in -5i128..=5,
        ) {
            prop_assume!(u != 0 || v != 0);
            let (l1, l2) = (line(a1, b1, c1), line(a2, b2, c2));
            let (co, si) = rot(u, v);
            let (tx, ty) = (Q::from_i128(tx), Q::from_i128(ty));
            let m1 = rigid_line(&l1, &co, &si, &tx, &ty);
            let m2 = rigid_line(&l2, &co, &si, &tx, &ty);
            prop_assert_eq!(predicates::parallel(&l1, &l2), predicates::parallel(&m1, &m2));
            prop_assert_eq!(predicates::coincident(&l1, &l2), predicates::coincident(&m1, &m2));
        }

        /// Every point `line_circle` returns lies exactly on both carriers
        /// (residual-zero oracle), and two points come back in sweep order.
        #[test]
        fn line_circle_points_on_carriers(
            a in -6i128..=6, b in -6i128..=6, c in -8i128..=8,
            cx in -5i128..=5, cy in -5i128..=5, r2 in 1i128..=30,
        ) {
            prop_assume!(a != 0 || b != 0);
            let (l, cc) = (line(a, b, c), circle(cx, cy, r2));
            match line_circle(&l, &cc) {
                Intersections::Two(p, q) => {
                    prop_assert!(on_line(&l, &p) && on_circle(&cc, &p));
                    prop_assert!(on_line(&l, &q) && on_circle(&cc, &q));
                    prop_assert!(p < q);
                }
                Intersections::One(p) => {
                    prop_assert!(on_line(&l, &p) && on_circle(&cc, &p));
                }
                Intersections::Empty => {}
                Intersections::SharedCarrier => {
                    prop_assert!(false, "line∩circle can never be SharedCarrier")
                }
            }
        }

        /// Every point `circle_circle` returns lies exactly on both circles.
        #[test]
        fn circle_circle_points_on_carriers(
            c1x in -5i128..=5, c1y in -5i128..=5, r1 in 1i128..=25,
            c2x in -5i128..=5, c2y in -5i128..=5, r2 in 1i128..=25,
        ) {
            let (a, b) = (circle(c1x, c1y, r1), circle(c2x, c2y, r2));
            match circle_circle(&a, &b) {
                Intersections::Two(p, q) => {
                    prop_assert!(on_circle(&a, &p) && on_circle(&b, &p));
                    prop_assert!(on_circle(&a, &q) && on_circle(&b, &q));
                    prop_assert!(p < q);
                }
                Intersections::One(p) => {
                    prop_assert!(on_circle(&a, &p) && on_circle(&b, &p));
                }
                _ => {}
            }
        }
    }
}
