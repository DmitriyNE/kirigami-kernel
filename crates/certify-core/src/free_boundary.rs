//! The FREE-BOUNDARY / ANCHOR checker (spec §3.4:151, the **exact** part of ANCHOR
//! spec §8.5:372) — decide whether an authored substrate free boundary is a valid solid
//! footprint.
//!
//! The substrate free boundary is a **σ-band with rational μ-boundary splines**: two
//! authored curves `μ⁻(σ), μ⁺(σ)` over a σ-span `[σ_lo, σ_hi]`, lifted into a chart to
//! bound the retained material. This checker certifies the exact obligations that make
//! that band a valid closed-solid footprint, **composing** the reused
//! [`certify1d`](crate::certify1d) positivity foundations (the `slab_s0` / `trim_local`
//! mold — a bundle of searcher-supplied `RegCert`/`EdgeRegCert`, each with Sturm chains
//! the checker re-verifies before it trusts a count):
//!
//! - **positive width** `μ⁺(σ) − μ⁻(σ) ≥ m > 0` on the span — the band is non-degenerate,
//!   so the solid has a strictly positive cross-section (a [`reg_q`] instance);
//! - **boundary regularity** `|â′|² ≥ m` for each lifted μ-rail — the boundary curves are
//!   regular immersions, no cusp/stall (an [`edge_reg`] instance on the rail's squared
//!   speed);
//! - **σ̂-monotonicity** `σ̂′ ≥ m > 0` — the anchor's σ-projection is strictly monotone
//!   (a [`reg_q`] instance). For the σ-graph form (`σ̂ = σ`) this is trivially `σ̂′ = 1`;
//!   it is the composition-free slice of the general-`(σ(t), μ(t))` anchor obligation, so
//!   it is implemented and refutation-tested (a fold-back `σ̂` refuses) even though the
//!   σ-graph makes it trivial.
//!
//! The result is [`Verified(ValidFreeBoundary)`](crate::Verdict::Verified) /
//! [`Refuted(FreeBoundaryFault)`](crate::Verdict::Refuted); pure, total, `no_std`,
//! panic-free — the `arrange.rs` mold. This is the **exact** part of ANCHOR; the
//! *transcendental* backward-error bound `sup|D(â) − g| ≤ ε` and the DRC (`spec:402`) are
//! a separate milestone (DEV / M-E, `docs/vv-guide.md §8`), out of scope here.
//!
//! # Example
//!
//! ```
//! use certify_core::free_boundary::{free_boundary, FreeBoundaryCert};
//! use certify_core::certify1d::{EdgeRegCert, RegCert};
//! use certify_core::{MarginSq, Verdict};
//! use lattice::{Bignum, Interval, Poly, Rat, SturmChain};
//!
//! let poly = |cs: &[i128]| Poly::<Bignum>::from_coeffs(cs.iter().map(|&c| Rat::from_i128(c)).collect());
//! let span = Interval { lo: Rat::from_i128(-1), hi: Rat::from_i128(1) };
//! // A reg_q positivity cert `num/den ≥ m` with honest (searcher-supplied) Sturm chains.
//! let reg = |num: Poly<Bignum>, den: Poly<Bignum>, m: Rat<Bignum>| {
//!     let r = num.sub(&den.scale(&m));
//!     RegCert {
//!         den_chain: SturmChain::new(&den),
//!         res_chain: SturmChain::new(&r),
//!         num,
//!         den,
//!         m: MarginSq(m),
//!         span: span.clone(),
//!     }
//! };
//! // width μ⁺−μ⁻ ≡ 2 ≥ 1; each rail |â′|² = σ²+1 ≥ 1/2; σ̂′ ≡ 1 ≥ 1/2.
//! let cert = FreeBoundaryCert::<Bignum> {
//!     span: span.clone(),
//!     width: reg(poly(&[2]), poly(&[1]), Rat::from_i128(1)),
//!     reg_lo: EdgeRegCert { speed_sq: reg(poly(&[1, 0, 1]), poly(&[1]), Rat::new(1, 2)), failure: None },
//!     reg_hi: EdgeRegCert { speed_sq: reg(poly(&[1, 0, 1]), poly(&[1]), Rat::new(1, 2)), failure: None },
//!     monotone: reg(poly(&[1]), poly(&[1]), Rat::new(1, 2)),
//! };
//! assert!(matches!(free_boundary(&cert), Verdict::Verified(_)));
//! ```

use crate::certify1d::{EdgeFail, EdgeReg, EdgeRegCert, RegCert, RegFault, edge_reg, reg_q};
use crate::margin::MarginSq;
use crate::verdict::Verdict;
use lattice::{Backend, Bignum, Interval, Rat};

/// A FREE-BOUNDARY certificate: the authored σ-band substrate boundary `μ⁻(σ), μ⁺(σ)` over
/// `span`, as the bundle of exact-ANCHOR positivity obligations the checker re-verifies.
///
/// Each sub-certificate carries its own searcher-supplied Sturm chains (re-verified by the
/// reused [`reg_q`] / [`edge_reg`]); they must all be authored over the same `span`, which
/// the checker cross-checks ([`FreeBoundaryFault::SpanMismatch`]) so a positivity certified
/// on a *sub*-interval cannot masquerade as one over the whole footprint.
pub struct FreeBoundaryCert<B: Backend = Bignum> {
    /// The σ-span `[σ_lo, σ_hi]` the boundary is authored over (the footprint's σ-extent).
    pub span: Interval<B>,
    /// **Positive width** `μ⁺(σ) − μ⁻(σ) ≥ m > 0`: a [`reg_q`] cert whose `num/den` is the
    /// width rational function (the searcher forms `μ⁺ − μ⁻` and clears it to `num/den`).
    pub width: RegCert<B>,
    /// **Boundary regularity** of the lifted `μ⁻` rail: an [`edge_reg`] cert on its squared
    /// speed `|â′|² ≥ m` (the rail is `c + μ⁻·r` at `w = 0`).
    pub reg_lo: EdgeRegCert<B>,
    /// **Boundary regularity** of the lifted `μ⁺` rail (`c + μ⁺·r` at `w = 0`).
    pub reg_hi: EdgeRegCert<B>,
    /// **σ̂-monotonicity** `σ̂′ ≥ m > 0`: a [`reg_q`] cert on the anchor's σ-projection
    /// derivative. For the σ-graph form this is the constant `1` (`σ̂ = σ`).
    pub monotone: RegCert<B>,
}

/// The evidence a valid free boundary carries: the certified footprint span and the four
/// exact-ANCHOR positivity margins.
pub struct ValidFreeBoundary<B: Backend = Bignum> {
    /// The σ-span over which every obligation was certified.
    pub span: Interval<B>,
    /// The certified **positive-width** margin (`μ⁺ − μ⁻ ≥ width`).
    pub width: MarginSq<Rat<B>>,
    /// The certified regularity margin of the `μ⁻` rail (`|â′|² ≥ reg_lo`).
    pub reg_lo: MarginSq<Rat<B>>,
    /// The certified regularity margin of the `μ⁺` rail (`|â′|² ≥ reg_hi`).
    pub reg_hi: MarginSq<Rat<B>>,
    /// The certified **σ̂-monotonicity** margin (`σ̂′ ≥ monotone`).
    pub monotone: MarginSq<Rat<B>>,
}

/// Why the FREE-BOUNDARY checker refused a certificate.
pub enum FreeBoundaryFault<B: Backend = Bignum> {
    /// The σ-span is empty or degenerate (`σ_lo ≥ σ_hi`) — no footprint to certify.
    EmptySupport,
    /// A sub-certificate is authored over a span different from the claimed `span`, so it
    /// cannot compose into a whole-footprint claim (audit, not trust).
    SpanMismatch,
    /// **Positive width** failed: `μ⁺ − μ⁻` is not bounded `≥ m > 0` on the span — the two
    /// boundary splines cross or touch (a real degeneracy), or the paperwork is malformed.
    /// Carries the underlying [`reg_q`] fault.
    CrossedBounds(RegFault<B>),
    /// **Boundary regularity** failed: a lifted μ-rail is not a regular immersion (a
    /// geometric cusp or a removable parametrization stall). Carries the [`edge_reg`] witness.
    NonRegular(EdgeFail<B>),
    /// **σ̂-monotonicity** failed: the anchor's σ-projection is not strictly monotone (it
    /// folds back / stalls in σ). Carries the underlying [`reg_q`] fault.
    NonMonotone(RegFault<B>),
}

/// Whether two σ-spans are equal endpoint-for-endpoint (exact, `no_std`).
fn same_span<B: Backend>(a: &Interval<B>, b: &Interval<B>) -> bool {
    a.lo.cmp(&b.lo) == core::cmp::Ordering::Equal && a.hi.cmp(&b.hi) == core::cmp::Ordering::Equal
}

/// FREE-BOUNDARY / ANCHOR (exact part, spec §3.4 / §8.5): certify that an authored σ-band
/// substrate boundary is a valid closed-solid footprint — a non-degenerate positive width,
/// two regular boundary rails, and a strictly-monotone σ-projection, each over the claimed
/// `span`. Total — `Verified(`[`ValidFreeBoundary`]`)` or `Refuted(`[`FreeBoundaryFault`]`)`.
///
/// The checker is a **composition**: it re-derives nothing itself beyond the span guards; the
/// positivity is decided by the reused [`reg_q`] / [`edge_reg`] (which re-verify their
/// searcher-supplied Sturm chains, so a forged chain is rejected there). Its own additions
/// are the [`FreeBoundaryFault::EmptySupport`] and [`FreeBoundaryFault::SpanMismatch`] guards
/// and the free-boundary fault taxonomy.
pub fn free_boundary<B: Backend>(
    cert: &FreeBoundaryCert<B>,
) -> Verdict<ValidFreeBoundary<B>, FreeBoundaryFault<B>, ()> {
    // A footprint needs a non-degenerate σ-span.
    if cert.span.lo.cmp(&cert.span.hi) != core::cmp::Ordering::Less {
        return Verdict::Refuted(FreeBoundaryFault::EmptySupport);
    }
    // Every sub-certificate must be authored over the *same* span, or the composed
    // whole-footprint claim is unsound (a positivity on a sub-interval says nothing about
    // the rest of the band). Audit it, never trust the searcher's alignment.
    if !same_span(&cert.width.span, &cert.span)
        || !same_span(&cert.monotone.span, &cert.span)
        || !same_span(&cert.reg_lo.speed_sq.span, &cert.span)
        || !same_span(&cert.reg_hi.speed_sq.span, &cert.span)
    {
        return Verdict::Refuted(FreeBoundaryFault::SpanMismatch);
    }

    // Positive width: μ⁺ − μ⁻ ≥ m > 0.
    let width = match reg_q(&cert.width) {
        Verdict::Verified(m) => m,
        Verdict::Refuted(f) => return Verdict::Refuted(FreeBoundaryFault::CrossedBounds(f)),
        // reg_q is total (never Unresolved); route the impossible arm to a paperwork fault
        // rather than panic, keeping the checker panic-free.
        Verdict::Unresolved(()) => {
            return Verdict::Refuted(FreeBoundaryFault::CrossedBounds(
                RegFault::NonPositiveMargin,
            ));
        }
    };

    // σ̂-monotonicity: σ̂′ ≥ m > 0.
    let monotone = match reg_q(&cert.monotone) {
        Verdict::Verified(m) => m,
        Verdict::Refuted(f) => return Verdict::Refuted(FreeBoundaryFault::NonMonotone(f)),
        Verdict::Unresolved(()) => {
            return Verdict::Refuted(FreeBoundaryFault::NonMonotone(RegFault::NonPositiveMargin));
        }
    };

    // Boundary regularity of each lifted μ-rail: |â′|² ≥ m.
    let reg_lo = match edge_reg(&cert.reg_lo) {
        EdgeReg::Pass(m) => m,
        EdgeReg::Fail(t) => {
            return Verdict::Refuted(FreeBoundaryFault::NonRegular(EdgeFail::Cusp(t)));
        }
        EdgeReg::Stall { t_star, order } => {
            return Verdict::Refuted(FreeBoundaryFault::NonRegular(EdgeFail::Stalled {
                t_star,
                order,
            }));
        }
    };
    let reg_hi = match edge_reg(&cert.reg_hi) {
        EdgeReg::Pass(m) => m,
        EdgeReg::Fail(t) => {
            return Verdict::Refuted(FreeBoundaryFault::NonRegular(EdgeFail::Cusp(t)));
        }
        EdgeReg::Stall { t_star, order } => {
            return Verdict::Refuted(FreeBoundaryFault::NonRegular(EdgeFail::Stalled {
                t_star,
                order,
            }));
        }
    };

    Verdict::Verified(ValidFreeBoundary {
        span: cert.span.clone(),
        width,
        reg_lo,
        reg_hi,
        monotone,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice::{Poly, SturmChain};

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
    /// A REG-Q cert with honest (searcher-correct) Sturm chains for `num/den > m` on `iv`.
    fn reg(num: &[i128], den: &[i128], m: Q, iv: Interval<Bignum>) -> RegCert<Bignum> {
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
    /// A regular EDGE-REG cert (`|â′|² = num/den ≥ m`) with honest chains and no failure tag.
    fn edge(num: &[i128], den: &[i128], m: Q, iv: Interval<Bignum>) -> EdgeRegCert<Bignum> {
        EdgeRegCert {
            speed_sq: reg(num, den, m, iv),
            failure: None,
        }
    }

    /// A valid tapered μ-band: width `μ⁺−μ⁻ = 2 − 2σ ≥ 2` on `[−1/8, 0]`, both rail speeds
    /// `σ²+1 ≥ 1/2`, and the σ-graph monotonicity `σ̂′ ≡ 1`. Certifies.
    #[test]
    fn a_valid_mu_band_certifies() {
        let iv = span(-1, 0);
        let cert = FreeBoundaryCert::<Bignum> {
            span: iv.clone(),
            // μ⁺ − μ⁻ = (1 − σ) − (−1 + σ) = 2 − 2σ ∈ [2, 4] ≥ 1 (strict) on σ ∈ [−1, 0].
            width: reg(&[2, -2], &[1], Q::from_i128(1), iv.clone()),
            reg_lo: edge(&[1, 0, 1], &[1], Q::new(1, 2), iv.clone()),
            reg_hi: edge(&[1, 0, 1], &[1], Q::new(1, 2), iv.clone()),
            monotone: reg(&[1], &[1], Q::new(1, 2), iv.clone()),
        };
        match free_boundary(&cert) {
            Verdict::Verified(v) => assert!(same_span(&v.span, &iv)),
            _ => panic!("a valid μ-band must certify"),
        }
    }

    /// Crossed bounds: `μ⁺ − μ⁻ = σ` dips to 0 (and below) on `[−1, 1]` ⇒ width refuted.
    #[test]
    fn crossed_bounds_are_refuted() {
        let iv = span(-1, 1);
        let cert = FreeBoundaryCert::<Bignum> {
            span: iv.clone(),
            width: reg(&[0, 1], &[1], Q::new(1, 4), iv.clone()), // σ, not ≥ 1/4 on [−1,1]
            reg_lo: edge(&[1, 0, 1], &[1], Q::new(1, 2), iv.clone()),
            reg_hi: edge(&[1, 0, 1], &[1], Q::new(1, 2), iv.clone()),
            monotone: reg(&[1], &[1], Q::new(1, 2), iv.clone()),
        };
        assert!(matches!(
            free_boundary(&cert),
            Verdict::Refuted(FreeBoundaryFault::CrossedBounds(_))
        ));
    }

    /// A non-regular boundary rail: the `μ⁺` rail's squared speed `σ²` vanishes at σ = 0 on
    /// `[−1, 1]` (a cusp) ⇒ regularity refuted.
    #[test]
    fn a_non_regular_rail_is_refuted() {
        let iv = span(-1, 1);
        let cert = FreeBoundaryCert::<Bignum> {
            span: iv.clone(),
            width: reg(&[2], &[1], Q::from_i128(1), iv.clone()),
            reg_lo: edge(&[1, 0, 1], &[1], Q::new(1, 2), iv.clone()),
            // |â′|² = σ² — zero at σ = 0, so no positive margin.
            reg_hi: edge(&[0, 0, 1], &[1], Q::new(1, 2), iv.clone()),
            monotone: reg(&[1], &[1], Q::new(1, 2), iv.clone()),
        };
        assert!(matches!(
            free_boundary(&cert),
            Verdict::Refuted(FreeBoundaryFault::NonRegular(_))
        ));
    }

    /// A fold-back anchor: `σ̂′ = 1 − 2σ` changes sign on `[0, 1]` (σ̂ stalls at σ = 1/2) ⇒
    /// monotonicity refuted. This is the arm the σ-graph makes trivial but a general anchor
    /// exercises.
    #[test]
    fn a_fold_back_anchor_is_non_monotone() {
        let iv = span(0, 1);
        let cert = FreeBoundaryCert::<Bignum> {
            span: iv.clone(),
            width: reg(&[2], &[1], Q::from_i128(1), iv.clone()),
            reg_lo: edge(&[1, 0, 1], &[1], Q::new(1, 2), iv.clone()),
            reg_hi: edge(&[1, 0, 1], &[1], Q::new(1, 2), iv.clone()),
            // σ̂′ = 1 − 2σ, positive at 0 but crosses zero in (0, 1].
            monotone: reg(&[1, -2], &[1], Q::new(1, 4), iv.clone()),
        };
        assert!(matches!(
            free_boundary(&cert),
            Verdict::Refuted(FreeBoundaryFault::NonMonotone(_))
        ));
    }

    /// A degenerate span (`σ_lo ≥ σ_hi`) is refused before any positivity check.
    #[test]
    fn an_empty_support_is_refuted() {
        let iv = span(1, 1); // lo == hi
        let cert = FreeBoundaryCert::<Bignum> {
            span: iv.clone(),
            width: reg(&[2], &[1], Q::from_i128(1), iv.clone()),
            reg_lo: edge(&[1, 0, 1], &[1], Q::new(1, 2), iv.clone()),
            reg_hi: edge(&[1, 0, 1], &[1], Q::new(1, 2), iv.clone()),
            monotone: reg(&[1], &[1], Q::new(1, 2), iv.clone()),
        };
        assert!(matches!(
            free_boundary(&cert),
            Verdict::Refuted(FreeBoundaryFault::EmptySupport)
        ));
    }

    /// A sub-certificate authored over a different span cannot compose into a
    /// whole-footprint claim — refused `SpanMismatch` (audit, not trust).
    #[test]
    fn a_sub_cert_on_a_different_span_is_refused() {
        let iv = span(-1, 0);
        let cert = FreeBoundaryCert::<Bignum> {
            span: iv.clone(),
            width: reg(&[2], &[1], Q::from_i128(1), iv.clone()),
            reg_lo: edge(&[1, 0, 1], &[1], Q::new(1, 2), iv.clone()),
            reg_hi: edge(&[1, 0, 1], &[1], Q::new(1, 2), iv.clone()),
            // monotone certified on a wider span than the claim.
            monotone: reg(&[1], &[1], Q::new(1, 2), span(-2, 2)),
        };
        assert!(matches!(
            free_boundary(&cert),
            Verdict::Refuted(FreeBoundaryFault::SpanMismatch)
        ));
    }

    /// A forged Sturm chain in a sub-certificate is rejected (inherited from `reg_q`'s
    /// chain re-verification) — the free-boundary checker never trusts unverified evidence.
    #[test]
    fn a_forged_chain_is_rejected() {
        let iv = span(-1, 0);
        // A non-constant residual (R = σ²+1/2), so a forged constant chain provably mismatches.
        let mut width = reg(&[1, 0, 1], &[1], Q::new(1, 2), iv.clone());
        width.res_chain = SturmChain::new(&poly(&[7])); // not R's chain
        let cert = FreeBoundaryCert::<Bignum> {
            span: iv.clone(),
            width,
            reg_lo: edge(&[1, 0, 1], &[1], Q::new(1, 2), iv.clone()),
            reg_hi: edge(&[1, 0, 1], &[1], Q::new(1, 2), iv.clone()),
            monotone: reg(&[1], &[1], Q::new(1, 2), iv.clone()),
        };
        assert!(matches!(
            free_boundary(&cert),
            Verdict::Refuted(FreeBoundaryFault::CrossedBounds(
                RegFault::InvalidResidualChain
            ))
        ));
    }
}
