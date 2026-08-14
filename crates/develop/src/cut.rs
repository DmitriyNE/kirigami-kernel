//! The **cut-fit certificate** — G2: certify that a proposed rational ruling-rail
//! `μ̂(σ)` traces a cutting surface `{F(X) = 0}` on the cone, so it can enter the
//! certified unroll/anchor pipeline as a genuine cut curve.
//!
//! A rail point `C(σ, μ̂(σ)) = pedal(σ) + μ̂(σ)·ruling(σ) + w·normal(σ)` is **on the
//! cone by construction**; the only obligation is that it also lies on the cut
//! *surface*. So the certificate is a **geometric-distance bound**
//! `sup_σ dist(C(σ, μ̂(σ)), {F=0}) ≤ ε`, gated by the DRC `ε < clearance/2`
//! (spec:192) — "on the surface ∧ on the cone ⟹ on the cut curve". It is the
//! rational sibling of [`crate::anchor::anchor_dev`]: same scaffolding (subdivide
//! the σ-span, interval-enclose, take `ε = max`, DRC), but the residual is
//! **purely rational in σ** — no `cos`/`sin`/`arctan` development — so it reuses
//! only the rational interval primitives, never the transcendental ones.
//!
//! Two surface kinds cover the Stage-1 cuts (the 3-D lift of
//! [`certify_core::cap_in`]'s `Carrier` line/circle): an **offset plane**
//! `{n·X = d}` — whose exact rail [`plane_cut_rail`] is *rational*, verified with
//! `ε ≈ 0` — and a **cylinder**, whose cone∩cylinder rail is a surd fitted by a
//! float oracle (`export::cut_oracle`) and re-verified here. Fail-closed: a loose
//! fit or wrong branch yields a large `ε` ⇒ [`Unresolved`](Verdict::Unresolved),
//! never a wrong [`Verified`](Verdict::Verified). No float enters this certificate.

use crate::cone::DevConfig;
use crate::interval::{RatIv, abs_on, eval_ratfunc_on, sqrt, sqrt_on};
use certify_core::Verdict;
use geom::chart::Chart;
use lattice::{Backend, Bignum, Interval, Poly, Rat, RatFunc, Vec3Rat};

/// A rational cutting surface `{F(X) = 0}` — the 3-D lift of
/// [`certify_core::cap_in`]'s 2-D `Carrier` (line ↦ plane, circle ↦ cylinder).
#[derive(Clone)]
pub enum CutSurface<B: Backend = Bignum> {
    /// The plane `{n·X = d}`. `n` need not be unit — the certificate divides the
    /// implicit residual `n·X − d` by `|n|` to get a true geometric distance.
    Plane {
        /// The plane normal `n` (nonzero).
        n: [Rat<B>; 3],
        /// The plane offset `d`.
        d: Rat<B>,
    },
    /// The cylinder of radius `√r2` about the axis through `axis_point` in direction
    /// `axis_dir` (which need not be unit — the certificate divides by `axis_dir·axis_dir`).
    Cylinder {
        /// A point on the cylinder axis.
        axis_point: [Rat<B>; 3],
        /// The axis direction (nonzero).
        axis_dir: [Rat<B>; 3],
        /// The squared radius `R²` (positive).
        r2: Rat<B>,
    },
}

/// A cut-fit certificate: a proposed ruling-rail `μ̂(σ)` (at layer offset `w`) that
/// claims to trace `surface` over the σ-`span`, checked against the fab `clearance`.
#[derive(Clone)]
pub struct CutFitCert<B: Backend = Bignum> {
    /// The proposed rail `μ̂(σ)` — the ruling coordinate as a rational function of σ
    /// (exact for a plane cut; a float-oracle fit for a cylinder cut).
    pub mu_hat: RatFunc<B>,
    /// The layer offset `w` along the normal (`0` for the single-layer mid-surface).
    pub w: Rat<B>,
    /// The cutting surface the rail claims to lie on.
    pub surface: CutSurface<B>,
    /// The σ-span `[σ_lo, σ_hi]` the rail is authored over.
    pub span: Interval<B>,
    /// The number of equal σ-sub-intervals the rigorous `sup_σ` is taken over — the
    /// refinement handle (more sub-intervals ⇒ a tighter `ε`).
    pub subdiv: usize,
    /// The item's exact fab clearance; the DRC gate is `ε < clearance/2`.
    pub clearance: Rat<B>,
    /// The `√`-bisection budget (`sqrt_eps`) for the radius / norm enclosures.
    pub cfg: DevConfig<B>,
}

/// The evidence a valid cut-fit carries: the certified σ-span and the uniform
/// distance bound `ε`, under the recorded clearance.
#[derive(Clone)]
pub struct ValidCutFit<B: Backend = Bignum> {
    /// The σ-span over which the bound holds.
    pub span: Interval<B>,
    /// The certified uniform bound `sup_σ dist(C(σ, μ̂(σ)), {F=0}) ≤ ε`.
    pub eps: Rat<B>,
    /// The clearance the DRC compared against (`ε < clearance/2`).
    pub clearance: Rat<B>,
}

/// Why the cut-fit checker refused a certificate (looseness is *not* here — a loose
/// fit is [`Unresolved`](Verdict::Unresolved), refined by `subdiv`, never `Refuted`).
#[derive(Clone, Debug)]
pub enum CutFitFault {
    /// The σ-span is empty or degenerate (`σ_lo ≥ σ_hi`).
    DegenerateSpan,
    /// The surface is malformed: a zero plane normal or zero cylinder axis direction.
    DegenerateSurface,
    /// A rational field (the rail `μ̂`, a chart field, or the residual) had a
    /// denominator enclosure straddling zero on a sub-interval — a possible pole, so
    /// the quotient is unbounded there. Refine, or re-author the span away from the pole.
    PoleInEval,
}

/// A constant vector as a degree-0 [`Vec3Rat`] (denominator `1`), so it dots with the
/// chart's σ-rational fields. (Local copy of `closure::trim`'s helper — `develop`
/// does not depend on `closure`.)
fn const_vec3<B: Backend>(v: &[Rat<B>; 3]) -> Vec3Rat<B> {
    Vec3Rat::new(
        [
            Poly::constant(v[0].clone()),
            Poly::constant(v[1].clone()),
            Poly::constant(v[2].clone()),
        ],
        Poly::constant(Rat::from_i128(1)),
    )
}

/// The exact dot product of two constant 3-vectors.
fn dot3<B: Backend>(a: &[Rat<B>; 3], b: &[Rat<B>; 3]) -> Rat<B> {
    a[0].mul(&b[0]).add(&a[1].mul(&b[1])).add(&a[2].mul(&b[2]))
}

/// The **exact** offset-plane cut rail `μ(σ) = (d − n·pedal(σ)) / (n·ruling(σ))`, the
/// solution of `n·C(σ,μ) = d` (affine in μ). Rational — no fit — so [`cut_fit`]
/// verifies it with `ε ≈ 0`. The denominator `n·ruling(σ)` vanishes exactly where the
/// ruling is parallel to the plane (the cut exits the gore); keep the span clear of it.
pub fn plane_cut_rail<B: Backend>(chart: &Chart<B>, n: &[Rat<B>; 3], d: &Rat<B>) -> RatFunc<B> {
    let nv = const_vec3(n);
    let g0 = chart.pedal().dot(&nv); // n·pedal(σ)
    let g_mu = chart.ruling().dot(&nv); // n·ruling(σ)
    let num = RatFunc::from_poly(Poly::constant(d.clone())).sub(&g0); // d − n·pedal
    num.div(&g_mu)
}

/// The **µ̂-pullback** of a cut surface onto a chart: the implicit residual of the surface along
/// the ruling, `s(σ, µ̂) = a(σ)·µ̂² + b(σ)·µ̂ + c(σ)`, with σ-rational coefficients — every
/// [`CutSurface`] is degree ≤ 2 in µ̂ because the chart is ruled (`X = pedal + µ̂·ruling + w·normal`
/// is affine in µ̂). Reads the **true** chart fields, so it is correct on offset supports (`h ≠ 0`)
/// and under wrapping parametrizations — never an apex-ray shortcut. Built by [`cut_mu_form`].
///
/// Sign semantics: for a [`CutSurface::Cylinder`] the residual is `perp² − R²` — **negative
/// strictly inside** the solid cylinder; for a [`CutSurface::Plane`] it is `n·X − d` (`a ≡ 0`) —
/// negative on the `n·X < d` side. So `s < 0` is the natural "inside the solid cutter" predicate.
pub struct MuCut<B: Backend = Bignum> {
    /// The µ̂² coefficient (`0` for a plane; `≥ 0` pointwise for a cylinder, by Cauchy–Schwarz).
    pub a: RatFunc<B>,
    /// The µ̂ coefficient.
    pub b: RatFunc<B>,
    /// The µ̂⁰ term.
    pub c: RatFunc<B>,
}

impl<B: Backend> MuCut<B> {
    /// The residual `s(σ, µ̂)` at a rational point, or `None` on a coefficient pole.
    pub fn eval(&self, sigma: &Rat<B>, mu_hat: &Rat<B>) -> Option<Rat<B>> {
        let a = self.a.eval(sigma)?;
        let b = self.b.eval(sigma)?;
        let c = self.c.eval(sigma)?;
        Some(a.mul(mu_hat).add(&b).mul(mu_hat).add(&c))
    }

    /// The discriminant `b² − 4ac` — for a cylinder cut, **positive exactly where the ruling
    /// crosses the cylinder** (two real µ̂ branches); its roots are the true tangent rulings, the
    /// σ-extent of an interior hole. (For a plane, `a ≡ 0` and this degenerates to `b²`.)
    pub fn disc(&self) -> RatFunc<B> {
        self.b.mul(&self.b).sub(
            &self
                .a
                .mul(&self.c)
                .mul(&RatFunc::from_poly(Poly::constant(Rat::from_i128(4)))),
        )
    }
}

/// Pull a [`CutSurface`] back to its µ̂-form [`MuCut`] on `chart` at layer offset `w` (the
/// residual along `X(σ, µ̂) = pedal + µ̂·ruling + w·normal`). `None` for a degenerate surface
/// (zero plane normal / zero cylinder axis).
///
/// This is the single pedal-general pullback the trim/authoring layer composes on: plane rails
/// come from the linear root (`µ̂ = −c/b`, see [`plane_cut_rail`]), cylinder branches from the
/// quadratic roots, hole σ-extents from [`MuCut::disc`].
pub fn cut_mu_form<B: Backend>(
    chart: &Chart<B>,
    surface: &CutSurface<B>,
    w: &Rat<B>,
) -> Option<MuCut<B>> {
    let base = chart.pedal().add(&chart.normal().scale_rat(w)); // pedal + w·normal
    let u = chart.ruling();
    match surface {
        CutSurface::Plane { n, d } => {
            if dot3(n, n).sign() <= 0 {
                return None;
            }
            let nv = const_vec3(n);
            Some(MuCut {
                a: RatFunc::zero(),
                b: u.dot(&nv),
                c: base
                    .dot(&nv)
                    .sub(&RatFunc::from_poly(Poly::constant(d.clone()))),
            })
        }
        CutSurface::Cylinder {
            axis_point,
            axis_dir,
            r2,
        } => {
            let a2 = dot3(axis_dir, axis_dir);
            if a2.sign() <= 0 {
                return None;
            }
            let inv_a2 = a2.recip();
            let ax = const_vec3(axis_dir);
            let v0 = base.sub(&const_vec3(axis_point)); // pedal + w·n − p
            let ua = u.dot(&ax);
            let va = v0.dot(&ax);
            let a = u.dot(u).sub(&ua.mul(&ua).scale(&inv_a2));
            let b = v0
                .dot(u)
                .sub(&va.mul(&ua).scale(&inv_a2))
                .scale(&Rat::from_i128(2));
            let c = v0
                .dot(&v0)
                .sub(&va.mul(&va).scale(&inv_a2))
                .sub(&RatFunc::from_poly(Poly::constant(r2.clone())));
            Some(MuCut {
                a: a.reduce(),
                b: b.reduce(),
                c: c.reduce(),
            })
        }
    }
}

/// The σ-sub-interval `[σ_lo + k·width, σ_lo + (k+1)·width]`.
fn subiv<B: Backend>(lo: &Rat<B>, width: &Rat<B>, k: usize) -> RatIv<B> {
    let a = lo.add(&width.mul(&Rat::from_i128(k as i128)));
    let b = a.add(width);
    RatIv::new(a, b)
}

/// The larger of two rationals.
fn max_rat<B: Backend>(a: Rat<B>, b: Rat<B>) -> Rat<B> {
    if a.cmp(&b) == core::cmp::Ordering::Less {
        b
    } else {
        a
    }
}

/// Certify the cut-fit obligation `sup_σ dist(C(σ, μ̂(σ)), {F=0}) ≤ ε` and gate it by
/// the DRC `ε < clearance/2`.
///
/// The checker **computes** the bound itself (its interval arithmetic is the trusted
/// part; it does not trust a searcher-supplied `ε`): it builds the rail point
/// `C(σ) = pedal + μ̂·ruling + w·normal` as a [`Vec3Rat`], subdivides `[σ_lo, σ_hi]`
/// into `subdiv` equal sub-intervals, and on each encloses the **geometric distance**
/// to the surface —
/// - **plane:** `|n·C − d| / |n|` (`|n|` a constant `√` enclosure);
/// - **cylinder:** `|√perp2(σ) − R|`, `perp2 = |C−p|² − ((C−p)·â)²/(â·â)`, `R = √r2`
///
/// — taking the maximum `ε`. Refining `subdiv` shrinks `ε`. Total:
/// `Verified(`[`ValidCutFit`]`)` when `ε < clearance/2`, `Unresolved(ε)` when not
/// (refine `subdiv`, or the oracle re-fits), or `Refuted(`[`CutFitFault`]`)` for a
/// degenerate span/surface or a pole in the evaluated residual.
pub fn cut_fit<B: Backend>(
    chart: &Chart<B>,
    cert: &CutFitCert<B>,
) -> Verdict<ValidCutFit<B>, CutFitFault, Rat<B>> {
    // The rail point on the cone: C(σ) = pedal + μ̂(σ)·ruling + w·normal, rational in σ.
    let c = chart
        .pedal()
        .add(&chart.ruling().scale(&cert.mu_hat))
        .add(&chart.normal().scale_rat(&cert.w));
    traced_cut_fit(
        &c,
        &cert.surface,
        &cert.span,
        cert.subdiv,
        &cert.clearance,
        &cert.cfg,
    )
}

/// Certify that a **p-curve** traces a cutting surface: the same obligation as [`cut_fit`], stated
/// over the curve's own parameter — `sup_t dist(X(t), {F=0}) ≤ ε` for the curve's 3-D image
/// `X(t)` ([`PCurve::lift`](crate::pcurve::PCurve::lift)) — and gated by the same DRC.
///
/// A graph rail is the special case `σ(t) = t`, so this subsumes [`cut_fit`] rather than competing
/// with it: both hand the same core the traced point as a rational vector function of whatever
/// parameter it is authored over. The generalization is what lets a cut **turn around in σ** — at
/// a solid cutter's tangent rulings, where a graph has to stop short — and so lets a closed cut be
/// certified as one curve instead of two branches plus bridges.
///
/// `Refuted(DegenerateSpan)` on an empty parameter span, `PoleInEval` where the traced point or
/// residual poles; a curve that drifts off the surface is `Unresolved(ε)`, never a wrong
/// `Verified`.
pub fn pcurve_cut_fit<B: Backend>(
    chart: &Chart<B>,
    curve: &crate::pcurve::PCurve<B>,
    surface: &CutSurface<B>,
    w: &Rat<B>,
    subdiv: usize,
    clearance: &Rat<B>,
    cfg: &DevConfig<B>,
) -> Verdict<ValidCutFit<B>, CutFitFault, Rat<B>> {
    let x = match curve.lift(chart, w) {
        Some(x) => x,
        None => return Verdict::Refuted(CutFitFault::PoleInEval),
    };
    traced_cut_fit(&x, surface, &curve.domain, subdiv, clearance, cfg)
}

/// The shared core: the traced point `c` as a rational vector function of its own parameter, the
/// rigorous `sup` of its distance to `surface` over `span`, and the DRC gate. Both the graph and
/// p-curve entry points differ only in how `c` is built.
fn traced_cut_fit<B: Backend>(
    c: &Vec3Rat<B>,
    surface: &CutSurface<B>,
    span: &Interval<B>,
    subdiv: usize,
    clearance: &Rat<B>,
    cfg: &DevConfig<B>,
) -> Verdict<ValidCutFit<B>, CutFitFault, Rat<B>> {
    use core::cmp::Ordering;
    let (lo, hi) = (&span.lo, &span.hi);
    if lo.cmp(hi) != Ordering::Less {
        return Verdict::Refuted(CutFitFault::DegenerateSpan);
    }
    let n_sub = subdiv.max(1);
    let width = hi.sub(lo).div(&Rat::from_i128(n_sub as i128));

    let eps = match surface {
        CutSurface::Plane { n, d } => {
            let norm = sqrt(&dot3(n, n), &cfg.sqrt_eps); // |n| enclosure
            let inv_norm = match norm.recip_pos() {
                Some(iv) => iv,
                None => return Verdict::Refuted(CutFitFault::DegenerateSurface),
            };
            // residual(σ) = n·C(σ) − d, a rational function of σ.
            let residual = c
                .dot(&const_vec3(n))
                .sub(&RatFunc::from_poly(Poly::constant(d.clone())))
                .reduce();
            let mut eps = Rat::from_i128(0);
            for k in 0..n_sub {
                let sig = subiv(lo, &width, k);
                let res = match eval_ratfunc_on(&residual, &sig) {
                    Some(r) => r,
                    None => return Verdict::Refuted(CutFitFault::PoleInEval),
                };
                // distance = |residual| / |n|
                let dist = abs_on(&res).mul(&inv_norm);
                eps = max_rat(eps, dist.hi().clone());
            }
            eps
        }
        CutSurface::Cylinder {
            axis_point,
            axis_dir,
            r2,
        } => {
            let a2 = dot3(axis_dir, axis_dir); // â·â
            if a2.sign() <= 0 {
                return Verdict::Refuted(CutFitFault::DegenerateSurface);
            }
            let inv_a2 = a2.recip();
            let dvec = c.sub(&const_vec3(axis_point)); // X − p
            let ax = const_vec3(axis_dir);
            let axdot = dvec.dot(&ax); // (X − p)·â
            // perp2(σ) = |X−p|² − ((X−p)·â)² / (â·â), the squared distance to the axis.
            let perp2 = dvec
                .dot(&dvec)
                .sub(&axdot.mul(&axdot).scale(&inv_a2))
                .reduce();
            let r = sqrt(r2, &cfg.sqrt_eps); // R = √r2 enclosure
            let mut eps = Rat::from_i128(0);
            for k in 0..n_sub {
                let sig = subiv(lo, &width, k);
                let p2 = match eval_ratfunc_on(&perp2, &sig) {
                    Some(p) => p,
                    None => return Verdict::Refuted(CutFitFault::PoleInEval),
                };
                let rho = sqrt_on(&p2, &cfg.sqrt_eps); // √perp2 = distance to axis
                let dist = abs_on(&rho.sub(&r)); // |ρ − R|
                eps = max_rat(eps, dist.hi().clone());
            }
            eps
        }
    };

    let half = clearance.mul(&Rat::new(1, 2));
    if eps.cmp(&half) == Ordering::Less {
        Verdict::Verified(ValidCutFit {
            span: span.clone(),
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

    fn to_f64(r: &Q) -> f64 {
        let (n, d) = r.numer_denom_decimal();
        n.parse::<f64>().unwrap() / d.parse::<f64>().unwrap()
    }
    fn ivl(lo: i128, hi: i128) -> Interval<Bignum> {
        Interval {
            lo: Q::from_i128(lo),
            hi: Q::from_i128(hi),
        }
    }

    /// A **graph** p-curve certifies exactly as the graph checker does — the p-curve certificate
    /// subsumes [`cut_fit`] rather than competing with it.
    #[test]
    fn a_graph_pcurve_certifies_like_the_graph_checker() {
        let chart = cone();
        let n = [Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)];
        let d = Q::from_i128(1);
        let surface = CutSurface::Plane {
            n: n.clone(),
            d: d.clone(),
        };
        let rail = plane_cut_rail(&chart, &n, &d);
        let curve = crate::pcurve::PCurve::graph(rail, ivl(1, 3));
        match pcurve_cut_fit(
            &chart,
            &curve,
            &surface,
            &Q::from_i128(0),
            8,
            &Q::new(1, 100),
            &DevConfig::tight(),
        ) {
            Verdict::Verified(v) => assert!(
                v.eps <= Q::new(1, 1_000_000),
                "exact rail ε ≈ 0, got {}",
                to_f64(&v.eps)
            ),
            other => panic!("expected Verified, got {:?}", verdict_tag(&other)),
        }
    }

    /// **The capability the graph model cannot express**: a cut curve that *turns around in σ*
    /// still certifies. The same exact plane rail is re-parametrized by `σ(t) = 1 − t²/4`, which
    /// reverses at `t = 0` — so `dµ̂/dσ` is unbounded there and no `µ̂ = f(σ)` covers the curve in
    /// one piece — yet the traced point lies on the plane throughout and the certificate says so
    /// at ε ≈ 0. The obligation is stated over the curve's own parameter, so it never sees the
    /// turn as a singularity.
    #[test]
    fn a_curve_that_turns_around_in_sigma_still_certifies() {
        let chart = cone();
        let n = [Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)];
        let d = Q::from_i128(1);
        let surface = CutSurface::Plane {
            n: n.clone(),
            d: d.clone(),
        };
        let rail = plane_cut_rail(&chart, &n, &d);
        // σ(t) = 1 − t²/4 on t ∈ [−1, 1]: σ ∈ [3/4, 1], reversing at t = 0.
        let sigma = RatFunc::from_poly(Poly::from_coeffs(vec![
            Q::from_i128(1),
            Q::from_i128(0),
            Q::new(-1, 4),
        ]));
        let mu = crate::pcurve::compose(&rail, &sigma).expect("composable");
        let curve = crate::pcurve::PCurve {
            sigma,
            mu,
            domain: ivl(-1, 1),
        };
        assert_eq!(
            curve.sigma_turning_points(64, 40).unwrap().len(),
            1,
            "the fixture must genuinely turn around in σ"
        );
        match pcurve_cut_fit(
            &chart,
            &curve,
            &surface,
            &Q::from_i128(0),
            16,
            &Q::new(1, 100),
            &DevConfig::tight(),
        ) {
            Verdict::Verified(v) => assert!(
                v.eps <= Q::new(1, 1_000_000),
                "a turning curve on the plane is still exact, got {}",
                to_f64(&v.eps)
            ),
            other => panic!("expected Verified, got {:?}", verdict_tag(&other)),
        }
    }

    /// Fail-closed: nudge the traced curve off the cutting surface and the certificate refuses to
    /// call it a cut — a loose curve is `Unresolved`, never a wrong `Verified`.
    #[test]
    fn a_curve_off_the_surface_is_not_certified() {
        let chart = cone();
        let n = [Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)];
        let d = Q::from_i128(1);
        let surface = CutSurface::Plane {
            n: n.clone(),
            d: d.clone(),
        };
        let drifted =
            plane_cut_rail(&chart, &n, &d).add(&RatFunc::from_poly(Poly::constant(Q::new(1, 10))));
        let curve = crate::pcurve::PCurve::graph(drifted, ivl(1, 3));
        match pcurve_cut_fit(
            &chart,
            &curve,
            &surface,
            &Q::from_i128(0),
            8,
            &Q::new(1, 100),
            &DevConfig::tight(),
        ) {
            Verdict::Unresolved(e) => assert!(
                e > Q::new(1, 1000),
                "the drift must show up in ε, got {}",
                to_f64(&e)
            ),
            other => panic!("expected Unresolved, got {:?}", verdict_tag(&other)),
        }
    }

    /// The exact offset-plane rail verifies with ε ≈ 0 (the residual is identically 0).
    #[test]
    fn plane_cut_rail_verifies_near_zero() {
        let chart = cone();
        let n = [Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)]; // z = d plane
        let d = Q::from_i128(1);
        let mu_hat = plane_cut_rail(&chart, &n, &d);
        let cert = CutFitCert {
            mu_hat,
            w: Q::from_i128(0),
            surface: CutSurface::Plane { n, d },
            span: ivl(1, 3),
            subdiv: 8,
            clearance: Q::new(1, 100),
            cfg: DevConfig::tight(),
        };
        match cut_fit(&chart, &cert) {
            Verdict::Verified(v) => assert!(
                v.eps <= Q::new(1, 1_000_000),
                "exact rail ε ≈ 0, got {}",
                to_f64(&v.eps)
            ),
            other => panic!("expected Verified, got {:?}", verdict_tag(&other)),
        }
    }

    /// An offset rail (μ̂ + δ) is off the plane: Unresolved at a tight clearance,
    /// Verified at a generous one; ε shrinks as the σ-subdivision refines.
    #[test]
    fn offset_plane_rail_is_unresolved_then_verified() {
        let chart = cone();
        let n = [Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)];
        let d = Q::from_i128(1);
        let delta = Q::new(1, 20); // push the rail off the plane
        let mu_hat = plane_cut_rail(&chart, &n, &d).add(&RatFunc::from_poly(Poly::constant(delta)));
        let mk = |clearance: Q, subdiv: usize| CutFitCert {
            mu_hat: mu_hat.clone(),
            w: Q::from_i128(0),
            surface: CutSurface::Plane {
                n: n.clone(),
                d: d.clone(),
            },
            span: ivl(1, 3),
            subdiv,
            clearance,
            cfg: DevConfig::tight(),
        };
        // Tight clearance ⇒ Unresolved with a positive ε.
        let eps_coarse = match cut_fit(&chart, &mk(Q::new(1, 1000), 4)) {
            Verdict::Unresolved(e) => e,
            other => panic!("expected Unresolved, got {:?}", verdict_tag(&other)),
        };
        assert!(eps_coarse.sign() > 0, "offset ⇒ positive ε");
        // Refining shrinks (or holds) the certified sup.
        let eps_fine = match cut_fit(&chart, &mk(Q::new(1, 1000), 32)) {
            Verdict::Unresolved(e) => e,
            Verdict::Verified(v) => v.eps,
            other => panic!("unexpected {:?}", verdict_tag(&other)),
        };
        assert!(eps_fine <= eps_coarse, "ε tightens with subdiv");
        // Generous clearance ⇒ Verified.
        match cut_fit(&chart, &mk(Q::from_i128(10), 16)) {
            Verdict::Verified(_) => {}
            other => panic!("expected Verified, got {:?}", verdict_tag(&other)),
        }
    }

    /// A degenerate span and a degenerate surface are Refuted (structural, not loose).
    #[test]
    fn degenerate_span_and_surface_are_refuted() {
        let chart = cone();
        let n = [Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)];
        let base = |span: Interval<Bignum>, surface: CutSurface<Bignum>| CutFitCert {
            mu_hat: plane_cut_rail(&chart, &n, &Q::from_i128(1)),
            w: Q::from_i128(0),
            surface,
            span,
            subdiv: 4,
            clearance: Q::from_i128(1),
            cfg: DevConfig::tight(),
        };
        // σ_lo ≥ σ_hi.
        match cut_fit(
            &chart,
            &base(
                ivl(2, 1),
                CutSurface::Plane {
                    n: n.clone(),
                    d: Q::from_i128(1),
                },
            ),
        ) {
            Verdict::Refuted(CutFitFault::DegenerateSpan) => {}
            other => panic!("expected DegenerateSpan, got {:?}", verdict_tag(&other)),
        }
        // Zero plane normal.
        match cut_fit(
            &chart,
            &base(
                ivl(1, 2),
                CutSurface::Plane {
                    n: [Q::from_i128(0), Q::from_i128(0), Q::from_i128(0)],
                    d: Q::from_i128(1),
                },
            ),
        ) {
            Verdict::Refuted(CutFitFault::DegenerateSurface) => {}
            other => panic!("expected DegenerateSurface, got {:?}", verdict_tag(&other)),
        }
    }

    /// A rail with a pole inside the span is Refuted (PoleInEval), not silently wrong.
    #[test]
    fn a_pole_in_span_is_refused() {
        let chart = cone();
        let n = [Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)];
        // μ̂ = 1/(σ − 2), a pole at σ = 2 ∈ [1, 3].
        let mu_hat = RatFunc::new(
            Poly::constant(Q::from_i128(1)),
            Poly::from_coeffs(vec![Q::from_i128(-2), Q::from_i128(1)]),
        );
        let cert = CutFitCert {
            mu_hat,
            w: Q::from_i128(0),
            surface: CutSurface::Plane {
                n,
                d: Q::from_i128(1),
            },
            span: ivl(1, 3),
            subdiv: 4,
            clearance: Q::from_i128(1),
            cfg: DevConfig::tight(),
        };
        match cut_fit(&chart, &cert) {
            Verdict::Refuted(CutFitFault::PoleInEval) => {}
            other => panic!("expected PoleInEval, got {:?}", verdict_tag(&other)),
        }
    }

    /// The plane µ̂-form vanishes identically on the exact plane rail: `b·µ̂₁ + c ≡ 0`.
    #[test]
    fn the_plane_mu_form_vanishes_on_the_exact_rail() {
        let chart = cone();
        let n = [Q::from_i128(1), Q::new(-1, 2), Q::from_i128(2)]; // a generic plane
        let d = Q::new(3, 4);
        let rail = plane_cut_rail(&chart, &n, &d);
        let form = cut_mu_form(&chart, &CutSurface::Plane { n, d }, &Q::from_i128(0)).unwrap();
        assert!(form.a.is_zero(), "a plane is affine in µ̂");
        let residual = form.b.mul(&rail).add(&form.c).reduce();
        assert!(residual.is_zero(), "s(σ, µ̂₁(σ)) ≡ 0 exactly");
    }

    /// The cylinder µ̂-form classifies inside/outside by sign, and its discriminant is positive
    /// exactly where the ruling crosses the cylinder — on the true surface, at a layer offset too.
    #[test]
    fn the_cylinder_mu_form_classifies_by_sign() {
        let chart = cone();
        // The demo D4 disk: a small vertical cylinder at (0, 11/5), R² = 1/25.
        let surface = CutSurface::Cylinder {
            axis_point: [Q::from_i128(0), Q::new(11, 5), Q::from_i128(0)],
            axis_dir: [Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)],
            r2: Q::new(1, 25),
        };
        let form = cut_mu_form(&chart, &surface, &Q::from_i128(0)).unwrap();
        // At σ = 0 the +y ruling passes through the disk: the surface point at the certified
        // annulus band µ̂ ≈ 2.2 lies inside (s < 0); µ̂ = 1 is well inside the disk radially? No —
        // µ̂ = 1 sits at xy-radius ≈ 0.9 from the origin, far outside the disk (s > 0).
        let s_in = form.eval(&Q::from_i128(0), &Q::new(11, 5)).unwrap();
        let s_out = form.eval(&Q::from_i128(0), &Q::from_i128(1)).unwrap();
        // Corroborate the signs against the actual 3-D distance (exact arithmetic).
        let check = |mu: &Q, want_inside: bool| {
            let p = chart
                .surface(mu, &Q::from_i128(0))
                .eval(&Q::from_i128(0))
                .unwrap();
            let dy = p[1].sub(&Q::new(11, 5));
            let perp2 = p[0].mul(&p[0]).add(&dy.mul(&dy));
            assert_eq!(
                perp2.cmp(&Q::new(1, 25)) == core::cmp::Ordering::Less,
                want_inside
            );
        };
        assert!(s_in.sign() < 0, "µ̂ on the ruling chord is inside");
        check(&Q::new(11, 5), true);
        assert!(s_out.sign() > 0, "µ̂ = 1 is outside the disk");
        check(&Q::from_i128(1), false);
        // The discriminant: positive at σ = 0 (the ruling crosses the disk), negative at σ = 1
        // (azimuth 90° away — the ruling misses it).
        let disc = form.disc();
        assert!(disc.eval(&Q::from_i128(0)).unwrap().sign() > 0);
        assert!(disc.eval(&Q::from_i128(1)).unwrap().sign() < 0);
    }

    /// The cylinder metric: the certified ε upper-bounds the float distance-to-cylinder
    /// at sampled σ (ε is the sup, so it dominates every sample).
    #[test]
    fn cylinder_distance_upper_bounds_the_float_distance() {
        let chart = cone();
        // Cylinder about the z-axis through the origin, radius √(1/4) = 1/2.
        let axis_point = [Q::from_i128(0), Q::from_i128(0), Q::from_i128(0)];
        let axis_dir = [Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)];
        let r2 = Q::new(1, 4);
        // A simple linear rail μ̂(σ) = σ.
        let mu_hat = RatFunc::from_poly(Poly::from_coeffs(vec![Q::from_i128(0), Q::from_i128(1)]));
        let cert = CutFitCert {
            mu_hat: mu_hat.clone(),
            w: Q::from_i128(0),
            surface: CutSurface::Cylinder {
                axis_point,
                axis_dir,
                r2: r2.clone(),
            },
            span: ivl(1, 2),
            subdiv: 32,
            clearance: Q::from_i128(1000), // generous: we only read ε here
            cfg: DevConfig::tight(),
        };
        let eps = match cut_fit(&chart, &cert) {
            Verdict::Verified(v) => v.eps,
            other => panic!(
                "expected Verified (generous clearance), got {:?}",
                verdict_tag(&other)
            ),
        };
        // Float audit: at sampled σ, |√(Cx²+Cy²) − R| ≤ ε.
        let r = 0.5f64;
        for i in 0..=10 {
            let s = Q::new(100 + 10 * i, 100); // σ ∈ [1.0, 2.0]
            let mu = mu_hat.eval(&s).unwrap();
            let pt = chart.surface(&mu, &Q::from_i128(0)).eval(&s).unwrap();
            let (x, y) = (to_f64(&pt[0]), to_f64(&pt[1]));
            let dist = ((x * x + y * y).sqrt() - r).abs();
            assert!(
                dist <= to_f64(&eps) + 1e-9,
                "certified ε {} must dominate float dist {dist} at σ={}",
                to_f64(&eps),
                to_f64(&s)
            );
        }
    }

    fn verdict_tag<E, W: core::fmt::Debug, M>(v: &Verdict<E, W, M>) -> String {
        match v {
            Verdict::Verified(_) => "Verified".into(),
            Verdict::Refuted(w) => format!("Refuted({w:?})"),
            Verdict::Unresolved(_) => "Unresolved".into(),
        }
    }
}
