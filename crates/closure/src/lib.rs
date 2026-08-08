#![forbid(unsafe_code)]
//! `closure` — the joint searcher for the `CLOSURE` treatment (spec §8.5/§8.6; M4).
//!
//! A **joint** is two developable flank strips meeting along a straight **crease** (a
//! Gauss-map jump across a shared ruling — an interface object between two charts, spec §5).
//! `closure` is the untrusted **searcher**: from the joint it builds the fields
//!
//! - `b_J = s_J·(n_A − n_B)` — the oriented bisector (`b_A = b_J`, `b_B = −b_J`),
//! - `G_i = (C_i − x₀)·b_i` — the retained-side field (kept side `G_i ≥ 0`; the raw `H_i` is
//!   diagnostic only, never in a predicate),
//! - `V` / `s_bev` — the fan/collar generator and bevel slope,
//!
//! and the flank cut-edges, then emits `(claim, certificate)` bundles. Every **certified**
//! predicate is decided by the pure-tier `certify_core` checkers (the extraction/TCB surface),
//! never here — the same searcher/checker split as `arrange2d` vs `certify_core::arrange`.
//!
//! The treatment obligation `CLOSURE_VALID(j)` splits its cap into a disjunction of two
//! constructions, `CLOSURE-CAP(j) := MITER-BRANCH ∨ LEDGE-BRANCH`: a **clean miter** pairs the
//! flanks' cut edges directly; a **forced ledge** builds a planar cap region by the §6 boolean
//! arrangement (`arrange2d::boolean::ledge_dom_certified`). `SEW` closes both and is M5.
//!
//! # Generality
//!
//! [`Joint::new`] takes two **arbitrary** [`geom::chart::Chart`]s — the flank *type* (cone,
//! cylinder, …) is carried by the chart's quaternion spline, never by a Rust branch. The
//! kernel is not a cone kernel. The vertical slice is built on the **cylinder** (the
//! representable developable whose ruling cut-edges are straight lines); a genuine plane is a
//! `planar` span (`n′ ≡ 0`) not yet representable as a [`Chart`], so it — and the petal
//! cone-flank — are deferred. See `docs/closure-scoping.md` and `docs/vv-guide.md §8`.
//!
//! # Status
//!
//! C0 skeleton: the joint **input** data model ([`Flank`], [`Joint`]). C1 adds the
//! [`cap_in`] searcher — projecting a flank chart into the cap plane and licensing the
//! result through `certify_core::cap_in`. The remaining per-branch certificates land per
//! phase (C2–C6, `docs/vv-guide.md §8`); no soundness decision is taken in this crate.

pub mod cap_in;

use geom::chart::Chart;
use lattice::{Backend, Bignum, Rat};

/// The retained ruling-parameter range `[μ⁻, μ⁺]` of a flank — the strip of the chart kept on
/// the joint's retained side (`G_i ≥ 0`). Endpoints are exact rationals.
#[derive(Clone, Debug)]
pub struct MuRange<B: Backend = Bignum> {
    /// The inboard ruling parameter `μ⁻`.
    pub lo: Rat<B>,
    /// The outboard ruling parameter `μ⁺`.
    pub hi: Rat<B>,
}

/// One side of a joint: a **strip** flank backed by a ruled [`Chart`] (`|n′| > 0` — a cone,
/// cylinder, …) together with the retained ruling range it contributes to the closure.
///
/// The flank *type* is carried entirely by the chart's data (its quaternion spline), never by
/// a Rust variant — a cone and a cylinder are the same `Flank`, differing only in `q`. A
/// genuine plane (a `planar` span, `n′ ≡ 0`) is **not** yet a [`Chart`], and so not yet a
/// `Flank`; that representation is deferred (`docs/closure-scoping.md §8`).
pub struct Flank<B: Backend = Bignum> {
    chart: Chart<B>,
    mu: MuRange<B>,
}

impl<B: Backend> Flank<B> {
    /// Build a flank from its strip chart and retained ruling range.
    pub fn new(chart: Chart<B>, mu: MuRange<B>) -> Self {
        Self { chart, mu }
    }
    /// The backing strip chart.
    pub fn chart(&self) -> &Chart<B> {
        &self.chart
    }
    /// The retained ruling range `[μ⁻, μ⁺]`.
    pub fn mu_range(&self) -> &MuRange<B> {
        &self.mu
    }
}

/// The joint orientation sign `s_J ∈ {+1, −1}`, which orients the bisector
/// `b_J = s_J·(n_A − n_B)` (`b_A = b_J`, `b_B = −b_J`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JointSign {
    /// `s_J = +1`.
    Plus,
    /// `s_J = −1`.
    Minus,
}

/// A straight **crease**: the shared ruling along which the two flanks' Gauss maps jump (spec
/// §5, *"a Gauss-map jump across a shared ruling"*). In the straight-crease scope this is a
/// single station per flank — the parameter `σ*` at which each chart meets the fold line.
#[derive(Clone, Debug)]
pub struct Crease<B: Backend = Bignum> {
    /// The meeting parameter `σ*` on flank A.
    pub sigma_a: Rat<B>,
    /// The meeting parameter `σ*` on flank B.
    pub sigma_b: Rat<B>,
}

/// A **joint**: two strip flanks meeting along a straight crease, with the orientation that
/// sets the bisector `b_J`. This is the *input* to the closure searcher — the derived fields
/// (`b_J`, the retained-side `G_i`, the fan generator `V`) and the per-branch certificates are
/// produced by the later phases and verified by the pure-tier `certify_core` checkers.
///
/// Nothing here keys on the flanks being cones: [`Joint::new`] takes two arbitrary [`Flank`]s.
///
/// # Example
///
/// ```
/// use closure::{Crease, Flank, Joint, JointSign, MuRange};
/// use geom::chart::Chart;
/// use lattice::{Bignum, Poly, Rat, RatFunc};
///
/// let poly = |cs: &[i128]| Poly::<Bignum>::from_coeffs(cs.iter().map(|&c| Rat::from_i128(c)).collect());
/// // Two cylinder flanks (q = 1 + σi): the line-carrier developable the M4 slice is built on.
/// let cyl = || Chart::new([poly(&[1]), poly(&[0, 1]), poly(&[0]), poly(&[0])], RatFunc::zero());
/// let mu = MuRange { lo: Rat::from_i128(-1), hi: Rat::new(-1, 2) };
/// let joint = Joint::new(
///     Flank::new(cyl(), mu.clone()),
///     Flank::new(cyl(), mu),
///     Crease { sigma_a: Rat::from_i128(0), sigma_b: Rat::from_i128(0) },
///     JointSign::Plus,
/// );
/// assert_eq!(joint.orientation(), JointSign::Plus);
/// assert_eq!(joint.crease().sigma_a, Rat::from_i128(0));
/// ```
pub struct Joint<B: Backend = Bignum> {
    flank_a: Flank<B>,
    flank_b: Flank<B>,
    crease: Crease<B>,
    orientation: JointSign,
}

impl<B: Backend> Joint<B> {
    /// Assemble a joint from two flanks, their shared crease, and the orientation sign `s_J`.
    pub fn new(
        flank_a: Flank<B>,
        flank_b: Flank<B>,
        crease: Crease<B>,
        orientation: JointSign,
    ) -> Self {
        Self {
            flank_a,
            flank_b,
            crease,
            orientation,
        }
    }
    /// The A-side flank (`b_A = b_J`).
    pub fn flank_a(&self) -> &Flank<B> {
        &self.flank_a
    }
    /// The B-side flank (`b_B = −b_J`).
    pub fn flank_b(&self) -> &Flank<B> {
        &self.flank_b
    }
    /// The shared straight crease.
    pub fn crease(&self) -> &Crease<B> {
        &self.crease
    }
    /// The orientation sign `s_J`.
    pub fn orientation(&self) -> JointSign {
        self.orientation
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice::{Poly, RatFunc};

    fn poly(cs: &[i128]) -> Poly<Bignum> {
        Poly::from_coeffs(cs.iter().map(|&c| Rat::from_i128(c)).collect())
    }
    /// A cylinder about the x-axis (`q = 1 + σi`) — the line-carrier developable flank.
    fn cylinder() -> Chart<Bignum> {
        Chart::new(
            [poly(&[1]), poly(&[0, 1]), poly(&[0]), poly(&[0])],
            RatFunc::zero(),
        )
    }
    /// A cone through the origin (`q = (9, 4, 4σ, 9σ)`) — a *different* developable class.
    fn cone() -> Chart<Bignum> {
        Chart::new(
            [poly(&[9]), poly(&[4]), poly(&[0, 4]), poly(&[0, 9])],
            RatFunc::zero(),
        )
    }

    fn mu() -> MuRange<Bignum> {
        MuRange {
            lo: Rat::from_i128(-1),
            hi: Rat::new(-1, 2),
        }
    }

    #[test]
    fn joint_accepts_two_arbitrary_charts() {
        // A mixed cone×cylinder joint: the searcher's input surface is generic in the flank
        // type — the two flanks are different developable classes and the API does not care.
        let joint = Joint::new(
            Flank::new(cone(), mu()),
            Flank::new(cylinder(), mu()),
            Crease {
                sigma_a: Rat::from_i128(0),
                sigma_b: Rat::from_i128(0),
            },
            JointSign::Minus,
        );
        assert_eq!(joint.orientation(), JointSign::Minus);
        // The flanks are stored verbatim; their charts read back through the accessors.
        assert_eq!(joint.flank_a().mu_range().lo, Rat::from_i128(-1));
        assert_eq!(
            joint
                .flank_b()
                .chart()
                .normal()
                .dot(joint.flank_b().chart().normal()),
            RatFunc::one(),
        );
    }
}
