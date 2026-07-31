//! Sturm sequences over ℚ — exact real-root counting, isolation, and
//! sign-on-interval (`docs/agent-glossary.md`; spec §8.1 "A/1D inequalities:
//! total", §8.3 azimuth). Pure, `no_std`, total.
//!
//! **Verification (vv-guide §0/§5/§8):** the variation theorem
//! (`#distinct real roots in (a,b] = V(a) − V(b)`) is cited, not re-proven; its
//! hypotheses are checked at runtime by [`SturmChain::verify_chain`] (a
//! runtime-checked hypothesis, `docs/proofs/ledger.md`). The only piece proven in
//! Kani is the finite [`sign_variations`] counter. The chain builder here is the
//! naive Euclidean ℚ-PRS (simplest, correct; fine at predicate degrees) — a
//! primitive/subresultant builder is a later perf-only swap, and because
//! `verify_chain` checks each entry is a *positive multiple* (not an exact
//! match), that swap touches neither the checker nor the counter.

use crate::backend::Backend;
use crate::bignum::Bignum;
use crate::poly::Poly;
use crate::rat::Rat;
use alloc::vec;
use alloc::vec::Vec;
use core::cmp::Ordering;

/// Number of sign variations in a sign sequence (`-1|0|1`), zeros skipped. Total,
/// `≤ signs.len().saturating_sub(1)`. The bounded object Kani proves and the
/// task-3 spike lifts to Lean.
pub fn sign_variations(signs: &[i8]) -> u32 {
    let mut last = 0i8;
    let mut v = 0u32;
    for &s in signs {
        if s != 0 {
            if last != 0 && s != last {
                v += 1;
            }
            last = s;
        }
    }
    v
}

/// A closed rational interval `[lo, hi]` (root counts are over the half-open
/// `(lo, hi]`, per Sturm's theorem).
pub struct Interval<B: Backend = Bignum> {
    pub lo: Rat<B>,
    pub hi: Rat<B>,
}

impl<B: Backend> Clone for Interval<B> {
    fn clone(&self) -> Self {
        Interval {
            lo: self.lo.clone(),
            hi: self.hi.clone(),
        }
    }
}

/// The Sturm chain `p₀ = p, p₁ = p', pₖ₊₁ = −(pₖ₋₁ mod pₖ)` (last entry the
/// gcd of `p, p'`). Distinct-real-root counts are exact via [`Self::count_in`].
pub struct SturmChain<B: Backend = Bignum> {
    pub(crate) chain: Vec<Poly<B>>,
}

fn abs<B: Backend>(r: &Rat<B>) -> Rat<B> {
    if r.sign() < 0 { r.neg() } else { r.clone() }
}

/// `u` is a strictly-positive rational multiple of `v` (`u = c·v, c > 0`), or
/// both zero. Fraction-free: `u·lead(v) == v·lead(u)` and the leads agree in sign.
fn pos_proportional<B: Backend>(u: &Poly<B>, v: &Poly<B>) -> bool {
    match (u.leading(), v.leading()) {
        (None, None) => true,
        (Some(lu), Some(lv)) => u.scale(lv) == v.scale(lu) && lu.sign() == lv.sign(),
        _ => false,
    }
}

impl<B: Backend> SturmChain<B> {
    /// Build the chain for `p` (naive Euclidean ℚ-PRS). Counts *distinct* roots
    /// even when `p` is not squarefree (the chain then ends at `gcd(p, p')`).
    pub fn new(p: &Poly<B>) -> Self {
        if p.degree().unwrap_or(0) == 0 {
            return SturmChain {
                chain: vec![p.clone()],
            };
        }
        let mut chain = vec![p.clone(), p.derivative()];
        loop {
            let n = chain.len();
            if chain[n - 1].degree() == Some(0) {
                break; // reached a nonzero constant — chain complete
            }
            let next = chain[n - 2].rem(&chain[n - 1]).neg();
            if next.is_zero() {
                break; // remainder 0 — last entry is gcd(p, p')
            }
            chain.push(next);
        }
        SturmChain { chain }
    }

    /// Runtime-checked hypothesis for the Sturm variation theorem: the stored
    /// chain really is a Sturm chain for `p` — `p₀ ∝₊ p`, `p₁ ∝₊ p'`, each
    /// `pₖ₊₁ ∝₊ −(pₖ₋₁ mod pₖ)`, strictly descending degrees, terminating
    /// (`pₙ₋₂ mod pₙ₋₁ == 0`, so the tail is `gcd(p, p')`). PRS-agnostic.
    pub fn verify_chain(&self, p: &Poly<B>) -> bool {
        let c = &self.chain;
        if c.is_empty() {
            return false;
        }
        if !pos_proportional(&c[0], p) {
            return false;
        }
        if c.len() == 1 {
            return p.degree() == Some(0); // constant p ⇒ single-entry chain
        }
        if !pos_proportional(&c[1], &p.derivative()) {
            return false;
        }
        for i in 1..c.len() - 1 {
            if c[i].degree() >= c[i - 1].degree() {
                return false;
            }
            let rem = c[i - 1].rem(&c[i]).neg();
            if !pos_proportional(&c[i + 1], &rem) {
                return false;
            }
        }
        let n = c.len();
        c[n - 1].degree() < c[n - 2].degree() && c[n - 2].rem(&c[n - 1]).is_zero()
    }

    fn signs_at(&self, x: &Rat<B>) -> Vec<i8> {
        self.chain.iter().map(|q| q.eval(x).sign()).collect()
    }

    /// Sign variations `V(x)` of the chain evaluated at a rational point.
    pub fn variations_at(&self, x: &Rat<B>) -> u32 {
        sign_variations(&self.signs_at(x))
    }

    fn variations_at_pos_inf(&self) -> u32 {
        let s: Vec<i8> = self
            .chain
            .iter()
            .map(|q| q.leading().map_or(0, |c| c.sign()))
            .collect();
        sign_variations(&s)
    }
    fn variations_at_neg_inf(&self) -> u32 {
        let s: Vec<i8> = self
            .chain
            .iter()
            .map(|q| {
                q.leading().map_or(0, |c| {
                    // sign of q(−∞) = sign(lead)·(−1)^deg
                    if q.degree().unwrap_or(0) % 2 == 1 {
                        -c.sign()
                    } else {
                        c.sign()
                    }
                })
            })
            .collect();
        sign_variations(&s)
    }

    /// Number of distinct real roots in the half-open `(lo, hi]`.
    pub fn count_in(&self, lo: &Rat<B>, hi: &Rat<B>) -> u32 {
        self.variations_at(lo)
            .saturating_sub(self.variations_at(hi))
    }
    /// Number of distinct real roots in the interval (`(lo, hi]`).
    pub fn root_count(&self, iv: &Interval<B>) -> u32 {
        self.count_in(&iv.lo, &iv.hi)
    }
    /// Total number of distinct real roots (`V(−∞) − V(+∞)`).
    pub fn count_all(&self) -> u32 {
        self.variations_at_neg_inf()
            .saturating_sub(self.variations_at_pos_inf())
    }

    /// A Cauchy bound `M` with every real root of `p₀` in `(−M, M)`:
    /// `1 + maxᵢ |aᵢ / aₙ|`.
    fn cauchy_bound(&self) -> Rat<B> {
        let p = &self.chain[0];
        let lead = match p.leading() {
            Some(l) => l,
            None => return Rat::from_i128(1),
        };
        let mut m = Rat::from_i128(0);
        for c in &p.coeffs[..p.coeffs.len().saturating_sub(1)] {
            let r = abs(&c.div(lead));
            if r.cmp(&m) == Ordering::Greater {
                m = r;
            }
        }
        Rat::from_i128(1).add(&m)
    }

    /// Isolate every distinct real root into a half-open `(lo, hi]` interval, each
    /// containing exactly one root. Bisection from the Cauchy bound; terminates by
    /// root separation.
    pub fn isolate_all(&self) -> Vec<Interval<B>> {
        let m = self.cauchy_bound();
        self.isolate(&Interval { lo: m.neg(), hi: m })
    }

    /// Isolate the distinct real roots inside `iv` (each into its own `(lo, hi]`).
    pub fn isolate(&self, iv: &Interval<B>) -> Vec<Interval<B>> {
        let two = Rat::from_i128(2);
        let mut out = Vec::new();
        let mut stack = vec![iv.clone()];
        while let Some(cur) = stack.pop() {
            match self.count_in(&cur.lo, &cur.hi) {
                0 => {}
                1 => out.push(cur),
                _ => {
                    let mid = cur.lo.add(&cur.hi).div(&two);
                    stack.push(Interval {
                        lo: cur.lo,
                        hi: mid.clone(),
                    });
                    stack.push(Interval {
                        lo: mid,
                        hi: cur.hi,
                    });
                }
            }
        }
        out
    }
}

/// `Some(interior sign)` if `q` is single-signed on the half-open `(lo, hi]`
/// (no distinct real root there), else `None`. The sign-on-interval primitive
/// REG/CLIP predicates consume.
pub fn sign_on_interval<B: Backend>(q: &Poly<B>, iv: &Interval<B>) -> Option<i8> {
    let sc = SturmChain::new(q);
    if sc.count_in(&iv.lo, &iv.hi) == 0 {
        Some(q.eval(&iv.hi).sign()) // no root in (lo, hi] ⇒ q(hi) ≠ 0
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type P = Poly<Bignum>;
    type Q = Rat<Bignum>;

    fn from_roots(roots: &[i128]) -> P {
        // ∏ (x − r)
        let mut p = P::constant(Q::from_i128(1));
        for &r in roots {
            p = p.mul(&P::from_coeffs(vec![Q::from_i128(-r), Q::from_i128(1)]));
        }
        p
    }
    fn iv(lo: i128, hi: i128) -> Interval<Bignum> {
        Interval {
            lo: Q::from_i128(lo),
            hi: Q::from_i128(hi),
        }
    }

    #[test]
    fn sign_variations_basic() {
        assert_eq!(sign_variations(&[1, 1, 1]), 0);
        assert_eq!(sign_variations(&[1, -1, 1]), 2);
        assert_eq!(sign_variations(&[1, 0, -1, 0, 1]), 2); // zeros skipped
        assert_eq!(sign_variations(&[]), 0);
    }

    #[test]
    fn count_and_isolate_known_roots() {
        // (x+2)·x·(x−1)·(x−3): distinct real roots {−2, 0, 1, 3}
        let p = from_roots(&[-2, 0, 1, 3]);
        let sc = SturmChain::new(&p);
        assert!(sc.verify_chain(&p));
        assert_eq!(sc.count_all(), 4);
        assert_eq!(sc.count_in(&Q::from_i128(0), &Q::from_i128(4)), 2); // (0,4]: roots 1,3
        assert_eq!(sc.root_count(&iv(-10, 10)), 4);

        let isolated = sc.isolate_all();
        assert_eq!(isolated.len(), 4);
        // each interval holds exactly one root
        for i in &isolated {
            assert_eq!(sc.count_in(&i.lo, &i.hi), 1);
        }
    }

    #[test]
    fn irrational_and_no_roots() {
        // x² − 2: two real roots ±√2, isolate √2 into (1, 2]
        let sq2 = P::from_coeffs(vec![Q::from_i128(-2), Q::from_i128(0), Q::from_i128(1)]);
        let sc = SturmChain::new(&sq2);
        assert!(sc.verify_chain(&sq2));
        assert_eq!(sc.count_all(), 2);
        assert_eq!(sc.count_in(&Q::from_i128(1), &Q::from_i128(2)), 1);
        assert_eq!(sc.count_in(&Q::from_i128(-2), &Q::from_i128(2)), 2);

        // x² + 1: no real roots; single-signed positive everywhere
        let x2p1 = P::from_coeffs(vec![Q::from_i128(1), Q::from_i128(0), Q::from_i128(1)]);
        assert_eq!(SturmChain::new(&x2p1).count_all(), 0);
        assert_eq!(sign_on_interval(&x2p1, &iv(-5, 5)), Some(1));
        assert_eq!(sign_on_interval(&sq2, &iv(-2, 2)), None); // has roots
    }

    #[test]
    fn rational_root_isolated() {
        // (x − 1/2)(x² − 2): roots {1/2, ±√2}
        let half = P::from_coeffs(vec![Q::new(-1, 2), Q::from_i128(1)]); // x − 1/2
        let sq2 = P::from_coeffs(vec![Q::from_i128(-2), Q::from_i128(0), Q::from_i128(1)]);
        let p = half.mul(&sq2);
        let sc = SturmChain::new(&p);
        assert!(sc.verify_chain(&p));
        assert_eq!(sc.count_all(), 3);
        assert_eq!(sc.isolate_all().len(), 3);
    }

    #[test]
    fn verify_chain_rejects_tampered() {
        let p = from_roots(&[-1, 2]);
        let mut sc = SturmChain::new(&p);
        assert!(sc.verify_chain(&p));
        // corrupt an entry → checker must reject
        sc.chain[1] = P::constant(Q::from_i128(7));
        assert!(!sc.verify_chain(&p));
    }

    use proptest::prelude::*;
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// Random ∏(x−rᵢ)·(x²+1)ᵉ (distinct integer rᵢ; the quadratics add no real
        /// roots): count_all == #distinct integer roots, and verify_chain holds —
        /// a differential against constructed ground truth over random polynomials.
        #[test]
        fn count_matches_constructed_roots(
            roots in prop::collection::btree_set(-30i128..=30, 0..5),
            extra in 0usize..3,
        ) {
            let x2p1 = P::from_coeffs(vec![Q::from_i128(1), Q::from_i128(0), Q::from_i128(1)]);
            let mut p = P::constant(Q::from_i128(1));
            for &r in &roots {
                p = p.mul(&from_roots(&[r]));
            }
            for _ in 0..extra {
                p = p.mul(&x2p1);
            }
            let sc = SturmChain::new(&p);
            prop_assert!(sc.verify_chain(&p));
            prop_assert_eq!(sc.count_all(), roots.len() as u32);
        }
    }
}
