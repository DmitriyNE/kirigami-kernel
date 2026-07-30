//! Generic degree-12 Sturm polynomial-remainder-sequence over rationals — the
//! speed yardstick's inner loop (chain construction dominates; NO root
//! isolation, that is the production Sturm's job, not this throwaway). A
//! polynomial is `Vec<R>` with `p[i]` the coefficient of xⁱ (highest index =
//! leading). Deterministic ~256-bit coefficients so all backends compute the
//! identical chain and must agree on the root count (a free cross-check).

use crate::backends::Rat;
use core::cmp::Ordering;

/// A deterministic integer-valued rational of ~300 bits, built by Horner in base
/// 10¹⁸ from an LCG seed (identical across backends).
fn big_int_rat<R: Rat>(mut s: u64) -> R {
    let base = R::from_i64(1_000_000_000_000_000_000); // 10^18 (~60 bits)
    let mut x = R::from_i64(1);
    for _ in 0..4 {
        // ~240 bits total (≈ the 256-bit yardstick)
        s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let digit = (s % 1_000_000_000_000_000_000) as i64;
        x = x.mul(&base).add(&R::from_i64(digit));
    }
    x
}

/// A ~256-bit rational: a big coprime-ish numerator over a big denominator (the
/// reduction only removes their gcd, so it stays large).
pub fn big256<R: Rat>(seed: u64) -> R {
    let num = big_int_rat::<R>(seed);
    let den = big_int_rat::<R>(seed ^ 0x9e37_79b9_7f4a_7c15);
    num.div(&den)
}

fn trim<R: Rat>(mut p: Vec<R>) -> Vec<R> {
    while p.len() > 1 && p.last().unwrap().is_zero() {
        p.pop();
    }
    p
}

fn deriv<R: Rat>(p: &[R]) -> Vec<R> {
    // d/dx Σ p[i] xⁱ = Σ_{i≥1} i·p[i] x^{i-1}
    let d: Vec<R> = (1..p.len())
        .map(|i| p[i].mul(&R::from_i64(i as i64)))
        .collect();
    if d.is_empty() {
        vec![R::zero()]
    } else {
        trim(d)
    }
}

/// Remainder of `a` divided by `b` over the rationals (b nonzero, leading coeff
/// used as pivot). Each step cancels the leading term, so it terminates.
fn rem<R: Rat>(a: &[R], b: &[R]) -> Vec<R> {
    let mut r = trim(a.to_vec());
    let db = b.len() - 1;
    let lead_b = b[db].clone();
    while r.len() > db && !(r.len() == 1 && r[0].is_zero()) {
        let dr = r.len() - 1;
        let factor = r[dr].div(&lead_b);
        let shift = dr - db;
        for i in 0..=db {
            r[shift + i] = r[shift + i].sub(&factor.mul(&b[i]));
        }
        r = trim(r);
    }
    r
}

/// The Sturm chain: p, p', then pₖ₊₁ = −(pₖ₋₁ mod pₖ) until a constant or zero.
pub fn sturm_chain<R: Rat>(p: &[R]) -> Vec<Vec<R>> {
    let mut chain = vec![trim(p.to_vec()), deriv(p)];
    loop {
        let n = chain.len();
        if chain[n - 1].len() == 1 {
            break; // degree-0: chain complete
        }
        let r = rem(&chain[n - 2], &chain[n - 1]);
        let neg = trim(r.iter().map(|c| R::zero().sub(c)).collect());
        if neg.len() == 1 && neg[0].is_zero() {
            break; // remainder 0: last entry is the gcd, stop
        }
        chain.push(neg);
    }
    chain
}

fn eval<R: Rat>(p: &[R], x: &R) -> R {
    let mut acc = R::zero();
    for c in p.iter().rev() {
        acc = acc.mul(x).add(c);
    }
    acc
}

fn sign_variations<R: Rat>(chain: &[Vec<R>], x: &R) -> u32 {
    let mut last = 0i32;
    let mut v = 0u32;
    for poly in chain {
        let s = match eval(poly, x).cmp0() {
            Ordering::Less => -1,
            Ordering::Greater => 1,
            Ordering::Equal => 0,
        };
        if s != 0 {
            if last != 0 && s != last {
                v += 1;
            }
            last = s;
        }
    }
    v
}

/// Number of distinct real roots in (−M, +M] via V(−M) − V(+M); the whole PRS is
/// the measured work, the count is the cross-backend checksum.
pub fn sturm_root_count<R: Rat>(poly: &[R]) -> u32 {
    let chain = sturm_chain(poly);
    let m = R::from_i64(1000);
    let neg_m = R::zero().sub(&m);
    sign_variations(&chain, &neg_m) - sign_variations(&chain, &m)
}
