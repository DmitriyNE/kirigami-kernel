//! The **extruded cutter's apex and walls** — one homogeneous [`Apex`] `[a : w]`, and the two
//! builders that turn a profile edge into the [`CutSurface`] it sweeps.
//!
//! A sketch-extrude cutter is *frame × profile × apex × span* (`docs/cutter-extrude-design.md`).
//! This module owns the **apex** piece and the edge → wall map; the frame and the profile region
//! sit above it, and the σ-pullback and the certificate sit below it in [`crate::cut`].
//!
//! ## One apex, not two extrusion modes
//!
//! A parallel extrusion *is* a projection from a point at infinity, so `Apex = [a : w]` covers
//! both: `w ≠ 0` is a finite cast point `a/w` (a draft angle), `w = 0` is a direction (today's
//! parallel drill). `w == 0` is an **exact** `Rat` test — no float, no tolerance — and the two
//! builders below take the apex whole, so the generatrix, the wall and the nappe selector are one
//! formula each rather than a pair of near-duplicates. Pushing a finite apex outward degrades the
//! taper to parallel continuously, with no API discontinuity.
//!
//! ## What a wall is
//!
//! The wall over a profile edge is the surface swept by the generatrices through that edge — the
//! *cone over the edge with the given apex*. Its class follows the edge class, and the apex kind
//! only changes the coefficients:
//!
//! | profile edge | apex at infinity (`w = 0`) | finite apex (`w ≠ 0`) |
//! |---|---|---|
//! | segment | plane | plane (through the apex and the edge) |
//! | ellipse | elliptic cylinder | elliptic cone |
//!
//! Both right-hand cases are [`CutSurface::Quadric`]; the segment cases are [`CutSurface::Plane`].
//! Everything stays degree ≤ 2, so every wall pulls back to a degree-≤2 rail over ℚ(σ) through
//! [`cut_mu_form`](crate::cut::cut_mu_form).
//!
//! An **arc**'s wall is the wall of the full ellipse it lies on — the surface is unbounded either
//! way, and trimming to the authored arc happens in the `(σ, µ̂)` domain, where it is a boolean
//! against the other walls rather than a surface property.
//!
//! ## Orientation
//!
//! [`CutSurface`]'s sign convention is *negative inside the solid cutter*. An ellipse wall is
//! oriented for free: the cone's interior is `F < 0`. A segment wall has no intrinsic inside, so
//! the caller picks it by **edge direction** — swapping `p0` and `p1` negates the wall normal.
//!
//! ```
//! use develop::extrude::{Apex, segment_wall};
//! use lattice::{Bignum, Rat};
//!
//! type Q = Rat<Bignum>;
//! let q = |n: i128| Q::from_i128(n);
//! // A vertical wall over the x-axis segment (0,0,0)–(1,0,0), drilled straight down.
//! let down = Apex::direction([q(0), q(0), q(-1)]).expect("nonzero direction");
//! let wall = segment_wall(&[q(0), q(0), q(0)], &[q(1), q(0), q(0)], &down).expect("a real wall");
//! // Reversing the edge gives the same plane with the opposite normal.
//! let flipped = segment_wall(&[q(1), q(0), q(0)], &[q(0), q(0), q(0)], &down).expect("a real wall");
//! assert!(matches!((&wall, &flipped),
//!     (develop::cut::CutSurface::Plane { n, .. }, develop::cut::CutSurface::Plane { n: m, .. })
//!         if n[1].add(&m[1]).is_zero()));
//! ```

use crate::cut::{CutSurface, Nappe};
use lattice::{Backend, Bignum, Rat};

/// A rational 3-vector.
type V3<B> = [Rat<B>; 3];

/// Why a wall could not be built. Every one of these is a **refusal** — the authored data does not
/// describe a surface — never a tolerance that a finer subdivision would clear.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExtrudeFault {
    /// The apex is `[0 : 0]`, which is not a point of projective space.
    DegenerateApex,
    /// The profile edge is degenerate: a segment with coincident endpoints, or an ellipse whose two
    /// conjugate semi-axes are parallel (so it spans no plane).
    DegenerateProfile,
    /// The generatrices through the edge do not sweep a surface: the apex lies **on** the segment's
    /// line, or (for `w = 0`) the extrusion direction is parallel to the segment.
    DegenerateWall,
    /// The apex lies **in** the profile's own plane (`n · (a − w·Q) = 0`). A finite apex there sees
    /// the profile edge-on and casts no cone; a direction there extrudes parallel to the plane. This
    /// is the §4.1 apex-clearance condition at build time — refused, never repaired.
    ApexInPlane,
}

/// The extrusion apex as **one homogeneous point** `[a : w]`.
///
/// `w ≠ 0` is the finite cast point `a/w`; `w = 0` is the direction `a`. See the module docs for
/// why these are one object rather than two variants.
#[derive(Clone, Debug)]
pub struct Apex<B: Backend = Bignum> {
    a: V3<B>,
    w: Rat<B>,
}

impl<B: Backend> Apex<B> {
    /// The finite cast point `p` — a drafted extrusion, whose generatrices all meet at `p`.
    pub fn point(p: V3<B>) -> Self {
        Self {
            a: p,
            w: Rat::from_i128(1),
        }
    }

    /// The direction `d` — a parallel extrusion, the apex at infinity. `None` for `d = 0`, which is
    /// not a direction.
    pub fn direction(d: V3<B>) -> Option<Self> {
        (!is_zero3(&d)).then_some(Self {
            a: d,
            w: Rat::from_i128(0),
        })
    }

    /// The raw homogeneous point `[a : w]`. `None` for `[0 : 0]`.
    pub fn homogeneous(a: V3<B>, w: Rat<B>) -> Option<Self> {
        (!is_zero3(&a) || !w.is_zero()).then_some(Self { a, w })
    }

    /// The homogeneous numerator `a`.
    pub fn a(&self) -> &V3<B> {
        &self.a
    }

    /// The homogeneous weight `w`.
    pub fn w(&self) -> &Rat<B> {
        &self.w
    }

    /// Whether this apex is at infinity (`w == 0`) — an **exact** rational test.
    pub fn is_direction(&self) -> bool {
        self.w.is_zero()
    }

    /// The finite cast point `a/w`, or `None` for a direction.
    pub fn finite(&self) -> Option<V3<B>> {
        (!self.is_direction()).then(|| scale3(&self.a, &self.w.recip()))
    }

    /// `a − w·p`, the apex taken relative to `p`. This is the one expression both builders need: at
    /// `w = 0` it is the direction, at `w = 1` it is `apex − p`, and in general it is `w·(a/w − p)`
    /// — the same vector up to a positive-or-negative scale, which every use below squares or
    /// crosses away.
    fn relative_to(&self, p: &V3<B>) -> V3<B> {
        sub3(&self.a, &scale3(p, &self.w))
    }
}

/// The wall swept by the generatrices through the segment `p0`–`p1`: the **plane through the
/// segment and the apex**.
///
/// One determinant covers both apex kinds — `n = (p1 − p0) × (a − w·p0)`, which at `w = 0` is the
/// plane containing the direction `a`. The returned normal points so that `n·X − d` is negative on
/// the side the edge direction `p0 → p1` leaves to its left about the apex; reverse the endpoints
/// to flip it (see the module docs on orientation).
pub fn segment_wall<B: Backend>(
    p0: &V3<B>,
    p1: &V3<B>,
    apex: &Apex<B>,
) -> Result<CutSurface<B>, ExtrudeFault> {
    check_apex(apex)?;
    let edge = sub3(p1, p0);
    if is_zero3(&edge) {
        return Err(ExtrudeFault::DegenerateProfile);
    }
    let n = cross3(&edge, &apex.relative_to(p0));
    if is_zero3(&n) {
        return Err(ExtrudeFault::DegenerateWall);
    }
    let d = dot3(&n, p0);
    Ok(CutSurface::Plane { n, d })
}

/// The wall swept by the generatrices through the ellipse `Q + cos t·e1 + sin t·e2`: an **elliptic
/// cone** for a finite apex, an **elliptic cylinder** for a direction.
///
/// `e1` and `e2` are conjugate semi-axes — any two independent in-plane vectors, not necessarily
/// orthogonal and not necessarily of equal length. That generality is exactly what an *affine*
/// frame needs: a circle of radius `r` in frame coordinates `(u, v)` is the ellipse
/// `e1 = r·u, e2 = r·v`, which is a metric circle only when the frame happens to be orthonormal.
///
/// The surface is `{ (ℓ₁·v)² + (ℓ₂·v)² = T(v)² }` for `v = X − Q`, where `ℓ₁, ℓ₂` read off the
/// projection's frame coordinates and `T` is its homogeneous weight — so it is exactly the locus of
/// points whose central projection from the apex onto the profile plane lands **on** the ellipse.
/// The interior of the cone is `F < 0`, matching [`CutSurface`]'s sign convention.
///
/// The returned [`Nappe`] selects the single nappe on the authored side (§4.1): a finite apex
/// generates a *double* cone, and without the selector the cut would reappear mirrored beyond the
/// apex. For `w = 0` there is no nappe to choose and the selector is vacuously true — one formula,
/// no branch.
///
/// ```
/// use develop::extrude::{Apex, ellipse_wall};
/// use lattice::{Bignum, Rat};
///
/// type Q = Rat<Bignum>;
/// let q = |n: i128| Q::from_i128(n);
/// // The unit circle in the z = 0 plane, cast from the apex (0,0,4): a right circular cone.
/// let wall = ellipse_wall(
///     &[q(0), q(0), q(0)],
///     &[q(1), q(0), q(0)],
///     &[q(0), q(1), q(0)],
///     &Apex::point([q(0), q(0), q(4)]),
/// )
/// .expect("a real cone");
/// let f = |p: [Q; 3]| wall.residual(&p).expect("a well-formed wall has a residual");
/// // The generating circle lies on it, exactly.
/// assert!(f([q(1), q(0), q(0)]).is_zero());
/// assert!(f([Q::new(3, 5), Q::new(4, 5), q(0)]).is_zero());
/// // Halfway to the apex the cone has halved: (1/2, 0, 2) is on it, (1, 0, 2) is outside.
/// assert!(f([Q::new(1, 2), q(0), q(2)]).is_zero());
/// assert!(f([q(1), q(0), q(2)]).sign() > 0);
/// ```
pub fn ellipse_wall<B: Backend>(
    center: &V3<B>,
    e1: &V3<B>,
    e2: &V3<B>,
    apex: &Apex<B>,
) -> Result<CutSurface<B>, ExtrudeFault> {
    check_apex(apex)?;
    // The profile plane. `n = e1 × e2` is nonzero exactly when the two semi-axes are independent,
    // which is also what makes the Gram determinant positive below.
    let n = cross3(e1, e2);
    if is_zero3(&n) {
        return Err(ExtrudeFault::DegenerateProfile);
    }
    // `k = n·(a − w·Q)` measures the apex against the profile plane: zero means the apex lies in it.
    let ap = apex.relative_to(center);
    let k = dot3(&n, &ap);
    if k.is_zero() {
        return Err(ExtrudeFault::ApexInPlane);
    }

    // The reciprocal semi-axes `r1, r2` read a point's ellipse coordinates: `r_i·(P − Q) = (cos t,
    // sin t)_i` on the ellipse. Rational, via the 2×2 Gram inverse.
    let (g11, g12, g22) = (dot3(e1, e1), dot3(e1, e2), dot3(e2, e2));
    let inv_det = g11.mul(&g22).sub(&g12.mul(&g12)).recip();
    let r1 = scale3(&sub3(&scale3(e1, &g22), &scale3(e2, &g12)), &inv_det);
    let r2 = scale3(&sub3(&scale3(e2, &g11), &scale3(e1, &g12)), &inv_det);

    // Project `X` from the apex onto the profile plane, in homogeneous form: the projected point is
    // `(N·v : T(v))` relative to `Q`, with `N = k·I − a'·nᵀ` and `T(v) = k − w·(n·v)`. So its
    // ellipse coordinates are `ℓ_i·v / T(v)`, and `X` is on the wall iff they sit on the unit
    // circle: `(ℓ₁·v)² + (ℓ₂·v)² = T(v)²`.
    let el = |r: &V3<B>| sub3(&scale3(r, &k), &scale3(&n, &dot3(&ap, r)));
    let (l1, l2) = (el(&r1), el(&r2));
    let tau = scale3(&n, &apex.w.neg()); // T's linear part, in absolute X
    let t = k.add(&apex.w.mul(&dot3(&n, center))); // T's constant, in absolute X

    // Expand `F = (ℓ₁·X + c₁)² + (ℓ₂·X + c₂)² − (τ·X + t)²` into `XᵀMX + b·X + c`.
    let (c1, c2) = (dot3(&l1, center).neg(), dot3(&l2, center).neg());
    let m = sub3x3(
        &add3x3(&outer3(&l1, &l1), &outer3(&l2, &l2)),
        &outer3(&tau, &tau),
    );
    let two = Rat::from_i128(2);
    let b = scale3(
        &sub3(
            &add3(&scale3(&l1, &c1), &scale3(&l2, &c2)),
            &scale3(&tau, &t),
        ),
        &two,
    );
    let c = c1.mul(&c1).add(&c2.mul(&c2)).sub(&t.mul(&t));

    // The authored nappe is the one the profile itself sits on — where `T` has the sign it has on
    // the ellipse, namely `sign(k)`. So the selector is `k·T(X) > 0`. At `w = 0` this collapses to
    // `k² > 0`: a cylinder has no wrong nappe, and the selector says so rather than branching.
    let nappe = Nappe {
        n: scale3(&tau, &k),
        d: k.mul(&t).neg(),
    };
    Ok(CutSurface::Quadric(Box::new(crate::cut::Quadric {
        m,
        b,
        c,
        nappe,
    })))
}

/// `[0 : 0]` is not a projective point.
fn check_apex<B: Backend>(apex: &Apex<B>) -> Result<(), ExtrudeFault> {
    if is_zero3(&apex.a) && apex.w.is_zero() {
        return Err(ExtrudeFault::DegenerateApex);
    }
    Ok(())
}

// --- exact rational vector helpers ------------------------------------------------------------

/// Whether every component is exactly zero.
fn is_zero3<B: Backend>(a: &V3<B>) -> bool {
    a.iter().all(|x| x.is_zero())
}

/// `a · b`.
fn dot3<B: Backend>(a: &V3<B>, b: &V3<B>) -> Rat<B> {
    a[0].mul(&b[0]).add(&a[1].mul(&b[1])).add(&a[2].mul(&b[2]))
}

/// `a × b`.
fn cross3<B: Backend>(a: &V3<B>, b: &V3<B>) -> V3<B> {
    [
        a[1].mul(&b[2]).sub(&a[2].mul(&b[1])),
        a[2].mul(&b[0]).sub(&a[0].mul(&b[2])),
        a[0].mul(&b[1]).sub(&a[1].mul(&b[0])),
    ]
}

/// `a + b`.
fn add3<B: Backend>(a: &V3<B>, b: &V3<B>) -> V3<B> {
    [a[0].add(&b[0]), a[1].add(&b[1]), a[2].add(&b[2])]
}

/// `a − b`.
fn sub3<B: Backend>(a: &V3<B>, b: &V3<B>) -> V3<B> {
    [a[0].sub(&b[0]), a[1].sub(&b[1]), a[2].sub(&b[2])]
}

/// `k·a`.
fn scale3<B: Backend>(a: &V3<B>, k: &Rat<B>) -> V3<B> {
    [a[0].mul(k), a[1].mul(k), a[2].mul(k)]
}

/// The outer product `a bᵀ`.
fn outer3<B: Backend>(a: &V3<B>, b: &V3<B>) -> [[Rat<B>; 3]; 3] {
    core::array::from_fn(|i| core::array::from_fn(|j| a[i].mul(&b[j])))
}

/// `A + B`.
fn add3x3<B: Backend>(a: &[[Rat<B>; 3]; 3], b: &[[Rat<B>; 3]; 3]) -> [[Rat<B>; 3]; 3] {
    core::array::from_fn(|i| core::array::from_fn(|j| a[i][j].add(&b[i][j])))
}

/// `A − B`.
fn sub3x3<B: Backend>(a: &[[Rat<B>; 3]; 3], b: &[[Rat<B>; 3]; 3]) -> [[Rat<B>; 3]; 3] {
    core::array::from_fn(|i| core::array::from_fn(|j| a[i][j].sub(&b[i][j])))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice::Bignum;

    type Q = Rat<Bignum>;

    fn q(n: i128) -> Q {
        Q::from_i128(n)
    }

    /// The origin.
    fn o3() -> [Q; 3] {
        [q(0), q(0), q(0)]
    }

    /// The fault a refused wall carries. (`CutSurface` is deliberately not `Debug`/`PartialEq` — it
    /// is geometry, not a value to compare — so the `Ok` side is dropped rather than unwrapped.)
    fn fault(r: Result<CutSurface<Bignum>, ExtrudeFault>) -> Option<ExtrudeFault> {
        r.err()
    }

    /// The residual of a wall built here — always defined, since these are all real surfaces.
    fn res(wall: &CutSurface<Bignum>, p: &[Q; 3]) -> Q {
        wall.residual(p).expect("a well-formed wall has a residual")
    }

    /// Rational points on the unit circle — exact `(cos t, sin t)` samples, so "the profile lies on
    /// its own wall" is testable with `== 0`, not with a tolerance.
    const CIRCLE: [(i128, i128, i128); 6] = [
        (1, 0, 1),
        (0, 1, 1),
        (3, 4, 5),
        (-4, 3, 5),
        (-5, -12, 13),
        (8, -15, 17),
    ];

    fn apex_pt(x: i128, y: i128, z: i128) -> Apex<Bignum> {
        Apex::point([q(x), q(y), q(z)])
    }

    fn apex_dir(x: i128, y: i128, z: i128) -> Apex<Bignum> {
        Apex::direction([q(x), q(y), q(z)]).expect("nonzero direction")
    }

    #[test]
    fn apex_is_one_projective_point() {
        let p = apex_pt(1, 2, 3);
        assert!(!p.is_direction());
        assert_eq!(p.finite().unwrap()[2], q(3));
        // `w == 0` is exact: a cast point pushed a billion units out is still finite.
        let far = apex_pt(0, 0, 1_000_000_000);
        assert!(!far.is_direction());

        let d = apex_dir(0, 0, 1);
        assert!(d.is_direction());
        assert!(d.finite().is_none());

        assert!(Apex::<Bignum>::direction(o3()).is_none());
        assert!(Apex::<Bignum>::homogeneous(o3(), q(0)).is_none());
        // A scaled homogeneous point is the same cast point.
        let h = Apex::<Bignum>::homogeneous([q(2), q(4), q(6)], q(2)).unwrap();
        assert_eq!(h.finite().unwrap(), apex_pt(1, 2, 3).finite().unwrap());
    }

    /// Every wall contains the edge that generates it — exactly, for both apex kinds.
    #[test]
    fn walls_contain_their_profile_edge() {
        let centre = [q(1), q(-2), q(3)];
        let (e1, e2) = ([q(2), q(0), q(0)], [q(0), q(3), q(0)]); // a genuine ellipse, not a circle
        for apex in [apex_pt(5, 7, 11), apex_dir(1, 2, 5), apex_pt(0, 0, 40)] {
            let wall = ellipse_wall(&centre, &e1, &e2, &apex).expect("a real wall");
            for (cn, sn, den) in CIRCLE {
                let (c, s) = (Q::new(cn, den), Q::new(sn, den));
                let p = add3(&centre, &add3(&scale3(&e1, &c), &scale3(&e2, &s)));
                assert!(res(&wall, &p).is_zero(), "ellipse point off its own wall");
            }
            // The apex itself is the cone's vertex (and lies at infinity for a direction).
            if let Some(a) = apex.finite() {
                assert!(res(&wall, &a).is_zero(), "apex off its own cone");
            }
        }
    }

    /// A segment wall contains the segment and the apex, and reversing the edge flips the normal.
    #[test]
    fn segment_wall_contains_edge_and_apex() {
        let (p0, p1) = ([q(1), q(0), q(0)], [q(0), q(2), q(1)]);
        for apex in [apex_pt(3, 3, 9), apex_dir(0, 0, 1)] {
            let wall = segment_wall(&p0, &p1, &apex).expect("a real wall");
            assert!(res(&wall, &p0).is_zero());
            assert!(res(&wall, &p1).is_zero());
            if let Some(a) = apex.finite() {
                assert!(res(&wall, &a).is_zero());
            }
            let back = segment_wall(&p1, &p0, &apex).expect("a real wall");
            // Same plane, opposite orientation: the residuals are exact negatives.
            let probe = [q(7), q(-3), q(2)];
            assert!(res(&wall, &probe).add(&res(&back, &probe)).is_zero());
        }
    }

    /// The interior of the cone is `F < 0` — [`CutSurface`]'s convention — and the mirror nappe is
    /// excluded by the selector rather than by the residual.
    #[test]
    fn cone_interior_is_negative_and_the_mirror_nappe_is_selected_out() {
        let apex = apex_pt(0, 0, 4);
        let wall = ellipse_wall(&o3(), &[q(1), q(0), q(0)], &[q(0), q(1), q(0)], &apex)
            .expect("a real cone");
        // On the axis between the plane and the apex: strictly inside.
        assert!(res(&wall, &[q(0), q(0), q(1)]).sign() < 0);
        // Far off-axis in the same plane: strictly outside.
        assert!(res(&wall, &[q(3), q(0), q(1)]).sign() > 0);
        // The mirror nappe: the residual vanishes (it is the same double cone) but the nappe
        // selector rejects it. z = 8 is one unit beyond the apex, where the mirror radius is 1.
        let mirror = [q(1), q(0), q(8)];
        assert!(res(&wall, &mirror).is_zero());
        assert!(!wall.on_nappe(&mirror));
        assert!(wall.on_nappe(&[q(1), q(0), q(0)]));
    }

    /// A cylinder has no wrong nappe, and the vacuous selector says so for every point.
    #[test]
    fn a_cylinder_has_no_nappe_to_get_wrong() {
        let wall = ellipse_wall(
            &o3(),
            &[q(1), q(0), q(0)],
            &[q(0), q(1), q(0)],
            &apex_dir(0, 0, 1),
        )
        .expect("a real cylinder");
        for z in [-1000, 0, 1000] {
            assert!(wall.on_nappe(&[q(1), q(0), q(z)]));
            assert!(res(&wall, &[q(1), q(0), q(z)]).is_zero());
        }
    }

    #[test]
    fn degenerate_authoring_is_refused() {
        let e1 = [q(1), q(0), q(0)];
        let e2 = [q(0), q(1), q(0)];
        let o = o3();
        // The apex sits in the profile plane: no cone.
        assert_eq!(
            fault(ellipse_wall(&o, &e1, &e2, &apex_pt(9, 0, 0)),),
            Some(ExtrudeFault::ApexInPlane)
        );
        // The extrusion runs parallel to the profile plane.
        assert_eq!(
            fault(ellipse_wall(&o, &e1, &e2, &apex_dir(1, 1, 0)),),
            Some(ExtrudeFault::ApexInPlane)
        );
        // Parallel semi-axes span no plane.
        assert_eq!(
            fault(ellipse_wall(
                &o,
                &e1,
                &[q(2), q(0), q(0)],
                &apex_dir(0, 0, 1)
            ),),
            Some(ExtrudeFault::DegenerateProfile)
        );
        // A segment of zero length, and one whose line runs through the apex.
        assert_eq!(
            fault(segment_wall(&o, &o, &apex_dir(0, 0, 1)),),
            Some(ExtrudeFault::DegenerateProfile)
        );
        assert_eq!(
            fault(segment_wall(&o, &[q(0), q(0), q(1)], &apex_dir(0, 0, 1)),),
            Some(ExtrudeFault::DegenerateWall)
        );
        assert_eq!(
            fault(segment_wall(&o, &[q(1), q(0), q(0)], &apex_pt(2, 0, 0)),),
            Some(ExtrudeFault::DegenerateWall)
        );
    }

    /// Pushing the cast point outward degrades the cone to the cylinder continuously — the API has
    /// no discontinuity at "parallel", because parallel is just `w = 0`.
    #[test]
    fn a_distant_cast_point_approaches_the_parallel_extrusion() {
        let (o, e1, e2) = (o3(), [q(1), q(0), q(0)], [q(0), q(1), q(0)]);
        // On the unit cylinder at height 1 — and *outside* every cone cast from above it, since a
        // cone tapers toward its apex. The further out the cast point, the less it tapers.
        let probe = [q(1), q(0), q(1)];
        let cyl = ellipse_wall(&o, &e1, &e2, &apex_dir(0, 0, 1)).expect("cylinder");
        assert!(res(&cyl, &probe).is_zero());
        let mut last: Option<Q> = None;
        for z in [10i128, 100, 1_000, 10_000] {
            // Normalize by the squared weight `k² = z²` so the residuals are comparable across
            // apexes — the surface is a zero set, so any positive rescaling is the same surface.
            let cone = ellipse_wall(&o, &e1, &e2, &apex_pt(0, 0, z)).expect("cone");
            let r = res(&cone, &probe).div(&q(z).mul(&q(z)));
            assert!(r.sign() > 0, "the probe is outside a cone tapering upward");
            if let Some(prev) = last {
                assert!(r.sub(&prev).sign() < 0, "the residual must fall toward 0");
            }
            last = Some(r);
        }
        // At a cast point 10⁴ away the cone and the cylinder agree here to ~2·10⁻⁴.
        assert!(last.unwrap().cmp(&Q::new(1, 1000)) == core::cmp::Ordering::Less);
    }
}
