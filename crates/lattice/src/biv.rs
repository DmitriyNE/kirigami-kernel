//! Dense bivariate polynomials over ℚ — `Biv<B> = Σᵢ rows[i](y)·xⁱ`, a polynomial in
//! `x` whose `x`-coefficients are univariate [`Poly<B>`]s in `y`. Pure, `no_std`, total.
//!
//! The `x`-coefficients-low-degree-first convention matches
//! [`resultant_bivariate`](crate::resultant_bivariate)'s `&[Poly<B>]`. Built for the
//! transverse **MITER-FIT** correspondence `R(σ_A, σ_B)` and its resultant-conditioned
//! **cofactor identities** `X == R·Q` (spec §5.3): the identity is an *exact* bivariate
//! polynomial equality, so its soundness is `Biv`'s value `==` — `X = R·Q ⇒ X ≡ 0` on the
//! correspondence variety `{R = 0}`, with no floating point and no bivariate division.
//!
//! # Example
//!
//! ```
//! use lattice::{Biv, Bignum, Poly, Rat};
//!
//! let p = |cs: &[i128]| Poly::<Bignum>::from_coeffs(cs.iter().map(|&c| Rat::from_i128(c)).collect());
//! // (x − y)·(x + y) = x² − y²  — the difference-of-squares identity, exact.
//! let x = Biv::from_x_poly(&p(&[0, 1])); // x
//! let y = Biv::from_y_poly(&p(&[0, 1])); // y
//! let lhs = x.sub(&y).mul(&x.add(&y));
//! let rhs = x.mul(&x).sub(&y.mul(&y));
//! assert_eq!(lhs, rhs);
//! assert_eq!(lhs.eval(&Rat::from_i128(3), &Rat::from_i128(2)), Rat::from_i128(5)); // 9 − 4
//! ```

use crate::backend::Backend;
use crate::bignum::Bignum;
use crate::poly::Poly;
use crate::rat::Rat;
use alloc::vec;
use alloc::vec::Vec;

/// A polynomial in `ℚ[x, y]`, stored as `x`-coefficients low-degree first: `rows[i]` is
/// the `Poly<B>` in `y` multiplying `xⁱ`. Canonical form: `rows` is empty for the zero
/// polynomial, otherwise its last entry is nonzero — [`Biv::from_rows`] is the single
/// trimming point, so structural `==` on `rows` is value equality (as with [`Poly`]).
pub struct Biv<B: Backend = Bignum> {
    rows: Vec<Poly<B>>,
}

impl<B: Backend> Biv<B> {
    /// The zero polynomial.
    pub fn zero() -> Self {
        Biv { rows: Vec::new() }
    }

    /// Build from `x`-coefficients (each a `y`-[`Poly`]), low-`x`-degree first, trimming
    /// trailing zero rows to canonical form. The one canonicalization point.
    pub fn from_rows(mut rows: Vec<Poly<B>>) -> Self {
        while rows.last().is_some_and(|r| r.is_zero()) {
            rows.pop();
        }
        Biv { rows }
    }

    /// Lift a univariate [`Poly`] in `x` (constant in `y`): the `xⁱ` coefficient is the
    /// constant `y`-poly `p[i]`.
    pub fn from_x_poly(p: &Poly<B>) -> Self {
        Self::from_rows(
            p.coeffs()
                .iter()
                .map(|c| Poly::constant(c.clone()))
                .collect(),
        )
    }

    /// Lift a univariate [`Poly`] in `y` (constant in `x`): the single `x⁰` coefficient is `p`.
    pub fn from_y_poly(p: &Poly<B>) -> Self {
        Self::from_rows(vec![p.clone()])
    }

    /// Whether this is the zero polynomial.
    pub fn is_zero(&self) -> bool {
        self.rows.is_empty()
    }

    /// The `x`-coefficient rows (each a `y`-[`Poly`]), low-`x`-degree first (empty for zero).
    pub fn rows(&self) -> &[Poly<B>] {
        &self.rows
    }

    /// `self + o`.
    pub fn add(&self, o: &Self) -> Self {
        let n = self.rows.len().max(o.rows.len());
        let mut rows = Vec::with_capacity(n);
        for i in 0..n {
            rows.push(match (self.rows.get(i), o.rows.get(i)) {
                (Some(a), Some(b)) => a.add(b),
                (Some(a), None) => a.clone(),
                (None, Some(b)) => b.clone(),
                (None, None) => Poly::zero(),
            });
        }
        Self::from_rows(rows)
    }

    /// `self - o`.
    pub fn sub(&self, o: &Self) -> Self {
        let n = self.rows.len().max(o.rows.len());
        let mut rows = Vec::with_capacity(n);
        for i in 0..n {
            rows.push(match (self.rows.get(i), o.rows.get(i)) {
                (Some(a), Some(b)) => a.sub(b),
                (Some(a), None) => a.clone(),
                (None, Some(b)) => b.neg(),
                (None, None) => Poly::zero(),
            });
        }
        Self::from_rows(rows)
    }

    /// `self * o` — schoolbook convolution in `x`, [`Poly`] products in `y`.
    pub fn mul(&self, o: &Self) -> Self {
        if self.is_zero() || o.is_zero() {
            return Self::zero();
        }
        let mut rows: Vec<Poly<B>> = (0..self.rows.len() + o.rows.len() - 1)
            .map(|_| Poly::zero())
            .collect();
        for (i, a) in self.rows.iter().enumerate() {
            for (j, b) in o.rows.iter().enumerate() {
                rows[i + j] = rows[i + j].add(&a.mul(b));
            }
        }
        Self::from_rows(rows)
    }

    /// Evaluate at `(x, y)` — Horner in `x`, each row evaluated at `y`.
    pub fn eval(&self, x: &Rat<B>, y: &Rat<B>) -> Rat<B> {
        let mut acc = Rat::from_i128(0);
        for row in self.rows.iter().rev() {
            acc = acc.mul(x).add(&row.eval(y));
        }
        acc
    }
}

impl<B: Backend> Clone for Biv<B> {
    fn clone(&self) -> Self {
        Biv {
            rows: self.rows.clone(),
        }
    }
}
impl<B: Backend> PartialEq for Biv<B> {
    fn eq(&self, o: &Self) -> bool {
        self.rows == o.rows // canonical ⇒ structural equality is value equality
    }
}
impl<B: Backend> Eq for Biv<B> {}
impl<B: Backend> core::fmt::Debug for Biv<B> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Biv(x_deg={:?})", self.rows.len().checked_sub(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type P = Poly<Bignum>;
    type Q = Rat<Bignum>;
    fn p(cs: &[i128]) -> P {
        P::from_coeffs(cs.iter().map(|&c| Q::from_i128(c)).collect())
    }
    fn x() -> Biv<Bignum> {
        Biv::from_x_poly(&p(&[0, 1]))
    }
    fn y() -> Biv<Bignum> {
        Biv::from_y_poly(&p(&[0, 1]))
    }

    #[test]
    fn zero_and_lifts() {
        assert!(Biv::<Bignum>::zero().is_zero());
        assert!(Biv::from_x_poly(&P::zero()).is_zero());
        assert!(Biv::from_y_poly(&P::zero()).is_zero());
        // x and y are distinct and nonzero.
        assert!(!x().is_zero() && !y().is_zero());
        assert_ne!(x(), y());
    }

    #[test]
    fn square_of_sum_expands() {
        // (x + y)² = x² + 2xy + y².
        let s = x().add(&y());
        let sq = s.mul(&s);
        // check by evaluation at a spread of points (exact).
        for (xv, yv) in [(0, 0), (1, 0), (0, 1), (2, 3), (-1, 5), (7, -2)] {
            let want = (xv + yv) * (xv + yv);
            assert_eq!(
                sq.eval(&Q::from_i128(xv), &Q::from_i128(yv)),
                Q::from_i128(want)
            );
        }
    }

    #[test]
    fn cofactor_identity_shape() {
        // R = x − y (a correspondence); X = x² − y² = R·(x + y). The exact cofactor check
        // the transverse MITER-FIT certificate runs: X == R·Q with Q = x + y.
        let r = x().sub(&y());
        let x2_minus_y2 = x().mul(&x()).sub(&y().mul(&y()));
        let q = x().add(&y());
        assert_eq!(x2_minus_y2, r.mul(&q));
        // A wrong cofactor is caught: X != R·(x + y + 1).
        let wrong = q.add(&Biv::from_x_poly(&p(&[1])));
        assert_ne!(x2_minus_y2, r.mul(&wrong));
    }

    #[test]
    fn add_sub_cancel() {
        let a = x().mul(&x()).add(&y());
        let b = x().sub(&y().mul(&y()));
        assert_eq!(a.add(&b).sub(&b), a);
        assert!(a.sub(&a).is_zero());
    }
}
