//! Certified **unroll** (product direction ①): develop a free-boundary μ-band on a cone
//! chart into its flat pattern outline (`docs/implementation-plan-v1.md §6`).
//!
//! The band is the σ-band bounded by two authored rational μ-rails `μ⁻(σ), μ⁺(σ)` over
//! `σ ∈ [σ_lo, σ_hi]` (at `w = 0`) — the same free boundary [`certify_core::free_boundary`]
//! certifies as a valid footprint. [`unroll_freeboundary`] develops its boundary loop into a
//! flat **polyline** and certifies it is a faithful flat pattern: each polyline vertex is a
//! rigorous [`FlatBox`] enclosure of the developed boundary point, and each **rail edge** is
//! certified — via the DEV.2c [`anchor_dev`](crate::anchor::anchor_dev) lift bound — to lie
//! within `ε` of the *true continuous* developed rail (not merely at the vertices). The whole
//! outline carries the uniform `ε = max` over edges, gated by the DRC `ε < clearance/2`.
//!
//! The two σ-**caps** (fixed σ, μ̂ sweeping `μ⁻ → μ⁺`) are rulings, which develop to *exact*
//! straight radial segments `μ̂·ρ(σ)·e(ψ(σ))`, so they need no fidelity bound — only the two
//! curved rails do. No float enters the certificate; the diagnostic `develop_cone`
//! corroborates (see the `export` corroboration test).

use crate::anchor::{AnchorDevCert, AnchorDevFault, anchor_dev};
use crate::cone::{ConeDevelopment, DevConfig, FlatBox};
use certify_core::Verdict;
use lattice::{Backend, Bignum, Interval, Poly, Rat, RatFunc};

/// A certified flat pattern outline: the developed boundary loop as an ordered polyline of
/// [`FlatBox`] vertices, with the uniform curve-fidelity bound `ε` (the largest rail-edge
/// [`anchor_dev`](crate::anchor::anchor_dev) bound) under the recorded clearance.
#[derive(Clone)]
pub struct FlatOutline<B: Backend = Bignum> {
    /// The developed boundary vertices, ordered `μ⁻` rail (`σ_lo→σ_hi`) then `μ⁺` rail
    /// (`σ_hi→σ_lo`) — a closed loop (the two σ-caps are the exact straight edges joining them).
    pub vertices: Vec<FlatBox<B>>,
    /// The uniform flat-pattern backward error: `max` over rail edges of the certified
    /// `sup |D(rail) − chord| ≤ ε`. The polyline is within `ε` of the true development everywhere.
    pub eps: Rat<B>,
    /// The clearance the DRC compared against (`ε < clearance/2`).
    pub clearance: Rat<B>,
}

/// Why the unroll refused to certify a flat outline.
#[derive(Clone, Debug)]
pub enum UnrollFault {
    /// The σ-span is empty or degenerate (`σ_lo ≥ σ_hi`).
    DegenerateSpan,
    /// A rail `μ±(σ)` or the development had a pole on a sub-interval (propagated from the
    /// per-edge [`anchor_dev`](crate::anchor::anchor_dev)).
    PoleInEval,
}

/// The linear rational function `t ↦ v0 + (t − t0)·(v1 − v0)/(t1 − t0)` through `(t0,v0)` and
/// `(t1,v1)` — the flat chord coordinate over one rail edge, as a `RatFunc` in the curve
/// parameter. `t1 ≠ t0` by construction (a non-degenerate edge).
fn linear_through<B: Backend>(t0: &Rat<B>, v0: &Rat<B>, t1: &Rat<B>, v1: &Rat<B>) -> RatFunc<B> {
    let slope = v1.sub(v0).div(&t1.sub(t0));
    let intercept = v0.sub(&t0.mul(&slope));
    RatFunc::new(
        Poly::from_coeffs(vec![intercept, slope]),
        Poly::from_coeffs(vec![Rat::from_i128(1)]),
    )
}

/// The certified lift bound of one rail edge `[σ_a, σ_b]`: the true developed rail vs the
/// straight chord between its developed endpoints, via [`anchor_dev`](crate::anchor::anchor_dev)
/// with the parameter `t = σ` (identity reparam) and a permissive clearance (so the raw `ε` is
/// read back). Returns the edge's `ε`, or the propagated fault.
#[allow(clippy::too_many_arguments)]
fn rail_edge_eps<B: Backend>(
    dev: &ConeDevelopment<B>,
    mu_rail: &RatFunc<B>,
    sigma_a: &Rat<B>,
    sigma_b: &Rat<B>,
    p_a: &FlatBox<B>,
    p_b: &FlatBox<B>,
    subdiv: usize,
    cfg: &DevConfig<B>,
) -> Result<Rat<B>, UnrollFault> {
    // Chord g(t) between the developed endpoint centers, parametrized by t = σ on [σ_a, σ_b].
    let (ax, ay) = p_a.center();
    let (bx, by) = p_b.center();
    let target = [
        linear_through(sigma_a, &ax, sigma_b, &bx),
        linear_through(sigma_a, &ay, sigma_b, &by),
    ];
    let cert = AnchorDevCert {
        dev: dev.clone(),
        sigma: RatFunc::new(
            Poly::from_coeffs(vec![Rat::from_i128(0), Rat::from_i128(1)]), // σ(t) = t
            Poly::from_coeffs(vec![Rat::from_i128(1)]),
        ),
        mu: mu_rail.clone(),
        target,
        span: Interval {
            lo: sigma_a.clone(),
            hi: sigma_b.clone(),
        },
        subdiv,
        // Permissive: we want the computed ε back, then apply one outline-level DRC.
        clearance: Rat::from_i128(1_000_000),
        cfg: cfg.clone(),
    };
    match anchor_dev(&cert) {
        Verdict::Verified(v) => Ok(v.eps),
        // Only DegenerateSpan / PoleInEval are refutations; the huge clearance rules out
        // Unresolved. A degenerate edge cannot occur here (σ_a < σ_b by construction).
        Verdict::Refuted(AnchorDevFault::PoleInEval) => Err(UnrollFault::PoleInEval),
        Verdict::Refuted(AnchorDevFault::DegenerateSpan) => Err(UnrollFault::DegenerateSpan),
        Verdict::Unresolved(_) => Err(UnrollFault::PoleInEval),
    }
}

/// Develop a free-boundary μ-band into a certified flat pattern outline (direction ①).
///
/// Discretizes each rail into `segments` edges over `[σ_lo, σ_hi]`, develops every station to a
/// [`FlatBox`], and certifies each rail edge's straight chord against the true developed rail
/// (via the DEV.2c lift bound). Returns the ordered flat polyline plus the uniform `ε` (max
/// edge bound), gated by the DRC: `Verified(`[`FlatOutline`]`)` when `ε < clearance/2`,
/// `Unresolved(ε)` when the outline is not yet within tolerance (refine `segments`), or
/// `Refuted(`[`UnrollFault`]`)` for a degenerate span / pole.
pub fn unroll_freeboundary<B: Backend>(
    dev: &ConeDevelopment<B>,
    sigma: &Interval<B>,
    mu_lo: &RatFunc<B>,
    mu_hi: &RatFunc<B>,
    segments: usize,
    cfg: &DevConfig<B>,
    clearance: &Rat<B>,
) -> Verdict<FlatOutline<B>, UnrollFault, Rat<B>> {
    use core::cmp::Ordering;
    if sigma.lo.cmp(&sigma.hi) != Ordering::Less {
        return Verdict::Refuted(UnrollFault::DegenerateSpan);
    }
    let n = segments.max(1);
    let step = sigma.hi.sub(&sigma.lo).div(&Rat::from_i128(n as i128));
    let station = |k: usize| sigma.lo.add(&step.mul(&Rat::from_i128(k as i128)));

    // Develop one rail's stations to FlatBoxes (μ̂ = rail(σ_k)); `None` on a rail pole.
    let develop_rail = |rail: &RatFunc<B>| -> Option<Vec<FlatBox<B>>> {
        (0..=n)
            .map(|k| {
                let s = station(k);
                let mu = rail.eval(&s)?;
                Some(dev.point(&s, &mu, cfg))
            })
            .collect()
    };
    let lo_pts = match develop_rail(mu_lo) {
        Some(v) => v,
        None => return Verdict::Refuted(UnrollFault::PoleInEval),
    };
    let hi_pts = match develop_rail(mu_hi) {
        Some(v) => v,
        None => return Verdict::Refuted(UnrollFault::PoleInEval),
    };

    // Certify each rail edge's chord fidelity; the uniform outline ε is their maximum. The
    // subdivision inside each edge's lift bound is a few sub-intervals (the edge is already short).
    let edge_subdiv = 4usize;
    let mut eps = Rat::from_i128(0);
    for (rail, pts) in [(mu_lo, &lo_pts), (mu_hi, &hi_pts)] {
        for k in 0..n {
            let (sa, sb) = (station(k), station(k + 1));
            let e = match rail_edge_eps(dev, rail, &sa, &sb, &pts[k], &pts[k + 1], edge_subdiv, cfg)
            {
                Ok(e) => e,
                Err(f) => return Verdict::Refuted(f),
            };
            if e.cmp(&eps) == Ordering::Greater {
                eps = e;
            }
        }
    }

    // The boundary loop: μ⁻ rail (σ_lo→σ_hi) then μ⁺ rail (σ_hi→σ_lo). The two σ-caps are the
    // exact straight radial edges joining `lo_pts.last()`↔`hi_pts.last()` and back.
    let mut vertices = lo_pts;
    vertices.extend(hi_pts.into_iter().rev());

    let half = clearance.mul(&Rat::new(1, 2));
    if eps.cmp(&half) == Ordering::Less {
        Verdict::Verified(FlatOutline {
            vertices,
            eps,
            clearance: clearance.clone(),
        })
    } else {
        Verdict::Unresolved(eps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fixtures::devices::cone;

    type Q = Rat<Bignum>;

    fn poly(cs: &[i128]) -> Poly<Bignum> {
        Poly::from_coeffs(cs.iter().map(|&c| Q::from_i128(c)).collect())
    }
    fn ratf(cs: &[i128]) -> RatFunc<Bignum> {
        RatFunc::from_poly(poly(cs))
    }
    fn ivl(lo: i128, hi: i128) -> Interval<Bignum> {
        Interval {
            lo: Q::from_i128(lo),
            hi: Q::from_i128(hi),
        }
    }
    fn dev() -> ConeDevelopment<Bignum> {
        ConeDevelopment::new(&cone()).unwrap()
    }
    fn eps_of(v: &Verdict<FlatOutline<Bignum>, UnrollFault, Q>) -> Q {
        match v {
            Verdict::Verified(o) => o.eps.clone(),
            Verdict::Unresolved(e) => e.clone(),
            Verdict::Refuted(_) => panic!("unexpected refutation"),
        }
    }

    /// The device cone's tapered band μ⁻ = −1, μ⁺ = −1/2 develops to a flat outline whose
    /// polyline is within ε of the true development, and ε shrinks as the rail discretization
    /// refines (finer polyline ⇒ tighter chords).
    #[test]
    fn outline_epsilon_shrinks_with_segments() {
        let (d, s) = (dev(), ivl(0, 1));
        let (mu_lo, mu_hi) = (ratf(&[-1]), ratf(&[-1, 1])); // μ⁻ = −1, μ⁺(σ) = −1 + σ (tapered)
        let coarse = unroll_freeboundary(
            &d,
            &s,
            &mu_lo,
            &mu_hi,
            4,
            &DevConfig::tight(),
            &Q::from_i128(1000),
        );
        let fine = unroll_freeboundary(
            &d,
            &s,
            &mu_lo,
            &mu_hi,
            32,
            &DevConfig::tight(),
            &Q::from_i128(1000),
        );
        assert!(
            eps_of(&fine).cmp(&eps_of(&coarse)) == core::cmp::Ordering::Less,
            "outline ε must shrink with finer segments"
        );
        // The outline is a closed loop: (segments+1) vertices per rail, two rails.
        if let Verdict::Verified(o) = fine {
            assert_eq!(o.vertices.len(), 2 * (32 + 1));
        }
    }

    /// A generous clearance certifies the flat outline; a tight one leaves it Unresolved.
    #[test]
    fn generous_clearance_certifies_tight_is_unresolved() {
        let (d, s) = (dev(), ivl(0, 1));
        let (mu_lo, mu_hi) = (ratf(&[-1]), ratf(&[-1, 1])); // μ⁺(σ) = −1 + σ (tapered, affine)
        // Measure ε with a huge clearance, then bracket it.
        let e = eps_of(&unroll_freeboundary(
            &d,
            &s,
            &mu_lo,
            &mu_hi,
            16,
            &DevConfig::tight(),
            &Q::from_i128(1000),
        ));
        let generous = e.mul(&Q::from_i128(4));
        let tight = e.div(&Q::from_i128(100));
        assert!(matches!(
            unroll_freeboundary(&d, &s, &mu_lo, &mu_hi, 16, &DevConfig::tight(), &generous),
            Verdict::Verified(_)
        ));
        assert!(matches!(
            unroll_freeboundary(&d, &s, &mu_lo, &mu_hi, 16, &DevConfig::tight(), &tight),
            Verdict::Unresolved(_)
        ));
    }

    /// A degenerate σ-span is refused before any development.
    #[test]
    fn degenerate_span_is_refuted() {
        let d = dev();
        assert!(matches!(
            unroll_freeboundary(
                &d,
                &ivl(1, 1),
                &ratf(&[-1]),
                &ratf(&[-1, 1]),
                8,
                &DevConfig::tight(),
                &Q::from_i128(1),
            ),
            Verdict::Refuted(UnrollFault::DegenerateSpan)
        ));
    }

    /// Each outline vertex is a rigorous enclosure: the developed boundary point sits in its box.
    #[test]
    fn outline_vertices_enclose_the_development() {
        let (d, s) = (dev(), ivl(0, 1));
        let (mu_lo, mu_hi) = (ratf(&[-1]), ratf(&[-1, 1]));
        if let Verdict::Verified(o) = unroll_freeboundary(
            &d,
            &s,
            &mu_lo,
            &mu_hi,
            8,
            &DevConfig::tight(),
            &Q::from_i128(1000),
        ) {
            // Vertex 0 is the μ⁻ rail at σ = 0: D(0, −1) = (|−1|·ρ(0), 0) = (144/97, 0).
            let v0 = &o.vertices[0];
            assert!(v0.x.contains(&Q::new(144, 97)) && v0.y.contains(&Q::from_i128(0)));
        } else {
            panic!("must certify under a generous clearance");
        }
    }
}
