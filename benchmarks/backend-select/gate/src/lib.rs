//! no_std+alloc gate probes. Each `*_ok` builds two ~256-bit values and does
//! add/mul/compare — just enough to force the candidate to link. If the crate
//! (with its `std` feature off) still needs `std`, this fails to compile for
//! `thumbv7em-none-eabi`, which is exactly the gate.
#![no_std]

extern crate alloc;

#[cfg(feature = "dashu")]
pub fn dashu_ok() -> bool {
    use dashu::integer::{IBig, UBig};
    use dashu::rational::RBig;
    let n = (IBig::from(1) << 255) + IBig::from(7);
    let d = (UBig::from(1u8) << 200) + UBig::from(3u8);
    let a = RBig::from_parts(n, d);
    let b = RBig::from_parts(IBig::from(5), UBig::from(2u8));
    let c = &a + &b;
    let e = &a * &b;
    (c > e) || (a == b)
}

#[cfg(feature = "num")]
pub fn num_ok() -> bool {
    use num_bigint::BigInt;
    use num_rational::Ratio; // BigRational alias needs num-rational's num-bigint feature; Ratio<BigInt> avoids it
    let n = (BigInt::from(1) << 255u32) + BigInt::from(7);
    let d = (BigInt::from(1) << 200u32) + BigInt::from(3);
    let a = Ratio::new(n, d);
    let b = Ratio::new(BigInt::from(5), BigInt::from(2));
    let c = &a + &b;
    let e = &a * &b;
    (c > e) || (a == b)
}

#[cfg(feature = "malachite")]
pub fn malachite_ok() -> bool {
    use malachite::Integer;
    use malachite::Rational;
    let n = (Integer::from(1) << 255u64) + Integer::from(7);
    let d = (Integer::from(1) << 200u64) + Integer::from(3);
    let a = Rational::from_integers(n, d);
    let b = Rational::from_integers(Integer::from(5), Integer::from(2));
    let c = &a + &b;
    let e = &a * &b;
    (c > e) || (a == b)
}

#[cfg(feature = "ibig")]
pub fn ibig_ok() -> bool {
    // ibig is INTEGER-ONLY (no native rational) — probed at the integer level.
    use ibig::IBig;
    let a = (IBig::from(1) << 255) + IBig::from(7);
    let b = (IBig::from(1) << 200) + IBig::from(3);
    let c = &a + &b;
    let e = &a * &b;
    (c > e) || (a == b)
}
