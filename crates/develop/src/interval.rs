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

use lattice::{Backend, Poly, Rat, RatFunc};

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
    /// The reciprocal `1/[lo, hi] = [1/hi, 1/lo]` of a **strictly positive**
    /// interval (`lo > 0`), or `None` when the interval touches or crosses zero
    /// (where the reciprocal is unbounded). Used to divide by a surd radius `q`
    /// once its enclosure is tight enough to sign it.
    pub fn recip_pos(&self) -> Option<Self> {
        if self.lo.sign() > 0 {
            Some(RatIv {
                lo: self.hi.recip(),
                hi: self.lo.recip(),
            })
        } else {
            None
        }
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

// --- range reduction (mod 2π) + generic interval cos/sin -----------------------------------
//
// `develop`'s flat-development angle `ψ = c·arctan σ` crosses 0 and, for general
// placements, is not centered at 0 (`docs/roadmap-flex-pcb.md`, gap G1). The
// interval `cos`/`sin` therefore accept **any** real angle range, via mod-2π
// range reduction into `[−π, π]` followed by even/odd symmetry — the standard
// rigorous interval-trig, not a fixed window. A single rational chart's angle is
// always sub-period (`k` small), so the reduction is cheap and tight; `k = 0`
// (`|ψ| < π`, the per-chart common case) reduces nothing and reproduces the
// earlier `[0, π]` result exactly.

/// The nearest integer to `q` as an integer-valued `Rat` (ties toward `+∞`):
/// `⌊q + 1/2⌋`. Picks the reduction multiple `k ≈ x/2π`.
fn nearest_int<B: Backend>(q: &Rat<B>) -> Rat<B> {
    q.add(&Rat::new(1, 2)).floor()
}

/// A certified enclosure of `x mod 2π` with representative in `[−π, π]`: subtract
/// `k·2π` for the nearest integer `k = round(x/2π)`. `2π` is only known to an
/// enclosure, so the result is a (thin) interval, not a point; its width is
/// `|k|·width(2π)` — negligible for the small `k` a sub-2π chart ever needs. The
/// `k = 0` fast path (`|x| ≤ π`) subtracts nothing and adds no width.
fn reduce_point<B: Backend>(x: &Rat<B>, terms: usize) -> RatIv<B> {
    let pi_iv = pi::<B>(terms);
    let ax = if x.sign() < 0 { x.neg() } else { x.clone() };
    if ax <= *pi_iv.lo() {
        return RatIv::point(x.clone());
    }
    let tau = pi_iv.scale(&Rat::from_i128(2));
    let k = nearest_int(&x.div(&tau.mid()));
    RatIv::point(x.clone()).sub(&tau.scale(&k))
}

/// `cos` over an interval already reduced into `[−π, π]` (± a thin enclosure
/// slack). `cos` is even and decreasing in `|θ|` on `[0, π]`, so the infimum sits
/// at the largest `|θ|` (clamped to `−1` once `|θ|` can reach `π`) and the
/// supremum is the exact `1` when `θ` straddles `0`, else `cos` at the smallest
/// `|θ|` (capped at `1`).
fn cos_core<B: Backend>(xr: &RatIv<B>, terms: usize) -> RatIv<B> {
    let one = Rat::from_i128(1);
    let neg_one = Rat::from_i128(-1);
    let pilo = pi::<B>(terms).lo;
    let a = abs_on(xr);
    let inf = if *a.hi() >= pilo {
        neg_one
    } else {
        max2(cos_at(a.hi(), terms).lo, neg_one)
    };
    let sup = if xr.lo().sign() <= 0 && xr.hi().sign() >= 0 {
        one
    } else {
        min2(cos_at(a.lo(), terms).hi, one)
    };
    RatIv::new(inf, sup)
}

/// `sin` at a single rational via oddness `sin(−a) = −sin(a)`. [`RatIv::neg`]
/// swaps the endpoints, keeping the folded enclosure ordered.
fn sin_at_signed<B: Backend>(a: &Rat<B>, terms: usize) -> RatIv<B> {
    if a.sign() >= 0 {
        sin_at(a, terms)
    } else {
        sin_at(&a.neg(), terms).neg()
    }
}

/// `sin` over an interval already reduced into `[−π, π]`: monotone endpoint hull,
/// clamped to the exact peak `+1` when `θ` can reach `+π/2` and trough `−1` when
/// it can reach `−π/2`. Both clamps over-approximate containment against the
/// `π/2` enclosure — always sound since `sin ∈ [−1, 1]`.
fn sin_core<B: Backend>(xr: &RatIv<B>, terms: usize) -> RatIv<B> {
    let ph = pi_half::<B>(terms);
    let sl = sin_at_signed(xr.lo(), terms);
    let sh = sin_at_signed(xr.hi(), terms);
    let sup = if *xr.lo() <= *ph.hi() && *xr.hi() >= *ph.lo() {
        Rat::from_i128(1)
    } else {
        max2(sl.hi().clone(), sh.hi().clone())
    };
    let inf = if *xr.lo() <= ph.lo().neg() && *xr.hi() >= ph.hi().neg() {
        Rat::from_i128(-1)
    } else {
        min2(sl.lo().clone(), sh.lo().clone())
    };
    RatIv::new(inf, sup)
}

/// `cos(x)` for any rational `x`, via mod-2π reduction then [`cos_core`].
fn cos_pt<B: Backend>(x: &Rat<B>, terms: usize) -> RatIv<B> {
    cos_core(&reduce_point(x, terms), terms)
}

/// `sin(x)` for any rational `x`, via mod-2π reduction then [`sin_core`].
fn sin_pt<B: Backend>(x: &Rat<B>, terms: usize) -> RatIv<B> {
    sin_core(&reduce_point(x, terms), terms)
}

/// Whether the interval `θ` can contain an angle congruent to `base` modulo `2π`
/// — some `base + 2πm`. Over-approximates (overlap tested against the `base` and
/// `2π` enclosures), so a `±1` extremum clamp keyed on it never misses a
/// genuinely enclosed critical point. Only integer `m` within one step of the
/// aligning estimate are tested (a `θ` narrower than a period touches at most one
/// congruent angle).
fn contains_angle<B: Backend>(theta: &RatIv<B>, base: &RatIv<B>, terms: usize) -> bool {
    let tau = pi::<B>(terms).scale(&Rat::from_i128(2));
    let target = theta.mid().sub(&base.mid()).div(&tau.mid());
    let m0 = nearest_int(&target);
    for d in [-1i128, 0, 1] {
        let m = m0.add(&Rat::from_i128(d));
        let ang = base.add(&tau.scale(&m));
        if *theta.lo() <= *ang.hi() && *theta.hi() >= *ang.lo() {
            return true;
        }
    }
    false
}

/// A certified enclosure of `cos(θ)` for an *interval* `θ`, for **any** real angle
/// range — `develop`'s two-sided cone gore (`ψ = c·arctan σ` crossing 0).
///
/// Generic mod-2π range reduction: each endpoint is reduced into `[−π, π]` and
/// evaluated ([`cos_pt`]); the supremum is the exact `+1` when `θ` encloses an
/// even multiple of `π` and the infimum the exact `−1` at an odd multiple
/// ([`contains_angle`]), else the monotone endpoint hull. A `θ` at least a full
/// period wide is the honest full range `[−1, 1]`. On `θ ⊆ [0, π]` this reproduces
/// the earlier decreasing-hull result exactly.
pub fn cos_on<B: Backend>(theta: &RatIv<B>, terms: usize) -> RatIv<B> {
    let tau = pi::<B>(terms).scale(&Rat::from_i128(2));
    if theta.width() >= *tau.lo() {
        return RatIv::new(Rat::from_i128(-1), Rat::from_i128(1));
    }
    let el = cos_pt(theta.lo(), terms);
    let eh = cos_pt(theta.hi(), terms);
    let sup = if contains_angle(theta, &RatIv::point(Rat::from_i128(0)), terms) {
        Rat::from_i128(1)
    } else {
        max2(el.hi().clone(), eh.hi().clone())
    };
    let inf = if contains_angle(theta, &pi::<B>(terms), terms) {
        Rat::from_i128(-1)
    } else {
        min2(el.lo().clone(), eh.lo().clone())
    };
    RatIv::new(inf, sup)
}

/// A certified enclosure of `sin(θ)` for an *interval* `θ`, for **any** real angle
/// range. Generic mod-2π range reduction (see [`cos_on`]): endpoints via
/// [`sin_pt`]; the supremum is clamped to `+1` at a `+π/2 + 2πm` maximum and the
/// infimum to `−1` at a `−π/2 + 2πm` minimum ([`contains_angle`]), else the
/// endpoint hull; `[−1, 1]` once `θ` spans a full period. Reproduces the earlier
/// `[0, π]` dispatch exactly on that sub-domain.
pub fn sin_on<B: Backend>(theta: &RatIv<B>, terms: usize) -> RatIv<B> {
    let tau = pi::<B>(terms).scale(&Rat::from_i128(2));
    if theta.width() >= *tau.lo() {
        return RatIv::new(Rat::from_i128(-1), Rat::from_i128(1));
    }
    let el = sin_pt(theta.lo(), terms);
    let eh = sin_pt(theta.hi(), terms);
    let ph = pi_half::<B>(terms);
    let sup = if contains_angle(theta, &ph, terms) {
        Rat::from_i128(1)
    } else {
        max2(el.hi().clone(), eh.hi().clone())
    };
    let inf = if contains_angle(theta, &ph.neg(), terms) {
        Rat::from_i128(-1)
    } else {
        min2(el.lo().clone(), eh.lo().clone())
    };
    RatIv::new(inf, sup)
}

/// A certified enclosure of `arctan(θ)` for an *interval* argument `θ`.
///
/// `arctan` is strictly increasing on all of ℝ, so the enclosure is the hull of
/// the two endpoint enclosures `[arctan(lo).lo, arctan(hi).hi]`. Used when the
/// argument itself carries the surd `1/q` of a general (non-canonical) cone
/// placement (`develop::cone::angle_enclosure`).
pub fn arctan_on<B: Backend>(theta: &RatIv<B>, terms: usize) -> RatIv<B> {
    RatIv::new(
        arctan(theta.lo(), terms).lo().clone(),
        arctan(theta.hi(), terms).hi().clone(),
    )
}

// --- log -----------------------------------------------------------------------------------

/// A certified enclosure of `atanh(u) = Σ u²ᵏ⁺¹/(2k+1)` for `u ∈ [0, 1/3]`.
///
/// Every term is *positive*, so the partial sums increase to the limit — a
/// rigorous **lower** bracket. The omitted tail is bounded above geometrically:
/// with each `1/(2k+1) ≤ 1/(2n+3)` for `k ≥ n+1`,
/// `Σ_{k≥n+1} u²ᵏ⁺¹/(2k+1) ≤ u²ⁿ⁺³/((2n+3)(1−u²))`, giving the **upper** bracket.
/// For `u ≤ 1/3` the factor `(1−u²) ≥ 8/9 > 0`, so the bound is finite and tight.
fn atanh_small<B: Backend>(u: &Rat<B>, terms: usize) -> RatIv<B> {
    let u2 = u.mul(u);
    let n = terms.max(1);
    let mut s = RatIv::point(Rat::from_i128(0));
    let mut power = RatIv::point(u.clone()); // u^{2k+1}, k = 0 → u
    for k in 0..=n {
        let term = power.scale(&Rat::new(1, (2 * k + 1) as i128)).rounded();
        s = s.add(&term).rounded();
        power = power.scale(&u2).rounded(); // now u^{2(k+1)+1}
    }
    // After the loop `power` encloses u^{2n+3} (the first omitted term is k = n+1);
    // `power.hi ≥ u^{2n+3} ≥ 0` and the denominator is a positive rational, so
    // `tail_hi` is a rigorous upper bound on the omitted positive tail.
    let one = Rat::from_i128(1);
    let denom = one.sub(&u2).mul(&Rat::from_i128((2 * n + 3) as i128));
    let tail_hi = power.hi().div(&denom);
    RatIv::new(s.lo().clone(), s.hi().add(&tail_hi))
}

/// A certified enclosure of the natural logarithm `ln(x)` for `x > 0`.
///
/// Reduces `x = 2ᵐ·y` with `y ∈ [1, 2)` (powers of two factor out *exactly* in ℚ),
/// so `ln x = m·ln 2 + ln y`; then `ln y = 2·atanh(u)`, `u = (y−1)/(y+1) ∈ [0, 1/3]`
/// (geometric convergence), and `ln 2 = 2·atanh(1/3)` is enclosed by the same
/// series. Endpoints stay bounded-digit via the series' outward rounding. A
/// non-positive argument is out of the domain of `ln` and returns the sentinel
/// `[0, 0]` (never reached from a positive-definite denominator).
///
/// ```
/// use develop::interval::log;
/// use lattice::{Bignum, Rat};
///
/// // ln 2 ≈ 0.693147; the enclosure brackets it tightly.
/// let l2 = log::<Bignum>(&Rat::from_i128(2), 40);
/// assert!(*l2.lo() >= Rat::new(6931, 10000) && *l2.hi() <= Rat::new(6932, 10000));
/// assert!(l2.width() < Rat::new(1, 1_000_000));
/// ```
pub fn log<B: Backend>(x: &Rat<B>, terms: usize) -> RatIv<B> {
    if x.sign() <= 0 {
        return RatIv::point(Rat::from_i128(0));
    }
    let two = Rat::from_i128(2);
    let one = Rat::from_i128(1);
    // Reduce x = 2^m · y with y ∈ [1, 2); each step is an exact ℚ halving/doubling.
    let mut y = x.clone();
    let mut m: i128 = 0;
    while y >= two {
        y = y.div(&two);
        m += 1;
    }
    while y < one {
        y = y.mul(&two);
        m -= 1;
    }
    let u = y.sub(&one).div(&y.add(&one)); // ∈ [0, 1/3]
    let ln_y = atanh_small(&u, terms).scale(&two);
    if m == 0 {
        return ln_y;
    }
    let ln2 = atanh_small(&Rat::new(1, 3), terms).scale(&two);
    ln_y.add(&ln2.scale(&Rat::from_i128(m)))
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

/// A certified enclosure of `√[a, b]` for a non-negative interval: `[√a, √b]`
/// (`√` monotone). A lower endpoint that has rounded slightly below 0 is clamped
/// (`sqrt` returns `[0, 0]` there), keeping the result a valid `ρ ≥ 0` lower bound.
pub fn sqrt_on<B: Backend>(iv: &RatIv<B>, eps: &Rat<B>) -> RatIv<B> {
    RatIv::new(sqrt(iv.lo(), eps).lo, sqrt(iv.hi(), eps).hi)
}

/// The absolute value of an interval: `|[lo, hi]|`. Sign-uniform intervals map to
/// `[|·|min, |·|max]`; an interval straddling 0 has infimum `0`.
pub fn abs_on<B: Backend>(iv: &RatIv<B>) -> RatIv<B> {
    if iv.lo().sign() >= 0 {
        iv.clone()
    } else if iv.hi().sign() <= 0 {
        iv.neg()
    } else {
        let hi = max2(iv.lo().neg(), iv.hi().clone());
        RatIv::new(Rat::from_i128(0), hi)
    }
}

// --- interval polynomial / rational-function evaluation ------------------------------------

/// A certified enclosure of `p(x)` for an *interval* argument `x`, by Horner in
/// interval arithmetic (rounded outward each step so intermediates stay
/// bounded-digit). Rigorous: the true `p(x₀)` for any `x₀ ∈ x` lies in the result.
pub fn eval_poly_on<B: Backend>(p: &Poly<B>, x: &RatIv<B>) -> RatIv<B> {
    let mut acc = RatIv::point(Rat::from_i128(0));
    for c in p.coeffs().iter().rev() {
        acc = acc.mul(x).add(&RatIv::point(c.clone())).rounded();
    }
    acc
}

/// The same enclosure by the **mean-value form**, intersected with [`eval_poly_on`]'s.
///
/// Interval Horner evaluates every term independently, so a value that is a *small difference of
/// large terms* comes back with an enclosure the size of the terms rather than of the value. That is
/// not a rounding effect and it does not shrink with precision: measured on the device's tab-fillet
/// walls, the µ̂-discriminant `b² − 4ac` — a modest positive number — enclosed to
/// `[−3.0227e2, +5.8864e2]`, straddling zero, and every certificate that needed its **sign** was
/// lost (#292). Writing the same quantity as one polynomial and Horner-ing that does not help: the
/// cancellation moves into the coefficients and the dependency is in the arithmetic either way.
///
/// The mean-value form breaks it by evaluating the centre **exactly**:
///
/// ```text
///     p(X) ⊆ p(m) + p′(X)·(X − m),      m = mid X
/// ```
///
/// `p(m)` is a point — an exact rational, zero width, cancellation and all — and the dependency is
/// confined to `p′(X)`, where it is multiplied by the radius. So the enclosure width becomes `O(r)`
/// in the sub-interval radius instead of `O(‖terms‖)`, and refinement actually buys something.
///
/// Both forms are sound, so their **intersection** is too, and neither is uniformly tighter: Horner
/// wins on a wide interval where `p′` is badly behaved, the mean-value form wins wherever there is
/// cancellation to lose. Taking both costs one extra Horner pass over the derivative.
pub fn eval_poly_on_centred<B: Backend>(p: &Poly<B>, x: &RatIv<B>) -> RatIv<B> {
    let plain = eval_poly_on(p, x);
    let m = x.mid();
    let centre = p.eval(&m);
    let radius = x.hi().sub(x.lo()).mul(&Rat::new(1, 2));
    let slope = eval_poly_on(&p.derivative(), x);
    // `p′(X)·(X − m)` ⊆ `p′(X)·[−r, r]`, then shifted by the exact centre value.
    let spread = slope.mul(&RatIv::new(radius.neg(), radius)).rounded();
    let mv = RatIv::point(centre).add(&spread).rounded();
    let lo = if plain.lo().cmp(mv.lo()) == core::cmp::Ordering::Greater {
        plain.lo().clone()
    } else {
        mv.lo().clone()
    };
    let hi = if plain.hi().cmp(mv.hi()) == core::cmp::Ordering::Less {
        plain.hi().clone()
    } else {
        mv.hi().clone()
    };
    RatIv::new(lo, hi)
}

/// [`eval_ratfunc_on`] through [`eval_poly_on_centred`] — the cancellation-resistant reading.
///
/// `None` on the same condition: a denominator enclosure that straddles zero is a possible pole and
/// the caller refines or refuses. Note the centred form makes that *less* likely to fire spuriously,
/// since a denominator whose Horner enclosure straddles zero by cancellation alone no longer does.
pub fn eval_ratfunc_on_centred<B: Backend>(f: &RatFunc<B>, x: &RatIv<B>) -> Option<RatIv<B>> {
    let den = eval_poly_on_centred(f.den(), x);
    let inv = if den.lo().sign() > 0 || den.hi().sign() < 0 {
        RatIv::new(den.hi().recip(), den.lo().recip())
    } else {
        return None;
    };
    Some(eval_poly_on_centred(f.num(), x).mul(&inv).rounded())
}

/// A certified enclosure of `f(x) = num(x)/den(x)` for an *interval* argument `x`,
/// or `None` when `den(x)`'s enclosure straddles zero (a possible pole on the
/// sub-interval — the caller refines or refuses rather than risk an unbounded
/// quotient). Reuses [`eval_poly_on`] for both parts and interval reciprocal.
pub fn eval_ratfunc_on<B: Backend>(f: &RatFunc<B>, x: &RatIv<B>) -> Option<RatIv<B>> {
    let den = eval_poly_on(f.den(), x);
    // 1/den is bounded only when the denominator interval is sign-uniform.
    let inv = if den.lo().sign() > 0 || den.hi().sign() < 0 {
        RatIv::new(den.hi().recip(), den.lo().recip())
    } else {
        return None;
    };
    Some(eval_poly_on(f.num(), x).mul(&inv).rounded())
}

/// A certified enclosure of `∫_lo^σ f(s) ds` by an **interval Riemann sum** over `panels` equal
/// subintervals: each panel `[s_i, s_{i+1}]` contributes `f([s_i, s_{i+1}]) · width`, a sound
/// enclosure because `f(s) ∈ f([s_i, s_{i+1}])` for every `s` in the panel — so the sum contains
/// the true integral. The enclosure *width* is the certified quadrature error; it shrinks `∝
/// 1/panels` for a Lipschitz `f`. Returns `None` if a panel evaluation is `None` (a pole), if
/// `panels == 0`, or if `σ < lo`.
///
/// This is the DEV.3 "method (b)" primitive: the flat directrix `γ(σ) = ∫₀^σ e(ψ)·(pedal speed)`
/// is **non-elementary** (`rational × cos(c·arctan σ)` for a curved-support developable), so it is
/// enclosed by *validated quadrature* rather than a closed form — the honest transcendental frontier
/// the apex cone (`ψ` closed-form, `γ ≡ 0`) never reaches.
///
/// ```
/// use develop::interval::{RatIv, integrate_on};
/// use lattice::{Bignum, Rat};
///
/// // ∫₀¹ s² ds = 1/3 — the enclosure brackets it, and narrows as panels grow.
/// let sq = |iv: &RatIv<Bignum>| Some(iv.mul(iv));
/// let coarse = integrate_on(sq, &Rat::from_i128(0), &Rat::from_i128(1), 8).unwrap();
/// let fine = integrate_on(sq, &Rat::from_i128(0), &Rat::from_i128(1), 256).unwrap();
/// assert!(coarse.contains(&Rat::new(1, 3)) && fine.contains(&Rat::new(1, 3)));
/// assert!(fine.width() < coarse.width());
/// ```
pub fn integrate_on<B, F>(f: F, lo: &Rat<B>, sigma: &Rat<B>, panels: usize) -> Option<RatIv<B>>
where
    B: Backend,
    F: Fn(&RatIv<B>) -> Option<RatIv<B>>,
{
    use core::cmp::Ordering::Less;
    if panels == 0 || sigma.cmp(lo) == Less {
        return None;
    }
    let width = sigma.sub(lo).div(&Rat::from_i128(panels as i128));
    let mut acc = RatIv::point(Rat::from_i128(0));
    let mut a = lo.clone();
    for _ in 0..panels {
        let b = a.add(&width);
        let fv = f(&RatIv::new(a.clone(), b.clone()))?;
        acc = acc.add(&fv.scale(&width)).rounded();
        a = b;
    }
    Some(acc)
}

/// A certified enclosure of `∫_lo^σ f(s) ds` by a **verified midpoint rule with a first-derivative
/// (slope) remainder** — the higher-order successor to [`integrate_on`]. On each panel `[a, b]`
/// (`h = b − a`, midpoint `m`),
///
/// ```text
///   ∫_a^b f ds  =  f(m)·h  +  R ,      R = ∫_a^b (f(s) − f(m)) ds .
/// ```
///
/// The **main term** `f(m)·h` evaluates the integrand at the *thin point* `m` (`f` applied to the
/// degenerate interval `[m, m]`), so it carries none of the interval-*dependency* overestimation that
/// makes [`integrate_on`]'s panel-wide `f([a,b])` loose for a high-degree / large-coefficient
/// integrand — the exact failure that forced the self-lapping ramp down from a quintic to a cubic
/// support. The **remainder** is bounded rigorously from the mean-value theorem: for every `s ∈ [a,b]`,
/// `f(s) − f(m) = f′(ξ)(s − m)` with `ξ ∈ [a,b]`, so `f(s) − f(m) ∈ F′·(s − m)` where
/// `F′ = fprime([a,b])` encloses `f′` over the whole panel; integrating the set-valued bound (and using
/// `∫_a^b (s − m) ds = 0`, which cancels the leading term) gives the closed enclosure
///
/// ```text
///   R  ∈  [ −(h²/8)·w ,  (h²/8)·w ] ,      w = width(F′) .
/// ```
///
/// Since `w ≈ |f″|·h`, the per-panel remainder is `O(h³)` and the composite error is **`O(h²)`** —
/// quadratic, versus [`integrate_on`]'s `O(1/panels)`. `fprime` need only be a *sound* enclosure of
/// `f′` over a panel (it is multiplied by `h²/8`, so looseness there barely matters); the derivative
/// is supplied by the caller (e.g. via interval automatic differentiation), keeping this primitive
/// integrand-agnostic. Returns `None` if `panels == 0`, `σ < lo`, or either evaluation is `None`
/// (a pole).
///
/// ```
/// use develop::interval::{RatIv, cos_on, sin_on, integrate_on_slope};
/// use lattice::{Bignum, Rat};
///
/// // ∫₀¹ cos s ds = sin 1. f = cos, f′ = −sin — the slope rule brackets it and, at matched panels,
/// // is dramatically tighter than the first-order Riemann sum.
/// let cos = |iv: &RatIv<Bignum>| Some(cos_on(iv, 24));
/// let dcos = |iv: &RatIv<Bignum>| Some(sin_on(iv, 24).neg());
/// let iv = integrate_on_slope(cos, dcos, &Rat::from_i128(0), &Rat::from_i128(1), 64).unwrap();
/// assert!(iv.contains(&Rat::new(841_470, 1_000_000))); // ≈ sin 1 = 0.841471
/// assert!(iv.width() < Rat::new(1, 10_000)); // O(h²): ~6e-5 at 64 panels
/// ```
pub fn integrate_on_slope<B, F, D>(
    f: F,
    fprime: D,
    lo: &Rat<B>,
    sigma: &Rat<B>,
    panels: usize,
) -> Option<RatIv<B>>
where
    B: Backend,
    F: Fn(&RatIv<B>) -> Option<RatIv<B>>,
    D: Fn(&RatIv<B>) -> Option<RatIv<B>>,
{
    let [v] = integrate_on_slope_n(
        |iv| f(iv).map(|x| [x]),
        |iv| fprime(iv).map(|x| [x]),
        lo,
        sigma,
        panels,
    )?;
    Some(v)
}

/// [`integrate_on_slope`] for an **`N`-component** integrand, evaluated **once per point**.
///
/// The scalar form above is the `N = 1` case, so there is one implementation of the rule and no
/// second copy to drift.
///
/// This exists because integrating a vector-valued integrand component-by-component evaluates it
/// once *per component*, discarding the rest of each result. Measured on the acceptance device,
/// `γ` did exactly that — `directrix_velocity` and `directrix_accel` each return `[x, y]` and were
/// called twice per point, once keeping `[0]` and once keeping `[1]` — so **every γ integrand
/// evaluation happened twice**. That is invisible to a wall-clock reading and to a cell count; it
/// shows up only when the integrand evaluations themselves are counted
/// ([`counters::gamma_velocity`](crate::counters::gamma_velocity), which exists because of this).
pub fn integrate_on_slope_n<B, F, D, const N: usize>(
    f: F,
    fprime: D,
    lo: &Rat<B>,
    sigma: &Rat<B>,
    panels: usize,
) -> Option<[RatIv<B>; N]>
where
    B: Backend,
    F: Fn(&RatIv<B>) -> Option<[RatIv<B>; N]>,
    D: Fn(&RatIv<B>) -> Option<[RatIv<B>; N]>,
{
    use core::cmp::Ordering::Less;
    if panels == 0 || sigma.cmp(lo) == Less {
        return None;
    }
    let width = sigma.sub(lo).div(&Rat::from_i128(panels as i128));
    let two = Rat::from_i128(2);
    // h²/8 — the remainder scale; `∫_a^b (s−m) ds = 0` cancels the linear term, leaving this.
    let coeff = width.mul(&width).div(&Rat::from_i128(8));
    let mut acc: [RatIv<B>; N] = core::array::from_fn(|_| RatIv::point(Rat::from_i128(0)));
    let mut a = lo.clone();
    for _ in 0..panels {
        let b = a.add(&width);
        let m = a.add(&b).div(&two);
        let fm = f(&RatIv::point(m))?; // thin midpoint value — no dependency overestimation
        let fp = fprime(&RatIv::new(a.clone(), b.clone()))?;
        for i in 0..N {
            let bound = coeff.mul(&fp[i].width());
            let rem = RatIv::new(bound.neg(), bound);
            acc[i] = acc[i].add(&fm[i].scale(&width)).add(&rem).rounded();
        }
        a = b;
    }
    Some(acc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice::Bignum;

    type Q = Rat<Bignum>;

    /// **The mean-value form is `O(r)` where Horner is not, and it is never looser.**
    ///
    /// Two things are worth keeping straight, because conflating them cost a wrong first attempt at
    /// this test. Interval Horner's error on a polynomial in *monomial form* is **repeated-`x`
    /// dependency** — the coefficients are already combined, so there are no terms left to cancel —
    /// and it scales with the interval's powers rather than its width. The other problem, the one
    /// that actually loses the µ̂-discriminant's sign on the device's tab fillets (#292), is
    /// combining the enclosures of `a`, `b` and `c` *separately*: that throws away a cancellation
    /// the polynomial form never had to make. Forming the expression symbolically fixes the second;
    /// this form fixes the first.
    ///
    /// A high-degree polynomial far from the origin is where the difference shows.
    #[test]
    fn the_centred_form_is_first_order_in_the_interval_width() {
        // (x − 3)⁶ expanded — big alternating coefficients, evaluated near x = 3 where they cancel.
        let mut p = Poly::from_coeffs(vec![Q::from_i128(1)]);
        let root = Poly::from_coeffs(vec![Q::from_i128(-3), Q::from_i128(1)]);
        for _ in 0..6 {
            p = p.mul(&root);
        }
        let at = |w: i128| RatIv::new(Q::from_i128(3).sub(&Q::new(1, w)), Q::from_i128(3));

        for w in [1024i128, 4096] {
            let x = at(w);
            let (horner, centred) = (eval_poly_on(&p, &x), eval_poly_on_centred(&p, &x));
            // Sound, and never looser than Horner — the intersection guarantees the second.
            let truth = p.eval(&x.mid());
            assert!(horner.contains(&truth), "Horner must enclose the truth");
            assert!(
                centred.contains(&truth),
                "the centred form must enclose it too"
            );
            assert!(
                centred.width().cmp(&horner.width()) != core::cmp::Ordering::Greater,
                "at width 1/{w}: centred {} should not exceed Horner {}",
                to_f64(&centred.width()),
                to_f64(&horner.width())
            );
        }
        // And it is first order: quartering the interval quarters the width, up to a factor of two
        // of slack for the derivative enclosure's own dependency.
        let coarse = eval_poly_on_centred(&p, &at(1024)).width();
        let fine = eval_poly_on_centred(&p, &at(4096)).width();
        assert!(
            fine.mul(&Q::from_i128(2)).cmp(&coarse) == core::cmp::Ordering::Less,
            "quartering the interval must more than halve the centred width: {} vs {}",
            to_f64(&fine),
            to_f64(&coarse)
        );
    }

    fn close(iv: &RatIv<Bignum>, v: f64, tol: f64) -> bool {
        let lo = to_f64(iv.lo());
        let hi = to_f64(iv.hi());
        lo - tol <= v && v <= hi + tol
    }
    fn to_f64(r: &Q) -> f64 {
        let (n, d) = r.numer_denom_decimal();
        n.parse::<f64>().unwrap() / d.parse::<f64>().unwrap()
    }

    /// The validated quadrature brackets a *transcendental* integrand and narrows with panels:
    /// `∫₀^1 cos s ds = sin 1 ≈ 0.841471`. Exercises the `cos_on` enclosure inside `integrate_on`
    /// (the shape the flat directrix `γ = ∫ rational·cos ψ` takes).
    #[test]
    fn integrate_on_brackets_a_transcendental_integrand() {
        let cosf = |iv: &RatIv<Bignum>| Some(cos_on(iv, 24));
        let coarse = integrate_on(cosf, &Q::from_i128(0), &Q::from_i128(1), 16).unwrap();
        let fine = integrate_on(cosf, &Q::from_i128(0), &Q::from_i128(1), 512).unwrap();
        let want = 1.0f64.sin();
        assert!(close(&coarse, want, 1e-9), "coarse must bracket sin 1");
        assert!(close(&fine, want, 1e-9), "fine must bracket sin 1");
        assert!(
            fine.width() < coarse.width(),
            "the enclosure narrows with panels"
        );
        assert!(
            to_f64(&fine.width()) < 1e-2,
            "512 panels give a tight enclosure"
        );
    }

    /// SPIKE (task #216): the midpoint-slope rule converges **quadratically** and, at matched panels,
    /// is orders tighter than the first-order Riemann sum — the two properties that unlock the quintic
    /// ramp and the tighter γ. Integrand `∫₀¹ cos = sin 1`, with `f′ = −sin` supplied directly.
    #[test]
    fn integrate_on_slope_is_quadratic_and_beats_riemann() {
        let cos = |iv: &RatIv<Bignum>| Some(cos_on(iv, 24));
        let dcos = |iv: &RatIv<Bignum>| Some(sin_on(iv, 24).neg());
        let (lo, hi) = (Q::from_i128(0), Q::from_i128(1));
        let want = 1.0f64.sin();

        // Widths at a doubling panel ladder.
        let ws: Vec<f64> = [16usize, 32, 64, 128]
            .iter()
            .map(|&n| {
                let iv = integrate_on_slope(cos, dcos, &lo, &hi, n).unwrap();
                assert!(
                    close(&iv, want, 1e-12),
                    "slope rule must bracket sin 1 at {n}"
                );
                to_f64(&iv.width())
            })
            .collect();

        // Quadratic: doubling panels shrinks the width ≈ 4× (allow 3×–5× for rounding slack).
        for pair in ws.windows(2) {
            let ratio = pair[0] / pair[1];
            assert!(
                (3.0..=5.0).contains(&ratio),
                "expected ≈4× per doubling (quadratic), got {ratio}"
            );
        }

        // At matched panels the slope rule crushes the first-order Riemann sum.
        let riemann = integrate_on(cos, &lo, &hi, 64).unwrap();
        let slope = integrate_on_slope(cos, dcos, &lo, &hi, 64).unwrap();
        assert!(
            to_f64(&slope.width()) * 50.0 < to_f64(&riemann.width()),
            "slope ≥50× tighter at 64 panels: slope={} riemann={}",
            to_f64(&slope.width()),
            to_f64(&riemann.width())
        );
    }

    /// SPIKE (task #216): the dependency-taming that matters for the ramp. A large-coefficient
    /// polynomial integrand `f(s) = 200 s⁴ − 400 s³ + 210 s² − 10 s` (quintic-support-like magnitudes)
    /// is enclosed far tighter by the slope rule — its main term reads `f` at a *thin* midpoint, so the
    /// interval-Horner blowup that wrecks the Riemann panel enclosure never enters.
    #[test]
    fn integrate_on_slope_tames_large_coefficient_integrand() {
        let p = Poly::from_coeffs(
            [0, -10, 210, -400, 200]
                .iter()
                .map(|&c| Q::from_i128(c))
                .collect(),
        );
        let dp = p.derivative();
        let f = |iv: &RatIv<Bignum>| Some(eval_poly_on(&p, iv));
        let fp = |iv: &RatIv<Bignum>| Some(eval_poly_on(&dp, iv));
        let (lo, hi) = (Q::from_i128(0), Q::from_i128(1));
        // ∫₀¹ = 40 − 100 + 70 − 5 = 5.
        let want = 5.0;
        let n = 64;
        let riemann = integrate_on(f, &lo, &hi, n).unwrap();
        let slope = integrate_on_slope(f, fp, &lo, &hi, n).unwrap();
        assert!(
            close(&riemann, want, 1e-9) && close(&slope, want, 1e-9),
            "both bracket 5"
        );
        assert!(
            to_f64(&slope.width()) * 20.0 < to_f64(&riemann.width()),
            "slope ≥20× tighter on the big-coefficient integrand: slope={} riemann={}",
            to_f64(&slope.width()),
            to_f64(&riemann.width())
        );
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

    // --- G1: generic (mod-2π) interval cos/sin over the two-sided gore ----------

    #[test]
    fn cos_sin_on_two_sided_symmetric() {
        // θ ∈ [−0.6, 0.6] crosses 0 (both below π/2). cos peaks at 0 (exact 1);
        // sin brackets ±sin(0.6) and does not reach its peak.
        let iv = RatIv::new(Q::new(-6, 10), Q::new(6, 10));
        let c = cos_on::<Bignum>(&iv, 24);
        let s = sin_on::<Bignum>(&iv, 24);
        assert_eq!(*c.hi(), Q::from_i128(1), "cos peak at 0 is exact 1");
        assert!(close(&c, 0.6f64.cos(), 1e-9) && close(&c, (-0.6f64).cos(), 1e-9));
        assert!(close(&s, 0.6f64.sin(), 1e-9) && close(&s, (-0.6f64).sin(), 1e-9));
        assert!(*s.hi() < Q::from_i128(1) && *s.lo() > Q::from_i128(-1));
    }

    #[test]
    fn cos_sin_on_purely_negative() {
        // θ ∈ [−1.2, −0.8]: no ±π/2 or 0 inside, so pure monotone endpoint hulls.
        let iv = RatIv::new(Q::new(-12, 10), Q::new(-8, 10));
        let c = cos_on::<Bignum>(&iv, 24);
        let s = sin_on::<Bignum>(&iv, 24);
        assert!(close(&c, (-1.2f64).cos(), 1e-9) && close(&c, (-0.8f64).cos(), 1e-9));
        assert!(close(&s, (-1.2f64).sin(), 1e-9) && close(&s, (-0.8f64).sin(), 1e-9));
        assert!(*c.hi() < Q::from_i128(1), "no cos peak");
        assert!(*s.lo() > Q::from_i128(-1), "no sin trough");
    }

    #[test]
    fn sin_on_straddling_plus_half_pi_is_one() {
        // θ ∈ [1.4, 1.7] straddles +π/2 → sup is the exact 1.
        let iv = RatIv::new(Q::new(14, 10), Q::new(17, 10));
        let s = sin_on::<Bignum>(&iv, 24);
        assert_eq!(*s.hi(), Q::from_i128(1));
    }

    #[test]
    fn sin_on_straddling_minus_half_pi_is_minus_one() {
        // θ ∈ [−1.7, −1.4] straddles −π/2 → inf is the exact −1.
        let iv = RatIv::new(Q::new(-17, 10), Q::new(-14, 10));
        let s = sin_on::<Bignum>(&iv, 24);
        assert_eq!(*s.lo(), Q::from_i128(-1));
    }

    #[test]
    fn cos_on_straddling_zero_is_one() {
        // θ ∈ [−0.3, 0.5] straddles 0 → sup is the exact 1.
        let iv = RatIv::new(Q::new(-3, 10), Q::new(5, 10));
        let c = cos_on::<Bignum>(&iv, 24);
        assert_eq!(*c.hi(), Q::from_i128(1));
    }

    #[test]
    fn cos_on_straddling_pi_is_minus_one() {
        // θ ∈ [3.0, 3.3] straddles π ≈ 3.14159 → inf is the exact −1.
        let iv = RatIv::new(Q::from_i128(3), Q::new(33, 10));
        let c = cos_on::<Bignum>(&iv, 24);
        assert_eq!(*c.lo(), Q::from_i128(-1));
        assert!(close(&c, 3.0f64.cos(), 1e-9) && close(&c, 3.3f64.cos(), 1e-9));
    }

    #[test]
    fn cos_sin_on_regression_positive_matches_old_formula() {
        // On θ ⊆ [0, π] the generic path must reproduce the earlier endpoint-hull
        // result byte-for-byte (k = 0, no clamp fires).
        let iv = RatIv::new(Q::new(4, 10), Q::new(6, 10));
        let c = cos_on::<Bignum>(&iv, 20);
        let s = sin_on::<Bignum>(&iv, 20);
        // cos: [cos_at(hi).lo, cos_at(lo).hi]; sin: [sin_at(lo).lo, sin_at(hi).hi].
        // cos: [cos_at(hi).lo, cos_at(lo).hi]; sin: [sin_at(lo).lo, sin_at(hi).hi].
        assert_eq!(*c.lo(), *cos_at::<Bignum>(&Q::new(6, 10), 20).lo());
        assert_eq!(*c.hi(), *cos_at::<Bignum>(&Q::new(4, 10), 20).hi());
        assert_eq!(*s.lo(), *sin_at::<Bignum>(&Q::new(4, 10), 20).lo());
        assert_eq!(*s.hi(), *sin_at::<Bignum>(&Q::new(6, 10), 20).hi());
    }

    #[test]
    fn cos_sin_on_shifted_large_argument() {
        // Arguments outside [−π, π] prove the reduction is real, not a window:
        // 2π + 0.5 ≡ 0.5, and 5π/2 ≡ π/2 (sin peak).
        let two_pi_plus = RatIv::point(Q::new(6_783_185, 1_000_000)); // ≈ 2π + 0.5
        let c = cos_on::<Bignum>(&two_pi_plus, 24);
        let s = sin_on::<Bignum>(&two_pi_plus, 24);
        assert!(close(&c, 0.5f64.cos(), 1e-6), "cos(2π+0.5) = cos(0.5)");
        assert!(close(&s, 0.5f64.sin(), 1e-6), "sin(2π+0.5) = sin(0.5)");
        // 5π/2 ≈ 7.853982 → sin = 1, cos = 0.
        let five_half_pi = RatIv::point(Q::new(7_853_982, 1_000_000));
        let s2 = sin_on::<Bignum>(&five_half_pi, 24);
        assert!(close(&s2, 1.0, 1e-6), "sin(5π/2) = 1");
    }

    #[test]
    fn cos_sin_on_wide_interval_is_full_range() {
        // θ spanning ≥ one full period is the honest full range [−1, 1].
        let iv = RatIv::new(Q::from_i128(-4), Q::from_i128(4)); // width 8 > 2π
        let c = cos_on::<Bignum>(&iv, 24);
        let s = sin_on::<Bignum>(&iv, 24);
        assert_eq!(*c.lo(), Q::from_i128(-1));
        assert_eq!(*c.hi(), Q::from_i128(1));
        assert_eq!(*s.lo(), Q::from_i128(-1));
        assert_eq!(*s.hi(), Q::from_i128(1));
    }

    #[test]
    fn cos_sin_on_negative_narrows_with_terms() {
        // A negative (reduced) argument genuinely encloses — the interval shrinks
        // with the term budget instead of sitting at the [−1, 1] fallback.
        let iv = RatIv::new(Q::new(-11, 10), Q::new(-9, 10));
        let a = cos_on::<Bignum>(&iv, 6).width();
        let b = cos_on::<Bignum>(&iv, 24).width();
        assert!(b < a, "cos enclosure tightens with terms");
        assert!(b < Q::from_i128(2), "genuinely narrower than [−1, 1]");
    }

    #[test]
    fn cos_sin_on_multi_period_soundness_sweep() {
        // The safety net: over a grid of sub-intervals spanning several periods
        // (≈[−12, 12] ⊃ [−4π, 4π]), the enclosure must contain the true cos/sin at
        // both endpoints and the midpoint — catching any sign/swap/k error.
        let terms = 20;
        let mut n = -48i128;
        while n <= 48 {
            for &span in &[1i128, 5, 11] {
                let a = Q::new(n, 4);
                let b = Q::new(n + span, 4);
                let iv = RatIv::new(a.clone(), b.clone());
                let c = cos_on::<Bignum>(&iv, terms);
                let s = sin_on::<Bignum>(&iv, terms);
                let mid = Q::new(2 * n + span, 8);
                for t in [&a, &mid, &b] {
                    let tf = to_f64(t);
                    assert!(
                        close(&c, tf.cos(), 1e-6),
                        "cos fails on [{n}/4, {}/4] at {tf}",
                        n + span
                    );
                    assert!(
                        close(&s, tf.sin(), 1e-6),
                        "sin fails on [{n}/4, {}/4] at {tf}",
                        n + span
                    );
                }
            }
            n += 3;
        }
    }

    #[test]
    fn round_out_widens_to_bounded_denominator() {
        // A tight interval with an odd (non-dyadic) denominator.
        let iv = RatIv::new(Q::new(1, 7), Q::new(2, 7));
        let r = iv.round_out(10); // snap outward to a /2^10 grid
        assert!(
            *r.lo() <= *iv.lo() && *r.hi() >= *iv.hi(),
            "must widen (contain)"
        );
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
            assert!(
                d.len() < 32,
                "denominator digit count bounded, got {}",
                d.len()
            );
        }
        assert!(
            close(&iv, 0.5f64.atan(), 1e-12),
            "still brackets arctan(1/2)"
        );
    }

    #[test]
    fn log_brackets_known_values() {
        // Tolerance absorbs the f64 readout of the rational endpoints; the interval
        // itself is far tighter than an f64 ULP at 40 terms. Covers m = 0 (y ∈ [1,2)),
        // m > 0 (x ≥ 2), and m < 0 (x < 1 → negative logarithm).
        let tol = 1e-12;
        // Exact rationals so `x` and the `want` oracle are the *same* number (no
        // float-truncation gap). Covers m = 0 (y ∈ [1,2)), m > 0 (x ≥ 2), m < 0 (x < 1),
        // and a non-dyadic surd-radius argument (144/97).
        for x in [
            Q::from_i128(1),
            Q::new(3, 2),
            Q::from_i128(2),
            Q::from_i128(3),
            Q::from_i128(10),
            Q::new(1, 2),
            Q::new(1, 10),
            Q::new(144, 97),
        ] {
            let v = to_f64(&x);
            assert!(close(&log::<Bignum>(&x, 40), v.ln(), tol), "ln {v}");
        }
        // ln 1 = 0 exactly (u = 0, m = 0).
        let l1 = log::<Bignum>(&Q::from_i128(1), 40);
        assert_eq!(*l1.lo(), Q::from_i128(0));
    }

    #[test]
    fn log_width_shrinks_with_terms() {
        let a = log::<Bignum>(&Q::from_i128(10), 4).width();
        let b = log::<Bignum>(&Q::from_i128(10), 30).width();
        assert!(b < a);
        assert!(b < Q::new(1, 1_000_000_000));
    }

    #[test]
    fn arctan_on_interval_brackets_endpoints() {
        // arctan is increasing, so the interval enclosure contains both endpoints.
        let iv = RatIv::new(Q::new(1, 2), Q::from_i128(2));
        let a = arctan_on::<Bignum>(&iv, 24);
        assert!(close(&a, 0.5f64.atan(), 1e-9));
        assert!(close(&a, 2.0f64.atan(), 1e-9));
        assert!(a.contains(&arctan::<Bignum>(&Q::from_i128(1), 24).mid()));
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
