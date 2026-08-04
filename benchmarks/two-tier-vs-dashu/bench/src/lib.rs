//! Two-tier (`lattice::Rat<Bignum>` = Fast(i128)/Slow(dashu)) vs dashu-only
//! (`Backend::rat_*` on `BigRat`). Same exact ℚ arithmetic, the only difference
//! being whether the i128 fast path is present. Workloads stay in the small-
//! coordinate regime the fast path targets, except `crossover_*` which grows past
//! i128 to expose the promotion cost.

use core::cmp::Ordering;
use lattice::{Backend, BigInt, BigRat, Bignum, Int, Rat};

pub type TInt = Int<Bignum>;
pub type TRat = Rat<Bignum>;

// -------- deterministic data (no rand crate; a fixed LCG) --------

fn lcg(s: &mut u64) -> u64 {
    *s = s
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *s
}

/// `n` integers in `[-bound, bound]`.
pub fn small_ints(n: usize, seed: u64, bound: i128) -> Vec<i128> {
    let mut s = seed;
    (0..n)
        .map(|_| (lcg(&mut s) as i128).rem_euclid(2 * bound + 1) - bound)
        .collect()
}

/// `n` rationals `(num, den)` with `num ∈ [-bound, bound]`, `den ∈ [1, bound]`.
pub fn small_rats(n: usize, seed: u64, bound: i128) -> Vec<(i128, i128)> {
    let mut s = seed;
    (0..n)
        .map(|_| {
            let num = (lcg(&mut s) as i128).rem_euclid(2 * bound + 1) - bound;
            let den = (lcg(&mut s) as i128).rem_euclid(bound) + 1;
            (num, den)
        })
        .collect()
}

/// `n` multipliers in `[2, 10]` (a running product overflows i128 within ~40).
pub fn small_mults(n: usize, seed: u64) -> Vec<i128> {
    let mut s = seed;
    (0..n).map(|_| (lcg(&mut s) % 9) as i128 + 2).collect()
}

// -------- builders (pre-materialize both representations off the timed path) --------

pub fn twotier_ints(d: &[i128]) -> Vec<TInt> {
    d.iter().map(|&v| Int::from_i128(v)).collect()
}
pub fn dashu_ints(d: &[i128]) -> Vec<BigInt> {
    d.iter().map(|&v| Bignum::int_from_i128(v)).collect()
}
pub fn twotier_rats(d: &[(i128, i128)]) -> Vec<TRat> {
    d.iter().map(|&(n, dn)| Rat::new(n, dn)).collect()
}
pub fn dashu_rats(d: &[(i128, i128)]) -> Vec<BigRat> {
    d.iter()
        .map(|&(n, dn)| Bignum::rat_from_ints(Bignum::int_from_i128(n), Bignum::int_from_i128(dn)))
        .collect()
}

// -------- 1. integer dot product: Σ aᵢ·bᵢ (add + mul, stays in i128) --------

pub fn int_dot_twotier(a: &[TInt], b: &[TInt]) -> TInt {
    let mut acc = Int::from_i128(0);
    for (x, y) in a.iter().zip(b) {
        acc = acc.add(&x.mul(y));
    }
    acc
}
pub fn int_dot_dashu(a: &[BigInt], b: &[BigInt]) -> BigInt {
    let mut acc = Bignum::int_from_i128(0);
    for (x, y) in a.iter().zip(b) {
        acc = Bignum::int_add(&acc, &Bignum::int_mul(x, y));
    }
    acc
}

// -------- 2. rational 2×2 determinants: a·d − b·c (mul + sub + gcd-reduce) --------

pub fn rat_det_twotier(q: &[TRat]) -> u64 {
    let mut nz = 0;
    for w in q.chunks_exact(4) {
        let det = w[0].mul(&w[3]).sub(&w[1].mul(&w[2]));
        nz += (det.sign() != 0) as u64;
    }
    nz
}
pub fn rat_det_dashu(q: &[BigRat]) -> u64 {
    let mut nz = 0;
    for w in q.chunks_exact(4) {
        let det = Bignum::rat_sub(&Bignum::rat_mul(&w[0], &w[3]), &Bignum::rat_mul(&w[1], &w[2]));
        nz += (Bignum::rat_sign(&det) != 0) as u64;
    }
    nz
}

// -------- 3. rational comparison: count aᵢ < bᵢ (cross-radical cmp) --------

pub fn rat_cmp_twotier(q: &[TRat]) -> u64 {
    let mut lt = 0;
    for w in q.chunks_exact(2) {
        lt += (w[0].cmp(&w[1]) == Ordering::Less) as u64;
    }
    lt
}
pub fn rat_cmp_dashu(q: &[BigRat]) -> u64 {
    let mut lt = 0;
    for w in q.chunks_exact(2) {
        lt += (Bignum::rat_cmp(&w[0], &w[1]) == Ordering::Less) as u64;
    }
    lt
}

// -------- 4. crossover: running product that overflows i128 (promotion cost) --------

pub fn crossover_twotier(m: &[TInt]) -> TInt {
    let mut acc = Int::from_i128(1);
    for x in m {
        acc = acc.mul(x);
    }
    acc
}
pub fn crossover_dashu(m: &[BigInt]) -> BigInt {
    let mut acc = Bignum::int_from_i128(1);
    for x in m {
        acc = Bignum::int_mul(&acc, x);
    }
    acc
}
