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
use develop::cut::{CutFitCert, CutFitFault, CutSurface, cut_fit, cut_mu_form, plane_cut_rail};
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

// The rational root primitives live in `develop::pcurve` — below this layer, because the p-curve
// core needs them to locate σ-turning points and station crossings. Re-exported here (rather than
// kept as a second copy) so the trim layer and the curve core cannot drift apart: the duplicate
// here silently skipped a root landing *exactly* on a scan node, which symmetric geometry
// produces routinely.
pub use develop::pcurve::{bisect_root, scan_roots};

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

/// The two tangent-ruling σ where a **solid quadric cutter** (today: a cylinder of any axis)
/// grazes the surface within `span` — the two sign changes of the true-surface discriminant
/// ([`MuCut::disc`](develop::cut::MuCut::disc)); between them the ruling crosses the cutter (two
/// real µ̂ branches), the σ-extent of an interior hole. Pedal-general — it reads the real chart
/// fields (pedal *with* support), so it is correct on offset tails and under wrapping
/// parametrizations; the apex-ray shortcut it replaces (the A2 finding) silently assumed a cone
/// through the origin. `None` unless the cutter subtends exactly one clean two-tangent arc.
pub fn surface_tangents<B: Backend>(
    chart: &Chart<B>,
    surface: &CutSurface<B>,
    span: &Interval<B>,
    scan: usize,
    iters: usize,
) -> Option<(Rat<B>, Rat<B>)> {
    let roots = surface_disc_roots(chart, surface, span, scan, iters)?;
    if roots.len() == 2 {
        Some((roots[0].clone(), roots[1].clone()))
    } else {
        None
    }
}

/// **All** sign-change roots of the true-surface discriminant within `span`, in σ order — the
/// tangent rulings of every arc the cutter subtends in the gore (a wide gore meets a solid
/// cylinder along *two* windows, one per sheet of the ruling line; [`surface_tangents`] is the
/// single-window special case). Consecutive pairs with a positive discriminant between them are
/// the cutter's real σ-windows. `None` on a pole at a scan node.
pub fn surface_disc_roots<B: Backend>(
    chart: &Chart<B>,
    surface: &CutSurface<B>,
    span: &Interval<B>,
    scan: usize,
    iters: usize,
) -> Option<Vec<Rat<B>>> {
    let g = cut_mu_form(chart, surface, &Rat::from_i128(0))?.disc();
    scan_roots(&g, &span.lo, &span.hi, scan, iters)
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

impl RailFit {
    /// The **low-degree STEP re-fit** profile: a curved rail exported to OCCT must stay a
    /// handful of Bézier control points, or `MakeEdge`'s `f64` endpoints drift off the shared
    /// vertices (the G7 finding). Every STEP emission site uses this; the SVG/flat side keeps
    /// the tighter [`default`](RailFit::default).
    pub fn occt_low() -> Self {
        RailFit {
            degree: 4,
            subdiv: 256,
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
    certified_rail_surface(chart, &disk.surface, disk.pick, span, fit, clearance, cfg)
}

/// Fit **and** certify the ruling-rail of a bare [`CutSurface`] branch over `span` — the
/// footprint-free core of [`certified_rail`] (a [`TrimDisk`] just bundles the surface with its
/// xy footprint). The float oracle proposes, [`cut_fit`] decides; fail-closed as ever.
pub fn certified_rail_surface<B: Backend>(
    chart: &Chart<B>,
    surface: &CutSurface<B>,
    pick: RootPick,
    span: &Interval<B>,
    fit: RailFit,
    clearance: &Rat<B>,
    cfg: &DevConfig<B>,
) -> Verdict<(RatFunc<B>, Rat<B>), CutFitFault, Rat<B>> {
    let mu_hat = match fit_cut_rail(chart, surface, span, fit.degree, pick, fit.bits) {
        Some(m) => m,
        // The oracle declined (cut not real at a node / singular solve) — fail-closed.
        None => return Verdict::Unresolved(clearance.clone()),
    };
    let cert = CutFitCert {
        mu_hat: mu_hat.clone(),
        w: Rat::from_i128(0),
        surface: clone_surface(surface),
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

/// The **piecewise-region** certified rail: one rail per region band, each fitted and certified
/// against **its own region's chart** (supports differ per region; the frame is shared). This is
/// the flat-side sibling of `brep_trim_solid_regions`' piecewise boundaries — the self-lapping
/// demo's per-region `cyl_rails` lifted into the engine (A4). `charts` are the ordered,
/// contiguous region bands. Returns the ordered `(band, rail)` pieces (the shape
/// [`crate::brep_build::brep_trim_solid_regions`] and the piecewise develop consume) and the max
/// certified ε over the pieces. Fail-closed on the first piece that does not certify.
#[allow(clippy::type_complexity)]
pub fn certified_rail_piecewise<B: Backend>(
    charts: &[(Interval<B>, &Chart<B>)],
    surface: &CutSurface<B>,
    pick: RootPick,
    fit: RailFit,
    clearance: &Rat<B>,
    cfg: &DevConfig<B>,
) -> Verdict<(Vec<(Interval<B>, RatFunc<B>)>, Rat<B>), CutFitFault, Rat<B>> {
    let mut pieces = Vec::with_capacity(charts.len());
    let mut eps = Rat::from_i128(0);
    for (band, chart) in charts {
        let (mu, e) = match certified_rail_surface(chart, surface, pick, band, fit, clearance, cfg)
        {
            Verdict::Verified(x) => x,
            Verdict::Unresolved(e) => return Verdict::Unresolved(e),
            Verdict::Refuted(f) => return Verdict::Refuted(f),
        };
        if eps.cmp(&e) == core::cmp::Ordering::Less {
            eps = e;
        }
        pieces.push((band.clone(), mu));
    }
    Verdict::Verified((pieces, eps))
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
    /// The residual µ̂ gap where the two branches meet at a tangent ruling — the loop closes to a
    /// single vertex there, and this is how far that vertex sits from the true tangent point. It
    /// is included in `eps`, so it is a bound the caller already honours, not a loose diagnostic.
    pub tangent_gap: Rat<B>,
}

/// Build the interior-hole boundary loop for the disk `(cx, cy, r²)`: the vertical-cylinder
/// idiom of [`surface_hole_loop`].
#[allow(clippy::too_many_arguments)]
pub fn hole_loop<B: Backend>(
    chart: &Chart<B>,
    cx: &Rat<B>,
    cy: &Rat<B>,
    r2: &Rat<B>,
    span: &Interval<B>,
    clearance: &Rat<B>,
    cfg: &DevConfig<B>,
    segments: usize,
) -> Verdict<HoleLoop<B>, CutFitFault, Rat<B>> {
    let surface = CutSurface::Cylinder {
        axis_point: [cx.clone(), cy.clone(), Rat::from_i128(0)],
        axis_dir: [Rat::from_i128(0), Rat::from_i128(0), Rat::from_i128(1)],
        r2: r2.clone(),
    };
    surface_hole_loop(chart, &surface, span, clearance, cfg, segments)
}

/// Build the interior-hole boundary loop of a **solid quadric cutter** piercing the sheet: the
/// closed **p-curve loop** through both of the cutter's tangent rulings
/// ([`develop::cut::quadric_cut_loop`]).
///
/// This used to fit the cut's near and far branches as graphs `µ̂ = f(σ)`. A graph cannot reach a
/// tangent ruling — the cut turns around in σ there — so each branch stopped an inset short and
/// the leftover gap was bridged by a straight radial chord, which no fit quality could shrink
/// below ~30% of the hole's height. The loop now walks the branches to where they meet, so
/// `fit` and `margin` are vestigial (kept for call-site compatibility until PC.6 removes them)
/// and `tangent_gap` reports the residual gap where the branches meet, which is *inside* `eps` rather than
/// unaccounted. Pedal-general as before (reads the true surface, so it is correct on offset
/// supports and wrapped charts); fail-closed on a coarse loop (`Unresolved`) or a degenerate
/// window (`Refuted`).
#[allow(clippy::too_many_arguments)]
pub fn surface_hole_loop<B: Backend>(
    chart: &Chart<B>,
    surface: &CutSurface<B>,
    span: &Interval<B>,
    clearance: &Rat<B>,
    cfg: &DevConfig<B>,
    segments: usize,
) -> Verdict<HoleLoop<B>, CutFitFault, Rat<B>> {
    let (t_lo, t_hi) = match surface_tangents(chart, surface, span, 256, 60) {
        Some(t) => t,
        None => return Verdict::Unresolved(clearance.clone()),
    };
    match develop::cut::quadric_cut_loop(
        chart,
        surface,
        &Interval { lo: t_lo, hi: t_hi },
        &Rat::from_i128(0),
        segments.max(8),
        clearance,
        cfg,
    ) {
        Verdict::Verified(l) => Verdict::Verified(HoleLoop {
            arcs: l
                .pieces
                .into_iter()
                .map(|curve| BoundaryArc::Curve { curve, segments: 1 })
                .collect(),
            eps: l.eps,
            tangent_gap: l.tangent_gap,
        }),
        Verdict::Unresolved(e) => Verdict::Unresolved(e),
        Verdict::Refuted(f) => Verdict::Refuted(f),
    }
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

/// Split a trim [`OuterLoop`] into the two piecewise μ-boundaries the curved-rail solid builder
/// (`export::brep_build::brep_trim_solid`) consumes: the **outer** chain (the forward rails,
/// σ-increasing — D1 / D3 notch / D1) and the **inner** chain (the backward rails reversed to
/// σ-increasing — D2), each an ordered list of `(σ-range, μ̂)` pieces. Rails are classified by
/// traversal direction (`σ_start < σ_end` ⇒ outer/top, `>` ⇒ inner/bottom); the σ-caps and the
/// ~zero notch micro-caps drop out (the band ends and the notch corner are recovered from the rail
/// σ-extents and the `μ̂_D1 = μ̂_D3` shared corner). `None` if either chain is empty.
#[allow(clippy::type_complexity)]
pub fn trim_rail_chains<B: Backend>(
    outer: &OuterLoop<B>,
) -> Option<(
    Vec<(Interval<B>, RatFunc<B>)>,
    Vec<(Interval<B>, RatFunc<B>)>,
)> {
    use core::cmp::Ordering::{Greater, Less};
    let mut inner_ch: Vec<(Interval<B>, RatFunc<B>)> = Vec::new();
    let mut outer_ch: Vec<(Interval<B>, RatFunc<B>)> = Vec::new();
    for arc in &outer.arcs {
        if let BoundaryArc::Rail {
            mu,
            sigma_start,
            sigma_end,
            ..
        } = arc
        {
            match sigma_start.cmp(sigma_end) {
                Less => outer_ch.push((
                    Interval {
                        lo: sigma_start.clone(),
                        hi: sigma_end.clone(),
                    },
                    mu.clone(),
                )),
                Greater => inner_ch.push((
                    Interval {
                        lo: sigma_end.clone(),
                        hi: sigma_start.clone(),
                    },
                    mu.clone(),
                )),
                _ => {} // a degenerate rail — skip
            }
        }
    }
    outer_ch.sort_by(|a, b| a.0.lo.cmp(&b.0.lo));
    inner_ch.sort_by(|a, b| a.0.lo.cmp(&b.0.lo));
    if inner_ch.is_empty() || outer_ch.is_empty() {
        return None;
    }
    // Snap the interior piece boundaries (the D3∩D1 crossings, bisected to ~60-bit denominators) to a
    // small-denominator dyadic. A huge-rational σ-station makes the exported Bézier control points
    // huge rationals whose f64 endpoints drift from the vertex, so OCCT's `MakeEdge` rejects them; the
    // crossing σ needs no such precision — the builder's stitch re-establishes the corner exactly at
    // the snapped σ. (Only the STEP path snaps; the flat SVG uses `outer_loop`'s full-precision σ.)
    snap_boundaries(&mut inner_ch);
    snap_boundaries(&mut outer_ch);
    Some((inner_ch, outer_ch))
}

/// Adapt a developed [`HoleLoop`] to a [`HoleRail`](crate::brep_build::HoleRail) for the
/// curved-rail solid builder: the loop's **near** and **far** branches as contiguous `(band,
/// rail)` chains over the hole's σ-extent.
///
/// The loop is a closed p-curve through both tangent rulings, traversed left tangent → far branch
/// → right tangent → near branch. Each piece is straight in `(σ, µ̂)` with distinct endpoint σ, so
/// it is exactly a linear rail over its own σ-band — the branches *are* functions of σ (that was
/// never the problem); what they are not is polynomials near the tangents, which is why they
/// arrive as many short pieces rather than one fitted graph. Splitting the loop at its two σ-
/// extremes recovers the near/far band the slice builder consumes, so a hole may still span
/// σ-stations.
///
/// σ are dyadic-snapped as before, so exported Bézier control points stay small-denominator.
/// `None` if the loop is not a single σ-extreme-to-σ-extreme traversal.
pub fn hole_rail<B: Backend>(hole: &HoleLoop<B>) -> Option<crate::brep_build::HoleRail<B>> {
    use core::cmp::Ordering::{Equal, Greater, Less};
    let snap = |r: &Rat<B>| crate::approx::f64_to_rat::<B>(crate::approx::rat_to_f64(r), 30);
    // The loop's corners in traversal order.
    let mut pts: Vec<(Rat<B>, Rat<B>)> = Vec::with_capacity(hole.arcs.len());
    for arc in &hole.arcs {
        match arc {
            BoundaryArc::Curve { curve, .. } => {
                let [sg, m] = curve.eval(&curve.domain.lo)?;
                pts.push((snap(&sg), m));
            }
            _ => return None,
        }
    }
    if pts.len() < 4 {
        return None;
    }
    // The two σ-extremes are the tangent rulings; they split the loop into its two branches.
    let idx_min = (0..pts.len()).min_by(|&a, &b| pts[a].0.cmp(&pts[b].0))?;
    let idx_max = (0..pts.len()).max_by(|&a, &b| pts[a].0.cmp(&pts[b].0))?;
    let n = pts.len();
    let walk = |from: usize, to: usize| -> Vec<(Rat<B>, Rat<B>)> {
        let mut out = Vec::new();
        let mut i = from;
        loop {
            out.push(pts[i].clone());
            if i == to {
                break;
            }
            i = (i + 1) % n;
        }
        out
    };
    // One branch runs min→max in traversal order, the other max→min.
    let branch_a = walk(idx_min, idx_max);
    let branch_b = walk(idx_max, idx_min);
    // Turn a σ-monotone vertex run into a chain of linear rails over its σ-bands.
    let chain = |run: &[(Rat<B>, Rat<B>)]| -> Option<Vec<(Interval<B>, RatFunc<B>)>> {
        let mut out = Vec::with_capacity(run.len().saturating_sub(1));
        for w in run.windows(2) {
            let ((sa, ma), (sb, mb)) = (&w[0], &w[1]);
            let (lo, hi, va, vb) = match sa.cmp(sb) {
                Less => (sa, sb, ma, mb),
                Greater => (sb, sa, mb, ma),
                Equal => return None, // a vertical piece is not a rail
            };
            let slope = vb.sub(va).div(&hi.sub(lo));
            let intercept = va.sub(&lo.mul(&slope));
            out.push((
                Interval {
                    lo: lo.clone(),
                    hi: hi.clone(),
                },
                RatFunc::from_poly(Poly::from_coeffs(vec![intercept, slope])),
            ));
        }
        out.sort_by(|x, y| x.0.lo.cmp(&y.0.lo));
        (!out.is_empty()).then_some(out)
    };
    let (chain_a, chain_b) = (chain(&branch_a)?, chain(&branch_b)?);
    // Which branch is the far (larger µ̂) one? Compare them where both are defined.
    let probe = pts[idx_min].0.add(&pts[idx_max].0).mul(&Rat::new(1, 2));
    let va = crate::brep_build::chain_eval(&chain_a, &probe)?;
    let vb = crate::brep_build::chain_eval(&chain_b, &probe)?;
    let (near, far) = if va.cmp(&vb) == Less {
        (chain_a, chain_b)
    } else {
        (chain_b, chain_a)
    };
    Some(crate::brep_build::HoleRail {
        near,
        far,
        s1: pts[idx_min].0.clone(),
        s2: pts[idx_max].0.clone(),
    })
}

/// Snap each interior piece boundary of a contiguous chain to a 2⁻³⁰ dyadic (via `f64`), keeping
/// adjacent pieces adjacent; the outer σ-ends are authored and left untouched.
fn snap_boundaries<B: Backend>(chain: &mut [(Interval<B>, RatFunc<B>)]) {
    let snap = |r: &Rat<B>| crate::approx::f64_to_rat::<B>(crate::approx::rat_to_f64(r), 30);
    for i in 0..chain.len().saturating_sub(1) {
        let s = snap(&chain[i].0.hi);
        chain[i].0.hi = s.clone();
        chain[i + 1].0.lo = s;
    }
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
        let fit = RailFit::default();
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
        // The gore develops the UPPER half-plane: σ = 0 ⇒ azimuth 90° (+y), σ = ±1 ⇒ 0°/180°.
        // So disks live around +y. D4: a small disk R = 0.2 at (0, 2.2) — inside the annulus (D2
        // exits near r ≈ 1.9, D1 at r ≈ 2.71), apex well outside it.
        let (cx, cy, r2) = (Q::from_i128(0), Q::new(11, 5), Q::new(1, 25));

        let d4_surface = CutSurface::Cylinder {
            axis_point: [cx.clone(), cy.clone(), Q::from_i128(0)],
            axis_dir: [Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)],
            r2: r2.clone(),
        };
        let (tlo, thi) =
            surface_tangents(&chart, &d4_surface, &span, 256, 60).expect("two tangents");
        println!("D4 tangents: σ ∈ [{:.4}, {:.4}]", f(&tlo), f(&thi));
        assert!(tlo.cmp(&thi) == core::cmp::Ordering::Less);

        let hole = match hole_loop(&chart, &cx, &cy, &r2, &span, &clearance, &cfg, 32) {
            Verdict::Verified(h) => h,
            other => panic!("hole_loop not Verified: {}", tag(&other)),
        };
        println!(
            "D4 hole: rail ε = {:.3e}, max micro-cap = {:.3e} (μ̂ units, hole is ≈0.4 tall)",
            f(&hole.eps),
            f(&hole.tangent_gap)
        );
        // The tangent micro-cap is the √-branch residual (the developed circle's two tangent
        // points are slightly flattened) — an exact Cap, watertight, small vs the hole height.
        assert!(
            hole.tangent_gap.cmp(&Q::new(1, 10)) == core::cmp::Ordering::Less,
            "tangent micro-cap should stay small, got {:.4}",
            f(&hole.tangent_gap)
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

    /// On an **apex cone** (`h ≡ 0`) the true-surface discriminant tangents coincide EXACTLY with
    /// the old apex-ray formula (they differ by the factor `−4·|r_xy|²` via the Lagrange identity,
    /// so the sign scans walk identical brackets) — the A2 fix is a strict generalization.
    #[test]
    fn true_surface_tangents_match_the_apex_ray_on_the_cone() {
        let chart = cone();
        let span = Interval {
            lo: Q::from_i128(-1),
            hi: Q::from_i128(1),
        };
        let (cx, cy, r2) = (Q::from_i128(0), Q::new(11, 5), Q::new(1, 25));
        let d4 = CutSurface::Cylinder {
            axis_point: [cx.clone(), cy.clone(), Q::from_i128(0)],
            axis_dir: [Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)],
            r2: r2.clone(),
        };
        let (t_lo, t_hi) = surface_tangents(&chart, &d4, &span, 256, 60).unwrap();
        // The old apex-ray residual: (cx·r_y − cy·r_x)² − R²·(r_x² + r_y²).
        let (rx, ry) = ruling_xy(&chart);
        let konst = |r: &Q| RatFunc::from_poly(Poly::constant(r.clone()));
        let cross = ry.mul(&konst(&cx)).sub(&rx.mul(&konst(&cy)));
        let norm2 = rx.mul(&rx).add(&ry.mul(&ry));
        let old = cross.mul(&cross).sub(&konst(&r2).mul(&norm2));
        let roots = scan_roots(&old, &span.lo, &span.hi, 256, 60).unwrap();
        assert_eq!(roots.len(), 2);
        assert_eq!(t_lo, roots[0], "identical bisection on the apex cone");
        assert_eq!(t_hi, roots[1]);
    }

    /// On an **offset support** (`h ≠ 0` — the seam-ramp fixture) the apex-ray shortcut mislocates
    /// the tangents, while the true-surface discriminant brackets the disk correctly and the whole
    /// hole loop still certifies — the A2 correctness fix, end to end.
    #[test]
    fn offset_support_hole_certifies_with_true_tangents() {
        use fixtures::devices::cone_seam_ramp;
        let chart = cone_seam_ramp();
        let cfg = DevConfig::tight();
        // Drop the disk exactly onto the ramp surface: its centre is the (exact) xy of the
        // surface point at σ = 1/4, µ̂ = −2 — the ruling provably pierces it.
        let p = chart
            .surface(&Q::from_i128(-2), &Q::from_i128(0))
            .eval(&Q::new(1, 4))
            .unwrap();
        let (cx, cy, r2) = (p[0].clone(), p[1].clone(), Q::new(1, 25));
        let span = Interval {
            lo: Q::from_i128(0),
            hi: Q::new(1, 2),
        };
        let drill = CutSurface::Cylinder {
            axis_point: [cx.clone(), cy.clone(), Q::from_i128(0)],
            axis_dir: [Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)],
            r2: r2.clone(),
        };
        let (t_lo, t_hi) = surface_tangents(&chart, &drill, &span, 512, 60)
            .expect("the true-surface disc brackets the disk");
        assert!(
            t_lo < Q::new(1, 4) && Q::new(1, 4) < t_hi,
            "extent straddles the pierce"
        );
        // The apex-ray shortcut on the SAME disk: its bracket differs (or fails outright) —
        // quantify the mislocation when it does return two roots.
        let (rx, ry) = ruling_xy(&chart);
        let konst = |r: &Q| RatFunc::from_poly(Poly::constant(r.clone()));
        let cross = ry.mul(&konst(&cx)).sub(&rx.mul(&konst(&cy)));
        let norm2 = rx.mul(&rx).add(&ry.mul(&ry));
        let old = cross.mul(&cross).sub(&konst(&r2).mul(&norm2));
        let tol = Q::new(1, 1000);
        match scan_roots(&old, &span.lo, &span.hi, 512, 60) {
            Some(r) if r.len() == 2 => {
                let d0 = r[0].sub(&t_lo);
                let d1 = r[1].sub(&t_hi);
                assert!(
                    d0.mul(&d0).cmp(&tol.mul(&tol)) == core::cmp::Ordering::Greater
                        || d1.mul(&d1).cmp(&tol.mul(&tol)) == core::cmp::Ordering::Greater,
                    "the apex-ray tangents must be visibly wrong on the offset support"
                );
            }
            _ => {} // failing to bracket at all is the bug too
        }
        // End to end: the hole loop over the true extent certifies against the REAL surface.
        let hole = match hole_loop(&chart, &cx, &cy, &r2, &span, &Q::from_i128(1), &cfg, 16) {
            Verdict::Verified(h) => h,
            other => panic!("ramp hole_loop not Verified: {}", tag(&other)),
        };
        // The loop is a closed chain of p-curve pieces through both tangent rulings — no
        // near/far graphs and no straight bridge, so there is no fixed arc count to assert.
        assert!(hole.arcs.len() >= 8, "a closed loop of p-curve pieces");
        assert!(
            hole.arcs
                .iter()
                .all(|a| matches!(a, BoundaryArc::Curve { .. })),
            "every piece is a domain curve"
        );
        // What used to be a straight chord across the tangents is now the residual gap where the
        // two branches meet, and it is small against the hole itself.
        let mc = develop::cut::cut_mu_form(&chart, &drill, &Q::from_i128(0)).unwrap();
        let (_, h_mid) = mc
            .branch_at(&t_lo.add(&t_hi).mul(&Q::new(1, 2)), &cfg.sqrt_eps)
            .expect("the mid ruling cuts the drill");
        assert!(
            hole.tangent_gap.mul(&Q::from_i128(100)) < h_mid.mul(&Q::from_i128(2)),
            "the tangent gap must be far below the graph model's floor: gap {:.3e} vs height {:.3e}",
            f(&hole.tangent_gap),
            f(&h_mid.mul(&Q::from_i128(2)))
        );
        println!(
            "ramp hole: ε = {:.3e}, tangent gap = {:.3e}, extent σ ∈ [{:.4}, {:.4}]",
            f(&hole.eps),
            f(&hole.tangent_gap),
            f(&t_lo),
            f(&t_hi)
        );
    }

    /// A piecewise rail certifies **per region against its own chart** (A4): the device cone on
    /// `[0, 1/4]` glued to the seam-ramp flap on `[1/4, 1/2]` (the PR-1 `PiecewiseDevelopment`
    /// pair), cut by one apex-containing cylinder — two contiguous pieces, one max ε, and the
    /// pieces nearly agree at the join (both approximate the same cut within the DRC bound).
    #[test]
    fn a_piecewise_rail_certifies_per_region() {
        use develop::cut::CutSurface;
        use fixtures::devices::cone_seam_ramp;
        let body = cone();
        let ramp = cone_seam_ramp();
        let bands = [
            (
                Interval {
                    lo: Q::from_i128(0),
                    hi: Q::new(1, 4),
                },
                &body,
            ),
            (
                Interval {
                    lo: Q::new(1, 4),
                    hi: Q::new(1, 2),
                },
                &ramp,
            ),
        ];
        let surface = CutSurface::Cylinder {
            axis_point: [Q::from_i128(0), Q::new(1, 2), Q::from_i128(0)],
            axis_dir: [Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)],
            r2: Q::from_i128(2),
        };
        let clearance = Q::from_i128(1);
        let (pieces, eps) = match certified_rail_piecewise(
            &bands,
            &surface,
            RootPick::Upper,
            RailFit::occt_low(),
            &clearance,
            &DevConfig::tight(),
        ) {
            Verdict::Verified(x) => x,
            other => panic!("piecewise rail not certified: {}", tag(&other)),
        };
        assert_eq!(pieces.len(), 2);
        assert_eq!(pieces[0].0.hi, pieces[1].0.lo, "contiguous bands");
        assert!(
            eps.sign() >= 0 && eps < Q::new(1, 2),
            "ε under the DRC gate"
        );
        // At the join both rails approximate the same cut curve — the µ̂ mismatch stays within
        // the fab clearance (the micro-cap the loop assembly would bridge).
        let j = &pieces[0].0.hi;
        let gap = pieces[0]
            .1
            .eval(j)
            .unwrap()
            .sub(&pieces[1].1.eval(j).unwrap());
        assert!(
            gap.mul(&gap).cmp(&clearance.mul(&clearance)) == core::cmp::Ordering::Less,
            "join gap within clearance"
        );
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
        let fit = RailFit::default();
        let chart = cone();
        let dev = ConeDevelopment::new(&chart).unwrap();
        let cfg = DevConfig::tight();
        let clearance = Q::from_i128(1);
        let span = Interval {
            lo: Q::from_i128(-1),
            hi: Q::from_i128(1),
        };
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
        let hole = match hole_loop(&chart, &d4[0], &d4[1], &d4[2], &span, &clearance, &cfg, 32) {
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

    /// The eccentric annulus + D3 notch built as a certified closed cone solid via `brep_trim_solid`
    /// from the real D1/D2/D3 rails, and round-tripped through OCCT (Stage A, no interior holes).
    #[cfg(feature = "step")]
    #[test]
    fn annulus_notch_solid_exports() {
        use crate::brep_build::brep_trim_solid;
        use crate::step::write_brep;
        use certify_core::shell::closed_shell_holed;
        let chart = cone();
        let cfg = DevConfig::tight();
        let clearance = Q::from_i128(1);
        let span = Interval {
            lo: Q::from_i128(-1),
            hi: Q::from_i128(1),
        };
        let (d1, d2, d3, _d4) = demo_disks(&chart);
        let lowfit = RailFit {
            degree: 4,
            subdiv: 256,
            bits: 44,
        };
        let outer = match outer_loop(
            &chart,
            &d1,
            &d2,
            (&d3[0], &d3[1], &d3[2]),
            &span,
            lowfit,
            &clearance,
            &cfg,
            &Q::new(1, 20),
            8,
        ) {
            Verdict::Verified(o) => o,
            other => panic!("outer_loop: {}", tag(&other)),
        };
        let (inner, outer_ch) = trim_rail_chains(&outer).expect("rail chains");
        let w = Interval {
            lo: Q::from_i128(0),
            hi: Q::new(1, 8),
        };
        let solid = brep_trim_solid(&chart, &w, &inner, &outer_ch, &[]).expect("trim solid");
        assert_eq!(solid.free_edges(), 0, "annulus+notch solid is watertight");
        assert_eq!(solid.nonmanifold_edges(), 0);
        let sc = solid.to_shell_certificate();
        assert!(
            matches!(
                closed_shell_holed(
                    sc.n_verts,
                    &sc.edge_start,
                    &sc.edge_end,
                    &sc.wire_edge,
                    &sc.wire_reversed,
                    &sc.loop_start,
                    &sc.face_start,
                ),
                Verdict::Verified(_)
            ),
            "annulus+notch solid is a certified closed 2-manifold"
        );
        let path = format!("{}/trim_annulus.step", std::env::temp_dir().display());
        assert_eq!(write_brep(&path, &solid), "ok", "OCCT round-trip");
    }

    /// The **finished panel** (STEP II): the annulus + D3 notch with the **D4** circular through-hole
    /// (which sits at σ = 0, a positive-weight station → a curved cross-ring **notch**) and the
    /// authored **quad** cut both drilled through, from the real developed rails. A certified genus-2
    /// closed solid that round-trips through OCCT — the end-to-end trimmed-panel STEP the demo emits.
    #[cfg(feature = "step")]
    #[test]
    fn full_panel_solid_exports() {
        use crate::brep_build::{HoleRail, brep_trim_solid};
        use crate::step::write_brep;
        use certify_core::shell::closed_shell_holed;
        use lattice::Poly;
        let chart = cone();
        let cfg = DevConfig::tight();
        let clearance = Q::from_i128(1);
        let span = Interval {
            lo: Q::from_i128(-1),
            hi: Q::from_i128(1),
        };
        let (d1, d2, d3, d4) = demo_disks(&chart);
        let lowfit = RailFit {
            degree: 4,
            subdiv: 256,
            bits: 44,
        };
        let outer = match outer_loop(
            &chart,
            &d1,
            &d2,
            (&d3[0], &d3[1], &d3[2]),
            &span,
            lowfit,
            &clearance,
            &cfg,
            &Q::new(1, 20),
            8,
        ) {
            Verdict::Verified(o) => o,
            other => panic!("outer_loop: {}", tag(&other)),
        };
        let (inner, outer_ch) = trim_rail_chains(&outer).expect("rail chains");
        let d4_hole = match hole_loop(
            &chart,
            &d4[0],
            &d4[1],
            &d4[2],
            &span,
            lowfit,
            &clearance,
            &cfg,
            &Q::new(1, 200),
            4,
        ) {
            Verdict::Verified(h) => hole_rail(&h).expect("D4 hole rail"),
            other => panic!("hole_loop: {}", tag(&other)),
        };
        let konst = |n: i128, dd: i128| RatFunc::<Bignum>::from_poly(Poly::constant(Q::new(n, dd)));
        let quad = HoleRail {
            near: konst(43, 20),
            far: konst(47, 20),
            s1: Q::new(-9, 20),
            s2: Q::new(-6, 20),
        };
        let w = Interval {
            lo: Q::from_i128(0),
            hi: Q::new(1, 8),
        };
        let solid = brep_trim_solid(&chart, &w, &inner, &outer_ch, &[d4_hole, quad])
            .expect("full panel trim solid");
        assert_eq!(solid.free_edges(), 0, "the drilled panel is watertight");
        assert_eq!(solid.nonmanifold_edges(), 0);
        let sc = solid.to_shell_certificate();
        assert!(
            matches!(
                closed_shell_holed(
                    sc.n_verts,
                    &sc.edge_start,
                    &sc.edge_end,
                    &sc.wire_edge,
                    &sc.wire_reversed,
                    &sc.loop_start,
                    &sc.face_start,
                ),
                Verdict::Verified(_)
            ),
            "the finished panel is a certified closed 2-manifold"
        );
        let path = format!("{}/trim_full_panel.step", std::env::temp_dir().display());
        assert_eq!(write_brep(&path, &solid), "ok", "OCCT round-trip");
    }

    fn tag<T, E: core::fmt::Debug, M>(v: &Verdict<T, E, M>) -> String {
        match v {
            Verdict::Verified(_) => "Verified".into(),
            Verdict::Refuted(w) => format!("Refuted({w:?})"),
            Verdict::Unresolved(_) => "Unresolved".into(),
        }
    }
}
