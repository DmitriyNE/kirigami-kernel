//! Rigorous rational enclosures of the elementary transcendentals the cone
//! development needs — `arctan`, `π`, `cos`, `sin`, `√` — all over ℚ.
//!
//! The development map is transcendental, but the *certificate* must stay
//! float-free (`vv-guide` "Milestone E · Doctrine"). Every function here returns
//! a [`RatIv`] — a closed rational interval `[lo, hi]` proven to contain the true
//! value — built by a **truncated series with an explicit, rational tail bound**
//! (alternating-series bracket for `arctan`/`cos`/`sin`; bisection for `√`). The
//! interval *width* is the certified error; refining the `terms`/`iters` budget
//! shrinks it toward zero.
//!
//! ```
//! use develop::interval::arctan;
//! use lattice::{Bignum, Rat};
//!
//! // arctan(1) = π/4 ≈ 0.785398…; the tight enclosure sits inside [0.785, 0.786].
//! let iv = arctan::<Bignum>(&Rat::from_i128(1), 24);
//! assert!(*iv.lo() >= Rat::new(785, 1000) && *iv.hi() <= Rat::new(786, 1000));
//! assert!(iv.width() < Rat::new(1, 1_000_000));
//! ```

use lattice::{Backend, Rat};

/// Series-internal rounding budget (DEV.2a): every intermediate enclosure is snapped
/// *outward* to a denominator dividing `2^ROUND_BITS`, so digit growth is bounded at
/// any term budget while containment is preserved. `60` gives ~18-digit denominators
/// and a per-op error `≤ 2^−60 ≈ 8.7e−19` — far below any fab tolerance.
pub const ROUND_BITS: u32 = 60;

/// A closed rational interval `[lo, hi]` with `lo ≤ hi`, used as a *certified
/// enclosure*: the true (possibly transcendental) value is proven to lie inside.
#[derive(Debug, PartialEq, Eq)]
pub struct RatIv<B: Backend = lattice::Bignum> {
    lo: Rat<B>,
    hi: Rat<B>,
}

// Hand-written so `B` need not be `Clone` (the backend marker types are not) —
// `Rat<B>` is `Clone` regardless. Mirrors `lattice::Interval`'s manual impl.
impl<B: Backend> Clone for RatIv<B> {
    fn clone(&self) -> Self {
        RatIv {
            lo: self.lo.clone(),
            hi: self.hi.clone(),
        }
    }
}

fn min2<B: Backend>(a: Rat<B>, b: Rat<B>) -> Rat<B> {
    if a <= b { a } else { b }
}
fn max2<B: Backend>(a: Rat<B>, b: Rat<B>) -> Rat<B> {
    if a >= b { a } else { b }
}

impl<B: Backend> RatIv<B> {
    /// The interval `[lo, hi]`; the endpoints are sorted, so the caller need not.
    pub fn new(lo: Rat<B>, hi: Rat<B>) -> Self {
        if lo <= hi {
            RatIv { lo, hi }
        } else {
            RatIv { lo: hi, hi: lo }
        }
    }
    /// The degenerate interval `[r, r]` (an exact rational value).
    pub fn point(r: Rat<B>) -> Self {
        RatIv {
            lo: r.clone(),
            hi: r,
        }
    }
    /// The tightest interval containing both `a` and `b`.
    pub fn hull(a: Rat<B>, b: Rat<B>) -> Self {
        RatIv::new(a, b)
    }
    /// The lower endpoint.
    pub fn lo(&self) -> &Rat<B> {
        &self.lo
    }
    /// The upper endpoint.
    pub fn hi(&self) -> &Rat<B> {
        &self.hi
    }
    /// The certified error: `hi − lo ≥ 0`.
    pub fn width(&self) -> Rat<B> {
        self.hi.sub(&self.lo)
    }
    /// The exact rational midpoint `(lo + hi)/2`.
    pub fn mid(&self) -> Rat<B> {
        self.lo.add(&self.hi).mul(&Rat::new(1, 2))
    }
    /// Whether `r ∈ [lo, hi]`.
    pub fn contains(&self, r: &Rat<B>) -> bool {
        self.lo <= *r && *r <= self.hi
    }
    /// `[lo, hi] + [c, d] = [lo+c, hi+d]`.
    pub fn add(&self, o: &Self) -> Self {
        RatIv {
            lo: self.lo.add(&o.lo),
            hi: self.hi.add(&o.hi),
        }
    }
    /// `[lo, hi] − [c, d] = [lo−d, hi−c]`.
    pub fn sub(&self, o: &Self) -> Self {
        RatIv {
            lo: self.lo.sub(&o.hi),
            hi: self.hi.sub(&o.lo),
        }
    }
    /// `−[lo, hi] = [−hi, −lo]`.
    pub fn neg(&self) -> Self {
        RatIv {
            lo: self.hi.neg(),
            hi: self.lo.neg(),
        }
    }
    /// Scale by an exact rational (sign-aware, so the result stays ordered).
    pub fn scale(&self, k: &Rat<B>) -> Self {
        if k.sign() >= 0 {
            RatIv {
                lo: self.lo.mul(k),
                hi: self.hi.mul(k),
            }
        } else {
            RatIv {
                lo: self.hi.mul(k),
                hi: self.lo.mul(k),
            }
        }
    }
    /// Interval product `[lo, hi]·[c, d]` — the hull of the four corner products.
    pub fn mul(&self, o: &Self) -> Self {
        let p = [
            self.lo.mul(&o.lo),
            self.lo.mul(&o.hi),
            self.hi.mul(&o.lo),
            self.hi.mul(&o.hi),
        ];
        let mut lo = p[0].clone();
        let mut hi = p[0].clone();
        for q in &p[1..] {
            lo = min2(lo, q.clone());
            hi = max2(hi, q.clone());
        }
        RatIv { lo, hi }
    }
    /// The smallest interval containing both `self` and `o`.
    pub fn hull_with(&self, o: &Self) -> Self {
        RatIv::new(
            min2(self.lo.clone(), o.lo.clone()),
            max2(self.hi.clone(), o.hi.clone()),
        )
    }
    /// Widen to `[⌊lo·2^bits⌋/2^bits, ⌈hi·2^bits⌉/2^bits]` — an enclosing interval whose
    /// endpoints have denominator dividing `2^bits` (`bits` capped at 62). Outward, so
    /// containment holds; the standard fixed-precision-rounding step that keeps the
    /// series intermediates bounded-digit.
    pub fn round_out(&self, bits: u32) -> Self {
        let scale = Rat::from_i128(1i128 << bits.min(62));
        RatIv {
            lo: self.lo.mul(&scale).floor().div(&scale),
            hi: self.hi.mul(&scale).ceil().div(&scale),
        }
    }
    /// [`round_out`](Self::round_out) at the default [`ROUND_BITS`] budget.
    pub fn rounded(&self) -> Self {
        self.round_out(ROUND_BITS)
    }
}

// --- arctan --------------------------------------------------------------------------------

/// `arctan(t)` for `|t| ≤ 1/2` via the Maclaurin series `Σ (−1)ᵏ t²ᵏ⁺¹/(2k+1)`.
///
/// For `|t| ≤ 1/2` the term magnitudes strictly decrease, so the series is
/// alternating and consecutive partial sums `Sₙ, Sₙ₊₁` bracket the limit — a
/// rigorous rational enclosure with width `≤ |t|²ⁿ⁺¹/(2n+1)`.
fn arctan_small<B: Backend>(t: &Rat<B>, terms: usize) -> RatIv<B> {
    let t2 = t.mul(t);
    let zero = RatIv::point(Rat::from_i128(0));
    let mut s = zero.clone();
    let mut prev = zero;
    // The partial sum and the running power `t^{2k+1}` are carried as *intervals*,
    // rounded outward each step so their denominators stay bounded (DEV.2a). Each
    // interval still encloses the exact partial sum, so `hull(Sₙ, Sₙ₊₁)` remains a
    // rigorous alternating-series bracket.
    let mut power = RatIv::point(t.clone()); // t^{2k+1}, k = 0 → t
    for k in 0..=terms.max(1) {
        let mut mag = power.scale(&Rat::new(1, (2 * k + 1) as i128)).rounded();
        if k % 2 == 1 {
            mag = mag.neg();
        }
        prev = s.clone();
        s = s.add(&mag).rounded();
        power = power.scale(&t2).rounded();
    }
    prev.hull_with(&s) // [Sₙ, Sₙ₊₁]
}

/// A certified enclosure of `arctan(x)` for any rational `x`, tight for the
/// `terms` budget.
///
/// Odd-symmetry folds `x < 0`; the addition formula reduces `1/2 < x ≤ 1` to two
/// small-argument series (`arctan ½ + arctan y`, `|y| ≤ 1/3`); and
/// `arctan x = π/2 − arctan(1/x)` reduces `x > 1`. So the fast small-argument
/// series carries every case.
pub fn arctan<B: Backend>(x: &Rat<B>, terms: usize) -> RatIv<B> {
    let half = Rat::new(1, 2);
    let one = Rat::from_i128(1);
    match x.sign() {
        0 => RatIv::point(Rat::from_i128(0)),
        s if s < 0 => arctan(&x.neg(), terms).neg(),
        _ => {
            if *x <= half {
                arctan_small(x, terms)
            } else if *x <= one {
                // arctan x = arctan ½ + arctan y, y = (x − ½)/(1 + x/2) ∈ (0, 1/3].
                let y = x.sub(&half).div(&one.add(&x.mul(&half)));
                arctan_small(&half, terms).add(&arctan_small(&y, terms))
            } else {
                pi_half(terms).sub(&arctan(&x.recip(), terms))
            }
        }
    }
}

/// A certified enclosure of `π/4 = arctan ½ + arctan ⅓` (both small-argument,
/// fast).
pub fn pi_quarter<B: Backend>(terms: usize) -> RatIv<B> {
    arctan_small(&Rat::new(1, 2), terms).add(&arctan_small(&Rat::new(1, 3), terms))
}

/// A certified enclosure of `π/2`.
pub fn pi_half<B: Backend>(terms: usize) -> RatIv<B> {
    pi_quarter(terms).scale(&Rat::from_i128(2))
}

/// A certified enclosure of `π`.
pub fn pi<B: Backend>(terms: usize) -> RatIv<B> {
    pi_quarter(terms).scale(&Rat::from_i128(4))
}

// --- cos / sin -----------------------------------------------------------------------------

/// The smallest index from which the Maclaurin term magnitudes for argument `p`
/// are strictly decreasing (so an alternating-series bracket is valid): the least
/// `n` with `(2n+a)(2n+b) > p²`.
fn decreasing_from<B: Backend>(p2: &Rat<B>, a: usize, b: usize) -> usize {
    let mut n = 0usize;
    loop {
        let prod = Rat::from_i128(((2 * n + a) * (2 * n + b)) as i128);
        if prod > *p2 {
            return n;
        }
        n += 1;
    }
}

/// A certified enclosure of `cos(p)` for `p ≥ 0` via `Σ (−1)ᵏ p²ᵏ/(2k)!`.
///
/// The series is summed to an index past the point where its terms start
/// decreasing (`(2k+1)(2k+2) > p²`), where consecutive partial sums bracket the
/// limit. The factorial denominator makes the width collapse very fast.
pub fn cos_at<B: Backend>(p: &Rat<B>, terms: usize) -> RatIv<B> {
    let p2 = p.mul(p);
    let n = terms.max(decreasing_from(&p2, 1, 2) + 2);
    let zero = RatIv::point(Rat::from_i128(0));
    let mut s = zero.clone();
    let mut prev = zero;
    let mut pow = RatIv::point(Rat::from_i128(1)); // p^{2k}, k = 0 → 1
    let mut recip = Rat::from_i128(1); // 1/(2k)!
    for k in 0..=n {
        let mut mag = pow.scale(&recip).rounded();
        if k % 2 == 1 {
            mag = mag.neg();
        }
        prev = s.clone();
        s = s.add(&mag).rounded();
        pow = pow.scale(&p2).rounded();
        recip = recip.div(&Rat::from_i128(((2 * k + 1) * (2 * k + 2)) as i128));
    }
    prev.hull_with(&s)
}

/// A certified enclosure of `sin(p)` for `p ≥ 0` via `Σ (−1)ᵏ p²ᵏ⁺¹/(2k+1)!`.
pub fn sin_at<B: Backend>(p: &Rat<B>, terms: usize) -> RatIv<B> {
    let p2 = p.mul(p);
    let n = terms.max(decreasing_from(&p2, 2, 3) + 2);
    let zero = RatIv::point(Rat::from_i128(0));
    let mut s = zero.clone();
    let mut prev = zero;
    let mut pow = RatIv::point(p.clone()); // p^{2k+1}, k = 0 → p
    let mut recip = Rat::from_i128(1); // 1/(2k+1)!
    for k in 0..=n {
        let mut mag = pow.scale(&recip).rounded();
        if k % 2 == 1 {
            mag = mag.neg();
        }
        prev = s.clone();
        s = s.add(&mag).rounded();
        pow = pow.scale(&p2).rounded();
        recip = recip.div(&Rat::from_i128(((2 * k + 2) * (2 * k + 3)) as i128));
    }
    prev.hull_with(&s)
}

/// A certified enclosure of `cos(θ)` for an *interval* `θ ⊆ [0, π]`.
///
/// `cos` is monotone decreasing on `[0, π]`, so the enclosure is the hull of the
/// endpoint enclosures. Outside `[0, π]` this returns the trivial `[−1, 1]`
/// (still rigorous).
pub fn cos_on<B: Backend>(theta: &RatIv<B>, terms: usize) -> RatIv<B> {
    if theta.lo.sign() < 0 || *theta.hi() > pi::<B>(terms).lo {
        return RatIv::new(Rat::from_i128(-1), Rat::from_i128(1));
    }
    RatIv::new(cos_at(theta.hi(), terms).lo, cos_at(theta.lo(), terms).hi)
}

/// A certified enclosure of `sin(θ)` for an *interval* `θ ⊆ [0, π]`.
///
/// `sin` increases on `[0, π/2]` and decreases on `[π/2, π]`. When `θ` lies on
/// one side, the enclosure is the endpoint hull; when it straddles `π/2` the
/// upper bound is the exact peak `1`. Outside `[0, π]` returns `[−1, 1]`.
pub fn sin_on<B: Backend>(theta: &RatIv<B>, terms: usize) -> RatIv<B> {
    if theta.lo.sign() < 0 || *theta.hi() > pi::<B>(terms).lo {
        return RatIv::new(Rat::from_i128(-1), Rat::from_i128(1));
    }
    let ph = pi_half::<B>(terms);
    if *theta.hi() <= ph.lo {
        // increasing branch
        RatIv::new(sin_at(theta.lo(), terms).lo, sin_at(theta.hi(), terms).hi)
    } else if *theta.lo() >= ph.hi {
        // decreasing branch
        RatIv::new(sin_at(theta.hi(), terms).lo, sin_at(theta.lo(), terms).hi)
    } else {
        // straddles π/2 — peak is 1
        let lo = min2(sin_at(theta.lo(), terms).lo, sin_at(theta.hi(), terms).lo);
        RatIv::new(lo, Rat::from_i128(1))
    }
}

// --- sqrt ----------------------------------------------------------------------------------

/// A certified enclosure of `√r` for `r ≥ 0`, refined by bisection until the
/// width is `< eps`.
///
/// The endpoints are rationals `lo, hi` with `lo² ≤ r ≤ hi²` — so the enclosure
/// is rigorous even when `√r` is irrational (a surd), which is the general cone's
/// ruling-speed radius `ρ = |n′|`.
///
/// ```
/// use develop::interval::sqrt;
/// use lattice::{Bignum, Rat};
///
/// // √2 ≈ 1.41421…; the endpoints bracket it (lo² ≤ 2 ≤ hi²) to width < 1e-6.
/// let two = sqrt::<Bignum>(&Rat::from_i128(2), &Rat::new(1, 1_000_000));
/// assert!(two.lo().mul(two.lo()) <= Rat::from_i128(2));
/// assert!(two.hi().mul(two.hi()) >= Rat::from_i128(2));
/// assert!(two.width() < Rat::new(1, 1_000_000));
/// ```
pub fn sqrt<B: Backend>(r: &Rat<B>, eps: &Rat<B>) -> RatIv<B> {
    if r.sign() <= 0 {
        return RatIv::point(Rat::from_i128(0));
    }
    let mut lo = Rat::from_i128(0);
    // √r ≤ r + 1: (r+1)² = r² + 2r + 1 ≥ r for r ≥ 0.
    let mut hi = r.add(&Rat::from_i128(1));
    let two = Rat::from_i128(2);
    while hi.sub(&lo) >= *eps {
        let mid = lo.add(&hi).div(&two);
        if mid.mul(&mid) <= *r {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    RatIv::new(lo, hi)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice::Bignum;

    type Q = Rat<Bignum>;

    fn close(iv: &RatIv<Bignum>, v: f64, tol: f64) -> bool {
        let lo = to_f64(iv.lo());
        let hi = to_f64(iv.hi());
        lo - tol <= v && v <= hi + tol
    }
    fn to_f64(r: &Q) -> f64 {
        let (n, d) = r.numer_denom_decimal();
        n.parse::<f64>().unwrap() / d.parse::<f64>().unwrap()
    }

    #[test]
    fn arctan_brackets_known_values() {
        // Tolerance absorbs f64 rounding of the endpoints — the interval itself is
        // rational and tighter than an f64 ULP at 24 terms.
        let tol = 1e-12;
        assert!(close(
            &arctan::<Bignum>(&Q::from_i128(1), 24),
            std::f64::consts::FRAC_PI_4,
            tol
        ));
        assert!(close(
            &arctan::<Bignum>(&Q::new(1, 2), 24),
            0.5f64.atan(),
            tol
        ));
        assert!(close(
            &arctan::<Bignum>(&Q::from_i128(3), 24),
            3.0f64.atan(),
            tol
        ));
        // odd symmetry, and the 1/2 < |x| ≤ 1 addition-formula reduction
        assert!(close(
            &arctan::<Bignum>(&Q::new(-2, 3), 24),
            (-2.0f64 / 3.0).atan(),
            tol
        ));
    }

    #[test]
    fn arctan_width_shrinks_with_terms() {
        let a = arctan::<Bignum>(&Q::from_i128(1), 6).width();
        let b = arctan::<Bignum>(&Q::from_i128(1), 20).width();
        assert!(b < a);
        assert!(b < Q::new(1, 1_000_000_000));
    }

    #[test]
    fn pi_enclosure_is_tight() {
        assert!(close(&pi::<Bignum>(24), std::f64::consts::PI, 0.0));
        assert!(pi::<Bignum>(24).width() < Q::new(1, 1_000_000_000));
    }

    #[test]
    fn cos_sin_point_brackets() {
        for &v in &[0.0f64, 0.3, 0.7, 1.2, 1.55, 2.1, 3.0] {
            let p = Q::new((v * 1_000_000.0) as i128, 1_000_000);
            assert!(close(&cos_at::<Bignum>(&p, 20), v.cos(), 1e-6), "cos {v}");
            assert!(close(&sin_at::<Bignum>(&p, 20), v.sin(), 1e-6), "sin {v}");
        }
    }

    #[test]
    fn cos_sin_on_interval_contains_endpoints() {
        // θ ∈ [0.4, 0.6] (below π/2): cos decreasing, sin increasing.
        let iv = RatIv::new(Q::new(4, 10), Q::new(6, 10));
        let c = cos_on::<Bignum>(&iv, 20);
        let s = sin_on::<Bignum>(&iv, 20);
        assert!(close(&c, 0.4f64.cos(), 1e-9) && close(&c, 0.6f64.cos(), 1e-9));
        assert!(close(&s, 0.4f64.sin(), 1e-9) && close(&s, 0.6f64.sin(), 1e-9));
    }

    #[test]
    fn sin_on_straddling_peak_is_one() {
        // θ ∈ [1.5, 1.65] straddles π/2 ≈ 1.5708 → sup is the exact 1.
        let iv = RatIv::new(Q::new(150, 100), Q::new(165, 100));
        let s = sin_on::<Bignum>(&iv, 20);
        assert_eq!(*s.hi(), Q::from_i128(1));
    }

    #[test]
    fn round_out_widens_to_bounded_denominator() {
        // A tight interval with an odd (non-dyadic) denominator.
        let iv = RatIv::new(Q::new(1, 7), Q::new(2, 7));
        let r = iv.round_out(10); // snap outward to a /2^10 grid
        assert!(*r.lo() <= *iv.lo() && *r.hi() >= *iv.hi(), "must widen (contain)");
        // Endpoints now have a denominator dividing 2^10 → at most 4 decimal digits.
        for endpoint in [r.lo(), r.hi()] {
            let (_, d) = endpoint.numer_denom_decimal();
            assert!(d.len() <= 4, "bounded denominator, got {d}");
        }
    }

    #[test]
    fn high_term_budget_stays_bounded_digit() {
        // DEV.2a: at a large term budget the certified endpoints stay bounded-digit
        // (denominator divides 2^ROUND_BITS) instead of exploding, and still bracket.
        let iv = arctan::<Bignum>(&Q::new(1, 2), 200);
        for endpoint in [iv.lo(), iv.hi()] {
            let (_, d) = endpoint.numer_denom_decimal();
            assert!(d.len() < 32, "denominator digit count bounded, got {}", d.len());
        }
        assert!(close(&iv, 0.5f64.atan(), 1e-12), "still brackets arctan(1/2)");
    }

    #[test]
    fn sqrt_brackets_and_narrows() {
        let s = sqrt::<Bignum>(&Q::from_i128(2), &Q::new(1, 1_000_000));
        assert!(s.lo().mul(s.lo()) <= Q::from_i128(2));
        assert!(s.hi().mul(s.hi()) >= Q::from_i128(2));
        assert!(s.width() < Q::new(1, 1_000_000));
        // perfect square: 144/9409 = (12/97)²
        let ps = sqrt::<Bignum>(&Q::new(144, 9409), &Q::new(1, 10_000_000));
        assert!(close(&ps, 12.0 / 97.0, 0.0));
    }
}
