//! Dense univariate polynomials over ℚ — `Poly<B> = Σ coeffs[i]·xⁱ`. Pure,
//! `no_std`, total. The base for Sturm (`sturm`), resultants (`resultant`), and
//! the L3 algebraic-number defining polynomials (`algebraic`).
//!
//! Canonical form: `coeffs` is empty for the zero polynomial, otherwise its last
//! entry (the leading coefficient) is nonzero. [`Poly::from_coeffs`] is the single
//! trimming point, so structural `==` on `coeffs` is value equality.

use crate::backend::Backend;
use crate::bignum::Bignum;
use crate::rat::Rat;
use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

/// A polynomial in `ℚ[x]`, coefficients low-degree first (`coeffs[i]` = coeff of xⁱ).
pub struct Poly<B: Backend = Bignum> {
    pub(crate) coeffs: Vec<Rat<B>>,
}

fn q0<B: Backend>() -> Rat<B> {
    Rat::from_i128(0)
}

impl<B: Backend> Poly<B> {
    /// The zero polynomial.
    pub fn zero() -> Self {
        Poly { coeffs: Vec::new() }
    }
    /// The constant polynomial `c` (trimmed to zero if `c == 0`).
    pub fn constant(c: Rat<B>) -> Self {
        Self::from_coeffs(vec![c])
    }
    /// Build from low-degree-first coefficients, trimming trailing zeros to
    /// canonical form. The one canonicalization point.
    pub fn from_coeffs(mut coeffs: Vec<Rat<B>>) -> Self {
        while coeffs.last().is_some_and(|c| c.is_zero()) {
            coeffs.pop();
        }
        Poly { coeffs }
    }

    /// Whether this is the zero polynomial.
    pub fn is_zero(&self) -> bool {
        self.coeffs.is_empty()
    }
    /// Degree, or `None` for the zero polynomial.
    pub fn degree(&self) -> Option<usize> {
        self.coeffs.len().checked_sub(1)
    }
    /// Leading (highest-degree) coefficient, or `None` if zero.
    pub fn leading(&self) -> Option<&Rat<B>> {
        self.coeffs.last()
    }
    /// The coefficients, low-degree first (empty for the zero polynomial).
    pub fn coeffs(&self) -> &[Rat<B>] {
        &self.coeffs
    }

    // ---- ring operations -----------------------------------------------------

    /// `self + o`.
    pub fn add(&self, o: &Self) -> Self {
        let n = self.coeffs.len().max(o.coeffs.len());
        let mut c = Vec::with_capacity(n);
        for i in 0..n {
            let a = self.coeffs.get(i).cloned().unwrap_or_else(q0);
            let b = o.coeffs.get(i).cloned().unwrap_or_else(q0);
            c.push(a.add(&b));
        }
        Self::from_coeffs(c)
    }
    /// `self - o`.
    pub fn sub(&self, o: &Self) -> Self {
        let n = self.coeffs.len().max(o.coeffs.len());
        let mut c = Vec::with_capacity(n);
        for i in 0..n {
            let a = self.coeffs.get(i).cloned().unwrap_or_else(q0);
            let b = o.coeffs.get(i).cloned().unwrap_or_else(q0);
            c.push(a.sub(&b));
        }
        Self::from_coeffs(c)
    }
    /// `-self` (negate every coefficient).
    pub fn neg(&self) -> Self {
        Poly {
            coeffs: self.coeffs.iter().map(|c| c.neg()).collect(),
        }
    }
    /// Multiply every coefficient by the scalar `s`.
    pub fn scale(&self, s: &Rat<B>) -> Self {
        if s.is_zero() {
            return Self::zero();
        }
        Poly {
            coeffs: self.coeffs.iter().map(|c| c.mul(s)).collect(),
        }
    }
    /// `self * o` (schoolbook convolution).
    pub fn mul(&self, o: &Self) -> Self {
        if self.is_zero() || o.is_zero() {
            return Self::zero();
        }
        let mut c = vec![q0::<B>(); self.coeffs.len() + o.coeffs.len() - 1];
        for (i, a) in self.coeffs.iter().enumerate() {
            for (j, b) in o.coeffs.iter().enumerate() {
                c[i + j] = c[i + j].add(&a.mul(b));
            }
        }
        Self::from_coeffs(c)
    }

    /// Evaluate at a rational point (Horner).
    pub fn eval(&self, x: &Rat<B>) -> Rat<B> {
        let mut acc = q0::<B>();
        for c in self.coeffs.iter().rev() {
            acc = acc.mul(x).add(c);
        }
        acc
    }

    /// Formal derivative `Σ i·coeffs[i]·x^{i-1}`.
    pub fn derivative(&self) -> Self {
        if self.coeffs.len() < 2 {
            return Self::zero();
        }
        let c = (1..self.coeffs.len())
            .map(|i| self.coeffs[i].mul(&Rat::from_i128(i as i128)))
            .collect();
        Self::from_coeffs(c)
    }

    /// `(quotient, remainder)` with `deg(rem) < deg(divisor)`. `d != 0` by contract
    /// (a zero divisor yields `(0, self)`, panic-free).
    pub fn divrem(&self, d: &Self) -> (Self, Self) {
        let dd = match d.degree() {
            Some(dd) => dd,
            None => {
                debug_assert!(false, "Poly::divrem: division by the zero polynomial");
                return (Self::zero(), self.clone());
            }
        };
        if self.coeffs.len() <= dd {
            return (Self::zero(), self.clone());
        }
        // `dd = d.degree()` was `Some`, so `d` is nonzero and `d.leading()` is `Some`; the
        // `None` arm is unreachable but total (a zero divisor yields `(0, self)`).
        let dlead = match d.leading() {
            Some(l) => l.clone(),
            None => return (Self::zero(), self.clone()),
        };
        let mut r = self.coeffs.clone();
        let mut q = vec![q0::<B>(); self.coeffs.len() - dd];
        while r.len() > dd {
            let rdeg = r.len() - 1;
            let factor = r[rdeg].div(&dlead);
            let shift = rdeg - dd;
            for i in 0..=dd {
                r[shift + i] = r[shift + i].sub(&factor.mul(&d.coeffs[i]));
            }
            q[shift] = factor;
            // the leading term is now zero; drop it (and any it exposed)
            while r.last().is_some_and(|c| c.is_zero()) {
                r.pop();
            }
        }
        (Self::from_coeffs(q), Self::from_coeffs(r))
    }
    /// Polynomial remainder of `self` divided by `d` (the `.1` of [`Self::divrem`]).
    pub fn rem(&self, d: &Self) -> Self {
        self.divrem(d).1
    }

    /// Divide by the leading coefficient → monic (zero stays zero). Canonical form
    /// for gcd / squarefree results.
    pub fn monic(&self) -> Self {
        match self.leading() {
            None => Self::zero(),
            Some(lead) => self.scale(&lead.recip()),
        }
    }

    /// Monic gcd (Euclid). `gcd(0,0) = 0`.
    pub fn gcd(&self, o: &Self) -> Self {
        let mut a = self.clone();
        let mut b = o.clone();
        while !b.is_zero() {
            let r = a.rem(&b);
            a = b;
            b = r;
        }
        a.monic()
    }

    /// The squarefree part `self / gcd(self, self')` (monic). Zero stays zero.
    pub fn squarefree_part(&self) -> Self {
        if self.is_zero() {
            return Self::zero();
        }
        let g = self.gcd(&self.derivative());
        self.divrem(&g).0.monic()
    }
}

impl<B: Backend> Clone for Poly<B> {
    fn clone(&self) -> Self {
        Poly {
            coeffs: self.coeffs.clone(),
        }
    }
}
impl<B: Backend> PartialEq for Poly<B> {
    fn eq(&self, o: &Self) -> bool {
        self.coeffs == o.coeffs // canonical ⇒ structural equality is value equality
    }
}
impl<B: Backend> Eq for Poly<B> {}
impl<B: Backend> fmt::Debug for Poly<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Poly(deg={:?})", self.degree())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type P = Poly<Bignum>;
    fn p(cs: &[i128]) -> P {
        P::from_coeffs(cs.iter().map(|&c| Rat::from_i128(c)).collect())
    }

    #[test]
    fn arith_and_eval() {
        let a = p(&[1, 2, 3]); // 1 + 2x + 3x²
        let b = p(&[0, 1]); // x
        assert_eq!(a.add(&b), p(&[1, 3, 3]));
        assert_eq!(a.mul(&b), p(&[0, 1, 2, 3]));
        assert_eq!(a.eval(&Rat::from_i128(2)), Rat::from_i128(17)); // 1+4+12
        assert_eq!(a.derivative(), p(&[2, 6])); // 2 + 6x
        assert!(p(&[0, 0]).is_zero());
        assert_eq!(p(&[5]).degree(), Some(0));
        assert_eq!(P::zero().degree(), None);
    }

    #[test]
    fn divrem_roundtrip() {
        // (x² − 1) = (x − 1)(x + 1) + 0
        let n = p(&[-1, 0, 1]);
        let d = p(&[-1, 1]); // x − 1
        let (q, r) = n.divrem(&d);
        assert_eq!(q, p(&[1, 1])); // x + 1
        assert!(r.is_zero());
        assert_eq!(q.mul(&d).add(&r), n);
        // a remainder case: x³ + 1 by x² + 1 → q = x, r = −x + 1
        let (q2, r2) = p(&[1, 0, 0, 1]).divrem(&p(&[1, 0, 1]));
        assert_eq!(q2.mul(&p(&[1, 0, 1])).add(&r2), p(&[1, 0, 0, 1]));
        assert!(r2.degree().unwrap() < 2);
    }

    #[test]
    fn gcd_and_squarefree() {
        // gcd(x² − 1, x − 1) = x − 1 (monic)
        assert_eq!(p(&[-1, 0, 1]).gcd(&p(&[-1, 1])), p(&[-1, 1]));
        // squarefree_part((x−1)²(x−2)) = (x−1)(x−2) monic
        let sq = p(&[-1, 1]).mul(&p(&[-1, 1])).mul(&p(&[-2, 1])); // (x−1)²(x−2)
        let want = p(&[-1, 1]).mul(&p(&[-2, 1])); // (x−1)(x−2) = x²−3x+2
        assert_eq!(sq.squarefree_part(), want);
    }
}
