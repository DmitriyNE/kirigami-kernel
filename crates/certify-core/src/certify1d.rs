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

/// The `(min, max)` of a set of corner values, or `None` if empty.
///
/// Sound as the box range **only for an affine form**, whose extremum over a box is
/// attained at a corner (spec §8.2 convexity rider — never use this on a form whose
/// interior can beat its corners). The CLIP-σ signed test ranges `∂_σG` (affine) this way.
pub fn corner_range<B: Backend>(corners: &[Rat<B>]) -> Option<(Rat<B>, Rat<B>)> {
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
        assert_eq!(corner_range::<Bignum>(&[]), None);
    }
}
