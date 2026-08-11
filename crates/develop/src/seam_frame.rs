//! The **seam-frame reduction** (Stage-2 S2 / DEV.3-β): bring a developable's lap
//! seam to a *finite, regular* parameter so a subdivision certificate can converge
//! there, and certify the re-centered chart is an **exact reparametrization** of the
//! base.
//!
//! A single rational chart sweeps a bounded azimuth `< 2π` (`σ ∈ ℝ ↔ φ₃D ∈ (−π,π)`);
//! the lap seam is the one ruling it misses, at `σ = ±∞`, where every interval
//! enclosure width (`µ̂ ∝ 1+σ²`) is unbounded and non-refinable. Re-centering does not
//! add representational power — it moves the seam off that coordinate singularity to a
//! finite parameter `σ'`, so the BONDED certificate (`certify-core`, S3) runs over a
//! finite σ'-box. The re-centering is exact and rational (for a cone, the axis
//! half-turn `σ = −1/σ'`), so it is *certified*, not assumed.
//!
//! The reduction is deliberately **chart-agnostic**: [`seam_frame_exact`] takes any
//! base [`Chart`], any re-centered `view`, and any rational reparametrization
//! `σ = transition(σ')`, and discharges the obligation as an exact `RatFunc` identity.
//! The device-cone instance is `view = fixtures::devices::cone_seam()` with the axis
//! half-turn [`halfturn_transition`]; the γ≠0 seam ramp is a second instance (its
//! reduction is near-identity). Composition is done here over `RatFunc`'s public ops —
//! the exact `lattice`/TCB core is untouched.

use certify_core::Verdict;
use geom::chart::Chart;
use lattice::{Backend, Bignum, Poly, Rat, RatFunc};

/// A seam-conditioning view of a developable: the chart [`view`](SeamFrame::view),
/// reached from the base chart by the exact rational reparametrization
/// `σ = transition(σ')`, in which the lap seam sits at the finite, regular parameter
/// [`seam_param`](SeamFrame::seam_param) (a `σ'` value).
pub struct SeamFrame<B: Backend = Bignum> {
    /// The re-centered chart in which the seam is a finite, regular point.
    pub view: Chart<B>,
    /// The exact reparametrization `σ = transition(σ')` relating `view` back to the base.
    pub transition: RatFunc<B>,
    /// The seam's parameter in `view` (the `σ'` of the seam ruling).
    pub seam_param: Rat<B>,
}

/// Evidence that a [`SeamFrame`] is an exact reparametrization of its base with a
/// regular seam — carries the seam parameter for the downstream (S3 BONDED) certifier.
pub struct ValidSeamFrame<B: Backend = Bignum> {
    /// The certified seam parameter (`σ'` at the seam ruling).
    pub seam_param: Rat<B>,
}

/// Why a [`seam_frame_exact`] check refuted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeamFrameFault {
    /// The view's frame disagrees with `base ∘ transition` on a frame component:
    /// `0..3` = normal component `x/y/z`, `3..6` = pedal component — so `view` is
    /// **not** the base developable reparametrized by `transition`.
    NotAReparametrization(usize),
    /// The ruling stalls at the seam (`|n′|²(seam_param) = 0`, or its denominator
    /// vanishes there), so the seam is not a regular point of `view` and a
    /// subdivision cannot be conditioned on it.
    SeamNotRegular,
}

/// The axis **half-turn** reparametrization `σ = −1/σ'` (`φ₃D → φ₃D + π`) — the exact
/// rational Möbius that re-centers any azimuthal cone chart on its back ruling, sending
/// the seam `σ = ±∞` to `σ' = 0`.
///
/// ```
/// use develop::seam_frame::halfturn_transition;
/// use lattice::{Bignum, Rat};
///
/// // σ = −1/σ' evaluated at σ' = −1/2 gives σ = 2.
/// let phi = halfturn_transition::<Bignum>();
/// assert_eq!(phi.eval(&Rat::new(-1, 2)), Some(Rat::from_i128(2)));
/// ```
pub fn halfturn_transition<B: Backend>() -> RatFunc<B> {
    // −1 / σ'
    RatFunc::new(
        Poly::from_coeffs(vec![Rat::from_i128(-1)]),
        Poly::from_coeffs(vec![Rat::from_i128(0), Rat::from_i128(1)]),
    )
}

/// Certify that `view` is the `base` developable reparametrized by `σ = transition(σ')`,
/// exactly, with the seam a regular point of `view`.
///
/// The obligation is an **exact rational identity**, float-free: `view.normal ≡
/// base.normal ∘ transition` and `view.pedal ≡ base.pedal ∘ transition`, component by
/// component (over ℚ). The ruling direction then follows — `r_view = transition′ ·
/// (r_base ∘ transition)`, the same ruling *line* rescaled by the reparametrization
/// speed — so a matching normal-and-pedal frame proves the same developable. Finally
/// the seam is checked regular (`|n′|²(seam_param) > 0`).
///
/// `Verified(ValidSeamFrame)` when both identities hold and the seam is regular;
/// `Refuted(NotAReparametrization | SeamNotRegular)` otherwise. There is no
/// `Unresolved`: exactness of a reparametrization is a decidable rational identity.
pub fn seam_frame_exact<B: Backend>(
    base: &Chart<B>,
    frame: &SeamFrame<B>,
) -> Verdict<ValidSeamFrame<B>, SeamFrameFault, ()> {
    let phi = &frame.transition;
    for j in 0..3 {
        if frame.view.normal().comp(j).reduce() != compose_ratfunc(&base.normal().comp(j), phi) {
            return Verdict::Refuted(SeamFrameFault::NotAReparametrization(j));
        }
        if frame.view.pedal().comp(j).reduce() != compose_ratfunc(&base.pedal().comp(j), phi) {
            return Verdict::Refuted(SeamFrameFault::NotAReparametrization(3 + j));
        }
    }
    match frame.view.normal_deriv_sq().eval(&frame.seam_param) {
        Some(v) if v > Rat::from_i128(0) => Verdict::Verified(ValidSeamFrame {
            seam_param: frame.seam_param.clone(),
        }),
        _ => Verdict::Refuted(SeamFrameFault::SeamNotRegular),
    }
}

/// `f ∘ φ` for a rational `f` and rational reparametrization `φ`, reduced — composing
/// numerator and denominator polynomials via Horner over `RatFunc`.
fn compose_ratfunc<B: Backend>(f: &RatFunc<B>, phi: &RatFunc<B>) -> RatFunc<B> {
    compose_poly(f.num(), phi)
        .div(&compose_poly(f.den(), phi))
        .reduce()
}

/// `p ∘ φ` for a polynomial `p` and rational `φ` — Horner: `Σ pᵢ φⁱ`.
fn compose_poly<B: Backend>(p: &Poly<B>, phi: &RatFunc<B>) -> RatFunc<B> {
    let mut acc = RatFunc::zero();
    for c in p.coeffs().iter().rev() {
        acc = acc
            .mul(phi)
            .add(&RatFunc::from_poly(Poly::from_coeffs(vec![c.clone()])));
    }
    acc
}

#[cfg(test)]
mod tests {
    use super::*;
    use fixtures::devices::{cone, cone_alt, cone_seam};

    fn cone_frame() -> SeamFrame<Bignum> {
        SeamFrame {
            view: cone_seam(),
            transition: halfturn_transition(),
            seam_param: Rat::from_i128(0),
        }
    }

    #[test]
    fn cone_seam_frame_is_an_exact_reparametrization() {
        // cone_seam() is the device cone reparametrized by the axis half-turn σ = −1/σ',
        // with the seam at the regular finite point σ' = 0 — certified exactly.
        match seam_frame_exact(&cone(), &cone_frame()) {
            Verdict::Verified(ev) => assert_eq!(ev.seam_param, Rat::from_i128(0)),
            _ => panic!("expected a Verified seam frame"),
        }
    }

    #[test]
    fn the_identity_transition_is_refuted() {
        // σ = σ' (identity) is NOT the reparametrization relating the two charts:
        // cone_seam().normal ≢ cone().normal, so the frame identity fails.
        let frame = SeamFrame {
            view: cone_seam(),
            transition: RatFunc::from_poly(Poly::from_coeffs(vec![
                Rat::from_i128(0),
                Rat::from_i128(1),
            ])), // φ(σ') = σ'
            seam_param: Rat::from_i128(0),
        };
        assert!(matches!(
            seam_frame_exact(&cone(), &frame),
            Verdict::Refuted(SeamFrameFault::NotAReparametrization(_))
        ));
    }

    #[test]
    fn a_different_cone_is_refuted() {
        // The half-turn view of the device cone is not a reparametrization of a
        // *different* cone (cone_alt, n·ẑ ≡ 3/5) — the normal fields disagree.
        assert!(matches!(
            seam_frame_exact(&cone_alt(), &cone_frame()),
            Verdict::Refuted(SeamFrameFault::NotAReparametrization(_))
        ));
    }
}
