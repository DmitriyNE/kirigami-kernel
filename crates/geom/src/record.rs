//! The certified single-chart record (spec §8) — the assembled output of a certified
//! chart: its primitive tag, the M2 regularity/slab verdicts, and the mesh curvature cap.
//!
//! A [`ChartRecord`] bundles a [`Chart`](crate::chart::Chart) with the verdicts a searcher
//! obtains by running the `certify_core::certify1d` checkers on its exact fields, plus the
//! `min(s_max, 1/κ₁)` mesh step cap. It is the Milestone-B exit artifact; the device-cone
//! instance is built in the `fixtures` crate.

use crate::chart::Chart;
use crate::tags::Tag;
use certify_core::{MarginSq, Verdict};
use lattice::{Backend, Bignum, Rat};

/// The verdict shape the M2 positivity checkers ([`reg_q`](certify_core::certify1d::reg_q),
/// [`slab_s0`](certify_core::certify1d::slab_s0)) return: `Verified(margin)` or a refuting
/// σ, never `Unresolved` (Sturm is exact).
pub type RegVerdict<B> = Verdict<MarginSq<Rat<B>>, Rat<B>, ()>;

/// A certified single-chart record (spec §8): a chart, its primitive classification, the
/// regularity/slab verdicts, and the mesh curvature cap. Fully certified iff every verdict
/// [`Verdict::Verified`] — see [`ChartRecord::is_certified`].
pub struct ChartRecord<B: Backend = Bignum> {
    /// The chart whose exact fields the record certifies.
    pub chart: Chart<B>,
    /// The primitive-surface classification (cone / cylinder), with its exact witness.
    pub tag: Tag<B>,
    /// REG-Q on `|q|²`: the quaternion spline is non-degenerate on the support.
    pub q_reg: RegVerdict<B>,
    /// REG-Q on `|n′|²`: the ruling is non-degenerate (no stall) on the support.
    pub ruling_reg: RegVerdict<B>,
    /// SLAB-S0: the offset slab stays regular (`inf(det J) > 0`) across the `(μ, w)` box.
    pub slab: RegVerdict<B>,
    /// The mesh curvature cap `min(s_max, 1/κ₁)` — the largest admissible mesh step, from
    /// the support's tightest principal radius `1/κ₁`.
    pub kappa_cap: Rat<B>,
}

impl<B: Backend> ChartRecord<B> {
    /// Whether every certificate verified — the record is fully certified.
    pub fn is_certified(&self) -> bool {
        matches!(self.q_reg, Verdict::Verified(_))
            && matches!(self.ruling_reg, Verdict::Verified(_))
            && matches!(self.slab, Verdict::Verified(_))
    }
}
