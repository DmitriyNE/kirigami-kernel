//! Primitive-tag classification of a [`Chart`] (spec §3.6): which ruled-surface
//! family the chart belongs to, with an exact witness (apex, axis, …).
//!
//! [`classify`] returns the recognized [`Tag`], or `None` for a generic chart. Each tag
//! is decided exactly — a candidate witness is solved from a few sample points, then
//! **verified as a polynomial identity** over ℚ(σ), so a `Some` is a proof, never an
//! approximation.
//!
//! - [`Tag::Cone`] — the rulings pass through a common apex `A`: `h ≡ n·A`. The
//!   support-through-apex solve mints the apex ([`cone_apex`]).
//! - [`Tag::Cylinder`] — the normal traces a great circle about an axis `a` (`n·a ≡ 0`),
//!   the axis being the direction only ([`cylinder_axis`]).
//!
//! # Example
//!
//! ```
//! use geom::chart::Chart;
//! use geom::tags::{classify, Tag};
//! use lattice::{Bignum, Poly, Rat, RatFunc};
//!
//! let poly = |cs: &[i128]| Poly::<Bignum>::from_coeffs(cs.iter().map(|&c| Rat::from_i128(c)).collect());
//! // A rational cone with apex at the origin: q(σ) = (9, 4, 4σ, 9σ), h ≡ 0.
//! let q = [poly(&[9]), poly(&[4]), poly(&[0, 4]), poly(&[0, 9])];
//! let chart = Chart::new(q, RatFunc::zero());
//!
//! match classify(&chart) {
//!     Some(Tag::Cone { apex }) => {
//!         assert_eq!(apex, [Rat::from_i128(0), Rat::from_i128(0), Rat::from_i128(0)]);
//!     }
//!     other => panic!("expected a cone, got {other:?}"),
//! }
//! ```

use crate::chart::Chart;
use core::fmt;
use lattice::{Backend, Bignum, Rat, RatFunc, Vec3Rat};

/// The primitive family of a chart, with its exact witness (spec §3.6).
pub enum Tag<B: Backend = Bignum> {
    /// A cone: every ruling passes through the apex `A`, i.e. `h ≡ n·A`.
    Cone {
        /// The common apex point.
        apex: [Rat<B>; 3],
    },
    /// A (generalized) cylinder: the normal lies on the great circle `n·a ≡ 0`.
    Cylinder {
        /// The cylinder axis (direction only, unnormalized).
        axis: [Rat<B>; 3],
    },
}

/// The apex `A` of a cone chart (`h ≡ n·A`), or `None` if the chart is not a cone.
///
/// Solves the linear system `n(σⱼ)·A = h(σⱼ)` at three sample points whose normals are
/// independent, then verifies `h ≡ n·A` exactly. A cylinder's normal matrix is singular
/// (a normal component is identically zero), so no independent triple exists and this
/// returns `None`.
pub fn cone_apex<B: Backend>(chart: &Chart<B>) -> Option<[Rat<B>; 3]> {
    let mut ns: Vec<[Rat<B>; 3]> = Vec::new();
    let mut hs: Vec<Rat<B>> = Vec::new();
    for k in [0i128, 1, 2, 3, 4, -1, -2] {
        let s = Rat::from_i128(k);
        if let (Some(n), Some(h)) = (chart.normal().eval(&s), chart.support().eval(&s)) {
            ns.push(n);
            hs.push(h);
        }
    }
    let len = ns.len();
    for i in 0..len {
        for j in (i + 1)..len {
            for k in (j + 1)..len {
                let m = [ns[i].clone(), ns[j].clone(), ns[k].clone()];
                let d = det3(&m);
                if d.is_zero() {
                    continue;
                }
                let b = [hs[i].clone(), hs[j].clone(), hs[k].clone()];
                let a = solve3(&m, &b, &d);
                // Exact verification: h ≡ n·A as rational functions.
                return if chart.support() == &normal_dot(chart.normal(), &a) {
                    Some(a)
                } else {
                    None
                };
            }
        }
    }
    None
}

/// The axis `a` of a cylinder chart (`n·a ≡ 0`), or `None` if the chart is not a
/// cylinder. Takes the cross product of two independent normals as the candidate axis,
/// then verifies `n·a ≡ 0` exactly.
pub fn cylinder_axis<B: Backend>(chart: &Chart<B>) -> Option<[Rat<B>; 3]> {
    let mut ns: Vec<[Rat<B>; 3]> = Vec::new();
    for k in [0i128, 1, 2, 3, 4, -1, -2] {
        let s = Rat::from_i128(k);
        if let Some(n) = chart.normal().eval(&s) {
            ns.push(n);
        }
    }
    for i in 0..ns.len() {
        for j in (i + 1)..ns.len() {
            let a = cross3(&ns[i], &ns[j]);
            if a.iter().all(Rat::is_zero) {
                continue; // parallel normals — no axis from this pair
            }
            if normal_dot(chart.normal(), &a).is_zero() {
                return Some(a);
            }
        }
    }
    None
}

/// Classify a chart into its primitive [`Tag`] (cone, then cylinder), or `None` for a
/// generic chart.
pub fn classify<B: Backend>(chart: &Chart<B>) -> Option<Tag<B>> {
    if let Some(apex) = cone_apex(chart) {
        return Some(Tag::Cone { apex });
    }
    if let Some(axis) = cylinder_axis(chart) {
        return Some(Tag::Cylinder { axis });
    }
    None
}

/// `n·a` as a rational function, for a constant vector `a` (shares `n`'s denominator).
fn normal_dot<B: Backend>(n: &Vec3Rat<B>, a: &[Rat<B>; 3]) -> RatFunc<B> {
    let num = n.num()[0]
        .scale(&a[0])
        .add(&n.num()[1].scale(&a[1]))
        .add(&n.num()[2].scale(&a[2]));
    RatFunc::new(num, n.den().clone())
}

fn cross3<B: Backend>(a: &[Rat<B>; 3], b: &[Rat<B>; 3]) -> [Rat<B>; 3] {
    [
        a[1].mul(&b[2]).sub(&a[2].mul(&b[1])),
        a[2].mul(&b[0]).sub(&a[0].mul(&b[2])),
        a[0].mul(&b[1]).sub(&a[1].mul(&b[0])),
    ]
}

fn det3<B: Backend>(m: &[[Rat<B>; 3]; 3]) -> Rat<B> {
    let c0 = m[1][1].mul(&m[2][2]).sub(&m[1][2].mul(&m[2][1]));
    let c1 = m[1][0].mul(&m[2][2]).sub(&m[1][2].mul(&m[2][0]));
    let c2 = m[1][0].mul(&m[2][1]).sub(&m[1][1].mul(&m[2][0]));
    m[0][0]
        .mul(&c0)
        .sub(&m[0][1].mul(&c1))
        .add(&m[0][2].mul(&c2))
}

/// Cramer's rule: solve `m·x = b` given a nonzero `det`.
fn solve3<B: Backend>(m: &[[Rat<B>; 3]; 3], b: &[Rat<B>; 3], det: &Rat<B>) -> [Rat<B>; 3] {
    let col = |i: usize| {
        let mut mi = m.clone();
        for (row, mrow) in mi.iter_mut().enumerate() {
            mrow[i] = b[row].clone();
        }
        det3(&mi).div(det)
    };
    [col(0), col(1), col(2)]
}

impl<B: Backend> Clone for Tag<B> {
    fn clone(&self) -> Self {
        match self {
            Tag::Cone { apex } => Tag::Cone { apex: apex.clone() },
            Tag::Cylinder { axis } => Tag::Cylinder { axis: axis.clone() },
        }
    }
}
impl<B: Backend> PartialEq for Tag<B> {
    fn eq(&self, o: &Self) -> bool {
        match (self, o) {
            (Tag::Cone { apex: a }, Tag::Cone { apex: b }) => a == b,
            (Tag::Cylinder { axis: a }, Tag::Cylinder { axis: b }) => a == b,
            _ => false,
        }
    }
}
impl<B: Backend> Eq for Tag<B> {}
impl<B: Backend> fmt::Debug for Tag<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tag::Cone { apex } => write!(f, "Cone {{ apex: {apex:?} }}"),
            Tag::Cylinder { axis } => write!(f, "Cylinder {{ axis: {axis:?} }}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice::Poly;

    type Q = Rat<Bignum>;
    fn poly(cs: &[i128]) -> Poly<Bignum> {
        Poly::from_coeffs(cs.iter().map(|&c| Q::from_i128(c)).collect())
    }
    fn rat3(a: i128, b: i128, c: i128) -> [Q; 3] {
        [Q::from_i128(a), Q::from_i128(b), Q::from_i128(c)]
    }

    /// The rational cone q(σ) = (9, 4, 4σ, 9σ), h ≡ 0 — apex at the origin, n·ẑ ≡ 65/97.
    fn cone() -> Chart<Bignum> {
        let q = [poly(&[9]), poly(&[4]), poly(&[0, 4]), poly(&[0, 9])];
        Chart::new(q, RatFunc::zero())
    }
    /// The x-rotation q(σ) = (1, σ, 0, 0) — a cylinder about the x-axis (n_x ≡ 0).
    fn cylinder() -> Chart<Bignum> {
        let q = [poly(&[1]), poly(&[0, 1]), poly(&[0]), poly(&[0])];
        Chart::new(q, RatFunc::zero())
    }

    #[test]
    fn cone_classifies_with_origin_apex() {
        assert_eq!(cone_apex(&cone()), Some(rat3(0, 0, 0)));
        assert_eq!(
            classify(&cone()),
            Some(Tag::Cone {
                apex: rat3(0, 0, 0)
            })
        );
    }

    #[test]
    fn cone_apex_offset() {
        // Shift the support so the apex moves to (0, 0, 1): h = n·(0,0,1) = n_z.
        let q = [poly(&[9]), poly(&[4]), poly(&[0, 4]), poly(&[0, 9])];
        let n_z = RatFunc::from_poly(poly(&[65])).div(&RatFunc::from_poly(poly(&[97]))); // 65/97
        let chart = Chart::new(q, n_z);
        assert_eq!(cone_apex(&chart), Some(rat3(0, 0, 1)));
    }

    #[test]
    fn cylinder_not_a_cone() {
        assert_eq!(cone_apex(&cylinder()), None, "a cylinder has no apex");
        assert_eq!(
            classify(&cylinder()),
            Some(Tag::Cylinder {
                axis: rat3(1, 0, 0)
            }),
        );
    }

    #[test]
    fn cone_is_not_a_cylinder() {
        assert_eq!(cylinder_axis(&cone()), None, "a cone has no cylinder axis");
    }
}
