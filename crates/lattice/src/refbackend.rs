//! A **reference** exact-arithmetic backend (`RefBackend`) — a second, independent
//! `Backend` implementation over little-endian `Vec<u64>` limbs, deliberately simple
//! and dashu-free.
//!
//! Purpose (algebra-rehaul R.4, `docs/algebra-trust.md`): shrink the dashu trust.
//! `RefBackend` is (1) an independent cross-check of the concrete [`crate::Bignum`]
//! backend — the differential in `rat` runs dashu ≡ `RefBackend` over the full i128
//! range — and (2) written in a safe, allocation-explicit, index-loop style so it can
//! be lifted by Aeneas and proved `= ℤ`/`ℚ` in Lean (a *proven* oracle, no trusted
//! hand-model), unlike the closed-source dashu. Correctness over speed: magnitude
//! division is bit-serial long division. Never the hot path — the two-tier `Rat`/`Int`
//! keep [`crate::Bignum`] as the default slow tier.

use crate::backend::Backend;
use alloc::vec;
use alloc::vec::Vec;
use core::cmp::Ordering;

// ===========================================================================
// RefNat — a nonnegative magnitude as little-endian base-2^64 limbs.
// Invariant (normalized): no trailing zero limb; zero is the empty limb vector.
// ===========================================================================

/// A nonnegative arbitrary-precision magnitude (little-endian `u64` limbs, normalized).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RefNat {
    limbs: Vec<u64>,
}

fn normalize(limbs: &mut Vec<u64>) {
    while !limbs.is_empty() && limbs[limbs.len() - 1] == 0 {
        limbs.pop();
    }
}

impl RefNat {
    fn zero() -> RefNat {
        RefNat { limbs: Vec::new() }
    }
    fn from_u128(v: u128) -> RefNat {
        let lo = v as u64;
        let hi = (v >> 64) as u64;
        let mut limbs = vec![lo, hi];
        normalize(&mut limbs);
        RefNat { limbs }
    }
    fn is_zero(&self) -> bool {
        self.limbs.is_empty()
    }

    /// Compare magnitudes (normalized): by limb count, then most-significant limb down.
    fn cmp(&self, o: &RefNat) -> Ordering {
        if self.limbs.len() != o.limbs.len() {
            return self.limbs.len().cmp(&o.limbs.len());
        }
        let mut i = self.limbs.len();
        while i > 0 {
            i -= 1;
            if self.limbs[i] != o.limbs[i] {
                return self.limbs[i].cmp(&o.limbs[i]);
            }
        }
        Ordering::Equal
    }

    /// `self + o` (schoolbook, `u128` carry).
    fn add(&self, o: &RefNat) -> RefNat {
        // Explicit `if` rather than `usize::max` — the `Ord::max` *default* method does not lift
        // cleanly under the pinned Aeneas (it mis-applies `Ord::max.default` to the `Ord` instance).
        // Same liftability-driven shaping as the R.3a `Backend` refactor; see docs/engineering-log.md.
        let n = if self.limbs.len() >= o.limbs.len() {
            self.limbs.len()
        } else {
            o.limbs.len()
        };
        let mut out = Vec::with_capacity(n + 1);
        let mut carry: u128 = 0;
        let mut i = 0;
        while i < n {
            let a = if i < self.limbs.len() {
                self.limbs[i] as u128
            } else {
                0
            };
            let b = if i < o.limbs.len() {
                o.limbs[i] as u128
            } else {
                0
            };
            let s = a + b + carry;
            out.push(s as u64);
            carry = s >> 64;
            i += 1;
        }
        if carry != 0 {
            out.push(carry as u64);
        }
        normalize(&mut out);
        RefNat { limbs: out }
    }

    /// `self - o`, requires `self ≥ o` (schoolbook borrow).
    fn sub(&self, o: &RefNat) -> RefNat {
        let mut out = Vec::with_capacity(self.limbs.len());
        let mut borrow: i128 = 0;
        let mut i = 0;
        while i < self.limbs.len() {
            let a = self.limbs[i] as i128;
            let b = if i < o.limbs.len() {
                o.limbs[i] as i128
            } else {
                0
            };
            let mut d = a - b - borrow;
            if d < 0 {
                d += 1i128 << 64;
                borrow = 1;
            } else {
                borrow = 0;
            }
            out.push(d as u64);
            i += 1;
        }
        normalize(&mut out);
        RefNat { limbs: out }
    }

    /// `self * o` (schoolbook `O(n·m)`, `u128` products).
    fn mul(&self, o: &RefNat) -> RefNat {
        if self.is_zero() || o.is_zero() {
            return RefNat::zero();
        }
        let mut out = vec![0u64; self.limbs.len() + o.limbs.len()];
        let mut i = 0;
        while i < self.limbs.len() {
            let mut carry: u128 = 0;
            let ai = self.limbs[i] as u128;
            let mut j = 0;
            while j < o.limbs.len() {
                let cur = out[i + j] as u128 + ai * (o.limbs[j] as u128) + carry;
                out[i + j] = cur as u64;
                carry = cur >> 64;
                j += 1;
            }
            // propagate the final carry into higher limbs
            let mut k = i + o.limbs.len();
            while carry != 0 {
                let cur = out[k] as u128 + carry;
                out[k] = cur as u64;
                carry = cur >> 64;
                k += 1;
            }
            i += 1;
        }
        normalize(&mut out);
        RefNat { limbs: out }
    }

    /// Total bit length (0 for zero).
    fn bit_len(&self) -> usize {
        if self.is_zero() {
            return 0;
        }
        let top = self.limbs.len() - 1;
        top * 64 + (64 - self.limbs[top].leading_zeros() as usize)
    }
    /// Bit `i` of the magnitude.
    fn testbit(&self, i: usize) -> bool {
        let limb = i / 64;
        if limb >= self.limbs.len() {
            return false;
        }
        (self.limbs[limb] >> (i % 64)) & 1 == 1
    }
    /// `self << 1`.
    fn shl1(&self) -> RefNat {
        let mut out = Vec::with_capacity(self.limbs.len() + 1);
        let mut carry: u64 = 0;
        let mut i = 0;
        while i < self.limbs.len() {
            let v = self.limbs[i];
            out.push((v << 1) | carry);
            carry = v >> 63;
            i += 1;
        }
        if carry != 0 {
            out.push(carry);
        }
        normalize(&mut out);
        RefNat { limbs: out }
    }

    /// Truncated division: `(q, r)` with `self = q·d + r`, `0 ≤ r < d`. `d ≠ 0`.
    /// Bit-serial long division (correctness-first; magnitudes here are small).
    fn divrem(&self, d: &RefNat) -> (RefNat, RefNat) {
        debug_assert!(!d.is_zero(), "RefNat::divrem by zero");
        if self.cmp(d) == Ordering::Less {
            return (RefNat::zero(), self.clone());
        }
        let n = self.bit_len();
        let mut q = vec![0u64; n.div_ceil(64)];
        let mut r = RefNat::zero();
        let mut i = n;
        while i > 0 {
            i -= 1;
            // r = (r << 1) | bit_i(self)
            r = r.shl1();
            if self.testbit(i) {
                if r.limbs.is_empty() {
                    r.limbs.push(1);
                } else {
                    r.limbs[0] |= 1;
                }
            }
            if r.cmp(d) != Ordering::Less {
                r = r.sub(d);
                q[i / 64] |= 1u64 << (i % 64);
            }
        }
        normalize(&mut q);
        (RefNat { limbs: q }, r)
    }

    /// gcd via Euclid (result `≥ 0`; `gcd(0,0) = 0`).
    fn gcd(&self, o: &RefNat) -> RefNat {
        let mut x = self.clone();
        let mut y = o.clone();
        while !y.is_zero() {
            let (_, r) = x.divrem(&y);
            x = y;
            y = r;
        }
        x
    }

    /// `(self / d, self % d)` for a single-limb `d` (for decimal rendering, tests).
    #[cfg(test)]
    fn div_rem_small(&self, d: u64) -> (RefNat, u64) {
        let mut q = vec![0u64; self.limbs.len()];
        let mut rem: u128 = 0;
        let mut i = self.limbs.len();
        while i > 0 {
            i -= 1;
            let acc = (rem << 64) | (self.limbs[i] as u128);
            q[i] = (acc / d as u128) as u64;
            rem = acc % d as u128;
        }
        normalize(&mut q);
        (RefNat { limbs: q }, rem as u64)
    }
}

// ===========================================================================
// RefInt — sign + magnitude. Zero is canonical (neg = false).
// ===========================================================================

/// Reference arbitrary-precision integer (sign + [`RefNat`] magnitude).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RefInt {
    neg: bool,
    mag: RefNat,
}

impl RefInt {
    fn zero() -> RefInt {
        RefInt {
            neg: false,
            mag: RefNat::zero(),
        }
    }
    /// Build from sign + magnitude, canonicalizing the sign of zero to `false`.
    fn make(neg: bool, mag: RefNat) -> RefInt {
        if mag.is_zero() {
            RefInt { neg: false, mag }
        } else {
            RefInt { neg, mag }
        }
    }
    fn from_i128(v: i128) -> RefInt {
        RefInt::make(v < 0, RefNat::from_u128(v.unsigned_abs()))
    }
    fn is_zero(&self) -> bool {
        self.mag.is_zero()
    }
    fn add(&self, o: &RefInt) -> RefInt {
        if self.neg == o.neg {
            RefInt::make(self.neg, self.mag.add(&o.mag))
        } else {
            match self.mag.cmp(&o.mag) {
                Ordering::Equal => RefInt::zero(),
                Ordering::Greater => RefInt::make(self.neg, self.mag.sub(&o.mag)),
                Ordering::Less => RefInt::make(o.neg, o.mag.sub(&self.mag)),
            }
        }
    }
    fn neg(&self) -> RefInt {
        RefInt::make(!self.neg, self.mag.clone())
    }
    fn sub(&self, o: &RefInt) -> RefInt {
        self.add(&o.neg())
    }
    fn mul(&self, o: &RefInt) -> RefInt {
        RefInt::make(self.neg != o.neg, self.mag.mul(&o.mag))
    }
    fn cmp(&self, o: &RefInt) -> Ordering {
        match (self.neg, o.neg) {
            (false, true) => Ordering::Greater, // includes 0 vs negative
            (true, false) => Ordering::Less,
            (false, false) => self.mag.cmp(&o.mag),
            (true, true) => o.mag.cmp(&self.mag),
        }
    }
    fn sign(&self) -> i8 {
        if self.mag.is_zero() {
            0
        } else if self.neg {
            -1
        } else {
            1
        }
    }
}

// ===========================================================================
// RefRat — reduced num / positive-den. Invariant: den > 0, gcd(|num|, den) = 1.
// ===========================================================================

/// Reference arbitrary-precision rational (reduced; `den > 0`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RefRat {
    num: RefInt,
    den: RefNat,
}

impl RefRat {
    /// Reduce `neg · num_mag / den_mag` (den_mag ≠ 0) to lowest terms, den > 0.
    fn reduce(neg: bool, num_mag: RefNat, den_mag: RefNat) -> RefRat {
        if num_mag.is_zero() {
            return RefRat {
                num: RefInt::zero(),
                den: RefNat { limbs: vec![1] },
            };
        }
        let g = num_mag.gcd(&den_mag);
        let (nq, _) = num_mag.divrem(&g);
        let (dq, _) = den_mag.divrem(&g);
        RefRat {
            num: RefInt::make(neg, nq),
            den: dq,
        }
    }
    fn from_i128(v: i128) -> RefRat {
        RefRat {
            num: RefInt::from_i128(v),
            den: RefNat { limbs: vec![1] },
        }
    }
}

// ===========================================================================
// RefBackend — the reference `Backend` (drop-in beside `Bignum`).
// ===========================================================================

/// The reference backend (`Backend::Int = RefInt`, `Backend::Rat = RefRat`).
#[derive(Clone, Copy, Debug, Default)]
pub struct RefBackend;

impl Backend for RefBackend {
    type Int = RefInt;
    type Rat = RefRat;

    fn int_clone(a: &RefInt) -> RefInt {
        a.clone()
    }
    fn rat_clone(a: &RefRat) -> RefRat {
        a.clone()
    }

    fn int_zero() -> RefInt {
        RefInt::zero()
    }
    fn int_one() -> RefInt {
        RefInt::from_i128(1)
    }
    fn int_from_i128(v: i128) -> RefInt {
        RefInt::from_i128(v)
    }
    fn int_try_to_i128(a: &RefInt) -> Option<i128> {
        if a.mag.limbs.len() > 2 {
            return None;
        }
        let lo = if a.mag.limbs.is_empty() {
            0
        } else {
            a.mag.limbs[0] as u128
        };
        let hi = if a.mag.limbs.len() >= 2 {
            a.mag.limbs[1] as u128
        } else {
            0
        };
        let m: u128 = (hi << 64) | lo;
        if a.neg {
            // magnitude fits iff m ≤ 2^127 (i128::MIN = -2^127)
            if m <= (1u128 << 127) {
                Some((m as i128).wrapping_neg())
            } else {
                None
            }
        } else if m <= (i128::MAX as u128) {
            Some(m as i128)
        } else {
            None
        }
    }

    fn int_add(a: &RefInt, b: &RefInt) -> RefInt {
        a.add(b)
    }
    fn int_sub(a: &RefInt, b: &RefInt) -> RefInt {
        a.sub(b)
    }
    fn int_mul(a: &RefInt, b: &RefInt) -> RefInt {
        a.mul(b)
    }
    fn int_neg(a: &RefInt) -> RefInt {
        a.neg()
    }
    fn int_cmp(a: &RefInt, b: &RefInt) -> Ordering {
        a.cmp(b)
    }
    fn int_sign(a: &RefInt) -> i8 {
        a.sign()
    }
    fn int_is_zero(a: &RefInt) -> bool {
        a.is_zero()
    }
    fn int_gcd(a: &RefInt, b: &RefInt) -> RefInt {
        RefInt::make(false, a.mag.gcd(&b.mag))
    }
    fn int_lcm(a: &RefInt, b: &RefInt) -> RefInt {
        if a.is_zero() || b.is_zero() {
            return RefInt::zero();
        }
        let g = a.mag.gcd(&b.mag);
        let (q, _) = a.mag.mul(&b.mag).divrem(&g);
        RefInt::make(false, q)
    }
    fn int_divrem(a: &RefInt, b: &RefInt) -> (RefInt, RefInt) {
        if b.is_zero() {
            debug_assert!(false, "int_divrem: division by zero (out of contract)");
            return (RefInt::zero(), a.clone());
        }
        let (q, r) = a.mag.divrem(&b.mag);
        // truncate toward zero: quotient sign = a·b, remainder sign = a.
        (RefInt::make(a.neg != b.neg, q), RefInt::make(a.neg, r))
    }

    fn rat_from_i128(v: i128) -> RefRat {
        RefRat::from_i128(v)
    }
    fn rat_from_ints(num: RefInt, den: RefInt) -> RefRat {
        if den.is_zero() {
            debug_assert!(
                false,
                "rat_from_ints: zero denominator (out of §2.2 contract)"
            );
            return RefRat::from_i128(0);
        }
        RefRat::reduce(num.neg != den.neg, num.mag, den.mag)
    }

    fn rat_add(a: &RefRat, b: &RefRat) -> RefRat {
        // a.num/a.den + b.num/b.den = (a.num·b.den + b.num·a.den) / (a.den·b.den)
        let n1 = a.num.mul(&RefInt::make(false, b.den.clone()));
        let n2 = b.num.mul(&RefInt::make(false, a.den.clone()));
        let n = n1.add(&n2);
        RefRat::reduce(n.neg, n.mag, a.den.mul(&b.den))
    }
    fn rat_sub(a: &RefRat, b: &RefRat) -> RefRat {
        let nb = RefRat {
            num: b.num.neg(),
            den: b.den.clone(),
        };
        Self::rat_add(a, &nb)
    }
    fn rat_mul(a: &RefRat, b: &RefRat) -> RefRat {
        let n = a.num.mul(&b.num);
        RefRat::reduce(n.neg, n.mag, a.den.mul(&b.den))
    }
    fn rat_div(a: &RefRat, b: &RefRat) -> RefRat {
        if b.num.is_zero() {
            debug_assert!(false, "rat_div: division by zero (out of contract)");
            return RefRat::from_i128(0);
        }
        // a / b = (a.num·b.den) / (a.den·b.num); sign from b.num, den stays positive.
        let n = a.num.mul(&RefInt::make(false, b.den.clone()));
        RefRat::reduce(n.neg != b.num.neg, n.mag, a.den.mul(&b.num.mag))
    }
    fn rat_neg(a: &RefRat) -> RefRat {
        RefRat {
            num: a.num.neg(),
            den: a.den.clone(),
        }
    }
    fn rat_cmp(a: &RefRat, b: &RefRat) -> Ordering {
        // a.num·b.den vs b.num·a.den (both dens > 0, so no sign flip).
        let l = a.num.mul(&RefInt::make(false, b.den.clone()));
        let r = b.num.mul(&RefInt::make(false, a.den.clone()));
        l.cmp(&r)
    }
    fn rat_sign(a: &RefRat) -> i8 {
        a.num.sign()
    }
    fn rat_is_zero(a: &RefRat) -> bool {
        a.num.is_zero()
    }
    fn rat_numer(a: &RefRat) -> RefInt {
        a.num.clone()
    }
    fn rat_denom(a: &RefRat) -> RefInt {
        RefInt::make(false, a.den.clone())
    }
}

// ===========================================================================
// TEST/FUZZ-ONLY seed helpers — NOT part of the `Backend` trait, NOT proven.
// ===========================================================================

#[cfg(any(test, feature = "fuzzing"))]
impl RefBackend {
    /// **TEST/FUZZ-ONLY seed constructor — NOT a `Backend` trait method, NOT proven.**
    ///
    /// Builds a `RefInt` straight from little-endian bytes (`bytes` = the magnitude,
    /// `neg` = the sign) so the differential/fuzz harness can seed operands of *any*
    /// size and reach the large-operand multiply regimes (Karatsuba / Toom-Cook / FFT)
    /// that [`RefBackend::int_from_i128`] (≤ 2 limbs) can never trigger. Its correctness
    /// is runtime-checked in the harness — every seed is decimal-compared against the
    /// dashu backend before use — never relied on for soundness.
    ///
    /// ⚠️  DO NOT expose beyond the test/fuzz harness. If this ever enters the `Backend`
    /// trait or any Aeneas-lifted / production path, it MUST first be proven
    /// `den(result) = value` in `certify-check/CertifyCheck/RefBackend.lean`, exactly as
    /// `from_i128` is (`int_from_i128_eq`). The `#[cfg(any(test, feature = "fuzzing"))]`
    /// gate keeps it physically out of the trait and the Aeneas lift until then.
    pub fn int_from_le_bytes(neg: bool, bytes: &[u8]) -> RefInt {
        let mut limbs: Vec<u64> = Vec::with_capacity(bytes.len() / 8 + 1);
        let mut chunks = bytes.chunks_exact(8);
        for c in chunks.by_ref() {
            let mut w = [0u8; 8];
            w.copy_from_slice(c);
            limbs.push(u64::from_le_bytes(w));
        }
        let rem = chunks.remainder();
        if !rem.is_empty() {
            let mut w = [0u8; 8];
            w[..rem.len()].copy_from_slice(rem);
            limbs.push(u64::from_le_bytes(w));
        }
        normalize(&mut limbs);
        RefInt::make(neg, RefNat { limbs })
    }

    /// TEST/FUZZ-ONLY: limb count of the magnitude — a cheap O(1) size proxy for the
    /// harness's operand-growth guard. Not a `Backend` method.
    pub fn int_limbs(a: &RefInt) -> usize {
        a.mag.limbs.len()
    }

    /// TEST/FUZZ-ONLY: sign + minimal little-endian magnitude bytes — an **O(n)** canonical
    /// form for fast cross-backend equality (decimal is O(n²), which throttles the fuzzer at
    /// large operands). Zero ⇒ `(false, [])`. Not a `Backend` method.
    pub fn int_le_bytes(a: &RefInt) -> (bool, Vec<u8>) {
        let mut bytes: Vec<u8> = Vec::with_capacity(a.mag.limbs.len() * 8);
        for &limb in &a.mag.limbs {
            bytes.extend_from_slice(&limb.to_le_bytes());
        }
        while bytes.last() == Some(&0) {
            bytes.pop();
        }
        (a.neg && !bytes.is_empty(), bytes)
    }
}

// Decimal rendering (test-only) — the cross-backend canonical form used by the
// `rat` differential to compare `RefBackend` against dashu.
#[cfg(test)]
pub(crate) fn to_dec_string(a: &RefInt) -> alloc::string::String {
    use alloc::string::String;
    if a.mag.is_zero() {
        return String::from("0");
    }
    let mut digits: Vec<u8> = Vec::new();
    let mut cur = a.mag.clone();
    while !cur.is_zero() {
        let (q, rem) = cur.div_rem_small(10);
        digits.push(b'0' + rem as u8);
        cur = q;
    }
    let mut s = String::new();
    if a.neg {
        s.push('-');
    }
    let mut i = digits.len();
    while i > 0 {
        i -= 1;
        s.push(digits[i] as char);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    fn i(v: i128) -> RefInt {
        RefBackend::int_from_i128(v)
    }

    #[test]
    fn int_ops_and_narrowing() {
        // 2^200 is beyond i128; narrowing returns None, small values round-trip.
        let p100 = i(1i128 << 100);
        let p200 = RefBackend::int_mul(&p100, &p100);
        assert_eq!(RefBackend::int_try_to_i128(&p200), None);
        assert_eq!(RefBackend::int_try_to_i128(&i(i128::MAX)), Some(i128::MAX));
        assert_eq!(RefBackend::int_try_to_i128(&i(i128::MIN)), Some(i128::MIN));
        assert_eq!(RefBackend::int_try_to_i128(&i(-7)), Some(-7));
        // gcd(2^200, 2^100) = 2^100 ; lcm = 2^200 ; sign-agnostic; gcd(0,0)=0
        assert_eq!(RefBackend::int_gcd(&p200, &p100), p100);
        assert_eq!(RefBackend::int_lcm(&p200, &p100), p200);
        assert_eq!(
            RefBackend::int_gcd(&RefBackend::int_neg(&p100), &p200),
            p100
        );
        assert!(RefBackend::int_is_zero(&RefBackend::int_gcd(
            &RefBackend::int_zero(),
            &RefBackend::int_zero()
        )));
        assert!(RefBackend::int_is_zero(&RefBackend::int_lcm(
            &p200,
            &RefBackend::int_zero()
        )));
        // int_divrem truncates toward zero: -7/2 = (-3, -1); exact 2^200/2^100 = (2^100, 0)
        assert_eq!(RefBackend::int_divrem(&i(-7), &i(2)), (i(-3), i(-1)));
        let (bq, br) = RefBackend::int_divrem(&p200, &p100);
        assert_eq!(bq, p100);
        assert!(RefBackend::int_is_zero(&br));
    }

    #[test]
    fn rat_normalizes_and_signs() {
        // 6/(-4) → -3/2 reduced, den > 0
        let r = RefBackend::rat_from_ints(i(6), i(-4));
        assert_eq!(RefBackend::rat_numer(&r), i(-3));
        assert_eq!(RefBackend::rat_denom(&r), i(2));
        assert_eq!(RefBackend::rat_sign(&r), -1);
        // 1/2 + 1/2 == 1 ; cmp exact + total
        let half = RefBackend::rat_from_ints(i(1), i(2));
        let one = RefBackend::rat_add(&half, &half);
        assert_eq!(
            RefBackend::rat_cmp(&one, &RefBackend::rat_from_i128(1)),
            Ordering::Equal
        );
        assert_eq!(RefBackend::rat_cmp(&half, &one), Ordering::Less);
        assert_eq!(RefBackend::rat_sign(&RefBackend::rat_sub(&half, &half)), 0);
        // division: (5/6) / (2/3) = 5/4
        let q = RefBackend::rat_div(
            &RefBackend::rat_from_ints(i(5), i(6)),
            &RefBackend::rat_from_ints(i(2), i(3)),
        );
        assert_eq!(RefBackend::rat_numer(&q), i(5));
        assert_eq!(RefBackend::rat_denom(&q), i(4));
    }

    #[test]
    fn decimal_rendering() {
        assert_eq!(to_dec_string(&i(0)), "0");
        assert_eq!(to_dec_string(&i(-7)), "-7");
        assert_eq!(to_dec_string(&i(i128::MAX)), i128::MAX.to_string());
        assert_eq!(to_dec_string(&i(i128::MIN)), i128::MIN.to_string());
        // 2^128 = 340282366920938463463374607431768211456
        let p64 = i(1i128 << 64);
        let p128 = RefBackend::int_mul(&p64, &p64);
        assert_eq!(
            to_dec_string(&p128),
            "340282366920938463463374607431768211456"
        );
    }
}
