//! Exact rational functions of one variable — [`RatFunc`] = `num/den` over [`Poly`],
//! and [`Vec3Rat`], a 3-vector of them sharing one denominator.
//!
//! This is the arithmetic substrate for σ-parametric geometry: a chart's normal,
//! ruling, and pedal fields are rational functions of the parameter σ, built by
//! composing polynomial numerators over a shared denominator (e.g. `|q|²`). A
//! [`Vec3Rat`] keeps that single common denominator across all three components, which
//! avoids gcd blow-up and makes [`dot`](Vec3Rat::dot) / [`cross`](Vec3Rat::cross)
//! single-denominator.
//!
//! Everything is exact over ℚ — no floating point. Two values are compared by
//! cross-multiplication (the `PartialEq` impls), so unreduced and reduced forms of the
//! same function compare equal; call [`RatFunc::reduce`] when you want a canonical
//! representative. The denominator is a nonzero-polynomial invariant (a zero
//! denominator is out of contract, debug-asserted, never a panic).
//!
//! # Example
//!
//! ```
//! use lattice::{Bignum, Poly, Rat, RatFunc};
//!
//! // f = x / (x + 1)
//! let x = Poly::<Bignum>::from_coeffs(vec![Rat::from_i128(0), Rat::from_i128(1)]);
//! let x_plus_1 = Poly::from_coeffs(vec![Rat::from_i128(1), Rat::from_i128(1)]);
//! let f = RatFunc::new(x, x_plus_1);
//!
//! assert_eq!(f.eval(&Rat::from_i128(1)), Some(Rat::new(1, 2))); // f(1) = 1/2
//!
//! // f + f = 2x/(x+1); at x = 1 that is 1.
//! let g = f.add(&f);
//! assert_eq!(g.eval(&Rat::from_i128(1)), Some(Rat::from_i128(1)));
//! ```

use crate::backend::Backend;
use crate::bignum::Bignum;
use crate::poly::Poly;
use crate::rat::Rat;

/// A rational function `num / den` over ℚ, with `den` a nonzero polynomial.
///
/// Not stored in reduced form — arithmetic cross-multiplies and lets numerator and
/// denominator grow; [`reduce`](Self::reduce) divides out their gcd when a canonical
/// form is wanted. Equality is by value (`na·db == nb·da`), independent of reduction.
pub struct RatFunc<B: Backend = Bignum> {
    num: Poly<B>,
    den: Poly<B>,
}

impl<B: Backend> RatFunc<B> {
    /// The rational function `num / den`. `den` must be nonzero (debug-asserted;
    /// panic-free by convention like [`Rat::new`]).
    pub fn new(num: Poly<B>, den: Poly<B>) -> Self {
        debug_assert!(!den.is_zero(), "RatFunc::new: zero denominator");
        RatFunc { num, den }
    }
    /// The polynomial `p` as `p / 1`.
    pub fn from_poly(p: Poly<B>) -> Self {
        RatFunc {
            num: p,
            den: Poly::constant(Rat::from_i128(1)),
        }
    }
    /// The constant `0`.
    pub fn zero() -> Self {
        Self::from_poly(Poly::zero())
    }
    /// The constant `1`.
    pub fn one() -> Self {
        Self::from_poly(Poly::constant(Rat::from_i128(1)))
    }

    /// The numerator (not necessarily reduced against the denominator).
    pub fn num(&self) -> &Poly<B> {
        &self.num
    }
    /// The denominator (nonzero).
    pub fn den(&self) -> &Poly<B> {
        &self.den
    }
    /// Whether this is the zero function (numerator zero; the denominator is nonzero).
    pub fn is_zero(&self) -> bool {
        self.num.is_zero()
    }

    /// `self + o`.
    pub fn add(&self, o: &Self) -> Self {
        RatFunc {
            num: self.num.mul(&o.den).add(&o.num.mul(&self.den)),
            den: self.den.mul(&o.den),
        }
    }
    /// `self - o`.
    pub fn sub(&self, o: &Self) -> Self {
        RatFunc {
            num: self.num.mul(&o.den).sub(&o.num.mul(&self.den)),
            den: self.den.mul(&o.den),
        }
    }
    /// `self * o`.
    pub fn mul(&self, o: &Self) -> Self {
        RatFunc {
            num: self.num.mul(&o.num),
            den: self.den.mul(&o.den),
        }
    }
    /// Multiply by the scalar `s`.
    pub fn scale(&self, s: &Rat<B>) -> Self {
        RatFunc {
            num: self.num.scale(s),
            den: self.den.clone(),
        }
    }
    /// `-self`.
    pub fn neg(&self) -> Self {
        RatFunc {
            num: self.num.neg(),
            den: self.den.clone(),
        }
    }
    /// `self / o`. `o` must be nonzero (debug-asserted; panic-free by convention).
    pub fn div(&self, o: &Self) -> Self {
        debug_assert!(!o.num.is_zero(), "RatFunc::div: division by zero");
        RatFunc {
            num: self.num.mul(&o.den),
            den: self.den.mul(&o.num),
        }
    }
    /// The reciprocal `1 / self`. `self` must be nonzero (debug-asserted).
    pub fn recip(&self) -> Self {
        debug_assert!(!self.num.is_zero(), "RatFunc::recip: reciprocal of zero");
        RatFunc {
            num: self.den.clone(),
            den: self.num.clone(),
        }
    }

    /// The derivative, by the quotient rule `(n′·d − n·d′) / d²`.
    pub fn derivative(&self) -> Self {
        let n = &self.num;
        let d = &self.den;
        RatFunc {
            num: n.derivative().mul(d).sub(&n.mul(&d.derivative())),
            den: d.mul(d),
        }
    }

    /// A canonical representative: numerator and denominator divided by their gcd, with
    /// the denominator's leading coefficient made positive. Value-preserving.
    pub fn reduce(&self) -> Self {
        if self.num.is_zero() {
            return Self::zero();
        }
        let g = self.num.gcd(&self.den);
        let mut num = self.num.divrem(&g).0;
        let mut den = self.den.divrem(&g).0;
        // Canonical sign: denominator leads positive.
        if den.leading().map(Rat::sign) == Some(-1) {
            num = num.neg();
            den = den.neg();
        }
        RatFunc { num, den }
    }

    /// Evaluate at the rational point `x`, or `None` where the denominator vanishes.
    pub fn eval(&self, x: &Rat<B>) -> Option<Rat<B>> {
        let d = self.den.eval(x);
        if d.is_zero() {
            None
        } else {
            Some(self.num.eval(x).div(&d))
        }
    }
}

/// A 3-vector of rational functions in **common-denominator form**: three numerator
/// polynomials over one shared denominator.
///
/// The shared denominator is what keeps [`dot`](Self::dot) and [`cross`](Self::cross)
/// single-denominator and stops `|q|²`-style factors from being duplicated across
/// components. Build one from three polynomials with [`from_polys`](Self::from_polys)
/// (denominator `1`) or from an explicit `(numerators, den)` with [`new`](Self::new).
pub struct Vec3Rat<B: Backend = Bignum> {
    num: [Poly<B>; 3],
    den: Poly<B>,
}

impl<B: Backend> Vec3Rat<B> {
    /// The vector `num / den` (each component `num[i] / den`). `den` must be nonzero
    /// (debug-asserted; panic-free by convention).
    pub fn new(num: [Poly<B>; 3], den: Poly<B>) -> Self {
        debug_assert!(!den.is_zero(), "Vec3Rat::new: zero denominator");
        Vec3Rat { num, den }
    }
    /// Three polynomials as a vector with denominator `1`.
    pub fn from_polys(num: [Poly<B>; 3]) -> Self {
        Vec3Rat {
            num,
            den: Poly::constant(Rat::from_i128(1)),
        }
    }

    /// The three numerator polynomials (over the shared [`den`](Self::den)).
    pub fn num(&self) -> &[Poly<B>; 3] {
        &self.num
    }
    /// The shared denominator (nonzero).
    pub fn den(&self) -> &Poly<B> {
        &self.den
    }
    /// Component `i` (`0..3`) as a standalone [`RatFunc`].
    pub fn comp(&self, i: usize) -> RatFunc<B> {
        RatFunc::new(self.num[i].clone(), self.den.clone())
    }
    /// Whether every component is zero.
    pub fn is_zero(&self) -> bool {
        self.num.iter().all(Poly::is_zero)
    }

    /// `self + o`, over the shared denominator `self.den · o.den`.
    pub fn add(&self, o: &Self) -> Self {
        let den = self.den.mul(&o.den);
        let mk = |i: usize| self.num[i].mul(&o.den).add(&o.num[i].mul(&self.den));
        Vec3Rat {
            num: [mk(0), mk(1), mk(2)],
            den,
        }
    }
    /// `self - o`.
    pub fn sub(&self, o: &Self) -> Self {
        let den = self.den.mul(&o.den);
        let mk = |i: usize| self.num[i].mul(&o.den).sub(&o.num[i].mul(&self.den));
        Vec3Rat {
            num: [mk(0), mk(1), mk(2)],
            den,
        }
    }
    /// Multiply every component by the scalar `s`.
    pub fn scale_rat(&self, s: &Rat<B>) -> Self {
        Vec3Rat {
            num: [
                self.num[0].scale(s),
                self.num[1].scale(s),
                self.num[2].scale(s),
            ],
            den: self.den.clone(),
        }
    }
    /// Multiply every component by the rational function `f`.
    pub fn scale(&self, f: &RatFunc<B>) -> Self {
        Vec3Rat {
            num: [
                self.num[0].mul(f.num()),
                self.num[1].mul(f.num()),
                self.num[2].mul(f.num()),
            ],
            den: self.den.mul(f.den()),
        }
    }

    /// The dot product `self · o`, as a single [`RatFunc`] over `self.den · o.den`.
    pub fn dot(&self, o: &Self) -> RatFunc<B> {
        let num = self.num[0]
            .mul(&o.num[0])
            .add(&self.num[1].mul(&o.num[1]))
            .add(&self.num[2].mul(&o.num[2]));
        RatFunc::new(num, self.den.mul(&o.den))
    }
    /// The cross product `self × o`, over the shared denominator `self.den · o.den`.
    pub fn cross(&self, o: &Self) -> Self {
        let (a, b) = (&self.num, &o.num);
        Vec3Rat {
            num: [
                a[1].mul(&b[2]).sub(&a[2].mul(&b[1])),
                a[2].mul(&b[0]).sub(&a[0].mul(&b[2])),
                a[0].mul(&b[1]).sub(&a[1].mul(&b[0])),
            ],
            den: self.den.mul(&o.den),
        }
    }

    /// The component-wise derivative, by the quotient rule over the shared denominator:
    /// `(N′·d − N·d′) / d²`.
    pub fn derivative(&self) -> Self {
        let d = &self.den;
        let dp = d.derivative();
        let mk = |i: usize| self.num[i].derivative().mul(d).sub(&self.num[i].mul(&dp));
        Vec3Rat {
            num: [mk(0), mk(1), mk(2)],
            den: d.mul(d),
        }
    }

    /// Evaluate at the rational point `x`, or `None` where the denominator vanishes.
    pub fn eval(&self, x: &Rat<B>) -> Option<[Rat<B>; 3]> {
        let d = self.den.eval(x);
        if d.is_zero() {
            return None;
        }
        Some([
            self.num[0].eval(x).div(&d),
            self.num[1].eval(x).div(&d),
            self.num[2].eval(x).div(&d),
        ])
    }
}

impl<B: Backend> Clone for RatFunc<B> {
    fn clone(&self) -> Self {
        RatFunc {
            num: self.num.clone(),
            den: self.den.clone(),
        }
    }
}
impl<B: Backend> PartialEq for RatFunc<B> {
    /// Value equality by cross-multiplication (`na·db == nb·da`), so unreduced and
    /// reduced forms of the same function are equal.
    fn eq(&self, o: &Self) -> bool {
        self.num.mul(&o.den) == o.num.mul(&self.den)
    }
}
impl<B: Backend> Eq for RatFunc<B> {}
impl<B: Backend> core::fmt::Debug for RatFunc<B> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "RatFunc({:?}/{:?})", self.num, self.den)
    }
}

impl<B: Backend> Clone for Vec3Rat<B> {
    fn clone(&self) -> Self {
        Vec3Rat {
            num: [
                self.num[0].clone(),
                self.num[1].clone(),
                self.num[2].clone(),
            ],
            den: self.den.clone(),
        }
    }
}
impl<B: Backend> PartialEq for Vec3Rat<B> {
    /// Value equality by per-component cross-multiplication.
    fn eq(&self, o: &Self) -> bool {
        (0..3).all(|i| self.num[i].mul(&o.den) == o.num[i].mul(&self.den))
    }
}
impl<B: Backend> Eq for Vec3Rat<B> {}
impl<B: Backend> core::fmt::Debug for Vec3Rat<B> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Vec3Rat({:?},{:?},{:?} / {:?})",
            self.num[0], self.num[1], self.num[2], self.den
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Q = Rat<Bignum>;
    fn p(cs: &[i128]) -> Poly<Bignum> {
        Poly::from_coeffs(cs.iter().map(|&c| Q::from_i128(c)).collect())
    }
    fn rf(num: &[i128], den: &[i128]) -> RatFunc<Bignum> {
        RatFunc::new(p(num), p(den))
    }

    #[test]
    fn eval_and_zero_denominator() {
        // f = (2x) / (x − 1)
        let f = rf(&[0, 2], &[-1, 1]);
        assert_eq!(f.eval(&Q::from_i128(3)), Some(Q::from_i128(3))); // 6/2
        assert_eq!(f.eval(&Q::from_i128(1)), None); // denominator vanishes
        assert!(RatFunc::<Bignum>::zero().is_zero());
    }

    #[test]
    fn div_and_recip() {
        let a = rf(&[0, 2], &[-1, 1]); // 2x/(x−1)
        let b = rf(&[0, 1], &[1]); // x
        // a/b = 2x/(x(x−1)); at x = 2 that is 2.
        assert_eq!(a.div(&b).eval(&Q::from_i128(2)), Some(Q::from_i128(2)));
        assert_eq!(a.div(&b), a.mul(&b.recip())); // a/b = a·(1/b)
        // recip of x/(x−1) is (x−1)/x; at x = 2 that is 1/2.
        assert_eq!(
            rf(&[0, 1], &[-1, 1]).recip().eval(&Q::from_i128(2)),
            Some(Q::new(1, 2))
        );
    }

    #[test]
    fn ring_axioms() {
        let a = rf(&[1, 1], &[0, 1]); // (1+x)/x
        let b = rf(&[2], &[1, 1]); // 2/(1+x)
        // commutativity + distributivity, checked as value equality (cross-multiplied)
        assert_eq!(a.add(&b), b.add(&a));
        assert_eq!(a.mul(&b), b.mul(&a));
        assert_eq!(a.mul(&a.add(&b)), a.mul(&a).add(&a.mul(&b)));
        // a − a = 0, a + 0 = a
        assert!(a.sub(&a).is_zero());
        assert_eq!(a.add(&RatFunc::zero()), a);
        assert_eq!(a.mul(&RatFunc::one()), a);
    }

    #[test]
    fn derivative_quotient_rule() {
        // d/dx [ x / (x+1) ] = 1/(x+1)²; check by value at several points.
        let f = rf(&[0, 1], &[1, 1]);
        let df = f.derivative();
        for x in [-3i128, 0, 2, 5] {
            let want = Q::from_i128(1).div(
                &p(&[1, 1])
                    .eval(&Q::from_i128(x))
                    .mul(&p(&[1, 1]).eval(&Q::from_i128(x))),
            );
            assert_eq!(df.eval(&Q::from_i128(x)), Some(want));
        }
    }

    #[test]
    fn reduce_is_canonical_and_value_preserving() {
        // (x² − 1)/(x − 1) reduces to (x + 1)/1.
        let f = rf(&[-1, 0, 1], &[-1, 1]);
        let r = f.reduce();
        assert_eq!(r.den(), &p(&[1])); // denominator cancelled to a constant
        assert_eq!(&r, &f); // value preserved
        // negative-leading denominator is flipped positive
        let g = rf(&[1], &[0, -1]); // 1/(−x)
        let rg = g.reduce();
        assert_eq!(rg.den().leading().map(Rat::sign), Some(1));
        assert_eq!(&rg, &g);
    }

    #[test]
    fn vec3_dot_cross_derivative() {
        // a = (x, 1, 0), b = (0, x, 1) as polynomial vectors (denominator 1).
        let a = Vec3Rat::from_polys([p(&[0, 1]), p(&[1]), p(&[0])]);
        let b = Vec3Rat::from_polys([p(&[0]), p(&[0, 1]), p(&[1])]);
        // a·b = 0·x + 1·x + 0·1 = x
        assert_eq!(a.dot(&b), RatFunc::from_poly(p(&[0, 1])));
        // a×b = (1·1 − 0·x, 0·0 − x·1, x·x − 1·0) = (1, −x, x²)
        let c = a.cross(&b);
        assert_eq!(
            c,
            Vec3Rat::from_polys([p(&[1]), p(&[0, -1]), p(&[0, 0, 1])])
        );
        // cross is anti-symmetric
        assert_eq!(
            b.cross(&a),
            Vec3Rat::from_polys([p(&[-1]), p(&[0, 1]), p(&[0, 0, -1])])
        );
        // derivative of (x, 1, 0) is (1, 0, 0)
        assert_eq!(
            a.derivative(),
            Vec3Rat::from_polys([p(&[1]), p(&[0]), p(&[0])])
        );
    }

    #[test]
    fn vec3_common_denominator_scale() {
        // n = (x, 1, 0)/(x²+1); scaling by the RatFunc (x²+1) clears the denominator.
        let n = Vec3Rat::new([p(&[0, 1]), p(&[1]), p(&[0])], p(&[1, 0, 1]));
        let cleared = n.scale(&RatFunc::from_poly(p(&[1, 0, 1])));
        assert_eq!(cleared, Vec3Rat::from_polys([p(&[0, 1]), p(&[1]), p(&[0])]));
        assert_eq!(n.comp(0).eval(&Q::from_i128(1)), Some(Q::new(1, 2))); // x/(x²+1) at 1
    }
}
