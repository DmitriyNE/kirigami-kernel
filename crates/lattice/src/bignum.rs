//! The concrete bignum backend: dashu (`docs/lattice-backend-benchmark.md`).
//!
//! This is the **slow path** and the **semantic reference** — its results are
//! the definition of correctness the L0 fast path is checked against (Kani +
//! differential), and they match Lean's `Int`/`Rat` (`vv-guide §4`). `IBig`/`RBig`
//! are wrapped in newtypes so no dashu type leaks past this module and the
//! `no_std + alloc` boundary is the [`Backend`] API, not the backend crate.

use crate::backend::Backend;
use core::cmp::Ordering;
use dashu::integer::{IBig, UBig};
use dashu::rational::RBig;

/// Arbitrary-precision integer (reduced-form invariant is trivial for integers).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BigInt(pub(crate) IBig);

/// Arbitrary-precision rational, always reduced with denominator `> 0`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BigRat(pub(crate) RBig);

/// The default backend (`Backend::Int = BigInt`, `Backend::Rat = BigRat`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Bignum;

fn sign_of(o: Ordering) -> i8 {
    match o {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

impl Backend for Bignum {
    type Int = BigInt;
    type Rat = BigRat;

    fn int_zero() -> BigInt {
        BigInt(IBig::ZERO)
    }
    fn int_one() -> BigInt {
        BigInt(IBig::ONE)
    }
    fn int_from_i128(v: i128) -> BigInt {
        BigInt(IBig::from(v))
    }
    fn int_try_to_i128(a: &BigInt) -> Option<i128> {
        i128::try_from(a.0.clone()).ok()
    }

    fn int_add(a: &BigInt, b: &BigInt) -> BigInt {
        BigInt(&a.0 + &b.0)
    }
    fn int_sub(a: &BigInt, b: &BigInt) -> BigInt {
        BigInt(&a.0 - &b.0)
    }
    fn int_mul(a: &BigInt, b: &BigInt) -> BigInt {
        BigInt(&a.0 * &b.0)
    }
    fn int_neg(a: &BigInt) -> BigInt {
        BigInt(-a.0.clone())
    }
    fn int_cmp(a: &BigInt, b: &BigInt) -> Ordering {
        a.0.cmp(&b.0)
    }
    fn int_sign(a: &BigInt) -> i8 {
        sign_of(a.0.cmp(&IBig::ZERO))
    }
    fn int_is_zero(a: &BigInt) -> bool {
        a.0 == IBig::ZERO
    }
    fn int_gcd(a: &BigInt, b: &BigInt) -> BigInt {
        // Euclid on magnitudes (result ≥ 0). Avoids depending on a backend gcd API.
        let mut x = a.0.clone();
        let mut y = b.0.clone();
        if x < IBig::ZERO {
            x = -x;
        }
        if y < IBig::ZERO {
            y = -y;
        }
        while y != IBig::ZERO {
            let r = &x % &y;
            x = y;
            y = r;
        }
        BigInt(x)
    }
    fn int_lcm(a: &BigInt, b: &BigInt) -> BigInt {
        if a.0 == IBig::ZERO || b.0 == IBig::ZERO {
            return BigInt(IBig::ZERO);
        }
        let g = Self::int_gcd(a, b).0; // > 0
        let mut l = (&a.0 * &b.0) / &g; // exact
        if l < IBig::ZERO {
            l = -l;
        }
        BigInt(l)
    }
    fn int_divrem(a: &BigInt, b: &BigInt) -> (BigInt, BigInt) {
        if b.0 == IBig::ZERO {
            debug_assert!(false, "int_divrem: division by zero (out of contract)");
            return (BigInt(IBig::ZERO), a.clone());
        }
        let q = &a.0 / &b.0; // dashu IBig `/` truncates toward zero (matches i128)
        let r = &a.0 - &q * &b.0;
        (BigInt(q), BigInt(r))
    }

    fn rat_from_i128(v: i128) -> BigRat {
        BigRat(RBig::from_parts(IBig::from(v), UBig::ONE))
    }
    fn rat_from_ints(num: BigInt, den: BigInt) -> BigRat {
        if den.0 == IBig::ZERO {
            debug_assert!(
                false,
                "rat_from_ints: zero denominator (out of §2.2 contract)"
            );
            return BigRat(RBig::ZERO);
        }
        // Move the denominator's sign into the numerator so the denominator is > 0.
        let den_neg = den.0 < IBig::ZERO;
        let n = if den_neg { -num.0 } else { num.0 };
        let (_sign, d_mag) = den.0.into_parts(); // magnitude UBig (> 0)
        BigRat(RBig::from_parts(n, d_mag)) // from_parts reduces to lowest terms
    }

    fn rat_add(a: &BigRat, b: &BigRat) -> BigRat {
        BigRat(&a.0 + &b.0)
    }
    fn rat_sub(a: &BigRat, b: &BigRat) -> BigRat {
        BigRat(&a.0 - &b.0)
    }
    fn rat_mul(a: &BigRat, b: &BigRat) -> BigRat {
        BigRat(&a.0 * &b.0)
    }
    fn rat_div(a: &BigRat, b: &BigRat) -> BigRat {
        if b.0 == RBig::ZERO {
            debug_assert!(false, "rat_div: division by zero (out of contract)");
            return BigRat(RBig::ZERO);
        }
        BigRat(&a.0 / &b.0)
    }
    fn rat_neg(a: &BigRat) -> BigRat {
        BigRat(-a.0.clone())
    }
    fn rat_cmp(a: &BigRat, b: &BigRat) -> Ordering {
        a.0.cmp(&b.0)
    }
    fn rat_sign(a: &BigRat) -> i8 {
        sign_of(a.0.cmp(&RBig::ZERO))
    }
    fn rat_is_zero(a: &BigRat) -> bool {
        a.0 == RBig::ZERO
    }
    fn rat_numer(a: &BigRat) -> BigInt {
        BigInt(a.0.numerator().clone())
    }
    fn rat_denom(a: &BigRat) -> BigInt {
        BigInt(IBig::from(a.0.denominator().clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2^200 — well beyond i128 (max ~2^127). Exercises the slow path directly.
    fn two_pow_200() -> BigInt {
        let p100 = Bignum::int_from_i128(1i128 << 100); // 2^100 fits i128
        Bignum::int_mul(&p100, &p100) // 2^200
    }

    #[test]
    fn beyond_i128_narrowing() {
        let big = two_pow_200();
        assert_eq!(Bignum::int_try_to_i128(&big), None);
        let small = Bignum::int_from_i128(i128::MAX);
        assert_eq!(Bignum::int_try_to_i128(&small), Some(i128::MAX));
        assert_eq!(
            Bignum::int_try_to_i128(&Bignum::int_from_i128(-7)),
            Some(-7)
        );
    }

    #[test]
    fn gcd_lcm_big() {
        // gcd(2^200, 2^100) = 2^100 ; lcm = 2^200
        let p100 = Bignum::int_from_i128(1i128 << 100);
        let p200 = two_pow_200();
        assert_eq!(Bignum::int_gcd(&p200, &p100), p100);
        assert_eq!(Bignum::int_lcm(&p200, &p100), p200);
        // sign-agnostic, and gcd(0,0)=0, lcm(_,0)=0
        let neg = Bignum::int_neg(&p100);
        assert_eq!(Bignum::int_gcd(&neg, &p200), p100);
        assert_eq!(
            Bignum::int_gcd(&Bignum::int_zero(), &Bignum::int_zero()),
            Bignum::int_zero()
        );
        assert_eq!(
            Bignum::int_lcm(&p200, &Bignum::int_zero()),
            Bignum::int_zero()
        );
    }

    #[test]
    fn rat_normalizes_and_signs() {
        // 6/(-4) → -3/2 (reduced, den > 0)
        let r = Bignum::rat_from_ints(Bignum::int_from_i128(6), Bignum::int_from_i128(-4));
        assert_eq!(Bignum::rat_numer(&r), Bignum::int_from_i128(-3));
        assert_eq!(Bignum::rat_denom(&r), Bignum::int_from_i128(2));
        assert_eq!(Bignum::rat_sign(&r), -1);
        assert!(!Bignum::rat_is_zero(&r));

        // 1/2 + 1/2 == 1 ; cmp is exact and total
        let half = Bignum::rat_from_ints(Bignum::int_one(), Bignum::int_from_i128(2));
        let one = Bignum::rat_add(&half, &half);
        assert_eq!(
            Bignum::rat_cmp(&one, &Bignum::rat_from_i128(1)),
            Ordering::Equal
        );
        assert_eq!(Bignum::rat_cmp(&half, &one), Ordering::Less);
        assert_eq!(Bignum::rat_sign(&Bignum::rat_sub(&half, &half)), 0);
    }

    #[test]
    fn div_and_divrem() {
        // rat_div beyond i128: (2^200 / 3) / 2 == 2^200 / 6, checked by q·b == a.
        let a = Bignum::rat_from_ints(two_pow_200(), Bignum::int_from_i128(3));
        let b = Bignum::rat_from_i128(2);
        let q = Bignum::rat_div(&a, &b);
        assert_eq!(
            Bignum::rat_cmp(&Bignum::rat_mul(&q, &b), &a),
            Ordering::Equal
        );

        // int_divrem truncates toward zero (sign of dividend): −7 / 2 = (−3, −1).
        let (dq, dr) = Bignum::int_divrem(&Bignum::int_from_i128(-7), &Bignum::int_from_i128(2));
        assert_eq!(
            (dq, dr),
            (Bignum::int_from_i128(-3), Bignum::int_from_i128(-1))
        );

        // exact division beyond i128: 2^200 / 2^100 == 2^100, remainder 0.
        let p100 = Bignum::int_from_i128(1i128 << 100);
        let (bq, br) = Bignum::int_divrem(&two_pow_200(), &p100);
        assert_eq!(bq, p100);
        assert!(Bignum::int_is_zero(&br));
    }
}
