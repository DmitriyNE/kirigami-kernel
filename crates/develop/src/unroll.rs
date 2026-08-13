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
//!
//! The band is only the simplest closed region. [`unroll_trim_loop`] is the **general engine**:
//! it develops an *arbitrary ordered loop* of [`BoundaryArc`]s — σ-monotone **rail** arcs
//! `μ̂(σ)` (e.g. the G2 [cut rails](crate::cut)) joined by ruling **cap**s — into the same
//! certified [`FlatOutline`], with `ε = max` over the rail edges (caps develop exactly). The
//! loop must close in `(σ, μ̂)` (checked exactly, [`ArcDiscontinuity`](UnrollFault::ArcDiscontinuity)
//! otherwise), and must not cross the apex `μ̂ = 0` (the development uses `|μ̂|·ρ`, so a boundary
//! through the apex would fold — a real cut region never touches it). [`unroll_freeboundary`] is
//! the band expressed as the 4-arc loop `[Rail μ⁻, Cap, Rail μ⁺, Cap]` through that engine.

use crate::anchor::{AnchorDevCert, AnchorDevFault, anchor_dev};
use crate::cone::{DevConfig, FlatBox};
use crate::part::Development;
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
    /// The σ-span is empty or degenerate (`σ_lo ≥ σ_hi`, or a [`BoundaryArc::Rail`] with
    /// `sigma_start == sigma_end`).
    DegenerateSpan,
    /// A rail `μ±(σ)` or the development had a pole on a sub-interval (propagated from the
    /// per-edge [`anchor_dev`](crate::anchor::anchor_dev)).
    PoleInEval,
    /// A [trim loop](unroll_trim_loop) does not close: the arc at this index starts at a
    /// `(σ, μ̂)` corner different from where the previous arc ended (index `0` flags the
    /// wrap-around gap between the last arc's end and the first arc's start).
    ArcDiscontinuity {
        /// The index of the arc whose start does not meet the previous arc's end.
        index: usize,
    },
    /// A [trim loop](unroll_trim_loop) was handed no arcs.
    EmptyLoop,
}

/// One directed element of a [trim loop](unroll_trim_loop): either a curved rail arc `μ̂(σ)`
/// or a straight ruling cap. Arcs are traversed in list order and must chain end-to-start in
/// `(σ, μ̂)` to form a closed boundary.
#[derive(Clone)]
pub enum BoundaryArc<B: Backend = Bignum> {
    /// A σ-monotone **rail** arc: the boundary follows the cone point `C(σ, μ̂(σ))` as σ runs
    /// `sigma_start → sigma_end` (`sigma_start` may exceed `sigma_end` — a backward-traversed
    /// rail). It develops to a polyline of `segments` chords, each certified against the true
    /// developed rail by the DEV.2c lift bound.
    Rail {
        /// The ruling coordinate `μ̂(σ)` as a rational function of σ.
        mu: RatFunc<B>,
        /// The σ the arc starts at.
        sigma_start: Rat<B>,
        /// The σ the arc ends at.
        sigma_end: Rat<B>,
        /// The number of chord segments the rail is discretized into (`≥ 1`).
        segments: usize,
    },
    /// A ruling **cap** at fixed σ: the boundary follows the ruling as μ̂ runs
    /// `mu_start → mu_end`. It develops to a *single exact* straight radial edge, so it carries
    /// no curve-fidelity bound.
    Cap {
        /// The fixed σ of the ruling.
        sigma: Rat<B>,
        /// The ruling coordinate the cap starts at.
        mu_start: Rat<B>,
        /// The ruling coordinate the cap ends at.
        mu_end: Rat<B>,
    },
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
/// read back). The edge anchors piece-by-piece through the development's
/// [`anchor_pieces`](Development::anchor_pieces) decomposition — one frameless piece for a
/// single-region development (the original path, byte-identical), one framed piece per region
/// for a piecewise gluing — and the edge's `ε` is the max over its pieces (the chord is one
/// function of σ across all of them). Returns the edge's `ε`, or the propagated fault.
#[allow(clippy::too_many_arguments)]
fn rail_edge_eps<B: Backend>(
    dev: &impl Development<B>,
    mu_rail: &RatFunc<B>,
    sigma_a: &Rat<B>,
    sigma_b: &Rat<B>,
    p_a: &FlatBox<B>,
    p_b: &FlatBox<B>,
    subdiv: usize,
    cfg: &DevConfig<B>,
) -> Result<Rat<B>, UnrollFault> {
    use core::cmp::Ordering;
    // Chord g(t) between the developed endpoint centers, parametrized by t = σ on [σ_a, σ_b].
    let (ax, ay) = p_a.center();
    let (bx, by) = p_b.center();
    let target = [
        linear_through(sigma_a, &ax, sigma_b, &bx),
        linear_through(sigma_a, &ay, sigma_b, &by),
    ];
    let span = Interval {
        lo: sigma_a.clone(),
        hi: sigma_b.clone(),
    };
    let pieces = dev
        .anchor_pieces(&span, cfg)
        .ok_or(UnrollFault::PoleInEval)?;
    let mut eps = Rat::from_i128(0);
    for piece in pieces {
        let cert = AnchorDevCert {
            dev: piece.dev.clone(),
            sigma: RatFunc::new(
                Poly::from_coeffs(vec![Rat::from_i128(0), Rat::from_i128(1)]), // σ(t) = t
                Poly::from_coeffs(vec![Rat::from_i128(1)]),
            ),
            mu: mu_rail.clone(),
            target: target.clone(),
            span: piece.span,
            subdiv,
            // Permissive: we want the computed ε back, then apply one outline-level DRC.
            clearance: Rat::from_i128(1_000_000),
            cfg: cfg.clone(),
            frame: piece.frame,
        };
        match anchor_dev(&cert) {
            Verdict::Verified(v) => {
                if v.eps.cmp(&eps) == Ordering::Greater {
                    eps = v.eps;
                }
            }
            // Only DegenerateSpan / PoleInEval are refutations; the huge clearance rules out
            // Unresolved. A degenerate edge cannot occur here (σ_a < σ_b by construction).
            Verdict::Refuted(AnchorDevFault::PoleInEval) => return Err(UnrollFault::PoleInEval),
            Verdict::Refuted(AnchorDevFault::DegenerateSpan) => {
                return Err(UnrollFault::DegenerateSpan);
            }
            Verdict::Unresolved(_) => return Err(UnrollFault::PoleInEval),
        }
    }
    Ok(eps)
}

/// Develop a free-boundary μ-band into a certified flat pattern outline (direction ①).
///
/// The band `μ⁻(σ), μ⁺(σ)` over `[σ_lo, σ_hi]` is the 4-arc [trim loop](unroll_trim_loop)
/// `[Rail μ⁻ (σ_lo→σ_hi), Cap σ_hi, Rail μ⁺ (σ_hi→σ_lo), Cap σ_lo]`; this is the ergonomic
/// constructor for that common shape and simply delegates to [`unroll_trim_loop`]. Returns the
/// ordered flat polyline plus the uniform `ε` (max rail-edge bound), gated by the DRC:
/// `Verified(`[`FlatOutline`]`)` when `ε < clearance/2`, `Unresolved(ε)` when the outline is not
/// yet within tolerance (refine `segments`), or `Refuted(`[`UnrollFault`]`)` for a degenerate
/// span / pole.
pub fn unroll_freeboundary<B: Backend>(
    dev: &impl Development<B>,
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
    // The two σ-caps sweep the ruling between the rails at each end; a rail pole at an endpoint
    // is a `PoleInEval`, exactly as the old per-station development reported.
    let (lo_at_hi, hi_at_hi, hi_at_lo, lo_at_lo) = match (
        mu_lo.eval(&sigma.hi),
        mu_hi.eval(&sigma.hi),
        mu_hi.eval(&sigma.lo),
        mu_lo.eval(&sigma.lo),
    ) {
        (Some(a), Some(b), Some(c), Some(d)) => (a, b, c, d),
        _ => return Verdict::Refuted(UnrollFault::PoleInEval),
    };
    let arcs = [
        BoundaryArc::Rail {
            mu: mu_lo.clone(),
            sigma_start: sigma.lo.clone(),
            sigma_end: sigma.hi.clone(),
            segments,
        },
        BoundaryArc::Cap {
            sigma: sigma.hi.clone(),
            mu_start: lo_at_hi,
            mu_end: hi_at_hi,
        },
        BoundaryArc::Rail {
            mu: mu_hi.clone(),
            sigma_start: sigma.hi.clone(),
            sigma_end: sigma.lo.clone(),
            segments,
        },
        BoundaryArc::Cap {
            sigma: sigma.lo.clone(),
            mu_start: hi_at_lo,
            mu_end: lo_at_lo,
        },
    ];
    unroll_trim_loop(dev, &arcs, cfg, clearance)
}

/// One developed [`BoundaryArc`]: its `(σ, μ̂)` endpoints, the developed [`FlatBox`] stations in
/// traversal order with their matching station σ's, and — for a rail — the `μ̂(σ)` used to
/// certify its edges (`None` for a cap, which develops exactly).
struct ArcData<B: Backend> {
    start_sm: (Rat<B>, Rat<B>),
    end_sm: (Rat<B>, Rat<B>),
    points: Vec<FlatBox<B>>,
    sigmas: Vec<Rat<B>>,
    rail_mu: Option<RatFunc<B>>,
}

/// Whether two `(σ, μ̂)` corners are exactly equal (development is injective on the gore, so this
/// is equivalent to the developed flat points coinciding — but stays float-free).
fn sm_eq<B: Backend>(a: &(Rat<B>, Rat<B>), b: &(Rat<B>, Rat<B>)) -> bool {
    use core::cmp::Ordering;
    a.0.cmp(&b.0) == Ordering::Equal && a.1.cmp(&b.1) == Ordering::Equal
}

/// Develop one [`BoundaryArc`] to its [`ArcData`], or a fault: a rail with `sigma_start ==
/// sigma_end` is `DegenerateSpan`, and a rail `μ̂` with a pole at a station is `PoleInEval`.
fn develop_arc<B: Backend>(
    dev: &impl Development<B>,
    arc: &BoundaryArc<B>,
    cfg: &DevConfig<B>,
) -> Result<ArcData<B>, UnrollFault> {
    use core::cmp::Ordering;
    match arc {
        BoundaryArc::Rail {
            mu,
            sigma_start,
            sigma_end,
            segments,
        } => {
            if sigma_start.cmp(sigma_end) == Ordering::Equal {
                return Err(UnrollFault::DegenerateSpan);
            }
            let n = (*segments).max(1);
            let step = sigma_end.sub(sigma_start).div(&Rat::from_i128(n as i128));
            let mut points = Vec::with_capacity(n + 1);
            let mut sigmas = Vec::with_capacity(n + 1);
            for k in 0..=n {
                let s = sigma_start.add(&step.mul(&Rat::from_i128(k as i128)));
                let m = mu.eval(&s).ok_or(UnrollFault::PoleInEval)?;
                points.push(dev.point(&s, &m, cfg).ok_or(UnrollFault::PoleInEval)?);
                sigmas.push(s);
            }
            // k=0 and k=n land exactly on σ_start and σ_end (ℚ arithmetic), so the endpoint μ̂'s
            // equal the first/last station μ̂'s — used as the closure key.
            let start_mu = mu.eval(sigma_start).ok_or(UnrollFault::PoleInEval)?;
            let end_mu = mu.eval(sigma_end).ok_or(UnrollFault::PoleInEval)?;
            Ok(ArcData {
                start_sm: (sigma_start.clone(), start_mu),
                end_sm: (sigma_end.clone(), end_mu),
                points,
                sigmas,
                rail_mu: Some(mu.clone()),
            })
        }
        BoundaryArc::Cap {
            sigma,
            mu_start,
            mu_end,
        } => Ok(ArcData {
            start_sm: (sigma.clone(), mu_start.clone()),
            end_sm: (sigma.clone(), mu_end.clone()),
            points: vec![
                dev.point(sigma, mu_start, cfg)
                    .ok_or(UnrollFault::PoleInEval)?,
                dev.point(sigma, mu_end, cfg)
                    .ok_or(UnrollFault::PoleInEval)?,
            ],
            sigmas: vec![sigma.clone(), sigma.clone()],
            rail_mu: None,
        }),
    }
}

/// Develop a general **trim loop** — an ordered, closed loop of [`BoundaryArc`]s — into a
/// certified flat pattern outline (direction ①); the general engine [`unroll_freeboundary`]
/// delegates to.
///
/// Each `Rail` arc is discretized into `segments` chords, every station developed to a
/// [`FlatBox`], and each chord certified against the true developed rail via the DEV.2c lift
/// bound; `Cap` arcs develop to exact straight radial edges (no fidelity cost). The arcs must
/// chain **end→start in `(σ, μ̂)`** and the last must meet the first (checked exactly). The
/// concatenated stations form one ordered closed polyline; the uniform `ε` is the max over all
/// rail edges.
///
/// **Precondition:** no arc may cross the apex `μ̂ = 0` (the development uses `|μ̂|·ρ`, so a
/// boundary through the apex would fold); a real cut region never touches it.
///
/// Returns `Verified(`[`FlatOutline`]`)` when `ε < clearance/2`, `Unresolved(ε)` when not yet
/// within tolerance (refine `segments`), or `Refuted(`[`UnrollFault`]`)` for an empty loop, a
/// degenerate/pole arc, or a loop that does not close.
///
/// ```
/// use develop::cone::{ConeDevelopment, DevConfig};
/// use develop::unroll::{unroll_trim_loop, BoundaryArc};
/// use certify_core::Verdict;
/// use fixtures::devices::cone;
/// use lattice::{Bignum, Poly, Rat, RatFunc};
///
/// let dev = ConeDevelopment::new(&cone()).unwrap();
/// let ratf = |c: i128| RatFunc::<Bignum>::from_poly(Poly::constant(Rat::from_i128(c)));
/// let q = |n: i128| Rat::<Bignum>::from_i128(n);
/// // The band μ⁻ = −1, μ⁺ = −2 over σ ∈ [0, 1] as an explicit 4-arc loop.
/// let arcs = [
///     BoundaryArc::Rail { mu: ratf(-1), sigma_start: q(0), sigma_end: q(1), segments: 8 },
///     BoundaryArc::Cap  { sigma: q(1), mu_start: q(-1), mu_end: q(-2) },
///     BoundaryArc::Rail { mu: ratf(-2), sigma_start: q(1), sigma_end: q(0), segments: 8 },
///     BoundaryArc::Cap  { sigma: q(0), mu_start: q(-2), mu_end: q(-1) },
/// ];
/// let v = unroll_trim_loop(&dev, &arcs, &DevConfig::tight(), &Rat::from_i128(1000));
/// assert!(matches!(v, Verdict::Verified(_)));
/// ```
pub fn unroll_trim_loop<B: Backend>(
    dev: &impl Development<B>,
    arcs: &[BoundaryArc<B>],
    cfg: &DevConfig<B>,
    clearance: &Rat<B>,
) -> Verdict<FlatOutline<B>, UnrollFault, Rat<B>> {
    use core::cmp::Ordering;
    if arcs.is_empty() {
        return Verdict::Refuted(UnrollFault::EmptyLoop);
    }
    // The subdivision inside each edge's lift bound is a few sub-intervals (the edge is short).
    let edge_subdiv = 4usize;
    let mut eps = Rat::from_i128(0);
    let mut vertices: Vec<FlatBox<B>> = Vec::new();
    let mut first_start: Option<(Rat<B>, Rat<B>)> = None;
    let mut prev_end: Option<(Rat<B>, Rat<B>)> = None;

    for (index, arc) in arcs.iter().enumerate() {
        let data = match develop_arc(dev, arc, cfg) {
            Ok(d) => d,
            Err(f) => return Verdict::Refuted(f),
        };
        // Closure: this arc's start meets the previous arc's end, exactly in (σ, μ̂).
        match &prev_end {
            Some(pe) if !sm_eq(pe, &data.start_sm) => {
                return Verdict::Refuted(UnrollFault::ArcDiscontinuity { index });
            }
            None => first_start = Some(data.start_sm.clone()),
            _ => {}
        }
        prev_end = Some(data.end_sm.clone());

        // ε from rail edges (caps develop exactly). `rail_edge_eps` needs σ_lo < σ_hi, so order
        // each edge's endpoints ascending in σ — a backward rail visits σ descending.
        if let Some(mu) = &data.rail_mu {
            for (sw, pw) in data.sigmas.windows(2).zip(data.points.windows(2)) {
                let (s_lo, p_lo, s_hi, p_hi) = if sw[0].cmp(&sw[1]) == Ordering::Less {
                    (&sw[0], &pw[0], &sw[1], &pw[1])
                } else {
                    (&sw[1], &pw[1], &sw[0], &pw[0])
                };
                match rail_edge_eps(dev, mu, s_lo, s_hi, p_lo, p_hi, edge_subdiv, cfg) {
                    Ok(e) => {
                        if e.cmp(&eps) == Ordering::Greater {
                            eps = e;
                        }
                    }
                    Err(f) => return Verdict::Refuted(f),
                }
            }
        }

        // Assemble: append the arc's points, skipping the first for every arc after the first
        // (it duplicates the shared corner already present as the running last vertex —
        // guaranteed identical by the closure check).
        let skip = if vertices.is_empty() { 0 } else { 1 };
        vertices.extend(data.points.into_iter().skip(skip));
    }

    // The loop must close: the last arc's end meets the first arc's start.
    if let (Some(fs), Some(pe)) = (&first_start, &prev_end) {
        if !sm_eq(pe, fs) {
            return Verdict::Refuted(UnrollFault::ArcDiscontinuity { index: 0 });
        }
    }
    // Drop the final vertex: it develops the same (σ, μ̂) as `vertices[0]` (loop closure).
    vertices.pop();

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
    use crate::cone::ConeDevelopment;
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

    /// Whether two flat boxes mutually contain each other's center (equal boxes trivially do).
    fn boxes_agree(a: &FlatBox<Bignum>, b: &FlatBox<Bignum>) -> bool {
        let (ax, ay) = a.center();
        let (bx, by) = b.center();
        a.x.contains(&bx) && a.y.contains(&by) && b.x.contains(&ax) && b.y.contains(&ay)
    }

    /// The band `unroll_freeboundary` produces equals the explicit 4-arc trim loop it delegates
    /// to — same ε, matching vertices. Pins the band ≡ `[Rail, Cap, Rail, Cap]` encoding.
    #[test]
    fn explicit_band_loop_matches_freeboundary() {
        let (d, s) = (dev(), ivl(0, 1));
        let (mu_lo, mu_hi) = (ratf(&[-2]), ratf(&[-1])); // μ⁻ ≡ −2, μ⁺ ≡ −1 (strictly negative)
        let clr = Q::from_i128(1000);
        let band = unroll_freeboundary(&d, &s, &mu_lo, &mu_hi, 8, &DevConfig::tight(), &clr);
        let arcs = [
            BoundaryArc::Rail {
                mu: mu_lo.clone(),
                sigma_start: Q::from_i128(0),
                sigma_end: Q::from_i128(1),
                segments: 8,
            },
            BoundaryArc::Cap {
                sigma: Q::from_i128(1),
                mu_start: Q::from_i128(-2),
                mu_end: Q::from_i128(-1),
            },
            BoundaryArc::Rail {
                mu: mu_hi.clone(),
                sigma_start: Q::from_i128(1),
                sigma_end: Q::from_i128(0),
                segments: 8,
            },
            BoundaryArc::Cap {
                sigma: Q::from_i128(0),
                mu_start: Q::from_i128(-1),
                mu_end: Q::from_i128(-2),
            },
        ];
        let loop_ = unroll_trim_loop(&d, &arcs, &DevConfig::tight(), &clr);
        match (band, loop_) {
            (Verdict::Verified(a), Verdict::Verified(b)) => {
                assert_eq!(
                    a.eps.cmp(&b.eps),
                    core::cmp::Ordering::Equal,
                    "ε must match"
                );
                assert_eq!(
                    a.vertices.len(),
                    b.vertices.len(),
                    "vertex count must match"
                );
                for (va, vb) in a.vertices.iter().zip(b.vertices.iter()) {
                    assert!(boxes_agree(va, vb), "vertices must coincide");
                }
            }
            _ => panic!("both the band and its explicit loop must certify"),
        }
    }

    /// A triangular region (two rails + one cap) certifies under a generous clearance, and its ε
    /// shrinks as the rails are discretized more finely.
    #[test]
    fn triangle_loop_certifies_and_shrinks() {
        let d = dev();
        let clr = Q::from_i128(1000);
        // Corners (σ,μ̂): (0,−1) →[rail μ̂≡−1]→ (1,−1) →[cap]→ (1,−2) →[rail μ̂=−1−σ]→ (0,−1).
        let mk = |seg: usize| {
            [
                BoundaryArc::Rail {
                    mu: ratf(&[-1]),
                    sigma_start: Q::from_i128(0),
                    sigma_end: Q::from_i128(1),
                    segments: seg,
                },
                BoundaryArc::Cap {
                    sigma: Q::from_i128(1),
                    mu_start: Q::from_i128(-1),
                    mu_end: Q::from_i128(-2),
                },
                BoundaryArc::Rail {
                    mu: ratf(&[-1, -1]),
                    sigma_start: Q::from_i128(1),
                    sigma_end: Q::from_i128(0),
                    segments: seg,
                },
            ]
        };
        assert!(matches!(
            unroll_trim_loop(&d, &mk(8), &DevConfig::tight(), &clr),
            Verdict::Verified(_)
        ));
        let coarse = eps_of(&unroll_trim_loop(&d, &mk(4), &DevConfig::tight(), &clr));
        let fine = eps_of(&unroll_trim_loop(&d, &mk(32), &DevConfig::tight(), &clr));
        assert!(
            fine.cmp(&coarse) == core::cmp::Ordering::Less,
            "ε must shrink with finer rails"
        );
    }

    /// A G2 plane-cut rail ([`crate::cut::plane_cut_rail`]) feeds the general trim loop: banded
    /// against a scaled copy of itself (same sign ⇒ no apex crossing), it certifies at a generous
    /// clearance and is Unresolved at a tight one — the G2→G3 composition.
    #[test]
    fn plane_cut_rail_trim_loop() {
        use crate::cut::plane_cut_rail;
        let chart = cone();
        let d = ConeDevelopment::new(&chart).unwrap();
        let n = [Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)]; // plane z = 1
        let dd = Q::from_i128(1);
        let mu_p = plane_cut_rail(&chart, &n, &dd); // outer cut rail (no pole/zero on [1,3])
        let mu_in = mu_p.scale(&Q::from_i128(2)); // inner rail 2·μ_p — same sign, so no apex
        let (sa, sb) = (Q::from_i128(1), Q::from_i128(3));
        let (a, b) = (mu_p.eval(&sa).unwrap(), mu_p.eval(&sb).unwrap());
        let (a2, b2) = (mu_in.eval(&sa).unwrap(), mu_in.eval(&sb).unwrap());
        let arcs = [
            BoundaryArc::Rail {
                mu: mu_p.clone(),
                sigma_start: sa.clone(),
                sigma_end: sb.clone(),
                segments: 8,
            },
            BoundaryArc::Cap {
                sigma: sb.clone(),
                mu_start: b,
                mu_end: b2,
            },
            BoundaryArc::Rail {
                mu: mu_in.clone(),
                sigma_start: sb.clone(),
                sigma_end: sa.clone(),
                segments: 8,
            },
            BoundaryArc::Cap {
                sigma: sa.clone(),
                mu_start: a2,
                mu_end: a,
            },
        ];
        let eps = eps_of(&unroll_trim_loop(
            &d,
            &arcs,
            &DevConfig::tight(),
            &Q::from_i128(1000),
        ));
        assert!(
            eps.sign() > 0,
            "a curved cut rail has a positive fidelity ε"
        );
        let generous = eps.mul(&Q::from_i128(4));
        let tight = eps.div(&Q::from_i128(100));
        assert!(matches!(
            unroll_trim_loop(&d, &arcs, &DevConfig::tight(), &generous),
            Verdict::Verified(_)
        ));
        assert!(matches!(
            unroll_trim_loop(&d, &arcs, &DevConfig::tight(), &tight),
            Verdict::Unresolved(_)
        ));
    }

    /// A band spanning σ across 0 (two-sided gore, `|ψ| < π`) develops and certifies — exercising
    /// the G1 mod-2π range reduction end-to-end (negative σ ⇒ negative ψ).
    #[test]
    fn two_sided_gore_loop() {
        let d = dev();
        let arcs = [
            BoundaryArc::Rail {
                mu: ratf(&[-2]),
                sigma_start: Q::from_i128(-1),
                sigma_end: Q::from_i128(1),
                segments: 8,
            },
            BoundaryArc::Cap {
                sigma: Q::from_i128(1),
                mu_start: Q::from_i128(-2),
                mu_end: Q::from_i128(-1),
            },
            BoundaryArc::Rail {
                mu: ratf(&[-1]),
                sigma_start: Q::from_i128(1),
                sigma_end: Q::from_i128(-1),
                segments: 8,
            },
            BoundaryArc::Cap {
                sigma: Q::from_i128(-1),
                mu_start: Q::from_i128(-1),
                mu_end: Q::from_i128(-2),
            },
        ];
        assert!(matches!(
            unroll_trim_loop(&d, &arcs, &DevConfig::tight(), &Q::from_i128(1000)),
            Verdict::Verified(_)
        ));
    }

    /// A loop whose arcs do not chain in `(σ, μ̂)` is refused (ArcDiscontinuity), not silently sewn.
    #[test]
    fn open_loop_is_refuted() {
        let d = dev();
        let arcs = [
            BoundaryArc::Rail {
                mu: ratf(&[-1]),
                sigma_start: Q::from_i128(0),
                sigma_end: Q::from_i128(1),
                segments: 4,
            },
            // The cap starts at σ = 2 ≠ 1 (where the rail ended) — a gap at arc index 1.
            BoundaryArc::Cap {
                sigma: Q::from_i128(2),
                mu_start: Q::from_i128(-1),
                mu_end: Q::from_i128(-2),
            },
        ];
        assert!(matches!(
            unroll_trim_loop(&d, &arcs, &DevConfig::tight(), &Q::from_i128(1000)),
            Verdict::Refuted(UnrollFault::ArcDiscontinuity { index: 1 })
        ));
    }

    /// A rail with a pole inside its span is refused (PoleInEval).
    #[test]
    fn pole_in_rail_is_refuted() {
        let d = dev();
        // μ̂ = 1/(σ − 2), a pole at σ = 2 — a station of [1,3] at 4 segments (1, 1.5, 2, 2.5, 3).
        let mu = RatFunc::new(Poly::constant(Q::from_i128(1)), poly(&[-2, 1]));
        let arcs = [BoundaryArc::Rail {
            mu,
            sigma_start: Q::from_i128(1),
            sigma_end: Q::from_i128(3),
            segments: 4,
        }];
        assert!(matches!(
            unroll_trim_loop(&d, &arcs, &DevConfig::tight(), &Q::from_i128(1000)),
            Verdict::Refuted(UnrollFault::PoleInEval)
        ));
    }

    /// An empty loop is refused (EmptyLoop).
    #[test]
    fn empty_loop_is_refuted() {
        let d = dev();
        let arcs: [BoundaryArc<Bignum>; 0] = [];
        assert!(matches!(
            unroll_trim_loop(&d, &arcs, &DevConfig::tight(), &Q::from_i128(1)),
            Verdict::Refuted(UnrollFault::EmptyLoop)
        ));
    }

    /// A rail arc with `sigma_start == sigma_end` is refused (DegenerateSpan).
    #[test]
    fn degenerate_rail_arc_is_refuted() {
        let d = dev();
        let arcs = [BoundaryArc::Rail {
            mu: ratf(&[-1]),
            sigma_start: Q::from_i128(1),
            sigma_end: Q::from_i128(1),
            segments: 4,
        }];
        assert!(matches!(
            unroll_trim_loop(&d, &arcs, &DevConfig::tight(), &Q::from_i128(1)),
            Verdict::Refuted(UnrollFault::DegenerateSpan)
        ));
    }

    /// The general engine's vertices are rigorous enclosures too: the (0,−1) corner of a triangle
    /// loop encloses its known development `D(0, −1) = (144/97, 0)`.
    #[test]
    fn trim_loop_vertices_enclose_development() {
        let d = dev();
        let arcs = [
            BoundaryArc::Rail {
                mu: ratf(&[-1]),
                sigma_start: Q::from_i128(0),
                sigma_end: Q::from_i128(1),
                segments: 8,
            },
            BoundaryArc::Cap {
                sigma: Q::from_i128(1),
                mu_start: Q::from_i128(-1),
                mu_end: Q::from_i128(-2),
            },
            BoundaryArc::Rail {
                mu: ratf(&[-1, -1]),
                sigma_start: Q::from_i128(1),
                sigma_end: Q::from_i128(0),
                segments: 8,
            },
        ];
        if let Verdict::Verified(o) =
            unroll_trim_loop(&d, &arcs, &DevConfig::tight(), &Q::from_i128(1000))
        {
            let v0 = &o.vertices[0]; // the (0, −1) corner
            assert!(v0.x.contains(&Q::new(144, 97)) && v0.y.contains(&Q::from_i128(0)));
        } else {
            panic!("must certify under a generous clearance");
        }
    }

    #[test]
    fn a_piecewise_band_unrolls_with_chord_certified_edges() {
        // The first chord-certified piecewise flat outline: the device cone (γ≡0) on [0, 1/4]
        // glued to the ramp flap (γ≠0) on [1/4, 1/2], unrolled over σ ∈ [1/8, 3/8] with
        // segments = 3 — so the middle rail edge [5/24, 7/24] genuinely straddles the region
        // join at 1/4 and its ε comes from TWO framed anchor pieces (one per region), not from
        // a pointwise backward error.
        use crate::part::PiecewiseDevelopment;
        use fixtures::devices::cone_seam_ramp;
        let pw = PiecewiseDevelopment::new(vec![
            (
                Interval {
                    lo: Q::from_i128(0),
                    hi: Q::new(1, 4),
                },
                ConeDevelopment::new(&cone()).unwrap(),
            ),
            (
                Interval {
                    lo: Q::new(1, 4),
                    hi: Q::new(1, 2),
                },
                ConeDevelopment::new_developable(&cone_seam_ramp(), 32).unwrap(),
            ),
        ])
        .unwrap();
        let span = Interval {
            lo: Q::new(1, 8),
            hi: Q::new(3, 8),
        };
        match unroll_freeboundary(
            &pw,
            &span,
            &ratf(&[-2]),
            &ratf(&[-1]),
            3,
            &DevConfig::tight(),
            &Q::from_i128(1000),
        ) {
            Verdict::Verified(o) => {
                // 4-arc loop, 2·(segments+1) + 2·2 stations − 3 shared corners − 1 closing dup.
                assert_eq!(o.vertices.len(), 2 * (3 + 1));
                // A real (positive) chord-lift bound, comfortably finite.
                assert!(o.eps.cmp(&Q::from_i128(0)) == core::cmp::Ordering::Greater);
                assert!(o.eps.cmp(&Q::from_i128(1)) == core::cmp::Ordering::Less);
            }
            Verdict::Refuted(f) => panic!("piecewise unroll refuted: {f:?}"),
            Verdict::Unresolved(e) => panic!("piecewise unroll unresolved: eps = {e:?}"),
        }
    }
}
