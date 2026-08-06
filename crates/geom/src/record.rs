//! The certified single-chart record (spec §8) — an **unforgeable** certified chart.
//!
//! A [`CertifiedChart`] bundles a [`Chart`](crate::chart::Chart) with the regularity margins
//! the M2 `certify_core::certify1d` checkers established on its exact fields, plus the mesh
//! curvature cap. It is **opaque**: its only constructor, [`CertifiedChart::certify`], runs
//! those checkers and yields the wrapper *only if every one verifies*, so holding a
//! `CertifiedChart` is itself the certificate — the value cannot be forged by placing a
//! `Verdict::Verified` in a field, a margin cannot be transplanted from another chart, and
//! the chart cannot be swapped out afterward. It is the Milestone-B exit artifact; the
//! device-cone instance is built in the `fixtures` crate.

use crate::chart::Chart;
use crate::tags::Tag;
use certify_core::certify1d::{RegCert, SlabS0Cert, reg_q, slab_s0};
use certify_core::{MarginSq, Verdict};
use lattice::{Backend, Bignum, Rat};

/// Which regularity certificate [`CertifiedChart::certify`] refuted, carrying the σ witness
/// where the margin failed.
pub enum ChartFault<B: Backend = Bignum> {
    /// REG-Q on `|q|²` failed — the quaternion spline degenerates at this σ.
    QReg(Rat<B>),
    /// REG-Q on `|n′|²` failed — the ruling stalls at this σ.
    RulingReg(Rat<B>),
    /// SLAB-S0 failed — the offset slab loses regularity at this σ.
    Slab(Rat<B>),
}

/// A certified single-chart record (spec §8): a chart, its primitive classification, the
/// three regularity margins the M2 checkers established, and the mesh curvature cap.
///
/// Opaque and immutable — construct it only via [`CertifiedChart::certify`]. Every field is
/// private, so holding a value *is* the proof: a `Verdict::Verified` cannot be forged into
/// it, and the chart/margins cannot be altered after certification. Read the contents through
/// the accessors.
pub struct CertifiedChart<B: Backend = Bignum> {
    chart: Chart<B>,
    tag: Tag<B>,
    q_margin: MarginSq<Rat<B>>,
    ruling_margin: MarginSq<Rat<B>>,
    slab_margin: MarginSq<Rat<B>>,
    kappa_cap: Rat<B>,
}

impl<B: Backend> CertifiedChart<B> {
    /// Certify a chart (spec §8): run REG-Q on `|q|²` and `|n′|²` and SLAB-S0 on `det J`
    /// over the supplied certificates, and — only if all three [`Verdict::Verified`] — mint
    /// the [`CertifiedChart`]. `Refuted(ChartFault)` names the first failing certificate with
    /// its σ witness; the M2 positivity checkers are total, so `Unresolved` does not occur in
    /// practice (it is threaded through only for totality).
    ///
    /// `kappa_cap` is the searcher-computed mesh step `min(s_max, 1/κ₁)`; it rides along as
    /// derived data — the certified guarantee is the three regularity margins, not this value.
    pub fn certify(
        chart: Chart<B>,
        tag: Tag<B>,
        q_cert: RegCert<B>,
        ruling_cert: RegCert<B>,
        slab_cert: SlabS0Cert<B>,
        kappa_cap: Rat<B>,
    ) -> Verdict<CertifiedChart<B>, ChartFault<B>, ()> {
        let q_margin = match reg_q(&q_cert) {
            Verdict::Verified(m) => m,
            Verdict::Refuted(s) => return Verdict::Refuted(ChartFault::QReg(s)),
            Verdict::Unresolved(()) => return Verdict::Unresolved(()),
        };
        let ruling_margin = match reg_q(&ruling_cert) {
            Verdict::Verified(m) => m,
            Verdict::Refuted(s) => return Verdict::Refuted(ChartFault::RulingReg(s)),
            Verdict::Unresolved(()) => return Verdict::Unresolved(()),
        };
        let slab_margin = match slab_s0(&slab_cert) {
            Verdict::Verified(m) => m,
            Verdict::Refuted(s) => return Verdict::Refuted(ChartFault::Slab(s)),
            Verdict::Unresolved(()) => return Verdict::Unresolved(()),
        };
        Verdict::Verified(CertifiedChart {
            chart,
            tag,
            q_margin,
            ruling_margin,
            slab_margin,
            kappa_cap,
        })
    }

    /// The certified chart.
    pub fn chart(&self) -> &Chart<B> {
        &self.chart
    }
    /// The primitive-surface classification (cone / cylinder), with its exact witness.
    pub fn tag(&self) -> &Tag<B> {
        &self.tag
    }
    /// The REG-Q margin on `|q|²` — the quaternion spline is non-degenerate on the support.
    pub fn q_margin(&self) -> &MarginSq<Rat<B>> {
        &self.q_margin
    }
    /// The REG-Q margin on `|n′|²` — the ruling is non-degenerate (no stall) on the support.
    pub fn ruling_margin(&self) -> &MarginSq<Rat<B>> {
        &self.ruling_margin
    }
    /// The SLAB-S0 margin — the offset slab stays regular (`inf(det J) > 0`) across the box.
    pub fn slab_margin(&self) -> &MarginSq<Rat<B>> {
        &self.slab_margin
    }
    /// The mesh curvature cap `min(s_max, 1/κ₁)` — the largest admissible mesh step, from the
    /// support's tightest principal radius `1/κ₁`.
    pub fn kappa_cap(&self) -> &Rat<B> {
        &self.kappa_cap
    }
}
