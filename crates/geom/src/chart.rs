//! The exact chart field layer (spec §3.2): from a quaternion spline `q(σ)` and a
//! support spline `h(σ)`, the strip's normal, ruling, pedal, and thickened surface —
//! all exact rational functions of the parameter σ (no floating point).
//!
//! A [`Chart`] is built from `q` (four [`Poly`]s — a polynomial quaternion) and `h` (a
//! [`RatFunc`]). Its geometry is derived once and read back through accessors:
//!
//! - [`normal`](Chart::normal) `n = q·e₃·q̄ / |q|²` — the unit surface normal
//!   (Euler–Rodrigues); `n·n = 1` holds exactly for any nonzero `q`.
//! - [`ruling`](Chart::ruling) `r = n × n′` — the ruling direction.
//! - [`pedal`](Chart::pedal) `c = h·n + (h′/|n′|²)·n′` — the pedal point (`c·r = 0`).
//! - [`surface`](Chart::surface) `C(σ,μ,w) = c + μ·r + w·n` — the thickened map,
//!   affine in the ruling parameter `μ` and thickness `w`.
//! - [`det_j`](Chart::det_j) `det J = (c′+μr′)·n′ + w|n′|²` and
//!   [`psi_prime`](Chart::psi_prime) `ψ′ = det(n,n′,n″)/|n′|²`.
//!
//! The normal auto-normalizes (`|n|² ≡ 1`), so every field stays a ratio of polynomials
//! over ℚ — no `√` appears at this layer.
//!
//! # Example
//!
//! ```
//! use geom::chart::Chart;
//! use lattice::{Bignum, Poly, Rat, RatFunc};
//!
//! let poly = |cs: &[i128]| Poly::<Bignum>::from_coeffs(cs.iter().map(|&c| Rat::from_i128(c)).collect());
//! // q(σ) = 1 + σ·i — a rotation about the x-axis; rulings through the origin (h ≡ 0).
//! let q = [poly(&[1]), poly(&[0, 1]), poly(&[0]), poly(&[0])];
//! let chart = Chart::new(q, RatFunc::zero());
//!
//! // The normal is a genuine unit vector, exactly.
//! assert_eq!(chart.normal().dot(chart.normal()), RatFunc::one());
//! // At σ = 1 it is (0, −1, 0): [0, −2σ, 1−σ²]/(1+σ²) evaluated at σ = 1.
//! assert_eq!(
//!     chart.normal().eval(&Rat::from_i128(1)),
//!     Some([Rat::from_i128(0), Rat::from_i128(-1), Rat::from_i128(0)]),
//! );
//! ```

use lattice::{Backend, Bignum, Poly, Rat, RatFunc, Vec3Rat};

/// A strip chart: the exact σ-parametric geometry derived from a quaternion spline `q`
/// and a support spline `h` (spec §3.2). Build with [`Chart::new`]; read the fields back
/// through the accessors.
pub struct Chart<B: Backend = Bignum> {
    q: [Poly<B>; 4],
    h: RatFunc<B>,
    n: Vec3Rat<B>,
    n1: Vec3Rat<B>,
    r: Vec3Rat<B>,
    n1_sq: RatFunc<B>,
    c: Vec3Rat<B>,
}

/// The Jacobian determinant `det J = (c′+μr′)·n′ + w|n′|²`, held as its affine
/// decomposition in the ruling parameter `μ` and thickness `w`, with σ-rational
/// coefficients. Evaluate a concrete `(σ, μ, w)` with [`DetJ::eval`].
pub struct DetJ<B: Backend = Bignum> {
    /// The `μ⁰w⁰` term `c′·n′`.
    pub constant: RatFunc<B>,
    /// The coefficient of `μ`: `r′·n′`.
    pub mu: RatFunc<B>,
    /// The coefficient of `w`: `|n′|²` (structurally positive).
    pub w: RatFunc<B>,
}

impl<B: Backend> DetJ<B> {
    /// `det J` at `(σ, μ, w)`, or `None` where a coefficient's denominator vanishes.
    pub fn eval(&self, sigma: &Rat<B>, mu: &Rat<B>, w: &Rat<B>) -> Option<Rat<B>> {
        let c = self.constant.eval(sigma)?;
        let m = self.mu.eval(sigma)?;
        let ww = self.w.eval(sigma)?;
        Some(c.add(&m.mul(mu)).add(&ww.mul(w)))
    }
}

impl<B: Backend> Chart<B> {
    /// Build a chart from the quaternion spline `q = [q₀, q₁, q₂, q₃]` (scalar part
    /// first) and support spline `h`. `q` must not be identically zero, and its normal
    /// must have a non-vanishing derivative (`|n′|² ≢ 0`, i.e. the strip actually rules);
    /// both are debug-asserted.
    pub fn new(q: [Poly<B>; 4], h: RatFunc<B>) -> Self {
        let two = Rat::from_i128(2);
        let (q0, q1, q2, q3) = (&q[0], &q[1], &q[2], &q[3]);

        // n = q·e₃·q̄ / |q|². Numerator = third column of the rotation matrix times |q|²:
        //   [ 2(q₁q₃ + q₀q₂), 2(q₂q₃ − q₀q₁), q₀² − q₁² − q₂² + q₃² ].
        let nx = q1.mul(q3).add(&q0.mul(q2)).scale(&two);
        let ny = q2.mul(q3).sub(&q0.mul(q1)).scale(&two);
        let nz = q0
            .mul(q0)
            .sub(&q1.mul(q1))
            .sub(&q2.mul(q2))
            .add(&q3.mul(q3));
        let q_sq = q0
            .mul(q0)
            .add(&q1.mul(q1))
            .add(&q2.mul(q2))
            .add(&q3.mul(q3));
        debug_assert!(!q_sq.is_zero(), "Chart::new: |q|² is identically zero");
        let n = Vec3Rat::new([nx, ny, nz], q_sq);

        let n1 = n.derivative();
        let r = n.cross(&n1);
        let n1_sq = n1.dot(&n1);
        debug_assert!(
            !n1_sq.is_zero(),
            "Chart::new: |n′|² is identically zero (no ruling)"
        );

        // Pedal c = h·n + (h′/|n′|²)·n′.
        let factor = h.derivative().div(&n1_sq);
        let c = n.scale(&h).add(&n1.scale(&factor));

        Chart {
            q,
            h,
            n,
            n1,
            r,
            n1_sq,
            c,
        }
    }

    /// The quaternion spline `[q₀, q₁, q₂, q₃]`.
    pub fn quaternion(&self) -> &[Poly<B>; 4] {
        &self.q
    }
    /// The support spline `h`.
    pub fn support(&self) -> &RatFunc<B> {
        &self.h
    }
    /// The unit surface normal `n` (satisfies `n·n = 1`).
    pub fn normal(&self) -> &Vec3Rat<B> {
        &self.n
    }
    /// The normal derivative `n′`.
    pub fn normal_deriv(&self) -> &Vec3Rat<B> {
        &self.n1
    }
    /// The ruling direction `r = n × n′`.
    pub fn ruling(&self) -> &Vec3Rat<B> {
        &self.r
    }
    /// `|n′|²` — the ruling speed squared (structurally positive on a ruling chart).
    pub fn normal_deriv_sq(&self) -> &RatFunc<B> {
        &self.n1_sq
    }
    /// The pedal point `c = h·n + (h′/|n′|²)·n′` (satisfies `c·r = 0`).
    pub fn pedal(&self) -> &Vec3Rat<B> {
        &self.c
    }

    /// The thickened surface `C(σ,μ,w) = c + μ·r + w·n` at the given rational ruling
    /// parameter `μ` and thickness `w`, as a σ-parametric vector.
    pub fn surface(&self, mu: &Rat<B>, w: &Rat<B>) -> Vec3Rat<B> {
        self.c.add(&self.r.scale_rat(mu)).add(&self.n.scale_rat(w))
    }

    /// The Jacobian determinant `det J = (c′+μr′)·n′ + w|n′|²`, as its affine-in-`(μ,w)`
    /// decomposition (spec §3.2). `det J / |n′|² = R₁ + w = 1/κ₁`.
    pub fn det_j(&self) -> DetJ<B> {
        DetJ {
            constant: self.c.derivative().dot(&self.n1),
            mu: self.r.derivative().dot(&self.n1),
            w: self.n1_sq.clone(),
        }
    }

    /// The deflation invariant `ψ′ = det(n, n′, n″) / |n′|²` (spec §3.2).
    pub fn psi_prime(&self) -> RatFunc<B> {
        let n2 = self.n1.derivative();
        self.n.dot(&self.n1.cross(&n2)).div(&self.n1_sq)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Q = Rat<Bignum>;
    fn poly(cs: &[i128]) -> Poly<Bignum> {
        Poly::from_coeffs(cs.iter().map(|&c| Q::from_i128(c)).collect())
    }
    /// `q = [q0, q1, q2, q3]` from coefficient lists; `h` from a coefficient list.
    fn chart(q: [&[i128]; 4], h: &[i128]) -> Chart<Bignum> {
        Chart::new(
            [poly(q[0]), poly(q[1]), poly(q[2]), poly(q[3])],
            RatFunc::from_poly(poly(h)),
        )
    }

    /// A spread of non-degenerate charts: x-, y-rotations and two generic quaternions.
    fn corpus() -> Vec<Chart<Bignum>> {
        vec![
            chart([&[1], &[0, 1], &[0], &[0]], &[0]), // 1 + σi, h = 0
            chart([&[1], &[0], &[0, 1], &[0]], &[0]), // 1 + σj, h = 0
            chart([&[1], &[0, 1], &[1], &[0]], &[1]), // 1 + σi + j, h = 1
            chart([&[2], &[0, 1], &[1], &[0, 0, 1]], &[0, 1]), // generic, h = σ
        ]
    }

    #[test]
    fn normal_is_unit_exactly() {
        for c in corpus() {
            assert_eq!(c.normal().dot(c.normal()), RatFunc::one(), "n·n = 1");
        }
    }

    #[test]
    fn normal_perp_its_derivative() {
        for c in corpus() {
            assert!(c.normal().dot(c.normal_deriv()).is_zero(), "n·n′ = 0");
        }
    }

    #[test]
    fn ruling_orthogonality_and_pedal() {
        for c in corpus() {
            assert!(c.ruling().dot(c.normal()).is_zero(), "r·n = 0");
            assert!(c.ruling().dot(c.normal_deriv()).is_zero(), "r·n′ = 0");
            assert!(
                c.pedal().dot(c.ruling()).is_zero(),
                "c·r = 0 (pedal ⊥ ruling)"
            );
        }
    }

    #[test]
    fn normal_pointwise_hand_value() {
        // q = 1 + σi ⇒ n = [0, −2σ, 1−σ²]/(1+σ²); at σ = 2 that is [0, −4, −3]/5.
        let c = chart([&[1], &[0, 1], &[0], &[0]], &[0]);
        assert_eq!(
            c.normal().eval(&Q::from_i128(2)),
            Some([Q::from_i128(0), Q::new(-4, 5), Q::new(-3, 5)]),
        );
    }

    #[test]
    fn cone_has_zero_pedal_and_no_constant_detj() {
        // h ≡ 0 ⇒ rulings pass through the origin ⇒ pedal c ≡ 0 ⇒ det J has no μ⁰w⁰ term.
        let c = chart([&[1], &[0, 1], &[1], &[0]], &[0]);
        assert!(c.pedal().is_zero(), "h ≡ 0 ⇒ pedal is the origin");
        assert!(
            c.det_j().constant.is_zero(),
            "c ≡ 0 ⇒ det J constant term vanishes"
        );
        // det J's w-coefficient is exactly |n′|².
        assert_eq!(c.det_j().w, *c.normal_deriv_sq());
    }

    #[test]
    fn det_j_eval_matches_affine_form() {
        let c = chart([&[1], &[0, 1], &[1], &[0]], &[1]);
        let dj = c.det_j();
        let (sigma, mu, w) = (Q::from_i128(2), Q::new(3, 2), Q::from_i128(-1));
        let want = dj
            .constant
            .eval(&sigma)
            .unwrap()
            .add(&dj.mu.eval(&sigma).unwrap().mul(&mu))
            .add(&dj.w.eval(&sigma).unwrap().mul(&w));
        assert_eq!(dj.eval(&sigma, &mu, &w), Some(want));
    }

    #[test]
    fn offsets_in_family() {
        // C(σ,μ,w; h) = X(σ,μ; h+w): thickening by a constant w equals shifting h by w.
        let q = [&[1i128] as &[i128], &[0, 1], &[1], &[0]];
        let base = chart(q, &[1]); // h = 1
        let shifted = chart(q, &[3]); // h = 1 + 2
        let w0 = Q::from_i128(2);
        let mu = Q::new(1, 2);
        // surface of base at thickness w0 == surface of h-shifted-by-w0 at thickness 0.
        assert_eq!(
            base.surface(&mu, &w0),
            shifted.surface(&mu, &Q::from_i128(0))
        );
    }
}
