//! L0 fixed-limb fast path: `i128` integer helpers + [`SmallRat`] (reduced,
//! `den > 0`). Every fallible op returns `Option`; `None` means an `i128`
//! overflow — the caller promotes to the BigInt slow path. Pure, panic-free,
//! and `i128::MIN`-safe (all magnitudes go through `u128`, never `.abs()` on
//! `i128`). This module is what the Kani fast≡slow harness proves against a
//! wider fixed-width reference (`vv-guide §5`).

use core::cmp::Ordering;

/// gcd of two `u128` magnitudes (exact; `u128` holds `|i128::MIN| = 2^127`).
pub(crate) fn gcd_u128(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

/// `i128` gcd magnitude (`≥ 0`). `None` iff the gcd is `2^127` (only
/// `gcd(i128::MIN, i128::MIN)`), which does not fit `i128` → promote.
pub(crate) fn i128_gcd(a: i128, b: i128) -> Option<i128> {
    i128::try_from(gcd_u128(a.unsigned_abs(), b.unsigned_abs())).ok()
}

/// `-(m)` as `i128` if representable (`m ≤ 2^127`, mapping `2^127 → i128::MIN`).
fn neg_mag(m: u128) -> Option<i128> {
    const LIM: u128 = i128::MAX as u128 + 1; // 2^127 = |i128::MIN|
    match m.cmp(&LIM) {
        Ordering::Less => Some(-(m as i128)),
        Ordering::Equal => Some(i128::MIN),
        Ordering::Greater => None,
    }
}

/// A reduced `i128` rational: `gcd(|num|, den) = 1`, `den > 0`, canonical zero
/// `{num: 0, den: 1}`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SmallRat {
    pub(crate) num: i128,
    pub(crate) den: i128,
}

impl SmallRat {
    /// The integer `v` as `v/1`.
    pub(crate) fn int(v: i128) -> Self {
        SmallRat { num: v, den: 1 }
    }

    /// Reduce `num/den` to lowest terms with `den > 0`. `None` if `den == 0`, or
    /// if the reduced form does not fit `i128` (the `den`-magnitude-`2^127` edge)
    /// → the caller promotes.
    pub(crate) fn reduce(num: i128, den: i128) -> Option<Self> {
        if den == 0 {
            return None;
        }
        let neg = (num < 0) ^ (den < 0);
        let g = gcd_u128(num.unsigned_abs(), den.unsigned_abs()); // ≥ 1 (den ≠ 0)
        let n_mag = num.unsigned_abs() / g;
        let d_mag = den.unsigned_abs() / g;
        let n = if neg {
            neg_mag(n_mag)?
        } else {
            i128::try_from(n_mag).ok()?
        };
        let d = i128::try_from(d_mag).ok()?; // den > 0
        Some(SmallRat { num: n, den: d })
    }

    /// Pack an already-reduced `(num, den)` (coprime, `den > 0`) coming from the
    /// canonical backend rational — no re-reduction. Debug-checked.
    pub(crate) fn from_reduced(num: i128, den: i128) -> Self {
        debug_assert!(den > 0, "from_reduced: den must be > 0");
        SmallRat { num, den }
    }
}

/// `x + y`, reduced; `None` on any `i128` overflow. Uses `lcm` as the common
/// denominator (via `gcd`) to keep intermediates small and stay in the fast path.
pub(crate) fn add(x: &SmallRat, y: &SmallRat) -> Option<SmallRat> {
    let g = i128_gcd(x.den, y.den)?; // > 0
    let xd = x.den / g;
    let den = xd.checked_mul(y.den)?; // lcm(x.den, y.den) > 0
    let t1 = x.num.checked_mul(y.den / g)?;
    let t2 = y.num.checked_mul(xd)?;
    let num = t1.checked_add(t2)?;
    SmallRat::reduce(num, den)
}

/// `x - y`, reduced; `None` on any `i128` overflow.
pub(crate) fn sub(x: &SmallRat, y: &SmallRat) -> Option<SmallRat> {
    let g = i128_gcd(x.den, y.den)?; // > 0
    let xd = x.den / g;
    let den = xd.checked_mul(y.den)?;
    let t1 = x.num.checked_mul(y.den / g)?;
    let t2 = y.num.checked_mul(xd)?;
    let num = t1.checked_sub(t2)?;
    SmallRat::reduce(num, den)
}

/// `x * y`, reduced; `None` on any `i128` overflow.
pub(crate) fn mul(x: &SmallRat, y: &SmallRat) -> Option<SmallRat> {
    let num = x.num.checked_mul(y.num)?;
    let den = x.den.checked_mul(y.den)?; // both > 0
    SmallRat::reduce(num, den)
}

/// `-x`, reduced; `None` iff `x.num == i128::MIN`.
pub(crate) fn neg(x: &SmallRat) -> Option<SmallRat> {
    Some(SmallRat {
        num: x.num.checked_neg()?,
        den: x.den, // unchanged, still > 0 and coprime
    })
}

/// Exact `x.cmp(y)`; `None` on `i128` overflow of the cross products. Since both
/// denominators are `> 0`, `sign(x − y) = sign(x.num·y.den − y.num·x.den)`.
pub(crate) fn cmp(x: &SmallRat, y: &SmallRat) -> Option<Ordering> {
    let l = x.num.checked_mul(y.den)?;
    let r = y.num.checked_mul(x.den)?;
    Some(l.cmp(&r))
}

/// `-1 | 0 | 1` (total, never overflows: `den > 0`).
pub(crate) fn sign(x: &SmallRat) -> i8 {
    x.num.signum() as i8
}
