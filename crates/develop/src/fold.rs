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
    /// The chart slice handed to the piecewise fold is not parallel to the gluing's regions:
    /// wrong count, or a chart whose exact flat data (angle coefficient, ruling speed,
    /// directrix dot products) does not re-derive the paired region's development
    /// ([`fold_point_pw`] needs the owning region's chart for the 3-D lift).
    ChartMismatch,
    /// Two **disjoint** σ-enclosures both round-trip within the DRC: the flat point lies where
    /// the development overlaps itself (a gluing whose flat span exceeds 2π — the self-lap
    /// wedge), so the preimage is not unique and no certified choice exists. Author the feature
    /// outside the wedge, or fold through a narrower gluing.
    AmbiguousPreimage,
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
    // MAP.1: the seed only ever *narrows* the starting bracket — the bisection below still runs to
    // the same width target, so a bad or widened seed costs a little time and never accuracy.
    let seeded = match seed_sigma(dev, None, None, x, y, domain, cfg, flip) {
        Some(seed) => bracket_verified(|s| Ok(xat(s)), domain, &seed, iters, BRACKET_ATTEMPTS)?,
        None => None,
    };
    let (mut lo, mut hi) = match seeded {
        Some((a, b)) => {
            crate::counters::bump_bracket_seeded();
            (a, b)
        }
        None => {
            crate::counters::bump_bracket_bisected();
            (domain.lo.clone(), domain.hi.clone())
        }
    };
    two_probe_bisect(&mut lo, &mut hi, iters, &target_width(domain, iters), |s| {
        Ok(xat(s))
    })?;
    Ok(RatIv::new(lo, hi))
}

/// The bracket width `iters` of bisection would have reached: `width·(3/7)^(iters/2)`, i.e.
/// `width·2^-0.611·iters`. Used both as the seeded window's starting size and as the stopping
/// width, so ε is the same function of `iters` however the bracket was obtained.
fn target_width<B: Backend>(domain: &Interval<B>, iters: usize) -> Rat<B> {
    let shift = ((iters as u64 * 611) / 1000).clamp(1, 100) as u32;
    domain.hi.sub(&domain.lo).mul(&Rat::new(1, 1i128 << shift))
}

/// A certified bracket around the root, or `None` when the search should fall back.
type Bracket<B> = Option<(Rat<B>, Rat<B>)>;

/// How many geometric widenings [`bracket_verified`] tries before conceding to the bisection.
///
/// **Deliberately small.** Each attempt quadruples the window, so a seed that needs many widenings
/// yields a bracket the bisection must close anyway — paying for the widening *and* keeping the
/// work. Measured on the acceptance outline (40 vertices, one `fold` call): seed off 158.0 ms/pt,
/// `3` attempts **136.8 ms/pt** at a 69% hit rate and identical ε, `26` attempts *slower than not
/// trying at all* despite a 100% hit rate. Three buys the cheap hits and abandons the rest.
const BRACKET_ATTEMPTS: usize = 3;

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
    target: &Rat<B>,
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
        // Stop once the bracket is as tight as the budget was ever going to make it. With a
        // seeded start (MAP.1) this is usually true immediately, which is where the saving comes
        // from; without one it fires exactly when the plain bisection would have finished.
        if w.cmp(target) != core::cmp::Ordering::Greater {
            return Ok(());
        }
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

// ─────────────────────────────────────────────────────────────────────────────────────────────
// MAP.1 — the search is not the certificate.
//
// `two_probe_bisect` spends ~`iters` enclosure evaluations locating σ. But nothing downstream
// depends on *how* σ was found: `fold_point` certifies by re-developing the recovered (σ, µ̂) and
// measuring the residual to the authored point. So the search may be replaced by anything that
// proposes a σ, provided the proposal is turned into a **certified bracket** before use.
//
// That is the split below: `seed_sigma` proposes (uncertified, floating-point — the repo's
// float-search-then-certify doctrine, one level down from its use in 3-D placement), and
// `bracket_verified` proves the proposal brackets the root with two enclosure evaluations. A
// failed proposal costs nothing: the caller falls back to the bisection, unchanged.
//
// The seam is deliberately a **proposed σ**, not a solver object. A fitted embedding map (MAP.2)
// proposes σ exactly as the float solve does, so it substitutes here with no retrofit.
// ─────────────────────────────────────────────────────────────────────────────────────────────

/// A rational as `f64`, for the **search only** — never for a certificate. `None` when the value
/// does not survive the conversion (huge numerator/denominator overflowing to infinity), which
/// simply costs the caller its fast path.
fn to_f64<B: Backend>(r: &Rat<B>) -> Option<f64> {
    let (n, d) = r.numer_denom_decimal();
    let (n, d) = (n.parse::<f64>().ok()?, d.parse::<f64>().ok()?);
    let v = n / d;
    if v.is_finite() && d != 0.0 {
        Some(v)
    } else {
        None
    }
}

/// An `f64` snapped onto the `2^-50` dyadic grid as an exact rational. `None` if out of range.
///
/// The grid must be **finer than the bracket the seed is searched in**, or the snap's own error
/// puts the root outside the initial window and every attempt widens past it. `2^-50` sits below
/// `f64`'s own resolution at these magnitudes, so the snap costs nothing the estimate had.
fn from_f64<B: Backend>(v: f64) -> Option<Rat<B>> {
    const SCALE: i128 = 1 << 50;
    if !v.is_finite() || v.abs() > 1e9 {
        return None;
    }
    let n = (v * SCALE as f64).round();
    if !n.is_finite() || n.abs() >= i128::MAX as f64 {
        return None;
    }
    Some(Rat::new(n as i128, SCALE))
}

/// Propose the σ that develops to `(x, y)` — **uncertified**, the search half of MAP.1.
///
/// The development sends σ to the ray at angle `ψ(σ) = c·arctan σ` offset by the directrix, so
/// inverting the angle is closed-form: `σ = tan(ψ/c)`. With `γ ≠ 0` the residual `(x,y) − base − γ(σ)`
/// depends on σ, so one fixed-point correction is taken using γ at the first estimate — γ is the
/// small ramp offset, so this converges immediately in practice, and any error is absorbed by the
/// bracket widening rather than trusted.
///
/// `base`/`lo_frame` carry the piecewise running frame (`None` for the single-region form). `flip`
/// marks the `γ ≠ 0, µ̂ < 0` case where the residual points at `ψ + π`.
///
/// Returns `None` whenever the estimate cannot be formed — the caller then bisects as before.
#[allow(clippy::too_many_arguments)]
fn seed_sigma<B: Backend>(
    dev: &ConeDevelopment<B>,
    base: Option<&[RatIv<B>; 2]>,
    lo_frame: Option<&Rat<B>>,
    x: &Rat<B>,
    y: &Rat<B>,
    domain: &Interval<B>,
    cfg: &DevConfig<B>,
    flip: bool,
) -> Option<Rat<B>> {
    use core::f64::consts::PI;
    let c = to_f64(dev.angle_coeff())?;
    if c.abs() < 1e-12 {
        return None;
    }
    let (xf, yf) = (to_f64(x)?, to_f64(y)?);
    let (s_lo, s_hi) = (to_f64(&domain.lo)?, to_f64(&domain.hi)?);
    let (bx, by) = match base {
        Some(b) => (to_f64(&b[0].mid())?, to_f64(&b[1].mid())?),
        None => (0.0, 0.0),
    };
    // ψ is only recoverable modulo 2π from an `atan2`, and a wrapping chart (`c ≥ 2`) reaches past
    // one turn — so the estimate is unwrapped into the domain's own ψ-range, which is narrower
    // than 2π for any region and therefore picks the branch uniquely.
    let (psi_lo, psi_hi) = (c * s_lo.atan(), c * s_hi.atan());
    let (mut gx, mut gy) = (0.0f64, 0.0f64);
    let mut sigma = None;
    for _ in 0..2 {
        let theta = (yf - by - gy).atan2(xf - bx - gx);
        let mut psi = if flip { theta - PI } else { theta };
        while psi < psi_lo - 1e-9 {
            psi += 2.0 * PI;
        }
        while psi > psi_hi + 1e-9 {
            psi -= 2.0 * PI;
        }
        if psi < psi_lo - 1e-9 || psi > psi_hi + 1e-9 {
            return None; // outside this piece's angular range — let the bisection say so
        }
        let s = (psi / c).tan().clamp(s_lo, s_hi);
        sigma = Some(s);
        if !dev.has_directrix() {
            break;
        }
        // Re-read γ at the estimate and correct once. `directrix_*` is memoized (OPT.1), so this
        // costs a partial cell rather than a full quadrature.
        let sr = from_f64::<B>(s)?;
        let sr = clamp_rat(&sr, domain);
        let g = match lo_frame {
            Some(lo) => dev.directrix_between(lo, &sr, cfg)?,
            None => dev.directrix_at(&sr, cfg)?,
        };
        gx = to_f64(&g[0].mid())?;
        gy = to_f64(&g[1].mid())?;
    }
    let s = from_f64::<B>(sigma?)?;
    Some(clamp_rat(&s, domain))
}

fn clamp_rat<B: Backend>(s: &Rat<B>, domain: &Interval<B>) -> Rat<B> {
    use core::cmp::Ordering::{Greater, Less};
    if s.cmp(&domain.lo) == Less {
        domain.lo.clone()
    } else if s.cmp(&domain.hi) == Greater {
        domain.hi.clone()
    } else {
        s.clone()
    }
}

/// Turn an uncertified proposal into a **certified bracket**: the narrowest window around `seed`
/// whose endpoints carry *definite, opposite* signed-area signs, so the root provably lies inside.
///
/// This is the certificate half of MAP.1, and it is what makes the search disposable. Two
/// enclosure evaluations per attempt, against ~`iters` for the bisection. The window widens
/// geometrically when the signs are not yet definite — which happens exactly when an endpoint sits
/// within enclosure resolution of the root, and widening is the correct response. `Ok(None)` means
/// every attempt failed and the caller should bisect; correctness never depends on success.
fn bracket_verified<B: Backend>(
    xat: impl Fn(&Rat<B>) -> Result<RatIv<B>, FoldFault>,
    domain: &Interval<B>,
    seed: &Rat<B>,
    iters: usize,
    attempts: usize,
) -> Result<Bracket<B>, FoldFault> {
    use core::cmp::Ordering::{Greater, Less};
    // `iters` stays the caller's accuracy dial. The bisection reaches a bracket of roughly
    // `width·(3/7)^(iters/2)`, i.e. `width·2^-0.611·iters`, so the window starts there: same ε for
    // the same budget, at two enclosure evaluations instead of `iters`. (Ignoring `iters` here
    // would silently remove the dial — ε would stop responding to it.)
    let shift = ((iters as u64 * 611) / 1000).clamp(1, 100) as u32;
    let mut half = domain.hi.sub(&domain.lo).mul(&Rat::new(1, 1i128 << shift));
    for _ in 0..attempts {
        let a = clamp_rat(&seed.sub(&half), domain);
        let b = clamp_rat(&seed.add(&half), domain);
        if a.cmp(&b) != Less {
            half = half.mul(&Rat::from_i128(4));
            continue;
        }
        // Decreasing signed area: definite `≥ 0` at the left end and `≤ 0` at the right end
        // brackets the root. A straddling enclosure is *not* definite and does not count —
        // *except* at a domain endpoint, where the caller's gore precondition already established
        // the same one-sided fact and the enclosure straddles precisely because the root sits
        // within enclosure resolution of the boundary (a vertex on a region seam). Accepting the
        // endpoint there is no weaker than the bisection, which starts from exactly that bracket
        // under exactly that precondition — and in both cases the *certificate* is the downstream
        // round-trip residual, not this bracket.
        let ok_lo = a.cmp(&domain.lo) == core::cmp::Ordering::Equal || xat(&a)?.lo().sign() >= 0;
        let ok_hi = b.cmp(&domain.hi) == core::cmp::Ordering::Equal || xat(&b)?.hi().sign() <= 0;
        if ok_lo && ok_hi {
            return Ok(Some((a, b)));
        }
        half = half.mul(&Rat::from_i128(4));
        if half.cmp(&domain.hi.sub(&domain.lo)) == Greater {
            break;
        }
    }
    Ok(None)
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
    // MAP.1, running-frame form — seeded against the same residual `(x, y) − base − γ(σ)` this
    // piece's signed area uses. As above, the seed narrows; the bisection still sets the width.
    let seeded = match seed_sigma(dev, Some(base), Some(lo_frame), x, y, domain, cfg, flip) {
        Some(seed) => bracket_verified(xat, domain, &seed, iters, BRACKET_ATTEMPTS)?,
        None => None,
    };
    let (mut lo, mut hi) = match seeded {
        Some((a, b)) => {
            crate::counters::bump_bracket_seeded();
            (a, b)
        }
        None => {
            crate::counters::bump_bracket_bisected();
            (domain.lo.clone(), domain.hi.clone())
        }
    };
    two_probe_bisect(&mut lo, &mut hi, iters, &target_width(domain, iters), xat)?;
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
/// owning region's surface; refused as [`FoldFault::ChartMismatch`] unless each chart
/// re-derives its region's exact flat data — count alone is not trusted). Returns `Verified`
/// under the DRC `ε < clearance/2`, `Unresolved(ε)` to refine (`iters`, `cfg`), or `Refuted`
/// when no region develops to the point's direction (`OutOfGore`), a field poles, or two
/// σ-disjoint preimages both pass the DRC ([`FoldFault::AmbiguousPreimage`] — the lap wedge of
/// a gluing whose flat span exceeds 2π, where the development is genuinely 2-to-1).
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
    if !charts_paired(pw, charts) {
        return Verdict::Refuted(FoldFault::ChartMismatch);
    }
    match fold_point_pw_at(pw, charts, x, y, w, iters, mu_negative, cfg, clearance) {
        Err(f) => Verdict::Refuted(f),
        Ok(f) => {
            let half = clearance.mul(&Rat::new(1, 2));
            if f.eps.cmp(&half) == Ordering::Less {
                Verdict::Verified(f)
            } else {
                Verdict::Unresolved(f.eps)
            }
        }
    }
}

/// The [`fold_point_pw`] pairing guard: one chart per region, each re-deriving the region's
/// exact flat data ([`ConeDevelopment::derives_from`]) — checked once per call, hoisted out of
/// [`fold_outline_pw`]'s per-vertex loop.
fn charts_paired<B: Backend>(pw: &PiecewiseDevelopment<B>, charts: &[Chart<B>]) -> bool {
    charts.len() == pw.regions().len()
        && pw
            .regions()
            .iter()
            .zip(charts)
            .all(|((_, dev), chart)| dev.derives_from(chart))
}

/// The unguarded piecewise fold: the best (min-ε) candidate across every region ×
/// faithfulness piece, with its raw round-trip ε — the DRC gate is the caller's. Errs
/// [`FoldFault::AmbiguousPreimage`] when two candidates with **disjoint** σ-enclosures both
/// round-trip inside the DRC (`ε < clearance/2`): on a gluing whose flat span exceeds 2π the
/// development is genuinely 2-to-1 in the lap wedge, and a min-ε pick between two certified
/// preimages would be arbitrary. Touching or overlapping enclosures are one root seen from
/// adjacent pieces, never ambiguous.
#[allow(clippy::too_many_arguments)]
fn fold_point_pw_at<B: Backend>(
    pw: &PiecewiseDevelopment<B>,
    charts: &[Chart<B>],
    x: &Rat<B>,
    y: &Rat<B>,
    w: &Rat<B>,
    iters: usize,
    mu_negative: bool,
    cfg: &DevConfig<B>,
    clearance: &Rat<B>,
) -> Result<Fold3D<B>, FoldFault> {
    use core::cmp::Ordering;
    let half = clearance.mul(&Rat::new(1, 2));
    let mut best: Option<Fold3D<B>> = None;
    let mut admissible: Vec<RatIv<B>> = Vec::new();
    for (k, (band, dev)) in pw.regions().iter().enumerate() {
        let base = match pw.cum_before(k, cfg) {
            Some(b) => b,
            None => return Err(FoldFault::PoleInEval),
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
                Err(f) => return Err(f),
            };
            // radius → |µ̂| = |res|/ρ, res the running-frame residual over the σ-enclosure.
            let g = match dev.directrix_between_on(&band.lo, &sigma, cfg) {
                Some(g) => g,
                None => return Err(FoldFault::PoleInEval),
            };
            let xr = RatIv::point(x.clone()).sub(&base[0]).sub(&g[0]);
            let yr = RatIv::point(y.clone()).sub(&base[1]).sub(&g[1]);
            let r = sqrt_on(&xr.mul(&xr).add(&yr.mul(&yr)), &cfg.sqrt_eps);
            let inv_rho = match dev
                .radius_on(&sigma, &cfg.sqrt_eps)
                .and_then(|r| r.recip_pos())
            {
                Some(iv) => iv,
                None => return Err(FoldFault::PoleInEval),
            };
            let abs_mu = r.mul(&inv_rho);
            let mu = if mu_negative { abs_mu.neg() } else { abs_mu };
            let point = lift_box(&charts[k], &sigma, &mu, w)?;
            // Round-trip: re-develop through the region's running frame, measure the residual.
            let back = match dev.point_from_on(&base, &band.lo, &sigma, &mu, cfg) {
                Some(b) => b,
                None => return Err(FoldFault::PoleInEval),
            };
            let (ex, ey) = (axis_residual(&back.x, x), axis_residual(&back.y, y));
            let eps = sqrt(&ex.mul(&ex).add(&ey.mul(&ey)), &cfg.sqrt_eps)
                .hi()
                .clone();
            if eps.cmp(&half) == Ordering::Less {
                admissible.push(sigma.clone());
            }
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
    // Two DRC-passing preimages separated by a σ-gap: the point sits in the lap wedge, and a
    // min-ε pick between genuine preimages would be arbitrary — refuse.
    for i in 0..admissible.len() {
        for j in i + 1..admissible.len() {
            let (a, b) = (&admissible[i], &admissible[j]);
            if a.hi().cmp(b.lo()) == Ordering::Less || b.hi().cmp(a.lo()) == Ordering::Less {
                return Err(FoldFault::AmbiguousPreimage);
            }
        }
    }
    best.ok_or(FoldFault::OutOfGore)
}

/// Fold a whole flat loop back through a **piecewise development**: fold every vertex (each in
/// whichever region's running frame brackets it) and collect the 3-D boxes into a
/// [`FoldedWire`]. Each vertex's raw round-trip ε is read back and the uniform `ε = max` is
/// gated once by the DRC `ε < clearance/2` — `Unresolved(ε)` to refine, `Refuted` for an empty
/// loop, a vertex outside the glued gore, mismatched charts, or a vertex in the lap wedge
/// ([`FoldFault::AmbiguousPreimage`], checked per vertex against this same clearance).
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
    if !charts_paired(pw, charts) {
        return Verdict::Refuted(FoldFault::ChartMismatch);
    }
    if flat.is_empty() {
        return Verdict::Refuted(FoldFault::EmptyLoop);
    }
    let mut points: Vec<[RatIv<B>; 3]> = Vec::with_capacity(flat.len());
    let mut eps = Rat::from_i128(0);
    for p in flat {
        match fold_point_pw_at(
            pw,
            charts,
            &p[0],
            &p[1],
            w,
            iters,
            mu_negative,
            cfg,
            clearance,
        ) {
            Ok(f) => {
                if f.eps.cmp(&eps) == Ordering::Greater {
                    eps = f.eps;
                }
                points.push(f.point);
            }
            Err(fault) => return Verdict::Refuted(fault),
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

    /// On a gluing whose flat span exceeds 2π (`[−3, 3]` on the wrapping chart: ≈ 384°) the
    /// development overlaps itself: a flat point in the lap wedge has **two** genuine σ-preimages
    /// (σ = 12/5 develops to ψ ≈ +180.6°, and a second root near σ ≈ −2.35 develops to the same
    /// direction at ψ − 360°). Both round-trip exactly, so no certified choice exists — the fold
    /// must refuse rather than pick by ε. Off the wedge the same gluing still certifies.
    #[test]
    fn a_lap_wedge_point_is_refused_as_ambiguous() {
        let chart = fixtures::devices::cone_wrap();
        let pw = PiecewiseDevelopment::new(vec![(
            Interval {
                lo: Q::from_i128(-3),
                hi: Q::from_i128(3),
            },
            ConeDevelopment::new(&chart).unwrap(),
        )])
        .unwrap();
        let charts = [fixtures::devices::cone_wrap()];
        let cfg = DevConfig::tight();
        let m0 = Q::from_i128(-1);
        let (x, y) = Development::point(&pw, &Q::new(12, 5), &m0, &cfg)
            .unwrap()
            .center();
        assert!(matches!(
            fold_point_pw(
                &pw,
                &charts,
                &x,
                &y,
                &Q::from_i128(0),
                60,
                true,
                &cfg,
                &Q::from_i128(1),
            ),
            Verdict::Refuted(FoldFault::AmbiguousPreimage)
        ));
        // Off the wedge (ψ(1) ≈ 120.6° is single-covered): the unique preimage certifies.
        let s0 = Q::from_i128(1);
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
            Verdict::Verified(f) => assert!(f.sigma.contains(&s0)),
            _ => panic!("off-wedge points on the wide gluing must still certify"),
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
        // Same length, wrong pairing (the two charts swapped): the derived-data guard refuses
        // it even at a point where the flat inversion alone would certify — the round-trip
        // certificate never sees the lift, so the pairing must be checked, not trusted.
        let (gx, gy) = Development::point(&pw, &Q::new(1, 8), &Q::from_i128(-1), &cfg)
            .unwrap()
            .center();
        let swapped = [fixtures::devices::cone_seam_ramp(), cone()];
        assert!(matches!(
            fold_point_pw(
                &pw,
                &swapped,
                &gx,
                &gy,
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
