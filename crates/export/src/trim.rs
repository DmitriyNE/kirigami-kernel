//! **xy → (σ,μ) trim bridge** (G-B): author trimming cylinders in the cone's *physical*
//! xy-plane and develop the trimmed panel.
//!
//! The cone is trimmed by an arrangement of vertical cylinders (disks in xy). Each disk is a
//! [`CutSurface`] — an axis-centered parallel becomes an *exact* [`CutSurface::Plane`]
//! `{z = d}`, an eccentric one a [`CutSurface::Cylinder`] — pulled back to a ruling-rail
//! `μ̂(σ)` by the G2 machinery: the float oracle [`fit_cut_rail`](crate::cut_oracle::fit_cut_rail)
//! **proposes**, the exact [`cut_fit`] certificate **decides** (`ε < clearance/2`, fail-closed).
//!
//! Each disk carries both its 3-D cut surface (for the rail) and its exact circular footprint
//! `(cx, cy, r²)` (the [`arrange2d`](arrange2d) operand) — the two are the same cylinder, so the
//! physical-xy arrangement and the developed rails stay consistent within the certified `ε`.
//!
//! The panel lives on the `μ > 0` side of the ruling (the apex `μ̂ = 0` is excised by the inner
//! disk), so physical-xy is the natural `(x, y)`. For the device cone the gore develops the
//! **upper** half-plane — `σ = 0` maps to azimuth 90° (`+y`), `σ = ±1` to 0°/180° — so trimming
//! disks are authored around `+y`, and a disk `(cx, cy, R)` is exactly what the user draws.

use certify_core::Verdict;
use develop::cone::{ConeDevelopment, DevConfig};
use develop::cut::{CutFitCert, CutFitFault, CutSurface, cut_fit, plane_cut_rail};
use develop::unroll::{BoundaryArc, FlatOutline, UnrollFault, unroll_trim_loop};
use geom::chart::Chart;
use lattice::{Backend, Bignum, Interval, Poly, Rat, RatFunc, Vec3Rat};

use crate::cut_oracle::{RootPick, fit_cut_rail};

/// The `i`-th standard basis vector as a constant [`Vec3Rat`] (for picking a chart-field component
/// by dotting with it).
fn basis3<B: Backend>(i: usize) -> Vec3Rat<B> {
    let z = || Poly::constant(Rat::from_i128(0));
    let mut c = [z(), z(), z()];
    c[i] = Poly::constant(Rat::from_i128(1));
    Vec3Rat::new(c, Poly::constant(Rat::from_i128(1)))
}

/// The physical-xy components `(r_x(σ), r_y(σ))` of the ruling — the direction of the apex ray,
/// since `C(σ, μ) = μ·ruling(σ)` at `w = 0` on the apex-centred cone.
fn ruling_xy<B: Backend>(chart: &Chart<B>) -> (RatFunc<B>, RatFunc<B>) {
    (
        chart.ruling().dot(&basis3(0)),
        chart.ruling().dot(&basis3(1)),
    )
}

/// `g(σ) = (cx·r_y − cy·r_x)² − R²·(r_x² + r_y²)` — the (rational) tangency residual of the apex
/// ray to the disk `(cx, cy, R)`: **negative** where the ray crosses the disk (two real cut
/// branches), **zero** at the two tangent rulings, **positive** outside.
fn tangent_poly<B: Backend>(chart: &Chart<B>, cx: &Rat<B>, cy: &Rat<B>, r2: &Rat<B>) -> RatFunc<B> {
    let (rx, ry) = ruling_xy(chart);
    let konst = |r: &Rat<B>| RatFunc::from_poly(Poly::constant(r.clone()));
    let cross = ry.mul(&konst(cx)).sub(&rx.mul(&konst(cy))); // cx·r_y − cy·r_x
    let norm2 = rx.mul(&rx).add(&ry.mul(&ry)); // r_x² + r_y²
    cross.mul(&cross).sub(&konst(r2).mul(&norm2))
}

/// Bisect `f` for a sign change on `[lo, hi]` (needs `f(lo)·f(hi) < 0`), returning a rational in
/// the final bracket. `None` if no sign change or a pole is hit.
fn bisect_root<B: Backend>(
    f: &RatFunc<B>,
    lo: &Rat<B>,
    hi: &Rat<B>,
    iters: usize,
) -> Option<Rat<B>> {
    let half = Rat::new(1, 2);
    let mut a = lo.clone();
    let mut b = hi.clone();
    let sa = f.eval(&a)?.sign();
    if sa == 0 {
        return Some(a);
    }
    if f.eval(&b)?.sign() == sa {
        return None;
    }
    for _ in 0..iters {
        let m = a.add(&b).mul(&half);
        let sm = f.eval(&m)?.sign();
        if sm == 0 {
            return Some(m);
        }
        if sm == sa {
            a = m; // sign at `a` unchanged
        } else {
            b = m;
        }
    }
    Some(a.add(&b).mul(&half))
}

/// All sign-change roots of `f` on `[lo, hi]`, found by scanning `scan` sub-intervals and
/// bisecting each bracket. `None` if `f` has a pole at a scan node.
fn scan_roots<B: Backend>(
    f: &RatFunc<B>,
    lo: &Rat<B>,
    hi: &Rat<B>,
    scan: usize,
    iters: usize,
) -> Option<Vec<Rat<B>>> {
    let n = scan.max(4);
    let width = hi.sub(lo).div(&Rat::from_i128(n as i128));
    let mut prev: Option<(Rat<B>, i8)> = None;
    let mut roots: Vec<Rat<B>> = Vec::new();
    for k in 0..=n {
        let x = lo.add(&width.mul(&Rat::from_i128(k as i128)));
        let s = f.eval(&x)?.sign();
        if let Some((px, ps)) = &prev {
            if *ps != 0 && s != 0 && *ps != s {
                roots.push(bisect_root(f, px, &x, iters)?);
            }
        }
        prev = Some((x, s));
    }
    Some(roots)
}

/// The σ where the (exact) D1 rail point enters/exits a cylinder `(cx, cy, r²)` — the roots of
/// `h(σ) = (μ̂₁·r_x − cx)² + (μ̂₁·r_y − cy)² − r²`, which is exact-rational in σ. Lets the D1∩D3
/// crossings be found without ever fitting D3 (so its near branch is then fitted only over the
/// mid-span crossing range, clear of the √-branch tangents).
#[allow(clippy::too_many_arguments)]
fn rail_cylinder_crossings<B: Backend>(
    chart: &Chart<B>,
    mu1: &RatFunc<B>,
    cx: &Rat<B>,
    cy: &Rat<B>,
    r2: &Rat<B>,
    span: &Interval<B>,
    scan: usize,
    iters: usize,
) -> Option<Vec<Rat<B>>> {
    let (rx, ry) = ruling_xy(chart);
    let konst = |r: &Rat<B>| RatFunc::from_poly(Poly::constant(r.clone()));
    let px = mu1.mul(&rx).sub(&konst(cx));
    let py = mu1.mul(&ry).sub(&konst(cy));
    let h = px.mul(&px).add(&py.mul(&py)).sub(&konst(r2));
    scan_roots(&h, &span.lo, &span.hi, scan, iters)
}

/// The two tangent-ruling σ of a disk within the gore, as `(σ_lo, σ_hi)`. Scans [`tangent_poly`]
/// for its two sign changes (outside `+` → inside `−` → outside `+`). `None` unless the disk
/// subtends exactly one clean two-tangent arc in the gore.
fn disk_tangents<B: Backend>(
    chart: &Chart<B>,
    cx: &Rat<B>,
    cy: &Rat<B>,
    r2: &Rat<B>,
    span: &Interval<B>,
    scan: usize,
    iters: usize,
) -> Option<(Rat<B>, Rat<B>)> {
    let g = tangent_poly(chart, cx, cy, r2);
    let roots = scan_roots(&g, &span.lo, &span.hi, scan, iters)?;
    if roots.len() == 2 {
        Some((roots[0].clone(), roots[1].clone()))
    } else {
        None
    }
}

/// Clone a [`CutSurface`] without a `B: Clone` bound (its own `Clone` is derived, hence bounded;
/// the `Rat<B>` fields clone for any [`Backend`]). Local mirror of `content.rs`'s manual clones.
fn clone_surface<B: Backend>(s: &CutSurface<B>) -> CutSurface<B> {
    match s {
        CutSurface::Plane { n, d } => CutSurface::Plane {
            n: [n[0].clone(), n[1].clone(), n[2].clone()],
            d: d.clone(),
        },
        CutSurface::Cylinder {
            axis_point,
            axis_dir,
            r2,
        } => CutSurface::Cylinder {
            axis_point: [
                axis_point[0].clone(),
                axis_point[1].clone(),
                axis_point[2].clone(),
            ],
            axis_dir: [
                axis_dir[0].clone(),
                axis_dir[1].clone(),
                axis_dir[2].clone(),
            ],
            r2: r2.clone(),
        },
    }
}

/// Knobs for fitting + certifying a cut rail: the polynomial fit degree, the σ-subdivision the
/// certified `sup` distance is taken over (the ε refinement handle), and the dyadic snap bits.
#[derive(Clone, Copy)]
pub struct RailFit {
    /// The degree of the fitted rail polynomial (ignored for an exact plane rail).
    pub degree: usize,
    /// The number of equal σ-sub-intervals the certified distance bound is maximized over.
    pub subdiv: usize,
    /// The `2^bits` dyadic grid the oracle snaps fitted coefficients to.
    pub bits: u32,
}

impl Default for RailFit {
    fn default() -> Self {
        // The certified ε is an interval bound whose refinement handle is `subdiv` (the G2
        // finding), not fit degree. A plane rail is exact and ignores `degree`.
        RailFit {
            degree: 6,
            subdiv: 128,
            bits: 44,
        }
    }
}

/// A trimming cylinder authored in the cone's physical xy-plane: its 3-D cut [`surface`] (used
/// to pull it back to a rail), its exact circular footprint `(cx, cy, r²)` (the arrangement
/// operand), and the [`RootPick`] branch of the cone∩surface cut to trace.
///
/// [`surface`]: TrimDisk::surface
#[derive(Clone)]
pub struct TrimDisk<B: Backend = Bignum> {
    /// The 3-D cutting surface (a plane for the concentric case, a cylinder otherwise).
    pub surface: CutSurface<B>,
    /// Footprint centre x.
    pub cx: Rat<B>,
    /// Footprint centre y.
    pub cy: Rat<B>,
    /// Footprint squared radius `R²`.
    pub r2: Rat<B>,
    /// Which branch of the cone∩surface cut this disk's rail traces.
    pub pick: RootPick,
}

/// An eccentric vertical cylinder of radius `√r2` about `(cx, cy)`. Footprint = the circle
/// `(cx, cy, r²)`; `pick` selects the cut branch (`Upper` = larger-μ root).
pub fn eccentric_disk<B: Backend>(
    cx: Rat<B>,
    cy: Rat<B>,
    r2: Rat<B>,
    pick: RootPick,
) -> TrimDisk<B> {
    TrimDisk {
        surface: CutSurface::Cylinder {
            axis_point: [cx.clone(), cy.clone(), Rat::from_i128(0)],
            axis_dir: [Rat::from_i128(0), Rat::from_i128(0), Rat::from_i128(1)],
            r2: r2.clone(),
        },
        cx,
        cy,
        r2,
        pick,
    }
}

/// The concentric parallel `{z = d}` (apex-centred), whose rail is the **exact** plane rail
/// (`ε ≈ 0`). Its footprint `r²` is read off the chart at `σ = 0` (constant along a parallel).
/// `None` if the plane rail has a pole at `σ = 0` (a ruling parallel to `z`, impossible for a
/// canonical cone).
pub fn concentric_disk<B: Backend>(chart: &Chart<B>, d: &Rat<B>) -> Option<TrimDisk<B>> {
    let n = [Rat::from_i128(0), Rat::from_i128(0), Rat::from_i128(1)];
    let rail = plane_cut_rail(chart, &n, d);
    let zero = Rat::from_i128(0);
    let mu0 = rail.eval(&zero)?;
    let p = chart.surface(&mu0, &zero).eval(&zero)?;
    let r2 = p[0].mul(&p[0]).add(&p[1].mul(&p[1]));
    Some(TrimDisk {
        surface: CutSurface::Plane { n, d: d.clone() },
        cx: zero.clone(),
        cy: zero,
        r2,
        pick: RootPick::Upper,
    })
}

/// Fit **and** certify the ruling-rail `μ̂(σ)` for a trim disk over `span`: the float oracle
/// proposes, [`cut_fit`] decides. On success returns `(μ̂, ε)` with the certified distance
/// bound; a loose fit / declined oracle is `Unresolved` (fail-closed), a degenerate surface or
/// in-span pole is `Refuted`.
pub fn certified_rail<B: Backend>(
    chart: &Chart<B>,
    disk: &TrimDisk<B>,
    span: &Interval<B>,
    fit: RailFit,
    clearance: &Rat<B>,
    cfg: &DevConfig<B>,
) -> Verdict<(RatFunc<B>, Rat<B>), CutFitFault, Rat<B>> {
    let mu_hat = match fit_cut_rail(chart, &disk.surface, span, fit.degree, disk.pick, fit.bits) {
        Some(m) => m,
        // The oracle declined (cut not real at a node / singular solve) — fail-closed.
        None => return Verdict::Unresolved(clearance.clone()),
    };
    let cert = CutFitCert {
        mu_hat: mu_hat.clone(),
        w: Rat::from_i128(0),
        surface: clone_surface(&disk.surface),
        span: span.clone(),
        subdiv: fit.subdiv,
        clearance: clearance.clone(),
        cfg: cfg.clone(),
    };
    match cut_fit(chart, &cert) {
        Verdict::Verified(v) => Verdict::Verified((mu_hat, v.eps)),
        Verdict::Unresolved(e) => Verdict::Unresolved(e),
        Verdict::Refuted(f) => Verdict::Refuted(f),
    }
}

/// The outer boundary loop of an **eccentric annulus band**: the inner rail `μ̂_in` and outer
/// rail `μ̂_out` (both spanning the full gore `[σ_lo, σ_hi]`) joined by the two σ-caps. The cap
/// endpoints are the rails *evaluated* at the cap σ, so the loop chains exactly in `(σ, μ̂)`.
/// `None` if either rail has a pole at a cap σ.
pub fn annulus_loop<B: Backend>(
    mu_in: &RatFunc<B>,
    mu_out: &RatFunc<B>,
    span: &Interval<B>,
    segments: usize,
) -> Option<Vec<BoundaryArc<B>>> {
    let (lo, hi) = (&span.lo, &span.hi);
    let in_lo = mu_in.eval(lo)?;
    let in_hi = mu_in.eval(hi)?;
    let out_lo = mu_out.eval(lo)?;
    let out_hi = mu_out.eval(hi)?;
    Some(vec![
        BoundaryArc::Cap {
            sigma: lo.clone(),
            mu_start: in_lo.clone(),
            mu_end: out_lo,
        },
        BoundaryArc::Rail {
            mu: mu_out.clone(),
            sigma_start: lo.clone(),
            sigma_end: hi.clone(),
            segments,
        },
        BoundaryArc::Cap {
            sigma: hi.clone(),
            mu_start: out_hi,
            mu_end: in_hi,
        },
        BoundaryArc::Rail {
            mu: mu_in.clone(),
            sigma_start: hi.clone(),
            sigma_end: lo.clone(),
            segments,
        },
    ])
}

/// `|a − b|`.
fn abs_diff<B: Backend>(a: &Rat<B>, b: &Rat<B>) -> Rat<B> {
    let d = a.sub(b);
    if d.sign() < 0 {
        Rat::from_i128(0).sub(&d)
    } else {
        d
    }
}

/// A developed interior hole: its boundary loop (near + far cut branches joined by tangent
/// micro-caps) and the certified rail ε.
pub struct HoleLoop<B: Backend = Bignum> {
    /// The ordered boundary arcs of the hole.
    pub arcs: Vec<BoundaryArc<B>>,
    /// The larger of the two branch rails' certified distance bounds.
    pub eps: Rat<B>,
    /// The larger tangent micro-cap length (in μ̂ units) — the residual of snapping the algebraic
    /// tangent σ to a rational just inside the disk. A diagnostic on the fail-closed treatment.
    pub max_microcap: Rat<B>,
}

/// Build the interior-hole boundary loop for the disk `(cx, cy, r²)`: its **near** (Lower) and
/// **far** (Upper) cut branches over the disk's σ-extent, joined at each tangent ruling by a
/// **micro-cap** — a radial segment bridging the small μ̂ gap left by snapping the algebraic
/// tangent σ (a root of [`tangent_poly`]) to a rational `margin` inside the disk. The inset keeps
/// the polynomial fit clear of the √-branch point *and* makes the loop chain exactly in `(σ, μ̂)`.
/// Fail-closed: a loose branch fit is `Unresolved`, a degenerate cut `Refuted`.
#[allow(clippy::too_many_arguments)]
pub fn hole_loop<B: Backend>(
    chart: &Chart<B>,
    cx: &Rat<B>,
    cy: &Rat<B>,
    r2: &Rat<B>,
    span: &Interval<B>,
    fit: RailFit,
    clearance: &Rat<B>,
    cfg: &DevConfig<B>,
    margin: &Rat<B>,
    segments: usize,
) -> Verdict<HoleLoop<B>, CutFitFault, Rat<B>> {
    let (t_lo, t_hi) = match disk_tangents(chart, cx, cy, r2, span, 256, 60) {
        Some(t) => t,
        None => return Verdict::Unresolved(clearance.clone()),
    };
    let inset = t_hi.sub(&t_lo).mul(margin);
    let s1 = t_lo.add(&inset);
    let s2 = t_hi.sub(&inset);
    let sub = Interval {
        lo: s1.clone(),
        hi: s2.clone(),
    };
    let far = eccentric_disk(cx.clone(), cy.clone(), r2.clone(), RootPick::Upper);
    let near = eccentric_disk(cx.clone(), cy.clone(), r2.clone(), RootPick::Lower);
    let (mu_far, e_far) = match certified_rail(chart, &far, &sub, fit, clearance, cfg) {
        Verdict::Verified(x) => x,
        Verdict::Unresolved(e) => return Verdict::Unresolved(e),
        Verdict::Refuted(f) => return Verdict::Refuted(f),
    };
    let (mu_near, e_near) = match certified_rail(chart, &near, &sub, fit, clearance, cfg) {
        Verdict::Verified(x) => x,
        Verdict::Unresolved(e) => return Verdict::Unresolved(e),
        Verdict::Refuted(f) => return Verdict::Refuted(f),
    };
    let (f1, n1, f2, n2) = match (
        mu_far.eval(&s1),
        mu_near.eval(&s1),
        mu_far.eval(&s2),
        mu_near.eval(&s2),
    ) {
        (Some(a), Some(b), Some(c), Some(d)) => (a, b, c, d),
        _ => return Verdict::Refuted(CutFitFault::PoleInEval),
    };
    let max_microcap = {
        let (lo, hi) = (abs_diff(&f1, &n1), abs_diff(&f2, &n2));
        if lo.cmp(&hi) == core::cmp::Ordering::Less {
            hi
        } else {
            lo
        }
    };
    // far (s1→s2) · micro-cap @ s2 (far→near) · near (s2→s1) · micro-cap @ s1 (near→far).
    let arcs = vec![
        BoundaryArc::Rail {
            mu: mu_far,
            sigma_start: s1.clone(),
            sigma_end: s2.clone(),
            segments,
        },
        BoundaryArc::Cap {
            sigma: s2.clone(),
            mu_start: f2,
            mu_end: n2,
        },
        BoundaryArc::Rail {
            mu: mu_near,
            sigma_start: s2,
            sigma_end: s1.clone(),
            segments,
        },
        BoundaryArc::Cap {
            sigma: s1,
            mu_start: n1,
            mu_end: f1,
        },
    ];
    let eps = if e_far.cmp(&e_near) == core::cmp::Ordering::Less {
        e_near
    } else {
        e_far
    };
    Verdict::Verified(HoleLoop {
        arcs,
        eps,
        max_microcap,
    })
}

/// A developed panel outer boundary: the notched annulus loop, the max rail ε, and the largest
/// micro-cap (the D1∩D3-crossing snap residual).
pub struct OuterLoop<B: Backend = Bignum> {
    /// The ordered boundary arcs (CCW-ish traversal: cap · D1 · notch · D1 · cap · D2-back).
    pub arcs: Vec<BoundaryArc<B>>,
    /// Max over the D1/D2/D3 rails of the certified distance bound.
    pub eps: Rat<B>,
    /// The larger D1↔D3 crossing micro-cap (μ̂ units).
    pub max_microcap: Rat<B>,
}

/// Build the panel's **outer boundary** loop: the eccentric annulus band (inner rail from `d2`,
/// outer rail from `d1`) with the outer rail **notched** where `d3` bites across it. The two
/// D1↔D3 transition σ are located as the D1∩D3 crossings via [`rail_cylinder_crossings`] of the
/// D1 rail against the D3 cylinder (no D3 fit); the D3 near/Lower branch is fitted over that range
/// (clear of the √-branch tangents), then each crossing is **refined** to where the *fitted* D3
/// meets D1 (a bisection within `± d3_margin ×` the crossing range) so the D1↔D3 corner is a clean
/// join (the micro-cap collapses to the bisection residual). `d1` is the concentric outer (exact
/// plane rail), `d2` the eccentric inner (Upper branch). Fail-closed: a loose rail is `Unresolved`.
#[allow(clippy::too_many_arguments)]
pub fn outer_loop<B: Backend>(
    chart: &Chart<B>,
    d1: &TrimDisk<B>,
    d2: &TrimDisk<B>,
    d3: (&Rat<B>, &Rat<B>, &Rat<B>),
    span: &Interval<B>,
    fit: RailFit,
    clearance: &Rat<B>,
    cfg: &DevConfig<B>,
    d3_margin: &Rat<B>,
    segments: usize,
) -> Verdict<OuterLoop<B>, CutFitFault, Rat<B>> {
    macro_rules! rail {
        ($disk:expr, $sp:expr, $f:expr) => {
            match certified_rail(chart, $disk, $sp, $f, clearance, cfg) {
                Verdict::Verified(x) => x,
                Verdict::Unresolved(e) => return Verdict::Unresolved(e),
                Verdict::Refuted(fault) => return Verdict::Refuted(fault),
            }
        };
    }
    let (lo, hi) = (&span.lo, &span.hi);
    let (mu_d1, e1) = rail!(d1, span, fit);
    let (mu_d2, e2) = rail!(d2, span, fit);

    // Locate the notch: the D1∩D3 crossings computed EXACTLY from the D1 rail ∩ D3 cylinder (no
    // D3 fit). They sit mid-span, clear of D3's √-branch tangents, so the near branch is fitted
    // only there and certifies tightly.
    let cross0 = match rail_cylinder_crossings(chart, &mu_d1, d3.0, d3.1, d3.2, span, 256, 60) {
        Some(r) if r.len() == 2 => r,
        _ => return Verdict::Unresolved(clearance.clone()),
    };
    let d3_span = Interval {
        lo: cross0[0].clone(),
        hi: cross0[1].clone(),
    };
    let d3_disk = eccentric_disk(d3.0.clone(), d3.1.clone(), d3.2.clone(), RootPick::Lower);
    // The notch range is narrow and far from the σ-origin, where the oracle's monomial-basis
    // Vandermonde fit is ill-conditioned — degree ≥ 4 gives huge coefficients whose interval
    // evaluation explodes (ε ~ 100s). A low degree (the dip is gentle) certifies tightly.
    let d3_fit = RailFit { degree: 3, ..fit };
    let (mu_d3, e3) = rail!(&d3_disk, &d3_span, d3_fit);
    // Refine each crossing to where the FITTED D3 meets D1 exactly (`μ̂_D3fit − μ̂_D1 = 0`), so the
    // D1↔D3 corner is a clean join — the micro-cap collapses to the bisection residual (invisible)
    // instead of the degree-3 fit residual (a visible step). Fail-safe: if the fit does not bracket
    // a root in the window (`d3_margin` × the crossing range), fall back to the geometric crossing.
    let dmu = mu_d3.sub(&mu_d1);
    let w = cross0[1].sub(&cross0[0]).mul(d3_margin);
    let snl = bisect_root(&dmu, &cross0[0].sub(&w), &cross0[0].add(&w), 60)
        .unwrap_or_else(|| cross0[0].clone());
    let snr = bisect_root(&dmu, &cross0[1].sub(&w), &cross0[1].add(&w), 60)
        .unwrap_or_else(|| cross0[1].clone());

    // Evaluate every needed rail value; any pole ⇒ Refuted.
    let vals: Option<[Rat<B>; 8]> = (|| {
        Some([
            mu_d2.eval(lo)?,
            mu_d1.eval(lo)?,
            mu_d1.eval(&snl)?,
            mu_d3.eval(&snl)?,
            mu_d3.eval(&snr)?,
            mu_d1.eval(&snr)?,
            mu_d1.eval(hi)?,
            mu_d2.eval(hi)?,
        ])
    })();
    let [d2_lo, d1_lo, d1_l, d3_l, d3_r, d1_r, d1_hi, d2_hi] = match vals {
        Some(v) => v,
        None => return Verdict::Refuted(CutFitFault::PoleInEval),
    };
    let max_microcap = {
        let (l, r) = (abs_diff(&d1_l, &d3_l), abs_diff(&d1_r, &d3_r));
        if l.cmp(&r) == core::cmp::Ordering::Less {
            r
        } else {
            l
        }
    };

    let arcs = vec![
        BoundaryArc::Cap {
            sigma: lo.clone(),
            mu_start: d2_lo,
            mu_end: d1_lo,
        },
        BoundaryArc::Rail {
            mu: mu_d1.clone(),
            sigma_start: lo.clone(),
            sigma_end: snl.clone(),
            segments,
        },
        BoundaryArc::Cap {
            sigma: snl.clone(),
            mu_start: d1_l,
            mu_end: d3_l,
        },
        BoundaryArc::Rail {
            mu: mu_d3,
            sigma_start: snl,
            sigma_end: snr.clone(),
            segments,
        },
        BoundaryArc::Cap {
            sigma: snr.clone(),
            mu_start: d3_r,
            mu_end: d1_r,
        },
        BoundaryArc::Rail {
            mu: mu_d1,
            sigma_start: snr,
            sigma_end: hi.clone(),
            segments,
        },
        BoundaryArc::Cap {
            sigma: hi.clone(),
            mu_start: d1_hi,
            mu_end: d2_hi,
        },
        BoundaryArc::Rail {
            mu: mu_d2,
            sigma_start: hi.clone(),
            sigma_end: lo.clone(),
            segments,
        },
    ];
    let eps = [e1, e2, e3]
        .into_iter()
        .max_by(|a, b| a.cmp(b))
        .unwrap_or_else(|| Rat::from_i128(0));
    Verdict::Verified(OuterLoop {
        arcs,
        eps,
        max_microcap,
    })
}

/// Develop a trim loop to a certified flat outline (thin wrapper over [`unroll_trim_loop`]).
pub fn unroll_loop<B: Backend>(
    dev: &ConeDevelopment<B>,
    arcs: &[BoundaryArc<B>],
    cfg: &DevConfig<B>,
    clearance: &Rat<B>,
) -> Verdict<FlatOutline<B>, UnrollFault, Rat<B>> {
    unroll_trim_loop(dev, arcs, cfg, clearance)
}

/// A developed loop's rational polygon: each [`FlatOutline`] vertex reduced to its `FlatBox`
/// centre, with **exactly-coincident consecutive vertices dropped** (float-free). A micro-cap
/// whose μ̂ gap falls below the development's rounding precision develops to two identical
/// rational points — a zero-length edge the `arrange2d` boolean would reject as degenerate; the
/// dedup removes it. The wrap-around pair (last == first) is dropped too.
pub fn flat_to_poly<B: Backend>(outline: &FlatOutline<B>) -> Vec<[Rat<B>; 2]> {
    use core::cmp::Ordering::Equal;
    let same =
        |a: &[Rat<B>; 2], b: &[Rat<B>; 2]| a[0].cmp(&b[0]) == Equal && a[1].cmp(&b[1]) == Equal;
    let mut out: Vec<[Rat<B>; 2]> = Vec::with_capacity(outline.vertices.len());
    for v in &outline.vertices {
        let (x, y) = v.center();
        let p = [x, y];
        if out.last().is_none_or(|q| !same(q, &p)) {
            out.push(p);
        }
    }
    if out.len() > 1 && same(&out[0], &out[out.len() - 1]) {
        out.pop();
    }
    out
}

/// One closed polygon `pts` as exact [`arrange2d`] segment edges tagged `src` (mirrors
/// `develop::flat`'s `seg_edge`: the directed line through each consecutive pair).
fn poly_edges<B: Backend>(pts: &[[Rat<B>; 2]], src: u32) -> Vec<geom::content::Edge<B>> {
    use geom::content::{CurveId, Edge, Line, Orient, Point2, SegPiece};
    let n = pts.len();
    (0..n)
        .map(|i| {
            let s = &pts[i];
            let e = &pts[(i + 1) % n];
            let a = e[1].sub(&s[1]).neg();
            let b = e[0].sub(&s[0]);
            let c = a.mul(&s[0]).add(&b.mul(&s[1])).neg();
            Edge::Seg(Box::new(SegPiece {
                line: Line { a, b, c },
                start: Point2::from_rat(s[0].clone(), s[1].clone()),
                end: Point2::from_rat(e[0].clone(), e[1].clone()),
                orient: Orient::Ccw,
                source: CurveId(src),
            }))
        })
        .collect()
}

/// Assemble the final flat panel `Region` = `outer − ⋃ holes`, via the `BoolOp::Diff` arrangement
/// (operand A = the outer polygon, operand B = the union of the interior hole polygons — authored
/// pairwise-disjoint). Certified by `ledge_dom_certified`. This is [`crate::cut_oracle`]'s
/// downstream: the developed rails become flat polygons the exact 2-D boolean stitches together.
pub fn assemble_flat<B: Backend>(
    outer: &[[Rat<B>; 2]],
    holes: &[Vec<[Rat<B>; 2]>],
) -> Verdict<arrange2d::boolean::Region<B>, arrange2d::boolean::CapOutFault, ()> {
    use arrange2d::boolean::{BoolOp, OperandId, ledge_dom_certified};
    use geom::content::CurveId;
    let mut edges = poly_edges(outer, 0);
    for (i, h) in holes.iter().enumerate() {
        edges.extend(poly_edges(h, (i + 1) as u32));
    }
    let operand_of = |c: CurveId| {
        if c.0 == 0 { OperandId::A } else { OperandId::B }
    };
    match ledge_dom_certified(&edges, &operand_of, BoolOp::Diff) {
        Verdict::Verified(cap) => {
            let (region, _v, _pinch) = cap.into_parts();
            Verdict::Verified(region)
        }
        Verdict::Refuted(f) => Verdict::Refuted(f),
        Verdict::Unresolved(()) => Verdict::Unresolved(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::approx::rat_to_f64;
    use fixtures::devices::cone;

    type Q = Rat<Bignum>;

    fn f(r: &Q) -> f64 {
        rat_to_f64(r)
    }

    /// The eccentric annulus band (concentric outer D1 + eccentric inner D2) develops to a
    /// certified flat outline. Tuned once from `probe_cone_scale`.
    #[test]
    fn eccentric_annulus_unrolls() {
        let chart = cone();
        let dev = ConeDevelopment::new(&chart).unwrap();
        let cfg = DevConfig::tight();
        // A ~180° two-sided gore (σ = ±1 ⇒ ±90° azimuth), crossing σ = 0. Two conditions keep
        // the certified development tight: (1) the panel is band-scaled — a *cut* (circular)
        // boundary is a varying-μ̂ rail, so a large radius means a large μ̂ and loose interval
        // slop everywhere; the constant-μ band is exempt (μ̂ ≈ 2); (2) a moderate gore, since the
        // full ~300° blows μ̂ ∝ (1+σ²) up at the edges (μ̂ ~ 300 while ρ ~ 0) and interval
        // arithmetic can't cancel the huge×tiny product. Both are genuine wide-gore/large-radius
        // cut-rail strains (logged). ("mm" is nominal — the shape/eccentricity/apex-containment
        // are scale-invariant, so the demo uses band-scale units.)
        let clearance = Q::from_i128(1);
        let span = Interval {
            lo: Q::from_i128(-1),
            hi: Q::from_i128(1),
        };
        let fit = RailFit::default();

        // Outer D1: concentric parallel {z = 3} — exact plane rail, footprint R ≈ 2.7.
        let d1 = concentric_disk(&chart, &Q::from_i128(3)).unwrap();
        let (mu_d1, eps1) = match certified_rail(&chart, &d1, &span, fit, &clearance, &cfg) {
            Verdict::Verified(x) => x,
            other => panic!("D1 rail not certified: {}", tag(&other)),
        };
        println!("D1 (plane):    ε = {:.3e}", f(&eps1));

        // Inner D2: eccentric cylinder R = √2 ≈ 1.41 at (0, 1/2) — contains the apex (0.5 < 1.41),
        // fits inside D1 (0.5 + 1.41 < 2.7). +y-oriented (the gore centre). Upper (μ>0) branch.
        let d2 = eccentric_disk(
            Q::from_i128(0),
            Q::new(1, 2),
            Q::from_i128(2),
            RootPick::Upper,
        );
        let (mu_d2, eps2) = match certified_rail(&chart, &d2, &span, fit, &clearance, &cfg) {
            Verdict::Verified(x) => x,
            other => panic!("D2 rail not certified: {}", tag(&other)),
        };
        println!("D2 (cylinder): ε = {:.3e}", f(&eps2));

        // Sanity: over the gore, inner μ̂_D2 < outer μ̂_D1 and both > 0 (band, no apex).
        for s in [-3, -1, 0, 1, 3] {
            let s = Q::new(s, 2);
            let (a, b) = (mu_d2.eval(&s).unwrap(), mu_d1.eval(&s).unwrap());
            assert!(
                a.sign() > 0 && a.cmp(&b) == core::cmp::Ordering::Less,
                "0 < μ_D2 < μ_D1 at σ={:.3}: {:.4} vs {:.4}",
                f(&s),
                f(&a),
                f(&b)
            );
        }

        let arcs = annulus_loop(&mu_d2, &mu_d1, &span, 48).unwrap();
        match unroll_loop(&dev, &arcs, &cfg, &clearance) {
            Verdict::Verified(o) => {
                println!(
                    "annulus unroll: Verified  ε = {:.3e}  ({} flat verts)",
                    f(&o.eps),
                    o.vertices.len()
                );
            }
            other => panic!("annulus unroll not Verified: {}", tag(&other)),
        }
    }

    /// An interior circular hole (D4) develops to a certified loop: near + far cut branches
    /// joined by small tangent micro-caps, unrolling to a Verified flat wire.
    #[test]
    fn interior_hole_unrolls() {
        let chart = cone();
        let dev = ConeDevelopment::new(&chart).unwrap();
        let cfg = DevConfig::tight();
        let clearance = Q::from_i128(1);
        let span = Interval {
            lo: Q::from_i128(-1),
            hi: Q::from_i128(1),
        };
        let fit = RailFit::default();
        // The gore develops the UPPER half-plane: σ = 0 ⇒ azimuth 90° (+y), σ = ±1 ⇒ 0°/180°.
        // So disks live around +y. D4: a small disk R = 0.2 at (0, 2.2) — inside the annulus (D2
        // exits near r ≈ 1.9, D1 at r ≈ 2.71), apex well outside it.
        let (cx, cy, r2) = (Q::from_i128(0), Q::new(11, 5), Q::new(1, 25));
        let margin = Q::new(1, 200);

        let (tlo, thi) =
            disk_tangents(&chart, &cx, &cy, &r2, &span, 256, 60).expect("two tangents");
        println!("D4 tangents: σ ∈ [{:.4}, {:.4}]", f(&tlo), f(&thi));
        assert!(tlo.cmp(&thi) == core::cmp::Ordering::Less);

        let hole = match hole_loop(
            &chart, &cx, &cy, &r2, &span, fit, &clearance, &cfg, &margin, 32,
        ) {
            Verdict::Verified(h) => h,
            other => panic!("hole_loop not Verified: {}", tag(&other)),
        };
        println!(
            "D4 hole: rail ε = {:.3e}, max micro-cap = {:.3e} (μ̂ units, hole is ≈0.4 tall)",
            f(&hole.eps),
            f(&hole.max_microcap)
        );
        // The tangent micro-cap is the √-branch residual (the developed circle's two tangent
        // points are slightly flattened) — an exact Cap, watertight, small vs the hole height.
        assert!(
            hole.max_microcap.cmp(&Q::new(1, 10)) == core::cmp::Ordering::Less,
            "tangent micro-cap should stay small, got {:.4}",
            f(&hole.max_microcap)
        );
        match unroll_loop(&dev, &hole.arcs, &cfg, &clearance) {
            Verdict::Verified(o) => println!(
                "D4 hole unroll: Verified  ε = {:.3e}  ({} flat verts)",
                f(&o.eps),
                o.vertices.len()
            ),
            other => panic!("hole unroll not Verified: {}", tag(&other)),
        }
    }

    /// The full demo disk footprints, in the +y upper-half gore: D1 concentric outer, D2
    /// eccentric inner (contains apex), D3 boundary notch (straddles D1), D4 interior hole.
    fn demo_disks(chart: &Chart<Bignum>) -> (TrimDisk<Bignum>, TrimDisk<Bignum>, [Q; 3], [Q; 3]) {
        let d1 = concentric_disk(chart, &Q::from_i128(3)).unwrap();
        let d2 = eccentric_disk(
            Q::from_i128(0),
            Q::new(1, 2),
            Q::from_i128(2),
            RootPick::Upper,
        );
        // D3 straddles D1 deeply (centre |c|≈3.18 > D1≈2.71, R=0.75) so the D1∩D3 crossings land
        // mid-span, well clear of the √-branch tangents (a shallow straddle puts them on the
        // branch points and the near-branch fit goes loose).
        let d3 = [Q::new(-9, 4), Q::new(9, 4), Q::new(9, 16)];
        let d4 = [Q::from_i128(0), Q::new(11, 5), Q::new(1, 25)];
        (d1, d2, d3, d4)
    }

    /// The panel outer boundary (eccentric annulus + D3 boundary notch) develops to a Verified
    /// flat outline; the D1↔D3 crossing micro-caps stay small (transverse, not tangent).
    #[test]
    fn notched_outer_unrolls() {
        let chart = cone();
        let dev = ConeDevelopment::new(&chart).unwrap();
        let cfg = DevConfig::tight();
        let clearance = Q::from_i128(1);
        let span = Interval {
            lo: Q::from_i128(-1),
            hi: Q::from_i128(1),
        };
        let (d1, d2, d3, _d4) = demo_disks(&chart);
        let outer = match outer_loop(
            &chart,
            &d1,
            &d2,
            (&d3[0], &d3[1], &d3[2]),
            &span,
            RailFit::default(),
            &clearance,
            &cfg,
            &Q::new(1, 20),
            48,
        ) {
            Verdict::Verified(o) => o,
            other => panic!("outer_loop not Verified: {}", tag(&other)),
        };
        println!(
            "outer (notched): rail ε = {:.3e}, D1∩D3 micro-cap = {:.3e}",
            f(&outer.eps),
            f(&outer.max_microcap)
        );
        // Refining each crossing to where the FITTED D3 meets D1 collapses the micro-cap to the
        // bisection residual — the D1↔D3 corner is a clean join, not a visible step.
        assert!(
            outer.max_microcap.cmp(&Q::new(1, 1_000_000_000)) == core::cmp::Ordering::Less,
            "D1∩D3 crossing micro-cap should be ~0 (clean join), got {:.2e}",
            f(&outer.max_microcap)
        );
        match unroll_loop(&dev, &outer.arcs, &cfg, &clearance) {
            Verdict::Verified(o) => println!(
                "outer unroll: Verified  ε = {:.3e}  ({} flat verts)",
                f(&o.eps),
                o.vertices.len()
            ),
            other => panic!("outer unroll not Verified: {}", tag(&other)),
        }
    }

    /// The physical-xy arrangement `(D1 − D2) − D3 − D4` via the new `BoolOp::Diff` certifies to
    /// exactly the intended topology: **one face** (the notched annulus) with **two holes** (the
    /// eccentric inner D2 and the interior D4), and D3 contributes a boundary notch (no hole).
    #[test]
    fn arrangement_certifies_topology() {
        use arrange2d::boolean::{BoolOp, OperandId, ledge_dom_certified};
        use geom::content::{Circle, CurveId, Orient};
        let chart = cone();
        let (d1, _d2, d3, d4) = demo_disks(&chart);
        let disks = [
            (d1.cx.clone(), d1.cy.clone(), d1.r2.clone()), // A (source 0)
            (Q::from_i128(0), Q::new(1, 2), Q::from_i128(2)), // D2  (source 1)
            (d3[0].clone(), d3[1].clone(), d3[2].clone()), // D3  (source 2)
            (d4[0].clone(), d4[1].clone(), d4[2].clone()), // D4  (source 3)
        ];
        let mut edges = Vec::new();
        for (i, (cx, cy, r2)) in disks.iter().enumerate() {
            edges.extend(arrange2d::decompose::decompose(
                &geom::content::Curve::Circle {
                    circle: Circle {
                        cx: cx.clone(),
                        cy: cy.clone(),
                        r2: r2.clone(),
                    },
                    orient: Orient::Ccw,
                    source: CurveId(i as u32),
                },
            ));
        }
        let operand_of = |src: CurveId| {
            if src.0 == 0 {
                OperandId::A
            } else {
                OperandId::B
            }
        };
        match ledge_dom_certified(&edges, &operand_of, BoolOp::Diff) {
            Verdict::Verified(cap) => {
                let r = cap.region();
                assert_eq!(r.faces.len(), 1, "(D1−D2)−D3−D4 is one connected face");
                assert_eq!(
                    r.faces[0].holes.len(),
                    2,
                    "two interior holes (D2, D4); D3 only notches the outer boundary"
                );
                // D3 (source 2) appears on the OUTER loop (the notch), not as a hole.
                let outer_has_d3 = r.faces[0].outer.iter().any(|e| match e {
                    geom::content::Edge::Arc(a) => a.source == CurveId(2),
                    geom::content::Edge::Seg(_) => false,
                });
                assert!(outer_has_d3, "D3 arcs notch the outer boundary");
                println!("arrangement Diff: Verified — 1 face, 2 holes, D3 notches the rim");
            }
            other => panic!("arrangement not certified: {}", tag(&other)),
        }
    }

    /// The whole panel assembles: develop the notched outer boundary + the D4 hole, author a
    /// polygon (quad) cut, and stitch them with the flat `BoolOp::Diff` into one certified
    /// `Region` — one face, two holes (D4 + the quad).
    #[test]
    fn full_panel_assembles() {
        let chart = cone();
        let dev = ConeDevelopment::new(&chart).unwrap();
        let cfg = DevConfig::tight();
        let clearance = Q::from_i128(1);
        let span = Interval {
            lo: Q::from_i128(-1),
            hi: Q::from_i128(1),
        };
        let fit = RailFit::default();
        let (d1, d2, d3, d4) = demo_disks(&chart);

        let outer = match outer_loop(
            &chart,
            &d1,
            &d2,
            (&d3[0], &d3[1], &d3[2]),
            &span,
            fit,
            &clearance,
            &cfg,
            &Q::new(1, 20),
            48,
        ) {
            Verdict::Verified(o) => o,
            o => panic!("outer: {}", tag(&o)),
        };
        let outer_flat = match unroll_loop(&dev, &outer.arcs, &cfg, &clearance) {
            Verdict::Verified(o) => o,
            o => panic!("outer unroll: {}", tag(&o)),
        };
        let hole = match hole_loop(
            &chart,
            &d4[0],
            &d4[1],
            &d4[2],
            &span,
            fit,
            &clearance,
            &cfg,
            &Q::new(1, 200),
            32,
        ) {
            Verdict::Verified(h) => h,
            o => panic!("hole: {}", tag(&o)),
        };
        let d4_flat = match unroll_loop(&dev, &hole.arcs, &cfg, &clearance) {
            Verdict::Verified(o) => o,
            o => panic!("hole unroll: {}", tag(&o)),
        };

        // An authored quad cut, developed from (σ,μ) so it lands in the panel band (left of D4).
        let quad: Vec<[Q; 2]> = [
            (Q::new(-9, 20), Q::new(43, 20)),
            (Q::new(-6, 20), Q::new(43, 20)),
            (Q::new(-6, 20), Q::new(47, 20)),
            (Q::new(-9, 20), Q::new(47, 20)),
        ]
        .iter()
        .map(|(s, m)| {
            let (x, y) = dev.point(s, m, &cfg).center();
            [x, y]
        })
        .collect();

        let outer_poly = flat_to_poly(&outer_flat);
        let d4_poly = flat_to_poly(&d4_flat);
        let region = match assemble_flat(&outer_poly, &[d4_poly, quad]) {
            Verdict::Verified(r) => r,
            o => panic!("assemble: {}", tag(&o)),
        };
        assert_eq!(region.faces.len(), 1, "one connected panel face");
        assert_eq!(
            region.faces[0].holes.len(),
            2,
            "two interior cuts: the D4 circular hole and the authored quad"
        );
        println!(
            "full panel: 1 face, {} holes ({} outer verts)",
            region.faces[0].holes.len(),
            region.faces[0].outer.len()
        );
    }

    fn tag<T, E: core::fmt::Debug, M>(v: &Verdict<T, E, M>) -> String {
        match v {
            Verdict::Verified(_) => "Verified".into(),
            Verdict::Refuted(w) => format!("Refuted({w:?})"),
            Verdict::Unresolved(_) => "Unresolved".into(),
        }
    }
}
