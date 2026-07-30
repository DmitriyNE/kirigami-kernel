//! The two-tier exact `Int` / `Rat` (spec invariant 5): an L0 `i128` fast path
//! (`small`) over the BigInt slow path (`Backend`), promoting on overflow and
//! demoting back when a result fits `i128` again.
//!
//! **Soundness-first invariant:** `Eq`/`Ord` compare **by value, not by tier** —
//! a `Fast(1/2)` and a `Slow` holding `1/2` are equal. Canonicalization ("live in
//! `Fast` iff the reduced value fits `i128`") is therefore a *performance /
//! consistency* property (checked by the differential test + Kani), **not** a
//! soundness dependency: a single op that forgot to demote cannot make `==` lie.
//!
//! Rational `cmp`/`sign` are **total and exact** — never `Unresolved`. So is the
//! algebraic (√-carrying) comparison (`algebraic::{Surd, AlgReal}`, spec §8.1
//! "A/1D inequalities: total"): the three-valued `Unresolved` middle is
//! certify-core's A/nD strict-sign concern, not any lattice comparison.

use crate::backend::Backend;
use crate::bignum::Bignum;
use crate::small::{self, SmallRat};
use core::cmp::Ordering;
use core::fmt;

// ===========================================================================
// Int — two-tier integer
// ===========================================================================

/// Exact integer: `i128` fast path over the backend BigInt slow path.
pub enum Int<B: Backend = Bignum> {
    Fast(i128),
    Slow(B::Int),
}

fn to_slow_int<B: Backend>(a: &Int<B>) -> B::Int {
    match a {
        Int::Fast(x) => B::int_from_i128(*x),
        Int::Slow(i) => i.clone(),
    }
}

/// Pull a backend integer back to `Fast` when it fits `i128` (canonicalization).
fn demote_int<B: Backend>(i: B::Int) -> Int<B> {
    match B::int_try_to_i128(&i) {
        Some(x) => Int::Fast(x),
        None => Int::Slow(i),
    }
}

impl<B: Backend> Int<B> {
    pub fn from_i128(v: i128) -> Self {
        Int::Fast(v)
    }
    pub fn zero() -> Self {
        Int::Fast(0)
    }
    pub fn one() -> Self {
        Int::Fast(1)
    }

    pub fn add(&self, o: &Self) -> Self {
        if let (Int::Fast(a), Int::Fast(b)) = (self, o) {
            if let Some(r) = a.checked_add(*b) {
                return Int::Fast(r);
            }
        }
        demote_int::<B>(B::int_add(&to_slow_int(self), &to_slow_int(o)))
    }
    pub fn sub(&self, o: &Self) -> Self {
        if let (Int::Fast(a), Int::Fast(b)) = (self, o) {
            if let Some(r) = a.checked_sub(*b) {
                return Int::Fast(r);
            }
        }
        demote_int::<B>(B::int_sub(&to_slow_int(self), &to_slow_int(o)))
    }
    pub fn mul(&self, o: &Self) -> Self {
        if let (Int::Fast(a), Int::Fast(b)) = (self, o) {
            if let Some(r) = a.checked_mul(*b) {
                return Int::Fast(r);
            }
        }
        demote_int::<B>(B::int_mul(&to_slow_int(self), &to_slow_int(o)))
    }
    pub fn neg(&self) -> Self {
        if let Int::Fast(a) = self {
            if let Some(r) = a.checked_neg() {
                return Int::Fast(r);
            }
        }
        demote_int::<B>(B::int_neg(&to_slow_int(self)))
    }
    /// `-1 | 0 | 1`.
    pub fn sign(&self) -> i8 {
        match self {
            Int::Fast(a) => a.signum() as i8,
            Int::Slow(i) => B::int_sign(i),
        }
    }
    pub fn is_zero(&self) -> bool {
        match self {
            Int::Fast(a) => *a == 0,
            Int::Slow(i) => B::int_is_zero(i),
        }
    }
    /// gcd, always `≥ 0`.
    pub fn gcd(&self, o: &Self) -> Self {
        if let (Int::Fast(a), Int::Fast(b)) = (self, o) {
            if let Some(g) = small::i128_gcd(*a, *b) {
                return Int::Fast(g);
            }
        }
        demote_int::<B>(B::int_gcd(&to_slow_int(self), &to_slow_int(o)))
    }
    /// lcm, always `≥ 0`.
    pub fn lcm(&self, o: &Self) -> Self {
        if let (Int::Fast(a), Int::Fast(b)) = (self, o) {
            if *a == 0 || *b == 0 {
                return Int::Fast(0);
            }
            if let Some(g) = small::i128_gcd(*a, *b) {
                if let Some(prod) = (a / g).checked_mul(*b) {
                    if let Ok(l) = i128::try_from(prod.unsigned_abs()) {
                        return Int::Fast(l);
                    }
                }
            }
        }
        demote_int::<B>(B::int_lcm(&to_slow_int(self), &to_slow_int(o)))
    }
    /// Truncated quotient and remainder: `self = q·o + r`, `r` the sign of `self`,
    /// `|r| < |o|`. `o != 0` by contract.
    pub fn divrem(&self, o: &Self) -> (Self, Self) {
        if let (Int::Fast(a), Int::Fast(b)) = (self, o) {
            // `i128::MIN / -1` overflows the quotient → fall through to the backend.
            if *b != 0 && !(*a == i128::MIN && *b == -1) {
                return (Int::Fast(a / b), Int::Fast(a % b));
            }
        }
        let (q, r) = B::int_divrem(&to_slow_int(self), &to_slow_int(o));
        (demote_int::<B>(q), demote_int::<B>(r))
    }
    /// Truncated quotient (exact where `o | self`).
    pub fn div(&self, o: &Self) -> Self {
        self.divrem(o).0
    }
    /// Truncated remainder (sign of `self`).
    pub fn rem(&self, o: &Self) -> Self {
        self.divrem(o).1
    }
}

impl<B: Backend> Clone for Int<B> {
    fn clone(&self) -> Self {
        match self {
            Int::Fast(x) => Int::Fast(*x),
            Int::Slow(i) => Int::Slow(i.clone()),
        }
    }
}
impl<B: Backend> PartialEq for Int<B> {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl<B: Backend> Eq for Int<B> {}
impl<B: Backend> PartialOrd for Int<B> {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl<B: Backend> Ord for Int<B> {
    fn cmp(&self, o: &Self) -> Ordering {
        if let (Int::Fast(a), Int::Fast(b)) = (self, o) {
            return a.cmp(b);
        }
        B::int_cmp(&to_slow_int(self), &to_slow_int(o))
    }
}
impl<B: Backend> fmt::Debug for Int<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Int::Fast(x) => write!(f, "Int::Fast({x})"),
            Int::Slow(_) => write!(f, "Int::Slow(..)"),
        }
    }
}

// ===========================================================================
// Rat — two-tier rational
// ===========================================================================

/// Exact rational: L0 `SmallRat` fast path over the backend `Rat` slow path.
/// Always canonical (reduced, `den > 0`).
pub enum Rat<B: Backend = Bignum> {
    Fast(SmallRat),
    Slow(B::Rat),
}

fn promote_rat<B: Backend>(x: &SmallRat) -> B::Rat {
    B::rat_from_ints(B::int_from_i128(x.num), B::int_from_i128(x.den))
}
fn to_slow_rat<B: Backend>(a: &Rat<B>) -> B::Rat {
    match a {
        Rat::Fast(x) => promote_rat::<B>(x),
        Rat::Slow(r) => r.clone(),
    }
}
/// Pull a backend rational (already reduced, `den > 0`) back to `Fast` when both
/// numerator and denominator fit `i128` (canonicalization).
fn demote_rat<B: Backend>(r: B::Rat) -> Rat<B> {
    match (
        B::int_try_to_i128(&B::rat_numer(&r)),
        B::int_try_to_i128(&B::rat_denom(&r)),
    ) {
        (Some(n), Some(d)) => Rat::Fast(SmallRat::from_reduced(n, d)),
        _ => Rat::Slow(r),
    }
}

impl<B: Backend> Rat<B> {
    /// The integer `v` as `v/1`.
    pub fn from_i128(v: i128) -> Self {
        Rat::Fast(SmallRat::int(v))
    }
    /// `num/den`, reduced. `den == 0` is out of contract (debug-assert + a defined
    /// zero fallback; never panics).
    pub fn new(num: i128, den: i128) -> Self {
        if den == 0 {
            debug_assert!(false, "Rat::new: zero denominator");
            return Rat::from_i128(0);
        }
        match SmallRat::reduce(num, den) {
            Some(s) => Rat::Fast(s),
            // reduce only returns None here at the den-magnitude-2^127 edge → promote.
            None => demote_rat::<B>(B::rat_from_ints(
                B::int_from_i128(num),
                B::int_from_i128(den),
            )),
        }
    }

    pub fn add(&self, o: &Self) -> Self {
        if let (Rat::Fast(x), Rat::Fast(y)) = (self, o) {
            if let Some(r) = small::add(x, y) {
                return Rat::Fast(r);
            }
        }
        demote_rat::<B>(B::rat_add(&to_slow_rat(self), &to_slow_rat(o)))
    }
    pub fn sub(&self, o: &Self) -> Self {
        if let (Rat::Fast(x), Rat::Fast(y)) = (self, o) {
            if let Some(r) = small::sub(x, y) {
                return Rat::Fast(r);
            }
        }
        demote_rat::<B>(B::rat_sub(&to_slow_rat(self), &to_slow_rat(o)))
    }
    pub fn mul(&self, o: &Self) -> Self {
        if let (Rat::Fast(x), Rat::Fast(y)) = (self, o) {
            if let Some(r) = small::mul(x, y) {
                return Rat::Fast(r);
            }
        }
        demote_rat::<B>(B::rat_mul(&to_slow_rat(self), &to_slow_rat(o)))
    }
    /// Exact division `self / o`. `o != 0` by contract.
    pub fn div(&self, o: &Self) -> Self {
        if let (Rat::Fast(x), Rat::Fast(y)) = (self, o) {
            if let Some(r) = small::div(x, y) {
                return Rat::Fast(r);
            }
        }
        demote_rat::<B>(B::rat_div(&to_slow_rat(self), &to_slow_rat(o)))
    }
    /// Reciprocal `1 / self`. `self != 0` by contract.
    pub fn recip(&self) -> Self {
        if let Rat::Fast(x) = self {
            if let Some(r) = small::recip(x) {
                return Rat::Fast(r);
            }
        }
        demote_rat::<B>(B::rat_div(&B::rat_from_i128(1), &to_slow_rat(self)))
    }
    pub fn neg(&self) -> Self {
        if let Rat::Fast(x) = self {
            if let Some(r) = small::neg(x) {
                return Rat::Fast(r);
            }
        }
        demote_rat::<B>(B::rat_neg(&to_slow_rat(self)))
    }
    /// `-1 | 0 | 1`.
    pub fn sign(&self) -> i8 {
        match self {
            Rat::Fast(x) => small::sign(x),
            Rat::Slow(r) => B::rat_sign(r),
        }
    }
    pub fn is_zero(&self) -> bool {
        match self {
            Rat::Fast(x) => x.num == 0,
            Rat::Slow(r) => B::rat_is_zero(r),
        }
    }
}

impl<B: Backend> Clone for Rat<B> {
    fn clone(&self) -> Self {
        match self {
            Rat::Fast(x) => Rat::Fast(*x),
            Rat::Slow(r) => Rat::Slow(r.clone()),
        }
    }
}
impl<B: Backend> PartialEq for Rat<B> {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl<B: Backend> Eq for Rat<B> {}
impl<B: Backend> PartialOrd for Rat<B> {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl<B: Backend> Ord for Rat<B> {
    fn cmp(&self, o: &Self) -> Ordering {
        if let (Rat::Fast(x), Rat::Fast(y)) = (self, o) {
            if let Some(ord) = small::cmp(x, y) {
                return ord;
            }
        }
        B::rat_cmp(&to_slow_rat(self), &to_slow_rat(o))
    }
}
impl<B: Backend> fmt::Debug for Rat<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Rat::Fast(s) => write!(f, "Rat::Fast({}/{})", s.num, s.den),
            Rat::Slow(_) => write!(f, "Rat::Slow(..)"),
        }
    }
}

// ===========================================================================
// Tests — corpus seeds (exact cmp/sign on the fast path) + promotion/demotion
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    type I = Int<Bignum>;
    type Q = Rat<Bignum>;

    // cx-parallel-distinct-lines: L_A=(1,0,0), L_B=(1,0,−1). The three 2×2
    // triple-minors → {0, −1, 0}; PARALLEL (direction cross = 0) ∧ ¬COINCIDENT
    // (not all minors zero). Exercises Int mul/sub/sign/is_zero. (v0.21 18.7)
    #[test]
    fn cx_parallel_distinct_lines() {
        let (aa, ba, ca) = (I::from_i128(1), I::from_i128(0), I::from_i128(0));
        let (ab, bb, cb) = (I::from_i128(1), I::from_i128(0), I::from_i128(-1));
        let m_ab = aa.mul(&bb).sub(&ab.mul(&ba)); // 1·0 − 1·0 = 0
        let m_ac = aa.mul(&cb).sub(&ab.mul(&ca)); // 1·(−1) − 1·0 = −1
        let m_bc = ba.mul(&cb).sub(&bb.mul(&ca)); // 0·(−1) − 0·0 = 0
        assert_eq!((m_ab.sign(), m_ac.sign(), m_bc.sign()), (0, -1, 0));
        assert!(m_ab.is_zero(), "direction cross = 0 ⇒ PARALLEL");
        assert!(!m_ac.is_zero(), "some triple-minor ≠ 0 ⇒ ¬COINCIDENT");
    }

    // cx-coincident-vs-tangent-circles: d=0, r₁=r₂. The internal-tangency
    // identity d² = (r₁−r₂)² holds as an EXACT squared-form equality (0 = 0).
    // Exercises Rat sub/mul/cmp/is_zero. (v0.20 17.3)
    #[test]
    fn cx_coincident_vs_tangent_circles() {
        let d = Q::from_i128(0);
        let r1 = Q::new(5, 2);
        let r2 = Q::new(5, 2);
        let d2 = d.mul(&d);
        let diff = r1.sub(&r2);
        let diff2 = diff.mul(&diff);
        assert_eq!(d2.cmp(&diff2), Ordering::Equal);
        assert!(d2.is_zero() && diff2.is_zero());
    }

    // cx-diag-sylvester: JᵀJ − mI = diag(1,0,−1), m=2. Leading principal minors
    // {1,0,0}; strict-Sylvester (all > 0) FAILS at the second minor, and
    // σ²_min = 1 < 2 = m. Exercises Int mul/sign and Rat cmp. (v0.16 15.4)
    #[test]
    fn cx_diag_sylvester() {
        let (d1, d2, d3) = (I::from_i128(1), I::from_i128(0), I::from_i128(-1));
        let m1 = d1.clone(); // D₁ = 1
        let m2 = d1.mul(&d2); // D₂ = 1·0 = 0
        let m3 = m2.mul(&d3); // D₃ = 0·(−1) = 0
        assert_eq!((m1.sign(), m2.sign(), m3.sign()), (1, 0, 0));
        let strict_pd = m1.sign() > 0 && m2.sign() > 0 && m3.sign() > 0;
        assert!(!strict_pd, "strict Sylvester must fail on the zero minor");
        // σ²_min = 1 < 2 = m
        assert_eq!(Q::from_i128(1).cmp(&Q::from_i128(2)), Ordering::Less);
    }

    // Fast path stays fast, with exact reduction (1/2 + 1/2 = 1, 2/4 = 1/2).
    #[test]
    fn fast_path_reduces() {
        let half = Q::new(2, 4);
        assert!(matches!(half, Rat::Fast(SmallRat { num: 1, den: 2 })));
        let one = half.add(&Q::new(1, 2));
        assert!(matches!(one, Rat::Fast(SmallRat { num: 1, den: 1 })));
        assert_eq!(one.cmp(&Q::from_i128(1)), Ordering::Equal);
    }

    // Promotion: coprime near-i128::MAX denominators overflow the common
    // denominator → the result lives in Slow, still exact.
    #[test]
    fn promotes_on_overflow() {
        let a = Q::new(1, i128::MAX);
        let b = Q::new(1, i128::MAX - 2); // gcd(MAX, MAX−2) = 1 (both odd)
        let s = a.sub(&b); // 1/MAX − 1/(MAX−2) < 0, lcm overflows i128
        assert!(matches!(s, Rat::Slow(_)), "must promote to the slow path");
        assert_eq!(s.sign(), -1);
        // value-based Eq across tiers: a Slow negative is < a Fast zero.
        assert_eq!(s.cmp(&Q::from_i128(0)), Ordering::Less);
    }

    // Demotion: a Slow value whose result fits i128 again returns to Fast
    // (canonicalization). Slow − Slow = 0 ⇒ Fast(0).
    #[test]
    fn demotes_when_fits() {
        let big = Q::new(1, i128::MAX).sub(&Q::new(1, i128::MAX - 2)); // Slow
        assert!(matches!(big, Rat::Slow(_)));
        let zero = big.sub(&big);
        assert!(matches!(zero, Rat::Fast(_)), "0 must demote back to Fast");
        assert!(zero.is_zero());
    }

    // Int overflow promotes too; gcd of huge multiples is exact.
    #[test]
    fn int_promotes_and_gcd() {
        let big = I::from_i128(i128::MAX).mul(&I::from_i128(4)); // overflows → Slow
        assert!(matches!(big, Int::Slow(_)));
        // gcd(4·MAX, 2·MAX) = 2·MAX (also Slow), > 0
        let big2 = I::from_i128(i128::MAX).mul(&I::from_i128(2));
        let g = big.gcd(&big2);
        assert_eq!(g.sign(), 1);
        assert_eq!(g, big2, "gcd(4·MAX, 2·MAX) = 2·MAX");
    }

    // Exhaustive low-magnitude sweep of the L0 fast path: for every rational pair
    // with |num|,|den| ≤ 24 (den ≠ 0), add/sub/mul return a reduced Some equal to
    // the i128 cross-multiplied reference, and cmp matches. This is the ground
    // truth the Kani i16 harness proves symbolically — run natively (seconds) it
    // is a dense complement to the differential's random + boundary sweep.
    #[test]
    fn fast_path_small_grid_exhaustive() {
        use crate::small::{self, SmallRat};
        fn coprime(a: i128, b: i128) -> bool {
            let (mut a, mut b) = (a.unsigned_abs(), b.unsigned_abs());
            while b != 0 {
                let t = a % b;
                a = b;
                b = t;
            }
            a == 1
        }
        let g = 24i128;
        for xn in -g..=g {
            for xd in (-g..=g).filter(|d| *d != 0) {
                let x = SmallRat::reduce(xn, xd).unwrap();
                for yn in -g..=g {
                    for yd in (-g..=g).filter(|d| *d != 0) {
                        let y = SmallRat::reduce(yn, yd).unwrap();
                        let dd = x.den * y.den;
                        let r = small::add(&x, &y).unwrap();
                        assert!(r.den > 0 && coprime(r.num, r.den));
                        assert_eq!(r.num * dd, (x.num * y.den + y.num * x.den) * r.den);
                        let r = small::sub(&x, &y).unwrap();
                        assert_eq!(r.num * dd, (x.num * y.den - y.num * x.den) * r.den);
                        let r = small::mul(&x, &y).unwrap();
                        assert_eq!(r.num * dd, (x.num * y.num) * r.den);
                        assert_eq!(
                            small::cmp(&x, &y).unwrap(),
                            (x.num * y.den).cmp(&(y.num * x.den))
                        );
                        if y.num != 0 {
                            let r = small::div(&x, &y).unwrap();
                            assert!(r.den > 0 && coprime(r.num, r.den));
                            assert_eq!(r.num * (x.den * y.num), (x.num * y.den) * r.den);
                        }
                    }
                }
            }
        }
    }
}

// ===========================================================================
// Runtime differential (vv-guide §3) — dev-only, proptest-driven. Two properties
// over boundary-weighted inputs: fast ≡ slow (same backend: value AND tier) and
// dashu ≡ num (an INDEPENDENT second backend, compared as reduced decimal
// num/den). Covers the full i128 range against the real BigInt path — where the
// fast-path Kani harness (Step 7) cannot reach — plus the vv-matrix
// "differential (2nd backend)" row. In-crate so it can force the slow tier and
// read the backend's numerator/denominator.
// ===========================================================================

#[cfg(test)]
mod differential {
    use super::*;
    use alloc::string::{String, ToString};
    use num_bigint::BigInt as NInt;
    use num_rational::BigRational as NRat;
    use proptest::prelude::*;

    type Q = Rat<Bignum>;

    /// A lattice rational as its reduced (numerator, denominator) decimal strings
    /// (via the dashu backend) — the cross-backend canonical form.
    fn dashu_canon(q: &Q) -> (String, String) {
        let b = to_slow_rat(q);
        (
            Bignum::rat_numer(&b).0.to_string(),
            Bignum::rat_denom(&b).0.to_string(),
        )
    }
    fn num_of(n: i128, d: i128) -> NRat {
        NRat::new(NInt::from(n), NInt::from(d))
    }
    fn num_canon(r: &NRat) -> (String, String) {
        (r.numer().to_string(), r.denom().to_string())
    }

    /// Boundary-weighted i128: tiny, the i128 extremes, near-extreme, i64-range,
    /// full-range, and ± large powers of two — where the tier transitions live.
    fn coord() -> impl Strategy<Value = i128> {
        prop_oneof![
            (-8i128..=8),
            Just(0i128),
            Just(i128::MIN),
            Just(i128::MAX),
            (i128::MAX - 8..=i128::MAX),
            (i128::MIN..=i128::MIN + 8),
            any::<i64>().prop_map(|x| x as i128),
            any::<i128>(),
            (0u32..127).prop_map(|k| 1i128 << k),
            (0u32..127).prop_map(|k| -(1i128 << k)),
        ]
    }
    fn nz() -> impl Strategy<Value = i128> {
        coord().prop_filter("denominator != 0", |d| *d != 0)
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4096))]

        /// fast ≡ slow (same backend): every op's fast-path result equals the same
        /// op forced through the slow path — value AND tier (canonicalization).
        #[test]
        fn fast_matches_slow(n1 in coord(), d1 in nz(), n2 in coord(), d2 in nz()) {
            let q1 = Q::new(n1, d1);
            let q2 = Q::new(n2, d2);
            // Force the slow tier by wrapping the materialized backend value.
            let s1 = Rat::Slow(to_slow_rat(&q1));
            let s2 = Rat::Slow(to_slow_rat(&q2));
            for (fast, slow) in [
                (q1.add(&q2), s1.add(&s2)),
                (q1.sub(&q2), s1.sub(&s2)),
                (q1.mul(&q2), s1.mul(&s2)),
                (q1.neg(), s1.neg()),
            ] {
                prop_assert_eq!(&fast, &slow);
                // canonicalization: equal value ⇒ equal tier
                prop_assert_eq!(matches!(fast, Rat::Fast(_)), matches!(slow, Rat::Fast(_)));
            }
            prop_assert_eq!(q1.cmp(&q2), s1.cmp(&s2));
            prop_assert_eq!(q1.sign(), s1.sign());
            if n2 != 0 {
                let (f, s) = (q1.div(&q2), s1.div(&s2));
                prop_assert_eq!(&f, &s);
                prop_assert_eq!(matches!(f, Rat::Fast(_)), matches!(s, Rat::Fast(_)));
            }
            if n1 != 0 {
                prop_assert_eq!(q1.recip(), s1.recip());
            }
        }

        /// dashu ≡ num (an independent second backend), compared as reduced
        /// decimal num/den.
        #[test]
        fn dashu_matches_num(n1 in coord(), d1 in nz(), n2 in coord(), d2 in nz()) {
            let q1 = Q::new(n1, d1);
            let q2 = Q::new(n2, d2);
            let m1 = num_of(n1, d1);
            let m2 = num_of(n2, d2);
            prop_assert_eq!(dashu_canon(&q1.add(&q2)), num_canon(&(&m1 + &m2)));
            prop_assert_eq!(dashu_canon(&q1.sub(&q2)), num_canon(&(&m1 - &m2)));
            prop_assert_eq!(dashu_canon(&q1.mul(&q2)), num_canon(&(&m1 * &m2)));
            prop_assert_eq!(dashu_canon(&q1.neg()), num_canon(&(-&m1)));
            prop_assert_eq!(q1.cmp(&q2), m1.cmp(&m2));
            if n2 != 0 {
                prop_assert_eq!(dashu_canon(&q1.div(&q2)), num_canon(&(&m1 / &m2)));
            }
            if n1 != 0 {
                prop_assert_eq!(dashu_canon(&q1.recip()), num_canon(&m1.recip()));
            }
        }
    }
}
