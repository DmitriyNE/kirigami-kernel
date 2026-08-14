//! `place` — **exact rational rigid placements** (the extrinsic coordinate change).
//!
//! A [`Placement`] is a rigid motion `g·X = R·X + t` held exactly over ℚ: a nonzero rational
//! quaternion `q` (the rotation `R = rot(q)/|q|²` is a rational orthogonal matrix — rational
//! unit quaternions are dense in SO(3), so any pose is approximable arbitrarily well *within*
//! the exact representation) and a rational translation `t`. It is the one placement type shared
//! by cutter frames, assembly positioning, and reference bodies.
//!
//! **The `(q, h)` chart representation is equivariant.** Moving a chart by `g` is closed in the
//! representation and never forces a reparametrization:
//!
//! > `g · (q(σ), h(σ)) = (q_g ⊗ q(σ), h(σ) + ⟨n′ᵍ(σ), t⟩)`
//!
//! — a quaternion product plus a rational support shift ([`apply_chart`](Placement::apply_chart)),
//! both exact, *same σ*. The ruling coordinate µ̂ transports by the rational shift
//! [`mu_shift`](Placement::mu_shift) `δ(σ) = ⟨t, rᵍ(σ)⟩/|n′|²`: a surface point maps as
//! `g·X(σ, µ̂, w) = Xᵍ(σ, µ̂ + δ(σ), w)` — chart-indexed data (rails, bands) moves mechanically,
//! which is the covariant-core doctrine in action.
//!
//! Approximate poses enter through [`snap`](Placement::snap) — the float is *interpreted* and
//! snapped to a nearby exact rational placement, which is then the recorded truth (the same
//! doctrine as the azimuth snap: user intent is approximate, recipes are exact).
//!
//! ```
//! use develop::place::Placement;
//! use fixtures::devices::cone;
//! use lattice::{Bignum, Rat};
//!
//! type Q = Rat<Bignum>;
//! // A quarter turn about z (q = 1 + k) plus a unit x-shift — all exact.
//! let g = Placement::<Bignum>::new(
//!     [Q::from_i128(1), Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)],
//!     [Q::from_i128(1), Q::from_i128(0), Q::from_i128(0)],
//! )
//! .unwrap();
//! let chart = cone();
//! let moved = g.apply_chart(&chart);
//! let delta = g.mu_shift(&chart);
//!
//! // Equivariance: g·X(σ, µ̂, w) = Xᵍ(σ, µ̂ + δ(σ), w), exactly.
//! let (s, mu, w) = (Q::new(1, 3), Q::from_i128(2), Q::new(1, 8));
//! let x = chart.surface(&mu, &w).eval(&s).unwrap();
//! let mu_g = mu.add(&delta.eval(&s).unwrap());
//! assert_eq!(g.apply_point(&x), moved.surface(&mu_g, &w).eval(&s).unwrap());
//! ```

use geom::chart::Chart;
use lattice::{Backend, Bignum, Poly, Rat, RatFunc, Vec3Rat};

/// An exact rigid placement `X ↦ rot(q)/|q|²·X + t`: a nonzero rational quaternion (scalar part
/// first) and a rational translation. Build with [`new`](Placement::new) (exact data) or
/// [`snap`](Placement::snap) (an approximate pose, snapped to a nearby exact placement).
#[derive(Clone)]
pub struct Placement<B: Backend = Bignum> {
    /// The rotation quaternion `[q₀, q₁, q₂, q₃]` (scalar part first; any nonzero scale — the
    /// rotation `rot(q)/|q|²` is scale-invariant).
    q: [Rat<B>; 4],
    /// The translation `t`.
    t: [Rat<B>; 3],
}

/// `|q|²` of a rational quaternion.
fn quat_norm2<B: Backend>(q: &[Rat<B>; 4]) -> Rat<B> {
    q[0].mul(&q[0])
        .add(&q[1].mul(&q[1]))
        .add(&q[2].mul(&q[2]))
        .add(&q[3].mul(&q[3]))
}

impl<B: Backend> Placement<B> {
    /// The placement with rotation quaternion `q` and translation `t`, or `None` if `q = 0`
    /// (which carries no rotation). `q` need not be unit — `rot(q)/|q|²` is scale-invariant, so
    /// integer quaternions parametrize rational rotations directly.
    pub fn new(q: [Rat<B>; 4], t: [Rat<B>; 3]) -> Option<Self> {
        if quat_norm2(&q).sign() <= 0 {
            return None;
        }
        Some(Placement { q, t })
    }

    /// The identity placement.
    pub fn identity() -> Self {
        let zero = || Rat::from_i128(0);
        Placement {
            q: [Rat::from_i128(1), zero(), zero(), zero()],
            t: [zero(), zero(), zero()],
        }
    }

    /// The rotation quaternion `[q₀, q₁, q₂, q₃]` (exact recipe data — read it back).
    pub fn quaternion(&self) -> &[Rat<B>; 4] {
        &self.q
    }

    /// The translation `t` (exact recipe data — read it back).
    pub fn translation(&self) -> &[Rat<B>; 3] {
        &self.t
    }

    /// The rotated vector `rot(q)/|q|²·v` (no translation) — exact.
    pub fn rotate_vec(&self, v: &[Rat<B>; 3]) -> [Rat<B>; 3] {
        rotate(&self.q, v)
    }

    /// The placed point `rot(q)/|q|²·p + t` — exact.
    pub fn apply_point(&self, p: &[Rat<B>; 3]) -> [Rat<B>; 3] {
        let r = rotate(&self.q, p);
        [
            r[0].add(&self.t[0]),
            r[1].add(&self.t[1]),
            r[2].add(&self.t[2]),
        ]
    }

    /// The composition `self ∘ other` (apply `other` first): quaternions multiply, translations
    /// chain (`t = R_self·t_other + t_self`).
    pub fn compose(&self, other: &Self) -> Self {
        let q = quat_mul_rat(&self.q, &other.q);
        let rt = rotate(&self.q, &other.t);
        Placement {
            q,
            t: [
                rt[0].add(&self.t[0]),
                rt[1].add(&self.t[1]),
                rt[2].add(&self.t[2]),
            ],
        }
    }

    /// The inverse placement (`R⁻¹ = rot(q̄)/|q|²`, `t⁻¹ = −R⁻¹·t`).
    pub fn inverse(&self) -> Self {
        let conj = [
            self.q[0].clone(),
            self.q[1].neg(),
            self.q[2].neg(),
            self.q[3].neg(),
        ];
        let rt = rotate(&conj, &self.t);
        Placement {
            q: conj,
            t: [rt[0].neg(), rt[1].neg(), rt[2].neg()],
        }
    }

    /// The placed chart `g·(q(σ), h(σ)) = (q_g ⊗ q(σ), h(σ) + ⟨n′ᵍ(σ), t⟩)` — the equivariant
    /// action on the `(q, h)` representation (see the module docs). The support shift is computed
    /// as `⟨n(σ), R⁻¹t⟩`, a rational function; the σ parameter is untouched.
    ///
    /// The new chart parametrizes exactly the moved surface, with the ruling coordinate shifted
    /// by [`mu_shift`](Placement::mu_shift): `g·X(σ, µ̂, w) = Xᵍ(σ, µ̂ + δ(σ), w)`.
    pub fn apply_chart(&self, chart: &Chart<B>) -> Chart<B> {
        let old_q = chart.quaternion();
        let q = quat_mul_poly(&self.q, old_q);
        // ⟨R·n, t⟩ = ⟨n, Rᵀ·t⟩ with Rᵀ = rot(q̄)/|q|².
        let conj = [
            self.q[0].clone(),
            self.q[1].neg(),
            self.q[2].neg(),
            self.q[3].neg(),
        ];
        let rt = rotate(&conj, &self.t);
        let shift = chart.normal().dot(&const_vec3(&rt));
        Chart::new(q, chart.support().add(&shift))
    }

    /// The ruling-coordinate transport `δ(σ) = ⟨t, rᵍ(σ)⟩/|n′|² = ⟨R⁻¹t, r(σ)⟩/|n′|²` of
    /// [`apply_chart`](Placement::apply_chart): a µ̂-rail `µ̂(σ)` on the original chart becomes
    /// the rail `µ̂(σ) + δ(σ)` on the placed chart — chart-indexed data transforms mechanically.
    pub fn mu_shift(&self, chart: &Chart<B>) -> RatFunc<B> {
        let conj = [
            self.q[0].clone(),
            self.q[1].neg(),
            self.q[2].neg(),
            self.q[3].neg(),
        ];
        let rt = rotate(&conj, &self.t);
        chart
            .ruling()
            .dot(&const_vec3(&rt))
            .div(chart.normal_deriv_sq())
    }

    /// Interpret an **approximate pose** — a rotation as `axis`/`angle_deg` plus a translation,
    /// all `f64` — and snap it to a nearby exact rational placement on the `2^bits` dyadic grid
    /// (the recorded truth; read the exact pose back through
    /// [`quaternion`](Placement::quaternion)/[`translation`](Placement::translation)). Floats are
    /// for *interpretation* only — nothing downstream reads them. `None` for a degenerate axis or
    /// non-finite input. `bits` is clamped to 60.
    pub fn snap(axis: [f64; 3], angle_deg: f64, t: [f64; 3], bits: u32) -> Option<Self> {
        let norm = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
        if !norm.is_finite() || norm < 1e-300 || !angle_deg.is_finite() {
            return None;
        }
        let half = angle_deg.to_radians() / 2.0;
        let (s, c) = (half.sin() / norm, half.cos());
        let q = [
            dyadic::<B>(c, bits)?,
            dyadic::<B>(s * axis[0], bits)?,
            dyadic::<B>(s * axis[1], bits)?,
            dyadic::<B>(s * axis[2], bits)?,
        ];
        let t = [
            dyadic::<B>(t[0], bits)?,
            dyadic::<B>(t[1], bits)?,
            dyadic::<B>(t[2], bits)?,
        ];
        Placement::new(q, t)
    }
}

/// A finite `f64` snapped to the `2^bits` dyadic grid (`bits` clamped to 60), or `None` if it is
/// non-finite or too large for the grid.
fn dyadic<B: Backend>(x: f64, bits: u32) -> Option<Rat<B>> {
    let bits = bits.min(60);
    let scale = (1i128 << bits) as f64;
    let scaled = x * scale;
    if !scaled.is_finite() || scaled.abs() >= i64::MAX as f64 {
        return None;
    }
    #[allow(clippy::cast_possible_truncation)]
    Some(Rat::new(scaled.round() as i128, 1i128 << bits))
}

/// The Hamilton product `a ⊗ b` of two rational quaternions (scalar part first).
fn quat_mul_rat<B: Backend>(a: &[Rat<B>; 4], b: &[Rat<B>; 4]) -> [Rat<B>; 4] {
    [
        a[0].mul(&b[0])
            .sub(&a[1].mul(&b[1]))
            .sub(&a[2].mul(&b[2]))
            .sub(&a[3].mul(&b[3])),
        a[0].mul(&b[1])
            .add(&a[1].mul(&b[0]))
            .add(&a[2].mul(&b[3]))
            .sub(&a[3].mul(&b[2])),
        a[0].mul(&b[2])
            .sub(&a[1].mul(&b[3]))
            .add(&a[2].mul(&b[0]))
            .add(&a[3].mul(&b[1])),
        a[0].mul(&b[3])
            .add(&a[1].mul(&b[2]))
            .sub(&a[2].mul(&b[1]))
            .add(&a[3].mul(&b[0])),
    ]
}

/// The Hamilton product `g ⊗ q(σ)` of a constant rational quaternion with a polynomial one.
fn quat_mul_poly<B: Backend>(g: &[Rat<B>; 4], q: &[Poly<B>; 4]) -> [Poly<B>; 4] {
    [
        q[0].scale(&g[0])
            .sub(&q[1].scale(&g[1]))
            .sub(&q[2].scale(&g[2]))
            .sub(&q[3].scale(&g[3])),
        q[1].scale(&g[0])
            .add(&q[0].scale(&g[1]))
            .add(&q[3].scale(&g[2]))
            .sub(&q[2].scale(&g[3])),
        q[2].scale(&g[0])
            .sub(&q[3].scale(&g[1]))
            .add(&q[0].scale(&g[2]))
            .add(&q[1].scale(&g[3])),
        q[3].scale(&g[0])
            .add(&q[2].scale(&g[1]))
            .sub(&q[1].scale(&g[2]))
            .add(&q[0].scale(&g[3])),
    ]
}

/// Rotate `v` by `rot(q)/|q|²` — via the quaternion sandwich `q·(0,v)·q̄`, divided by `|q|²`.
fn rotate<B: Backend>(q: &[Rat<B>; 4], v: &[Rat<B>; 3]) -> [Rat<B>; 3] {
    let pv = [Rat::from_i128(0), v[0].clone(), v[1].clone(), v[2].clone()];
    let conj = [q[0].clone(), q[1].neg(), q[2].neg(), q[3].neg()];
    let s = quat_mul_rat(&quat_mul_rat(q, &pv), &conj);
    let inv = quat_norm2(q).recip();
    [s[1].mul(&inv), s[2].mul(&inv), s[3].mul(&inv)]
}

/// A constant vector as a degree-0 [`Vec3Rat`] (denominator 1), so it dots with chart fields.
fn const_vec3<B: Backend>(v: &[Rat<B>; 3]) -> Vec3Rat<B> {
    Vec3Rat::new(
        [
            Poly::constant(v[0].clone()),
            Poly::constant(v[1].clone()),
            Poly::constant(v[2].clone()),
        ],
        Poly::constant(Rat::from_i128(1)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use fixtures::devices::{cone, cone_seam_ramp, cone_wrap};

    type Q = Rat<Bignum>;

    fn q(n: i128, d: i128) -> Q {
        Q::new(n, d)
    }
    fn qi(n: i128) -> Q {
        Q::from_i128(n)
    }

    /// A small corpus of placements: a quarter-turn about z + x-shift, a pure translation, and a
    /// generic non-unit quaternion with a rational translation.
    fn corpus() -> Vec<Placement<Bignum>> {
        vec![
            Placement::new([qi(1), qi(0), qi(0), qi(1)], [qi(1), qi(0), qi(0)]).unwrap(),
            Placement::new([qi(1), qi(0), qi(0), qi(0)], [q(1, 2), qi(-1), q(3, 4)]).unwrap(),
            Placement::new([qi(2), qi(1), qi(3), qi(-1)], [qi(1), qi(-2), q(1, 2)]).unwrap(),
        ]
    }

    #[test]
    fn a_zero_quaternion_is_refused() {
        assert!(
            Placement::<Bignum>::new([qi(0), qi(0), qi(0), qi(0)], [qi(0), qi(0), qi(0)]).is_none()
        );
    }

    #[test]
    fn rotation_is_exactly_orthogonal() {
        // |R·v|² = |v|² for the generic non-unit quaternion — rot(q)/|q|² is orthogonal over ℚ.
        let g = &corpus()[2];
        for v in [[qi(1), qi(0), qi(0)], [q(1, 3), qi(-2), q(5, 7)]] {
            let r = g.rotate_vec(&v);
            let n2 = |w: &[Q; 3]| w[0].mul(&w[0]).add(&w[1].mul(&w[1])).add(&w[2].mul(&w[2]));
            assert_eq!(n2(&r), n2(&v));
        }
    }

    #[test]
    fn compose_with_inverse_is_the_identity_action() {
        for g in corpus() {
            let gi = g.inverse();
            let p = [q(3, 5), qi(-1), q(7, 2)];
            // Round-trip a point (the quaternion itself is only unique up to scale, so compare
            // the *action*, not the representation).
            assert_eq!(gi.apply_point(&g.apply_point(&p)), p);
            let both = g.compose(&gi);
            assert_eq!(both.apply_point(&p), p);
        }
    }

    #[test]
    fn the_chart_action_is_equivariant_with_the_mu_shift() {
        // g·X(σ, µ̂, w) = Xᵍ(σ, µ̂ + δ(σ), w) exactly, on cones and the γ≠0 ramp alike.
        for chart in [cone(), cone_wrap(), cone_seam_ramp()] {
            for g in corpus() {
                let moved = g.apply_chart(&chart);
                let delta = g.mu_shift(&chart);
                for (s, mu, w) in [
                    (q(1, 3), qi(2), qi(0)),
                    (q(-2, 5), q(-3, 2), q(1, 8)),
                    (qi(1), q(7, 4), q(-1, 16)),
                ] {
                    let x = chart.surface(&mu, &w).eval(&s).unwrap();
                    let mu_g = mu.add(&delta.eval(&s).unwrap());
                    assert_eq!(
                        g.apply_point(&x),
                        moved.surface(&mu_g, &w).eval(&s).unwrap(),
                        "equivariance failed"
                    );
                }
            }
        }
    }

    #[test]
    fn the_placed_chart_keeps_the_development_frame() {
        // A rigid motion cannot change intrinsic geometry: the placed cone is still a canonical
        // arctan cone with the same angle coefficient (ρ, ψ are rotation-invariant; the support
        // shift only moves the pedal).
        use crate::cone::{ConeDevelopment, cone_angle_coeff};
        let chart = cone();
        let g = &corpus()[0]; // rotation + translation
        let moved = g.apply_chart(&chart);
        // The rotated-only part keeps h ≡ 0 ⇒ still a canonical cone; with translation the
        // support shifts, so compare the angle coefficient via the developable constructor.
        let dev = ConeDevelopment::new_developable(&moved, 4).expect("placed cone develops");
        assert_eq!(
            *dev.angle_coeff(),
            cone_angle_coeff(&chart).expect("device cone coefficient")
        );
    }

    #[test]
    fn snap_interprets_an_approximate_pose_exactly() {
        // A ~90° turn about z: the snapped placement is exact data near the float intent.
        let g = Placement::<Bignum>::snap([0.0, 0.0, 1.0], 90.0, [0.25, 0.0, 0.0], 40).unwrap();
        // cos(45°) = sin(45°) ⇒ q₀ ≈ q₃, q₁ = q₂ = 0 exactly (0.0 snaps to 0).
        assert_eq!(g.quaternion()[1], qi(0));
        assert_eq!(g.quaternion()[2], qi(0));
        let d = g.quaternion()[0].sub(&g.quaternion()[3]);
        let tiny = q(1, 1 << 30);
        assert!(d.mul(&d).cmp(&tiny) == core::cmp::Ordering::Less, "q₀ ≈ q₃");
        // The translation snapped exactly (1/4 is dyadic).
        assert_eq!(g.translation()[0], q(1, 4));
        // A degenerate axis is refused.
        assert!(Placement::<Bignum>::snap([0.0, 0.0, 0.0], 90.0, [0.0; 3], 40).is_none());
    }
}
