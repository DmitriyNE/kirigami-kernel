//! **Ray-picked frames** — placing a sketch by casting a ray at the geometry, under the
//! search/certificate split.
//!
//! A ray meeting a rational developable solves a polynomial, so the hit is in general **algebraic**.
//! Carrying it as such would push `AlgReal` arithmetic into every downstream cut, so this follows
//! the split MAP.1 installed in [`crate::fold`]: the hit-finding is a **search**, and what it
//! produces is checked by **backward error** rather than trusted.
//!
//! ## What is exact, and what is not
//!
//! The split lands in an unusually favourable place here, and it is worth being precise about why.
//! A chart's `pedal`, `ruling` and `normal` are rational functions of σ, so at a **rational** σ they
//! evaluate to exact rational vectors. Therefore a frame built at a rational σ has:
//!
//! - an origin that is **exactly on the surface** — not near it, on it;
//! - axes that are **exactly** the chart's own ruling and normal there.
//!
//! What is *not* exact is which σ: the true hit sits at an algebraic σ\*, and the search returns a
//! rational σ nearby. So the entire backward error collapses into one measurable quantity — **how
//! far the frame's origin is from the cast ray** — and the certificate says: *this frame is the
//! exact pick of a ray parallel to the one you asked for, displaced by at most ε*. That is backward
//! error in its textbook sense: the nearby problem was solved exactly.
//!
//! ## The search need not be floating-point
//!
//! The ray meets the ruling at σ exactly where the two lines are coplanar, so the hits are the roots
//! of the rational `g(σ) = det[base(σ) − origin, ruling(σ), dir]`. [`scan_roots`] isolates those to
//! rationals by bisection, so this searcher uses **no float at all** — which is not a change of
//! doctrine but a demonstration of it: the certificate never asked how σ was found, and swapping a
//! float solve for a bisection changes nothing downstream. A float cast could be dropped in for
//! speed and the guarantee would be identical.
//!
//! ## What the certificate does *not* cover
//!
//! The ε bound is about the frame's **geometry**, not about the hit's **ordinal**. `scan_roots` owns
//! a scan density and can step over a double root or two roots inside one cell, so "the third
//! surface the ray meets" is only as reliable as the scan. Nothing here detects that, and a caller
//! selecting by ordinal — the span, `docs/cutter-extrude-design.md` §5 — needs a root **count** it
//! can trust, which is a Sturm question rather than a backward-error one.
//!
//! ```
//! use develop::pick::{pick_nth, Facing, Ray};
//! use certify_core::Verdict;
//! use fixtures::devices::cone;
//! use lattice::{Bignum, Interval, Rat};
//!
//! type Q = Rat<Bignum>;
//! let q = |n: i128| Q::from_i128(n);
//! // Cast along +x at z = 3, straight through the device cone: it crosses twice.
//! let ray = Ray { origin: [q(-5), q(0), q(3)], dir: [q(1), q(0), q(0)] };
//! let span = Interval { lo: q(-3), hi: q(3) };
//! let (frame, pick) = match pick_nth(
//!     &cone(),
//!     &q(0),                 // mid-surface
//!     &ray,
//!     &span,
//!     0,                     // the first surface the ray meets
//!     Facing::SurfaceToward, // sketch on the plane facing the caster
//!     &Q::new(1, 100),       // fab clearance
//!     &Q::new(1, 1 << 40),
//!     64,
//!     48,
//! ) {
//!     Verdict::Verified(v) => v,
//!     other => panic!("expected a certified pick"),
//! };
//! // The frame sits on the surface, and the certificate says how far it is from the ray.
//! assert!(pick.eps < Q::new(1, 1000));
//! // A surface facing puts the local ruling in the sketch plane exactly.
//! assert!(pick.skew_sin2.is_zero());
//! assert!(frame.metric().uv.is_zero()); // orthogonal — though not orthonormal, which it reports
//! ```

use crate::extrude::{ExtrudeFault, Frame};
use crate::interval::sqrt;
use crate::pcurve::scan_roots;
use certify_core::Verdict;
use geom::chart::Chart;
use lattice::{Backend, Bignum, Interval, Poly, Rat, RatFunc, SturmChain, Vec3Rat};

/// A rational 3-vector.
type V3<B> = [Rat<B>; 3];

/// The cast ray `origin + t·dir`, `t` increasing away from the caster.
///
/// A **ray**, not a line: only `t ≥ 0` counts. The surface may well continue behind the origin —
/// the self-lapping device's far wall does — but a cut does not reach backwards, so those crossings
/// are not hits.
#[derive(Debug)]
pub struct Ray<B: Backend = Bignum> {
    /// Where the ray starts.
    pub origin: V3<B>,
    /// Which way it points. Need not be unit — nothing here divides by its length except the
    /// distance bound, which divides exactly.
    pub dir: V3<B>,
}

impl<B: Backend> Clone for Ray<B> {
    fn clone(&self) -> Self {
        Ray {
            origin: self.origin.clone(),
            dir: self.dir.clone(),
        }
    }
}

/// Which way the picked frame's normal faces — the choice
/// `docs/cutter-extrude-design.md` §5 leaves to the caller.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Facing {
    /// The ray's own direction. The sketch is then perpendicular to the cast, which is what a
    /// caller drilling *along* the ray usually wants — at the price of a nonzero skew (below),
    /// since the surface's ruling does not generally lie in a plane perpendicular to the ray.
    Ray,
    /// The surface normal at the hit, oriented **against** the ray — facing the caster.
    SurfaceToward,
    /// The surface normal at the hit, oriented **with** the ray.
    SurfaceAway,
}

/// One place the ray meets the chart's ruled surface, as the search found it.
#[derive(Debug)]
pub struct Hit<B: Backend = Bignum> {
    /// The chart parameter.
    pub sigma: Rat<B>,
    /// The ruling coordinate.
    pub mu: Rat<B>,
    /// The ray parameter — what hits are **ordered by**.
    pub t: Rat<B>,
}

impl<B: Backend> Clone for Hit<B> {
    fn clone(&self) -> Self {
        Hit {
            sigma: self.sigma.clone(),
            mu: self.mu.clone(),
            t: self.t.clone(),
        }
    }
}

/// The evidence a picked frame carries.
#[derive(Debug)]
pub struct ValidPick<B: Backend = Bignum> {
    /// The **backward error**: the frame's origin lies on the surface exactly, and within `eps` of
    /// the ray's own point at parameter [`Hit::t`]. Equivalently, the frame is the exact pick of a
    /// ray parallel to the requested one and displaced by at most `eps` — and it is that *point*,
    /// not merely that line, so the ordering `t` carries is certified along with the position.
    pub eps: Rat<B>,
    /// `sin²` of the angle between the frame's `u` axis and the local ruling — **reported, not
    /// gated**. It is exactly zero for either surface [`Facing`], where the ruling already lies in
    /// the frame plane, and generally nonzero for [`Facing::Ray`], where it is not an error but the
    /// geometry of having chosen the ray as the normal.
    pub skew_sin2: Rat<B>,
    /// The clearance the DRC compared against (`eps < clearance/2`).
    pub clearance: Rat<B>,
    /// The hit the frame was built at.
    pub hit: Hit<B>,
}

/// Why a pick was refused. None of these is a tolerance a finer search would clear.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PickFault {
    /// The ray has no direction.
    DegenerateRay,
    /// The σ-span is empty or reversed.
    DegenerateSpan,
    /// A chart field or the coplanarity residual poled at a scan node — re-author the span away
    /// from the pole, or scan differently.
    PoleInScan,
    /// The ray meets no ruling of this chart within the span, or fewer than the requested ordinal.
    NoHit,
    /// The ray runs **along** a ruling, so the two lines are coplanar over a σ-interval rather than
    /// at a point and there is no isolated hit to pick.
    RayAlongRuling,
    /// The ray grazes: it is tangent to the surface at the hit (for a surface [`Facing`], the
    /// normal is perpendicular to the ray, so the pick has no well-defined side).
    Grazing,
    /// The frame the pick produced is degenerate — the ruling is parallel to the chosen normal, so
    /// the sketch plane has no orientation.
    Degenerate(ExtrudeFault),
}

/// A constant vector as a degree-0 [`Vec3Rat`], so it combines with the chart's σ-rational fields.
fn const_vec3<B: Backend>(v: &V3<B>) -> Vec3Rat<B> {
    Vec3Rat::new(
        [
            Poly::constant(v[0].clone()),
            Poly::constant(v[1].clone()),
            Poly::constant(v[2].clone()),
        ],
        Poly::constant(Rat::from_i128(1)),
    )
}

fn dot3<B: Backend>(a: &V3<B>, b: &V3<B>) -> Rat<B> {
    a[0].mul(&b[0]).add(&a[1].mul(&b[1])).add(&a[2].mul(&b[2]))
}

fn cross3<B: Backend>(a: &V3<B>, b: &V3<B>) -> V3<B> {
    [
        a[1].mul(&b[2]).sub(&a[2].mul(&b[1])),
        a[2].mul(&b[0]).sub(&a[0].mul(&b[2])),
        a[0].mul(&b[1]).sub(&a[1].mul(&b[0])),
    ]
}

fn sub3<B: Backend>(a: &V3<B>, b: &V3<B>) -> V3<B> {
    [a[0].sub(&b[0]), a[1].sub(&b[1]), a[2].sub(&b[2])]
}

fn add3<B: Backend>(a: &V3<B>, b: &V3<B>) -> V3<B> {
    [a[0].add(&b[0]), a[1].add(&b[1]), a[2].add(&b[2])]
}

fn scale3<B: Backend>(a: &V3<B>, k: &Rat<B>) -> V3<B> {
    [a[0].mul(k), a[1].mul(k), a[2].mul(k)]
}

fn is_zero3<B: Backend>(a: &V3<B>) -> bool {
    a.iter().all(|x| x.is_zero())
}

/// The chart's base curve at layer offset `w`: `pedal(σ) + w·normal(σ)`, the point the ruling
/// through σ passes through.
fn base_curve<B: Backend>(chart: &Chart<B>, w: &Rat<B>) -> Vec3Rat<B> {
    chart.pedal().add(&chart.normal().scale_rat(w))
}

/// Every place the ray meets the chart's ruled surface over `span`, **ordered by ray parameter**.
///
/// The ray meets the ruling at σ exactly where the two lines are coplanar, so the hits are the roots
/// of `g(σ) = det[base(σ) − origin, ruling(σ), dir]` — rational in σ, isolated to rationals by
/// [`scan_roots`]. Each root then fixes `(µ̂, t)` by solving the two best-conditioned of the three
/// coordinate equations; the third is not discarded silently but becomes the residual that
/// [`pick_frame`] measures.
///
/// **The ordering is only as complete as the scan.** `scan` owns the density, and a double root or
/// two roots inside one cell are invisible to it (see the module docs). The hits returned are real;
/// that they are *all* of them is the caller's precondition.
pub fn ray_hits<B: Backend>(
    chart: &Chart<B>,
    w: &Rat<B>,
    ray: &Ray<B>,
    span: &Interval<B>,
    scan: usize,
    iters: usize,
) -> Result<Vec<Hit<B>>, PickFault> {
    use core::cmp::Ordering;
    if is_zero3(&ray.dir) {
        return Err(PickFault::DegenerateRay);
    }
    if span.lo.cmp(&span.hi) != Ordering::Less {
        return Err(PickFault::DegenerateSpan);
    }
    let base = base_curve(chart, w);
    let dir = const_vec3(&ray.dir);
    // `g(σ) = (base(σ) − origin) · (ruling(σ) × dir)`, the coplanarity determinant.
    let g = base
        .sub(&const_vec3(&ray.origin))
        .dot(&chart.ruling().cross(&dir))
        .reduce();
    if g.is_zero() {
        // Identically coplanar: the ray lies in the ruled surface's own family of ruling planes, so
        // there is no isolated hit to pick.
        return Err(PickFault::RayAlongRuling);
    }
    let roots = scan_roots(&g, &span.lo, &span.hi, scan, iters).ok_or(PickFault::PoleInScan)?;

    let mut hits: Vec<Hit<B>> = Vec::with_capacity(roots.len());
    for sigma in roots {
        match solve_hit(chart, w, ray, &sigma) {
            // A **ray**, not a line: the surface may well continue behind the caster, but a drill
            // does not cut backwards, so `t < 0` is not a hit.
            Some(hit) if hit.t.sign() >= 0 => hits.push(hit),
            _ => continue,
        }
    }
    hits.sort_by(|a, b| a.t.cmp(&b.t));
    Ok(hits)
}

/// Fix `(µ̂, t)` at a given σ: `base(σ) + µ̂·ruling(σ) = origin + t·dir` is three equations in two
/// unknowns, so this solves the coordinate pair with the largest `|det[r, −dir]|` — the
/// best-conditioned 2×2, and zero for all three pairs exactly when the ruling is parallel to the
/// ray. The third equation is not discarded silently: it becomes the residual [`pick_frame`]
/// measures.
///
/// `None` when the ruling is parallel to the ray (no isolated crossing) or a chart field poles.
fn solve_hit<B: Backend>(
    chart: &Chart<B>,
    w: &Rat<B>,
    ray: &Ray<B>,
    sigma: &Rat<B>,
) -> Option<Hit<B>> {
    use core::cmp::Ordering;
    let (b, r) = (
        base_curve(chart, w).eval(sigma)?,
        chart.ruling().eval(sigma)?,
    );
    let rhs = sub3(&ray.origin, &b);
    let mut best: Option<(Rat<B>, usize, usize)> = None;
    for (i, j) in [(0, 1), (1, 2), (2, 0)] {
        let det = r[j].mul(&ray.dir[i]).sub(&r[i].mul(&ray.dir[j]));
        let mag = if det.sign() < 0 {
            det.neg()
        } else {
            det.clone()
        };
        if best
            .as_ref()
            .is_none_or(|(m, _, _)| m.cmp(&mag) == Ordering::Less)
        {
            best = Some((mag, i, j));
        }
    }
    let (mag, i, j) = best?;
    if mag.sign() <= 0 {
        return None;
    }
    let det = r[j].mul(&ray.dir[i]).sub(&r[i].mul(&ray.dir[j]));
    let mu = rhs[i]
        .mul(&ray.dir[j])
        .sub(&rhs[j].mul(&ray.dir[i]))
        .div(&det.neg());
    let t = r[i].mul(&rhs[j]).sub(&r[j].mul(&rhs[i])).div(&det);
    Some(Hit {
        sigma: sigma.clone(),
        mu,
        t,
    })
}

/// Build the frame at a hit and certify it by backward error.
///
/// The frame is the one `docs/cutter-extrude-design.md` §5 describes: **origin** at the hit,
/// **normal** per [`Facing`], and in-plane orientation taken from the local ruling. Every one of
/// those comes from the chart's own rational fields at a rational σ, so the frame is exact and the
/// origin is exactly on the surface; the only error is that σ is not quite the algebraic hit, and
/// that shows up as the origin's distance to the ray.
///
/// Returns `Verified` with the frame and its [`ValidPick`] when `eps < clearance/2`,
/// `Unresolved(eps)` when the search was too coarse for the clearance (scan or bisect harder), and
/// `Refuted` for a degenerate ray, a grazing pick, or a frame with no orientation.
///
/// The frame is orthogonal but **not** orthonormal: `v = n × u` has `|v| = |n|·|u|`, and rescaling
/// to fix that needs `|n|`, which is generally irrational. [`Frame::metric`] reports it, and a
/// caller that needs a metric-circle profile should read it rather than assume.
#[allow(clippy::too_many_arguments)]
pub fn pick_frame<B: Backend>(
    chart: &Chart<B>,
    w: &Rat<B>,
    ray: &Ray<B>,
    hit: &Hit<B>,
    facing: Facing,
    clearance: &Rat<B>,
    sqrt_eps: &Rat<B>,
) -> Verdict<(Frame<B>, ValidPick<B>), PickFault, Rat<B>> {
    use core::cmp::Ordering;
    if is_zero3(&ray.dir) {
        return Verdict::Refuted(PickFault::DegenerateRay);
    }
    let (Some(b), Some(r), Some(n)) = (
        base_curve(chart, w).eval(&hit.sigma),
        chart.ruling().eval(&hit.sigma),
        chart.normal().eval(&hit.sigma),
    ) else {
        return Verdict::Refuted(PickFault::PoleInScan);
    };
    // Exactly on the surface: the chart's own fields at a rational σ.
    let origin = add3(&b, &scale3(&r, &hit.mu));

    let nd = dot3(&n, &ray.dir);
    let normal = match facing {
        Facing::Ray => ray.dir.clone(),
        Facing::SurfaceToward | Facing::SurfaceAway => {
            if nd.is_zero() {
                // The surface normal is perpendicular to the ray: the cast is tangent here and
                // "toward" / "away" name nothing.
                return Verdict::Refuted(PickFault::Grazing);
            }
            let want = if facing == Facing::SurfaceToward {
                -1
            } else {
                1
            };
            if nd.sign() == want {
                n.clone()
            } else {
                scale3(&n, &Rat::from_i128(-1))
            }
        }
    };

    // `u` is the ruling with its normal component removed — rational, since scaling by `n·n`
    // avoids ever normalizing. It vanishes exactly when the ruling is parallel to the normal.
    let u = sub3(
        &scale3(&r, &dot3(&normal, &normal)),
        &scale3(&normal, &dot3(&r, &normal)),
    );
    if is_zero3(&u) {
        return Verdict::Refuted(PickFault::Degenerate(ExtrudeFault::DegenerateFrame));
    }
    let v = cross3(&normal, &u);
    let frame = match Frame::new(origin.clone(), u.clone(), v) {
        Ok(f) => f,
        Err(e) => return Verdict::Refuted(PickFault::Degenerate(e)),
    };

    // The backward error: how far the frame's origin is from **the point at ray parameter `t`** —
    // not merely from the ray's line. The distinction matters and is not pedantic: `t` is what a
    // span orders hits by, and a `t` of the wrong sign puts the point on the same line, so a
    // distance-to-line bound would certify it happily. This residual is `≥` that one and pins `t`.
    let off = sub3(&origin, &add3(&ray.origin, &scale3(&ray.dir, &hit.t)));
    let eps = sqrt(&dot3(&off, &off), sqrt_eps).hi().clone();

    // The reported skew: `sin²` between `u` and the ruling. Exactly zero when the ruling already
    // lies in the frame plane, which is both surface facings.
    let (uu, rr, ur) = (dot3(&u, &u), dot3(&r, &r), dot3(&u, &r));
    let skew_sin2 = Rat::from_i128(1).sub(&ur.mul(&ur).div(&uu.mul(&rr)));

    let half = clearance.mul(&Rat::new(1, 2));
    if eps.cmp(&half) == Ordering::Less {
        Verdict::Verified((
            frame,
            ValidPick {
                eps,
                skew_sin2,
                clearance: clearance.clone(),
                hit: hit.clone(),
            },
        ))
    } else {
        Verdict::Unresolved(eps)
    }
}

/// Cast at the chart and certify the frame at the **`nth`** hit (0-based, ordered by ray
/// parameter) — the whole pick in one call.
///
/// `Refuted(NoHit)` when the ray meets fewer than `nth + 1` rulings in the span. Read the module
/// docs on what the ordinal does and does not guarantee: it is as complete as `scan`.
#[allow(clippy::too_many_arguments)]
pub fn pick_nth<B: Backend>(
    chart: &Chart<B>,
    w: &Rat<B>,
    ray: &Ray<B>,
    span: &Interval<B>,
    nth: usize,
    facing: Facing,
    clearance: &Rat<B>,
    sqrt_eps: &Rat<B>,
    scan: usize,
    iters: usize,
) -> Verdict<(Frame<B>, ValidPick<B>), PickFault, Rat<B>> {
    let hits = match ray_hits(chart, w, ray, span, scan, iters) {
        Ok(h) => h,
        Err(e) => return Verdict::Refuted(e),
    };
    match hits.get(nth) {
        Some(hit) => pick_frame(chart, w, ray, hit, facing, clearance, sqrt_eps),
        None => Verdict::Refuted(PickFault::NoHit),
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────
// The span — which of the surfaces the ray meets actually get cut.
// ─────────────────────────────────────────────────────────────────────────────────────────────

/// A **neutral surface** the ray can meet: a chart embedded over a σ-band, trimmed to a µ̂-band.
///
/// The unit here is a *region*, not a chart, and that distinction is load-bearing. A part carries
/// one frame and several regions differing only in their **support law** `h(σ)`, and it is the
/// support that separates a lap from the sheet it laps. Measured on the self-lapping device: the
/// wrap chart taken bare sends two different σ to the **same 3-D point** at the lap, because with
/// `h ≡ 0` the flap and the body coincide exactly. Give each region its own support and they
/// separate. So "the surfaces the ray meets" is a statement about regions; a span computed against
/// a bare chart would be counting a double cover, not layers.
pub struct Sheet<'a, B: Backend = Bignum> {
    /// The region's chart, carrying **its own support** — the field that makes a lap a lap.
    pub chart: &'a Chart<B>,
    /// The σ-band the region is authored over.
    pub sigma: Interval<B>,
    /// The µ̂-band the region is trimmed to: a crossing of the ruling *line* outside this is not a
    /// crossing of the material.
    pub mu: Interval<B>,
}

/// How deep a cut reaches, counted in [`Sheet`] crossings along the reference ray.
///
/// This counts **neutral surfaces**, not layers and not faces: cuts are authored before any stackup
/// exists (`docs/cutter-extrude-design.md` §1), so there is nothing else to count yet.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Span {
    /// The first surface only.
    ToNext,
    /// The first `k`.
    NextN(usize),
    /// Every surface the ray meets.
    Through,
    /// The half-open ordinal range `start..end`.
    Range(usize, usize),
}

/// One crossing of the material by the reference ray.
#[derive(Debug)]
pub struct Crossing<B: Backend = Bignum> {
    /// Which [`Sheet`] was crossed, by index.
    pub sheet: usize,
    /// Where, in that sheet's own parameters and along the ray.
    pub hit: Hit<B>,
}

impl<B: Backend> Clone for Crossing<B> {
    fn clone(&self) -> Self {
        Crossing {
            sheet: self.sheet,
            hit: self.hit.clone(),
        }
    }
}

/// Why a span could not be resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpanFault {
    /// The cast itself was refused.
    Pick(PickFault),
    /// The coplanarity residual's Sturm chain did not verify, so its root **count** cannot be
    /// trusted — and without a trustworthy count there is no trustworthy ordinal.
    ChainUnverified,
    /// The residual has a repeated root over the span: the ray is **tangent** to the surface
    /// somewhere, so a crossing exists that no sign change reveals and the count is not meaningful.
    Grazing,
    /// Two crossings lie closer along the ray than the clearance, so which one is "next" is not a
    /// question this tolerance can answer.
    Indistinct,
    /// The requested ordinal reaches past the surfaces the ray actually meets.
    OutOfRange,
}

/// Every crossing of the given sheets by the ray, **ordered by ray parameter**, with the ordering
/// certified rather than assumed.
///
/// Three things separate this from [`ray_hits`], and each of them is what an *ordinal* needs that a
/// position does not:
///
/// - **The count is certified.** The crossings are the roots of the coplanarity residual, isolated
///   by a **Sturm chain** whose hypothesis is checked at runtime ([`SturmChain::verify_chain`]) —
///   so no crossing is missed. A scan owns a density and can step over a double root or two roots
///   in one cell; an ordinal computed from a scan is a guess.
/// - **Tangency is refused.** A repeated root means the ray grazes, where "how many surfaces" has
///   no stable answer. Detected exactly, as `gcd(g, g′)` having positive degree.
/// - **Indistinct crossings are refused.** Two surfaces closer along the ray than `clearance`
///   cannot be ordered at that tolerance, and saying which is "next" would be fiction.
///
/// `Unresolved` carries the smallest gap found when that is what failed, so a caller can see how
/// much clearance the geometry would need.
pub fn ray_crossings<B: Backend>(
    sheets: &[Sheet<'_, B>],
    w: &Rat<B>,
    ray: &Ray<B>,
    clearance: &Rat<B>,
    iters: usize,
) -> Verdict<Vec<Crossing<B>>, SpanFault, Rat<B>> {
    use core::cmp::Ordering;
    if is_zero3(&ray.dir) {
        return Verdict::Refuted(SpanFault::Pick(PickFault::DegenerateRay));
    }
    let mut out: Vec<Crossing<B>> = Vec::new();
    for (idx, sheet) in sheets.iter().enumerate() {
        if sheet.sigma.lo.cmp(&sheet.sigma.hi) != Ordering::Less {
            return Verdict::Refuted(SpanFault::Pick(PickFault::DegenerateSpan));
        }
        let g = coplanarity(sheet.chart, w, ray);
        if g.is_zero() {
            return Verdict::Refuted(SpanFault::Pick(PickFault::RayAlongRuling));
        }
        let num = g.num();
        // A constant residual has no roots — this sheet is simply missed.
        if num.degree().unwrap_or(0) == 0 {
            continue;
        }
        let chain = SturmChain::new(num);
        if !chain.verify_chain(num) {
            return Verdict::Refuted(SpanFault::ChainUnverified);
        }
        // Squarefree over ℚ ⟺ no repeated real root anywhere, so this refuses a graze without
        // having to localize it. Conservative in the right direction.
        if num.gcd(&num.derivative()).degree().unwrap_or(0) > 0 {
            return Verdict::Refuted(SpanFault::Grazing);
        }
        for iv in chain.isolate(&sheet.sigma) {
            let sigma = match refine_root(num, &iv, iters) {
                Some(s) => s,
                None => return Verdict::Refuted(SpanFault::Grazing),
            };
            match solve_hit(sheet.chart, w, ray, &sigma) {
                // Two filters, both exact. A crossing of the ruling *line* outside the region's
                // µ̂-trim is not a crossing of the material; and one behind the caster is not on the
                // ray at all — the surface may continue there, but a cut does not.
                Some(hit)
                    if hit.t.sign() >= 0
                        && hit.mu.cmp(&sheet.mu.lo) != Ordering::Less
                        && hit.mu.cmp(&sheet.mu.hi) != Ordering::Greater =>
                {
                    out.push(Crossing { sheet: idx, hit })
                }
                _ => continue, // parallel ruling, behind the caster, or outside the trim
            }
        }
    }
    out.sort_by(|a, b| a.hit.t.cmp(&b.hit.t));

    // The ordering has to be *resolvable*: two surfaces nearer than the clearance are not two
    // surfaces at this tolerance.
    let mut min_gap: Option<Rat<B>> = None;
    for pair in out.windows(2) {
        let gap = pair[1].hit.t.sub(&pair[0].hit.t);
        // `t` is measured in units of `|dir|`, so scale to true distance.
        let gap = gap.mul(
            &sqrt(&dot3(&ray.dir, &ray.dir), &Rat::new(1, 1 << 30))
                .lo()
                .clone(),
        );
        if min_gap
            .as_ref()
            .is_none_or(|m| gap.cmp(m) == Ordering::Less)
        {
            min_gap = Some(gap);
        }
    }
    if let Some(gap) = min_gap {
        if gap.cmp(clearance) != Ordering::Greater {
            return Verdict::Unresolved(gap);
        }
    }
    Verdict::Verified(out)
}

/// Bisect the single root inside an isolating interval to a rational. `None` if the polynomial does
/// not change sign across it, which for an isolated *simple* root cannot happen — so it signals a
/// multiplicity the squarefree check should already have caught.
fn refine_root<B: Backend>(p: &Poly<B>, iv: &Interval<B>, iters: usize) -> Option<Rat<B>> {
    let (mut lo, mut hi) = (iv.lo.clone(), iv.hi.clone());
    let (slo, shi) = (p.eval(&lo).sign(), p.eval(&hi).sign());
    if slo == 0 {
        return Some(lo);
    }
    if shi == 0 {
        return Some(hi);
    }
    if slo == shi {
        return None;
    }
    let two = Rat::from_i128(2);
    for _ in 0..iters {
        let mid = lo.add(&hi).div(&two);
        let s = p.eval(&mid).sign();
        if s == 0 {
            return Some(mid);
        }
        if s == slo {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Some(lo.add(&hi).div(&two))
}

/// Select the crossings a [`Span`] reaches.
pub fn select<B: Backend>(
    span: Span,
    crossings: &[Crossing<B>],
) -> Result<&[Crossing<B>], SpanFault> {
    let n = crossings.len();
    let (a, b) = match span {
        Span::ToNext => (0, 1),
        Span::NextN(k) => (0, k),
        Span::Through => (0, n),
        Span::Range(a, b) => (a, b),
    };
    if a > b || b > n {
        return Err(SpanFault::OutOfRange);
    }
    Ok(&crossings[a..b])
}

/// The residual `g(σ)` whose roots are the hits — exposed so a caller can count them independently
/// of [`ray_hits`]'s scan, which is what certifying an **ordinal** would need.
pub fn coplanarity<B: Backend>(chart: &Chart<B>, w: &Rat<B>, ray: &Ray<B>) -> RatFunc<B> {
    base_curve(chart, w)
        .sub(&const_vec3(&ray.origin))
        .dot(&chart.ruling().cross(&const_vec3(&ray.dir)))
        .reduce()
}

#[cfg(test)]
mod tests {
    use super::*;
    use fixtures::devices::cone;
    use lattice::Bignum;

    type Q = Rat<Bignum>;

    fn q(n: i128) -> Q {
        Q::from_i128(n)
    }

    fn to_f64(r: &Q) -> f64 {
        let (n, d) = r.numer_denom_decimal();
        n.parse::<f64>().unwrap_or(f64::NAN) / d.parse::<f64>().unwrap_or(f64::NAN)
    }

    fn span(lo: i128, hi: i128) -> Interval<Bignum> {
        Interval {
            lo: q(lo),
            hi: q(hi),
        }
    }

    fn tag<T>(v: &Verdict<T, PickFault, Q>) -> String {
        match v {
            Verdict::Verified(_) => "Verified".into(),
            Verdict::Refuted(f) => format!("Refuted({f:?})"),
            Verdict::Unresolved(e) => format!("Unresolved({})", to_f64(e)),
        }
    }

    /// A ray that crosses the device cone **twice** — along `+x` at `z = 3`, entering and leaving
    /// through the two rulings at `σ = ∓1`. Two hits are what makes an ordering claim non-vacuous.
    fn probe_ray() -> Ray<Bignum> {
        Ray {
            origin: [q(-5), q(0), q(3)],
            dir: [q(1), q(0), q(0)],
        }
    }

    /// The pick is exact where it can be: the frame's origin is **on the surface**, and its axes are
    /// the chart's own fields — so the only error left to measure is the ray's.
    #[test]
    fn a_picked_frame_sits_exactly_on_the_surface() {
        let chart = cone();
        let ray = probe_ray();
        let zero = q(0);
        let hits = ray_hits(&chart, &zero, &ray, &span(-3, 3), 64, 48).expect("a real cast");
        assert!(!hits.is_empty(), "the ray must meet the cone");
        for hit in &hits {
            let b = chart.pedal().eval(&hit.sigma).unwrap();
            let r = chart.ruling().eval(&hit.sigma).unwrap();
            let p = add3(&b, &scale3(&r, &hit.mu));
            // On the ruled surface by construction — an exact rational identity, not a bound.
            let back = chart.surface(&hit.mu, &zero).eval(&hit.sigma).unwrap();
            for k in 0..3 {
                assert!(
                    p[k].sub(&back[k]).is_zero(),
                    "the hit must lie on the chart's own surface exactly"
                );
            }
        }
    }

    /// The certificate measures the one thing that is not exact — how far the frame's origin is
    /// from the ray's own point at parameter `t` — and the surface facings put the ruling in the
    /// frame plane exactly.
    #[test]
    fn the_backward_error_pins_the_point_not_just_the_line() {
        let chart = cone();
        let ray = probe_ray();
        let (zero, sq) = (q(0), Q::new(1, 1 << 40));
        let hits = ray_hits(&chart, &zero, &ray, &span(-3, 3), 64, 48).expect("a real cast");
        for facing in [Facing::SurfaceToward, Facing::SurfaceAway, Facing::Ray] {
            match pick_frame(&chart, &zero, &ray, &hits[0], facing, &q(1), &sq) {
                Verdict::Verified((frame, pick)) => {
                    assert!(
                        pick.eps.cmp(&Q::new(1, 1_000_000)) == core::cmp::Ordering::Less,
                        "{facing:?}: ε = {}",
                        to_f64(&pick.eps)
                    );
                    // The frame's origin is the hit, and the frame is orthogonal — but not
                    // orthonormal, which it reports rather than hides.
                    let m = frame.metric();
                    assert!(m.uv.is_zero(), "the picked axes are perpendicular");
                    // The origin really is the ray's point at `t`, componentwise within ε — the
                    // claim a distance-to-line bound could not make.
                    let want = add3(&ray.origin, &scale3(&ray.dir, &pick.hit.t));
                    for (k, w) in want.iter().enumerate() {
                        let d = frame.origin()[k].sub(w);
                        let d = if d.sign() < 0 { d.neg() } else { d };
                        assert!(
                            d.cmp(&pick.eps) != core::cmp::Ordering::Greater,
                            "axis {k} off by {} > ε = {}",
                            to_f64(&d),
                            to_f64(&pick.eps)
                        );
                    }
                    if facing != Facing::Ray {
                        assert!(
                            pick.skew_sin2.is_zero(),
                            "a surface facing puts the ruling in the frame plane exactly"
                        );
                    }
                }
                other => panic!("{facing:?}: expected Verified, got {}", tag(&other)),
            }
        }
        // Facing::Ray tilts the frame away from the ruling, and says by how much.
        let by_ray = match pick_frame(&chart, &zero, &ray, &hits[0], Facing::Ray, &q(1), &sq) {
            Verdict::Verified((_, p)) => p.skew_sin2,
            other => panic!("expected Verified, got {}", tag(&other)),
        };
        assert!(
            by_ray.sign() > 0,
            "the ruling does not lie perpendicular to this ray"
        );
    }

    /// **The searcher is disposable.** Degrade it — hand the certificate a σ that is deliberately
    /// off the true hit — and ε grows with the damage, the DRC refuses at a tight clearance, and no
    /// wrong `Verified` is ever issued. Nothing about the guarantee depends on how σ was found.
    #[test]
    fn the_certificate_survives_a_degraded_searcher() {
        let chart = cone();
        let ray = probe_ray();
        let (zero, sq) = (q(0), Q::new(1, 1 << 40));
        let hits = ray_hits(&chart, &zero, &ray, &span(-3, 3), 64, 48).expect("a real cast");
        let good = &hits[0];

        let mut last = q(0);
        for damage in [
            Q::new(1, 10_000),
            Q::new(1, 1_000),
            Q::new(1, 100),
            Q::new(1, 10),
        ] {
            let bad = Hit {
                sigma: good.sigma.add(&damage),
                mu: good.mu.clone(),
                t: good.t.clone(),
            };
            // A clearance loose enough that the certificate reports rather than refuses.
            let eps = match pick_frame(
                &chart,
                &zero,
                &ray,
                &bad,
                Facing::SurfaceToward,
                &q(100),
                &sq,
            ) {
                Verdict::Verified((_, p)) => p.eps,
                other => panic!("expected a reported ε, got {}", tag(&other)),
            };
            assert!(
                eps.cmp(&last) == core::cmp::Ordering::Greater,
                "ε must grow with the damage: {} after {}",
                to_f64(&eps),
                to_f64(&last)
            );
            last = eps.clone();

            // And the DRC tracks ε exactly, at every damage level: a clearance comfortably above
            // `2ε` certifies, one at `ε` (so `clearance/2 = ε/2 < ε`) refuses. Pinning the gate to
            // the reported ε is sharper than any fixed threshold — a damage small enough to stay
            // inside the clearance *should* still certify, and does.
            let at = |c: Q| pick_frame(&chart, &zero, &ray, &bad, Facing::SurfaceToward, &c, &sq);
            assert!(
                matches!(at(eps.mul(&q(4))), Verdict::Verified(_)),
                "ε = {} must certify at a clearance of 4ε",
                to_f64(&eps)
            );
            assert!(
                matches!(at(eps.clone()), Verdict::Unresolved(_)),
                "ε = {} must be refused at a clearance of ε",
                to_f64(&eps)
            );
        }
        // The undamaged pick still certifies at that same tight clearance.
        assert!(matches!(
            pick_frame(
                &chart,
                &zero,
                &ray,
                good,
                Facing::SurfaceToward,
                &Q::new(1, 1000),
                &sq
            ),
            Verdict::Verified(_)
        ));
    }

    /// Hits come back ordered by ray parameter, which is what an ordinal reads.
    #[test]
    fn hits_are_ordered_along_the_ray() {
        let chart = cone();
        let ray = probe_ray();
        let hits = ray_hits(&chart, &q(0), &ray, &span(-3, 3), 64, 48).expect("a real cast");
        assert_eq!(
            hits.len(),
            2,
            "the ordering claim needs a ray that crosses twice"
        );
        for pair in hits.windows(2) {
            assert!(
                pair[0].t.cmp(&pair[1].t) != core::cmp::Ordering::Greater,
                "hits must be sorted by ray parameter"
            );
        }
        // Corroboration the ordering alone would not give: this ray is aimed straight at the cone's
        // axis, so its two crossings are symmetric about `x = 0` and their parameters must sum to
        // exactly twice the distance from the caster — `2·5 = 10`, in exact rationals. A `t` of the
        // wrong sign or scale fails here even though both hits lie on the ray's line.
        assert!(
            hits[0].t.add(&hits[1].t).sub(&q(10)).is_zero(),
            "t₀ + t₁ = {} + {} should be exactly 10",
            to_f64(&hits[0].t),
            to_f64(&hits[1].t)
        );
        // `pick_nth` reads that order, and refuses an ordinal the cast does not reach.
        let sq = Q::new(1, 1 << 40);
        assert!(matches!(
            pick_nth(
                &chart,
                &q(0),
                &ray,
                &span(-3, 3),
                hits.len(),
                Facing::SurfaceToward,
                &q(1),
                &sq,
                64,
                48
            ),
            Verdict::Refuted(PickFault::NoHit)
        ));
    }

    /// The acceptance device's three regions, as charts carrying their own support laws.
    fn lap_regions() -> Vec<Chart<Bignum>> {
        use fixtures::devices::cone_wrap;
        use lattice::{Poly, RatFunc};
        let qn = cone_wrap().quaternion().clone();
        let d = Q::new(1, 10);
        // smoothstep h(u) = d·(3u² − 2u³) on u = 2σ − 1.
        let u = Poly::from_coeffs(vec![q(-1), q(2)]);
        let u2 = u.mul(&u);
        let ramp = u2.scale(&q(3)).sub(&u2.mul(&u).scale(&q(2))).scale(&d);
        [
            RatFunc::from_poly(Poly::constant(q(0))),
            RatFunc::from_poly(ramp),
            RatFunc::from_poly(Poly::constant(d)),
        ]
        .into_iter()
        .map(|h| Chart::new(qn.clone(), h))
        .collect()
    }

    /// The device's three regions as [`Sheet`]s over their authored σ-bands, trimmed to a µ̂-window
    /// wide enough to hold the whole annulus.
    fn lap_sheets(charts: &[Chart<Bignum>]) -> Vec<Sheet<'_, Bignum>> {
        let bands = [
            (Q::new(-5, 4), Q::new(1, 2)),
            (Q::new(1, 2), q(1)),
            (q(1), Q::new(5, 4)),
        ];
        charts
            .iter()
            .zip(bands.iter())
            .map(|(c, (lo, hi))| Sheet {
                chart: c,
                sigma: Interval {
                    lo: lo.clone(),
                    hi: hi.clone(),
                },
                mu: Interval {
                    lo: q(-8),
                    hi: q(8),
                },
            })
            .collect()
    }

    fn span_tag<T>(v: &Verdict<T, SpanFault, Q>) -> String {
        match v {
            Verdict::Verified(_) => "Verified".into(),
            Verdict::Refuted(f) => format!("Refuted({f:?})"),
            Verdict::Unresolved(g) => format!("Unresolved({})", to_f64(g)),
        }
    }

    /// **The named AUTH.1d criterion.** On the self-lapping device, a ray through the lap meets the
    /// flap and then the body — *the same chart twice*, at two different support laws — so `ToNext`
    /// cuts the flap only while `NextN(2)` and `Through` cut flap **and** body.
    ///
    /// No new fixture: the geometry is the acceptance device's own, so this measures span semantics
    /// rather than re-testing the device. What it pins that a layer index could not:
    ///
    /// - both crossings come from **one** chart, distinguished only by which region's support law
    ///   they carry — a model that counted charts, or faces, would see one surface here;
    /// - the ordering is *physical*: the flap is nearer the caster than the body by exactly the
    ///   ramp's separation, and the span reads that from the ray parameter, not from σ (whose
    ///   order is the reverse);
    /// - the far wall of the cone, which the same *line* meets at `t < 0`, is not counted, because
    ///   a cut does not reach behind the caster.
    #[test]
    fn the_lap_is_two_surfaces_and_the_span_counts_them() {
        let charts = lap_regions();
        let sheets = lap_sheets(&charts);
        // Straight down the acceptance device's seam-drill axis, starting between the cone's two
        // walls so the cast meets the lap and nothing else.
        let ray = Ray {
            origin: [Q::new(-1, 2), Q::new(27, 10), q(0)],
            dir: [q(0), q(0), q(-1)],
        };
        let crossings = match ray_crossings(&sheets, &q(0), &ray, &Q::new(1, 100), 60) {
            Verdict::Verified(c) => c,
            other => panic!("expected certified crossings, got {}", span_tag(&other)),
        };
        assert_eq!(crossings.len(), 2, "the lap is two surfaces, not one");

        // The flap first, then the body — one chart, two regions, ordered by the ray.
        assert_eq!(crossings[0].sheet, 2, "the flap (ramped support) is nearer");
        assert_eq!(
            crossings[1].sheet, 0,
            "the body (flat support) is behind it"
        );
        assert!(
            crossings[0].hit.t.cmp(&crossings[1].hit.t) == core::cmp::Ordering::Less,
            "ordering is by ray parameter"
        );
        // σ runs the other way, so an ordinal read off σ would invert the two.
        assert!(
            crossings[0].hit.sigma.cmp(&crossings[1].hit.sigma) == core::cmp::Ordering::Greater,
            "the σ order is the reverse of the physical order — which is the point"
        );

        // The span semantics themselves.
        let sheets_of = |sp: Span| {
            select(sp, &crossings)
                .expect("in range")
                .iter()
                .map(|c| c.sheet)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            sheets_of(Span::ToNext),
            vec![2],
            "ToNext cuts the flap only"
        );
        assert_eq!(
            sheets_of(Span::NextN(2)),
            vec![2, 0],
            "NextN(2) cuts flap and body"
        );
        assert_eq!(
            sheets_of(Span::Through),
            vec![2, 0],
            "Through cuts flap and body"
        );
        assert_eq!(
            sheets_of(Span::Range(1, 2)),
            vec![0],
            "Range skips the flap"
        );
        assert_eq!(
            select(Span::NextN(3), &crossings).err(),
            Some(SpanFault::OutOfRange),
            "a span reaching past the material is refused, not clamped"
        );
    }

    /// The two searchers agree. `ray_hits` scans and bisects; `ray_crossings` isolates with a
    /// verified Sturm chain — independent routes to the same roots, so agreement is evidence and
    /// disagreement would be a bug in one of them. Only the Sturm route can *promise* completeness,
    /// which is why the ordinal is built on it, but the scan is a real second opinion on the values.
    #[test]
    fn the_sturm_and_scan_searchers_find_the_same_crossings() {
        let charts = lap_regions();
        let sheets = lap_sheets(&charts);
        let ray = Ray {
            origin: [Q::new(-1, 2), Q::new(27, 10), q(0)],
            dir: [q(0), q(0), q(-1)],
        };
        let certified = match ray_crossings(&sheets, &q(0), &ray, &Q::new(1, 100), 60) {
            Verdict::Verified(c) => c,
            other => panic!("expected Verified, got {}", span_tag(&other)),
        };
        for c in &certified {
            let sheet = &sheets[c.sheet];
            let scanned = ray_hits(sheet.chart, &q(0), &ray, &sheet.sigma, 256, 60)
                .expect("the scan searcher runs on the same sheet");
            // Some σ within a bisection step of the certified one must appear in the scan's list.
            let near = scanned.iter().any(|h| {
                let d = h.sigma.sub(&c.hit.sigma);
                let d = if d.sign() < 0 { d.neg() } else { d };
                d.cmp(&Q::new(1, 1_000_000)) == core::cmp::Ordering::Less
            });
            assert!(
                near,
                "the scan missed the certified crossing at σ = {}",
                to_f64(&c.hit.sigma)
            );
        }
    }

    /// Fail-closed: two surfaces closer along the ray than the clearance are not two surfaces at
    /// that tolerance, and the span says so instead of picking an order. The lap's own separation
    /// is the ramp height, so a clearance above it makes the very same cast `Unresolved`.
    #[test]
    fn surfaces_closer_than_the_clearance_are_not_ordered() {
        let charts = lap_regions();
        let sheets = lap_sheets(&charts);
        let ray = Ray {
            origin: [Q::new(-1, 2), Q::new(27, 10), q(0)],
            dir: [q(0), q(0), q(-1)],
        };
        // The measured gap is ≈ 0.149; a clearance of 1/4 exceeds it.
        match ray_crossings(&sheets, &q(0), &ray, &Q::new(1, 4), 60) {
            Verdict::Unresolved(gap) => assert!(
                gap.cmp(&Q::new(1, 10)) == core::cmp::Ordering::Greater
                    && gap.cmp(&Q::new(1, 4)) == core::cmp::Ordering::Less,
                "the reported gap should be the lap's own separation, got {}",
                to_f64(&gap)
            ),
            other => panic!("expected Unresolved, got {}", span_tag(&other)),
        }
    }

    #[test]
    fn degenerate_casts_are_refused() {
        let chart = cone();
        let zero = q(0);
        let dead = Ray {
            origin: [q(0), q(4), q(3)],
            dir: [q(0), q(0), q(0)],
        };
        assert_eq!(
            ray_hits(&chart, &zero, &dead, &span(-3, 3), 32, 32).err(),
            Some(PickFault::DegenerateRay)
        );
        assert_eq!(
            ray_hits(&chart, &zero, &probe_ray(), &span(3, 3), 32, 32).err(),
            Some(PickFault::DegenerateSpan)
        );
        // A ray through the apex along a ruling is coplanar with every ruling it meets in the
        // pencil, so there is no isolated hit — refused rather than picked arbitrarily.
        let along = Ray {
            origin: [q(0), q(0), q(0)],
            dir: chart.ruling().eval(&q(1)).unwrap(),
        };
        assert_eq!(
            ray_hits(&chart, &zero, &along, &span(-3, 3), 32, 32).err(),
            Some(PickFault::RayAlongRuling)
        );
    }
}
