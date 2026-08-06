//! Pure 1D certificate checkers (spec §8.5): decide whether a rational function is
//! bounded away from zero on a span, exactly, via Sturm root-counting.
//!
//! [`reg_q`] is the core positivity check `num/den > m` on a closed span (REG-Q — the
//! `|q|²`, `|n̂′|²` regularity margins). [`slab_s0`] adds the stall-end ring checks
//! (SLAB-S0). Both are **total** (Sturm is exact): each returns `Verified(margin)` or
//! `Refuted(witness)`, never `Unresolved`.
//!
//! The certificate is a *supplied* Sturm chain that the checker **re-verifies** with
//! [`SturmChain::verify_chain`] before it counts — a forged chain is rejected, never
//! trusted. Margins are squared ([`MarginSq`], spec §8.2): `|q|²`, `|n̂′|²` are already
//! √-cleared, so `num/den ≥ m` compares squared-against-squared.
//!
//! [`corner_range`] is the corner min/max evaluator (spec §8.2 convexity rider): for an
//! **affine** form the box extremum lives at a corner, so its range over the box is the
//! range over the corners — the sound basis for the CLIP-σ signed test.
//!
//! The CLIP transversality ladder (spec §8.5) certifies that a trim boundary crosses a
//! chart cleanly. Its rungs, short-circuiting, are wired by [`clip`]: CLIP-W → CLIP-μ
//! (both [`reg_q`] positivity instances on the cleared `(n·b_J)²`, `(r·b_J)²` gauges) →
//! per isolated common zero one of [`clip_a`] (the fiber misses the trim plane by a
//! separated constant) or [`clip_sigma`] (the signed-`∂_σG` disjunction) or a reject.
//! [`clip_sigma`] is the one √-free rung: its threshold is a **plain signed [`Rat`]**,
//! never a [`MarginSq`] — squaring an affine form reintroduces the interior-minimizing
//! `|·|` slip that falsely certifies the `G = σμ` singular crossing.
//!
//! Around the ladder sit the clipped-domain checkers (spec §8.5). [`trim_local`] keeps the
//! clip local — `G_i > 0` at every outer-fiber corner plus interior confinement — catching
//! the wrap re-entry a w-only test misses. [`clip_dom`] is the corner-sign census:
//! [`classify_fiber`] types each fiber `D ∩ {G ≥ 0}`, and the sweep reports the retained
//! support's connectivity. [`edge_reg`] certifies edge regularity `|e′|² ≥ m_e` with a
//! fourth [`EdgeReg::Stall`] state — a *removable* parametrization artifact routed to
//! REPARAM (spec §7), kept as a domain enum and lowered by [`EdgeReg::to_verdict`] to
//! `Refuted` (gate-failing as stored), **never** a new [`Verdict`] variant.

use crate::margin::MarginSq;
use crate::verdict::Verdict;
use alloc::vec::Vec;
use lattice::{Backend, Bignum, Interval, Poly, Rat, SturmChain};

/// A REG-Q / positivity certificate: `num/den > m` on `span`, with `den` structurally
/// positive there. The searcher supplies the Sturm chains of `den` and of the residual
/// `R = num − m·den`; the checker re-verifies them.
pub struct RegCert<B: Backend = Bignum> {
    /// The numerator (e.g. `|q|²` or `|n̂′|²`, already √-cleared).
    pub num: Poly<B>,
    /// The denominator (a positive gauge; `1` for the bare `|q|²` form).
    pub den: Poly<B>,
    /// The squared separation margin `m` (`num/den ≥ m`).
    pub m: MarginSq<Rat<B>>,
    /// The closed span the inequality must hold on.
    pub span: Interval<B>,
    /// The searcher-supplied Sturm chain of `den` (re-verified before use).
    pub den_chain: SturmChain<B>,
    /// The searcher-supplied Sturm chain of `R = num − m·den` (re-verified before use).
    pub res_chain: SturmChain<B>,
}

/// Why the REG-Q family ([`reg_q`] / [`slab_s0`] / [`trim_local`]) refused a certificate.
///
/// Separates a **malformed certificate** (bad paperwork — a non-positive margin or a forged
/// Sturm chain; there is no real counterexample and the inequality may even hold) from a
/// genuine **positivity failure** (a real degeneracy at a σ witness). Recovery logic — e.g. a
/// REPARAM trigger, or a searcher deciding whether to subdivide — needs to tell the two apart.
///
/// For the positivity variants the `sigma` is the checker's witness (the span's lower bound);
/// it marks the failing check, not necessarily the exact root (isolating the precise root is a
/// future refinement).
pub enum RegFault<B: Backend = Bignum> {
    /// The separation margin is not strictly positive — not a regularity certificate at all.
    NonPositiveMargin,
    /// The supplied Sturm chain of `den` is not `den`'s chain (a forged or stale chain).
    InvalidDenChain,
    /// The supplied Sturm chain of the residual `R = num − m·den` is not `R`'s chain.
    InvalidResidualChain,
    /// The denominator is not strictly positive on the span (a real degeneracy).
    NonPositiveDen {
        /// The checker's σ witness (the span lower bound).
        sigma: Rat<B>,
    },
    /// `num/den > m` does not hold on the span (a real margin failure).
    MarginFailure {
        /// The checker's σ witness (the span lower bound).
        sigma: Rat<B>,
    },
    /// A stall-end ring datum ([`slab_s0`]) is not strictly positive.
    StallLimit {
        /// The non-positive ring value.
        value: Rat<B>,
    },
    /// An outer-support-fiber corner ([`trim_local`]) is not strictly retained (`G_i ≤ 0`).
    OuterFiber {
        /// The non-positive corner value.
        value: Rat<B>,
    },
}

/// REG-Q (spec §8.5): certify `num/den > m` on the span, with `den > 0` and `m > 0`.
/// Total — `Verified(m)` or `Refuted(`[`RegFault`]`)`.
///
/// The margin must be **strictly positive**: a regularity certificate exists to bound a
/// √-cleared quantity *away* from zero. With `m ≤ 0` the residual `R = num − m·den` can be
/// positive without `num > 0` (a negative `m` makes `Verified` on a degenerate `num ≡ 0` —
/// the checker would certify nothing), so a non-positive margin is rejected outright rather
/// than trusted from the searcher. The [`RegFault`] distinguishes that (and a forged chain)
/// from a genuine positivity failure.
pub fn reg_q<B: Backend>(cert: &RegCert<B>) -> Verdict<MarginSq<Rat<B>>, RegFault<B>, ()> {
    let (lo, hi) = (&cert.span.lo, &cert.span.hi);
    // A non-positive separation margin is not a regularity certificate — reject it.
    if cert.m.0.sign() <= 0 {
        return Verdict::Refuted(RegFault::NonPositiveMargin);
    }
    let r = cert.num.sub(&cert.den.scale(&cert.m.0)); // R = num − m·den

    // Re-verify the supplied chains — a chain we did not check is not evidence.
    if !cert.den_chain.verify_chain(&cert.den) {
        return Verdict::Refuted(RegFault::InvalidDenChain);
    }
    if !cert.res_chain.verify_chain(&r) {
        return Verdict::Refuted(RegFault::InvalidResidualChain);
    }
    // den > 0 on [lo, hi]: positive at the closed lower end and no root in (lo, hi].
    if cert.den.eval(lo).sign() <= 0 || cert.den_chain.count_in(lo, hi) != 0 {
        return Verdict::Refuted(RegFault::NonPositiveDen { sigma: lo.clone() });
    }
    // R > 0 on [lo, hi] ⇒ num > m·den ⇒ num/den > m (den > 0).
    if r.eval(lo).sign() <= 0 || cert.res_chain.count_in(lo, hi) != 0 {
        return Verdict::Refuted(RegFault::MarginFailure { sigma: lo.clone() });
    }
    Verdict::Verified(cert.m.clone())
}

/// The stall-end ring datum `(c′+μ̂r̂′)·n̂′` at `σ*`, evaluated at the two `μ̂` endpoints
/// (spec §3.2.2). Both must be strictly positive for the hatted chart to stay regular.
pub struct StallLimit<B: Backend = Bignum> {
    /// The datum at the lower `μ̂` endpoint.
    pub value_lo: Rat<B>,
    /// The datum at the upper `μ̂` endpoint.
    pub value_hi: Rat<B>,
}

/// A SLAB-S0 certificate: the corner-collapsed `R₁ + w⁻ > 0` core, plus the stall-end
/// ring checks on spans that abut a stall.
pub struct SlabS0Cert<B: Backend = Bignum> {
    /// The positivity core: `num = (R₁ + w⁻)`, collapsed at the minimum-`w` corner.
    pub core: RegCert<B>,
    /// The stall-limit ring datum, on spans whose end is a stall (`None` otherwise).
    pub stall_end: Option<StallLimit<B>>,
}

/// SLAB-S0 (spec §8.5): the one-sided `inf(R₁ + w) > 0`, reduced by corner collapse at
/// `w⁻` (the `+w` coefficient is structurally `+1`) to the [`reg_q`] positivity core,
/// plus — on stall-end spans — that both `μ̂`-endpoint ring values are strictly positive.
pub fn slab_s0<B: Backend>(cert: &SlabS0Cert<B>) -> Verdict<MarginSq<Rat<B>>, RegFault<B>, ()> {
    let core = reg_q(&cert.core);
    if !matches!(core, Verdict::Verified(_)) {
        return core;
    }
    if let Some(sl) = &cert.stall_end {
        if sl.value_lo.sign() <= 0 {
            return Verdict::Refuted(RegFault::StallLimit {
                value: sl.value_lo.clone(),
            });
        }
        if sl.value_hi.sign() <= 0 {
            return Verdict::Refuted(RegFault::StallLimit {
                value: sl.value_hi.clone(),
            });
        }
    }
    core
}

/// The `(min, max)` of a set of corner values, or `None` if empty. Generic over any
/// totally-ordered `T` (the CLIP-σ decision is proven at `T = i128`, applied at `T = Rat`).
///
/// Sound as the box range **only for an affine form**, whose extremum over a box is
/// attained at a corner (spec §8.2 convexity rider — never use this on a form whose
/// interior can beat its corners). The CLIP-σ signed test ranges `∂_σG` (affine) this way.
pub fn corner_range<T: Ord + Clone>(corners: &[T]) -> Option<(T, T)> {
    let mut it = corners.iter();
    let first = it.next()?;
    let (mut lo, mut hi) = (first.clone(), first.clone());
    for c in it {
        if c.cmp(&lo) == core::cmp::Ordering::Less {
            lo = c.clone();
        }
        if c.cmp(&hi) == core::cmp::Ordering::Greater {
            hi = c.clone();
        }
    }
    Some((lo, hi))
}

/// The certified single sign of an affine trim form across a cell — the "side" a
/// transversality rung ([`clip_a`], [`clip_sigma`]) resolves to, fed to the CLIP-DOM
/// census (spec §8.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipBranch {
    /// The form is bounded strictly **above** zero across the cell (retained side by
    /// `sgn = +`).
    Positive,
    /// The form is bounded strictly **below** zero across the cell (retained side by
    /// `sgn = −`).
    Negative,
}

/// A CLIP-σ certificate: the four corner values of the **signed** affine `∂_σG` over the
/// `(μ, w)` box, and the separation threshold.
pub struct ClipSigmaCert<B: Backend = Bignum> {
    /// `∂_σG` evaluated at the four box corners, in any order (the checker takes the
    /// min and max). `∂_σG` is affine in `(μ, w)`, so its box range is the corner range.
    pub corners: [Rat<B>; 4],
    /// The separation threshold — a plain **signed** [`Rat`], **never** a [`MarginSq`].
    /// Squaring the affine `∂_σG` would let it dip to zero in the box interior (`G = σμ`
    /// has `∂_σG = μ`, zero on `μ = 0`) while every corner reads nonzero: the exact
    /// falsely-certifying slip this signed rung exists to kill.
    pub m_sigma: Rat<B>,
}

/// CLIP-σ (spec §8.5, ★): the signed disjunction `[min ∂_σG ≥ m_σ] ∨ [max ∂_σG ≤ −m_σ]`
/// — the affine corner range of `∂_σG` excludes zero with margin `m_σ > 0`.
///
/// `Verified(sign)` when single-signed and separated (the sign feeds the census free);
/// `Unresolved(range width)` when the range straddles zero or the margin is not met (the
/// searcher subdivides). Never `Refuted`: transversality failure is resolved by
/// subdivision or by the ladder's reject rung, not by this leaf.
///
/// Soundness rests on `corners` bounding an **affine** form: `Verified(Positive)` asserts
/// `∂_σG ≥ m_σ > 0` across the whole box, which holds because the corner minimum bounds
/// the box minimum of an affine form (a *squared* form would violate this — see
/// [`ClipSigmaCert::m_sigma`]). A non-positive `m_σ` certifies nothing ⇒ `Unresolved`.
///
/// ```
/// use certify_core::certify1d::{clip_sigma, ClipBranch, ClipSigmaCert};
/// use certify_core::Verdict;
/// use lattice::{Bignum, Rat};
///
/// // The `G = σμ` trap: ∂_σG = μ, whose corners over μ ∈ [−1, +1] straddle zero.
/// // A squared `|∂_σG|² ≥ m` test would falsely certify (the corners read μ² = 1);
/// // the signed range [−1, +1] does not — it stays Unresolved, forcing subdivision.
/// let trap = ClipSigmaCert::<Bignum> {
///     corners: [Rat::from_i128(-1), Rat::from_i128(1), Rat::from_i128(-1), Rat::from_i128(1)],
///     m_sigma: Rat::new(1, 4),
/// };
/// assert!(matches!(clip_sigma(&trap), Verdict::Unresolved(_)));
///
/// // A transverse trim: ∂_σG = 2 + μ ≥ 1 across the box ⇒ single-signed, separated.
/// let ok = ClipSigmaCert::<Bignum> {
///     corners: [Rat::from_i128(1), Rat::from_i128(3), Rat::from_i128(1), Rat::from_i128(3)],
///     m_sigma: Rat::new(1, 2),
/// };
/// assert_eq!(clip_sigma(&ok), Verdict::Verified(ClipBranch::Positive));
/// ```
pub fn clip_sigma<B: Backend>(cert: &ClipSigmaCert<B>) -> Verdict<ClipBranch, (), Rat<B>> {
    let (lo, hi) = match corner_range(&cert.corners) {
        Some(r) => r,
        None => return Verdict::Unresolved(Rat::from_i128(0)),
    };
    let neg_m = cert.m_sigma.neg();
    match clip_sigma_branch(&lo, &hi, &cert.m_sigma, &neg_m, cert.m_sigma.sign() > 0) {
        Some(branch) => Verdict::Verified(branch),
        // Neither single-signed-and-separated: hand back the range width as the refinement
        // handle (subdivision shrinks an affine range monotonically toward its true value).
        None => Verdict::Unresolved(hi.sub(&lo)),
    }
}

/// The CLIP-σ signed disjunction as a pure decision over the affine corner range,
/// factored over any totally-ordered `T`: `Positive` iff `lo ≥ m_σ`, `Negative` iff
/// `hi ≤ −m_σ`, both gated on `m_σ > 0` (`m_positive`); otherwise `None` (straddle).
///
/// The separation is strictly positive by construction — `lo ≥ m_σ ≤ 0` would not force
/// `lo > 0`, so a non-positive threshold licenses no single-sign conclusion. Factoring
/// this out lets the soundness-critical logic be Kani-proven on the tractable `i128`
/// domain (the two-tier `Rat` representation is a symbolic-execution trap); [`clip_sigma`]
/// applies the identical function at `T = Rat`, and `Rat`'s total order is proven in
/// `lattice`.
pub(crate) fn clip_sigma_branch<T: Ord>(
    lo: &T,
    hi: &T,
    m: &T,
    neg_m: &T,
    m_positive: bool,
) -> Option<ClipBranch> {
    if m_positive {
        if lo.cmp(m) != core::cmp::Ordering::Less {
            return Some(ClipBranch::Positive);
        }
        if hi.cmp(neg_m) != core::cmp::Ordering::Greater {
            return Some(ClipBranch::Negative);
        }
    }
    None
}

/// A CLIP-a certificate: the constant fiber offset `a` (the signed distance of the fiber
/// to the trim plane Π) and its **squared** separation margin.
pub struct ClipACert<B: Backend = Bignum> {
    /// The constant offset `a`; the fiber misses Π by `|a|`, on the uniform side `sgn a`.
    pub a: Rat<B>,
    /// The squared margin `m_a` (`a² ≥ m_a` ⇔ `|a| ≥ √m_a`) — [`MarginSq`], because `|a|`
    /// is cleared to `a²` (contrast [`ClipSigmaCert::m_sigma`], which must stay signed).
    pub m_a: MarginSq<Rat<B>>,
}

/// CLIP-a (spec §8.5): the fiber misses Π by a separated constant — `a² ≥ m_a`, `m_a > 0` —
/// and so takes the uniform side `sgn a`. `Verified(sign)` / `Unresolved(a²)`; never
/// `Refuted`. A non-positive `m_a` establishes no separation (the sign may be real, but the
/// fiber is not certified to clear Π), so it yields `Unresolved` — never a certified side.
pub fn clip_a<B: Backend>(cert: &ClipACert<B>) -> Verdict<ClipBranch, (), Rat<B>> {
    let a_sq = cert.a.mul(&cert.a);
    // No separation without a strictly-positive margin.
    if cert.m_a.0.sign() <= 0 {
        return Verdict::Unresolved(a_sq);
    }
    // `a² ≥ m_a` with `m_a > 0` forces `a ≠ 0`, so the sign is a genuine side.
    if a_sq.cmp(&cert.m_a.0) != core::cmp::Ordering::Less {
        match cert.a.sign() {
            1 => return Verdict::Verified(ClipBranch::Positive),
            -1 => return Verdict::Verified(ClipBranch::Negative),
            _ => {} // unreachable: m_a > 0 ⇒ a² ≥ m_a > 0 ⇒ a ≠ 0.
        }
    }
    Verdict::Unresolved(a_sq)
}

/// A per-common-zero resolution attempt in the CLIP ladder: at each isolated common zero
/// of the trim coefficients the searcher proposes one discharge.
pub enum ZeroClip<B: Backend = Bignum> {
    /// Discharge by CLIP-a — the fiber misses Π by a separated constant.
    ByA(ClipACert<B>),
    /// Discharge by CLIP-σ — the signed `∂_σG` transversality rung (★).
    BySigma(ClipSigmaCert<B>),
    /// All three partials vanish: Π osculates the offset (spec §8.5 → §14 singular stub).
    Osculation,
}

/// The CLIP ladder's terminal (spec §8.5: "terminates in `{certified, rejected}`").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipVerdict {
    /// The clip is transverse — certified.
    Certified,
    /// A common zero osculates Π (all partials vanish): reject to a band (spec §14).
    Rejected,
    /// A rung came back short of its margin: the searcher must subdivide and re-supply.
    /// Subdivision localizes rungs; it never substitutes for one.
    Subdivide,
}

/// The exhaustive common-zero census the CLIP ladder's per-zero rung must clear. The trim
/// coefficients' common zeros are the real roots of `b² + d²` (over ℝ, `b²+d² = 0 ⟺ b = 0 ∧
/// d = 0`). The searcher supplies that polynomial, a re-verified Sturm chain, and one
/// isolating interval per supplied [`ZeroClip`]; the checker (via [`census_ok`]) confirms the
/// zeros list is **complete** — no awkward root omitted to sneak a certification through.
pub struct ZeroCensus<B: Backend = Bignum> {
    /// `b² + d²`, whose real roots are exactly the common zeros.
    pub discriminant: Poly<B>,
    /// The searcher-supplied Sturm chain of `discriminant` (re-verified before counting).
    pub chain: SturmChain<B>,
    /// The σ-span the ladder runs over.
    pub span: Interval<B>,
    /// One isolating interval per common zero, in σ-order — one per supplied [`ZeroClip`].
    pub intervals: Vec<Interval<B>>,
}

/// Whether `census` proves the supplied `n_zeros` common zeros are the **complete** isolated
/// root set of `b² + d²` on the span: the chain is genuine, the total distinct-root count
/// equals `n_zeros`, and the `intervals` are in-span, σ-ordered, disjoint, and each isolate
/// exactly one root — so every root gets exactly one discharge and none is omitted.
pub fn census_ok<B: Backend>(census: &ZeroCensus<B>, n_zeros: usize) -> bool {
    if !census.chain.verify_chain(&census.discriminant) {
        return false;
    }
    let (lo, hi) = (&census.span.lo, &census.span.hi);
    if census.chain.count_in(lo, hi) as usize != n_zeros || census.intervals.len() != n_zeros {
        return false;
    }
    let mut prev_hi: Option<&Rat<B>> = None;
    for iv in &census.intervals {
        // Inside the span.
        if iv.lo.cmp(lo) == core::cmp::Ordering::Less
            || iv.hi.cmp(hi) == core::cmp::Ordering::Greater
        {
            return false;
        }
        // σ-ordered and disjoint: `iv.lo ≥ previous iv.hi` (half-open `(lo, hi]`).
        if let Some(p) = prev_hi {
            if iv.lo.cmp(p) == core::cmp::Ordering::Less {
                return false;
            }
        }
        // Exactly one root inside.
        if census.chain.count_in(&iv.lo, &iv.hi) != 1 {
            return false;
        }
        prev_hi = Some(&iv.hi);
    }
    true
}

/// The CLIP transversality ladder (spec §8.5): CLIP-W over the whole span → CLIP-μ on the
/// failing sub-spans → per isolated common zero one of [`clip_a`] / [`clip_sigma`] / an
/// osculation reject. Short-circuits: the first fully-certified rung wins.
///
/// `w` and `mu` are [`reg_q`] positivity certificates (the cleared `(n·b_J)²`, `(r·b_J)²`
/// gauges); the searcher supplies the failing sub-spans and the isolated common zeros. On the
/// per-zero path, `census` must prove the supplied `zeros` are the **complete** common-zero
/// set ([`census_ok`]) — an incomplete list (an omitted awkward zero) cannot certify.
/// A [`ClipVerdict::Subdivide`] means the supplied certificate did not converge — refine
/// and re-run; only [`ZeroClip::Osculation`] yields [`ClipVerdict::Rejected`].
pub fn clip<B: Backend>(
    w: &RegCert<B>,
    mu: &[RegCert<B>],
    zeros: &[ZeroClip<B>],
    census: Option<&ZeroCensus<B>>,
) -> ClipVerdict {
    // CLIP-W: w-transverse across the whole span ⇒ done.
    if matches!(reg_q(w), Verdict::Verified(_)) {
        return ClipVerdict::Certified;
    }
    // CLIP-μ: every supplied failing sub-span clears its own gauge ⇒ done.
    if !mu.is_empty() && mu.iter().all(|c| matches!(reg_q(c), Verdict::Verified(_))) {
        return ClipVerdict::Certified;
    }
    // Per common zero: each must be CLIP-a- or CLIP-σ-certified; any osculation rejects.
    if zeros.is_empty() {
        return ClipVerdict::Subdivide; // nothing localized the failure yet
    }
    let mut all_resolved = true;
    for z in zeros {
        match z {
            ZeroClip::Osculation => return ClipVerdict::Rejected,
            ZeroClip::ByA(c) => all_resolved &= matches!(clip_a(c), Verdict::Verified(_)),
            ZeroClip::BySigma(c) => all_resolved &= matches!(clip_sigma(c), Verdict::Verified(_)),
        }
    }
    // Certify only if every zero discharged AND the zeros are the complete census — closing
    // the local-proof-vs-global-coverage hole (an omitted zero is never certified).
    let complete = matches!(census, Some(c) if census_ok(c, zeros.len()));
    if all_resolved && complete {
        ClipVerdict::Certified
    } else {
        ClipVerdict::Subdivide
    }
}

/// A TRIM-LOCAL certificate: the `G_i` corner values on each outer support fiber, plus the
/// interior-confinement positivity certificate.
pub struct TrimLocalCert<B: Backend = Bignum> {
    /// `G_i` at the four corners of each non-exempt outer support fiber (the fiber's own
    /// spline μ-bounds × the w-range), one `[Rat; 4]` per fiber. Chart-boundary support
    /// ends are exempt — nothing beyond them exists to protect.
    pub outer_fibers: Vec<[Rat<B>; 4]>,
    /// Interior confinement: `G_i > 0` across the support σ-span (a [`reg_q`] positivity on
    /// the cleared `G_i` numerator) — the Sturm half a corner test alone cannot supply.
    pub confinement: RegCert<B>,
}

/// TRIM-LOCAL (spec §8.5): the clip stays local — every outer-fiber corner is strictly
/// retained (`G_i > 0`) **and** the interior is confined. Catches the re-entry a w-only
/// quantification misses: a 1.49-wrap flank re-crosses the trim plane at distant σ.
/// `Verified(m)` (the confinement margin) or `Refuted(`[`RegFault`]`)` at the first failure.
pub fn trim_local<B: Backend>(
    cert: &TrimLocalCert<B>,
) -> Verdict<MarginSq<Rat<B>>, RegFault<B>, ()> {
    for fiber in &cert.outer_fibers {
        for g in fiber {
            if g.sign() <= 0 {
                return Verdict::Refuted(RegFault::OuterFiber { value: g.clone() });
            }
        }
    }
    reg_q(&cert.confinement)
}

/// The combinatorial type of a clipped fiber `D ∩ {G ≥ 0}` (spec §8.5), fixed by the signs
/// of `G` at the four rectangle corners — `G` is affine on the fiber, so the per-corner
/// signs determine the cell exactly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FiberCell {
    /// Every corner `G < 0`: the fiber is entirely clipped away.
    Empty,
    /// Every corner `G ≥ 0`: the whole rectangle is retained (the clip is inactive here).
    Full,
    /// Mixed corner signs: the half-plane cuts the rectangle into a convex polygon.
    Clipped,
}

/// Classify a fiber from the `G`-signs at its four corners (spec §8.5). A `G = 0` corner is
/// retained (`≥ 0`), so `Full` needs no strictly-negative corner and `Empty` needs all four.
pub fn classify_fiber<B: Backend>(corners: &[Rat<B>; 4]) -> FiberCell {
    let neg = corners.iter().filter(|g| g.sign() < 0).count();
    match neg {
        4 => FiberCell::Empty,
        0 => FiberCell::Full,
        _ => FiberCell::Clipped,
    }
}

/// The CLIP-DOM census result (spec §8.5): the retained σ-support's connectivity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClipDomCensus {
    /// The number of maximal runs of retained (non-`Empty`) fibers. `1` is a connected
    /// support; `> 1` a disconnected support that splits into administrative sub-charts.
    pub retained_components: usize,
    /// Whether some fiber is a partial (`Clipped`) cell — the clip is active somewhere.
    pub has_clip: bool,
}

/// CLIP-DOM census (spec §8.5): sweep the σ-ordered fiber cells — the searcher has Sturm-
/// isolated the corner-sign events, one representative fiber per cell — and report the
/// retained support's connectivity, the datum consumers re-point onto `D^closure`. A
/// `retained_components > 1` is the disconnected-support split into sub-charts.
pub fn clip_dom<B: Backend>(cells: &[[Rat<B>; 4]]) -> ClipDomCensus {
    let mut retained_components = 0;
    let mut has_clip = false;
    let mut prev_retained = false;
    for cell in cells {
        let fc = classify_fiber(cell);
        if fc == FiberCell::Clipped {
            has_clip = true;
        }
        let retained = fc != FiberCell::Empty;
        if retained && !prev_retained {
            retained_components += 1;
        }
        prev_retained = retained;
    }
    ClipDomCensus {
        retained_components,
        has_clip,
    }
}

/// An EDGE-REG failure witness (spec §8.5): both cases gate-fail, but the road back differs.
pub enum EdgeFail<B: Backend = Bignum> {
    /// A geometric cusp (the point set has a genuine corner, e.g. `y² = x³`) at this
    /// parameter: reject to a band (spec §14). Reparametrization cannot remove it.
    Cusp(Rat<B>),
    /// A removable parametrization stall: the point set is regular, the road back REPARAM.
    Stalled {
        /// The stall parameter `t*`.
        t_star: Rat<B>,
        /// The removal order.
        order: usize,
    },
}

/// EDGE-REG's verdict (spec §8.5): edge-curve regularity `|e′|² ≥ m_e`, with a fourth state
/// beyond the shared [`Verdict`]. [`EdgeReg::Stall`] is a *removable* parametrization
/// artifact, kept distinct from a geometric [`EdgeReg::Fail`] because the road back differs
/// (REPARAM vs band). It is a domain-specific enum, **never** a new [`Verdict`] variant —
/// that blast radius on the shared TCB type is exactly what keeping it here avoids. Lower it
/// with [`EdgeReg::to_verdict`].
pub enum EdgeReg<B: Backend = Bignum> {
    /// `|e′|² ≥ m_e > 0` on the open interval: a regular immersion.
    Pass(MarginSq<Rat<B>>),
    /// `e′` vanishes at a geometric cusp: reject to band.
    Fail(Rat<B>),
    /// `e′` vanishes at an isolated `t*` but the point set is regular — a removable stall.
    Stall {
        /// The stall parameter `t*`.
        t_star: Rat<B>,
        /// The removal order.
        order: usize,
    },
}

impl<B: Backend> EdgeReg<B> {
    /// Lower to the shared [`Verdict`] for gate propagation. `Stall → Refuted(Stalled)`:
    /// **gate-failing as stored** — `Pending` is not "undecided" (never `Unresolved`), it is
    /// "decided: fails, pending a REPARAM". The witness keeps the cusp/stall distinction so
    /// the caller routes `Stalled → REPARAM` (spec §7) and `Cusp → band` (spec §14).
    pub fn to_verdict(&self) -> Verdict<MarginSq<Rat<B>>, EdgeFail<B>, ()> {
        match self {
            EdgeReg::Pass(m) => Verdict::Verified(m.clone()),
            EdgeReg::Fail(t) => Verdict::Refuted(EdgeFail::Cusp(t.clone())),
            EdgeReg::Stall { t_star, order } => Verdict::Refuted(EdgeFail::Stalled {
                t_star: t_star.clone(),
                order: *order,
            }),
        }
    }
}

/// An EDGE-REG certificate: the cleared `|e′|²` positivity, plus — used only when that
/// fails — the searcher's classification of the speed zero.
pub struct EdgeRegCert<B: Backend = Bignum> {
    /// The cleared `|e′|² ≥ m_e` positivity certificate (a [`reg_q`] instance on the edge's
    /// squared speed). Verifying it is the **only** path to a gate-pass.
    pub speed_sq: RegCert<B>,
    /// The searcher's classification of a speed zero, consulted only when `speed_sq` fails:
    /// `Some(Cusp)` for a geometric cusp, `Some(Stalled)` for a removable stall, `None` for
    /// an unclassified failure (treated conservatively as a cusp).
    pub failure: Option<EdgeFail<B>>,
}

/// EDGE-REG (spec §8.5): `Pass` iff the cleared `|e′|² ≥ m_e` positivity verifies (Sturm) —
/// the authoritative regular-immersion witness. Otherwise the edge is not regular and the
/// searcher's [`EdgeRegCert::failure`] tag routes the (gate-failing either way) recovery. A
/// wrong tag cannot manufacture a `Pass`; it only misdirects the recovery, which REPARAM's
/// re-certification then catches — so the checker trusts the tag on the failing path alone.
pub fn edge_reg<B: Backend>(cert: &EdgeRegCert<B>) -> EdgeReg<B> {
    if let Verdict::Verified(m) = reg_q(&cert.speed_sq) {
        return EdgeReg::Pass(m);
    }
    match &cert.failure {
        Some(EdgeFail::Stalled { t_star, order }) => EdgeReg::Stall {
            t_star: t_star.clone(),
            order: *order,
        },
        Some(EdgeFail::Cusp(t)) => EdgeReg::Fail(t.clone()),
        None => EdgeReg::Fail(cert.speed_sq.span.lo.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use lattice::Bignum;

    type Q = Rat<Bignum>;
    fn poly(cs: &[i128]) -> Poly<Bignum> {
        Poly::from_coeffs(cs.iter().map(|&c| Q::from_i128(c)).collect())
    }
    fn span(lo: i128, hi: i128) -> Interval<Bignum> {
        Interval {
            lo: Q::from_i128(lo),
            hi: Q::from_i128(hi),
        }
    }
    /// Build a REG-Q cert with correct (searcher-honest) Sturm chains for `num/den > m`.
    fn reg_cert(num: &[i128], den: &[i128], m: Q, iv: Interval<Bignum>) -> RegCert<Bignum> {
        let (num, den) = (poly(num), poly(den));
        let r = num.sub(&den.scale(&m));
        RegCert {
            den_chain: SturmChain::new(&den),
            res_chain: SturmChain::new(&r),
            num,
            den,
            m: MarginSq(m),
            span: iv,
        }
    }

    #[test]
    fn reg_q_verifies_a_separated_margin() {
        // num = x² + 1 ≥ 1, den = 1, m = 1/2 ⇒ num/den = x²+1 > 1/2 everywhere.
        let cert = reg_cert(&[1, 0, 1], &[1], Q::new(1, 2), span(-2, 2));
        assert!(matches!(reg_q(&cert), Verdict::Verified(_)));
    }

    #[test]
    fn reg_q_refutes_too_large_a_margin() {
        // Same, m = 2: at x = 0, x²+1 = 1 < 2 ⇒ R = x²−1 has roots in [−2,2] ⇒ Refuted.
        let cert = reg_cert(&[1, 0, 1], &[1], Q::from_i128(2), span(-2, 2));
        assert!(matches!(reg_q(&cert), Verdict::Refuted(_)));
    }

    #[test]
    fn reg_q_refutes_non_positive_denominator() {
        // den = x changes sign on [−2, 2] (den(−2) = −2 < 0) ⇒ Refuted.
        let cert = reg_cert(&[1, 0, 1], &[0, 1], Q::new(1, 4), span(-2, 2));
        assert!(matches!(reg_q(&cert), Verdict::Refuted(_)));
    }

    #[test]
    fn reg_q_rejects_a_forged_chain() {
        let mut cert = reg_cert(&[1, 0, 1], &[1], Q::new(1, 2), span(-2, 2));
        cert.res_chain = SturmChain::new(&poly(&[7])); // wrong chain (not R's)
        assert!(matches!(reg_q(&cert), Verdict::Refuted(_)));
    }

    #[test]
    fn reg_q_fault_distinguishes_paperwork_from_degeneracy() {
        // Bad paperwork — no real counterexample, no σ witness:
        let m0 = reg_cert(&[1, 0, 1], &[1], Q::from_i128(0), span(-2, 2));
        assert!(matches!(
            reg_q(&m0),
            Verdict::Refuted(RegFault::NonPositiveMargin)
        ));
        let mut forged = reg_cert(&[1, 0, 1], &[1], Q::new(1, 2), span(-2, 2));
        forged.res_chain = SturmChain::new(&poly(&[7]));
        assert!(matches!(
            reg_q(&forged),
            Verdict::Refuted(RegFault::InvalidResidualChain)
        ));
        // A genuine positivity failure — a real degeneracy carrying a σ witness:
        let too_large = reg_cert(&[1, 0, 1], &[1], Q::from_i128(2), span(-2, 2));
        assert!(matches!(
            reg_q(&too_large),
            Verdict::Refuted(RegFault::MarginFailure { .. })
        ));
        let bad_den = reg_cert(&[1, 0, 1], &[0, 1], Q::new(1, 4), span(-2, 2));
        assert!(matches!(
            reg_q(&bad_den),
            Verdict::Refuted(RegFault::NonPositiveDen { .. })
        ));
    }

    #[test]
    fn reg_q_refutes_a_non_positive_margin() {
        // num ≡ 0, den = 1, m = −1: R = num − m·den = 1 > 0, so the residual test alone
        // would *pass* — yet num/den ≡ 0 is degenerate. A negative margin certifies
        // nothing; the explicit m > 0 gate rejects it.
        let neg = reg_cert(&[0], &[1], Q::from_i128(-1), span(0, 1));
        assert!(matches!(reg_q(&neg), Verdict::Refuted(_)));
        // m = 0 is a zero-slack non-certificate — also rejected.
        let zero = reg_cert(&[1, 0, 1], &[1], Q::from_i128(0), span(-2, 2));
        assert!(matches!(reg_q(&zero), Verdict::Refuted(_)));
    }

    #[test]
    fn slab_s0_core_plus_stall_limit() {
        let core = reg_cert(&[1, 0, 1], &[1], Q::new(1, 2), span(-2, 2));
        // both endpoint ring values positive ⇒ Verified.
        let ok = SlabS0Cert {
            core: reg_cert(&[1, 0, 1], &[1], Q::new(1, 2), span(-2, 2)),
            stall_end: Some(StallLimit {
                value_lo: Q::from_i128(3),
                value_hi: Q::from_i128(1),
            }),
        };
        assert!(matches!(slab_s0(&ok), Verdict::Verified(_)));
        // a non-positive endpoint ⇒ Refuted, even though the core passes.
        let bad = SlabS0Cert {
            core,
            stall_end: Some(StallLimit {
                value_lo: Q::from_i128(-1),
                value_hi: Q::from_i128(1),
            }),
        };
        assert!(matches!(slab_s0(&bad), Verdict::Refuted(_)));
    }

    #[test]
    fn corner_range_min_and_max() {
        let cs = [
            Q::from_i128(3),
            Q::from_i128(-1),
            Q::from_i128(4),
            Q::from_i128(2),
        ];
        assert_eq!(corner_range(&cs), Some((Q::from_i128(-1), Q::from_i128(4))));
        assert_eq!(corner_range::<Q>(&[]), None);
    }

    fn corners(a: i128, b: i128, c: i128, d: i128) -> [Q; 4] {
        [
            Q::from_i128(a),
            Q::from_i128(b),
            Q::from_i128(c),
            Q::from_i128(d),
        ]
    }

    #[test]
    fn clip_sigma_verifies_a_single_signed_separated_range() {
        // ∂_σG ∈ [1, 3] ≥ 1/2 ⇒ Positive; ∂_σG ∈ [−3, −1] ≤ −1/2 ⇒ Negative.
        let pos = ClipSigmaCert::<Bignum> {
            corners: corners(1, 3, 1, 3),
            m_sigma: Q::new(1, 2),
        };
        assert_eq!(clip_sigma(&pos), Verdict::Verified(ClipBranch::Positive));
        let neg = ClipSigmaCert::<Bignum> {
            corners: corners(-1, -3, -1, -3),
            m_sigma: Q::new(1, 2),
        };
        assert_eq!(clip_sigma(&neg), Verdict::Verified(ClipBranch::Negative));
    }

    #[test]
    fn clip_sigma_straddle_is_unresolved_not_certified() {
        // The `G = σμ` trap: ∂_σG = μ ∈ [−1, +1] straddles zero ⇒ Unresolved.
        let trap = ClipSigmaCert::<Bignum> {
            corners: corners(-1, 1, -1, 1),
            m_sigma: Q::new(1, 4),
        };
        assert!(matches!(clip_sigma(&trap), Verdict::Unresolved(_)));
    }

    #[test]
    fn clip_sigma_single_signed_but_under_margin_is_unresolved() {
        // All positive, but the min corner 1 < m_σ = 10 ⇒ not separated ⇒ Unresolved.
        let cert = ClipSigmaCert::<Bignum> {
            corners: corners(1, 5, 2, 4),
            m_sigma: Q::from_i128(10),
        };
        assert!(matches!(clip_sigma(&cert), Verdict::Unresolved(_)));
    }

    #[test]
    fn clip_sigma_nonpositive_margin_never_certifies() {
        // A clearly positive range, but m_σ ≤ 0 licenses no single-sign conclusion.
        let cert = ClipSigmaCert::<Bignum> {
            corners: corners(1, 3, 1, 3),
            m_sigma: Q::from_i128(0),
        };
        assert!(matches!(clip_sigma(&cert), Verdict::Unresolved(_)));
        let cert = ClipSigmaCert::<Bignum> {
            corners: corners(1, 3, 1, 3),
            m_sigma: Q::from_i128(-1),
        };
        assert!(matches!(clip_sigma(&cert), Verdict::Unresolved(_)));
    }

    #[test]
    fn clip_a_takes_the_separated_side() {
        let pos = ClipACert::<Bignum> {
            a: Q::from_i128(3),
            m_a: MarginSq(Q::from_i128(4)),
        }; // 9 ≥ 4 ⇒ Positive.
        assert_eq!(clip_a(&pos), Verdict::Verified(ClipBranch::Positive));
        let neg = ClipACert::<Bignum> {
            a: Q::from_i128(-3),
            m_a: MarginSq(Q::from_i128(4)),
        };
        assert_eq!(clip_a(&neg), Verdict::Verified(ClipBranch::Negative));
        let under = ClipACert::<Bignum> {
            a: Q::from_i128(1),
            m_a: MarginSq(Q::from_i128(4)),
        }; // 1 < 4 ⇒ Unresolved.
        assert!(matches!(clip_a(&under), Verdict::Unresolved(_)));
        // A non-positive margin certifies no side, even with a genuine sign (9 ≥ 0).
        let toothless = ClipACert::<Bignum> {
            a: Q::from_i128(3),
            m_a: MarginSq(Q::from_i128(0)),
        };
        assert!(matches!(clip_a(&toothless), Verdict::Unresolved(_)));
    }

    /// A complete single-zero census: `b²+d² = σ` (one root at 0), one isolating interval.
    fn one_zero_census() -> ZeroCensus<Bignum> {
        ZeroCensus {
            discriminant: poly(&[0, 1]),
            chain: SturmChain::new(&poly(&[0, 1])),
            span: span(-2, 2),
            intervals: vec![span(-1, 1)],
        }
    }

    #[test]
    fn clip_ladder_certifies_when_clip_w_passes() {
        let w = reg_cert(&[1, 0, 1], &[1], Q::new(1, 2), span(-2, 2)); // reg_q Verifies
        assert_eq!(clip::<Bignum>(&w, &[], &[], None), ClipVerdict::Certified);
    }

    #[test]
    fn clip_ladder_certifies_a_common_zero_via_clip_a() {
        let w = reg_cert(&[1, 0, 1], &[1], Q::from_i128(2), span(-2, 2)); // reg_q Refutes
        let zeros = [ZeroClip::ByA(ClipACert {
            a: Q::from_i128(5),
            m_a: MarginSq(Q::from_i128(1)),
        })];
        let census = one_zero_census();
        assert_eq!(clip(&w, &[], &zeros, Some(&census)), ClipVerdict::Certified);
    }

    #[test]
    fn clip_ladder_rejects_an_osculation() {
        let w = reg_cert(&[1, 0, 1], &[1], Q::from_i128(2), span(-2, 2)); // reg_q Refutes
        let zeros = [ZeroClip::<Bignum>::Osculation];
        assert_eq!(clip(&w, &[], &zeros, None), ClipVerdict::Rejected);
    }

    #[test]
    fn clip_ladder_subdivides_on_an_unresolved_zero() {
        let w = reg_cert(&[1, 0, 1], &[1], Q::from_i128(2), span(-2, 2)); // reg_q Refutes
        let zeros = [ZeroClip::BySigma(ClipSigmaCert {
            corners: corners(-1, 1, -1, 1), // straddles ⇒ clip_sigma Unresolved
            m_sigma: Q::new(1, 4),
        })];
        let census = one_zero_census();
        assert_eq!(clip(&w, &[], &zeros, Some(&census)), ClipVerdict::Subdivide);
        // No zeros supplied at all ⇒ also Subdivide (failure not yet localized).
        assert_eq!(clip::<Bignum>(&w, &[], &[], None), ClipVerdict::Subdivide);
    }

    #[test]
    fn clip_census_rejects_an_omitted_zero() {
        // b²+d² = σ²−1 has TWO roots (±1) but the searcher supplies only one ZeroClip — the
        // census root-count catches the omission, so the ladder cannot certify (Subdivide).
        let w = reg_cert(&[1, 0, 1], &[1], Q::from_i128(2), span(-2, 2)); // reg_q Refutes
        let zeros = [ZeroClip::ByA(ClipACert {
            a: Q::from_i128(5),
            m_a: MarginSq(Q::from_i128(1)),
        })];
        let disc = poly(&[-1, 0, 1]); // σ² − 1
        let census = ZeroCensus {
            discriminant: disc.clone(),
            chain: SturmChain::new(&disc),
            span: span(-2, 2),
            intervals: vec![span(0, 2)], // "isolates" +1 only, omits −1
        };
        assert_eq!(clip(&w, &[], &zeros, Some(&census)), ClipVerdict::Subdivide);
    }

    #[test]
    fn census_ok_checks_completeness() {
        let disc = poly(&[-1, 0, 1]); // σ² − 1, roots ±1
        let complete = ZeroCensus {
            discriminant: disc.clone(),
            chain: SturmChain::new(&disc),
            span: span(-2, 2),
            intervals: vec![span(-2, 0), span(0, 2)], // isolate −1 and +1
        };
        assert!(census_ok(&complete, 2));
        assert!(!census_ok(&complete, 1)); // wrong claimed count
        // A forged chain is rejected.
        let forged = ZeroCensus {
            discriminant: disc.clone(),
            chain: SturmChain::new(&poly(&[7])),
            span: span(-2, 2),
            intervals: vec![span(-2, 0), span(0, 2)],
        };
        assert!(!census_ok(&forged, 2));
        // An interval covering both roots (not one-per-interval) is rejected.
        let not_isolating = ZeroCensus {
            discriminant: disc.clone(),
            chain: SturmChain::new(&disc),
            span: span(-2, 2),
            intervals: vec![span(-2, 2), span(-2, 2)],
        };
        assert!(!census_ok(&not_isolating, 2));
    }

    #[test]
    fn trim_local_certifies_all_positive_corners_and_confinement() {
        let ok = TrimLocalCert {
            outer_fibers: vec![corners(1, 2, 3, 4)],
            confinement: reg_cert(&[1, 0, 1], &[1], Q::new(1, 2), span(-2, 2)),
        };
        assert!(matches!(trim_local(&ok), Verdict::Verified(_)));
    }

    #[test]
    fn trim_local_refutes_a_re_entrant_corner() {
        // A single corner on the deleted side (G_i < 0) ⇒ Refuted, even though a w-only
        // test on the other corners would pass — the re-entry TRIM-LOCAL exists to catch.
        let bad = TrimLocalCert {
            outer_fibers: vec![corners(1, -2, 3, 4)],
            confinement: reg_cert(&[1, 0, 1], &[1], Q::new(1, 2), span(-2, 2)),
        };
        assert!(matches!(trim_local(&bad), Verdict::Refuted(_)));
    }

    #[test]
    fn classify_fiber_over_all_corner_sign_patterns() {
        for &a in &[-1i128, 0, 1] {
            for &b in &[-1i128, 0, 1] {
                for &c in &[-1i128, 0, 1] {
                    for &d in &[-1i128, 0, 1] {
                        let neg = [a, b, c, d].iter().filter(|&&x| x < 0).count();
                        let expected = match neg {
                            4 => FiberCell::Empty,
                            0 => FiberCell::Full,
                            _ => FiberCell::Clipped,
                        };
                        assert_eq!(classify_fiber(&corners(a, b, c, d)), expected);
                    }
                }
            }
        }
    }

    #[test]
    fn clip_dom_counts_retained_components() {
        let full = corners(1, 1, 1, 1); // Full (retained)
        let empty = corners(-1, -1, -1, -1); // Empty
        let clipped = corners(1, -1, 1, 1); // Clipped (retained, clip active)
        // retained | empty | retained ⇒ two disconnected components; the clip is active.
        let split = [full.clone(), empty, clipped.clone()];
        let census = clip_dom(&split);
        assert_eq!(census.retained_components, 2);
        assert!(census.has_clip);
        // A single connected run of retained fibers.
        let connected = [full.clone(), clipped, full];
        assert_eq!(clip_dom(&connected).retained_components, 1);
    }

    #[test]
    fn edge_reg_passes_a_regular_speed() {
        let cert = EdgeRegCert::<Bignum> {
            speed_sq: reg_cert(&[1, 0, 1], &[1], Q::new(1, 2), span(-2, 2)), // reg_q Verifies
            failure: None,
        };
        assert!(matches!(edge_reg(&cert), EdgeReg::Pass(_)));
        assert!(matches!(edge_reg(&cert).to_verdict(), Verdict::Verified(_)));
    }

    #[test]
    fn edge_reg_stall_lowers_to_refuted_stalled_never_unresolved() {
        let cert = EdgeRegCert::<Bignum> {
            speed_sq: reg_cert(&[1, 0, 1], &[1], Q::from_i128(2), span(-2, 2)), // reg_q Refutes
            failure: Some(EdgeFail::Stalled {
                t_star: Q::from_i128(0),
                order: 2,
            }),
        };
        assert!(matches!(edge_reg(&cert), EdgeReg::Stall { order: 2, .. }));
        // Gate-failing as stored: Refuted(Stalled), never Unresolved.
        match edge_reg(&cert).to_verdict() {
            Verdict::Refuted(EdgeFail::Stalled { order, .. }) => assert_eq!(order, 2),
            _ => panic!("a stall must lower to Refuted(Stalled)"),
        }
    }

    #[test]
    fn edge_reg_cusp_lowers_to_refuted_cusp() {
        let cert = EdgeRegCert::<Bignum> {
            speed_sq: reg_cert(&[1, 0, 1], &[1], Q::from_i128(2), span(-2, 2)), // reg_q Refutes
            failure: Some(EdgeFail::Cusp(Q::from_i128(0))),
        };
        assert!(matches!(edge_reg(&cert), EdgeReg::Fail(_)));
        assert!(matches!(
            edge_reg(&cert).to_verdict(),
            Verdict::Refuted(EdgeFail::Cusp(_))
        ));
    }
}
