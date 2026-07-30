//! A tiny uniform rational interface + one impl per candidate, so the same Sturm
//! PRS runs identically against each backend's own rational type. Throwaway.
use core::cmp::Ordering;

pub trait Rat: Clone {
    const NAME: &'static str;
    fn from_i64(n: i64) -> Self;
    fn zero() -> Self;
    fn add(&self, o: &Self) -> Self;
    fn sub(&self, o: &Self) -> Self;
    fn mul(&self, o: &Self) -> Self;
    fn div(&self, o: &Self) -> Self;
    fn is_zero(&self) -> bool;
    fn cmp0(&self) -> Ordering;
    /// Size proxy: characters in the value's decimal `Display` (num/den). Used
    /// only to profile the workload — confirms all backends see identical sizes.
    fn size_chars(&self) -> usize;
}

// ---- dashu ----------------------------------------------------------------
use dashu::rational::RBig;

#[derive(Clone)]
pub struct Dashu(RBig);

impl Rat for Dashu {
    const NAME: &'static str = "dashu 0.4.4";
    fn from_i64(n: i64) -> Self {
        Dashu(RBig::from(n))
    }
    fn zero() -> Self {
        Dashu(RBig::ZERO)
    }
    fn add(&self, o: &Self) -> Self {
        Dashu(&self.0 + &o.0)
    }
    fn sub(&self, o: &Self) -> Self {
        Dashu(&self.0 - &o.0)
    }
    fn mul(&self, o: &Self) -> Self {
        Dashu(&self.0 * &o.0)
    }
    fn div(&self, o: &Self) -> Self {
        Dashu(&self.0 / &o.0)
    }
    fn is_zero(&self) -> bool {
        self.0 == RBig::ZERO
    }
    fn cmp0(&self) -> Ordering {
        self.0.cmp(&RBig::ZERO)
    }
    fn size_chars(&self) -> usize {
        format!("{}", self.0).len()
    }
}

// ---- num-rational ---------------------------------------------------------
use num_bigint::BigInt;
use num_rational::Ratio;
use num_traits::Zero;

#[derive(Clone)]
pub struct Num(Ratio<BigInt>);

impl Rat for Num {
    const NAME: &'static str = "num-rational 0.4.2";
    fn from_i64(n: i64) -> Self {
        Num(Ratio::from_integer(BigInt::from(n)))
    }
    fn zero() -> Self {
        Num(Ratio::zero())
    }
    fn add(&self, o: &Self) -> Self {
        Num(&self.0 + &o.0)
    }
    fn sub(&self, o: &Self) -> Self {
        Num(&self.0 - &o.0)
    }
    fn mul(&self, o: &Self) -> Self {
        Num(&self.0 * &o.0)
    }
    fn div(&self, o: &Self) -> Self {
        Num(&self.0 / &o.0)
    }
    fn is_zero(&self) -> bool {
        self.0.is_zero()
    }
    fn cmp0(&self) -> Ordering {
        self.0.cmp(&Ratio::zero())
    }
    fn size_chars(&self) -> usize {
        format!("{}", self.0).len()
    }
}

// ---- malachite ------------------------------------------------------------
use malachite::Rational;

#[derive(Clone)]
pub struct Malachite(Rational);

impl Rat for Malachite {
    const NAME: &'static str = "malachite 0.4.22";
    fn from_i64(n: i64) -> Self {
        Malachite(Rational::from(n))
    }
    fn zero() -> Self {
        Malachite(Rational::from(0))
    }
    fn add(&self, o: &Self) -> Self {
        Malachite(&self.0 + &o.0)
    }
    fn sub(&self, o: &Self) -> Self {
        Malachite(&self.0 - &o.0)
    }
    fn mul(&self, o: &Self) -> Self {
        Malachite(&self.0 * &o.0)
    }
    fn div(&self, o: &Self) -> Self {
        Malachite(&self.0 / &o.0)
    }
    fn is_zero(&self) -> bool {
        self.0 == Rational::from(0)
    }
    fn cmp0(&self) -> Ordering {
        self.0.cmp(&Rational::from(0))
    }
    fn size_chars(&self) -> usize {
        format!("{}", self.0).len()
    }
}
