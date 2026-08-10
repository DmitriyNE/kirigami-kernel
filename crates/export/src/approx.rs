//! Exact→`f64` approximation for diagnostics rendering.
//!
//! The certified crates never produce floats (spec invariant 1); a viewer needs
//! pixels. This module is the single, quarantined bridge: it reads the exact value
//! out of `lattice` (reduced numerator/denominator as decimal strings, or the
//! `a + b√d` triple) and casts to `f64` at the last moment. The rounding here is a
//! display concern only — no certificate ever depends on it, and nothing upstream
//! of this module ever sees a float.

use lattice::{Backend, Rat, Surd};

/// Approximate an exact rational as `f64` (diagnostics only).
///
/// Reads the reduced numerator and denominator as base-10 strings — keeping the
/// float out of `lattice` — and divides in `f64`. For the modest coordinates the
/// viewer renders, both parts land well inside `f64`'s exact-integer range, so the
/// quotient is correctly rounded.
///
/// ```
/// use export::approx::rat_to_f64;
/// use lattice::{Bignum, Rat};
/// assert_eq!(rat_to_f64(&Rat::<Bignum>::new(-3, 2)), -1.5);
/// ```
pub fn rat_to_f64<B: Backend>(r: &Rat<B>) -> f64 {
    let (n, d) = r.numer_denom_decimal();
    let nf = n.parse::<f64>().unwrap_or(f64::NAN);
    let df = d.parse::<f64>().unwrap_or(f64::NAN);
    nf / df
}

/// Approximate an exact surd `a + b√d` as `f64` (diagnostics only).
///
/// `d ≥ 0` holds by [`Surd`]'s contract; the `max(0.0)` is a defensive no-op that
/// keeps `sqrt` off `NaN` even if a caller hands over a malformed value.
///
/// ```
/// use export::approx::surd_to_f64;
/// use lattice::{Bignum, Rat, Surd};
/// // 1 + √2 ≈ 2.414213562…
/// let s = Surd::<Bignum>::new(Rat::new(1, 1), Rat::new(1, 1), Rat::new(2, 1));
/// assert!((surd_to_f64(&s) - 2.414_213_562_373_095).abs() < 1e-12);
/// ```
pub fn surd_to_f64<B: Backend>(s: &Surd<B>) -> f64 {
    let (a, b, d) = s.parts();
    rat_to_f64(a) + rat_to_f64(b) * rat_to_f64(d).max(0.0).sqrt()
}

/// Approximate an exact rational 3-vector as `[f64; 3]` (diagnostics only) — a
/// 3D cone-surface point from [`geom`](../../geom/index.html)'s `Chart::surface`.
pub fn vec3_to_f64<B: Backend>(v: &[Rat<B>; 3]) -> [f64; 3] {
    [rat_to_f64(&v[0]), rat_to_f64(&v[1]), rat_to_f64(&v[2])]
}

/// Snap an `f64` to an exact rational on the dyadic grid `round(x·2^bits) / 2^bits`
/// (diagnostics only) — the **reverse** of [`rat_to_f64`].
///
/// This is the one place a float becomes exact, and it is used **only** to hand a
/// float-oracle *proposal* (e.g. a fitted cut-rail coefficient, `export::cut_oracle`)
/// to the certified side, which re-verifies it exactly ([`develop::cut::cut_fit`]).
/// The float never enters a certificate — a loose snap can only make the exact
/// re-check `Unresolved`, never a wrong `Verified`. `bits` is capped at 52 so
/// `x·2^bits` stays inside `f64`'s exact-integer range for the modest coefficients a
/// rail fit produces; a non-finite `x` (or one out of `i128` range) saturates
/// harmlessly under Rust's `as` cast (the exact checker then rejects the proposal).
///
/// ```
/// use export::approx::{f64_to_rat, rat_to_f64};
/// use lattice::Bignum;
/// let q = f64_to_rat::<Bignum>(0.375, 40); // 3/8 is dyadic → exact
/// assert_eq!(rat_to_f64(&q), 0.375);
/// ```
pub fn f64_to_rat<B: Backend>(x: f64, bits: u32) -> Rat<B> {
    let den = 1i128 << bits.min(52);
    let num = (x * den as f64).round();
    Rat::new(num as i128, den)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice::{Bignum, Rat, Surd};

    fn r(n: i128, d: i128) -> Rat<Bignum> {
        Rat::new(n, d)
    }

    #[test]
    fn rationals_round_trip() {
        assert_eq!(rat_to_f64(&r(4, 1)), 4.0);
        assert_eq!(rat_to_f64(&r(-3, 1)), -3.0);
        assert_eq!(rat_to_f64(&r(-6, 4)), -1.5); // reduces to -3/2
        assert_eq!(rat_to_f64(&r(0, 5)), 0.0);
        assert_eq!(rat_to_f64(&r(1, 8)), 0.125);
    }

    #[test]
    fn rational_surd_is_its_rational() {
        // A rational lands in `Surd` as `a + 0√0`; the disk crossing y = 3 is exact.
        let three = Surd::<Bignum>::from_rat(r(3, 1));
        assert_eq!(surd_to_f64(&three), 3.0);
    }

    #[test]
    fn irrational_surd_matches_reference() {
        // 2 + 3√5 ≈ 8.708203932… — an external decimal reference (not a known constant).
        let s = Surd::<Bignum>::new(r(2, 1), r(3, 1), r(5, 1));
        assert!((surd_to_f64(&s) - 8.708_203_932_499_37).abs() < 1e-12);
        // −√2: compare against f64's own √2 (a bare 1.4142… literal trips clippy::approx_constant).
        let neg = Surd::<Bignum>::new(r(0, 1), r(-1, 1), r(2, 1));
        assert!((surd_to_f64(&neg) + 2.0_f64.sqrt()).abs() < 1e-12);
    }

    #[test]
    fn vec3_triple() {
        let v = [r(1, 1), r(-5, 2), r(9, 4)];
        assert_eq!(vec3_to_f64(&v), [1.0, -2.5, 2.25]);
    }

    #[test]
    fn big_denominator_beyond_i128() {
        // 1 / 2^130 — forces the lattice slow path (beyond i128); must still render.
        let two = r(2, 1);
        let mut den = r(1, 1);
        for _ in 0..130 {
            den = den.mul(&two);
        }
        let tiny = r(1, 1).div(&den);
        let approx = rat_to_f64(&tiny);
        assert!(approx > 0.0 && approx < 1e-38);
    }
}
