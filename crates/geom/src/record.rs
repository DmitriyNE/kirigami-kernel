//! The certified single-chart record (spec §8) — an **unforgeable, claim-bound** certified
//! chart.
//!
//! A [`CertifiedChart`] bundles a [`Chart`](crate::chart::Chart) with the regularity margins
//! the M2 checkers established on its exact fields, over an explicit [`ChartDomain`]. It is
//! opaque *and* its evidence is bound to the chart: the only constructor,
//! [`CertifiedChart::certify`], **re-derives** the checked quantities (`|q|²`, `|n′|²`, and
//! `det J` at the domain's `(μ,w)` box corners) from `chart + domain` itself via
//! [`regularity_targets`], **recomputes** the tag, and uses the supplied [`ChartEvidence`]
//! (Sturm chains + margins) *only* as the search artifacts to verify against those derived
//! targets. So a certificate built for one chart cannot be attached to another — the chains
//! would fail to verify against the second chart's quantities — and a margin is always
//! qualified by the domain it was proved on. Holding a `CertifiedChart` therefore *is* the
//! certificate. It is the Milestone-B exit artifact; the device-cone instance is built in the
//! `fixtures` crate.

use crate::chart::Chart;
use crate::tags::{Tag, classify};
use certify_core::certify1d::{RegCert, reg_q};
use certify_core::{MarginSq, Verdict};
use lattice::{Backend, Bignum, Interval, Poly, Rat, SturmChain};

/// The `(μ, w)`-box × σ-span domain a chart's regularity is certified over. Regularity is
/// domain-dependent, so this is stored in the [`CertifiedChart`] — a margin proved on
/// `[0, 1]` is not a property of the chart everywhere.
pub struct ChartDomain<B: Backend = Bignum> {
    /// The σ support span.
    pub sigma: Interval<B>,
    /// The offset `μ` box, `(lo, hi)`.
    pub mu: (Rat<B>, Rat<B>),
    /// The offset `w` box, `(lo, hi)`.
    pub w: (Rat<B>, Rat<B>),
}

/// A searcher's evidence for one REG-Q target: the separation `margin` it claims and the
/// Sturm chains of `den` and of the residual `R = num − margin·den`. The checker supplies the
/// `num`/`den` (derived from the chart), re-verifies the chains against them, and counts.
pub struct RegEvidence<B: Backend = Bignum> {
    /// The claimed separation margin.
    pub margin: Rat<B>,
    /// The searcher-supplied Sturm chain of the target's denominator.
    pub den_chain: SturmChain<B>,
    /// The searcher-supplied Sturm chain of the residual `R = num − margin·den`.
    pub res_chain: SturmChain<B>,
}

/// The search artifacts a [`CertifiedChart::certify`] call verifies — one [`RegEvidence`] per
/// regularity target: `|q|²`, `|n′|²`, and `det J` at the four `(μ,w)` box corners (in the
/// [`regularity_targets`] order). The checker derives the *targets* itself, so this carries
/// only the searcher's margins and Sturm chains, never the polynomials being checked.
pub struct ChartEvidence<B: Backend = Bignum> {
    /// REG-Q evidence for `|q|²`.
    pub q: RegEvidence<B>,
    /// REG-Q evidence for `|n′|²`.
    pub ruling: RegEvidence<B>,
    /// REG-Q evidence for `det J` at the four `(μ,w)` box corners, `[(μlo,wlo), (μlo,whi),
    /// (μhi,wlo), (μhi,whi)]`.
    pub slab: [RegEvidence<B>; 4],
}

/// Which regularity check [`CertifiedChart::certify`] refuted.
pub enum ChartFault<B: Backend = Bignum> {
    /// The chart does not classify to a primitive tag.
    Untagged,
    /// REG-Q on `|q|²` was refuted (with the σ witness `reg_q` reported).
    QReg(Rat<B>),
    /// REG-Q on `|n′|²` was refuted.
    RulingReg(Rat<B>),
    /// `det J` positivity was refuted at box corner `corner` (`0..4`).
    Slab {
        /// Which `(μ,w)` box corner (index into [`regularity_targets`]'s slab block).
        corner: usize,
        /// The σ witness `reg_q` reported.
        sigma: Rat<B>,
    },
}

/// The √-cleared regularity targets `(num, den)` a [`CertifiedChart`] proves strictly
/// positive on the domain span — derived **purely from `chart + domain`**: `|q|²`, then
/// `|n′|²`, then `det J` at the four `(μ,w)` box corners `[(μlo,wlo), (μlo,whi), (μhi,wlo),
/// (μhi,whi)]`. `det J` is affine in `(μ,w)`, so its box minimum is a corner minimum;
/// positivity at all four corners over the span is exactly `inf(det J) > 0` on the box (the
/// SLAB guarantee). Both the checker (to verify) and a searcher (to build Sturm chains) call
/// this, so the two cannot derive different polynomials.
pub fn regularity_targets<B: Backend>(
    chart: &Chart<B>,
    domain: &ChartDomain<B>,
) -> [(Poly<B>, Poly<B>); 6] {
    let q = (
        chart.normal().den().clone(),
        Poly::constant(Rat::from_i128(1)),
    );
    let n1 = chart.normal_deriv_sq().reduce();
    let ruling = (n1.num().clone(), n1.den().clone());
    let dj = chart.det_j();
    let corner = |mc: &Rat<B>, wc: &Rat<B>| {
        let d = dj
            .constant
            .add(&dj.mu.scale(mc))
            .add(&dj.w.scale(wc))
            .reduce();
        (d.num().clone(), d.den().clone())
    };
    let (mu_lo, mu_hi) = (&domain.mu.0, &domain.mu.1);
    let (w_lo, w_hi) = (&domain.w.0, &domain.w.1);
    [
        q,
        ruling,
        corner(mu_lo, w_lo),
        corner(mu_lo, w_hi),
        corner(mu_hi, w_lo),
        corner(mu_hi, w_hi),
    ]
}

/// A certified single-chart record (spec §8): a chart, its recomputed primitive tag, the
/// domain, the regularity margins the M2 checkers established, and the mesh curvature cap.
///
/// Opaque and immutable — construct it only via [`CertifiedChart::certify`], whose evidence is
/// bound to the chart (see the module docs). Read the contents through the accessors.
pub struct CertifiedChart<B: Backend = Bignum> {
    chart: Chart<B>,
    tag: Tag<B>,
    domain: ChartDomain<B>,
    q_margin: MarginSq<Rat<B>>,
    ruling_margin: MarginSq<Rat<B>>,
    slab_margins: [MarginSq<Rat<B>>; 4],
    kappa_cap: Rat<B>,
}

/// Build the derived REG-Q certificate for `target` from the supplied evidence and run the
/// verified `reg_q` checker (which re-verifies the chains before counting).
fn check_reg<B: Backend>(
    target: &(Poly<B>, Poly<B>),
    ev: RegEvidence<B>,
    span: &Interval<B>,
) -> Verdict<MarginSq<Rat<B>>, Rat<B>, ()> {
    reg_q(&RegCert {
        num: target.0.clone(),
        den: target.1.clone(),
        m: MarginSq(ev.margin),
        span: span.clone(),
        den_chain: ev.den_chain,
        res_chain: ev.res_chain,
    })
}

impl<B: Backend> CertifiedChart<B> {
    /// Certify a chart over `domain` (spec §8). Re-derives the regularity targets from
    /// `chart + domain` ([`regularity_targets`]), recomputes the tag, verifies each supplied
    /// [`RegEvidence`] against its derived target with the `reg_q`/SLAB checkers, and — only
    /// if the tag classifies and all six checks [`Verdict::Verified`] — mints the
    /// [`CertifiedChart`]. `Refuted(ChartFault)` names the first failing check; the M2
    /// checkers are total, so `Unresolved` does not occur (it is threaded through only for
    /// totality).
    ///
    /// Because the checker derives what it checks, the evidence cannot be transplanted from a
    /// different chart (its chains fail to verify against this chart's quantities), and the
    /// certified margins are always qualified by the stored `domain`.
    ///
    /// `kappa_cap` is the searcher-computed mesh step `min(s_max, 1/κ₁)`; it rides along as
    /// derived data and is **not** itself certified here — the guarantee is the tag plus the
    /// regularity margins over `domain`.
    pub fn certify(
        chart: Chart<B>,
        domain: ChartDomain<B>,
        evidence: ChartEvidence<B>,
        kappa_cap: Rat<B>,
    ) -> Verdict<CertifiedChart<B>, ChartFault<B>, ()> {
        // Recompute the tag — never trust a supplied one.
        let tag = match classify(&chart) {
            Some(t) => t,
            None => return Verdict::Refuted(ChartFault::Untagged),
        };
        // Derive the six targets from chart + domain; the evidence is checked against THESE.
        let targets = regularity_targets(&chart, &domain);
        let ChartEvidence { q, ruling, slab } = evidence;
        let [c0, c1, c2, c3] = slab;

        let q_margin = match check_reg(&targets[0], q, &domain.sigma) {
            Verdict::Verified(m) => m,
            Verdict::Refuted(s) => return Verdict::Refuted(ChartFault::QReg(s)),
            Verdict::Unresolved(()) => return Verdict::Unresolved(()),
        };
        let ruling_margin = match check_reg(&targets[1], ruling, &domain.sigma) {
            Verdict::Verified(m) => m,
            Verdict::Refuted(s) => return Verdict::Refuted(ChartFault::RulingReg(s)),
            Verdict::Unresolved(()) => return Verdict::Unresolved(()),
        };
        // SLAB: det J > 0 at every (μ,w) box corner ⇔ inf(det J) > 0 on the box (affine).
        let s0 = match check_reg(&targets[2], c0, &domain.sigma) {
            Verdict::Verified(m) => m,
            Verdict::Refuted(s) => {
                return Verdict::Refuted(ChartFault::Slab {
                    corner: 0,
                    sigma: s,
                });
            }
            Verdict::Unresolved(()) => return Verdict::Unresolved(()),
        };
        let s1 = match check_reg(&targets[3], c1, &domain.sigma) {
            Verdict::Verified(m) => m,
            Verdict::Refuted(s) => {
                return Verdict::Refuted(ChartFault::Slab {
                    corner: 1,
                    sigma: s,
                });
            }
            Verdict::Unresolved(()) => return Verdict::Unresolved(()),
        };
        let s2 = match check_reg(&targets[4], c2, &domain.sigma) {
            Verdict::Verified(m) => m,
            Verdict::Refuted(s) => {
                return Verdict::Refuted(ChartFault::Slab {
                    corner: 2,
                    sigma: s,
                });
            }
            Verdict::Unresolved(()) => return Verdict::Unresolved(()),
        };
        let s3 = match check_reg(&targets[5], c3, &domain.sigma) {
            Verdict::Verified(m) => m,
            Verdict::Refuted(s) => {
                return Verdict::Refuted(ChartFault::Slab {
                    corner: 3,
                    sigma: s,
                });
            }
            Verdict::Unresolved(()) => return Verdict::Unresolved(()),
        };

        Verdict::Verified(CertifiedChart {
            chart,
            tag,
            domain,
            q_margin,
            ruling_margin,
            slab_margins: [s0, s1, s2, s3],
            kappa_cap,
        })
    }

    /// The certified chart.
    pub fn chart(&self) -> &Chart<B> {
        &self.chart
    }
    /// The recomputed primitive-surface classification (cone / cylinder).
    pub fn tag(&self) -> &Tag<B> {
        &self.tag
    }
    /// The domain the regularity margins are certified over.
    pub fn domain(&self) -> &ChartDomain<B> {
        &self.domain
    }
    /// The REG-Q margin on `|q|²` (the quaternion spline's non-degeneracy on `domain`).
    pub fn q_margin(&self) -> &MarginSq<Rat<B>> {
        &self.q_margin
    }
    /// The REG-Q margin on `|n′|²` (the ruling's non-degeneracy on `domain`).
    pub fn ruling_margin(&self) -> &MarginSq<Rat<B>> {
        &self.ruling_margin
    }
    /// The SLAB margins — `det J > 0` at the four `(μ,w)` box corners (thus on the whole box).
    pub fn slab_margins(&self) -> &[MarginSq<Rat<B>>; 4] {
        &self.slab_margins
    }
    /// The mesh curvature cap `min(s_max, 1/κ₁)`. Searcher-derived, **not** certified here.
    pub fn kappa_cap(&self) -> &Rat<B> {
        &self.kappa_cap
    }
}
