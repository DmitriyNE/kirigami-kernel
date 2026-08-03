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

use crate::margin::MarginSq;
use crate::verdict::Verdict;
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

/// REG-Q (spec §8.5): certify `num/den > m` on the span, with `den > 0`. Total —
/// `Verified(m)` or `Refuted(σ)` at a span point where the margin fails.
pub fn reg_q<B: Backend>(cert: &RegCert<B>) -> Verdict<MarginSq<Rat<B>>, Rat<B>, ()> {
    let (lo, hi) = (&cert.span.lo, &cert.span.hi);
    let r = cert.num.sub(&cert.den.scale(&cert.m.0)); // R = num − m·den

    // Re-verify the supplied chains — a chain we did not check is not evidence.
    if !cert.den_chain.verify_chain(&cert.den) || !cert.res_chain.verify_chain(&r) {
        return Verdict::Refuted(lo.clone());
    }
    // den > 0 on [lo, hi]: positive at the closed lower end and no root in (lo, hi].
    if cert.den.eval(lo).sign() <= 0 || cert.den_chain.count_in(lo, hi) != 0 {
        return Verdict::Refuted(lo.clone());
    }
    // R > 0 on [lo, hi] ⇒ num > m·den ⇒ num/den > m (den > 0).
    if r.eval(lo).sign() <= 0 || cert.res_chain.count_in(lo, hi) != 0 {
        return Verdict::Refuted(lo.clone());
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
pub fn slab_s0<B: Backend>(cert: &SlabS0Cert<B>) -> Verdict<MarginSq<Rat<B>>, Rat<B>, ()> {
    let core = reg_q(&cert.core);
    if !matches!(core, Verdict::Verified(_)) {
        return core;
    }
    if let Some(sl) = &cert.stall_end {
        if sl.value_lo.sign() <= 0 {
            return Verdict::Refuted(sl.value_lo.clone());
        }
        if sl.value_hi.sign() <= 0 {
            return Verdict::Refuted(sl.value_hi.clone());
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

/// CLIP-a (spec §8.5): the fiber misses Π by a separated constant — `a² ≥ m_a` — and so
/// takes the uniform side `sgn a`. `Verified(sign)` / `Unresolved(a²)`; never `Refuted`.
pub fn clip_a<B: Backend>(cert: &ClipACert<B>) -> Verdict<ClipBranch, (), Rat<B>> {
    let a_sq = cert.a.mul(&cert.a);
    // `a² ≥ m_a` with `m_a > 0` forces `a ≠ 0`, so the sign is a genuine side.
    if a_sq.cmp(&cert.m_a.0) != core::cmp::Ordering::Less {
        match cert.a.sign() {
            1 => return Verdict::Verified(ClipBranch::Positive),
            -1 => return Verdict::Verified(ClipBranch::Negative),
            _ => {} // a = 0 (only reachable at m_a = 0): not separated ⇒ Unresolved.
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

/// The CLIP transversality ladder (spec §8.5): CLIP-W over the whole span → CLIP-μ on the
/// failing sub-spans → per isolated common zero one of [`clip_a`] / [`clip_sigma`] / an
/// osculation reject. Short-circuits: the first fully-certified rung wins.
///
/// `w` and `mu` are [`reg_q`] positivity certificates (the cleared `(n·b_J)²`, `(r·b_J)²`
/// gauges); the searcher supplies the failing sub-spans and the isolated common zeros.
/// A [`ClipVerdict::Subdivide`] means the supplied certificate did not converge — refine
/// and re-run; only [`ZeroClip::Osculation`] yields [`ClipVerdict::Rejected`].
pub fn clip<B: Backend>(w: &RegCert<B>, mu: &[RegCert<B>], zeros: &[ZeroClip<B>]) -> ClipVerdict {
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
    if all_resolved {
        ClipVerdict::Certified
    } else {
        ClipVerdict::Subdivide
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    }

    #[test]
    fn clip_ladder_certifies_when_clip_w_passes() {
        let w = reg_cert(&[1, 0, 1], &[1], Q::new(1, 2), span(-2, 2)); // reg_q Verifies
        assert_eq!(clip::<Bignum>(&w, &[], &[]), ClipVerdict::Certified);
    }

    #[test]
    fn clip_ladder_certifies_a_common_zero_via_clip_a() {
        let w = reg_cert(&[1, 0, 1], &[1], Q::from_i128(2), span(-2, 2)); // reg_q Refutes
        let zeros = [ZeroClip::ByA(ClipACert {
            a: Q::from_i128(5),
            m_a: MarginSq(Q::from_i128(1)),
        })];
        assert_eq!(clip(&w, &[], &zeros), ClipVerdict::Certified);
    }

    #[test]
    fn clip_ladder_rejects_an_osculation() {
        let w = reg_cert(&[1, 0, 1], &[1], Q::from_i128(2), span(-2, 2)); // reg_q Refutes
        let zeros = [ZeroClip::<Bignum>::Osculation];
        assert_eq!(clip(&w, &[], &zeros), ClipVerdict::Rejected);
    }

    #[test]
    fn clip_ladder_subdivides_on_an_unresolved_zero() {
        let w = reg_cert(&[1, 0, 1], &[1], Q::from_i128(2), span(-2, 2)); // reg_q Refutes
        let zeros = [ZeroClip::BySigma(ClipSigmaCert {
            corners: corners(-1, 1, -1, 1), // straddles ⇒ clip_sigma Unresolved
            m_sigma: Q::new(1, 4),
        })];
        assert_eq!(clip(&w, &[], &zeros), ClipVerdict::Subdivide);
        // No zeros supplied at all ⇒ also Subdivide (failure not yet localized).
        assert_eq!(clip::<Bignum>(&w, &[], &[]), ClipVerdict::Subdivide);
    }
}
