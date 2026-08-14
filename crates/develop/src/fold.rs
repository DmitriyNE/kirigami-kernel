//! Certified **fold-inversion** (product direction ②, *per-panel*): the flat→3D map `D⁻¹`
//! for a single developable cone panel (`docs/implementation-plan-v1.md §6`).
//!
//! The forward development is the polar map `D(σ, μ̂) = |μ̂|·ρ(σ)·(cos ψ(σ), sin ψ(σ))`. Given a
//! flat point `(x, y)`, [`fold_point`] inverts it to a certified chart coordinate `(σ, μ̂)` and
//! lifts it back to 3D:
//!
//! - **angle → σ.** `θ = atan2(y, x) = ψ(σ)` is monotone in σ (`ψ′ = c/(1+σ²) > 0`), so σ is
//!   recovered by bisection — *without* ever computing the transcendental `θ`: the signed area
//!   `cos ψ(σ)·y − sin ψ(σ)·x = r·sin(θ − ψ(σ))` gives the search direction from certified
//!   `cos`/`sin` enclosures and the rational `(x, y)`. The result is a rational σ-enclosure.
//! - **radius → μ̂.** `r = √(x²+y²) = |μ̂|·ρ(σ)`, so `|μ̂| = r/ρ(σ)` (interval `√` over `r`, `ρ`
//!   over the σ-enclosure); the sign is the authored panel side.
//! - **lift.** the exact chart surface `C(σ,μ̂,w) = c(σ) + μ̂·r⃗(σ) + w·n(σ)` evaluated over the
//!   `(σ, μ̂)` enclosures → a rational 3D box.
//!
//! The certificate is the **round-trip backward error**: re-developing the recovered `(σ, μ̂)`
//! must reproduce the input flat point within `ε`, gated by the DRC `ε < clearance/2`. This is
//! the single-panel flat↔3D isometry; multi-panel creases / fold-mates are the **atlas** (D4.4)
//! + `closure`/`sew`, not `develop`. No float enters the certificate.

use crate::cone::{ConeDevelopment, DevConfig};
use crate::interval::{RatIv, cos_on, eval_ratfunc_on, sin_on, sqrt, sqrt_on};
use crate::part::PiecewiseDevelopment;
use certify_core::Verdict;
use geom::chart::Chart;
use lattice::{Backend, Bignum, Interval, Rat};

/// Quadrature budget for the flat directrix `γ` in the **γ ≠ 0 fold** (DD.3). Each `invert_sigma`
/// bisection step re-integrates `γ(σ)` from 0, so this trades fold speed against the round-trip ε;
/// `64` gives a fab-plausible ε on the seam ramp (the apex cone has `γ ≡ 0` and never integrates).
const GAMMA_PANELS: usize = 64;

/// A certified folded point: the recovered chart coordinate `(σ, μ̂)` enclosures, the lifted 3D
/// box `C(σ, μ̂, w)`, and the round-trip backward error `ε` under the recorded clearance.
#[derive(Clone)]
pub struct Fold3D<B: Backend = Bignum> {
    /// The recovered σ-coordinate enclosure (`ψ(σ) = atan2(y, x)`).
    pub sigma: RatIv<B>,
    /// The recovered ruling-coordinate enclosure `μ̂` (signed by the authored panel side).
    pub mu: RatIv<B>,
    /// The lifted 3D point `C(σ, μ̂, w)` as a rational box `[x] × [y] × [z]`.
    pub point: [RatIv<B>; 3],
    /// The round-trip backward error: an upper bound on `|D(σ, μ̂) − (x, y)|` (re-developing the
    /// recovered coordinate reproduces the input flat point to within `ε`).
    pub eps: Rat<B>,
    /// The clearance the DRC compared against (`ε < clearance/2`).
    pub clearance: Rat<B>,
}

/// A certified folded wire: the developed flat loop folded back to a 3-D polyline on the cone
/// (each vertex a [`fold_point`] 3-D box), with the uniform round-trip backward error `ε`.
pub struct FoldedWire<B: Backend = Bignum> {
    /// The folded 3-D wire vertices, in loop order — each a rational box `C(σ, μ̂, w)` on the cone.
    pub points: Vec<[RatIv<B>; 3]>,
    /// The uniform round-trip backward error: `max` over vertices of the per-point fold `ε`.
    pub eps: Rat<B>,
    /// The clearance the DRC compared against (`ε < clearance/2`).
    pub clearance: Rat<B>,
}

/// Why the fold-inversion refused a flat point.
#[derive(Clone, Debug)]
pub enum FoldFault {
    /// The chart is not a canonical arctangent cone ([`ConeDevelopment::new`] declined).
    NotACone,
    /// The σ-domain is empty or degenerate (`σ_lo ≥ σ_hi`).
    DegenerateDomain,
    /// The flat point's direction angle lies outside the gore's angular range
    /// `[ψ(σ_lo), ψ(σ_hi)]` — no σ in the domain develops to it.
    OutOfGore,
    /// A field denominator (`ρ²`, or a surface component) straddled zero on the enclosure — a
    /// pole risk, or the radius could not be signed strictly positive.
    PoleInEval,
    /// A [`fold_outline`] loop was handed no vertices.
    EmptyLoop,
    /// The chart slice handed to the piecewise fold is not parallel to the gluing's regions
    /// ([`fold_point_pw`] needs one chart per region for the 3-D lift).
    ChartMismatch,
}

/// The signed area `cos ψ(σ)·(y − γ_y(σ)) − sin ψ(σ)·(x − γ_x(σ))` at a rational σ — the
/// perpendicular component of the **directrix residual** `(x, y) − γ(σ)` against the developed ray
/// `e(ψ)`. It vanishes exactly when the residual is (anti)parallel to `e(ψ)`, i.e. at the true σ.
/// For the apex cone (`γ ≡ 0`) this reduces to the pure-radial `cos ψ·y − sin ψ·x` of DEV.2e.
fn cross_at<B: Backend>(
    dev: &ConeDevelopment<B>,
    s: &Rat<B>,
    x: &Rat<B>,
    y: &Rat<B>,
    cfg: &DevConfig<B>,
) -> RatIv<B> {
    let ang = dev.angle(s, cfg.terms);
    let c = cos_on(&ang, cfg.terms);
    let si = sin_on(&ang, cfg.terms);
    // Residual (x, y) − γ(σ); γ ≡ [0,0] for the apex cone, so this is byte-identical there.
    let g = dev.directrix_at(s, cfg).unwrap_or_else(|| {
        let z = RatIv::point(Rat::from_i128(0));
        [z.clone(), z]
    });
    let yg = RatIv::point(y.clone()).sub(&g[1]);
    let xg = RatIv::point(x.clone()).sub(&g[0]);
    c.mul(&yg).sub(&si.mul(&xg))
}

/// Recover the σ-enclosure with `ψ(σ) = atan2(y, x)` by monotone bisection on the signed area,
/// or a [`FoldFault`] if the domain is degenerate or the angle is outside the gore.
///
/// Precondition (G1): the sign of `cross = r·sin(θ − ψ(σ))` is faithful to `sign(θ − ψ(σ))` only
/// while the fold `domain`'s angular span keeps `|θ − ψ(σ)| < π`. `ψ = c·arctan σ` is monotone for
/// all σ, so any *one-sided* domain (σ all ≥ 0 or all ≤ 0) satisfies this automatically (its span is
/// `≤ c·π/2 < π`). A *two-sided* domain wide enough to reach span π (device: σ beyond ≈ ±2.3) must be
/// bisected within one sign of σ — split at σ = 0, chosen by `sign(θ)` — which is the caller's job
/// (future G4 `fold_outline`). `cos_on`/`sin_on` are now tight for negative ψ, so this bisection is
/// correct for σ < 0 without change, subject to that span bound.
fn invert_sigma<B: Backend>(
    dev: &ConeDevelopment<B>,
    x: &Rat<B>,
    y: &Rat<B>,
    domain: &Interval<B>,
    iters: usize,
    cfg: &DevConfig<B>,
    flip: bool,
) -> Result<RatIv<B>, FoldFault> {
    use core::cmp::Ordering;
    if domain.lo.cmp(&domain.hi) != Ordering::Less {
        return Err(FoldFault::DegenerateDomain);
    }
    // The signed area, with the **residual-direction flip**: for a γ ≠ 0 chart the flat point is
    // `γ(σ) + µ̂·ρ·e(ψ)` with *signed* µ̂, so a negative µ̂ (the device band) puts the residual
    // `(x, y) − γ(σ)` at angle `ψ + π` — the opposite bracketing convention. Negating the signed
    // area restores the `+ → −` monotonicity `invert_sigma` bisects on. The apex cone (`|µ̂|`,
    // residual always at `ψ`) never flips, so its bisection is byte-identical.
    let xat = |s: &Rat<B>| -> RatIv<B> {
        let c = cross_at(dev, s, x, y, cfg);
        if flip { c.neg() } else { c }
    };
    // The root must be bracketed: cross(σ_lo) ≥ 0 (θ ≥ ψ(σ_lo)) and cross(σ_hi) ≤ 0. If cross is
    // strictly negative at σ_lo (θ < ψ(σ_lo)) or strictly positive at σ_hi (θ > ψ(σ_hi)), the
    // target angle is outside the gore.
    if xat(&domain.lo).hi().sign() < 0 || xat(&domain.hi).lo().sign() > 0 {
        return Err(FoldFault::OutOfGore);
    }
    let (mut lo, mut hi) = (domain.lo.clone(), domain.hi.clone());
    two_probe_bisect(&mut lo, &mut hi, iters, |s| Ok(xat(s)))?;
    Ok(RatIv::new(lo, hi))
}

/// The **two-probe** monotone bisection step, shared by the single-panel and piecewise σ
/// inversions. Two probes at *non-dyadic* fractions (2/7, 5/7) of the bracket: a rational root
/// is never hit exactly, and — the load-bearing part — a probe whose signed-area enclosure
/// *straddles* zero (the root lies within enclosure width of it) does not end the search: the
/// other, well-separated probe still shrinks the bracket. Only both probes straddling means the
/// bracket is at the enclosures' resolution — the genuine convergence stop. (A single-probe
/// straddle-stop returns the *current* bracket, which at iteration 0 is the whole domain — a
/// boundary vertex sitting near the first split point folds with a domain-wide σ-enclosure.)
fn two_probe_bisect<B: Backend>(
    lo: &mut Rat<B>,
    hi: &mut Rat<B>,
    iters: usize,
    xat: impl Fn(&Rat<B>) -> Result<RatIv<B>, FoldFault>,
) -> Result<(), FoldFault> {
    let (r1, r2) = (Rat::new(2, 7), Rat::new(5, 7));
    let sign3 = |iv: &RatIv<B>| -> i8 {
        if iv.lo().sign() > 0 {
            1
        } else if iv.hi().sign() < 0 {
            -1
        } else {
            0
        }
    };
    let mut spent = 0usize;
    while spent < iters {
        let w = hi.sub(lo);
        let t1 = lo.add(&w.mul(&r1));
        let s1 = sign3(&xat(&t1)?);
        spent += 1;
        if s1 < 0 {
            *hi = t1; // ψ(t1) > θ ⇒ σ* < t1
            continue;
        }
        if spent >= iters {
            break;
        }
        let t2 = lo.add(&w.mul(&r2));
        let s2 = sign3(&xat(&t2)?);
        spent += 1;
        match (s1, s2) {
            (_, 1) => *lo = t2, // σ* > t2 (certified even when t1 straddles)
            (1, -1) => {
                *lo = t1;
                *hi = t2;
            }
            (1, 0) => *lo = t1,  // σ* within enclosure width of t2
            (0, -1) => *hi = t2, // σ* within enclosure width of t1
            // Both probes straddle: the bracket is at the enclosures' resolution.
            _ => return Ok(()),
        }
    }
    Ok(())
}

/// The largest `|c − t|` over `c ∈ box`, `t = target` — the axis residual of a round-trip.
fn axis_residual<B: Backend>(iv: &RatIv<B>, t: &Rat<B>) -> Rat<B> {
    let hi = iv.hi().sub(t);
    let lo = t.sub(iv.lo());
    let (hi, lo) = (abs(&hi), abs(&lo));
    if hi.cmp(&lo) == core::cmp::Ordering::Greater {
        hi
    } else {
        lo
    }
}
fn abs<B: Backend>(r: &Rat<B>) -> Rat<B> {
    if r.sign() < 0 { r.neg() } else { r.clone() }
}

/// Fold a flat point back onto a cone panel (direction ②): invert the polar development to a
/// certified `(σ, μ̂)` enclosure and lift it to a 3D box `C(σ, μ̂, w)`, gated by the round-trip
/// backward error.
///
/// `mu_negative` selects the authored panel side (the flat radius fixes only `|μ̂| = r/ρ`; the
/// sign is which side of the apex the ruling is retained — `true` for the device cone's band).
/// Returns `Verified(`[`Fold3D`]`)` when the re-developed point is within `clearance/2` of the
/// input, `Unresolved(ε)` when not (refine `iters`/`cfg`), or `Refuted(`[`FoldFault`]`)`.
#[allow(clippy::too_many_arguments)]
pub fn fold_point<B: Backend>(
    chart: &Chart<B>,
    x: &Rat<B>,
    y: &Rat<B>,
    w: &Rat<B>,
    domain: &Interval<B>,
    iters: usize,
    mu_negative: bool,
    cfg: &DevConfig<B>,
    clearance: &Rat<B>,
) -> Verdict<Fold3D<B>, FoldFault, Rat<B>> {
    use core::cmp::Ordering;
    // A curved-support developable admits a directrix γ (DD.2); the apex cone gets γ ≡ 0 and folds
    // byte-identically to DEV.2e (`new_developable` reduces to `new` when `pedal ≡ 0`).
    let dev = match ConeDevelopment::new_developable(chart, GAMMA_PANELS) {
        Some(d) => d,
        None => return Verdict::Refuted(FoldFault::NotACone),
    };
    // angle → σ. `flip` handles the γ ≠ 0, µ̂ < 0 residual-at-(ψ+π) case (see `invert_sigma`).
    let flip = dev.has_directrix() && mu_negative;
    let sigma = match invert_sigma(&dev, x, y, domain, iters, cfg, flip) {
        Ok(s) => s,
        Err(f) => return Verdict::Refuted(f),
    };
    // radius → |µ̂| = r / ρ(σ), where r = |(x, y) − γ(σ)| is the length of the directrix residual
    // (the pure `|(x, y)|` of DEV.2e when γ ≡ 0).
    let r = if dev.has_directrix() {
        let g = match dev.directrix_on_iv(&sigma, cfg) {
            Some(g) => g,
            None => return Verdict::Refuted(FoldFault::PoleInEval),
        };
        let xr = RatIv::point(x.clone()).sub(&g[0]);
        let yr = RatIv::point(y.clone()).sub(&g[1]);
        sqrt_on(&xr.mul(&xr).add(&yr.mul(&yr)), &cfg.sqrt_eps)
    } else {
        sqrt(&x.mul(x).add(&y.mul(y)), &cfg.sqrt_eps)
    };
    let rho = match dev.radius_on(&sigma, &cfg.sqrt_eps) {
        Some(rho) => rho,
        None => return Verdict::Refuted(FoldFault::PoleInEval),
    };
    let inv_rho = match rho.recip_pos() {
        Some(iv) => iv,
        None => return Verdict::Refuted(FoldFault::PoleInEval),
    };
    let abs_mu = r.mul(&inv_rho);
    let mu = if mu_negative { abs_mu.neg() } else { abs_mu };

    let point = match lift_box(chart, &sigma, &mu, w) {
        Ok(p) => p,
        Err(f) => return Verdict::Refuted(f),
    };

    // round-trip: re-develop the recovered (σ, μ̂) and measure the residual to the input (x, y).
    let back = match dev.point_on(&sigma, &mu, cfg) {
        Some(b) => b,
        None => return Verdict::Refuted(FoldFault::PoleInEval),
    };
    let (ex, ey) = (axis_residual(&back.x, x), axis_residual(&back.y, y));
    let eps = sqrt(&ex.mul(&ex).add(&ey.mul(&ey)), &cfg.sqrt_eps)
        .hi()
        .clone();

    let half = clearance.mul(&Rat::new(1, 2));
    if eps.cmp(&half) == Ordering::Less {
        Verdict::Verified(Fold3D {
            sigma,
            mu,
            point,
            eps,
            clearance: clearance.clone(),
        })
    } else {
        Verdict::Unresolved(eps)
    }
}

/// The one-sided σ sub-domain a point with flat height `y` folds within — the **σ=0 split**.
///
/// The signed-area bisection ([`invert_sigma`]) is only faithful while `|θ − ψ(σ)| < π`. A
/// one-sided domain always satisfies this (span `≤ c·π/2 < π`); a *two-sided* gore
/// (`σ_lo < 0 < σ_hi`) wide enough to reach span `π` does not, so it is split at σ=0 by the sign of
/// the flat angle. For a gore point `|θ| < c·π/2 < π`, so `sign(θ) = sign(sin θ) = sign(y) =
/// sign(σ)` exactly — `y ≥ 0` ⇒ the root is in `[0, σ_hi]`, `y < 0` ⇒ in `[σ_lo, 0]`. A
/// one-sided domain passes through unchanged.
fn split_domain<B: Backend>(domain: &Interval<B>, y: &Rat<B>) -> Interval<B> {
    if domain.lo.sign() < 0 && domain.hi.sign() > 0 {
        if y.sign() >= 0 {
            Interval {
                lo: Rat::from_i128(0),
                hi: domain.hi.clone(),
            }
        } else {
            Interval {
                lo: domain.lo.clone(),
                hi: Rat::from_i128(0),
            }
        }
    } else {
        domain.clone()
    }
}

/// Fold a whole flat loop back onto a cone panel (direction ②): [`fold_point`] every vertex into a
/// certified 3-D box and collect them into a [`FoldedWire`], with the **two-sided σ=0 split** so a
/// wide gore (ψ-span `> π`) folds correctly.
///
/// `domain` is the source gore's σ-range (may be two-sided); [`split_domain`] restricts each
/// vertex's bisection to the one-sided half matching `sign(y)`. `mu_negative` is the authored panel
/// side — one sign for the whole loop (a real cut region never crosses the apex `μ̂ = 0`). Each
/// vertex is folded under a permissive clearance to read its raw round-trip `ε` back; the uniform
/// `ε = max` is then gated once by the DRC.
///
/// Returns `Verified(`[`FoldedWire`]`)` when `ε < clearance/2`, `Unresolved(ε)` when the wire is not
/// yet within tolerance (refine `iters`/`cfg`), or `Refuted(`[`FoldFault`]`)` for an empty loop or a
/// vertex out of the gore / at a pole / on a non-cone chart.
///
/// ```
/// use develop::fold::fold_outline;
/// use develop::cone::{ConeDevelopment, DevConfig};
/// use certify_core::Verdict;
/// use fixtures::devices::cone;
/// use lattice::{Bignum, Interval, Rat};
///
/// let chart = cone();
/// let dev = ConeDevelopment::new(&chart).unwrap();
/// let q = |n: i128, d: i128| Rat::<Bignum>::new(n, d);
/// // A flat loop = the forward images of four (σ, μ̂) corners on the device gore.
/// let fwd = |s: Rat<Bignum>, m: Rat<Bignum>| {
///     let b = dev.point(&s, &m, &DevConfig::tight());
///     [b.x.mid(), b.y.mid()]
/// };
/// let loop_ = [fwd(q(0, 1), q(-1, 1)), fwd(q(1, 2), q(-1, 1)), fwd(q(1, 2), q(-2, 1)), fwd(q(0, 1), q(-2, 1))];
/// let domain = Interval { lo: q(0, 1), hi: q(1, 1) };
/// let v = fold_outline(&chart, &loop_, &q(0, 1), &domain, 60, true, &DevConfig::tight(), &q(1, 1));
/// assert!(matches!(v, Verdict::Verified(w) if w.points.len() == 4));
/// ```
#[allow(clippy::too_many_arguments)]
pub fn fold_outline<B: Backend>(
    chart: &Chart<B>,
    flat: &[[Rat<B>; 2]],
    w: &Rat<B>,
    domain: &Interval<B>,
    iters: usize,
    mu_negative: bool,
    cfg: &DevConfig<B>,
    clearance: &Rat<B>,
) -> Verdict<FoldedWire<B>, FoldFault, Rat<B>> {
    use core::cmp::Ordering;
    if flat.is_empty() {
        return Verdict::Refuted(FoldFault::EmptyLoop);
    }
    // Permissive per-vertex clearance: read each raw round-trip ε back, then apply one wire DRC
    // (the `unroll::rail_edge_eps` pattern).
    let permissive = Rat::from_i128(1_000_000);
    let mut points: Vec<[RatIv<B>; 3]> = Vec::with_capacity(flat.len());
    let mut eps = Rat::from_i128(0);
    for p in flat {
        let dom = split_domain(domain, &p[1]);
        match fold_point(
            chart,
            &p[0],
            &p[1],
            w,
            &dom,
            iters,
            mu_negative,
            cfg,
            &permissive,
        ) {
            Verdict::Verified(f) => {
                if f.eps.cmp(&eps) == Ordering::Greater {
                    eps = f.eps;
                }
                points.push(f.point);
            }
            Verdict::Refuted(fault) => return Verdict::Refuted(fault),
            // Unreachable under the permissive clearance; propagate defensively (panic-free).
            Verdict::Unresolved(e) => return Verdict::Unresolved(e),
        }
    }

    let half = clearance.mul(&Rat::new(1, 2));
    if eps.cmp(&half) == Ordering::Less {
        Verdict::Verified(FoldedWire {
            points,
            eps,
            clearance: clearance.clone(),
        })
    } else {
        Verdict::Unresolved(eps)
    }
}

// ---- The piecewise/side fold (the connected-frame extension) ------------------------------------

/// The exact chart lift `C(σ, μ̂, w)[i] = c_i(σ) + μ̂·r⃗_i(σ) + w·n_i(σ)` over the recovered
/// enclosures — a rational 3-D box, each field interval-evaluated over σ.
fn lift_box<B: Backend>(
    chart: &Chart<B>,
    sigma: &RatIv<B>,
    mu: &RatIv<B>,
    w: &Rat<B>,
) -> Result<[RatIv<B>; 3], FoldFault> {
    let eval = |f: &lattice::RatFunc<B>| eval_ratfunc_on(f, sigma);
    let mut point: [RatIv<B>; 3] = [
        RatIv::point(Rat::from_i128(0)),
        RatIv::point(Rat::from_i128(0)),
        RatIv::point(Rat::from_i128(0)),
    ];
    for (i, slot) in point.iter_mut().enumerate() {
        let (ci, ri, ni) = (
            eval(&chart.pedal().comp(i)),
            eval(&chart.ruling().comp(i)),
            eval(&chart.normal().comp(i)),
        );
        let (ci, ri, ni) = match (ci, ri, ni) {
            (Some(c), Some(r), Some(n)) => (c, r, n),
            _ => return Err(FoldFault::PoleInEval),
        };
        *slot = ci.add(&ri.mul(mu)).add(&ni.scale(w)).rounded();
    }
    Ok(point)
}

/// The signed area of the **running-frame directrix residual** at a rational σ within one glued
/// region: `cos ψ·(y − Γ_y(σ)) − sin ψ·(x − Γ_x(σ))` with `Γ(σ) = base + ∫_lo^σ γ′` — the region's
/// cumulative flat frame ([`PiecewiseDevelopment`]'s `point_from` shape). `None` on a γ pole
/// (propagated, unlike the single-panel [`cross_at`]'s γ≡0 fallback — a piecewise region's γ is
/// load-bearing).
fn cross_at_from<B: Backend>(
    dev: &ConeDevelopment<B>,
    base: &[RatIv<B>; 2],
    lo: &Rat<B>,
    s: &Rat<B>,
    x: &Rat<B>,
    y: &Rat<B>,
    cfg: &DevConfig<B>,
) -> Option<RatIv<B>> {
    let ang = dev.angle(s, cfg.terms);
    let c = cos_on(&ang, cfg.terms);
    let si = sin_on(&ang, cfg.terms);
    let g = dev.directrix_between(lo, s, cfg)?;
    let yg = RatIv::point(y.clone()).sub(&base[1]).sub(&g[1]);
    let xg = RatIv::point(x.clone()).sub(&base[0]).sub(&g[0]);
    Some(c.mul(&yg).sub(&si.mul(&xg)))
}

/// [`invert_sigma`] in a region's **running frame**: monotone bisection on the signed area of the
/// residual `(x, y) − Γ(σ)` over `domain` (one faithfulness piece — see [`faithful_pieces`]).
/// `flip` is the **side**: the piecewise development is always signed, so `µ̂ < 0` puts the
/// residual at `ψ + π` (the opposite bracketing convention) even where `γ ≡ 0` — unlike the
/// single-panel `|µ̂|` fold, the flip depends on the side alone.
#[allow(clippy::too_many_arguments)]
fn invert_sigma_from<B: Backend>(
    dev: &ConeDevelopment<B>,
    base: &[RatIv<B>; 2],
    lo_frame: &Rat<B>,
    x: &Rat<B>,
    y: &Rat<B>,
    domain: &Interval<B>,
    iters: usize,
    cfg: &DevConfig<B>,
    flip: bool,
) -> Result<RatIv<B>, FoldFault> {
    use core::cmp::Ordering;
    if domain.lo.cmp(&domain.hi) != Ordering::Less {
        return Err(FoldFault::DegenerateDomain);
    }
    let xat = |s: &Rat<B>| -> Result<RatIv<B>, FoldFault> {
        let c = cross_at_from(dev, base, lo_frame, s, x, y, cfg).ok_or(FoldFault::PoleInEval)?;
        Ok(if flip { c.neg() } else { c })
    };
    // Bracket: cross(σ_lo) ≥ 0 and cross(σ_hi) ≤ 0, else the angle is outside this piece.
    if xat(&domain.lo)?.hi().sign() < 0 || xat(&domain.hi)?.lo().sign() > 0 {
        return Err(FoldFault::OutOfGore);
    }
    let (mut lo, mut hi) = (domain.lo.clone(), domain.hi.clone());
    two_probe_bisect(&mut lo, &mut hi, iters, xat)?;
    Ok(RatIv::new(lo, hi))
}

/// Split a σ-domain into pieces on which the signed-area bisection is **faithful**: the sign of
/// `cross = |res|·sin(θ − ψ(σ))` tracks `sign(θ − ψ)` only while the piece's flat-angle span stays
/// below π. Splits at σ = 0 first (the arctan symmetry point — [`split_domain`]'s rule), then
/// bisects until each piece's certified ψ-span upper bound clears a rational lower bound of π.
/// This is what makes the fold **wrapping-safe**: on a wrapping chart (`c ≥ 2`, e.g. the
/// self-lapping `c = 260/97`) even a one-sided domain sweeps more than π, where the σ=0 split
/// alone would be unsound. Bounded depth; a piece still too wide at the cap is returned anyway —
/// a wrong bracket cannot *certify* (the round-trip ε is the certificate), only fail.
fn faithful_pieces<B: Backend>(
    dev: &ConeDevelopment<B>,
    domain: &Interval<B>,
    terms: usize,
) -> Vec<Interval<B>> {
    use core::cmp::Ordering;
    // 314159/100000 < π: a certified span-hi at or below it is strictly below π.
    let pi_lo = Rat::new(314_159, 100_000);
    let zero = Rat::from_i128(0);
    let mut queue: Vec<(Interval<B>, usize)> = Vec::new();
    if domain.lo.sign() < 0 && domain.hi.sign() > 0 {
        queue.push((
            Interval {
                lo: domain.lo.clone(),
                hi: zero.clone(),
            },
            0,
        ));
        queue.push((
            Interval {
                lo: zero,
                hi: domain.hi.clone(),
            },
            0,
        ));
    } else {
        queue.push((domain.clone(), 0));
    }
    let mut out = Vec::new();
    while let Some((piece, depth)) = queue.pop() {
        let span = dev
            .angle(&piece.hi, terms)
            .sub(&dev.angle(&piece.lo, terms));
        if span.hi().cmp(&pi_lo) != Ordering::Greater || depth >= 24 {
            out.push(piece);
        } else {
            let mid = piece.lo.add(&piece.hi).mul(&Rat::new(1, 2));
            queue.push((
                Interval {
                    lo: piece.lo,
                    hi: mid.clone(),
                },
                depth + 1,
            ));
            queue.push((
                Interval {
                    lo: mid,
                    hi: piece.hi,
                },
                depth + 1,
            ));
        }
    }
    out.sort_by(|a, b| a.lo.cmp(&b.lo));
    out
}

/// Fold a flat point back through a **piecewise development** (the connected glued frame):
/// invert the *signed* development `D = Γ(σ) + µ̂·ρ·e(ψ)` — `Γ` the running cumulative directrix —
/// to a certified `(σ, µ̂)` enclosure and lift it to a 3-D box on the owning region's chart.
///
/// Beyond the single-panel [`fold_point`]:
/// - **signed µ̂ throughout** (the [`PiecewiseDevelopment`] convention): with `mu_negative` the
///   residual sits at `ψ + π` even where `γ ≡ 0`, so the bisection flips on the side alone;
/// - **every region is tried in its own running frame**; a candidate must bracket inside one of
///   the region band's faithfulness pieces, and the smallest round-trip ε wins — sound however
///   the candidate was found, because the round-trip *is* the certificate;
/// - **wrapping-safe**: bands are split until each piece's certified ψ-span is below π
///   ([`faithful_pieces`]) — the σ=0 split alone is not enough on a wrapping chart (`c ≥ 2`).
///
/// `charts` are the per-region charts, parallel to the gluing's regions (the 3-D lift needs the
/// owning region's surface; refused as [`FoldFault::ChartMismatch`] if not parallel). Returns
/// `Verified` under the DRC `ε < clearance/2`, `Unresolved(ε)` to refine (`iters`, `cfg`), or
/// `Refuted` when no region develops to the point's direction (`OutOfGore`) or a field poles.
///
/// ```
/// use certify_core::Verdict;
/// use develop::cone::{ConeDevelopment, DevConfig};
/// use develop::fold::fold_point_pw;
/// use develop::part::{Development, PiecewiseDevelopment};
/// use fixtures::devices::cone;
/// use lattice::{Bignum, Interval, Rat};
///
/// let chart = cone();
/// let pw = PiecewiseDevelopment::new(vec![(
///     Interval { lo: Rat::<Bignum>::from_i128(0), hi: Rat::from_i128(1) },
///     ConeDevelopment::new(&chart).unwrap(),
/// )])
/// .unwrap();
/// let cfg = DevConfig::tight();
/// let (s, m) = (Rat::new(1, 2), Rat::from_i128(-1));
/// let (x, y) = Development::point(&pw, &s, &m, &cfg).unwrap().center();
/// let charts = [chart];
/// match fold_point_pw(&pw, &charts, &x, &y, &Rat::from_i128(0), 60, true, &cfg, &Rat::from_i128(1)) {
///     Verdict::Verified(f) => assert!(f.sigma.contains(&s) && f.mu.contains(&m)),
///     _ => panic!("the fold must certify"),
/// }
/// ```
#[allow(clippy::too_many_arguments)]
pub fn fold_point_pw<B: Backend>(
    pw: &PiecewiseDevelopment<B>,
    charts: &[Chart<B>],
    x: &Rat<B>,
    y: &Rat<B>,
    w: &Rat<B>,
    iters: usize,
    mu_negative: bool,
    cfg: &DevConfig<B>,
    clearance: &Rat<B>,
) -> Verdict<Fold3D<B>, FoldFault, Rat<B>> {
    use core::cmp::Ordering;
    if charts.len() != pw.regions().len() {
        return Verdict::Refuted(FoldFault::ChartMismatch);
    }
    let mut best: Option<Fold3D<B>> = None;
    for (k, (band, dev)) in pw.regions().iter().enumerate() {
        let base = match pw.cum_before(k, cfg) {
            Some(b) => b,
            None => return Verdict::Refuted(FoldFault::PoleInEval),
        };
        for piece in faithful_pieces(dev, band, cfg.terms) {
            let sigma = match invert_sigma_from(
                dev,
                &base,
                &band.lo,
                x,
                y,
                &piece,
                iters,
                cfg,
                mu_negative,
            ) {
                Ok(s) => s,
                Err(FoldFault::OutOfGore) => continue,
                Err(f) => return Verdict::Refuted(f),
            };
            // radius → |µ̂| = |res|/ρ, res the running-frame residual over the σ-enclosure.
            let g = match dev.directrix_between_on(&band.lo, &sigma, cfg) {
                Some(g) => g,
                None => return Verdict::Refuted(FoldFault::PoleInEval),
            };
            let xr = RatIv::point(x.clone()).sub(&base[0]).sub(&g[0]);
            let yr = RatIv::point(y.clone()).sub(&base[1]).sub(&g[1]);
            let r = sqrt_on(&xr.mul(&xr).add(&yr.mul(&yr)), &cfg.sqrt_eps);
            let inv_rho = match dev
                .radius_on(&sigma, &cfg.sqrt_eps)
                .and_then(|r| r.recip_pos())
            {
                Some(iv) => iv,
                None => return Verdict::Refuted(FoldFault::PoleInEval),
            };
            let abs_mu = r.mul(&inv_rho);
            let mu = if mu_negative { abs_mu.neg() } else { abs_mu };
            let point = match lift_box(&charts[k], &sigma, &mu, w) {
                Ok(p) => p,
                Err(f) => return Verdict::Refuted(f),
            };
            // Round-trip: re-develop through the region's running frame, measure the residual.
            let back = match dev.point_from_on(&base, &band.lo, &sigma, &mu, cfg) {
                Some(b) => b,
                None => return Verdict::Refuted(FoldFault::PoleInEval),
            };
            let (ex, ey) = (axis_residual(&back.x, x), axis_residual(&back.y, y));
            let eps = sqrt(&ex.mul(&ex).add(&ey.mul(&ey)), &cfg.sqrt_eps)
                .hi()
                .clone();
            if best
                .as_ref()
                .map(|b| eps.cmp(&b.eps) == Ordering::Less)
                .unwrap_or(true)
            {
                best = Some(Fold3D {
                    sigma,
                    mu,
                    point,
                    eps,
                    clearance: clearance.clone(),
                });
            }
        }
    }
    match best {
        None => Verdict::Refuted(FoldFault::OutOfGore),
        Some(f) => {
            let half = clearance.mul(&Rat::new(1, 2));
            if f.eps.cmp(&half) == Ordering::Less {
                Verdict::Verified(f)
            } else {
                Verdict::Unresolved(f.eps)
            }
        }
    }
}

/// Fold a whole flat loop back through a **piecewise development**: [`fold_point_pw`] every
/// vertex (each in whichever region's running frame brackets it) and collect the 3-D boxes into a
/// [`FoldedWire`]. Each vertex folds under a permissive clearance to read its raw round-trip ε
/// back; the uniform `ε = max` is gated once by the DRC `ε < clearance/2` — `Unresolved(ε)` to
/// refine, `Refuted` for an empty loop, a vertex outside the glued gore, or mismatched charts.
#[allow(clippy::too_many_arguments)]
pub fn fold_outline_pw<B: Backend>(
    pw: &PiecewiseDevelopment<B>,
    charts: &[Chart<B>],
    flat: &[[Rat<B>; 2]],
    w: &Rat<B>,
    iters: usize,
    mu_negative: bool,
    cfg: &DevConfig<B>,
    clearance: &Rat<B>,
) -> Verdict<FoldedWire<B>, FoldFault, Rat<B>> {
    use core::cmp::Ordering;
    if flat.is_empty() {
        return Verdict::Refuted(FoldFault::EmptyLoop);
    }
    let permissive = Rat::from_i128(1_000_000);
    let mut points: Vec<[RatIv<B>; 3]> = Vec::with_capacity(flat.len());
    let mut eps = Rat::from_i128(0);
    for p in flat {
        match fold_point_pw(
            pw,
            charts,
            &p[0],
            &p[1],
            w,
            iters,
            mu_negative,
            cfg,
            &permissive,
        ) {
            Verdict::Verified(f) => {
                if f.eps.cmp(&eps) == Ordering::Greater {
                    eps = f.eps;
                }
                points.push(f.point);
            }
            Verdict::Refuted(fault) => return Verdict::Refuted(fault),
            // Unreachable under the permissive clearance; propagate defensively (panic-free).
            Verdict::Unresolved(e) => return Verdict::Unresolved(e),
        }
    }
    let half = clearance.mul(&Rat::new(1, 2));
    if eps.cmp(&half) == Ordering::Less {
        Verdict::Verified(FoldedWire {
            points,
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

    fn ivl(lo: i128, hi: i128) -> Interval<Bignum> {
        Interval {
            lo: Q::from_i128(lo),
            hi: Q::from_i128(hi),
        }
    }
    fn to_f64(r: &Q) -> f64 {
        let (n, d) = r.numer_denom_decimal();
        n.parse::<f64>().unwrap() / d.parse::<f64>().unwrap()
    }
    /// The exact forward flat point D(σ, μ̂) of the device cone, as rational box centers.
    fn forward(sigma: Q, mu: Q) -> (Q, Q) {
        let dev = ConeDevelopment::new(&cone()).unwrap();
        let b = dev.point(&sigma, &mu, &DevConfig::tight());
        (b.x.mid(), b.y.mid())
    }

    /// Folding the forward image of a known (σ₀, μ₀) recovers σ₀ and μ₀ in the enclosures, lifts
    /// to the 3D point C(σ₀, μ₀, 0), and round-trips under a generous clearance.
    #[test]
    fn fold_recovers_a_known_chart_coordinate() {
        let (sigma0, mu0) = (Q::new(1, 2), Q::from_i128(-1));
        let (x, y) = forward(sigma0.clone(), mu0.clone());
        let v = fold_point(
            &cone(),
            &x,
            &y,
            &Q::from_i128(0),
            &ivl(0, 1),
            60,
            true,
            &DevConfig::tight(),
            &Q::from_i128(1),
        );
        match v {
            Verdict::Verified(f) => {
                assert!(
                    f.sigma.contains(&sigma0),
                    "σ enclosure must contain σ₀ = 1/2"
                );
                assert!(f.mu.contains(&mu0), "μ̂ enclosure must contain μ₀ = −1");
                // 3D lift: C(1/2, −1, 0) = −r⃗(1/2) (cone apex at origin, w = 0). Check |C| = r.
                let (cx, cy, cz) = (f.point[0].mid(), f.point[1].mid(), f.point[2].mid());
                let norm = (to_f64(&cx).powi(2) + to_f64(&cy).powi(2) + to_f64(&cz).powi(2)).sqrt();
                let r = (to_f64(&x).powi(2) + to_f64(&y).powi(2)).sqrt();
                assert!((norm - r).abs() < 1e-6, "apex distance preserved: |C| ≈ r");
            }
            _ => panic!("folding a forward image must certify"),
        }
    }

    /// The round-trip backward error shrinks as the σ-inversion refines (more bisection iters).
    #[test]
    fn roundtrip_epsilon_shrinks_with_iters() {
        let (x, y) = forward(Q::new(3, 4), Q::new(-3, 4));
        let eps_of = |it: usize| match fold_point(
            &cone(),
            &x,
            &y,
            &Q::from_i128(0),
            &ivl(0, 1),
            it,
            true,
            &DevConfig::tight(),
            &Q::from_i128(1000),
        ) {
            Verdict::Verified(f) => f.eps,
            Verdict::Unresolved(e) => e,
            Verdict::Refuted(_) => panic!("unexpected refutation"),
        };
        assert!(
            eps_of(64).cmp(&eps_of(8)) == core::cmp::Ordering::Less,
            "round-trip ε must shrink with bisection iters"
        );
    }

    /// A tight clearance leaves the fold Unresolved (refine); a generous one certifies.
    #[test]
    fn tight_clearance_is_unresolved() {
        let (x, y) = forward(Q::new(1, 2), Q::from_i128(-1));
        let e = match fold_point(
            &cone(),
            &x,
            &y,
            &Q::from_i128(0),
            &ivl(0, 1),
            40,
            true,
            &DevConfig::tight(),
            &Q::from_i128(1000),
        ) {
            Verdict::Verified(f) => f.eps,
            other => match other {
                Verdict::Unresolved(e) => e,
                _ => panic!(),
            },
        };
        let tight = e.div(&Q::from_i128(100));
        assert!(matches!(
            fold_point(
                &cone(),
                &x,
                &y,
                &Q::from_i128(0),
                &ivl(0, 1),
                40,
                true,
                &DevConfig::tight(),
                &tight,
            ),
            Verdict::Unresolved(_)
        ));
    }

    /// A flat point whose direction angle exceeds the gore's max ψ(σ_hi) is refused OutOfGore.
    #[test]
    fn a_point_outside_the_gore_is_refused() {
        // Develop at σ = 5 (well past the domain [0,1] whose max angle is ψ(1)), then try to
        // fold it back over [0,1] — its angle θ = ψ(5) > ψ(1), so no σ ∈ [0,1] reaches it.
        let (x, y) = forward(Q::from_i128(5), Q::from_i128(-1));
        assert!(matches!(
            fold_point(
                &cone(),
                &x,
                &y,
                &Q::from_i128(0),
                &ivl(0, 1),
                40,
                true,
                &DevConfig::tight(),
                &Q::from_i128(1),
            ),
            Verdict::Refuted(FoldFault::OutOfGore)
        ));
    }

    /// A degenerate σ-domain is refused before inversion.
    #[test]
    fn a_degenerate_domain_is_refuted() {
        let (x, y) = forward(Q::new(1, 2), Q::from_i128(-1));
        assert!(matches!(
            fold_point(
                &cone(),
                &x,
                &y,
                &Q::from_i128(0),
                &ivl(1, 1),
                40,
                true,
                &DevConfig::tight(),
                &Q::from_i128(1),
            ),
            Verdict::Refuted(FoldFault::DegenerateDomain)
        ));
    }

    /// The 3-D coordinates of two boxes agree to `< 1e-3` (f64 audit).
    fn boxes_close(b: &[RatIv<Bignum>; 3], c: &[Q; 3]) -> bool {
        (0..3).all(|i| (to_f64(&b[i].mid()) - to_f64(&c[i])).abs() < 1e-3)
    }

    /// The back-and-forth: unroll a band to a flat outline (direction ①), then fold it back
    /// (direction ②) recovers the original 3-D geometry — `develop ∘ fold ≈ identity`.
    #[test]
    fn roundtrip_unroll_then_fold() {
        use crate::unroll::{FlatOutline, unroll_freeboundary};
        use lattice::{Poly, RatFunc};

        let chart = cone();
        let dev = ConeDevelopment::new(&chart).unwrap();
        let ratf = |c: i128| RatFunc::<Bignum>::from_poly(Poly::constant(Q::from_i128(c)));
        let domain = ivl(0, 1);
        let outline: FlatOutline<Bignum> = match unroll_freeboundary(
            &dev,
            &domain,
            &ratf(-1),
            &ratf(-2),
            6,
            &DevConfig::tight(),
            &Q::from_i128(1000),
        ) {
            Verdict::Verified(o) => o,
            _ => panic!("the band must certify"),
        };
        let flat: Vec<[Q; 2]> = outline
            .vertices
            .iter()
            .map(|b| {
                let (x, y) = b.center();
                [x, y]
            })
            .collect();
        match fold_outline(
            &chart,
            &flat,
            &Q::from_i128(0),
            &domain,
            60,
            true,
            &DevConfig::tight(),
            &Q::from_i128(1),
        ) {
            Verdict::Verified(wire) => {
                assert_eq!(wire.points.len(), outline.vertices.len());
                // Vertex 0 is the μ̂ = −1 rail at σ = 0 → folds back to C(0, −1, 0).
                let orig = chart
                    .surface(&Q::from_i128(-1), &Q::from_i128(0))
                    .eval(&Q::from_i128(0))
                    .unwrap();
                assert!(
                    boxes_close(&wire.points[0], &orig),
                    "folded vertex 0 recovers the original 3-D point"
                );
            }
            _ => panic!("unroll then fold must certify"),
        }
    }

    /// The σ=0 split: a *wide* two-sided gore (ψ-span over [−3,3] ≈ 3.35 rad > π) folds correctly —
    /// vertices from σ = ±2 both recover their 3-D points. A single bisection over [−3,3] would be
    /// unsound (the signed-area sign is unfaithful past span π); the split rescues it.
    #[test]
    fn two_sided_fold_splits_at_zero() {
        let chart = cone();
        let pos = forward(Q::from_i128(2), Q::from_i128(-1)); // σ = +2 ⇒ y > 0
        let neg = forward(Q::from_i128(-2), Q::from_i128(-1)); // σ = −2 ⇒ y < 0
        let flat = [[pos.0, pos.1], [neg.0, neg.1]];
        match fold_outline(
            &chart,
            &flat,
            &Q::from_i128(0),
            &ivl(-3, 3),
            60,
            true,
            &DevConfig::tight(),
            &Q::from_i128(1),
        ) {
            Verdict::Verified(wire) => {
                let cp = chart
                    .surface(&Q::from_i128(-1), &Q::from_i128(0))
                    .eval(&Q::from_i128(2))
                    .unwrap();
                let cn = chart
                    .surface(&Q::from_i128(-1), &Q::from_i128(0))
                    .eval(&Q::from_i128(-2))
                    .unwrap();
                assert!(boxes_close(&wire.points[0], &cp), "σ = +2 vertex recovered");
                assert!(boxes_close(&wire.points[1], &cn), "σ = −2 vertex recovered");
            }
            _ => panic!("a wide two-sided fold must certify with the σ=0 split"),
        }
    }

    /// The wire's round-trip ε shrinks as the σ-inversion refines (more bisection iters).
    #[test]
    fn fold_outline_epsilon_shrinks_with_iters() {
        let chart = cone();
        let a = forward(Q::new(1, 2), Q::from_i128(-1));
        let b = forward(Q::new(3, 4), Q::from_i128(-2));
        let flat = [[a.0, a.1], [b.0, b.1]];
        let eps_of = |it: usize| match fold_outline(
            &chart,
            &flat,
            &Q::from_i128(0),
            &ivl(0, 1),
            it,
            true,
            &DevConfig::tight(),
            &Q::from_i128(1000),
        ) {
            Verdict::Verified(w) => w.eps,
            Verdict::Unresolved(e) => e,
            Verdict::Refuted(_) => panic!("unexpected refutation"),
        };
        assert!(
            eps_of(64).cmp(&eps_of(8)) == core::cmp::Ordering::Less,
            "wire ε must shrink with bisection iters"
        );
    }

    /// A vertex whose angle exceeds the gore's max ψ(σ_hi) fails the whole wire (OutOfGore).
    #[test]
    fn fold_outline_out_of_gore_is_refused() {
        let chart = cone();
        let p = forward(Q::from_i128(5), Q::from_i128(-1)); // ψ(5) > ψ(1)
        let flat = [[p.0, p.1]];
        assert!(matches!(
            fold_outline(
                &chart,
                &flat,
                &Q::from_i128(0),
                &ivl(0, 1),
                40,
                true,
                &DevConfig::tight(),
                &Q::from_i128(1),
            ),
            Verdict::Refuted(FoldFault::OutOfGore)
        ));
    }

    // ---- DD.3: the γ ≠ 0 fold (folding onto the seam-ramp flap) ----------------------------

    /// Folding a flat point on the **γ ≠ 0 seam-ramp flap** recovers its `(σ′, µ̂)` chart coordinate
    /// and round-trips within the DRC — the signed-µ̂ directrix-residual inversion `(x, y) − γ(σ)`.
    /// The device band is `µ̂ < 0`, so this exercises the residual-at-(ψ+π) `flip` in `invert_sigma`.
    #[test]
    fn fold_recovers_a_ramp_flap_coordinate() {
        use crate::cone::ConeDevelopment;
        let chart = fixtures::devices::cone_seam_ramp();
        let dev = ConeDevelopment::new_developable(&chart, 64).unwrap();
        let cfg = DevConfig::tight();
        let (s0, m0) = (Q::new(1, 4), Q::new(-3, 2)); // mid-flap, µ̂ < 0 (the device band side)
        let (x, y) = dev.point(&s0, &m0, &cfg).center();
        match fold_point(
            &chart,
            &x,
            &y,
            &Q::from_i128(0),
            &ivl(0, 1),
            40,
            true,
            &cfg,
            &Q::from_i128(1),
        ) {
            Verdict::Verified(f) => {
                assert!(
                    f.sigma.contains(&s0),
                    "σ′ enclosure must contain the flap σ′ = 1/4"
                );
                assert!(
                    f.mu.contains(&m0),
                    "signed µ̂ enclosure must contain µ̂ = −3/2"
                );
                assert!(
                    f.eps.cmp(&Q::new(1, 2)) == core::cmp::Ordering::Less,
                    "the round-trip backward error must clear the DRC (ε < clearance/2)"
                );
            }
            _ => panic!("the ramp-flap fold must certify"),
        }
    }

    /// A tight clearance leaves the ramp-flap fold `Unresolved` (fail-closed): the γ quadrature's
    /// floor exceeds `clearance/2`, so the certificate refuses rather than over-claim.
    #[test]
    fn a_ramp_fold_with_a_tight_clearance_is_unresolved() {
        use crate::cone::ConeDevelopment;
        let chart = fixtures::devices::cone_seam_ramp();
        let dev = ConeDevelopment::new_developable(&chart, 64).unwrap();
        let cfg = DevConfig::tight();
        let (x, y) = dev.point(&Q::new(1, 4), &Q::new(-3, 2), &cfg).center();
        assert!(matches!(
            fold_point(
                &chart,
                &x,
                &y,
                &Q::from_i128(0),
                &ivl(0, 1),
                40,
                true,
                &cfg,
                &Q::new(1, 100_000_000), // clearance/2 = 5e-9, far under the γ floor
            ),
            Verdict::Unresolved(_)
        ));
    }

    /// An empty loop is refused.
    #[test]
    fn empty_loop_is_refused() {
        let chart = cone();
        let flat: [[Q; 2]; 0] = [];
        assert!(matches!(
            fold_outline(
                &chart,
                &flat,
                &Q::from_i128(0),
                &ivl(0, 1),
                40,
                true,
                &DevConfig::tight(),
                &Q::from_i128(1),
            ),
            Verdict::Refuted(FoldFault::EmptyLoop)
        ));
    }

    // ---- The piecewise/side fold ------------------------------------------------------------

    use crate::part::Development;

    /// The device cone (γ≡0) on [0, 1/4] glued to the seam-ramp flap (γ≠0) on [1/4, 1/2] — the
    /// same two-region gluing as `part`'s tests — with its parallel charts.
    fn two_region_pw() -> (PiecewiseDevelopment<Bignum>, [Chart<Bignum>; 2]) {
        let body = cone();
        let ramp = fixtures::devices::cone_seam_ramp();
        let pw = PiecewiseDevelopment::new(vec![
            (
                Interval {
                    lo: Q::from_i128(0),
                    hi: Q::new(1, 4),
                },
                ConeDevelopment::new(&body).unwrap(),
            ),
            (
                Interval {
                    lo: Q::new(1, 4),
                    hi: Q::new(1, 2),
                },
                ConeDevelopment::new_developable(&ramp, 12).unwrap(),
            ),
        ])
        .unwrap();
        (pw, [body, ramp])
    }

    /// Folding forward images through the glued frame recovers the chart coordinates in **both**
    /// regions — the γ≡0 body in the signed convention, and the γ≠0 ramp in its running frame
    /// (base = the body's cumulative γ) — and lifts each onto its own region's surface.
    #[test]
    fn pw_fold_recovers_across_the_join() {
        let (pw, charts) = two_region_pw();
        // A fab-plausible budget: the γ quadrature runs once per bisection iter (the logged
        // γ-perf tech debt), so the tight default would spend most of the test integrating.
        let cfg = DevConfig {
            terms: 14,
            sqrt_eps: Q::new(1, 1_000_000_000),
        };
        for (s0, m0) in [
            (Q::new(1, 8), Q::from_i128(-1)),
            (Q::new(3, 8), Q::new(-3, 2)),
        ] {
            let (x, y) = Development::point(&pw, &s0, &m0, &cfg).unwrap().center();
            match fold_point_pw(
                &pw,
                &charts,
                &x,
                &y,
                &Q::from_i128(0),
                30,
                true,
                &cfg,
                &Q::from_i128(1),
            ) {
                Verdict::Verified(f) => {
                    assert!(
                        f.sigma.contains(&s0),
                        "σ enclosure must contain σ₀ = {s0:?}"
                    );
                    assert!(f.mu.contains(&m0), "µ̂ enclosure must contain µ̂₀");
                    // The lift lands on the owning region's surface.
                    let ri = if s0.cmp(&Q::new(1, 4)) == core::cmp::Ordering::Greater {
                        1
                    } else {
                        0
                    };
                    let orig = charts[ri].surface(&m0, &Q::from_i128(0)).eval(&s0).unwrap();
                    assert!(
                        boxes_close(&f.point, &orig),
                        "lifted 3-D box must recover the original surface point"
                    );
                }
                other => panic!(
                    "the glued fold must certify, got Unresolved/Refuted: {:?}",
                    {
                        match other {
                            Verdict::Unresolved(e) => format!("Unresolved({e:?})"),
                            Verdict::Refuted(f) => format!("Refuted({f:?})"),
                            _ => unreachable!(),
                        }
                    }
                ),
            }
        }
    }

    /// The **wrapping chart** (`c = 260/97 > 2`): even a one-sided σ-domain sweeps more than π, so
    /// the σ=0 split alone is unfaithful — the ψ-span pieces rescue it. Both far ends of the wide
    /// two-sided window `[−5/4, 5/4]` (ψ-span ≈ 275°) fold back to their surface points.
    #[test]
    fn pw_fold_handles_a_wrapping_chart() {
        let chart = fixtures::devices::cone_wrap();
        let pw = PiecewiseDevelopment::new(vec![(
            Interval {
                lo: Q::new(-5, 4),
                hi: Q::new(5, 4),
            },
            ConeDevelopment::new(&chart).unwrap(),
        )])
        .unwrap();
        let charts = [fixtures::devices::cone_wrap()];
        let cfg = DevConfig::tight();
        for s0 in [Q::from_i128(-1), Q::from_i128(1), Q::new(9, 8)] {
            let m0 = Q::from_i128(-1);
            let (x, y) = Development::point(&pw, &s0, &m0, &cfg).unwrap().center();
            match fold_point_pw(
                &pw,
                &charts,
                &x,
                &y,
                &Q::from_i128(0),
                60,
                true,
                &cfg,
                &Q::from_i128(1),
            ) {
                Verdict::Verified(f) => {
                    assert!(
                        f.sigma.contains(&s0),
                        "σ enclosure must contain σ₀ = {s0:?}"
                    );
                    let orig = chart.surface(&m0, &Q::from_i128(0)).eval(&s0).unwrap();
                    assert!(
                        boxes_close(&f.point, &orig),
                        "wrap lift recovers σ₀ = {s0:?}"
                    );
                }
                _ => panic!("the wrapping fold must certify at σ₀ = {s0:?}"),
            }
        }
    }

    /// The positive side: `mu_negative = false` folds a µ̂ > 0 signed point (no flip).
    #[test]
    fn pw_fold_positive_side() {
        let chart = fixtures::devices::cone_wrap();
        let pw =
            PiecewiseDevelopment::new(vec![(ivl(0, 1), ConeDevelopment::new(&chart).unwrap())])
                .unwrap();
        let charts = [fixtures::devices::cone_wrap()];
        let cfg = DevConfig::tight();
        let (s0, m0) = (Q::new(1, 2), Q::from_i128(2));
        let (x, y) = Development::point(&pw, &s0, &m0, &cfg).unwrap().center();
        match fold_point_pw(
            &pw,
            &charts,
            &x,
            &y,
            &Q::from_i128(0),
            60,
            false,
            &cfg,
            &Q::from_i128(1),
        ) {
            Verdict::Verified(f) => {
                assert!(f.sigma.contains(&s0) && f.mu.contains(&m0));
            }
            _ => panic!("the positive-side fold must certify"),
        }
    }

    /// A root sitting **on a probe fraction** of the bisection bracket still converges: the
    /// two-probe step shrinks past a straddling probe (a single-probe straddle-stop would return
    /// the whole domain as the σ-enclosure — the self-lapping tail-hole regression).
    #[test]
    fn a_root_on_the_probe_fraction_still_converges() {
        let chart = cone();
        let dev = ConeDevelopment::new(&chart).unwrap();
        // σ* at exactly 2/7 and 5/7 (the probe fractions) and 3/7 (the old single-probe split).
        for s0 in [Q::new(2, 7), Q::new(5, 7), Q::new(3, 7)] {
            let (x, y) = {
                let b = dev.point(&s0, &Q::from_i128(-1), &DevConfig::tight());
                (b.x.mid(), b.y.mid())
            };
            match fold_point(
                &chart,
                &x,
                &y,
                &Q::from_i128(0),
                &ivl(0, 1),
                60,
                true,
                &DevConfig::tight(),
                &Q::from_i128(1),
            ) {
                Verdict::Verified(f) => {
                    let width = f.sigma.hi().sub(f.sigma.lo());
                    assert!(
                        width.cmp(&Q::new(1, 10_000)) == core::cmp::Ordering::Less,
                        "σ-enclosure must converge past a straddling probe (σ* = {s0:?})"
                    );
                    assert!(f.sigma.contains(&s0));
                }
                _ => panic!("the probe-fraction fold must certify (σ* = {s0:?})"),
            }
        }
    }

    /// A point outside the glued gore is refused, as is a non-parallel chart slice.
    #[test]
    fn pw_fold_refuses_out_of_gore_and_chart_mismatch() {
        let (pw, charts) = two_region_pw();
        let cfg = DevConfig::tight();
        // Develop far outside the glued window [0, 1/2] on the body's own frame.
        let body = ConeDevelopment::new(&cone()).unwrap();
        let far = body.point_signed(&Q::from_i128(5), &Q::from_i128(-1), &cfg);
        let (x, y) = far.center();
        assert!(matches!(
            fold_point_pw(
                &pw,
                &charts,
                &x,
                &y,
                &Q::from_i128(0),
                40,
                true,
                &cfg,
                &Q::from_i128(1),
            ),
            Verdict::Refuted(FoldFault::OutOfGore)
        ));
        // One chart for two regions: refused before any work.
        assert!(matches!(
            fold_point_pw(
                &pw,
                &charts[..1],
                &x,
                &y,
                &Q::from_i128(0),
                40,
                true,
                &cfg,
                &Q::from_i128(1),
            ),
            Verdict::Refuted(FoldFault::ChartMismatch)
        ));
    }
}
