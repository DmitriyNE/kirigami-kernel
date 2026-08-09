//! Exact monomial→Bernstein conversion of σ-parametric rational geometry into a
//! **rational Bézier** carrier — the one new curve primitive slice 3 emits.
//!
//! A chart's σ-parametric fields (its pedal, normal, ruling) are exact rational
//! functions of σ (a [`Vec3Rat`] — three numerator polynomials over one shared
//! denominator). A boundary curve of a ruled flank face is one such `Vec3Rat`
//! restricted to a σ-interval. To hand that curve to a CAD kernel as an *exact*
//! rational Bézier we convert it from the power basis to the Bernstein basis of the
//! curve's own degree, over the interval — no sampling, no approximation.
//!
//! The homogeneous view makes this a change of basis on four polynomials. Writing the
//! curve as `(N₀, N₁, N₂, D)` in σ (the three numerators and the shared denominator),
//! each polynomial converts independently to degree-`n` Bernstein coefficients over
//! `[a, b]` (`n` = the max degree across the four). The Bernstein coefficients of `D`
//! are the **weights** `wᵢ`; the Bernstein coefficients of `Nₖ` are the **weighted
//! poles** `wᵢ·bᵢ` (the projective/homogeneous poles). This crate stores the
//! homogeneous form directly — it is total (no division by a zero weight) and lossless,
//! and the affine pole `bᵢ = wᵢbᵢ / wᵢ` is recovered on demand by [`RatBezier::pole`].
//!
//! Everything is exact over ℚ: [`Rat`] coefficients throughout, no floating point (the
//! exact→`f64` cast lives later, in the feature-gated STEP bridge). This module is
//! **always compiled** and float-free, like [`crate::shell`].
//!
//! # Example
//!
//! ```
//! use export::bezier::RatBezier;
//! use lattice::{Bignum, Poly, Rat, Vec3Rat};
//!
//! // The straight segment r(σ) = (σ, 2σ, 0) as a polynomial vector (denominator 1),
//! // restricted to σ ∈ [0, 1]. A degree-1 curve → two poles, unit weights.
//! let p = |cs: &[i128]| Poly::<Bignum>::from_coeffs(cs.iter().map(|&c| Rat::from_i128(c)).collect());
//! let r = Vec3Rat::from_polys([p(&[0, 1]), p(&[0, 2]), p(&[0])]);
//! let bez = RatBezier::from_vec3rat(&r, &Rat::from_i128(0), &Rat::from_i128(1));
//! assert_eq!(bez.degree(), 1);
//! assert_eq!(bez.pole(0), Some([Rat::from_i128(0), Rat::from_i128(0), Rat::from_i128(0)]));
//! assert_eq!(bez.pole(1), Some([Rat::from_i128(1), Rat::from_i128(2), Rat::from_i128(0)]));
//! // The Bézier reproduces the curve exactly: at t = ½, σ = ½ → (½, 1, 0).
//! assert_eq!(bez.eval(&Rat::new(1, 2)), Some([Rat::new(1, 2), Rat::from_i128(1), Rat::from_i128(0)]));
//! ```

use lattice::{Backend, Poly, Rat, Vec3Rat};

/// Multiply a power-basis polynomial (coefficient slice, low degree first) by the
/// linear factor `lin = [c₀, c₁]` (i.e. `c₀ + c₁·t`), returning the product's
/// coefficients.
fn mul_linear<B: Backend>(p: &[Rat<B>], lin: &[Rat<B>; 2]) -> Vec<Rat<B>> {
    let mut out = vec![Rat::<B>::from_i128(0); p.len() + 1];
    for (i, pi) in p.iter().enumerate() {
        out[i] = out[i].add(&pi.mul(&lin[0]));
        out[i + 1] = out[i + 1].add(&pi.mul(&lin[1]));
    }
    out
}

/// The Pascal triangle of binomial coefficients `C(i, k)` for `0 ≤ k ≤ i ≤ n`, as exact
/// [`Rat`]s (built by the additive recurrence `C(i,k) = C(i−1,k−1) + C(i−1,k)`, so no
/// factorial overflow). Row `i` has `i + 1` entries.
fn binomials<B: Backend>(n: usize) -> Vec<Vec<Rat<B>>> {
    let mut rows: Vec<Vec<Rat<B>>> = Vec::with_capacity(n + 1);
    for i in 0..=n {
        let mut row = vec![Rat::<B>::from_i128(1); i + 1];
        for k in 1..i {
            row[k] = rows[i - 1][k - 1].add(&rows[i - 1][k]);
        }
        rows.push(row);
    }
    rows
}

/// Convert a power-basis polynomial `p(σ)` to its degree-`n` **Bernstein coefficients**
/// over the interval `[a, b]` (`n` must be at least `deg p`). The returned vector has
/// `n + 1` entries `β₀ … βₙ` with `p(σ) = Σᵢ βᵢ · Bᵢⁿ(t)`, where `t = (σ − a)/(b − a)`
/// and `Bᵢⁿ(t) = C(n,i) tⁱ (1−t)ⁿ⁻ⁱ`.
///
/// Exact via two textbook steps: reparametrize `p(σ) → q(t) = p(a + (b−a)·t)` (a power
/// polynomial in `t`), then apply the identity `tᵐ = Σᵢ₌ₘⁿ [C(i,m)/C(n,m)] Bᵢⁿ(t)`, so
/// `βᵢ = Σₘ₌₀ⁱ q_m · C(i,m)/C(n,m)`. Degree elevation is free: choosing `n > deg p`
/// just yields the elevated coefficients.
///
/// # Example
///
/// ```
/// use export::bezier::poly_to_bernstein;
/// use lattice::{Bignum, Poly, Rat};
///
/// // The constant 1 in degree-2 Bernstein form is (1, 1, 1) (the basis is a partition of unity).
/// let one = Poly::<Bignum>::from_coeffs(vec![Rat::from_i128(1)]);
/// let b = poly_to_bernstein(&one, &Rat::from_i128(0), &Rat::from_i128(1), 2);
/// assert_eq!(b, vec![Rat::from_i128(1); 3]);
/// ```
pub fn poly_to_bernstein<B: Backend>(p: &Poly<B>, a: &Rat<B>, b: &Rat<B>, n: usize) -> Vec<Rat<B>> {
    // Reparametrize p(σ) → q(t) = p(a + h·t) by accumulating cⱼ·(a + h·t)ʲ.
    let h = b.sub(a);
    let lin = [a.clone(), h];
    let coeffs = p.coeffs();
    let mut q = vec![Rat::<B>::from_i128(0); coeffs.len().max(1)];
    let mut pow = vec![Rat::<B>::from_i128(1)]; // (a + h·t)⁰ = 1
    for (j, cj) in coeffs.iter().enumerate() {
        for (m, pm) in pow.iter().enumerate() {
            q[m] = q[m].add(&cj.mul(pm));
        }
        if j + 1 < coeffs.len() {
            pow = mul_linear(&pow, &lin);
        }
    }
    q.resize(n + 1, Rat::from_i128(0));

    // βᵢ = Σₘ₌₀ⁱ q_m · C(i,m)/C(n,m).
    let binom = binomials::<B>(n);
    (0..=n)
        .map(|i| {
            let mut beta = Rat::<B>::from_i128(0);
            for m in 0..=i {
                let ratio = binom[i][m].div(&binom[n][m]);
                beta = beta.add(&q[m].mul(&ratio));
            }
            beta
        })
        .collect()
}

/// An exact **rational Bézier** curve in the homogeneous (weighted-pole) form: a list of
/// weighted poles `wᵢ·bᵢ ∈ ℚ³` and matching weights `wᵢ ∈ ℚ`, of common length `n + 1`
/// (degree `n`). The curve is
///
/// ```text
/// C(t) = ( Σᵢ (wᵢ·bᵢ) Bᵢⁿ(t) ) / ( Σᵢ wᵢ Bᵢⁿ(t) ),   t ∈ [0, 1].
/// ```
///
/// Storing the weighted poles (rather than the affine poles `bᵢ`) keeps construction
/// total — a zero weight is representable and [`eval`](Self::eval) still works — and is
/// exactly the numerator/denominator Bernstein split that [`from_vec3rat`](Self::from_vec3rat)
/// produces. The affine pole `bᵢ` is recovered by [`pole`](Self::pole) where `wᵢ ≠ 0`.
pub struct RatBezier<B: Backend = lattice::Bignum> {
    /// The weighted poles `wᵢ·bᵢ` — the Bernstein coefficients of the curve's numerator.
    wpoles: Vec<[Rat<B>; 3]>,
    /// The weights `wᵢ` — the Bernstein coefficients of the curve's shared denominator.
    weights: Vec<Rat<B>>,
}

impl<B: Backend> RatBezier<B> {
    /// A rational Bézier from its weighted poles `wᵢ·bᵢ` and weights `wᵢ` (equal,
    /// nonzero length — debug-asserted, panic-free by convention).
    pub fn new(wpoles: Vec<[Rat<B>; 3]>, weights: Vec<Rat<B>>) -> Self {
        debug_assert!(
            !weights.is_empty() && wpoles.len() == weights.len(),
            "RatBezier::new: poles and weights must share a nonzero length"
        );
        RatBezier { wpoles, weights }
    }

    /// Convert a σ-parametric rational curve `v(σ) = N(σ)/D(σ)` (a [`Vec3Rat`]) to an
    /// exact rational Bézier over `σ ∈ [a, b]`. The weights are the Bernstein
    /// coefficients of the shared denominator `D`; the weighted poles are the Bernstein
    /// coefficients of the numerators `N₀, N₁, N₂`. Degree = the maximum degree across
    /// the four polynomials.
    pub fn from_vec3rat(v: &Vec3Rat<B>, a: &Rat<B>, b: &Rat<B>) -> Self {
        let deg = |p: &Poly<B>| p.degree().unwrap_or(0);
        let num = v.num();
        let n = deg(&num[0])
            .max(deg(&num[1]))
            .max(deg(&num[2]))
            .max(deg(v.den()));
        let weights = poly_to_bernstein(v.den(), a, b, n);
        let bx = poly_to_bernstein(&num[0], a, b, n);
        let by = poly_to_bernstein(&num[1], a, b, n);
        let bz = poly_to_bernstein(&num[2], a, b, n);
        let wpoles = (0..=n)
            .map(|i| [bx[i].clone(), by[i].clone(), bz[i].clone()])
            .collect();
        RatBezier { wpoles, weights }
    }

    /// The polynomial degree of the curve (`poles − 1`).
    pub fn degree(&self) -> usize {
        self.weights.len() - 1
    }

    /// The weights `wᵢ` (Bernstein coefficients of the denominator).
    pub fn weights(&self) -> &[Rat<B>] {
        &self.weights
    }

    /// The weighted poles `wᵢ·bᵢ` (Bernstein coefficients of the numerators).
    pub fn weighted_poles(&self) -> &[[Rat<B>; 3]] {
        &self.wpoles
    }

    /// The affine pole `bᵢ = (wᵢ·bᵢ) / wᵢ`, or `None` where the weight `wᵢ` is zero (a
    /// pole at infinity, which has no affine representative).
    pub fn pole(&self, i: usize) -> Option<[Rat<B>; 3]> {
        let w = self.weights.get(i)?;
        if w.is_zero() {
            return None;
        }
        let p = &self.wpoles[i];
        Some([p[0].div(w), p[1].div(w), p[2].div(w)])
    }

    /// Evaluate the curve at `t ∈ [0, 1]` (the *Bézier* parameter, not σ). Returns `None`
    /// where the weight denominator `Σᵢ wᵢ Bᵢⁿ(t)` vanishes. Exact: the Bernstein basis
    /// `Bᵢⁿ(t) = C(n,i) tⁱ (1−t)ⁿ⁻ⁱ` is evaluated over ℚ.
    pub fn eval(&self, t: &Rat<B>) -> Option<[Rat<B>; 3]> {
        let n = self.degree();
        let binom = binomials::<B>(n);
        let one_minus_t = Rat::<B>::from_i128(1).sub(t);
        // tⁱ and (1−t)ʲ tables.
        let mut t_pow = vec![Rat::<B>::from_i128(1); n + 1];
        let mut s_pow = vec![Rat::<B>::from_i128(1); n + 1];
        for i in 1..=n {
            t_pow[i] = t_pow[i - 1].mul(t);
            s_pow[i] = s_pow[i - 1].mul(&one_minus_t);
        }
        let mut num = [
            Rat::<B>::from_i128(0),
            Rat::<B>::from_i128(0),
            Rat::<B>::from_i128(0),
        ];
        let mut den = Rat::<B>::from_i128(0);
        for i in 0..=n {
            // Bᵢⁿ(t) = C(n,i) · tⁱ · (1−t)ⁿ⁻ⁱ.
            let basis = binom[n][i].mul(&t_pow[i]).mul(&s_pow[n - i]);
            den = den.add(&self.weights[i].mul(&basis));
            for (k, nk) in num.iter_mut().enumerate() {
                *nk = nk.add(&self.wpoles[i][k].mul(&basis));
            }
        }
        if den.is_zero() {
            return None;
        }
        Some([num[0].div(&den), num[1].div(&den), num[2].div(&den)])
    }
}

impl<B: Backend> Clone for RatBezier<B> {
    fn clone(&self) -> Self {
        RatBezier {
            wpoles: self.wpoles.clone(),
            weights: self.weights.clone(),
        }
    }
}

impl<B: Backend> core::fmt::Debug for RatBezier<B> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "RatBezier(deg={}, wpoles={:?}, weights={:?})",
            self.degree(),
            self.wpoles,
            self.weights
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice::Bignum;

    type Q = Rat<Bignum>;

    fn p(cs: &[i128]) -> Poly<Bignum> {
        Poly::from_coeffs(cs.iter().map(|&c| Q::from_i128(c)).collect())
    }

    /// The Bernstein basis is a partition of unity: the constant 1 has all-ones
    /// coefficients in any degree, over any interval.
    #[test]
    fn constant_is_partition_of_unity() {
        for n in 0..=4 {
            let b = poly_to_bernstein(&p(&[1]), &Q::from_i128(-2), &Q::from_i128(3), n);
            assert_eq!(b, vec![Q::from_i128(1); n + 1]);
        }
    }

    /// The identity σ over [0,1] in Bernstein form is (0, 1) — the endpoints interpolate,
    /// so the Bézier of a straight line has the segment's endpoints as its poles.
    #[test]
    fn linear_bernstein_over_unit_interval() {
        let b = poly_to_bernstein(&p(&[0, 1]), &Q::from_i128(0), &Q::from_i128(1), 1);
        assert_eq!(b, vec![Q::from_i128(0), Q::from_i128(1)]);
    }

    /// Bernstein coefficients over a non-unit interval [a,b] still interpolate the
    /// endpoints: β₀ = p(a) and βₙ = p(b), for the quadratic σ² over [1,3].
    #[test]
    fn endpoints_interpolate_over_shifted_interval() {
        let b = poly_to_bernstein(&p(&[0, 0, 1]), &Q::from_i128(1), &Q::from_i128(3), 2);
        assert_eq!(b[0], Q::from_i128(1)); // p(1) = 1
        assert_eq!(b[2], Q::from_i128(9)); // p(3) = 9
    }

    /// Degree elevation is free: converting σ (degree 1) into a degree-2 Bernstein form
    /// gives (0, ½, 1) over [0,1], and re-evaluating reproduces σ exactly.
    #[test]
    fn degree_elevation_reproduces_the_curve() {
        let b = poly_to_bernstein(&p(&[0, 1]), &Q::from_i128(0), &Q::from_i128(1), 2);
        assert_eq!(b, vec![Q::from_i128(0), Q::new(1, 2), Q::from_i128(1)]);
    }

    /// A rational curve reproduces its source exactly: the Bézier evaluated at t equals
    /// the `Vec3Rat` evaluated at σ = a + t·(b−a), for a genuinely rational curve
    /// (nonconstant denominator) sampled at several t.
    #[test]
    fn rational_bezier_reproduces_the_source_curve() {
        // v(σ) = (σ, 1, 0) / (σ² + 1) — a rational curve with a degree-2 denominator.
        let v = Vec3Rat::new([p(&[0, 1]), p(&[1]), p(&[0])], p(&[1, 0, 1]));
        let (a, b) = (Q::from_i128(0), Q::from_i128(2));
        let bez = RatBezier::from_vec3rat(&v, &a, &b);
        assert_eq!(bez.degree(), 2);
        for (tn, td) in [(0, 1), (1, 3), (1, 2), (3, 4), (1, 1)] {
            let t = Q::new(tn, td);
            let sigma = a.add(&t.mul(&b.sub(&a)));
            assert_eq!(
                bez.eval(&t),
                v.eval(&sigma),
                "curve reproduced at t = {tn}/{td}"
            );
        }
    }

    /// For a polynomial curve (denominator 1) the weights are all 1 and the affine poles
    /// are the numerator Bernstein coefficients — a genuine (non-rational) Bézier.
    #[test]
    fn polynomial_curve_has_unit_weights() {
        let v = Vec3Rat::from_polys([p(&[0, 0, 1]), p(&[0]), p(&[0])]); // (σ², 0, 0)
        let bez = RatBezier::from_vec3rat(&v, &Q::from_i128(0), &Q::from_i128(1));
        assert_eq!(
            bez.weights(),
            &[Q::from_i128(1), Q::from_i128(1), Q::from_i128(1)]
        );
        // σ² over [0,1] has poles x-coords (0, 0, 1).
        assert_eq!(
            bez.pole(0),
            Some([Q::from_i128(0), Q::from_i128(0), Q::from_i128(0)])
        );
        assert_eq!(
            bez.pole(2),
            Some([Q::from_i128(1), Q::from_i128(0), Q::from_i128(0)])
        );
    }
}
