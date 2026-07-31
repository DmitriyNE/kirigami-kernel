//! §8.3 azimuth calculus (M3 slice 3c). The half-angle stereographic tag of a
//! point on a circle, its exact CCW angular order, and the signed pole-crossing
//! winding integer — the angular primitive the coincidence lattice (3c) and the
//! future half-edge azimuth sort (3d) both use.
//!
//! For a point `p` on circle `(C, r²)` with normal `n = p − C`, the tag is
//! `t = n_y / (n_x + √r²) = tan(θ/2)` (spec §8.3: `t = n_y/(n_x+ρ)`), stored
//! unevaluated as the `(num, den)` pair of `Surd`s so the angle never
//! materializes. Both components live in the single radical `d = r²`, so every
//! comparison is an exact `Surd` sign. The pole `den = 0` is exactly the x-min
//! extremal `L = (cx − √r², cy)` (θ = π) — the decomposition's axis-aligned chart
//! puts the half-angle pole at the x-min extremal, so a decomposed x-monotone
//! piece has winding 0 structurally.

use core::cmp::Ordering;
use geom::content::{Circle, Point2};
use lattice::{Backend, Bignum, Rat, Surd};

/// `√(r²)` as a `Surd` (radical `d = r²`).
fn root_r2<B: Backend>(c: &Circle<B>) -> Surd<B> {
    Surd::new(Rat::from_i128(0), Rat::from_i128(1), c.r2.clone())
}

/// The half-angle stereographic tag of a point on a circle, `t = num/den` with
/// `num = p_y − cy`, `den = (p_x − cx) + √r²` — both `Surd`s in the radical `r²`.
pub struct Tag<B: Backend = Bignum> {
    pub num: Surd<B>,
    pub den: Surd<B>,
}

impl<B: Backend> Tag<B> {
    /// The tag of `p` (assumed on `c`). No division is performed.
    pub fn of(c: &Circle<B>, p: &Point2<B>) -> Self {
        let num = p.y.sub(&Surd::from_rat(c.cy.clone())).unwrap_surd();
        let den =
            p.x.sub(&Surd::from_rat(c.cx.clone()))
                .unwrap_surd()
                .add(&root_r2(c))
                .unwrap_surd();
        Tag { num, den }
    }
    /// Is this the pole (θ = π, the x-min extremal `L`)? Then `t = ∞`.
    pub fn is_pole(&self) -> bool {
        self.den.sign() == 0
    }
}

/// The CCW angular class of `p` on `c`, keyed so the linear order starts at `R`
/// and runs CCW: `R (0) < upper (1) < L (2) < lower (3)`. `R`/`L` are the two
/// points on the centre line `y = cy`.
fn phase<B: Backend>(c: &Circle<B>, p: &Point2<B>) -> u8 {
    match p.y.cmp(&Surd::from_rat(c.cy.clone())) {
        Ordering::Greater => 1, // upper
        Ordering::Less => 3,    // lower
        Ordering::Equal => {
            if p.x.cmp(&Surd::from_rat(c.cx.clone())) == Ordering::Greater {
                0 // R (x > cx)
            } else {
                2 // L (x < cx) — the pole
            }
        }
    }
}

/// Exact CCW angular order of two points on `c` (from `R`, CCW through upper, `L`,
/// lower). No angle is materialized; within a half the tags are compared by the
/// cross-product `num_a·den_b − num_b·den_a` (both `den > 0` off the pole).
pub fn tag_cmp<B: Backend>(c: &Circle<B>, a: &Point2<B>, b: &Point2<B>) -> Ordering {
    let (pa, pb) = (phase(c, a), phase(c, b));
    if pa != pb {
        return pa.cmp(&pb);
    }
    if pa == 0 || pa == 2 {
        return Ordering::Equal; // both R, or both L — a unique point
    }
    // same half (den > 0 for both): t_a ⋛ t_b ⟺ sign(num_a·den_b − num_b·den_a).
    let (ta, tb) = (Tag::of(c, a), Tag::of(c, b));
    let cross = ta
        .num
        .mul(&tb.den)
        .unwrap_surd()
        .sub(&tb.num.mul(&ta.den).unwrap_surd())
        .unwrap_surd();
    cross.sign().cmp(&0)
}

/// The pole `L = (cx − √r², cy)` of the circle (θ = π).
pub fn pole<B: Backend>(c: &Circle<B>) -> Point2<B> {
    Point2 {
        x: Surd::new(c.cx.clone(), Rat::from_i128(-1), c.r2.clone()),
        y: Surd::from_rat(c.cy.clone()),
    }
}

/// Is the pole `L` strictly interior to the CCW arc `a → b` (`a != b`)? Decided in
/// the cut-at-`R` linear order: a non-wrapping arc (`a < b`) contains `L` iff
/// `a < L < b`; a wrapping arc (`a > b`) contains it iff `L` is past `a` or before
/// `b`.
fn pole_inside_ccw<B: Backend>(c: &Circle<B>, a: &Point2<B>, b: &Point2<B>) -> bool {
    let l = pole(c);
    let (al, lb) = (tag_cmp(c, a, &l), tag_cmp(c, &l, b));
    match tag_cmp(c, a, b) {
        Ordering::Less => al == Ordering::Less && lb == Ordering::Less,
        Ordering::Greater => al == Ordering::Less || lb == Ordering::Less,
        Ordering::Equal => false,
    }
}

/// The signed winding of the oriented arc `start → end` around the pole: `+1` if
/// the CCW traversal crosses `L`, `−1` for a CW arc that does, else `0`. A single
/// x-monotone decomposed piece (endpoints among the extrema, no interior pole)
/// winds 0. Chains sum their spans' windings (spec §8.3: ±2π per crossing).
pub fn winding_of_arc<B: Backend>(
    c: &Circle<B>,
    start: &Point2<B>,
    end: &Point2<B>,
    ccw: bool,
) -> i32 {
    let (a, b) = if ccw { (start, end) } else { (end, start) };
    if a == b {
        return 0;
    }
    if pole_inside_ccw(c, a, b) {
        if ccw { 1 } else { -1 }
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice::Rat;

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

    /// r² = 25 ⇒ exact rational points (±5,0), (0,±5), (3,±4), (−3,±4).
    fn c5() -> Circle<Bignum> {
        circle(0, 0, 25)
    }

    #[test]
    fn phases_and_pole() {
        let c = c5();
        assert!(Tag::of(&c, &rp(-5, 0)).is_pole()); // L
        assert!(!Tag::of(&c, &rp(5, 0)).is_pole()); // R
        assert!(!Tag::of(&c, &rp(0, 5)).is_pole()); // top
        // pole() is L
        assert_eq!(pole(&c), rp(-5, 0));
    }

    #[test]
    fn ccw_angular_order() {
        let c = c5();
        // CCW from R(θ=0): R < (3,4) < (0,5) < (−3,4) < L < (−3,−4) < (0,−5) < (3,−4)
        let ccw = [
            rp(5, 0),   // R, θ=0
            rp(3, 4),   // θ≈53°
            rp(0, 5),   // θ=90°
            rp(-3, 4),  // θ≈127°
            rp(-5, 0),  // L, θ=180°
            rp(-3, -4), // θ≈233°
            rp(0, -5),  // θ=270°
            rp(3, -4),  // θ≈307°
        ];
        for w in ccw.windows(2) {
            assert_eq!(
                tag_cmp(&c, &w[0], &w[1]),
                Ordering::Less,
                "{:?} < {:?}",
                w[0],
                w[1]
            );
        }
        // antisymmetry + reflexivity
        assert_eq!(tag_cmp(&c, &rp(3, 4), &rp(3, 4)), Ordering::Equal);
        assert_eq!(tag_cmp(&c, &rp(0, 5), &rp(3, 4)), Ordering::Greater);
    }

    #[test]
    fn winding_crosses_pole() {
        let c = c5();
        // CCW upper arc R→top does NOT cross L.
        assert_eq!(winding_of_arc(&c, &rp(5, 0), &rp(0, 5), true), 0);
        // CCW arc top→bottom the long way (through L) crosses L.
        assert_eq!(winding_of_arc(&c, &rp(0, 5), &rp(0, -5), true), 1);
        // the same arc CW does not cross L (it goes the short way, R side).
        assert_eq!(winding_of_arc(&c, &rp(0, 5), &rp(0, -5), false), 0);
        // CW arc that crosses L: bottom→top the long way CW.
        assert_eq!(winding_of_arc(&c, &rp(0, -5), &rp(0, 5), false), -1);
        // an arc ending AT the pole does not have it strictly interior.
        assert_eq!(winding_of_arc(&c, &rp(0, 5), &rp(-5, 0), true), 0);
    }

    // --- properties ---

    use crate::testgen::{rigid, rigid_circle, rigid_pt};
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// `tag_cmp` is a consistent strict total order (antisymmetric; a sort of
        /// three points is transitive) and depends only on the position relative to
        /// the centre — so it is invariant under a rational TRANSLATION. (It is NOT
        /// rotation-invariant: the linear order is cut at the rightmost point R,
        /// which the rotation moves — only the *cyclic* order is a rigid invariant.)
        #[test]
        fn tag_cmp_order_and_translation_invariant(
            cx in -3i128..=3, cy in -3i128..=3, r in 1i128..=6,
            t1n in -6i128..=6, t1d in 1i128..=6,
            t2n in -6i128..=6, t2d in 1i128..=6,
            t3n in -6i128..=6, t3d in 1i128..=6,
            dx in -4i128..=4, dy in -4i128..=4,
        ) {
            let (cx, cy, r) = (Q::from_i128(cx), Q::from_i128(cy), Q::from_i128(r));
            let c = Circle { cx: cx.clone(), cy: cy.clone(), r2: r.mul(&r) };
            let on = |tn: i128, td: i128| crate::testgen::on_circle_pt(&cx, &cy, &r, tn, td);
            let (a, b, cc) = (on(t1n, t1d), on(t2n, t2d), on(t3n, t3d));

            // antisymmetry
            prop_assert_eq!(tag_cmp(&c, &a, &b), tag_cmp(&c, &b, &a).reverse());
            // transitivity: sort the three, then the order is consistent
            let mut v = [&a, &b, &cc];
            v.sort_by(|x, y| tag_cmp(&c, x, y));
            prop_assert!(tag_cmp(&c, v[0], v[1]) != Ordering::Greater);
            prop_assert!(tag_cmp(&c, v[1], v[2]) != Ordering::Greater);
            prop_assert!(tag_cmp(&c, v[0], v[2]) != Ordering::Greater);

            // translation invariance (p − C is unchanged)
            let m = rigid(1, 0, dx, dy); // identity rotation + translation
            let c2 = rigid_circle(&c, &m);
            prop_assert_eq!(
                tag_cmp(&c, &a, &b),
                tag_cmp(&c2, &rigid_pt(&a, &m), &rigid_pt(&b, &m))
            );
        }
    }
}
