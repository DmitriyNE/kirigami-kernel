//! **The number bridge** — decimal text to exact rational, and the honest bound when a parser
//! got there first.
//!
//! A CAD file does not contain floating-point numbers. It contains *decimal literals*, and a
//! decimal literal is exactly rational: `12.345` **is** `12345/1000`, `1.5e-3` **is** `3/2000`.
//! [`rat_from_decimal`] is that reading, and it loses nothing.
//!
//! Where a parser has already turned the text into an `f64` — which is what the SVG path grammar
//! hands over — the literal is recovered through Rust's shortest-round-trip `Display`
//! ([`rat_from_f64`]): the shortest decimal that reads back as the same `f64`. For any literal
//! under 17 significant digits that *is* the literal, exactly. Beyond that it differs by at most
//! [`transport_bound`], which is reported rather than assumed away — we cannot see the text, so we
//! do not claim to have recovered it.
//!
//! ```
//! use interchange::num::rat_from_decimal;
//! use lattice::{Bignum, Rat};
//!
//! let q = rat_from_decimal::<Bignum>("12.345").expect("a decimal literal is a rational");
//! assert_eq!(q, Rat::new(12345, 1000));
//! ```

use lattice::{Backend, Rat};

/// Decimal exponents beyond this magnitude are refused rather than expanded — a file asking for
/// `1e1000000` should not be able to make us build the integer.
const MAX_EXP: i64 = 6000;

/// Parse a decimal literal into the exact rational it denotes.
///
/// Accepts a leading sign, an optional integer part, an optional fractional part, and an optional
/// `e`/`E` exponent — the intersection of what DXF group values and SVG numbers may look like.
/// Returns `None` for anything else (including `NaN`, `inf`, and an empty mantissa), because a
/// coordinate that cannot be read is a refusal, not a zero.
///
/// This is **exact**: there is no rounding step and no tolerance.
///
/// ```
/// use interchange::num::rat_from_decimal;
/// use lattice::{Bignum, Rat};
///
/// assert_eq!(rat_from_decimal::<Bignum>("-0.5"), Some(Rat::new(-1, 2)));
/// assert_eq!(rat_from_decimal::<Bignum>("1.5e-3"), Some(Rat::new(3, 2000)));
/// assert_eq!(rat_from_decimal::<Bignum>(".25"), Some(Rat::new(1, 4)));
/// assert_eq!(rat_from_decimal::<Bignum>("7."), Some(Rat::from_i128(7)));
/// assert_eq!(rat_from_decimal::<Bignum>("nan"), None);
/// ```
pub fn rat_from_decimal<B: Backend>(s: &str) -> Option<Rat<B>> {
    let s = s.trim();
    let (negative, body) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };

    let (mantissa, exponent) = match body.find(['e', 'E']) {
        Some(i) => {
            let (m, e) = body.split_at(i);
            (m, parse_exponent(&e[1..])?)
        }
        None => (body, 0i64),
    };

    let (int_part, frac_part) = match mantissa.find('.') {
        Some(i) => (&mantissa[..i], &mantissa[i + 1..]),
        None => (mantissa, ""),
    };
    if int_part.is_empty() && frac_part.is_empty() {
        return None;
    }

    // Horner over the digits: `Rat` promotes off the fast path on its own, so an outsized
    // coordinate widens rather than wrapping.
    let ten = Rat::from_i128(10);
    let mut digits = Rat::from_i128(0);
    for part in [int_part, frac_part] {
        for b in part.bytes() {
            if !b.is_ascii_digit() {
                return None;
            }
            digits = digits.mul(&ten).add(&Rat::from_i128(i128::from(b - b'0')));
        }
    }

    // `digits` is the mantissa read as an integer, so it is scaled by 10^(exponent − |frac|).
    let scale = exponent.checked_sub(frac_part.len() as i64)?;
    if scale.abs() > MAX_EXP {
        return None;
    }
    let value = digits.mul(&pow10(scale));
    Some(if negative { value.neg() } else { value })
}

/// The exponent field of a decimal literal (`e` already stripped).
fn parse_exponent(s: &str) -> Option<i64> {
    let (negative, digits) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let magnitude: i64 = digits.parse().ok()?;
    Some(if negative { -magnitude } else { magnitude })
}

/// `10^k` as an exact rational, for either sign of `k`.
pub fn pow10<B: Backend>(k: i64) -> Rat<B> {
    let ten = Rat::from_i128(10);
    let mut acc = Rat::from_i128(1);
    for _ in 0..k.unsigned_abs().min(MAX_EXP as u64) {
        acc = acc.mul(&ten);
    }
    if k < 0 { acc.recip() } else { acc }
}

/// `2^e` as an exact rational, for either sign of `e`.
pub fn pow2<B: Backend>(e: i32) -> Rat<B> {
    let two = Rat::from_i128(2);
    let mut acc = Rat::from_i128(1);
    for _ in 0..e.unsigned_abs() {
        acc = acc.mul(&two);
    }
    if e < 0 { acc.recip() } else { acc }
}

/// Recover the decimal literal behind an `f64` — the shortest decimal that reads back as `x` —
/// as an exact rational.
///
/// `None` for `NaN`/`±inf`, which are not coordinates.
///
/// ```
/// use interchange::num::rat_from_f64;
/// use lattice::{Bignum, Rat};
///
/// // 0.1 is not a dyadic rational, and this does NOT return the f64's true binary value
/// // (0.1000000000000000055511151231257827…). It returns the literal the file wrote.
/// assert_eq!(rat_from_f64::<Bignum>(0.1), Some(Rat::new(1, 10)));
/// assert_eq!(rat_from_f64::<Bignum>(f64::NAN), None);
/// ```
pub fn rat_from_f64<B: Backend>(x: f64) -> Option<Rat<B>> {
    if !x.is_finite() {
        return None;
    }
    // Rust's `Display` for floats emits the shortest round-tripping decimal and never uses
    // exponential notation, but `rat_from_decimal` accepts an exponent anyway.
    rat_from_decimal(&format!("{x}"))
}

/// An upper bound on the **transport error**: how far [`rat_from_f64`]'s answer can be from the
/// decimal literal that produced `x`.
///
/// The file's literal `L` rounds to `x`, so `|L − x| ≤ ulp(x)/2`; the recovered decimal `q` reads
/// back as `x`, so `|q − x| ≤ ulp(x)/2`; hence `|q − L| ≤ ulp(x)`. It is **zero** whenever `L` has
/// under 17 significant digits — universally true of CAD output — but that is a fact about the
/// file, not something visible from this side of the parser, so the bound is what gets reported.
///
/// This is a different quantity from the construction backward error `δ` (see [`crate::arc`]):
/// transport is what a *parser* did, `δ` is what *we* did. Reporting one number for both would let
/// a lossy construction hide behind an ulp.
pub fn transport_bound<B: Backend>(x: f64) -> Rat<B> {
    if !x.is_finite() {
        return Rat::from_i128(0);
    }
    let bits = x.abs().to_bits();
    let biased = ((bits >> 52) & 0x7ff) as i32;
    // Subnormals (biased == 0) all share the smallest ulp, 2⁻¹⁰⁷⁴.
    let exponent = if biased == 0 {
        -1074
    } else {
        biased - 1023 - 52
    };
    pow2(exponent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice::Bignum;

    type Q = Rat<Bignum>;

    fn q(s: &str) -> Q {
        rat_from_decimal::<Bignum>(s).expect("a well-formed decimal")
    }

    /// The claim the whole milestone rests on: reading a decimal literal is **exact**, including
    /// the values that are famously not representable in binary floating point.
    #[test]
    fn a_decimal_literal_is_exactly_a_rational() {
        assert_eq!(q("12.345"), Q::new(12345, 1000));
        assert_eq!(q("0.1"), Q::new(1, 10));
        assert_eq!(q("0.3"), Q::new(3, 10));
        // 0.1 + 0.2 == 0.3 exactly here, which is the entire point.
        assert_eq!(q("0.1").add(&q("0.2")), q("0.3"));
    }

    /// Sign, exponent, and the two elided parts a real file uses.
    #[test]
    fn the_accepted_grammar_covers_what_files_write() {
        assert_eq!(q("-0.5"), Q::new(-1, 2));
        assert_eq!(q("+2"), Q::from_i128(2));
        assert_eq!(q("1.5e-3"), Q::new(3, 2000));
        assert_eq!(q("1.5E3"), Q::new(1500, 1));
        assert_eq!(q(".25"), Q::new(1, 4));
        assert_eq!(q("7."), Q::from_i128(7));
        assert_eq!(q("  3.0  "), Q::from_i128(3));
        assert_eq!(q("-0"), Q::from_i128(0));
    }

    /// A coordinate that cannot be read is a refusal, never a zero — a silently-zeroed vertex is
    /// a part with a spike in it, and nothing downstream could tell it was not authored.
    #[test]
    fn malformed_input_is_refused_not_defaulted() {
        for bad in [
            "", ".", "nan", "inf", "1.2.3", "1e", "1e+", "--1", "0x10", "1 2", "1,5", "e5",
        ] {
            assert_eq!(
                rat_from_decimal::<Bignum>(bad),
                None,
                "{bad:?} must not parse"
            );
        }
    }

    /// A coordinate too wide for the fast path widens instead of wrapping (`Rat` promotes).
    #[test]
    fn an_outsized_coordinate_widens_rather_than_wrapping() {
        let big = q("123456789012345678901234567890.5");
        assert_eq!(
            big.mul(&Q::from_i128(2)),
            q("246913578024691357802469135781")
        );
        // …and an exponent beyond the guard is refused rather than expanded.
        assert_eq!(rat_from_decimal::<Bignum>("1e999999"), None);
    }

    /// The f64 route recovers the *literal*, not the binary value the literal rounded to. This is
    /// the distinction the whole bridge turns on: `0.1` comes back as `1/10`, not as
    /// `3602879701896397/36028797018963968`.
    #[test]
    fn the_f64_route_recovers_the_literal_not_the_binary_value() {
        assert_eq!(rat_from_f64::<Bignum>(0.1), Some(Q::new(1, 10)));
        assert_eq!(rat_from_f64::<Bignum>(12.345), Some(Q::new(12345, 1000)));
        assert_eq!(rat_from_f64::<Bignum>(-0.0), Some(Q::from_i128(0)));
        assert_eq!(rat_from_f64::<Bignum>(f64::INFINITY), None);
        // The true binary value of 0.1 is strictly larger than 1/10 — so this is a real
        // distinction, not two spellings of one number.
        let dyadic = Q::new(3602879701896397, 1i128 << 55);
        assert!(dyadic > Q::new(1, 10));
    }

    /// Transport is an ulp bound, and it scales with the magnitude the way an ulp does.
    #[test]
    fn the_transport_bound_is_the_ulp() {
        // 1.0 has exponent 0, so ulp = 2⁻⁵².
        assert_eq!(transport_bound::<Bignum>(1.0), pow2::<Bignum>(-52));
        // 2.0 sits in the next binade: ulp = 2⁻⁵¹.
        assert_eq!(transport_bound::<Bignum>(2.0), pow2::<Bignum>(-51));
        // Sign is irrelevant.
        assert_eq!(
            transport_bound::<Bignum>(-100.0),
            transport_bound::<Bignum>(100.0)
        );
        // And it really does bound the round trip on a value with more digits than an f64 holds.
        let x = 1.234_567_890_123_456_7_f64;
        let recovered = rat_from_f64::<Bignum>(x).expect("finite");
        let literal = q("1.23456789012345678901234567890");
        let gap = recovered.sub(&literal);
        let gap = if gap.sign() < 0 { gap.neg() } else { gap };
        assert!(gap <= transport_bound::<Bignum>(x), "gap {gap:?} over ulp");
    }

    /// Powers, both signs.
    #[test]
    fn powers_are_exact_both_ways() {
        assert_eq!(pow10::<Bignum>(3), Q::from_i128(1000));
        assert_eq!(pow10::<Bignum>(-3), Q::new(1, 1000));
        assert_eq!(pow10::<Bignum>(0), Q::from_i128(1));
        assert_eq!(pow2::<Bignum>(10), Q::from_i128(1024));
        assert_eq!(pow2::<Bignum>(-10), Q::new(1, 1024));
    }
}
