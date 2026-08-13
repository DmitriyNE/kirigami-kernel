//! The ANCHOR development certificate — the **transcendental half** (`T,1D`) of ANCHOR
//! (`docs/paper.md:141`, spec §8.5:372).
//!
//! A content curve authored flat as `g(t)` gets a rational **anchor spline** `â(t)` in
//! chart coordinates; the certified obligation is the **uniform lift bound**
//! `sup_t |D(â(t)) − g(t)| ≤ ε` together with the **DRC** `ε < clearance/2` (the crossing
//! margin, spec:192). This is the shell-tier partner of the pure exact half
//! [`certify_core::free_boundary`] (`A,1D` — width / rail-regularity / σ̂-monotonicity):
//! [`anchor`] runs both and composes them into the full ANCHOR (`T,1D + A,1D`).
//!
//! The anchor is a **general rational-`t` curve** `â(t) = (σ(t), μ̂(t))` with `σ(t)` a
//! monotone rational reparametrization — *not* the σ-graph. It needs no polynomial
//! composition primitive: the checker never symbolically forms `ρ∘σ(t)`; it evaluates
//! `σ(t), μ̂(t), g(t)` over `t`-sub-intervals and feeds the resulting σ-interval into the
//! [interval-lifted development](crate::cone::ConeDevelopment::point_on). Because a
//! free-boundary band's μ-rail is affine in σ, `μ̂(t) = μ⁻(σ(t)) = σ(t)·β + α` is itself a
//! rational function of `t` (a `scale`+`add`), so the anchor can *ride the rail* the A-part
//! certifies. No float enters the certificate; the endpoints and `ε` are rationals.

use crate::cone::{ConeDevelopment, DevConfig};
use crate::interval::{RatIv, eval_ratfunc_on, sqrt};
use certify_core::Verdict;
use certify_core::free_boundary::{
    FreeBoundaryCert, FreeBoundaryFault, ValidFreeBoundary, free_boundary,
};
use lattice::{Backend, Bignum, Interval, Rat, RatFunc};

/// An ANCHOR development certificate (T-part): a rational anchor curve `â(t) = (σ(t), μ̂(t))`
/// on a cone chart, an authored flat target `g(t)`, and the fab clearance the lift bound is
/// checked against.
#[derive(Clone)]
pub struct AnchorDevCert<B: Backend = Bignum> {
    /// The closed-form cone development the anchor rides (its existence already witnesses
    /// closed-form-ness — [`ConeDevelopment::new`] rejects non-cones — so a `NotClosedForm`
    /// fault cannot arise here; the deferral lives in [`crate::cone::angle_enclosure`]).
    pub dev: ConeDevelopment<B>,
    /// The monotone rational reparametrization `σ(t)` — the anchor's chart σ-coordinate.
    pub sigma: RatFunc<B>,
    /// The anchor's ruling coordinate `μ̂(t)` (e.g. a free-boundary μ-rail composed with `σ(t)`).
    pub mu: RatFunc<B>,
    /// The authored flat target `g(t) = (gx(t), gy(t))`, a rational flat curve.
    pub target: [RatFunc<B>; 2],
    /// The `t`-span `[t_lo, t_hi]` the anchor and target are authored over.
    pub span: Interval<B>,
    /// The number of equal `t`-sub-intervals the rigorous `sup_t` is taken over — the
    /// refinement handle (more sub-intervals ⇒ a tighter `ε`).
    pub subdiv: usize,
    /// The item's exact flat clearance; the DRC gate is `ε < clearance/2`.
    pub clearance: Rat<B>,
    /// The transcendental-enclosure budget (`terms`, `sqrt_eps`).
    pub cfg: DevConfig<B>,
    /// The piecewise-support frame this anchor rides in, or `None` for the plain single-region
    /// development (the original [`ConeDevelopment::point_on`] path, byte-identical).
    pub frame: Option<AnchorFrame<B>>,
}

/// The piecewise-support frame an anchored edge rides in: the running flat-frame `base`
/// accumulated over the regions before this one, and the σ lower limit `lo` its region's γ
/// integrates from (the region's own window, where the support is tame — integrating a region's
/// support from `0` crosses its blow-up zone, which is exactly what this field avoids). With a
/// frame the checker develops via [`ConeDevelopment::point_from_on`] — the **signed**-µ̂ canonical
/// development, the piecewise gluing's requirement.
#[derive(Clone)]
pub struct AnchorFrame<B: Backend = Bignum> {
    /// The running flat-frame offset carried from the previous region's end.
    pub base: [RatIv<B>; 2],
    /// The σ lower limit the region's directrix integrates from (`lo ≤ σ` on the span).
    pub lo: Rat<B>,
}

/// The evidence a valid ANCHOR T-part carries: the certified `t`-span and the uniform lift
/// bound `ε` (a **bare `Rat`**, a linear-scale distance — not a squared [`MarginSq`]),
/// under the recorded clearance.
#[derive(Clone)]
pub struct ValidAnchorDev<B: Backend = Bignum> {
    /// The `t`-span over which the bound holds.
    pub span: Interval<B>,
    /// The certified uniform bound `sup_t |D(â(t)) − g(t)| ≤ ε`.
    pub eps: Rat<B>,
    /// The clearance the DRC compared against (`ε < clearance/2`).
    pub clearance: Rat<B>,
}

/// Why the ANCHOR T-part checker refused a certificate (looseness is *not* here — a loose
/// enclosure is [`Unresolved`](Verdict::Unresolved), refined by `subdiv`, never `Refuted`).
#[derive(Clone, Debug)]
pub enum AnchorDevFault {
    /// The `t`-span is empty or degenerate (`t_lo ≥ t_hi`).
    DegenerateSpan,
    /// A rational field (`σ`, `μ̂`, `g`, or `ρ²`) had a denominator enclosure straddling zero
    /// on a sub-interval — a possible pole, so the quotient is unbounded there. Refine or
    /// re-author away from the pole.
    PoleInEval,
}

/// The largest `|x − y|` over `x ∈ d`, `y ∈ g` — `max(d.hi − g.lo, g.hi − d.lo)` (the two
/// signed extremes of `d − g`), always the true axis-wise separation and `≥ 0`.
fn axis_sup<B: Backend>(d: &RatIv<B>, g: &RatIv<B>) -> Rat<B> {
    let e1 = d.hi().sub(g.lo());
    let e2 = g.hi().sub(d.lo());
    if e1.cmp(&e2) == core::cmp::Ordering::Greater {
        e1
    } else {
        e2
    }
}

/// ANCHOR, transcendental half (`T,1D`): certify the uniform lift bound
/// `sup_t |D(â(t)) − g(t)| ≤ ε` and gate it by the DRC `ε < clearance/2`.
///
/// The checker **computes** the bound itself (its interval arithmetic is the trusted part;
/// it does not trust a searcher-supplied `ε`): it subdivides `[t_lo, t_hi]` into `subdiv`
/// equal sub-intervals and, on each, encloses the developed anchor `D(â([a,b]))` (via
/// [`ConeDevelopment::point_on`] over the σ- and μ̂-intervals) and the target `g([a,b])`,
/// bounds their separation `√(Δx² + Δy²)`, and takes the maximum `ε`. Refining `subdiv`
/// shrinks `ε`. Total: `Verified(`[`ValidAnchorDev`]`)` when `ε < clearance/2`,
/// `Unresolved(ε)` when not (refine), or `Refuted(`[`AnchorDevFault`]`)` for a degenerate
/// span / pole.
pub fn anchor_dev<B: Backend>(
    cert: &AnchorDevCert<B>,
) -> Verdict<ValidAnchorDev<B>, AnchorDevFault, Rat<B>> {
    use core::cmp::Ordering;
    let (t_lo, t_hi) = (&cert.span.lo, &cert.span.hi);
    if t_lo.cmp(t_hi) != Ordering::Less {
        return Verdict::Refuted(AnchorDevFault::DegenerateSpan);
    }
    let n = cert.subdiv.max(1);
    let width = t_hi.sub(t_lo).div(&Rat::from_i128(n as i128));
    let mut eps = Rat::from_i128(0);
    for k in 0..n {
        let a = t_lo.add(&width.mul(&Rat::from_i128(k as i128)));
        let b = a.add(&width);
        let t_iv = RatIv::new(a, b);
        // The anchor's chart coordinates over this t-sub-interval.
        let sig = match eval_ratfunc_on(&cert.sigma, &t_iv) {
            Some(s) => s,
            None => return Verdict::Refuted(AnchorDevFault::PoleInEval),
        };
        let mu = match eval_ratfunc_on(&cert.mu, &t_iv) {
            Some(m) => m,
            None => return Verdict::Refuted(AnchorDevFault::PoleInEval),
        };
        // The developed anchor box D(â([a,b])) and the authored target box g([a,b]) — through
        // the piecewise frame when one is given (base + from-`lo` γ, signed µ̂), else the plain
        // single-region development (byte-identical to the pre-frame checker).
        let d = match &cert.frame {
            None => cert.dev.point_on(&sig, &mu, &cert.cfg),
            Some(f) => cert.dev.point_from_on(&f.base, &f.lo, &sig, &mu, &cert.cfg),
        };
        let d = match d {
            Some(d) => d,
            None => return Verdict::Refuted(AnchorDevFault::PoleInEval),
        };
        let gx = match eval_ratfunc_on(&cert.target[0], &t_iv) {
            Some(g) => g,
            None => return Verdict::Refuted(AnchorDevFault::PoleInEval),
        };
        let gy = match eval_ratfunc_on(&cert.target[1], &t_iv) {
            Some(g) => g,
            None => return Verdict::Refuted(AnchorDevFault::PoleInEval),
        };
        // sup |D − g| over the box ≤ √(supΔx² + supΔy²) — a rigorous upper bound.
        let dx = axis_sup(&d.x, &gx);
        let dy = axis_sup(&d.y, &gy);
        let dist_sq = dx.mul(&dx).add(&dy.mul(&dy));
        let dist = sqrt(&dist_sq, &cert.cfg.sqrt_eps).hi().clone();
        if dist.cmp(&eps) == Ordering::Greater {
            eps = dist;
        }
    }
    // DRC: fabricable when the lift bound is under half the clearance.
    let half = cert.clearance.mul(&Rat::new(1, 2));
    if eps.cmp(&half) == Ordering::Less {
        Verdict::Verified(ValidAnchorDev {
            span: cert.span.clone(),
            eps,
            clearance: cert.clearance.clone(),
        })
    } else {
        Verdict::Unresolved(eps)
    }
}

/// Why the composed full ANCHOR refused (`T,1D + A,1D`).
pub enum AnchorFault<B: Backend = Bignum> {
    /// The exact half ([`free_boundary`]) refused — carries its fault.
    Exact(FreeBoundaryFault<B>),
    /// The transcendental half ([`anchor_dev`]) refused — carries its fault.
    Transcendental(AnchorDevFault),
    /// The anchor's σ-range `[σ(t_lo), σ(t_hi)]` does not equal the band's σ-span, so the
    /// T-part and A-part are not certifying the *same* footprint (audit, not trust).
    SpanMismatch,
}

/// The evidence a full ANCHOR carries: the exact footprint (`A,1D`) paired with the
/// transcendental lift bound (`T,1D`).
pub type AnchorEvidence<B> = (ValidFreeBoundary<B>, ValidAnchorDev<B>);

/// The full **ANCHOR** (spec §8.5:372, `T,1D + A,1D`): compose the exact free-boundary
/// footprint ([`free_boundary`], `A,1D`) with the transcendental lift bound ([`anchor_dev`],
/// `T,1D`) on one shared footprint.
///
/// Verifies both halves and audits that the anchor rides the certified band — the anchor's
/// σ-range `[σ(t_lo), σ(t_hi)]` must equal the band's σ-span. `Verified((A, T))` only when
/// both pass and the spans agree; `Unresolved(ε)` when the exact half passes but the lift
/// bound is not yet under the clearance (refine `subdiv`); `Refuted` otherwise.
pub fn anchor<B: Backend>(
    fb: &FreeBoundaryCert<B>,
    ad: &AnchorDevCert<B>,
) -> Verdict<AnchorEvidence<B>, AnchorFault<B>, Rat<B>> {
    use core::cmp::Ordering;
    // A,1D — the exact footprint.
    let valid_a = match free_boundary(fb) {
        Verdict::Verified(v) => v,
        Verdict::Refuted(f) => return Verdict::Refuted(AnchorFault::Exact(f)),
        // free_boundary is total (never Unresolved); route the impossible arm to an
        // inconclusive with a trivial handle rather than panic (keeps the checker total).
        Verdict::Unresolved(()) => return Verdict::Unresolved(Rat::from_i128(0)),
    };
    // Audit that the anchor rides the certified band: its σ-range = the band's σ-span.
    let (s0, s1) = match (ad.sigma.eval(&ad.span.lo), ad.sigma.eval(&ad.span.hi)) {
        (Some(s0), Some(s1)) => (s0, s1),
        _ => return Verdict::Refuted(AnchorFault::SpanMismatch),
    };
    let (smin, smax) = if s0.cmp(&s1) == Ordering::Greater {
        (s1, s0)
    } else {
        (s0, s1)
    };
    if smin.cmp(&fb.span.lo) != Ordering::Equal || smax.cmp(&fb.span.hi) != Ordering::Equal {
        return Verdict::Refuted(AnchorFault::SpanMismatch);
    }
    // T,1D — the transcendental lift bound.
    match anchor_dev(ad) {
        Verdict::Verified(valid_t) => Verdict::Verified((valid_a, valid_t)),
        Verdict::Refuted(f) => Verdict::Refuted(AnchorFault::Transcendental(f)),
        Verdict::Unresolved(eps) => Verdict::Unresolved(eps),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cone::FlatBox;
    use certify_core::MarginSq;
    use certify_core::certify1d::{EdgeRegCert, RegCert};
    use fixtures::devices::cone;
    use lattice::{Poly, SturmChain};

    type Q = Rat<Bignum>;

    fn poly(cs: &[i128]) -> Poly<Bignum> {
        Poly::from_coeffs(cs.iter().map(|&c| Q::from_i128(c)).collect())
    }
    fn ratf(num: &[i128], den: &[i128]) -> RatFunc<Bignum> {
        RatFunc::new(poly(num), poly(den))
    }
    fn ivl(lo: Q, hi: Q) -> Interval<Bignum> {
        Interval { lo, hi }
    }
    fn to_f64(r: &Q) -> f64 {
        let (n, d) = r.numer_denom_decimal();
        n.parse::<f64>().unwrap() / d.parse::<f64>().unwrap()
    }
    /// A REG-Q cert with honest Sturm chains for `num/den ≥ m` on `iv` (mirrors free_boundary).
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
    fn edge(num: &[i128], den: &[i128], m: Q, iv: Interval<Bignum>) -> EdgeRegCert<Bignum> {
        EdgeRegCert {
            speed_sq: reg(num, den, m, iv),
            failure: None,
        }
    }

    /// The device cone's tapered free boundary over σ ∈ [0, 1]: rails μ⁻ = −1, μ⁺ = −σ/2 (affine),
    /// width μ⁺ − μ⁻ = 1 − σ/2 = (2 − σ)/2 ∈ [1/2, 1] ≥ 1/4 (strict) on [0,1]. Rail speeds and
    /// σ̂′ ≡ 1 as usual (placeholder-valid, the free_boundary-test mold — the checker re-verifies
    /// whatever RegCert data it is given via honest Sturm chains).
    fn cone_band() -> FreeBoundaryCert<Bignum> {
        let iv = ivl(Q::from_i128(0), Q::from_i128(1));
        FreeBoundaryCert {
            span: iv.clone(),
            width: reg(&[2, -1], &[2], Q::new(1, 4), iv.clone()), // (2 − σ)/2 ≥ 1/4 on [0,1]
            reg_lo: edge(&[1, 0, 1], &[1], Q::new(1, 2), iv.clone()),
            reg_hi: edge(&[1, 0, 1], &[1], Q::new(1, 2), iv.clone()),
            monotone: reg(&[1], &[1], Q::new(1, 2), iv.clone()),
        }
    }

    /// The anchor rides the μ⁻ = −1 rail under a non-identity monotone reparam σ(t) = t/2 on
    /// t ∈ [0, 2] (so σ ∈ [0, 1] — genuinely t ≠ σ), μ̂(t) ≡ −1. Target g(t) = the straight
    /// chord (linear in t) between the developed rail endpoints D(â(0)) and D(â(2)).
    fn cone_anchor(
        target: [RatFunc<Bignum>; 2],
        subdiv: usize,
        clearance: Q,
    ) -> AnchorDevCert<Bignum> {
        AnchorDevCert {
            dev: ConeDevelopment::new(&cone()).unwrap(),
            sigma: ratf(&[0, 1], &[2]), // σ(t) = t/2
            mu: ratf(&[-1], &[1]),      // μ̂(t) = −1
            target,
            span: ivl(Q::from_i128(0), Q::from_i128(2)),
            subdiv,
            clearance,
            cfg: DevConfig::tight(),
            frame: None,
        }
    }

    /// The developed rail endpoints, as f64, to build the authored chord target and to
    /// corroborate ε. `â(t) = (t/2, −1)`, so σ runs 0 → 1 as t runs 0 → 2.
    fn dev_point_f64(sigma: f64) -> (f64, f64) {
        let dev = ConeDevelopment::new(&cone()).unwrap();
        let s = Q::new((sigma * 1_000_000.0) as i128, 1_000_000);
        let fb = dev.point(&s, &Q::from_i128(-1), &DevConfig::tight());
        (to_f64(&fb.x.mid()), to_f64(&fb.y.mid()))
    }

    /// A straight-chord target between the exact developed endpoints, as rational linear
    /// functions of t on [0, 2]: g(t) = P0 + (t/2)·(P1 − P0). We build it from the *exact*
    /// developed endpoints (rational), so the chord is authored, not floated.
    fn chord_target() -> [RatFunc<Bignum>; 2] {
        let dev = ConeDevelopment::new(&cone()).unwrap();
        let cfg = DevConfig::tight();
        let p0 = dev.point(&Q::from_i128(0), &Q::from_i128(-1), &cfg);
        let p1 = dev.point(&Q::from_i128(1), &Q::from_i128(-1), &cfg);
        let (x0, y0) = (p0.x.mid(), p0.y.mid());
        let (x1, y1) = (p1.x.mid(), p1.y.mid());
        // g(t) = x0 + (t/2)(x1 − x0) = x0 + t·(x1 − x0)/2. Numerator poly [x0, (x1−x0)/2].
        let lin = |c0: Q, c1: Q| {
            let slope = c1.sub(&c0).mul(&Q::new(1, 2));
            RatFunc::new(
                Poly::from_coeffs(vec![c0, slope]),
                Poly::from_coeffs(vec![Q::from_i128(1)]),
            )
        };
        [lin(x0, x1), lin(y0, y1)]
    }

    /// The ε the checker computes at a given subdivision, read regardless of the DRC verdict
    /// (run with an enormous clearance so it always `Verified`s and exposes ε).
    fn measured_eps(subdiv: usize) -> Q {
        match anchor_dev(&cone_anchor(
            chord_target(),
            subdiv,
            Q::from_i128(1_000_000),
        )) {
            Verdict::Verified(v) => v.eps,
            _ => panic!("an enormous clearance must certify"),
        }
    }

    #[test]
    fn epsilon_shrinks_with_subdivision() {
        // The box-based sup tightens as the sub-intervals shrink: ε(coarse) > ε(fine).
        let coarse = measured_eps(4);
        let fine = measured_eps(64);
        assert!(
            fine.cmp(&coarse) == core::cmp::Ordering::Less,
            "ε must shrink with subdiv: coarse {} vs fine {}",
            to_f64(&coarse),
            to_f64(&fine)
        );
    }

    #[test]
    fn a_generous_clearance_certifies() {
        // Clearance comfortably above the computed lift bound (self-calibrated: 4·ε ⇒ half = 2ε > ε).
        let e = measured_eps(32);
        let clearance = e.mul(&Q::from_i128(4));
        assert!(matches!(
            anchor_dev(&cone_anchor(chord_target(), 32, clearance)),
            Verdict::Verified(_)
        ));
    }

    #[test]
    fn a_tight_clearance_is_unresolved() {
        // A clearance far below the true lift bound ⇒ ε ≥ clearance/2 ⇒ Unresolved (refine).
        let e = measured_eps(8);
        let clearance = e.div(&Q::from_i128(100));
        assert!(matches!(
            anchor_dev(&cone_anchor(chord_target(), 8, clearance)),
            Verdict::Unresolved(_)
        ));
    }

    #[test]
    fn epsilon_upper_bounds_the_float_sagitta() {
        // Corroboration: the certified ε upper-bounds a fine float sampling of the true
        // sup_t |D(â(t)) − g(t)| (the develop_cone-style oracle checks the rational cert).
        let eps = to_f64(&measured_eps(64));
        // Float chord and developed rail, sampled densely; the max deviation must sit under ε.
        let (x0, y0) = dev_point_f64(0.0);
        let (x1, y1) = dev_point_f64(1.0);
        let mut max_dev = 0.0f64;
        for i in 0..=400 {
            let s = i as f64 / 400.0; // σ ∈ [0,1]
            let (dx, dy) = dev_point_f64(s);
            let (gx, gy) = (x0 + s * (x1 - x0), y0 + s * (y1 - y0));
            let dist = ((dx - gx).powi(2) + (dy - gy).powi(2)).sqrt();
            if dist > max_dev {
                max_dev = dist;
            }
        }
        assert!(
            max_dev > 0.0,
            "the developed rail genuinely bows off the chord"
        );
        assert!(
            eps >= max_dev,
            "certified ε {eps:e} must upper-bound the float sagitta {max_dev:e}"
        );
    }

    #[test]
    fn the_full_anchor_composes_exact_and_transcendental() {
        // ANCHOR = T,1D + A,1D on one shared footprint: the band certifies (A) and the anchor
        // riding its μ⁻ rail certifies the lift bound (T) under a generous clearance.
        let fb = cone_band();
        let clearance = measured_eps(32).mul(&Q::from_i128(4));
        let ad = cone_anchor(chord_target(), 32, clearance);
        match anchor(&fb, &ad) {
            Verdict::Verified((_a, t)) => {
                assert_eq!(t.span.lo, Q::from_i128(0));
                assert_eq!(t.span.hi, Q::from_i128(2));
            }
            _ => panic!("the full ANCHOR must certify on the shared cone footprint"),
        }
    }

    #[test]
    fn a_span_mismatched_anchor_is_refused() {
        // The anchor's σ-range [σ(0), σ(2)] = [0, 1] matches the band; shift the reparam to
        // σ(t) = t/2 + 1 so the σ-range becomes [1, 2] ≠ the band's [0, 1] ⇒ SpanMismatch.
        let fb = cone_band();
        let mut ad = cone_anchor(chord_target(), 8, Q::new(1, 10));
        ad.sigma = ratf(&[1, 1], &[2]); // (1 + t)/2 → σ(0)=1/2, σ(2)=3/2 ⇒ range [1/2,3/2] ≠ [0,1]
        assert!(matches!(
            anchor(&fb, &ad),
            Verdict::Refuted(AnchorFault::SpanMismatch)
        ));
    }

    #[test]
    fn a_degenerate_span_is_refuted() {
        let mut ad = cone_anchor(chord_target(), 8, Q::new(1, 10));
        ad.span = ivl(Q::from_i128(1), Q::from_i128(1)); // t_lo == t_hi
        assert!(matches!(
            anchor_dev(&ad),
            Verdict::Refuted(AnchorDevFault::DegenerateSpan)
        ));
    }

    #[test]
    fn point_on_encloses_the_pointwise_development() {
        // The interval-lifted D(σ-iv, μ̂-iv) contains the pointwise D(σ, μ̂) at interior points.
        let dev = ConeDevelopment::new(&cone()).unwrap();
        let cfg = DevConfig::tight();
        let sig = RatIv::new(Q::new(1, 4), Q::new(3, 4));
        let mu = RatIv::new(Q::from_i128(-1), Q::from_i128(-1));
        let box_iv: FlatBox<Bignum> = dev.point_on(&sig, &mu, &cfg).unwrap();
        let pt = dev.point(&Q::new(1, 2), &Q::from_i128(-1), &cfg);
        assert!(box_iv.x.contains(&pt.x.mid()) && box_iv.y.contains(&pt.y.mid()));
    }

    /// A γ≠0 anchor cert on the ramp flap (σ ∈ [0, 1/2], µ̂ ≡ 1): σ(t) = t identity, a zero
    /// target, and a permissive clearance, so the returned ε is the raw sup |D|.
    fn ramp_anchor(frame: Option<AnchorFrame<Bignum>>) -> AnchorDevCert<Bignum> {
        use fixtures::devices::cone_seam_ramp;
        AnchorDevCert {
            dev: ConeDevelopment::new_developable(&cone_seam_ramp(), 64).unwrap(),
            sigma: ratf(&[0, 1], &[1]), // σ(t) = t
            mu: ratf(&[1], &[1]),       // μ̂(t) = 1
            target: [ratf(&[0], &[1]), ratf(&[0], &[1])],
            span: ivl(Q::from_i128(0), Q::new(1, 2)),
            subdiv: 8,
            clearance: Q::from_i128(1_000_000),
            cfg: DevConfig::tight(),
            frame,
        }
    }

    #[test]
    fn a_zero_frame_matches_the_frameless_gamma_anchor() {
        // frame = Some{base 0, lo 0} routes through `point_from_on`, which must reproduce the
        // frameless γ≠0 path (`point_on`'s signed branch: γ from 0 + the velocity-tail hull)
        // exactly — same integrals, same rounding, same ε.
        let z = RatIv::point(Q::from_i128(0));
        let framed = ramp_anchor(Some(AnchorFrame {
            base: [z.clone(), z],
            lo: Q::from_i128(0),
        }));
        let frameless = ramp_anchor(None);
        let (a, b) = match (anchor_dev(&framed), anchor_dev(&frameless)) {
            (Verdict::Verified(a), Verdict::Verified(b)) => (a.eps, b.eps),
            _ => panic!("both γ≠0 anchors certify under the permissive clearance"),
        };
        assert_eq!(a, b);
    }

    #[test]
    fn the_anchor_frame_is_translation_equivariant() {
        // The piecewise gluing's soundness core: shifting the frame base by an exact integer
        // offset and the authored target by the same offset leaves ε unchanged (the developed
        // box and the target box shift together; integer offsets live on the dyadic rounding
        // grid, so even the outward rounding commutes).
        let z = RatIv::point(Q::from_i128(0));
        let zero_frame = ramp_anchor(Some(AnchorFrame {
            base: [z.clone(), z],
            lo: Q::from_i128(0),
        }));
        let mut shifted = ramp_anchor(Some(AnchorFrame {
            base: [
                RatIv::point(Q::from_i128(2)),
                RatIv::point(Q::from_i128(-3)),
            ],
            lo: Q::from_i128(0),
        }));
        shifted.target = [ratf(&[2], &[1]), ratf(&[-3], &[1])];
        let (a, b) = match (anchor_dev(&zero_frame), anchor_dev(&shifted)) {
            (Verdict::Verified(a), Verdict::Verified(b)) => (a.eps, b.eps),
            _ => panic!("both framed anchors certify under the permissive clearance"),
        };
        assert_eq!(a, b);
    }
}
