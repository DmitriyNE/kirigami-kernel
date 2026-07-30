//! Backend-selection speed yardstick: the deg-12 Sturm polynomial-remainder
//! sequence over ~240-bit rational coefficients, generic over a small `Rat`
//! trait (`backends`). Timed by criterion (`benches/prs.rs`); the cross-backend
//! root-count agreement is the unit test below. Throwaway (see ../../README.md).
// Throwaway backend comparison: don't fuss over creating a `Rational::from(0)`
// per zero-check in the uniform trait impls.
#![allow(clippy::cmp_owned)]

pub mod backends;
pub mod prs;

use backends::Rat;

/// The fixed degree-12 polynomial with ~240-bit rational coefficients.
pub fn make_poly<R: Rat>() -> Vec<R> {
    (0..=12u64)
        .map(|i| prs::big256::<R>(0x1234_5678u64 ^ i.wrapping_mul(0x9e37_79b9)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use backends::{Dashu, Malachite};

    // Two independent backends compute the identical chain ⇒ identical root count
    // (num-rational agrees too but is ~47× slower, so it is left to the bench).
    #[test]
    fn backends_agree_on_root_count() {
        let rc_d = prs::sturm_root_count(&make_poly::<Dashu>());
        let rc_m = prs::sturm_root_count(&make_poly::<Malachite>());
        assert_eq!(rc_d, rc_m);
    }
}
