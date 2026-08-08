//! Real algebraic numbers with **total** exact comparison (spec §8.1: "A/1D
//! inequalities: total"; the three-valued `Unresolved` is certify-core's A/nD
//! strict-sign concern, not here). Pure, `no_std`, no floats.
//!
//! - [`Surd`] — L2 `a + b√d` (spec §2.2 D24: degree-≤2 intersections stay
//!   in-lattice), compared in closed form by sign-tracked squaring.
//! - [`AlgReal`] — general (L3) `(squarefree defining polynomial, isolating
//!   interval)`, compared by interval refinement (Sturm) with an exact
//!   common-factor equality test — the glossary's "interval-plus-separation".
//!
//! Margins on √-carrying quantities are squared: [`Surd::square`] feeds
//! `certify-core`'s `MarginSq<Rat>` (spec §8.2).

use crate::backend::Backend;
use crate::bignum::Bignum;
use crate::poly::Poly;
use crate::rat::Rat;
use crate::sturm::{Interval, SturmChain};
use alloc::vec;
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::fmt;

fn ord(i: i8) -> Ordering {
    i.cmp(&0)
}

/// Sign of `a + b√d` for rationals `a, b` and `d ≥ 0` (√d ≥ 0). Exact, by
/// sign-tracked squaring — no radical is ever materialized.
fn sign_ab<B: Backend>(a: &Rat<B>, b: &Rat<B>, d: &Rat<B>) -> i8 {
    let (sa, sb) = (a.sign(), b.sign());
    if sb == 0 || d.is_zero() {
        return sa; // b√d = 0
    }
    if sa == 0 {
        return sb; // sign = sign(b√d) = sign(b) since √d > 0
    }
    if sa == sb {
        return sa; // both terms the same sign
    }
    // Disagree: sign = sa·sign(a² − b²d) (compare magnitudes by squaring).
    let disc = a.mul(a).sub(&b.mul(b).mul(d));
    sa * disc.sign()
}

// ===========================================================================
// L2 — Surd = a + b√d
// ===========================================================================

/// The real algebraic number `a + b√d`, with `a, b ∈ ℚ` and `d ∈ ℚ≥0`.
pub struct Surd<B: Backend = Bignum> {
    pub(crate) a: Rat<B>,
    pub(crate) b: Rat<B>,
    pub(crate) d: Rat<B>,
}

impl<B: Backend> Surd<B> {
    /// `a + b√d`; `d` must be `≥ 0` (debug-checked).
    pub fn new(a: Rat<B>, b: Rat<B>, d: Rat<B>) -> Self {
        debug_assert!(d.sign() >= 0, "Surd: d must be ≥ 0");
        Surd { a, b, d }
    }
    /// A rational as a degenerate surd `a + 0√0`.
    pub fn from_rat(a: Rat<B>) -> Self {
        Surd {
            a,
            b: Rat::from_i128(0),
            d: Rat::from_i128(0),
        }
    }
    /// `-1 | 0 | 1`.
    pub fn sign(&self) -> i8 {
        sign_ab(&self.a, &self.b, &self.d)
    }
    /// The defining triple `(a, b, d)` of `a + b√d` — the surd's mathematical
    /// identity. Exposed for **diagnostics rendering** (the `export` crate
    /// approximates `a + b·√d` as a display float); each component is an
    /// opaque-tier [`Rat`], so this leaks no representation, only the algebraic
    /// form.
    pub fn parts(&self) -> (&Rat<B>, &Rat<B>, &Rat<B>) {
        (&self.a, &self.b, &self.d)
    }
    /// The squared value `(a+b√d)² = (a²+b²d) + (2ab)√d`, for `MarginSq<Rat>` use.
    pub fn square(&self) -> Surd<B> {
        let ab = self.a.mul(&self.b);
        Surd {
            a: self.a.mul(&self.a).add(&self.b.mul(&self.b).mul(&self.d)),
            b: ab.add(&ab),
            d: self.d.clone(),
        }
    }
    /// A squarefree defining polynomial + isolating interval for this number.
    // PANIC-FREEDOM: the `unreachable!` below is a fail-fast Sturm-isolation guard, discharged
    // by argument — not a total fallback (a wrong value would mask the regression). See
    // docs/trusted-invariants.md (`Surd::to_algreal`).
    #[allow(clippy::unreachable)]
    pub fn to_algreal(&self) -> AlgReal<B> {
        if self.b.is_zero() || self.d.is_zero() {
            return AlgReal::from_rat(&self.a);
        }
        // root of (x−a)² − b²d = x² − 2a·x + (a² − b²d); the two roots are a ± |b|√d.
        let a2 = self.a.mul(&self.a);
        let b2d = self.b.mul(&self.b).mul(&self.d);
        let two_a = self.a.add(&self.a);
        let poly = Poly::from_coeffs(vec![a2.sub(&b2d), two_a.neg(), Rat::from_i128(1)]);
        for iv in SturmChain::new(&poly).isolate_all() {
            // pick the interval (lo, hi] that contains a + b√d
            let above_lo = sign_ab(&self.a.sub(&iv.lo), &self.b, &self.d) > 0;
            let below_hi = sign_ab(&self.a.sub(&iv.hi), &self.b, &self.d) <= 0;
            if above_lo && below_hi {
                return AlgReal { poly, iv };
            }
        }
        // The loop always returns: `a+b√d` is a root of `poly`, and `isolate_all` covers every
        // real root of `poly` (Sturm), so exactly one interval contains it. See the fn-level
        // PANIC-FREEDOM tag / docs/trusted-invariants.md.
        unreachable!("a+b√d must lie in one isolating interval")
    }

    // --- exact field arithmetic (spec §2.2 D24: degree-≤2 stays in-lattice) ---
    //
    // The arrangement's carrier-intersection coordinates all share one radical
    // `d = Δ`, so `add`/`sub`/`mul` stay closed in `Surd` on that path. A
    // genuinely cross-radical combination (`d₁ ≠ d₂`, both irrational) leaves the
    // degree-2 field, so it escalates to `AlgReal` (degree ≤ 4) — hence the `Alg`
    // return. `scale`/`neg` are always in-field.

    /// True iff the radical term vanishes (`b = 0` or `d = 0`) — a rational value.
    fn is_rational(&self) -> bool {
        self.b.is_zero() || self.d.is_zero()
    }

    /// `k · (a + b√d)` for a rational `k` — closed in the same radical field.
    pub fn scale(&self, k: &Rat<B>) -> Surd<B> {
        Surd {
            a: self.a.mul(k),
            b: self.b.mul(k),
            d: self.d.clone(),
        }
    }

    /// `-(a + b√d)`.
    pub fn neg(&self) -> Surd<B> {
        Surd {
            a: self.a.neg(),
            b: self.b.neg(),
            d: self.d.clone(),
        }
    }

    /// `self + o`: a [`Surd`] when the radicals are compatible (rational operand,
    /// or equal `d`), else an [`AlgReal`] (cross-radical, degree 4).
    pub fn add(&self, o: &Self) -> Alg<B> {
        let a = self.a.add(&o.a);
        match (self.is_rational(), o.is_rational()) {
            (true, true) => Alg::Surd(Surd::from_rat(a)),
            (true, false) => Alg::Surd(Surd {
                a,
                b: o.b.clone(),
                d: o.d.clone(),
            }),
            (false, true) => Alg::Surd(Surd {
                a,
                b: self.b.clone(),
                d: self.d.clone(),
            }),
            (false, false) => {
                if self.d.cmp(&o.d) == Ordering::Equal {
                    Alg::Surd(Surd {
                        a,
                        b: self.b.add(&o.b),
                        d: self.d.clone(),
                    })
                } else {
                    Alg::Alg(self.cross_add(o))
                }
            }
        }
    }

    /// `self − o` (see [`Surd::add`]).
    pub fn sub(&self, o: &Self) -> Alg<B> {
        self.add(&o.neg())
    }

    /// `self · o` (see [`Surd::add`] for the radical-field discipline).
    pub fn mul(&self, o: &Self) -> Alg<B> {
        match (self.is_rational(), o.is_rational()) {
            (true, _) => Alg::Surd(o.scale(&self.a)),
            (_, true) => Alg::Surd(self.scale(&o.a)),
            (false, false) => {
                if self.d.cmp(&o.d) == Ordering::Equal {
                    // (a₁+b₁√d)(a₂+b₂√d) = (a₁a₂+b₁b₂d) + (a₁b₂+a₂b₁)√d
                    Alg::Surd(Surd {
                        a: self.a.mul(&o.a).add(&self.b.mul(&o.b).mul(&self.d)),
                        b: self.a.mul(&o.b).add(&o.a.mul(&self.b)),
                        d: self.d.clone(),
                    })
                } else {
                    Alg::Alg(self.cross_mul(o))
                }
            }
        }
    }

    /// Cross-radical `self + o` (`d₁ ≠ d₂`, both irrational). The four conjugates
    /// `(a₁±b₁√d₁)+(a₂±b₂√d₂)` are the roots of `(x−A)⁴ − 2P(x−A)² + (P²−Q²R)`,
    /// rational in `A=a₁+a₂`, `P=b₁²d₁+b₂²d₂`, `Q=2b₁b₂`, `R=d₁d₂`.
    fn cross_add(&self, o: &Self) -> AlgReal<B> {
        let a = self.a.add(&o.a);
        let p = self
            .b
            .mul(&self.b)
            .mul(&self.d)
            .add(&o.b.mul(&o.b).mul(&o.d));
        let q = {
            let t = self.b.mul(&o.b);
            t.add(&t)
        };
        let r = self.d.mul(&o.d);
        let one = Rat::from_i128(1);
        let xa = Poly::from_coeffs(vec![a.neg(), one]); // x − A
        let xa2 = xa.mul(&xa);
        let xa4 = xa2.mul(&xa2);
        let minpoly = xa4
            .sub(&xa2.scale(&p.add(&p)))
            .add(&Poly::constant(p.mul(&p).sub(&q.mul(&q).mul(&r))));
        isolate_sum(&minpoly, self.to_algreal(), o.to_algreal())
    }

    /// Cross-radical `self · o` (`d₁ ≠ d₂`, both irrational). The four product
    /// conjugates are the roots of `(x²−2m·x+N)² − 4n²R·x²`, rational in `m=a₁a₂`,
    /// `N=(a₁²−b₁²d₁)(a₂²−b₂²d₂)`, `n=b₁b₂`, `R=d₁d₂`.
    fn cross_mul(&self, o: &Self) -> AlgReal<B> {
        let m = self.a.mul(&o.a);
        let n = self
            .a
            .mul(&self.a)
            .sub(&self.b.mul(&self.b).mul(&self.d))
            .mul(&o.a.mul(&o.a).sub(&o.b.mul(&o.b).mul(&o.d)));
        let nn = self.b.mul(&o.b);
        let r = self.d.mul(&o.d);
        let one = Rat::from_i128(1);
        let zero = Rat::from_i128(0);
        let quad = Poly::from_coeffs(vec![n, m.add(&m).neg(), one]); // x² − 2m·x + N
        let four_n2r = {
            let t = nn.mul(&nn).mul(&r);
            let t2 = t.add(&t);
            t2.add(&t2)
        };
        let x2 = Poly::from_coeffs(vec![zero.clone(), zero, four_n2r]);
        isolate_prod(&quad.mul(&quad).sub(&x2), self.to_algreal(), o.to_algreal())
    }

    fn cmp_impl(&self, o: &Self) -> Ordering {
        // sign(self − o) with self − o = A + b₁√d₁ − b₂√d₂, A = a₁ − a₂.
        let a = self.a.sub(&o.a);
        let sp = sign_ab(&a, &self.b, &self.d); // sign(P), P = A + b₁√d₁
        let sq = sign_ab(&Rat::from_i128(0), &o.b, &o.d); // sign(Q), Q = b₂√d₂
        if sp != sq {
            return sp.cmp(&sq); // sign(P − Q) = sign(sP − sQ)
        }
        if sp == 0 {
            return Ordering::Equal;
        }
        // same nonzero sign s: sign(P − Q) = s·sign(P² − Q²); P²−Q² has a single
        // radical √d₁: (A² + b₁²d₁ − b₂²d₂) + (2A·b₁)√d₁.
        let c = a
            .mul(&a)
            .add(&self.b.mul(&self.b).mul(&self.d))
            .sub(&o.b.mul(&o.b).mul(&o.d));
        let ab1 = a.mul(&self.b);
        let e = ab1.add(&ab1); // 2A·b₁
        ord(sp * sign_ab(&c, &e, &self.d))
    }
}

impl<B: Backend> Clone for Surd<B> {
    fn clone(&self) -> Self {
        Surd {
            a: self.a.clone(),
            b: self.b.clone(),
            d: self.d.clone(),
        }
    }
}
impl<B: Backend> PartialEq for Surd<B> {
    fn eq(&self, o: &Self) -> bool {
        self.cmp_impl(o) == Ordering::Equal
    }
}
impl<B: Backend> Eq for Surd<B> {}
impl<B: Backend> PartialOrd for Surd<B> {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl<B: Backend> Ord for Surd<B> {
    fn cmp(&self, o: &Self) -> Ordering {
        self.cmp_impl(o)
    }
}
impl<B: Backend> fmt::Debug for Surd<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Surd({:?} + {:?}√{:?})", self.a, self.b, self.d)
    }
}

/// The result of exact [`Surd`] field arithmetic ([`Surd::add`] / [`Surd::mul`]):
/// a [`Surd`] when the operation stayed in one radical field (the arrangement
/// carrier path, where every coordinate shares `d = Δ`), or an [`AlgReal`] for a
/// genuinely cross-radical (degree-≤4) combination.
pub enum Alg<B: Backend = Bignum> {
    /// Stayed in the degree-≤2 field.
    Surd(Surd<B>),
    /// Escaped to a general (L3) algebraic number.
    Alg(AlgReal<B>),
}

impl<B: Backend> Alg<B> {
    /// The [`Surd`] when the arithmetic stayed in one radical field, else `None` (the
    /// cross-radical [`AlgReal`] escape). **Total** — the pure tier never panics; the caller
    /// decides how to treat an escape. The arrangement carrier/membership path never
    /// escalates (all coordinates share `d = Δ`), so it unwraps the `Some` in the shell tier.
    pub fn try_surd(self) -> Option<Surd<B>> {
        match self {
            Alg::Surd(s) => Some(s),
            Alg::Alg(_) => None,
        }
    }

    /// `-1 | 0 | 1`.
    pub fn sign(&self) -> i8 {
        match self {
            Alg::Surd(s) => s.sign(),
            Alg::Alg(a) => a.sign(),
        }
    }
}

/// Isolate `α + β` (cross-radical surds) among the roots of `minpoly` by refining
/// the operands' intervals until their sum-interval holds exactly one root.
fn isolate_sum<B: Backend>(minpoly: &Poly<B>, mut a: AlgReal<B>, mut b: AlgReal<B>) -> AlgReal<B> {
    let sf = minpoly.squarefree_part();
    let sc = SturmChain::new(&sf);
    loop {
        let lo = a.iv.lo.add(&b.iv.lo);
        let hi = a.iv.hi.add(&b.iv.hi);
        if sc.count_in(&lo, &hi) == 1 {
            return AlgReal {
                poly: sf,
                iv: Interval { lo, hi },
            };
        }
        a.refine();
        b.refine();
    }
}

/// Isolate `α · β` among the roots of `minpoly` (the product-interval is the
/// min/max of the four corner products; refine until it holds exactly one root).
fn isolate_prod<B: Backend>(minpoly: &Poly<B>, mut a: AlgReal<B>, mut b: AlgReal<B>) -> AlgReal<B> {
    let sf = minpoly.squarefree_part();
    let sc = SturmChain::new(&sf);
    loop {
        let corners = [
            a.iv.lo.mul(&b.iv.lo),
            a.iv.lo.mul(&b.iv.hi),
            a.iv.hi.mul(&b.iv.lo),
            a.iv.hi.mul(&b.iv.hi),
        ];
        // `corners` is a fixed 4-element array, so `min`/`max` are always `Some`; the
        // fallback is unreachable but keeps this total (no `unwrap`).
        let lo = corners
            .iter()
            .min()
            .cloned()
            .unwrap_or_else(|| corners[0].clone());
        let hi = corners
            .iter()
            .max()
            .cloned()
            .unwrap_or_else(|| corners[0].clone());
        if sc.count_in(&lo, &hi) == 1 {
            return AlgReal {
                poly: sf,
                iv: Interval { lo, hi },
            };
        }
        a.refine();
        b.refine();
    }
}

// ===========================================================================
// L3 — AlgReal = (squarefree defining polynomial, isolating interval)
// ===========================================================================

/// A real algebraic number as a squarefree polynomial with a rational interval
/// `(lo, hi]` isolating exactly one of its real roots. Total comparison via
/// Sturm interval refinement + exact common-factor equality.
pub struct AlgReal<B: Backend = Bignum> {
    pub(crate) poly: Poly<B>,
    pub(crate) iv: Interval<B>,
}

impl<B: Backend> AlgReal<B> {
    /// The rational `q` as `x − q` isolated by the point `[q, q]`.
    pub fn from_rat(q: &Rat<B>) -> Self {
        AlgReal {
            poly: Poly::from_coeffs(vec![q.neg(), Rat::from_i128(1)]),
            iv: Interval {
                lo: q.clone(),
                hi: q.clone(),
            },
        }
    }
    /// Every distinct real root of `poly`, as isolated algebraic numbers.
    pub fn isolate_roots(poly: &Poly<B>) -> Vec<Self> {
        let sf = poly.squarefree_part();
        SturmChain::new(&sf)
            .isolate_all()
            .into_iter()
            .map(|iv| AlgReal {
                poly: sf.clone(),
                iv,
            })
            .collect()
    }

    /// Halve the isolating interval, keeping the half that holds the root.
    fn refine(&mut self) {
        if self.iv.lo.cmp(&self.iv.hi) == Ordering::Equal {
            return; // exact rational root; already a point
        }
        let mid = self.iv.lo.add(&self.iv.hi).div(&Rat::from_i128(2));
        let sc = SturmChain::new(&self.poly);
        if sc.count_in(&self.iv.lo, &mid) == 1 {
            self.iv.hi = mid;
        } else {
            self.iv.lo = mid;
        }
    }

    /// `self` vs the rational `q` (total).
    pub fn cmp_rat(&self, q: &Rat<B>) -> Ordering {
        // q is the isolated root itself?
        if self.poly.eval(q).is_zero()
            && q.cmp(&self.iv.lo) == Ordering::Greater
            && q.cmp(&self.iv.hi) != Ordering::Greater
        {
            return Ordering::Equal;
        }
        let mut a = self.clone();
        loop {
            if q.cmp(&a.iv.lo) != Ordering::Greater {
                return Ordering::Greater; // q ≤ lo < root
            }
            if q.cmp(&a.iv.hi) == Ordering::Greater {
                return Ordering::Less; // q > hi ≥ root
            }
            a.refine();
        }
    }

    /// `-1 | 0 | 1`.
    pub fn sign(&self) -> i8 {
        match self.cmp_rat(&Rat::from_i128(0)) {
            Ordering::Less => -1,
            Ordering::Equal => 0,
            Ordering::Greater => 1,
        }
    }

    fn cmp_impl(&self, o: &Self) -> Ordering {
        // equal iff a common factor has its (unique) root in the interval overlap
        let g = self.poly.gcd(&o.poly);
        if g.degree().is_some_and(|d| d >= 1) {
            let lo = max(&self.iv.lo, &o.iv.lo);
            let hi = min(&self.iv.hi, &o.iv.hi);
            if lo.cmp(hi) != Ordering::Greater && SturmChain::new(&g).count_in(lo, hi) >= 1 {
                return Ordering::Equal;
            }
        }
        // distinct: refine both until the intervals separate
        let mut a = self.clone();
        let mut b = o.clone();
        loop {
            if a.iv.hi.cmp(&b.iv.lo) != Ordering::Greater {
                return Ordering::Less; // a ≤ hi_a ≤ lo_b < b
            }
            if b.iv.hi.cmp(&a.iv.lo) != Ordering::Greater {
                return Ordering::Greater;
            }
            a.refine();
            b.refine();
        }
    }
}

fn max<'a, B: Backend>(x: &'a Rat<B>, y: &'a Rat<B>) -> &'a Rat<B> {
    if x.cmp(y) == Ordering::Greater { x } else { y }
}
fn min<'a, B: Backend>(x: &'a Rat<B>, y: &'a Rat<B>) -> &'a Rat<B> {
    if x.cmp(y) == Ordering::Less { x } else { y }
}

impl<B: Backend> Clone for AlgReal<B> {
    fn clone(&self) -> Self {
        AlgReal {
            poly: self.poly.clone(),
            iv: self.iv.clone(),
        }
    }
}
impl<B: Backend> PartialEq for AlgReal<B> {
    fn eq(&self, o: &Self) -> bool {
        self.cmp_impl(o) == Ordering::Equal
    }
}
impl<B: Backend> Eq for AlgReal<B> {}
impl<B: Backend> PartialOrd for AlgReal<B> {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl<B: Backend> Ord for AlgReal<B> {
    fn cmp(&self, o: &Self) -> Ordering {
        self.cmp_impl(o)
    }
}
impl<B: Backend> fmt::Debug for AlgReal<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AlgReal({:?} in ({:?}, {:?}])",
            self.poly, self.iv.lo, self.iv.hi
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Q = Rat<Bignum>;
    type S = Surd<Bignum>;
    fn surd(a: i128, b: i128, d: i128) -> S {
        S::new(Q::from_i128(a), Q::from_i128(b), Q::from_i128(d))
    }

    #[test]
    fn surd_sign_and_compare() {
        assert_eq!(surd(0, 1, 2).sign(), 1); // √2 > 0
        assert_eq!(surd(1, -1, 2).sign(), -1); // 1 − √2 < 0
        assert_eq!(surd(3, -2, 2).sign(), 1); // 3 − 2√2 ≈ 0.17 > 0
        // √2 vs 17/12: (17/12)² = 289/144 > 2 ⇒ √2 < 17/12
        assert_eq!(
            surd(0, 1, 2).cmp(&S::from_rat(Q::new(17, 12))),
            Ordering::Less
        );
        // 1+√2 vs 1+√3 (cross-radical, d₁≠d₂)
        assert_eq!(surd(1, 1, 2).cmp(&surd(1, 1, 3)), Ordering::Less);
        // golden ratio (1+√5)/2 vs 13/8
        assert_eq!(
            surd(0, 1, 5).cmp(&S::from_rat(Q::new(9, 4))),
            Ordering::Less
        ); // √5 < 9/4
    }

    #[test]
    fn surd_cross_radical_equality() {
        // √8 == 2√2 — the sharp squaring test (d₁≠d₂, both equal)
        assert_eq!(surd(0, 1, 8), surd(0, 2, 2));
        assert_eq!(surd(0, 1, 8).cmp(&surd(0, 2, 2)), Ordering::Equal);
        // √4 == 2 (perfect square vs rational)
        assert_eq!(surd(0, 1, 4), S::from_rat(Q::from_i128(2)));
    }

    #[test]
    fn surd_arithmetic_same_radical() {
        let s = surd(1, 1, 2); // 1 + √2
        assert_eq!(s.scale(&Q::from_i128(2)), surd(2, 2, 2)); // 2 + 2√2
        assert_eq!(s.neg(), surd(-1, -1, 2)); // −1 − √2
        // add / sub with equal radical
        assert_eq!(s.add(&surd(3, 2, 2)).try_surd().unwrap(), surd(4, 3, 2)); // 4 + 3√2
        assert_eq!(surd(3, 2, 2).sub(&s).try_surd().unwrap(), surd(2, 1, 2)); // 2 + √2
        // rational operand stays in-field
        assert_eq!(
            s.add(&S::from_rat(Q::from_i128(3))).try_surd().unwrap(),
            surd(4, 1, 2)
        );
        assert_eq!(
            s.mul(&S::from_rat(Q::from_i128(2))).try_surd().unwrap(),
            surd(2, 2, 2)
        );
        // (1 + √2)² = 3 + 2√2
        assert_eq!(s.mul(&s).try_surd().unwrap(), surd(3, 2, 2));
    }

    #[test]
    fn surd_arithmetic_cross_radical() {
        // √2 + √3 (root of x⁴ − 10x² + 1) ≈ 3.14626
        match surd(0, 1, 2).add(&surd(0, 1, 3)) {
            Alg::Alg(r) => {
                assert_eq!(r.sign(), 1);
                assert_eq!(r.cmp_rat(&Q::from_i128(3)), Ordering::Greater);
                assert_eq!(r.cmp_rat(&Q::new(63, 20)), Ordering::Less); // < 3.15
            }
            Alg::Surd(_) => panic!("√2 + √3 must be cross-radical"),
        }
        // √2 · √3 = √6 (escalates, but exactly equals the surd 0 + 1√6)
        match surd(0, 1, 2).mul(&surd(0, 1, 3)) {
            Alg::Alg(r) => {
                assert_eq!(r.cmp(&surd(0, 1, 6).to_algreal()), Ordering::Equal);
            }
            Alg::Surd(_) => panic!("√2 · √3 must escalate (different radicals)"),
        }
    }

    #[test]
    fn algreal_roots_and_rational_compare() {
        // roots of x² − 2 are ±√2
        let roots = AlgReal::isolate_roots(&Poly::from_coeffs(vec![
            Q::from_i128(-2),
            Q::from_i128(0),
            Q::from_i128(1),
        ]));
        assert_eq!(roots.len(), 2);
        // the positive one equals the L2 √2
        let pos = if roots[0].cmp_rat(&Q::from_i128(0)) == Ordering::Greater {
            &roots[0]
        } else {
            &roots[1]
        };
        assert_eq!(pos, &surd(0, 1, 2).to_algreal());
        assert_eq!(pos.cmp_rat(&Q::new(17, 12)), Ordering::Less); // √2 < 17/12
        assert_eq!(pos.cmp_rat(&Q::new(7, 5)), Ordering::Greater); // √2 > 7/5 = 1.4
    }

    #[test]
    fn algreal_cross_poly_equality_and_order() {
        // √2 as a root of x²−2, and as a root of x⁴−4 — the same real number
        let a = &AlgReal::isolate_roots(&Poly::from_coeffs(vec![
            Q::from_i128(-2),
            Q::from_i128(0),
            Q::from_i128(1),
        ]))[1]; // positive root of x²−2 (isolate_all order: lo→? pick > 0 below)
        let x4m4 = Poly::from_coeffs(vec![
            Q::from_i128(-4),
            Q::from_i128(0),
            Q::from_i128(0),
            Q::from_i128(0),
            Q::from_i128(1),
        ]); // x⁴ − 4 = (x²−2)(x²+2): real roots ±√2
        let b_roots = AlgReal::isolate_roots(&x4m4);
        // pick the ones that are positive and compare equal to √2
        let sqrt2 = surd(0, 1, 2).to_algreal();
        assert!(b_roots.contains(&sqrt2));
        // cube root of 2 (x³−2) vs rationals: 1.259...
        let cbrt2 = &AlgReal::isolate_roots(&Poly::from_coeffs(vec![
            Q::from_i128(-2),
            Q::from_i128(0),
            Q::from_i128(0),
            Q::from_i128(1),
        ]))[0];
        assert_eq!(cbrt2.cmp_rat(&Q::from_i128(1)), Ordering::Greater);
        assert_eq!(cbrt2.cmp_rat(&Q::from_i128(2)), Ordering::Less);
        assert_eq!(cbrt2.cmp_rat(&Q::new(63, 50)), Ordering::Less); // 1.26 > 1.259...
        let _ = a;
    }

    // Differential oracle: compare two surds by refining rational √-bounds, and
    // check it agrees with the closed-form Surd::cmp (no float anywhere).
    fn sqrt_bounds(d: &Q, steps: u32) -> (Q, Q) {
        let two = Q::from_i128(2);
        let mut lo = Q::from_i128(0);
        let mut hi = if d.cmp(&Q::from_i128(1)) == Ordering::Greater {
            d.clone()
        } else {
            Q::from_i128(1)
        };
        for _ in 0..steps {
            let m = lo.add(&hi).div(&two);
            if m.mul(&m).cmp(d) == Ordering::Greater {
                hi = m;
            } else {
                lo = m;
            }
        }
        (lo, hi)
    }
    fn surd_bounds(s: &S, steps: u32) -> (Q, Q) {
        let (slo, shi) = sqrt_bounds(&s.d, steps);
        let (blo, bhi) = (s.b.mul(&slo), s.b.mul(&shi));
        let (lo, hi) = if s.b.sign() >= 0 {
            (blo, bhi)
        } else {
            (bhi, blo)
        };
        (s.a.add(&lo), s.a.add(&hi))
    }
    fn cmp_oracle(x: &S, y: &S) -> Ordering {
        for steps in [16u32, 32, 48, 64, 96, 160] {
            let (xl, xh) = surd_bounds(x, steps);
            let (yl, yh) = surd_bounds(y, steps);
            if xh.cmp(&yl) == Ordering::Less {
                return Ordering::Less;
            }
            if yh.cmp(&xl) == Ordering::Less {
                return Ordering::Greater;
            }
        }
        Ordering::Equal // unseparated after refinement ⇒ conjecture equal
    }

    use proptest::prelude::*;
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(400))]

        /// Surd::cmp (closed form) agrees with the interval-refinement oracle.
        #[test]
        fn surd_cmp_matches_interval_oracle(
            a1 in -20i128..=20, b1 in -20i128..=20, d1 in 0i128..=20,
            a2 in -20i128..=20, b2 in -20i128..=20, d2 in 0i128..=20,
        ) {
            let x = surd(a1, b1, d1);
            let y = surd(a2, b2, d2);
            prop_assert_eq!(x.cmp(&y), cmp_oracle(&x, &y));
        }
    }
}
