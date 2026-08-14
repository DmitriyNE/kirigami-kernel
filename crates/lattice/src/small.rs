//! L0 fixed-limb fast path: `i128` integer helpers + [`SmallRat`] (reduced,
//! `den > 0`). Every fallible op returns `Option`; `None` means an `i128`
//! overflow — the caller promotes to the BigInt slow path. Pure, panic-free,
//! and `i128::MIN`-safe (all magnitudes go through `u128`, never `.abs()` on
//! `i128`). This module is what the Kani fast≡slow harness proves against a
//! wider fixed-width reference (`vv-guide §5`).

use core::cmp::Ordering;

/// gcd of two `u128` magnitudes (exact; `u128` holds `|i128::MIN| = 2^127`).
///
/// Computed as **strip the common power of two, then Euclidean on the odd parts**, using the
/// standard identity
///
/// ```text
///   gcd(2^i·m, 2^j·n)  =  2^min(i,j) · gcd(m, n)        (m, n odd)
/// ```
///
/// and narrowing the Euclidean loop to `u64` as soon as both operands fit.
///
/// **Why (OPT.3).** Profiling the kernel put ~60% of *all* runtime in `u128` division: this
/// function is called ~4.6 M times a second, ARM64 has no hardware 128-bit divide, and the plain
/// Euclidean loop spends a software `u128 %` on each of ~12 iterations. Measured on the harvested
/// operand mix, **84.7% of calls have a power-of-two operand** — not a coincidence, since the
/// kernel snaps coordinates onto `2^-30`/`2^-50` dyadic grids, so denominators are powers of two by
/// construction. Under the identity such a call needs no division at all: a power of two has odd
/// part `1`, so the Euclidean step returns immediately. Benchmarked at **265.6 → 44.8 ns/call
/// (5.9×)** on that mix (`benchmarks/gcd-hot-path`).
///
/// The **value is unchanged** — this returns the same gcd as the Euclidean loop for every input, so
/// no enclosure, bound or certificate anywhere moves.
///
/// **Panic-freedom.** The only shifts are `>> ia`, `>> ib` with `ia, ib < 128` (trailing zeros of a
/// nonzero value), and the final `<< shift` with `shift = min(ia, ib) < 128`. That last one cannot
/// overflow the value either: writing `a = 2^ia·m`, the result is `gcd(m, n)·2^shift ≤ m·2^ia = a`,
/// so it is bounded by `min(a, b)`.
pub(crate) fn gcd_u128(a: u128, b: u128) -> u128 {
    if a == 0 {
        return b;
    }
    if b == 0 {
        return a;
    }
    let (ia, ib) = (a.trailing_zeros(), b.trailing_zeros());
    let shift = if ia < ib { ia } else { ib };
    // The odd parts. Their gcd is odd, and the common power of two is restored at the end.
    let (mut x, mut y) = (a >> ia, b >> ib);
    while y != 0 {
        // Both operands fit 64 bits: finish where the divide is a hardware instruction.
        if x <= u64::MAX as u128 && y <= u64::MAX as u128 {
            let (mut p, mut q) = (x as u64, y as u64);
            while q != 0 {
                let t = p % q;
                p = q;
                q = t;
            }
            return (p as u128) << shift;
        }
        let t = x % y;
        x = y;
        y = t;
    }
    x << shift
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

/// `x / y`, reduced; `None` on overflow. `y != 0` by contract; a `y.num == 0`
/// makes the denominator 0 ⇒ `reduce` returns `None` ⇒ the caller promotes.
pub(crate) fn div(x: &SmallRat, y: &SmallRat) -> Option<SmallRat> {
    let num = x.num.checked_mul(y.den)?;
    let den = x.den.checked_mul(y.num)?; // may be < 0 → reduce migrates the sign (den > 0)
    SmallRat::reduce(num, den)
}

/// `1 / x`, reduced; `None` iff `x.num` does not fit as a denominator
/// (`i128::MIN`). `x != 0` by contract (`x.num == 0` ⇒ `None` ⇒ promote).
pub(crate) fn recip(x: &SmallRat) -> Option<SmallRat> {
    SmallRat::reduce(x.den, x.num) // den/num; reduce migrates the sign so den > 0
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The reference [`gcd_u128`] replaced (OPT.3): plain `u128` Euclidean.
    fn gcd_euclid(mut a: u128, mut b: u128) -> u128 {
        while b != 0 {
            let t = a % b;
            a = b;
            b = t;
        }
        a
    }

    /// **The strip-twos gcd computes exactly what the Euclidean loop computed.**
    ///
    /// This is the whole safety argument for OPT.3: the optimization is a *speed* change and must
    /// not be an arithmetic one, since every enclosure, bound and certificate in the kernel is
    /// built out of these rationals. Covers the shapes the harvested operand mix is made of —
    /// powers of two (84.7% of real calls, the case the identity short-circuits), mixed widths
    /// straddling the `u64` narrowing boundary, and full 127-bit magnitudes including
    /// `|i128::MIN|`.
    #[test]
    fn the_strip_twos_gcd_agrees_with_the_euclidean_loop() {
        let check = |a: u128, b: u128| {
            assert_eq!(
                gcd_u128(a, b),
                gcd_euclid(a, b),
                "gcd({a}, {b}) diverged from the Euclidean reference"
            );
        };
        // Exhaustive small grid: ordering, parity and zero handling.
        for a in 0u128..64 {
            for b in 0u128..64 {
                check(a, b);
            }
        }
        // Powers of two against odd, even and huge partners — the dominant real shape.
        for k in [0u32, 1, 30, 50, 63, 64, 100, 126, 127] {
            let p = 1u128 << k;
            for other in [
                1u128,
                3,
                5,
                12,
                u64::MAX as u128,
                (1u128 << 100) + 1,
                u128::MAX,
            ] {
                check(p, other);
                check(other, p);
            }
        }
        // Straddling the u64 narrowing boundary in both directions.
        for d in 0u128..8 {
            check(u64::MAX as u128 - d, u64::MAX as u128 + d);
            check((1u128 << 64) + d, (1u128 << 64) - d - 1);
        }
        // A deterministic spread of wide magnitudes, including |i128::MIN| = 2^127.
        let mut s: u128 = 0x9E37_79B9_7F4A_7C15;
        for _ in 0..20_000 {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let t = s.rotate_left(41) | 1;
            check(s, t);
            check(s << 1, t << 3);
            check(1u128 << 127, s);
        }
    }
}
