//! Reconstruct the exact **B-rep** of a certified one-joint closure — the surface-tier
//! analogue of [`crate::shell`]'s triangle soup, and the exact geometry the STEP surface
//! bridge consumes (spec §10, exact ruled faces; not the §11 mesh).
//!
//! [`brep_from_closure`] emits each flank as an **exact ruled face at the fold crease**
//! (`w = 0`, the neutral sheet), the two flanks sharing the crease line `L` **by
//! identity**. The crease is where the certificate's MITER-FIT licenses a shared edge:
//! on the flipped support (see `fixtures::closure_joint::boxes`) each flank's `w = 0`
//! neutral sheet is on the *retained* side of the bisector plane Π, touching Π only at
//! the crease, so the two flanks' crease rulings coincide on `L`. The overlap of the two
//! crease rulings is one **shared edge** `M` referenced by both flank wires; the part of
//! the wider flank's ruling that sticks out past the narrower one's is an **honestly-open
//! overhang tip** — a free edge, the expected certified-seam / honest-open outcome.
//!
//! Watertight-by-identity, not by tolerance: two faces meet along `M` because both wires
//! reference the *same* edge id (and its two endpoints are the *same* vertex ids,
//! deduplicated by exact rational coordinates). No float, no coincidence test — the
//! exact→`f64` cast happens later, in the feature-gated [`step`](crate::step) bridge. This
//! module is **always compiled** and float-free, like [`crate::shell`].
//!
//! # Scope (M-D slice 3, cylinder-first)
//!
//! Each flank is emitted as a [`LinearExtrusion`](crate::brep::FaceSurface::LinearExtrusion)
//! of its `μ = μ⁻` rail along the (constant) ruling direction — exact for a **cylinder**
//! flank, whose rulings share one direction. A cone flank (rulings converging on an apex)
//! needs the full rational-patch path and is deferred. The fixture's crease lies on the
//! line `L ∥ x̂`; the `μ⁻`/`μ⁺` rails bound the retained ruling band, and `μ⁻` maps to the
//! low-`x` crease corner (debug-asserted).
//!
//! # Example
//!
//! ```
//! use certify_core::Verdict;
//! use closure::valid::closure_valid;
//! use export::brep_build::brep_from_closure;
//! use fixtures::closure_joint::{miter_cap, one_joint, treatment_miter};
//!
//! let joint = one_joint();
//! let cap = miter_cap();
//! let t = treatment_miter(&cap);
//! let valid = match closure_valid(&joint, &t) {
//!     Verdict::Verified(v) => v,
//!     other => panic!("the 90° fold is CLOSURE_VALID: {}", matches!(other, Verdict::Verified(_))),
//! };
//! let brep = brep_from_closure(&joint, &t, &valid);
//! // Two flank faces, both closed wires, exactly one shared (2-incidence) crease edge.
//! assert_eq!(brep.faces().len(), 2);
//! assert_eq!(brep.edge_incidence().iter().filter(|&&c| c == 2).count(), 1);
//! assert_eq!(brep.nonmanifold_edges(), 0);
//! ```

use crate::bezier::{RatBezier, RatBezierSurface, poly_to_bernstein};
use crate::brep::{Brep, EdgeGeom, FaceSurface, HalfEdge};
use arrange2d::boolean::{BoolOp, OperandId, ledge_dom_certified};
use certify_core::MarginSq;
use certify_core::Verdict;
use certify_core::certify1d::{EdgeRegCert, RegCert};
use certify_core::free_boundary::FreeBoundaryCert;
use closure::valid::{CapWitness, ClosureTreatment, ClosureValid};
use closure::{Joint, MuRange};
use geom::chart::Chart;
use geom::content::{CurveId, Edge as ArrEdge, Line, Orient, Point2, SegPiece};
use lattice::{Backend, Bignum, Interval, Poly, Rat, RatFunc, SturmChain, Surd, Vec3Rat};

/// The crease edge two flanks share: the overlap sub-segment `M` (its edge id and the two
/// endpoint vertex ids), computed as the intersection of the two flanks' crease rulings.
struct SharedCrease {
    /// The vertex id of the shared middle's low-`x` endpoint (larger of the two flanks' low corners).
    s_lo_v: usize,
    /// The vertex id of the shared middle's high-`x` endpoint (smaller of the two flanks' high corners).
    s_hi_v: usize,
    /// The shared edge id (`s_lo_v → s_hi_v`, a straight ruling on `L`).
    m_eid: usize,
}

/// The incremental exact-B-rep builder: the [`Brep`] under construction plus a vertex
/// **dedup table** keyed by exact rational coordinates, so a point emitted by two flanks
/// (the shared crease corners) becomes one vertex id — the identity the watertight seam
/// rides on.
struct Builder<B: Backend> {
    brep: Brep<B>,
    verts: Vec<([Rat<B>; 3], usize)>,
    /// Edge **dedup table** keyed by `(v_min, v_max, kind)` — the undirected endpoint pair and a
    /// geometry-kind tag (`0` = [`Line`](EdgeGeom::Line), `1` = [`RationalBezier`](EdgeGeom::RationalBezier)).
    /// An edge two faces share (a lid and its tube wall, two adjacent σ-slices' lids) is emitted
    /// once and found again by exact identity — the sole source of watertight seams in the holed
    /// construction, the edge-level analogue of [`verts`](Self::verts)' coordinate dedup.
    edge_keys: Vec<(usize, usize, u8, usize)>,
}

/// A rational vertex lifted into [`Surd`] components (the `b = 0` case), as [`crate::shell`].
fn vert<B: Backend>(p: &[Rat<B>; 3]) -> [Surd<B>; 3] {
    [
        Surd::from_rat(p[0].clone()),
        Surd::from_rat(p[1].clone()),
        Surd::from_rat(p[2].clone()),
    ]
}

/// Build one closed polyline loop of straight [`Line`](EdgeGeom::Line) edges through `pts` (in
/// order, closing last→first) into `brep`, returning its wire of forward half-edges. Each point
/// becomes a fresh vertex — a simple polyline loop revisits none, so no dedup is needed.
fn polyline_loop<B: Backend>(brep: &mut Brep<B>, pts: &[[Rat<B>; 3]]) -> Vec<HalfEdge> {
    let v: Vec<usize> = pts.iter().map(|p| brep.add_vertex(vert(p))).collect();
    let n = v.len();
    (0..n)
        .map(|i| (brep.add_edge(v[i], v[(i + 1) % n], EdgeGeom::Line), false))
        .collect()
}

/// Assemble one exact B-rep face on `surface` whose outer boundary is the polyline `outer` and
/// which carries one interior hole per loop in `holes` — the straight-chord
/// ([`Line`](EdgeGeom::Line)) wires of a folded flat pattern (G4's `develop::fold::FoldedWire`
/// boxes collapsed to their rational midpoints).
///
/// Every loop is closed last→first, and the outer and hole loops reference disjoint vertices and
/// edges, so the result is an honestly *open* holed sheet (all boundary edges free) — the STEP-II
/// panel. The `surface` is caller-supplied (a cone panel is a
/// [`RationalPatch`](FaceSurface::RationalPatch) built like [`brep_freeboundary`]'s side faces);
/// this builder is surface-agnostic. No float enters — points are exact rationals lifted to
/// [`Surd`].
///
/// ```
/// use export::brep::FaceSurface;
/// use export::brep_build::brep_holed_panel;
/// use lattice::{Bignum, Rat};
///
/// let q = |n: i128| Rat::<Bignum>::from_i128(n);
/// let p = |x: i128, y: i128| [q(x), q(y), q(0)];
/// // A 4×4 plane panel with a 2×2 interior hole.
/// let outer = [p(0, 0), p(4, 0), p(4, 4), p(0, 4)];
/// let hole = [p(1, 1), p(3, 1), p(3, 3), p(1, 3)];
/// let brep = brep_holed_panel(FaceSurface::Plane, &outer, &[&hole]);
/// assert_eq!(brep.faces()[0].holes.len(), 1);
/// assert!(brep.all_loops_closed(0));
/// assert_eq!(brep.free_edges(), 8); // 4 outer + 4 hole, all free (open sheet)
/// ```
pub fn brep_holed_panel<B: Backend>(
    surface: FaceSurface<B>,
    outer: &[[Rat<B>; 3]],
    holes: &[&[[Rat<B>; 3]]],
) -> Brep<B> {
    let mut brep = Brep::new();
    let outer_wire = polyline_loop(&mut brep, outer);
    let hole_wires: Vec<Vec<HalfEdge>> =
        holes.iter().map(|h| polyline_loop(&mut brep, h)).collect();
    brep.add_face_with_holes(surface, outer_wire, hole_wires);
    brep
}

/// The exact single-span ruled [`RationalPatch`](FaceSurface::RationalPatch) between two σ-rails
/// over `[a, b]`. The caller guarantees the span is narrow enough for positive weights (the σ-domain
/// is subdivided upstream by [`sigma_splits`]); this builds one Bézier patch per positive-weight
/// slice.
fn ruled_panel<B: Backend>(
    rail0: &Vec3Rat<B>,
    rail1: &Vec3Rat<B>,
    a: &Rat<B>,
    b: &Rat<B>,
) -> FaceSurface<B> {
    FaceSurface::RationalPatch(RatBezierSurface::ruled_from_rails(rail0, rail1, a, b))
}

/// The exact single-span ruled cone patch between two σ-rails, tolerant of rails with **different
/// denominators** (a trimmed-panel lid rules between the eccentric-inner and concentric-outer rails,
/// whose reduced denominators differ). Matching denominators (a wall's two thickness rails) go
/// straight to [`ruled_panel`]; otherwise both rails are cross-multiplied to the common denominator
/// `d₀·d₁` (value-preserving; positive weights preserved — a product of positive denominators) so
/// [`RatBezierSurface::ruled_from_rails`]' shared-denominator precondition holds.
fn ruled_common<B: Backend>(
    rail0: &Vec3Rat<B>,
    rail1: &Vec3Rat<B>,
    a: &Rat<B>,
    b: &Rat<B>,
) -> FaceSurface<B> {
    let (d0, d1) = (rail0.den(), rail1.den());
    if d0 == d1 {
        return ruled_panel(rail0, rail1, a, b);
    }
    let common = d0.mul(d1);
    let over = |rail: &Vec3Rat<B>, other_den: &Poly<B>| {
        let num = rail.num();
        Vec3Rat::new(
            [
                num[0].mul(other_den),
                num[1].mul(other_den),
                num[2].mul(other_den),
            ],
            common.clone(),
        )
    };
    ruled_panel(&over(rail0, d1), &over(rail1, d0), a, b)
}

/// Normalize a rail `μ̂` to a **polynomial** (denominator 1): reduce first (the concentric D1 plane
/// rail reduces to a *constant* denominator, which `reduce` leaves in place — constants are units),
/// then fold a constant denominator into the numerator. So `c + μ̂·r + w·n` carries only the chart
/// denominator, and — crucially — the reducible D1 rail is a *low-degree* polynomial that a later
/// constant stitch-shift keeps low-degree (shifting it *before* reducing would raise it to full
/// degree). `μ̂` with a genuine non-constant denominator (not expected here) passes through reduced.
fn poly_rail<B: Backend>(mu: &RatFunc<B>) -> RatFunc<B> {
    let m = mu.reduce();
    let d = m.den();
    if d.degree().unwrap_or(0) == 0 {
        let scale = d.eval(&Rat::from_i128(0)).recip();
        RatFunc::from_poly(m.num().scale(&scale))
    } else {
        m
    }
}

/// An interior hole for [`brep_trim_solid`]: a cut developed to its **near** and **far** σ-rails
/// over `[s1, s2]` (the tangent rulings), meeting at the two σ-caps. Authored in `(σ, μ̂)`.
///
/// Each side is a **chain** of contiguous `(σ-band, rail)` pieces, the same currency the inner and
/// outer boundaries use. A single rational rail per side was enough while a hole was two fitted
/// graphs; a cut that reaches its tangent rulings is not a polynomial graph anywhere near them, so
/// the branches arrive as many short pieces instead. Every interior piece boundary becomes a
/// σ-station, so within any one slice each side is a single piece — which is what lets the slice
/// footprint stay exactly as it was.
#[derive(Clone)]
pub struct HoleRail<B: Backend = Bignum> {
    /// The near (smaller-μ̂) branch, as ordered contiguous `(band, rail)` pieces.
    pub near: Vec<(Interval<B>, RatFunc<B>)>,
    /// The far (larger-μ̂) branch, as ordered contiguous `(band, rail)` pieces.
    pub far: Vec<(Interval<B>, RatFunc<B>)>,
    /// The lower tangent-ruling σ.
    pub s1: Rat<B>,
    /// The upper tangent-ruling σ.
    pub s2: Rat<B>,
}

impl<B: Backend> HoleRail<B> {
    /// A hole whose two branches are each **one** rail over the whole σ-extent — the shape a
    /// fitted-graph hole has, and still the natural input for an analytically-known cut.
    pub fn uniform(near: RatFunc<B>, far: RatFunc<B>, s1: Rat<B>, s2: Rat<B>) -> Self {
        let band = Interval {
            lo: s1.clone(),
            hi: s2.clone(),
        };
        HoleRail {
            near: vec![(band.clone(), near)],
            far: vec![(band, far)],
            s1,
            s2,
        }
    }

    /// The single-rail view of this hole over one slice — the piece of each side covering `at`
    /// (a σ inside both the slice and the hole). `None` if either side has no piece there.
    fn at(&self, at: &Rat<B>) -> Option<HoleSlice<'_, B>> {
        Some(HoleSlice {
            near: piece_at(&self.near, at)?,
            far: piece_at(&self.far, at)?,
            s1: self.s1.clone(),
            s2: self.s2.clone(),
        })
    }

    /// The σ where this hole's chains change piece, strictly inside `(s1, s2)` — the stations a
    /// slice partition must contain for [`HoleRail::at`] to be well defined. The tangent σ
    /// themselves are excluded deliberately: a slice boundary flush with a hole's σ-cap makes
    /// OCCT's sewing see intersecting wires.
    fn interior_breaks(&self) -> Vec<Rat<B>> {
        use core::cmp::Ordering::Less;
        let mut out = Vec::new();
        for side in [&self.near, &self.far] {
            for (band, _) in side {
                for s in [&band.lo, &band.hi] {
                    if self.s1.cmp(s) == Less && s.cmp(&self.s2) == Less {
                        out.push(s.clone());
                    }
                }
            }
        }
        out
    }
}

/// One hole resolved to a single rail per side, valid over one slice.
struct HoleSlice<'a, B: Backend> {
    near: &'a RatFunc<B>,
    far: &'a RatFunc<B>,
    s1: Rat<B>,
    s2: Rat<B>,
}

/// `x` clamped into `[lo, hi]` — a slice midpoint pulled onto a hole's own σ-range, so the hole's
/// chain piece is looked up somewhere it is actually defined even when the slice reaches past it.
fn clamp_to<B: Backend>(lo: &Rat<B>, hi: &Rat<B>, x: &Rat<B>) -> Rat<B> {
    use core::cmp::Ordering::Less;
    if x.cmp(lo) == Less {
        lo.clone()
    } else if hi.cmp(x) == Less {
        hi.clone()
    } else {
        x.clone()
    }
}

/// A piecewise boundary evaluated at a σ — the piece covering it, applied. Public so the trim
/// layer can orient a hole's two branches against each other.
pub fn chain_eval<B: Backend>(pieces: &[(Interval<B>, RatFunc<B>)], at: &Rat<B>) -> Option<Rat<B>> {
    piece_at(pieces, at)?.eval(at)
}

/// The piece of a piecewise boundary covering a σ, or `None` if none does.
fn piece_at<'a, B: Backend>(
    pieces: &'a [(Interval<B>, RatFunc<B>)],
    at: &Rat<B>,
) -> Option<&'a RatFunc<B>> {
    use core::cmp::Ordering;
    pieces
        .iter()
        .find(|(iv, _)| iv.lo.cmp(at) != Ordering::Greater && at.cmp(&iv.hi) != Ordering::Greater)
        .map(|(_, mu)| mu)
}

/// [`poly_rail`] applied to every piece of a chain.
fn poly_chain<B: Backend>(chain: &[(Interval<B>, RatFunc<B>)]) -> Vec<(Interval<B>, RatFunc<B>)> {
    chain
        .iter()
        .map(|(band, mu)| (band.clone(), poly_rail(mu)))
        .collect()
}

/// The developable σ-rail `c + μ̂·r + w·n` (μ̂ a polynomial) at thickness `wl`, its μ-base reduced
/// (low-degree, positive-weight — OCCT-friendly). Same-μ̂ rails then share a denominator.
fn trim_surf<B: Backend>(
    c: &Vec3Rat<B>,
    r: &Vec3Rat<B>,
    n: &Vec3Rat<B>,
    mu: &RatFunc<B>,
    wl: &Rat<B>,
) -> Vec3Rat<B> {
    c.add(&r.scale(mu)).reduce().add(&n.scale_rat(wl))
}

/// Polynomialize (via [`poly_rail`]) then **stitch** a piecewise rail chain so adjacent pieces agree
/// *exactly* at shared σ-boundaries (a left-to-right constant-shift propagation). A solid builder's
/// exact vertex dedup needs the notch corner `μ̂_left(σ*) = μ̂_right(σ*)` byte-identical; the pieces
/// meet only to bisection precision, so each piece is shifted by that O(1e-18) residual — inside the
/// certified ε — and, being a polynomial, stays low-degree.
fn stitched_poly_chain<B: Backend>(
    chain: &[(Interval<B>, RatFunc<B>)],
) -> Vec<(Interval<B>, RatFunc<B>)> {
    let mut v: Vec<(Interval<B>, RatFunc<B>)> = chain
        .iter()
        .map(|(iv, mu)| (iv.clone(), poly_rail(mu)))
        .collect();
    for i in 1..v.len() {
        let b = v[i].0.lo.clone();
        if let (Some(p), Some(cur)) = (v[i - 1].1.eval(&b), v[i].1.eval(&b)) {
            let shift = p.sub(&cur);
            v[i].1 = v[i].1.add(&RatFunc::from_poly(Poly::constant(shift)));
        }
    }
    v
}

/// Whether the rational Bézier over `[a, b]` with denominator `den` has **all-positive weights** —
/// the exact validity condition for a rational Bézier patch/edge (a CAD kernel rejects a
/// non-positive weight: a control point at/through infinity). The weights are the Bernstein
/// coefficients of `den` over `[a, b]`; checked at `deg(den)` (degree elevation preserves Bernstein
/// positivity, so this never *under*-reports — it may only subdivide slightly more than strictly
/// needed). A strictly-positive polynomial always passes on a small enough interval.
fn positive_weights<B: Backend>(den: &Poly<B>, a: &Rat<B>, b: &Rat<B>) -> bool {
    let deg = den.degree().unwrap_or(0);
    poly_to_bernstein(den, a, b, deg)
        .iter()
        .all(|w| w.sign() > 0)
}

/// The ordered σ-stations `[a = s₀, …, s_N = b]` subdividing `[a, b]` so **every** sub-interval's
/// rational Bézier (shared denominator `den`) has positive weights — the intrinsic,
/// parametrization-independent criterion for a valid exact Bézier piece (never keyed to a specific
/// σ value). Adaptive bisection: a sub-interval failing [`positive_weights`] is split at its
/// midpoint and each half recursed. Terminates because a strictly-positive polynomial's Bernstein
/// coefficients converge to its (positive) values under subdivision; `MAX_DEPTH` is a safety cap
/// (a positive developable denominator reaches positivity in a few levels).
fn sigma_splits<B: Backend>(den: &Poly<B>, a: &Rat<B>, b: &Rat<B>) -> Vec<Rat<B>> {
    const MAX_DEPTH: usize = 32;
    fn go<B: Backend>(den: &Poly<B>, a: &Rat<B>, b: &Rat<B>, depth: usize, out: &mut Vec<Rat<B>>) {
        if depth == 0 || positive_weights(den, a, b) {
            out.push(b.clone());
            return;
        }
        let mid = a.add(b).mul(&Rat::new(1, 2));
        go(den, a, &mid, depth - 1, out);
        go(den, &mid, b, depth - 1, out);
    }
    let mut stations = vec![a.clone()];
    go(den, a, b, MAX_DEPTH, &mut stations);
    stations
}

/// The exact σ-stations `[σ_lo, …, σ_hi]` that [`brep_freeboundary`] subdivides a chart's σ-support
/// into so every ruled Bézier patch has positive weights — its intrinsic, parametrization-
/// independent partition. The stations depend on the chart alone, **not** on any hole: the holed
/// construction lets a [`HoleRect`] cross a station freely (cutting it per slice). All four σ-rails
/// share one denominator (the `μ⁻` base fixes it), so this single partition serves the whole solid;
/// `w` does not affect it (the offset `n·w` is denominator-preserving). Exposed for callers that
/// want the partition itself.
pub fn sigma_stations<B: Backend>(
    chart: &Chart<B>,
    sigma: &Interval<B>,
    w: &Interval<B>,
    mu_lo: &RatFunc<B>,
    mu_hi: &RatFunc<B>,
) -> Vec<Rat<B>> {
    let _ = mu_hi; // both μ-bases share the denominator; the μ⁻ rail fixes the partition
    let c = chart.pedal().reduce();
    let r = chart.ruling().reduce();
    let n = chart.normal().reduce();
    let anchor = c.add(&r.scale(mu_lo)).reduce().add(&n.scale_rat(&w.lo));
    sigma_splits(anchor.den(), &sigma.lo, &sigma.hi)
}

impl<B: Backend> Builder<B> {
    fn new() -> Self {
        Builder {
            brep: Brep::new(),
            verts: Vec::new(),
            edge_keys: Vec::new(),
        }
    }

    /// The dedup key of an undirected edge `{a, b}` of `kind` (`0` = Line, `1` = Bézier), returning
    /// its edge id if already emitted.
    fn edge_key(&self, a: usize, b: usize, kind: u8) -> Option<usize> {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        self.edge_keys
            .iter()
            .find(|(x, y, k, _)| *x == lo && *y == hi && *k == kind)
            .map(|&(_, _, _, e)| e)
    }

    /// Record an edge id under its undirected `{a, b}`/`kind` key.
    fn record_edge(&mut self, a: usize, b: usize, kind: u8, eid: usize) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        self.edge_keys.push((lo, hi, kind, eid));
    }

    /// Get-or-add a straight [`Line`](EdgeGeom::Line) edge between two existing vertices, **deduped**
    /// by the undirected endpoint pair: a straight edge two faces share (a hole rim vertical shared
    /// by two tube walls; a σ-station radial shared by two adjacent slice lids) is one edge id. A
    /// straight edge is determined by its endpoints, so this dedup is exact and unambiguous.
    fn line_edge(&mut self, a: usize, b: usize) -> usize {
        if let Some(e) = self.edge_key(a, b, 0) {
            return e;
        }
        let e = self.brep.add_edge(a, b, EdgeGeom::Line);
        self.record_edge(a, b, 0, e);
        e
    }

    /// Get-or-add a σ-rail [`RationalBezier`](EdgeGeom::RationalBezier) edge for `surf` over `supp`,
    /// **deduped** by its endpoint pair: the σ-rail a lid and its tube wall (or a lid and a μ-wall)
    /// both carry — always `surf(μ, w)` for one `(μ, w)`, so distinct rails have distinct endpoints —
    /// is one edge id. Distinct rails between the *same* endpoints do not arise in the developable
    /// construction (a rail is pinned by its `(μ, w)`, which fixes both ends), so the endpoint key is
    /// exact here.
    fn rail_edge(&mut self, surf: &Vec3Rat<B>, supp: &Interval<B>) -> usize {
        let start = surf.eval(&supp.lo).expect("rail start finite");
        let end = surf.eval(&supp.hi).expect("rail end finite");
        let sv = self.vertex(&start);
        let ev = self.vertex(&end);
        if let Some(e) = self.edge_key(sv, ev, 1) {
            return e;
        }
        let bez = RatBezier::from_vec3rat(surf, &supp.lo, &supp.hi);
        let e = self.brep.add_edge(sv, ev, EdgeGeom::RationalBezier(bez));
        self.record_edge(sv, ev, 1, e);
        e
    }

    /// Get-or-add a vertex by **exact** rational coordinates: a point that coincides with
    /// one already emitted returns the same vertex id. This is the sole source of
    /// cross-flank identity — the shared crease corners deduplicate here.
    fn vertex(&mut self, p: &[Rat<B>; 3]) -> usize {
        if let Some((_, id)) = self.verts.iter().find(|(q, _)| q == p) {
            return *id;
        }
        let id = self.brep.add_vertex(vert(p));
        self.verts.push((p.clone(), id));
        id
    }

    /// The directed half-edge that traverses edge `eid` from vertex `from` to vertex `to`
    /// (matching or reversing its stored orientation).
    fn directed(&self, eid: usize, from: usize, to: usize) -> HalfEdge {
        let e = &self.brep.edges()[eid];
        if e.start == from && e.end == to {
            (eid, false)
        } else {
            debug_assert!(
                e.start == to && e.end == from,
                "directed: endpoints must match the edge"
            );
            (eid, true)
        }
    }

    /// Add a μ-rail as an exact rational-Bézier edge over the σ-support: the curve
    /// `surf(σ)` restricted to `[supp.lo, supp.hi]`, its endpoint vertices deduplicated.
    fn add_rail(&mut self, surf: &Vec3Rat<B>, supp: &Interval<B>) -> usize {
        // A single-span σ-rail Bézier over `supp`; the caller keeps `supp` narrow enough for
        // positive weights (the σ-domain is subdivided upstream by [`sigma_splits`]).
        let bez = RatBezier::from_vec3rat(surf, &supp.lo, &supp.hi);
        let start = surf.eval(&supp.lo).expect("rail start finite");
        let end = surf.eval(&supp.hi).expect("rail end finite");
        let sv = self.vertex(&start);
        let ev = self.vertex(&end);
        self.brep.add_edge(sv, ev, EdgeGeom::RationalBezier(bez))
    }

    /// Emit one flank's `w = 0` ruled sheet: the two μ-rails, the far cross-section edge,
    /// the crease wire (overhang tip · shared `M` · overhang tip), and the ruled face
    /// (`μ⁻` rail extruded along the ruling direction). `flip` reverses the wire so the
    /// two flanks traverse the shared edge `M` in **opposite** directions (a consistently
    /// oriented seam).
    #[allow(clippy::too_many_arguments)]
    fn add_flank(
        &mut self,
        chart: &Chart<B>,
        supp: &Interval<B>,
        sigma_crease: &Rat<B>,
        mu: &MuRange<B>,
        w: &Rat<B>,
        shared: &SharedCrease,
        flip: bool,
    ) {
        // The non-crease support endpoint (the far edge of the retained band).
        let sigma_far = if &supp.lo == sigma_crease {
            supp.hi.clone()
        } else {
            debug_assert!(
                &supp.hi == sigma_crease,
                "the crease station bounds the σ-support"
            );
            supp.lo.clone()
        };

        // The two μ-rails; the μ⁻ rail doubles as the ruled face's extrusion base.
        let surf_lo = chart.surface(&mu.lo, w);
        let surf_hi = chart.surface(&mu.hi, w);
        let rail_lo = self.add_rail(&surf_lo, supp);
        let rail_hi = self.add_rail(&surf_hi, supp);

        // Crease corners (μ⁻ = low-x corner, μ⁺ = high-x corner for a +x̂ ruling).
        let corner_lo_p = surf_lo.eval(sigma_crease).expect("crease lo corner finite");
        let corner_hi_p = surf_hi.eval(sigma_crease).expect("crease hi corner finite");
        debug_assert!(
            corner_lo_p[0].sub(&corner_hi_p[0]).sign() <= 0,
            "μ⁻ maps to the low-x crease corner (constant-x̂ ruling)"
        );
        let corner_lo_v = self.vertex(&corner_lo_p);
        let corner_hi_v = self.vertex(&corner_hi_p);

        // Far corners + the far cross-section (straight ruling) edge.
        let far_lo_p = surf_lo.eval(&sigma_far).expect("far lo corner finite");
        let far_hi_p = surf_hi.eval(&sigma_far).expect("far hi corner finite");
        let far_lo_v = self.vertex(&far_lo_p);
        let far_hi_v = self.vertex(&far_hi_p);
        let far = self.brep.add_edge(far_lo_v, far_hi_v, EdgeGeom::Line);

        // The crease wire, corner_lo → corner_hi: an overhang tip (free) where this flank
        // sticks out past the shared middle, the shared edge M, then the other tip. A tip
        // whose corner already coincides with the shared endpoint is omitted (that flank
        // contributes no overhang on that side).
        let mut wire: Vec<HalfEdge> = Vec::new();
        if corner_lo_v != shared.s_lo_v {
            let tip = self
                .brep
                .add_edge(corner_lo_v, shared.s_lo_v, EdgeGeom::Line);
            wire.push((tip, false));
        }
        wire.push(self.directed(shared.m_eid, shared.s_lo_v, shared.s_hi_v));
        if corner_hi_v != shared.s_hi_v {
            let tip = self
                .brep
                .add_edge(shared.s_hi_v, corner_hi_v, EdgeGeom::Line);
            wire.push((tip, false));
        }
        // …then around the far side back to the low corner.
        wire.push(self.directed(rail_hi, corner_hi_v, far_hi_v));
        wire.push(self.directed(far, far_hi_v, far_lo_v));
        wire.push(self.directed(rail_lo, far_lo_v, corner_lo_v));

        if flip {
            wire.reverse();
            for h in &mut wire {
                h.1 = !h.1;
            }
        }

        let dir = chart
            .ruling()
            .eval(sigma_crease)
            .expect("ruling direction finite");
        self.brep
            .add_face(FaceSurface::LinearExtrusion { base: rail_lo, dir }, wire);
    }

    fn into_brep(self) -> Brep<B> {
        self.brep
    }
}

/// The larger-`x` of two crease corners (the shared middle's low endpoint).
fn max_x<B: Backend>(a: &[Rat<B>; 3], b: &[Rat<B>; 3]) -> [Rat<B>; 3] {
    if a[0].sub(&b[0]).sign() >= 0 {
        a.clone()
    } else {
        b.clone()
    }
}

/// The smaller-`x` of two crease corners (the shared middle's high endpoint).
fn min_x<B: Backend>(a: &[Rat<B>; 3], b: &[Rat<B>; 3]) -> [Rat<B>; 3] {
    if a[0].sub(&b[0]).sign() <= 0 {
        a.clone()
    } else {
        b.clone()
    }
}

/// Assemble the exact [`Brep`] of a certified one-joint closure.
///
/// Emits the two flank faces as exact `w = 0` ruled sheets (each chart's `μ⁻` rail
/// extruded along its ruling direction over the retained σ-support), the two flanks
/// sharing the fold crease line `L` **by identity**: the overlap of their crease rulings
/// is one shared edge `M` referenced by both wires, and the wider flank's overhang past
/// that overlap is left as an honestly-open free tip (the certified-seam / honest-open
/// export).
///
/// The exact §10 body is the two flank sheets for **both** cap witnesses. On the
/// **MITER** branch ([`CapWitness::Miter`]) the flanks meet directly along `M`, so there
/// is no separate cap. On the **LEDGE** branch ([`CapWitness::Ledge`]) the only available
/// cap outline is the CAP-IN-D24 *licensing square* — a placeholder, not the real
/// projected cut — whose crease edge overlaps `M`, so no certificate backs a flank↔cap
/// seam; rather than fabricate one, the exact body emits only the certified flanks and
/// defers the exact cap face to the `V_∂` real-cut slice. (The `§11` mesh path, in
/// [`crate::shell`], still triangulates the placeholder cap for visualization.)
///
/// The result is exact: every vertex is a [`Surd`] (rational here), and no float is
/// produced — the exact→`f64` cast lives in the [`step`](crate::step) bridge.
pub fn brep_from_closure<B: Backend>(
    joint: &Joint<B>,
    t: &ClosureTreatment<'_, B>,
    valid: &ClosureValid<B>,
) -> Brep<B> {
    let mut bld = Builder::new();
    let w = Rat::from_i128(0); // the neutral crease sheet — where MITER-FIT licenses L
    let mu = &t.mu;
    let ca = joint.flank_a().chart();
    let cb = joint.flank_b().chart();
    let sa = &joint.crease().sigma_a;
    let sb = &joint.crease().sigma_b;

    // Each flank's crease ruling (μ⁻ = low-x, μ⁺ = high-x corner, on the shared line L).
    let a_lo = ca.surface(&mu.lo, &w).eval(sa).expect("A crease lo finite");
    let a_hi = ca.surface(&mu.hi, &w).eval(sa).expect("A crease hi finite");
    let b_lo = cb.surface(&mu.lo, &w).eval(sb).expect("B crease lo finite");
    let b_hi = cb.surface(&mu.hi, &w).eval(sb).expect("B crease hi finite");

    // The shared middle M = the overlap of the two crease rulings on L.
    let s_lo = max_x(&a_lo, &b_lo);
    let s_hi = min_x(&a_hi, &b_hi);
    debug_assert!(
        s_lo[0].sub(&s_hi[0]).sign() < 0,
        "the two flanks share a non-degenerate crease middle"
    );
    let s_lo_v = bld.vertex(&s_lo);
    let s_hi_v = bld.vertex(&s_hi);
    let m_eid = bld.brep.add_edge(s_lo_v, s_hi_v, EdgeGeom::Line);
    let shared = SharedCrease {
        s_lo_v,
        s_hi_v,
        m_eid,
    };

    // Flank A un-flipped, flank B flipped, so both traverse M in opposite directions.
    bld.add_flank(ca, &t.sigma_a, sa, mu, &w, &shared, false);
    bld.add_flank(cb, &t.sigma_b, sb, mu, &w, &shared, true);

    // Neither cap witness adds a face to the exact §10 body: MITER meets directly along M,
    // and the LEDGE cap is *not* emitted as an exact face here. The only geometry available
    // for a LEDGE cap is the CAP-IN-D24 *licensing square* — a placeholder outline, not the
    // real projected cut — and its crease edge overlaps the already-shared middle M, so no
    // certificate backs a flank↔cap seam. Rather than fabricate one (an ad-hoc face that
    // OCCT could only accept as a disconnected shell), we export what is certified — the two
    // flank sheets — and defer the exact cap to the `V_∂` real-cut slice, where the cap edge
    // genuinely lands on a flank edge and can be sewn watertight. See `docs/vv-guide.md §8`.
    match &valid.cap {
        CapWitness::Miter(_) | CapWitness::Ledge(_) => {}
    }

    bld.into_brep()
}

/// Assemble the exact [`Brep`] of a **single flank as a closed slab** — the first certified
/// closed solid (Milestone D slice 4, atlas assembly). The slab is the flank's support box
/// `σ ∈ t.sigma_a × μ ∈ [t.mu.lo, t.mu.hi] × w ∈ [t.w.lo, t.w.hi]` bounded by six exact
/// faces meeting along shared edges **by identity** — a topological box (8 vertices, 12
/// edges, 6 faces) whose combinatorics [`Brep::to_shell_certificate`] hands to
/// `certify_core::shell::closed_shell`, and whose geometry the OCCT oracle corroborates.
///
/// The six faces reduce to three exact surface kinds (each exact for a cylinder flank):
/// - the two **σ = const** end caps are [`Plane`](FaceSurface::Plane) — at fixed σ the map
///   `c + μr + wn` is affine in `(μ, w)`;
/// - the two **w = const** sheets are [`LinearExtrusion`](FaceSurface::LinearExtrusion) of a
///   σ-rail along the (constant-direction) ruling `r ∥ x̂`;
/// - the two **μ = const** walls are [`RationalPatch`](FaceSurface::RationalPatch)es —
///   `base(σ) + w·dir(σ)` ruled along the *rotating* normal `n(σ)`, which no
///   constant-direction extrusion expresses; the exact rational tensor patch does
///   ([`RatBezierSurface::ruled_from_rails`], the two σ-rails share a denominator so the
///   patch reproduces the affine ruling exactly).
///
/// Uses flank A. No cap witness is consumed — closedness is certified by `closed_shell`, not
/// the joint-local closure certificate. The result is exact (every vertex a [`Surd`], every
/// pole/weight a [`Rat`](lattice::Rat)); the exact→`f64` cast lives in the STEP bridge.
///
/// # Example
///
/// ```
/// use export::brep_build::brep_slab_from_closure;
/// use fixtures::closure_joint::{one_joint, treatment, ledge_d24};
///
/// let joint = one_joint();
/// let d24 = ledge_d24();
/// let t = treatment(&d24);
/// let slab = brep_slab_from_closure(&joint, &t);
/// assert_eq!(slab.verts().len(), 8);
/// assert_eq!(slab.faces().len(), 6);
/// // Every edge is shared by exactly two faces — a closed box, no free edge.
/// assert_eq!(slab.edge_incidence().iter().filter(|&&c| c == 2).count(), 12);
/// assert_eq!(slab.free_edges(), 0);
/// assert_eq!(slab.nonmanifold_edges(), 0);
/// ```
pub fn brep_slab_from_closure<B: Backend>(
    joint: &Joint<B>,
    t: &ClosureTreatment<'_, B>,
) -> Brep<B> {
    let mut bld = Builder::new();
    let chart = joint.flank_a().chart();
    let supp = &t.sigma_a;
    let sigmas = [supp.lo.clone(), supp.hi.clone()];
    let mus = [t.mu.lo.clone(), t.mu.hi.clone()];
    let ws = [t.w.lo.clone(), t.w.hi.clone()];

    // The exact σ-rail curve at `(μ_j, w_k)` = `base_j + w_k·n`, where `base_j = c + μ_j·r`.
    // `chart.surface`'s denominator-multiplying `add`s inflate the rational's degree (a
    // `c + μr + wn` piles up a common factor), which would blow the Bézier/BSpline poles up
    // to ±∞ after the `f64` cast. Reducing `base` and `n` first keeps the true low degree —
    // and, since `w` is a scalar, both `w`-rails of a μ-wall keep the shared denominator
    // `base_j.den · n.den`, exactly the shared-weights condition `ruled_from_rails` needs.
    let dir = chart.normal().reduce();
    let base = [
        chart.surface(&mus[0], &Rat::from_i128(0)).reduce(),
        chart.surface(&mus[1], &Rat::from_i128(0)).reduce(),
    ];
    let surf = |j: usize, k: usize| base[j].add(&dir.scale_rat(&ws[k]));

    // The (μ, w) cross-section ring: r0=(μlo,wlo), r1=(μhi,wlo), r2=(μhi,whi), r3=(μlo,whi).
    let ring = [(0usize, 0usize), (1, 0), (1, 1), (0, 1)];

    // 8 corner vertices: a[m] at σ = σlo, b[m] at σ = σhi, over ring corner m (deduped).
    let mut a = [0usize; 4];
    let mut b = [0usize; 4];
    for (m, &(j, k)) in ring.iter().enumerate() {
        let s = surf(j, k);
        a[m] = bld.vertex(&s.eval(&sigmas[0]).expect("slab corner (σlo) finite"));
        b[m] = bld.vertex(&s.eval(&sigmas[1]).expect("slab corner (σhi) finite"));
    }

    // 12 edges: the two straight cross-section rings (σlo, σhi) and the four curved σ-rails.
    let mut ring_a = [0usize; 4];
    let mut ring_b = [0usize; 4];
    let mut rails = [0usize; 4];
    for m in 0..4 {
        let n = (m + 1) % 4;
        ring_a[m] = bld.brep.add_edge(a[m], a[n], EdgeGeom::Line);
        ring_b[m] = bld.brep.add_edge(b[m], b[n], EdgeGeom::Line);
        let (j, k) = ring[m];
        rails[m] = bld.add_rail(&surf(j, k), supp);
    }

    // The constant ruling direction (cylinder rulings ∥ x̂) for the w-sheet extrusions.
    let dir = chart
        .ruling()
        .eval(&sigmas[0])
        .expect("ruling direction finite");

    // σ = σlo end cap (planar), wound A0→A3→A2→A1 (outward, matching the cube orientation).
    let cap_lo = vec![
        bld.directed(ring_a[3], a[0], a[3]),
        bld.directed(ring_a[2], a[3], a[2]),
        bld.directed(ring_a[1], a[2], a[1]),
        bld.directed(ring_a[0], a[1], a[0]),
    ];
    bld.brep.add_plane(cap_lo);

    // σ = σhi end cap (planar), wound B0→B1→B2→B3.
    let cap_hi = vec![
        bld.directed(ring_b[0], b[0], b[1]),
        bld.directed(ring_b[1], b[1], b[2]),
        bld.directed(ring_b[2], b[2], b[3]),
        bld.directed(ring_b[3], b[3], b[0]),
    ];
    bld.brep.add_plane(cap_hi);

    // The four side faces: A_m → A_{m+1} → B_{m+1} → B_m, sharing the rings and rails by
    // identity. Even m = w = const sheets (LinearExtrusion); odd m = μ = const walls
    // (RationalPatch ruled between the two σ-rails at w = wlo, whi).
    for m in 0..4 {
        let n = (m + 1) % 4;
        let wire = vec![
            bld.directed(ring_a[m], a[m], a[n]),
            bld.directed(rails[n], a[n], b[n]),
            bld.directed(ring_b[m], b[n], b[m]),
            bld.directed(rails[m], b[m], a[m]),
        ];
        let surface = if m % 2 == 0 {
            // w = const sheet: extrude this side's low σ-rail along the ruling.
            FaceSurface::LinearExtrusion {
                base: rails[m],
                dir: dir.clone(),
            }
        } else {
            // μ = const wall: exact rational patch ruled between the (μ, wlo) and (μ, whi)
            // σ-rails. Ring corner m is (j, k=?) — the μ index j is shared across w here.
            let (j, _) = ring[m];
            ruled_panel(&surf(j, 0), &surf(j, 1), &sigmas[0], &sigmas[1])
        };
        bld.brep.add_face(surface, wire);
    }

    bld.into_brep()
}

/// Assemble the exact [`Brep`] of a **single flank as a closed slab over an authored
/// substrate free boundary** — the D4.3 generalization of [`brep_slab_from_closure`] from a
/// rectangular support box to a σ-band bounded by authored rational μ-splines `μ⁻(σ), μ⁺(σ)`
/// (spec §3.4:151; the exact-over-anchor footprint, `spec:194`).
///
/// Identical 8-vertex / 12-edge / 6-face box topology to the slab (so
/// [`closed_shell`](certify_core::shell::closed_shell) applies unchanged), with the slab's
/// *constant* μ-bounds replaced by the σ-varying boundary splines: each μ-rail is
/// `c(σ) + μ±(σ)·r(σ) + w·n(σ)` — [`Vec3Rat::scale`] by the [`RatFunc`] boundary (the one
/// operation that changes vs the slab's scalar `scale_rat`). The pedal, ruling, and normal
/// are reduced once so both μ-bases collapse to one shared denominator, and every σ-rail is
/// then `base + w·n` — so all four rails keep that shared denominator, the exact-ruling
/// precondition [`RatBezierSurface::ruled_from_rails`] needs. Because the boundary now curves
/// in σ, **all four** side faces are exact rational patches (not the slab's two
/// `LinearExtrusion` w-sheets):
/// - the two **σ = const** end caps stay [`Plane`](FaceSurface::Plane) (at fixed σ the map
///   `c + μr + wn` is affine in `(μ, w)`);
/// - the two **w = const** sheets and the two **μ = const** walls are
///   [`RationalPatch`](FaceSurface::RationalPatch)es ruled between adjacent σ-rails.
///
/// Works for **any** developable [`Chart`] — a joint's cylinder flank, a rational cone, etc. —
/// over the σ-support `sigma`, thickness window `w`, and authored boundary splines
/// `mu_lo`/`mu_hi`. (For a certified one-joint closure, [`brep_freeboundary_from_closure`]
/// supplies flank A's chart and the treatment's boxes.) No cap witness is consumed — closedness
/// is certified by `closed_shell`, not a joint-local certificate; the authored boundary's own
/// validity is the [`free_boundary`](certify_core::free_boundary) obligation set (build its
/// certificate with [`free_boundary_cert`]). Exact throughout (every vertex a [`Surd`], every
/// pole/weight a [`Rat`]); the exact→`f64` cast lives in the STEP bridge.
///
/// # Example
///
/// ```
/// use export::brep_build::brep_freeboundary;
/// use fixtures::closure_joint::one_joint;
/// use lattice::{Bignum, Interval, Poly, Rat, RatFunc};
///
/// let joint = one_joint();
/// let poly = |cs: &[i128]| Poly::<Bignum>::from_coeffs(cs.iter().map(|&c| Rat::from_i128(c)).collect());
/// let sigma = Interval { lo: Rat::new(-1, 8), hi: Rat::from_i128(0) };
/// let w = Interval { lo: Rat::from_i128(1), hi: Rat::from_i128(2) };
/// // A tapered authored band: μ⁻(σ) = −1 + σ, μ⁺(σ) = 1 − σ (genuinely varying in σ).
/// let mu_lo = RatFunc::from_poly(poly(&[-1, 1]));
/// let mu_hi = RatFunc::from_poly(poly(&[1, -1]));
/// let solid = brep_freeboundary(joint.flank_a().chart(), &sigma, &w, &mu_lo, &mu_hi);
/// assert_eq!(solid.verts().len(), 8);
/// assert_eq!(solid.faces().len(), 6);
/// assert_eq!(solid.free_edges(), 0); // a closed slab has no free edge
/// assert_eq!(solid.nonmanifold_edges(), 0);
/// ```
pub fn brep_freeboundary<B: Backend>(
    chart: &Chart<B>,
    sigma: &Interval<B>,
    w: &Interval<B>,
    mu_lo: &RatFunc<B>,
    mu_hi: &RatFunc<B>,
) -> Brep<B> {
    // A hole-free slab is the empty-holes case of the general engine, which then never refuses
    // (refusal is only ever a hole that will not fit a positive-weight σ-slice).
    brep_freeboundary_holed(chart, sigma, w, mu_lo, mu_hi, &[])
        .expect("a hole-free free-boundary solid is always constructible")
}

/// A rectangular **through-hole** authored in the sheet's `(σ, μ)` parameter domain — the intrinsic
/// coordinates of the developable, so the *same* hole describes the cut in both the flat (unrolled)
/// and folded states. Its `sigma` range is where it sits along the ruling family and its `mu` range
/// is the band it removes; the hole is lifted to both thickness offsets and closed by a tube. It may
/// **cross any number of positive-weight σ-stations** — [`brep_freeboundary_holed`] cuts it out per
/// slice with the exact `arrange2d` boolean and splits its tube at every crossed station — but must
/// be **strictly interior** to the panel in both σ and μ (a genus-raising through-hole, not a
/// boundary slot). Multiple holes are supported (each raises the genus); disjoint holes may share a
/// slice.
pub struct HoleRect<B: Backend = Bignum> {
    /// The σ-interval the hole spans — strictly inside the panel's σ-support.
    pub sigma: Interval<B>,
    /// The μ-interval the hole removes — strictly inside the constant band `[μ⁻, μ⁺]`.
    pub mu: Interval<B>,
}

/// [`brep_freeboundary`] with rectangular **through-holes** (arbitrary genus), by the general
/// **arrangement-per-slice** construction. The solid is the prism over the exact 2-D region
/// `P ∖ H` (P = the panel `(σ,μ)` rectangle, H = the union of the [`HoleRect`]s) extruded through
/// the thickness. Positive-weight σ-stations subdivide the two developable lids into single-span
/// Bézier patches and **cross holes freely**: per σ-slice the lid is `strip ∖ (holes ∩ slice)`
/// computed by the *same* exact `arrange2d` boolean the flat side uses (`develop::flat::cut_hole`,
/// `A △ B = A ∖ B` for an interior hole). A hole may straddle any station — it opens onto the
/// station edge as a **notch**, or splits the strip into two **μ-bands**; the arrangement decides,
/// with no special case. Each hole is closed by a tube through the thickness, **split at every
/// station it crosses** so each wall is single-span; the result is watertight by edge identity
/// ([`Builder`]'s vertex + edge dedup) and certified by `certify_core::shell::closed_shell_holed`.
///
/// Holes must be **strictly interior** to the panel in both σ and μ (a through-hole raising the
/// genus, not a boundary slot), and the panel μ-band must be **constant** (a rectangular P — a
/// curved free boundary in `(σ,μ)` is not a polygon operand, so it is deferred). `None` on a hole
/// touching/exceeding the panel boundary, a non-rectangular panel, a degenerate hole, or an
/// internal arrangement fault (a self-touch pinch or a CAP-OUT refutation) — never a silent
/// mis-build.
///
/// With `holes` empty this delegates to [`brep_freeboundary_slab`] — the same `4(N+1)`/`8N+4`/`4N+2`
/// disk-face solid, which additionally supports a *curved* authored μ-boundary (the holed path does
/// not — it needs the rectangular panel).
pub fn brep_freeboundary_holed<B: Backend>(
    chart: &Chart<B>,
    sigma: &Interval<B>,
    w: &Interval<B>,
    mu_lo: &RatFunc<B>,
    mu_hi: &RatFunc<B>,
    holes: &[HoleRect<B>],
) -> Option<Brep<B>> {
    // A hole-free solid is the N-slice slab (which also handles a curved authored μ-boundary).
    if holes.is_empty() {
        return Some(brep_freeboundary_slab(chart, sigma, w, mu_lo, mu_hi));
    }

    let wlo = w.lo.clone();
    let whi = w.hi.clone();

    // The panel μ-band must be constant — a rectangular P (a curved ∂P is not a polygon operand).
    let mlo = ratfunc_const(mu_lo)?;
    let mhi = ratfunc_const(mu_hi)?;
    if mlo.sub(&mhi).sign() >= 0 {
        return None;
    }

    // Every hole strictly interior to the panel in σ and μ (a through-hole, not a boundary slot).
    let lt = |a: &Rat<B>, b: &Rat<B>| a.sub(b).sign() < 0;
    for h in holes {
        if !(lt(&sigma.lo, &h.sigma.lo)
            && lt(&h.sigma.lo, &h.sigma.hi)
            && lt(&h.sigma.hi, &sigma.hi)
            && lt(&mlo, &h.mu.lo)
            && lt(&h.mu.lo, &h.mu.hi)
            && lt(&h.mu.hi, &mhi))
        {
            return None;
        }
    }

    // Reduce the chart fields once (low degree; the same shared-denominator rails as the slab).
    let c = chart.pedal().reduce();
    let r = chart.ruling().reduce();
    let n = chart.normal().reduce();

    // The positive-weight σ-stations — the intrinsic partition, hole-independent (stations cross
    // holes freely). `N = stations − 1` slices.
    let stations = sigma_stations(chart, sigma, w, mu_lo, mu_hi);
    let nst = stations.len();
    if nst < 2 {
        return None;
    }

    let mut bld = Builder::new();

    // Per σ-slice: the two developable lids (`strip ∖ holes` at wlo/whi) + each crossed hole's tube.
    for k in 0..nst - 1 {
        let sk = stations[k].clone();
        let sk1 = stations[k + 1].clone();

        // Operand A = the slice strip; operand B = each hole clipped to this slice's σ-range.
        let mut edges = rect_edges(&sk, &sk1, &mlo, &mhi, 0);
        for (hi, h) in holes.iter().enumerate() {
            let clo = if h.sigma.lo.sub(&sk).sign() >= 0 {
                h.sigma.lo.clone()
            } else {
                sk.clone()
            };
            let chi = if h.sigma.hi.sub(&sk1).sign() <= 0 {
                h.sigma.hi.clone()
            } else {
                sk1.clone()
            };
            if lt(&clo, &chi) {
                edges.extend(rect_edges(&clo, &chi, &h.mu.lo, &h.mu.hi, (hi + 1) as u32));
            }
        }
        let operand_of = |cv: CurveId| {
            if cv.0 == 0 {
                OperandId::A
            } else {
                OperandId::B
            }
        };
        let region = match ledge_dom_certified(&edges, &operand_of, BoolOp::Xor) {
            Verdict::Verified(cap) => {
                let (region, _v_boundary, pinches) = cap.into_parts();
                if !pinches.is_empty() {
                    return None; // a self-touching cut — refuse rather than mis-build
                }
                region
            }
            _ => return None,
        };

        for face in &region.faces {
            let outer = ordered_loop(&face.outer, true)?;
            let hole_loops: Vec<Vec<(Rat<B>, Rat<B>)>> = face
                .holes
                .iter()
                .map(|h| ordered_loop(h, false))
                .collect::<Option<_>>()?;

            // Top lid (whi): arrangement winding as-is (outer CCW-in-(σ,μ), holes CW).
            let top_outer = lift_loop_forward(&mut bld, &c, &r, &n, &outer, &whi);
            let top_holes: Vec<Vec<HalfEdge>> = hole_loops
                .iter()
                .map(|h| lift_loop_forward(&mut bld, &c, &r, &n, h, &whi))
                .collect();
            let surf_top = ruled_panel(
                &rail_vec(&c, &r, &n, &mlo, &whi),
                &rail_vec(&c, &r, &n, &mhi, &whi),
                &sk,
                &sk1,
            );
            bld.brep.add_face_with_holes(surf_top, top_outer, top_holes);

            // Bottom lid (wlo): every loop reversed (opposite winding), so the two lids face out.
            let bot_outer = reversed_wire(lift_loop_forward(&mut bld, &c, &r, &n, &outer, &wlo));
            let bot_holes: Vec<Vec<HalfEdge>> = hole_loops
                .iter()
                .map(|h| reversed_wire(lift_loop_forward(&mut bld, &c, &r, &n, h, &wlo)))
                .collect();
            let surf_bot = ruled_panel(
                &rail_vec(&c, &r, &n, &mlo, &wlo),
                &rail_vec(&c, &r, &n, &mhi, &wlo),
                &sk,
                &sk1,
            );
            bld.brep.add_face_with_holes(surf_bot, bot_outer, bot_holes);

            // Tubes: one wall per hole-rim edge, in arrangement (top-lid) order — the reverse of
            // each lid's use of the shared edge, so every rim edge is once-forward-once-reversed.
            for corners in core::iter::once(&outer).chain(hole_loops.iter()) {
                let m = corners.len();
                for i in 0..m {
                    let a = &corners[i];
                    let b = &corners[(i + 1) % m];
                    if is_rim_edge(a, b, &sk, &sk1, &mlo, &mhi) {
                        emit_tube(&mut bld, &c, &r, &n, a, b, &wlo, &whi);
                    }
                }
            }
        }

        // The two panel μ-walls (μ⁺ = mhi, μ⁻ = mlo) for this slice, swept through the thickness.
        add_mu_wall(&mut bld, &c, &r, &n, &sk, &sk1, &mhi, &wlo, &whi, true);
        add_mu_wall(&mut bld, &c, &r, &n, &sk, &sk1, &mlo, &wlo, &whi, false);
    }

    // The two σ-end caps (planar (μ,w) rectangles at σlo/σhi; holes are strictly interior in σ).
    add_sigma_cap(
        &mut bld,
        &c,
        &r,
        &n,
        &stations[0],
        &mlo,
        &mhi,
        &wlo,
        &whi,
        false,
    );
    add_sigma_cap(
        &mut bld,
        &c,
        &r,
        &n,
        &stations[nst - 1],
        &mlo,
        &mhi,
        &wlo,
        &whi,
        true,
    );

    Some(bld.into_brep())
}

/// The hole-free **N-slice slab** — [`brep_freeboundary`]'s certified closed 2-manifold over an
/// authored σ-band `[μ⁻(σ), μ⁺(σ)]`, which may be a *curved* free boundary (unlike the holed path,
/// which needs a rectangular panel). σ is subdivided by [`sigma_stations`] into positive-weight
/// slices; each slice is four exact ruled-patch sides sharing the cross-ring and σ-rail edges by
/// identity, plus the two planar σ-end caps: `4(N+1)` verts, `8N+4` edges, `4N+2` faces.
fn brep_freeboundary_slab<B: Backend>(
    chart: &Chart<B>,
    sigma: &Interval<B>,
    w: &Interval<B>,
    mu_lo: &RatFunc<B>,
    mu_hi: &RatFunc<B>,
) -> Brep<B> {
    let mut bld = Builder::new();
    let ws = [w.lo.clone(), w.hi.clone()];
    let c = chart.pedal().reduce();
    let r = chart.ruling().reduce();
    let n = chart.normal().reduce();
    let bases = [
        c.add(&r.scale(mu_lo)).reduce(),
        c.add(&r.scale(mu_hi)).reduce(),
    ];
    let surf = |j: usize, k: usize| bases[j].add(&n.scale_rat(&ws[k]));
    // The (μ-side, w) cross-section ring: r0=(μ⁻,wlo), r1=(μ⁺,wlo), r2=(μ⁺,whi), r3=(μ⁻,whi).
    let ring = [(0usize, 0usize), (1, 0), (1, 1), (0, 1)];
    let stations = sigma_stations(chart, sigma, w, mu_lo, mu_hi);
    let nst = stations.len();

    // Corner vertices v[k][m]: ring corner m at station s_k (deduped by coordinate).
    let mut v: Vec<[usize; 4]> = vec![[0usize; 4]; nst];
    for (k, sk) in stations.iter().enumerate() {
        for (m, &(j, kk)) in ring.iter().enumerate() {
            v[k][m] = bld.vertex(&surf(j, kk).eval(sk).expect("free-boundary corner finite"));
        }
    }
    // Edges: a straight cross-ring of 4 at each station; 4 single-span σ-rails per slice.
    let mut ring_e: Vec<[usize; 4]> = vec![[0usize; 4]; nst];
    for k in 0..nst {
        for m in 0..4 {
            ring_e[k][m] = bld
                .brep
                .add_edge(v[k][m], v[k][(m + 1) % 4], EdgeGeom::Line);
        }
    }
    let mut rail_e: Vec<[usize; 4]> = vec![[0usize; 4]; nst - 1];
    for k in 0..nst - 1 {
        let supp_k = Interval {
            lo: stations[k].clone(),
            hi: stations[k + 1].clone(),
        };
        for m in 0..4 {
            let (j, kk) = ring[m];
            rail_e[k][m] = bld.add_rail(&surf(j, kk), &supp_k);
        }
    }
    // σ = σlo end cap (planar), wound v0→v3→v2→v1 (outward).
    let a = v[0];
    let cap_lo = vec![
        bld.directed(ring_e[0][3], a[0], a[3]),
        bld.directed(ring_e[0][2], a[3], a[2]),
        bld.directed(ring_e[0][1], a[2], a[1]),
        bld.directed(ring_e[0][0], a[1], a[0]),
    ];
    bld.brep.add_plane(cap_lo);
    // σ = σhi end cap (planar), wound v0→v1→v2→v3.
    let b = v[nst - 1];
    let cap_hi = vec![
        bld.directed(ring_e[nst - 1][0], b[0], b[1]),
        bld.directed(ring_e[nst - 1][1], b[1], b[2]),
        bld.directed(ring_e[nst - 1][2], b[2], b[3]),
        bld.directed(ring_e[nst - 1][3], b[3], b[0]),
    ];
    bld.brep.add_plane(cap_hi);
    // Per slice, four ruled-patch side faces sharing ring and rail edges by identity.
    for k in 0..nst - 1 {
        for m in 0..4 {
            let mp = (m + 1) % 4;
            let wire = vec![
                bld.directed(ring_e[k][m], v[k][m], v[k][mp]),
                bld.directed(rail_e[k][mp], v[k][mp], v[k + 1][mp]),
                bld.directed(ring_e[k + 1][m], v[k + 1][mp], v[k + 1][m]),
                bld.directed(rail_e[k][m], v[k + 1][m], v[k][m]),
            ];
            let (jm, km) = ring[m];
            let (jn, kn) = ring[mp];
            let surface = ruled_panel(&surf(jm, km), &surf(jn, kn), &stations[k], &stations[k + 1]);
            bld.brep.add_face(surface, wire);
        }
    }
    bld.into_brep()
}

/// One corner of a trim-solid `(σ, μ̂)` footprint loop: a σ-coordinate paired with the **rail**
/// `μ̂(σ)` it sits on (a `RatFunc`, not a scalar μ — the boundaries are curved). Two consecutive
/// corners share either their σ (→ a straight radial [`Line`], since `c + μ r + w n` is affine in
/// `(μ, w)` at fixed σ) or their rail (→ a σ-rail [`RationalBezier`] over the σ-range).
type TrimCorner<B> = (Rat<B>, RatFunc<B>);

/// Lift one footprint edge `a → b` at thickness level `w` to a deduped directed 3-D half-edge:
/// equal σ ⇒ a straight radial line; else a σ-rail Bézier of the shared rail over `[min σ, max σ]`.
fn lift_trim_edge<B: Backend>(
    bld: &mut Builder<B>,
    c: &Vec3Rat<B>,
    r: &Vec3Rat<B>,
    n: &Vec3Rat<B>,
    a: &TrimCorner<B>,
    b: &TrimCorner<B>,
    w: &Rat<B>,
) -> HalfEdge {
    let va = bld.vertex(
        &trim_surf(c, r, n, &a.1, w)
            .eval(&a.0)
            .expect("trim corner finite"),
    );
    let vb = bld.vertex(
        &trim_surf(c, r, n, &b.1, w)
            .eval(&b.0)
            .expect("trim corner finite"),
    );
    if req(&a.0, &b.0) {
        let eid = bld.line_edge(va, vb); // radial (σ = const)
        bld.directed(eid, va, vb)
    } else {
        let (lo, hi) = if a.0.cmp(&b.0) == core::cmp::Ordering::Less {
            (a.0.clone(), b.0.clone())
        } else {
            (b.0.clone(), a.0.clone())
        };
        let rv = trim_surf(c, r, n, &a.1, w); // a.1 == b.1 on a rail edge
        let eid = bld.rail_edge(&rv, &Interval { lo, hi });
        bld.directed(eid, va, vb)
    }
}

/// Lift a whole footprint loop (ordered corners) to a forward wire at thickness level `w`.
/// One slice's footprint: an outer loop with its inner (hole) loops.
type SliceFace<B> = (Vec<TrimCorner<B>>, Vec<Vec<TrimCorner<B>>>);

/// Drop corners that repeat the previous corner's `(σ, µ̂)` **point** — a degenerate edge.
///
/// A hole now meets its tangent ruling at a single point (both branches evaluate to the midline
/// there), so the σ-cap that used to bridge a visible gap has collapsed to nothing. Emitting it
/// anyway asks OCCT to build a zero-length line, which it refuses outright — the more faithful the
/// hole gets, the more certainly this fires. Collapsing the pair to one corner is also the honest
/// topology: the loop really does have a single vertex there.
fn dedup_trim_corners<B: Backend>(corners: &[TrimCorner<B>]) -> Vec<TrimCorner<B>> {
    let pt = |c: &TrimCorner<B>| c.1.eval(&c.0).map(|m| (c.0.clone(), m));
    let mut out: Vec<TrimCorner<B>> = Vec::with_capacity(corners.len());
    for c in corners {
        let same = match (out.last().and_then(&pt), pt(c)) {
            (Some(prev), Some(cur)) => prev == cur,
            _ => false,
        };
        if !same {
            out.push(c.clone());
        }
    }
    // The loop wraps, so the last corner may repeat the first.
    if out.len() > 1 {
        let ends = (out.first().and_then(&pt), out.last().and_then(&pt));
        if let (Some(a), Some(b)) = ends {
            if a == b {
                out.pop();
            }
        }
    }
    out
}

fn lift_trim_loop<B: Backend>(
    bld: &mut Builder<B>,
    c: &Vec3Rat<B>,
    r: &Vec3Rat<B>,
    n: &Vec3Rat<B>,
    corners: &[TrimCorner<B>],
    w: &Rat<B>,
) -> Vec<HalfEdge> {
    let m = corners.len();
    (0..m)
        .map(|i| lift_trim_edge(bld, c, r, n, &corners[i], &corners[(i + 1) % m], w))
        .collect()
}

/// Emit the wall sweeping one footprint edge `a → b` through the thickness `[wlo, whi]` — a ruled
/// patch for a rail edge (`μ̂ = const` rail, σ varies), planar for a radial edge (`σ = const`). The
/// winding is the reverse of the top (whi) lid's use of the shared edge (bottom-forward,
/// top-reversed), so — with the bottom (wlo) lid built from the *reversed* footprint — every shared
/// edge is traversed once each way. Rails are never shared between slices, so their walls are always
/// emitted; a radial at an interior σ-station is a shared cross-ring (no wall — see the caller).
#[allow(clippy::too_many_arguments)]
fn emit_trim_wall<B: Backend>(
    bld: &mut Builder<B>,
    c: &Vec3Rat<B>,
    r: &Vec3Rat<B>,
    n: &Vec3Rat<B>,
    a: &TrimCorner<B>,
    b: &TrimCorner<B>,
    wlo: &Rat<B>,
    whi: &Rat<B>,
) {
    let ab = bld.vertex(
        &trim_surf(c, r, n, &a.1, wlo)
            .eval(&a.0)
            .expect("wall finite"),
    );
    let at = bld.vertex(
        &trim_surf(c, r, n, &a.1, whi)
            .eval(&a.0)
            .expect("wall finite"),
    );
    let bb = bld.vertex(
        &trim_surf(c, r, n, &b.1, wlo)
            .eval(&b.0)
            .expect("wall finite"),
    );
    let bt = bld.vertex(
        &trim_surf(c, r, n, &b.1, whi)
            .eval(&b.0)
            .expect("wall finite"),
    );
    let bottom = lift_trim_edge(bld, c, r, n, a, b, wlo); // ab → bb
    let top = lift_trim_edge(bld, c, r, n, a, b, whi); // at → bt
    let va = bld.line_edge(ab, at);
    let vb = bld.line_edge(bb, bt);
    let wire = vec![
        bottom,                   // ab → bb
        bld.directed(vb, bb, bt), // bb → bt
        (top.0, !top.1),          // bt → at
        bld.directed(va, at, ab), // at → ab
    ];
    let surface = if req(&a.0, &b.0) {
        FaceSurface::Plane // radial (σ = const): affine in (μ, w)
    } else {
        let (lo, hi) = if a.0.cmp(&b.0) == core::cmp::Ordering::Less {
            (a.0.clone(), b.0.clone())
        } else {
            (b.0.clone(), a.0.clone())
        };
        ruled_common(
            &trim_surf(c, r, n, &a.1, wlo),
            &trim_surf(c, r, n, &a.1, whi),
            &lo,
            &hi,
        )
    };
    bld.brep.add_face(surface, wire);
}

/// One slice's `(σ, μ̂)` footprint = the band strip `[sk, sk1] × [μ̂_in, μ̂_out]` minus the holes
/// touching it, as a list of `(outer-loop, interior-hole-loops)` faces, every loop CCW-in-`(σ,μ)`
/// (holes wound CW). A hole strictly inside contributes a hole loop; a hole reaching a slice edge
/// opens the outer boundary as a **notch**; a hole spanning the full σ-width splits the lid into two
/// μ-bands. Holes are pairwise disjoint in σ (checked by the caller), so a slice has at most one
/// left-touch, one right-touch, or one span hole. `None` if the disjointness is violated locally.
#[allow(clippy::type_complexity)]
fn slice_footprint<B: Backend>(
    sk: &Rat<B>,
    sk1: &Rat<B>,
    mu_in: &RatFunc<B>,
    mu_out: &RatFunc<B>,
    holes: &[HoleSlice<'_, B>],
) -> Option<Vec<SliceFace<B>>> {
    use core::cmp::Ordering::{Greater, Less};
    let inn = |s: &Rat<B>| (s.clone(), mu_in.clone());
    let out = |s: &Rat<B>| (s.clone(), mu_out.clone());

    let mut left: Option<&HoleSlice<'_, B>> = None; // reaches the left edge (s1 ≤ sk < s2)
    let mut right: Option<&HoleSlice<'_, B>> = None; // reaches the right edge (s1 < sk1 ≤ s2)
    let mut span: Option<&HoleSlice<'_, B>> = None; // spans the whole slice (s1 ≤ sk, sk1 ≤ s2)
    let mut interior: Vec<&HoleSlice<'_, B>> = Vec::new();
    for h in holes {
        let a = if h.s1.cmp(sk) == Greater { &h.s1 } else { sk };
        let b = if h.s2.cmp(sk1) == Less { &h.s2 } else { sk1 };
        if a.cmp(b) != Less {
            continue; // does not overlap this slice
        }
        let touch_l = h.s1.cmp(sk) != Greater; // s1 ≤ sk
        let touch_r = h.s2.cmp(sk1) != Less; // s2 ≥ sk1
        match (touch_l, touch_r) {
            (false, false) => interior.push(h),
            (true, false) if left.is_none() => left = Some(h),
            (false, true) if right.is_none() => right = Some(h),
            (true, true) if span.is_none() => span = Some(h),
            _ => return None, // two holes touch the same edge — disjointness broke down
        }
    }

    if let Some(h) = span {
        // The hole cuts clear across σ: a bottom band [μ̂_in, near] and a top band [far, μ̂_out].
        if left.is_some() || right.is_some() || !interior.is_empty() {
            return None;
        }
        let near = |s: &Rat<B>| (s.clone(), (*h.near).clone());
        let far = |s: &Rat<B>| (s.clone(), (*h.far).clone());
        let bot = vec![inn(sk), inn(sk1), near(sk1), near(sk)];
        let top = vec![far(sk), far(sk1), out(sk1), out(sk)];
        return Some(vec![(bot, Vec::new()), (top, Vec::new())]);
    }

    // Outer loop: inner rail, up the right edge (with a right-notch), outer rail back, down the left
    // edge (with a left-notch). Consecutive corners share σ (radial) or rail (Bézier) by construction.
    let mut outer = vec![inn(sk), inn(sk1)];
    if let Some(h) = right {
        outer.push((sk1.clone(), (*h.near).clone())); // CR_lo up
        outer.push((h.s1.clone(), (*h.near).clone())); // near rail back
        outer.push((h.s1.clone(), (*h.far).clone())); // hole cap up
        outer.push((sk1.clone(), (*h.far).clone())); // far rail forward
    }
    outer.push(out(sk1)); // (CR_hi up, or the full right radial)
    outer.push(out(sk)); // outer rail back
    if let Some(h) = left {
        outer.push((sk.clone(), (*h.far).clone())); // CR_hi down
        outer.push((h.s2.clone(), (*h.far).clone())); // far rail forward
        outer.push((h.s2.clone(), (*h.near).clone())); // hole cap down
        outer.push((sk.clone(), (*h.near).clone())); // near rail back (→ CR_lo closes to inn(sk))
    }

    let hole_loops = interior
        .iter()
        .map(|h| {
            vec![
                (h.s1.clone(), (*h.near).clone()),
                (h.s1.clone(), (*h.far).clone()),
                (h.s2.clone(), (*h.far).clone()),
                (h.s2.clone(), (*h.near).clone()),
            ]
        })
        .collect();
    Some(vec![(outer, hole_loops)])
}

/// A trimmed cone solid over a **curved generalized band** `μ ∈ [μ̂_inner(σ), μ̂_outer(σ)]`,
/// `σ ∈ [σ_lo, σ_hi]`, extruded through `w`. `inner`/`outer` are the two μ-boundaries as **piecewise**
/// ruling-rails — ordered, contiguous `(σ-range, μ̂)` pieces covering `[σ_lo, σ_hi]` (σ-increasing);
/// the outer boundary carries the D3 notch as its middle piece(s).
///
/// Generalizes [`brep_freeboundary_slab`] to curved, *piecewise* μ-boundaries: σ is subdivided at
/// [`sigma_splits`] ∪ the piece boundaries so every ruled patch is single-span. Each slice's `(σ,μ̂)`
/// **footprint** — the band strip minus the holes touching it ([`slice_footprint`]) — is lifted to a
/// top (`w_hi`, forward) and bottom (`w_lo`, reversed) lid, and every footprint edge is swept through
/// the thickness as a wall ([`emit_trim_wall`]) *unless* it is a cross-ring radial shared by two
/// slices. A boundary edge that varies in σ is a rail [`RationalBezier`] (`c+μ̂·r+w·n`); a `σ=const`
/// edge is a straight radial [`Line`]. Watertight by the `Builder`'s vertex + edge dedup (the notch
/// corner `μ̂_D1=μ̂_D3` and every shared rail/radial deduplicate).
///
/// Each [`HoleRail`] in `holes` is a **through-hole** (`+1` genus): strictly interior in σ, holes
/// pairwise disjoint in σ. A hole strictly inside a slice is an annular inner loop; a hole reaching a
/// σ-station opens onto the cross-ring as a curved **notch**; a hole spanning a slice splits its lid
/// into two μ-bands — all handled uniformly by [`slice_footprint`]. The certified STEP builder for the
/// xy-trimmed panel (gap G-C). `None` on empty boundaries, a degenerate partition, a hole not strictly
/// interior in σ, or σ-overlapping holes.
pub fn brep_trim_solid<B: Backend>(
    chart: &Chart<B>,
    w: &Interval<B>,
    inner: &[(Interval<B>, RatFunc<B>)],
    outer: &[(Interval<B>, RatFunc<B>)],
    holes: &[HoleRail<B>],
) -> Option<Brep<B>> {
    use core::cmp::Ordering;
    if inner.is_empty() || outer.is_empty() {
        return None;
    }
    let sigma_lo = inner[0].0.lo.clone();
    let sigma_hi = inner.last()?.0.hi.clone();
    let c = chart.pedal().reduce();
    let r = chart.ruling().reduce();
    let n = chart.normal().reduce();
    let ws = [w.lo.clone(), w.hi.clone()];

    // σ-stations: the intrinsic positive-weight partition ∪ every piece boundary (so each slice lies
    // within a single inner and a single outer piece).
    let sigma = Interval {
        lo: sigma_lo,
        hi: sigma_hi,
    };
    let mut stations = sigma_stations(chart, &sigma, w, &inner[0].1, &outer[0].1);
    for (iv, _) in inner.iter().chain(outer.iter()) {
        stations.push(iv.lo.clone());
        stations.push(iv.hi.clone());
    }
    // A hole's chains change piece inside its σ-extent; each such σ must be a station, or a slice
    // would straddle two pieces and `HoleRail::at` would pick only one of them. The tangent σ are
    // deliberately NOT added (a slice boundary flush with a hole's σ-cap makes OCCT see
    // intersecting wires) — a hole still reaches a station as a curved notch.
    for h in holes {
        stations.extend(h.interior_breaks());
    }
    stations.sort();
    stations.dedup();
    let nst = stations.len();
    if nst < 2 {
        return None;
    }

    // Each hole is a **through-hole**: strictly interior in σ to the panel (never a boundary slot),
    // `s1 < s2`, and holes are pairwise **disjoint in σ** (each notches or spans slices on its own).
    // A hole's σ are deliberately **not** stations — aligning a slice boundary to a hole would make
    // its σ-cap flush with the slice's σ-cap (OCCT `IntersectingWires`); instead a hole reaching a
    // station opens onto the cross-ring as a curved **notch** (`slice_footprint`).
    for h in holes {
        if !(sigma.lo.cmp(&h.s1) == Ordering::Less
            && h.s1.cmp(&h.s2) == Ordering::Less
            && h.s2.cmp(&sigma.hi) == Ordering::Less)
        {
            return None;
        }
    }
    for i in 0..holes.len() {
        for j in i + 1..holes.len() {
            let (a, b) = (&holes[i], &holes[j]);
            let disjoint =
                a.s2.cmp(&b.s1) != Ordering::Greater || b.s2.cmp(&a.s1) != Ordering::Greater;
            if !disjoint {
                return None;
            }
        }
    }

    // Polynomialize + stitch both chains once, so every rail is low-degree and adjacent pieces share
    // their corner exactly (watertight notch). The σ-ranges are unchanged, so `stations` still hold.
    let inner = stitched_poly_chain(inner);
    let outer = stitched_poly_chain(outer);
    let holes: Vec<HoleRail<B>> = holes
        .iter()
        .map(|h| HoleRail {
            near: poly_chain(&h.near),
            far: poly_chain(&h.far),
            s1: h.s1.clone(),
            s2: h.s2.clone(),
        })
        .collect();

    // The developable σ-rail `c + μ̂·r + w·n` (μ̂ a polynomial) at thickness level `wl`, its μ-base
    // reduced (low-degree, positive weights — OCCT-friendly, like the legacy builder). Two rails at
    // the *same* μ̂ (a wall's two thickness rails) then share a denominator; the lids do not rule
    // between different-μ̂ rails (see below), so no cross-μ̂ denominator match is required.
    let surf =
        |mu_hat: &RatFunc<B>, wl: &Rat<B>| c.add(&r.scale(mu_hat)).reduce().add(&n.scale_rat(wl));

    // A radial at an *interior* σ-station is a cross-ring shared by the two adjacent lids — no wall.
    let interior_station = |s: &Rat<B>| stations[1..nst - 1].iter().any(|st| req(st, s));

    let mut bld = Builder::new();
    for k in 0..nst - 1 {
        let (sk, sk1) = (&stations[k], &stations[k + 1]);
        let smid = sk.add(sk1).mul(&Rat::new(1, 2));
        let mu_in = piece_at(&inner, &smid)?.clone();
        let mu_out = piece_at(&outer, &smid)?.clone();
        let slice_holes: Vec<HoleSlice<'_, B>> = holes
            .iter()
            .filter_map(|h| h.at(&clamp_to(&h.s1, &h.s2, &smid)))
            .collect();
        let faces: Vec<SliceFace<B>> = slice_footprint(sk, sk1, &mu_in, &mu_out, &slice_holes)?
            .into_iter()
            .map(|(o, hs)| {
                (
                    dedup_trim_corners(&o),
                    hs.iter().map(|h| dedup_trim_corners(h)).collect(),
                )
            })
            .collect();
        for (outer_loop, hole_loops) in &faces {
            // Lid patch: the cone ruled between this slice's inner and outer rails — it contains every
            // notched/banded footprint face, so OCCT just trims it to the wire (`ruled_common` shares
            // the two different-μ̂ rails' denominator).
            let surf_top = ruled_common(&surf(&mu_in, &ws[1]), &surf(&mu_out, &ws[1]), sk, sk1);
            let surf_bot = ruled_common(&surf(&mu_in, &ws[0]), &surf(&mu_out, &ws[0]), sk, sk1);
            // Top (whi) lid forward, bottom (wlo) lid the same loops reversed → both face outward.
            let top_outer = lift_trim_loop(&mut bld, &c, &r, &n, outer_loop, &ws[1]);
            let top_holes: Vec<Vec<HalfEdge>> = hole_loops
                .iter()
                .map(|h| lift_trim_loop(&mut bld, &c, &r, &n, h, &ws[1]))
                .collect();
            let bot_outer = reversed_wire(lift_trim_loop(&mut bld, &c, &r, &n, outer_loop, &ws[0]));
            let bot_holes: Vec<Vec<HalfEdge>> = hole_loops
                .iter()
                .map(|h| reversed_wire(lift_trim_loop(&mut bld, &c, &r, &n, h, &ws[0])))
                .collect();
            bld.brep.add_face_with_holes(surf_top, top_outer, top_holes);
            bld.brep.add_face_with_holes(surf_bot, bot_outer, bot_holes);
            // One wall per footprint edge, except a shared cross-ring radial (an interior station).
            for corners in core::iter::once(outer_loop).chain(hole_loops.iter()) {
                let m = corners.len();
                for i in 0..m {
                    let a = &corners[i];
                    let b = &corners[(i + 1) % m];
                    if req(&a.0, &b.0) && interior_station(&a.0) {
                        continue;
                    }
                    emit_trim_wall(&mut bld, &c, &r, &n, a, b, &ws[0], &ws[1]);
                }
            }
        }
    }
    Some(bld.into_brep())
}

/// A **trimmed developable solid** whose surface is **piecewise across σ-regions** that share the
/// ruling/normal frame (`r`, `n`) but differ in support (pedal `c`) — the self-lapping / offset-tail
/// device. `charts` are the region charts with their σ-sub-bands (contiguous, ascending); `inner` /
/// `outer` the piecewise ruling rails; `w` the thickness. Region joins land on σ-stations, and where
/// the pedal is continuous across a join (matching support *and* slope, so the rails coincide there)
/// the two adjacent slices share their cross-ring **exactly** — one watertight connected shell, no
/// internal cap. [`brep_trim_solid`] is the single-region special case.
///
/// The per-region intrinsic positive-weight partition (from each chart's own [`sigma_stations`]) and
/// every region/rail-piece boundary become stations, so each slice lies within one region, one inner
/// and one outer rail piece; its `c, r, n` are read from that region's chart.
/// A general `(σ,µ̂)` polygon hole loop as `TrimCorner`s: corner `i` carries the **line** through
/// `(σ_i,µ̂_i)` and the next vertex (its outgoing edge's rail), or the constant `µ̂_i` for a radial
/// (`σ=const`) edge. So the lifting reads each edge as a straight `(σ,µ̂)` segment — a round drill or a
/// polygon cut, not just a near/far µ̂-band.
fn poly_to_corners<B: Backend>(poly: &[(Rat<B>, Rat<B>)]) -> Vec<TrimCorner<B>> {
    use core::cmp::Ordering;
    let m = poly.len();
    (0..m)
        .map(|i| {
            let (s0, m0) = &poly[i];
            let (s1, m1) = &poly[(i + 1) % m];
            let rail = if s0.cmp(s1) == Ordering::Equal {
                RatFunc::from_poly(Poly::constant(m0.clone()))
            } else {
                let slope = m1.sub(m0).div(&s1.sub(s0));
                let intercept = m0.sub(&slope.mul(s0));
                RatFunc::from_poly(Poly::from_coeffs(vec![intercept, slope]))
            };
            (s0.clone(), rail)
        })
        .collect()
}

pub fn brep_trim_solid_regions<B: Backend>(
    charts: &[(Interval<B>, &Chart<B>)],
    w: &Interval<B>,
    inner: &[(Interval<B>, RatFunc<B>)],
    outer: &[(Interval<B>, RatFunc<B>)],
    holes: &[HoleRail<B>],
    poly_holes: &[Vec<(Rat<B>, Rat<B>)>],
) -> Option<Brep<B>> {
    use core::cmp::Ordering;
    if inner.is_empty() || outer.is_empty() || charts.is_empty() {
        return None;
    }
    let sigma = Interval {
        lo: inner[0].0.lo.clone(),
        hi: inner.last()?.0.hi.clone(),
    };
    let ws = [w.lo.clone(), w.hi.clone()];

    // The piece of a piecewise boundary covering a σ.
    // The region chart covering σ (by containment).
    // The reduced surface fields, **once per region** rather than once per slice. `reduce()` is a
    // polynomial gcd over degree-24 denominators, and the slice count is now driven by the hole
    // chains' piece boundaries, so recomputing these per slice made the cost scale with hole
    // fidelity — the dominant term in the build, not the face count.
    let region_fields: Vec<(Vec3Rat<B>, Vec3Rat<B>, Vec3Rat<B>)> = charts
        .iter()
        .map(|(_, ch)| {
            (
                ch.pedal().reduce(),
                ch.ruling().reduce(),
                ch.normal().reduce(),
            )
        })
        .collect();
    let region_at = |s: &Rat<B>| -> Option<usize> {
        charts.iter().position(|(iv, _)| {
            iv.lo.cmp(s) != Ordering::Greater && s.cmp(&iv.hi) != Ordering::Greater
        })
    };

    // σ-stations: each region's own positive-weight partition ∪ every region and rail-piece boundary.
    let mut stations = Vec::new();
    for (iv, ch) in charts {
        let rmid = iv.lo.add(&iv.hi).mul(&Rat::new(1, 2));
        let in_p = piece_at(inner, &rmid)?.clone();
        let out_p = piece_at(outer, &rmid)?.clone();
        stations.extend(sigma_stations(ch, iv, w, &in_p, &out_p));
        stations.push(iv.lo.clone());
        stations.push(iv.hi.clone());
    }
    for (iv, _) in inner.iter().chain(outer.iter()) {
        stations.push(iv.lo.clone());
        stations.push(iv.hi.clone());
    }
    // A hole's chains change piece inside its σ-extent; each such σ must be a station, or a slice
    // would straddle two pieces and `HoleRail::at` would pick only one of them. The tangent σ are
    // deliberately NOT added (a slice boundary flush with a hole's σ-cap makes OCCT see
    // intersecting wires) — a hole still reaches a station as a curved notch.
    for h in holes {
        stations.extend(h.interior_breaks());
    }
    stations.sort();
    stations.dedup();
    let nst = stations.len();
    if nst < 2 {
        return None;
    }

    // Holes: strictly interior in σ, `s1 < s2`, pairwise disjoint in σ (as `brep_trim_solid`).
    for h in holes {
        if !(sigma.lo.cmp(&h.s1) == Ordering::Less
            && h.s1.cmp(&h.s2) == Ordering::Less
            && h.s2.cmp(&sigma.hi) == Ordering::Less)
        {
            return None;
        }
    }
    for i in 0..holes.len() {
        for j in i + 1..holes.len() {
            let (a, b) = (&holes[i], &holes[j]);
            let disjoint =
                a.s2.cmp(&b.s1) != Ordering::Greater || b.s2.cmp(&a.s1) != Ordering::Greater;
            if !disjoint {
                return None;
            }
        }
    }

    // Polygon holes: each must lie strictly interior to a single slice (its whole σ-extent inside one
    // `[station_k, station_{k+1}]`) — a general `(σ,µ̂)` loop cut from that slice's lid, unlike a
    // `HoleRail` (which is a near/far µ̂-band and can span slices via the σ-caps).
    let poly_bounds: Vec<(Rat<B>, Rat<B>)> = poly_holes
        .iter()
        .map(|p| {
            let mut lo = p[0].0.clone();
            let mut hi = p[0].0.clone();
            for (s, _) in p {
                if s.cmp(&lo) == Ordering::Less {
                    lo = s.clone();
                }
                if s.cmp(&hi) == Ordering::Greater {
                    hi = s.clone();
                }
            }
            (lo, hi)
        })
        .collect();
    for (lo, hi) in &poly_bounds {
        let inside = (0..nst - 1).any(|k| {
            stations[k].cmp(lo) == Ordering::Less && hi.cmp(&stations[k + 1]) == Ordering::Less
        });
        if !inside {
            return None; // crosses a σ-station (would need per-slice clipping)
        }
    }

    let inner = stitched_poly_chain(inner);
    let outer = stitched_poly_chain(outer);
    let holes: Vec<HoleRail<B>> = holes
        .iter()
        .map(|h| HoleRail {
            near: poly_chain(&h.near),
            far: poly_chain(&h.far),
            s1: h.s1.clone(),
            s2: h.s2.clone(),
        })
        .collect();

    let interior_station = |s: &Rat<B>| stations[1..nst - 1].iter().any(|st| req(st, s));

    let mut bld = Builder::new();
    for k in 0..nst - 1 {
        let (sk, sk1) = (&stations[k], &stations[k + 1]);
        let smid = sk.add(sk1).mul(&Rat::new(1, 2));
        // The region's surface fields (only the pedal `c` varies across a shared-frame device).
        let (c, r, n) = &region_fields[region_at(&smid)?];
        let surf = |mu_hat: &RatFunc<B>, wl: &Rat<B>| {
            c.add(&r.scale(mu_hat)).reduce().add(&n.scale_rat(wl))
        };
        let mu_in = piece_at(&inner, &smid)?.clone();
        let mu_out = piece_at(&outer, &smid)?.clone();
        let slice_holes: Vec<HoleSlice<'_, B>> = holes
            .iter()
            .filter_map(|h| h.at(&clamp_to(&h.s1, &h.s2, &smid)))
            .collect();
        let mut faces: Vec<SliceFace<B>> = slice_footprint(sk, sk1, &mu_in, &mu_out, &slice_holes)?
            .into_iter()
            .map(|(o, hs)| {
                (
                    dedup_trim_corners(&o),
                    hs.iter().map(|h| dedup_trim_corners(h)).collect(),
                )
            })
            .collect();
        // Append the polygon holes interior to this slice as inner loops of its (outer) lid face.
        let extra: Vec<Vec<TrimCorner<B>>> = poly_holes
            .iter()
            .zip(&poly_bounds)
            .filter(|(_, (lo, hi))| sk.cmp(lo) == Ordering::Less && hi.cmp(sk1) == Ordering::Less)
            .map(|(p, _)| poly_to_corners(p))
            .collect();
        if !extra.is_empty() {
            if let Some(first) = faces.first_mut() {
                first.1.extend(extra);
            }
        }
        for (outer_loop, hole_loops) in &faces {
            let surf_top = ruled_common(&surf(&mu_in, &ws[1]), &surf(&mu_out, &ws[1]), sk, sk1);
            let surf_bot = ruled_common(&surf(&mu_in, &ws[0]), &surf(&mu_out, &ws[0]), sk, sk1);
            let top_outer = lift_trim_loop(&mut bld, c, r, n, outer_loop, &ws[1]);
            let top_holes: Vec<Vec<HalfEdge>> = hole_loops
                .iter()
                .map(|h| lift_trim_loop(&mut bld, c, r, n, h, &ws[1]))
                .collect();
            let bot_outer = reversed_wire(lift_trim_loop(&mut bld, c, r, n, outer_loop, &ws[0]));
            let bot_holes: Vec<Vec<HalfEdge>> = hole_loops
                .iter()
                .map(|h| reversed_wire(lift_trim_loop(&mut bld, c, r, n, h, &ws[0])))
                .collect();
            bld.brep.add_face_with_holes(surf_top, top_outer, top_holes);
            bld.brep.add_face_with_holes(surf_bot, bot_outer, bot_holes);
            for corners in core::iter::once(outer_loop).chain(hole_loops.iter()) {
                let m = corners.len();
                for i in 0..m {
                    let a = &corners[i];
                    let b = &corners[(i + 1) % m];
                    if req(&a.0, &b.0) && interior_station(&a.0) {
                        continue;
                    }
                    emit_trim_wall(&mut bld, c, r, n, a, b, &ws[0], &ws[1]);
                }
            }
        }
    }
    Some(bld.into_brep())
}

// ============================================================================
// The general holed construction's `(σ,μ)`→3-D lifting layer. Each slice's lid
// region comes from the exact `arrange2d` boolean; these free functions lift its
// loops and rim edges to the developable, deduping every shared edge so the
// lids, μ-walls, σ-caps, and hole tubes stitch watertight by identity.
// ============================================================================

/// The constant value of a `RatFunc` if it is constant (num and den both degree 0), else `None` —
/// the panel μ-band must be constant for the holed path (a curved ∂P is not a polygon operand).
fn ratfunc_const<B: Backend>(rf: &RatFunc<B>) -> Option<Rat<B>> {
    let rf = rf.reduce();
    let nd = rf.num().degree().unwrap_or(0);
    let dd = rf.den().degree().unwrap_or(0);
    if nd == 0 && dd == 0 {
        rf.eval(&Rat::from_i128(0))
    } else {
        None
    }
}

/// The rational value of an arrangement endpoint coordinate. The strip and every hole is an
/// axis-aligned *rational* rectangle, so their boolean's vertices are rational — a degenerate
/// [`Surd`] with no radical part.
fn surd_rat<B: Backend>(s: &Surd<B>) -> Rat<B> {
    let (a, b, d) = s.parts();
    debug_assert!(
        b.is_zero() || d.is_zero(),
        "an axis-aligned rational arrangement has rational vertices"
    );
    a.clone()
}

/// A rational `(σ, μ)` point in the panel parameter domain.
type SigMu<B> = (Rat<B>, Rat<B>);

/// The two `(σ, μ)` rational endpoints of an arrangement edge.
fn edge_endpoints<B: Backend>(e: &ArrEdge<B>) -> (SigMu<B>, SigMu<B>) {
    let (s, t) = match e {
        ArrEdge::Seg(sp) => (&sp.start, &sp.end),
        ArrEdge::Arc(ap) => (&ap.start, &ap.end),
    };
    (
        (surd_rat(&s.x), surd_rat(&s.y)),
        (surd_rat(&t.x), surd_rat(&t.y)),
    )
}

/// One axis-aligned polygon edge `(sx, sy) → (ex, ey)` as an exact [`arrange2d`] segment tagged with
/// operand source `src` — the directed carrier line passes through both endpoints exactly (as in
/// `develop::flat`, so the CAP-IN pre-pass accepts it).
fn seg_edge<B: Backend>(
    sx: &Rat<B>,
    sy: &Rat<B>,
    ex: &Rat<B>,
    ey: &Rat<B>,
    src: u32,
) -> ArrEdge<B> {
    let a = ey.sub(sy).neg();
    let b = ex.sub(sx);
    let c = a.mul(sx).add(&b.mul(sy)).neg();
    ArrEdge::Seg(Box::new(SegPiece {
        line: Line { a, b, c },
        start: Point2::from_rat(sx.clone(), sy.clone()),
        end: Point2::from_rat(ex.clone(), ey.clone()),
        orient: Orient::Ccw,
        source: CurveId(src),
    }))
}

/// The four CCW segment edges of the axis-aligned `(σ,μ)` rectangle `[slo,shi]×[mlo,mhi]`, source `src`.
fn rect_edges<B: Backend>(
    slo: &Rat<B>,
    shi: &Rat<B>,
    mlo: &Rat<B>,
    mhi: &Rat<B>,
    src: u32,
) -> Vec<ArrEdge<B>> {
    vec![
        seg_edge(slo, mlo, shi, mlo, src),
        seg_edge(shi, mlo, shi, mhi, src),
        seg_edge(shi, mhi, slo, mhi, src),
        seg_edge(slo, mhi, slo, mlo, src),
    ]
}

/// `a == b` for a rational.
fn req<B: Backend>(a: &Rat<B>, b: &Rat<B>) -> bool {
    a.sub(b).sign() == 0
}

/// `(σ,μ)` point equality.
fn pt_eq<B: Backend>(p: &(Rat<B>, Rat<B>), q: &(Rat<B>, Rat<B>)) -> bool {
    req(&p.0, &q.0) && req(&p.1, &q.1)
}

/// Twice the signed area of a `(σ,μ)` polygon (shoelace) — positive iff CCW.
fn signed_area2<B: Backend>(poly: &[(Rat<B>, Rat<B>)]) -> Rat<B> {
    let n = poly.len();
    let mut acc = Rat::from_i128(0);
    for i in 0..n {
        let (sx, sy) = &poly[i];
        let (tx, ty) = &poly[(i + 1) % n];
        acc = acc.add(&sx.mul(ty)).sub(&tx.mul(sy));
    }
    acc
}

/// Reconstruct an arrangement boundary loop as an ordered `(σ,μ)` corner list, oriented CCW iff
/// `want_ccw`. The loop is a simple cycle (each vertex degree 2), walked here by an adjacency chase
/// robust to the emitted edge order, then flipped to the requested winding by its signed area.
/// `None` for a degenerate (< 3-vertex) or broken loop.
fn ordered_loop<B: Backend>(loop_edges: &[ArrEdge<B>], want_ccw: bool) -> Option<Vec<SigMu<B>>> {
    let segs: Vec<(SigMu<B>, SigMu<B>)> = loop_edges.iter().map(edge_endpoints).collect();
    let n = segs.len();
    if n < 3 {
        return None;
    }
    let mut used = vec![false; n];
    let mut poly = vec![segs[0].0.clone(), segs[0].1.clone()];
    used[0] = true;
    while poly.len() < n {
        let cur = poly.last().expect("nonempty").clone();
        let mut found = false;
        for i in 0..n {
            if used[i] {
                continue;
            }
            if pt_eq(&segs[i].0, &cur) {
                poly.push(segs[i].1.clone());
                used[i] = true;
                found = true;
                break;
            }
            if pt_eq(&segs[i].1, &cur) {
                poly.push(segs[i].0.clone());
                used[i] = true;
                found = true;
                break;
            }
        }
        if !found {
            return None;
        }
    }
    let is_ccw = signed_area2(&poly).sign() > 0;
    if is_ccw != want_ccw {
        poly.reverse();
    }
    Some(poly)
}

/// The developable σ-rail `c + μ·r + w·n` at a scalar `(μ, w)`, reduced (low degree; the two rails of
/// a same-`μ` patch share a denominator — [`RatBezierSurface::ruled_from_rails`]'s precondition).
fn rail_vec<B: Backend>(
    c: &Vec3Rat<B>,
    r: &Vec3Rat<B>,
    n: &Vec3Rat<B>,
    mu: &Rat<B>,
    w: &Rat<B>,
) -> Vec3Rat<B> {
    c.add(&r.scale_rat(mu)).reduce().add(&n.scale_rat(w))
}

/// The deduped 3-D vertex id of `(σ, μ)` lifted to thickness level `w`.
fn pt3d<B: Backend>(
    bld: &mut Builder<B>,
    c: &Vec3Rat<B>,
    r: &Vec3Rat<B>,
    n: &Vec3Rat<B>,
    sigma: &Rat<B>,
    mu: &Rat<B>,
    w: &Rat<B>,
) -> usize {
    let p = rail_vec(c, r, n, mu, w)
        .eval(sigma)
        .expect("free-boundary point finite");
    bld.vertex(&p)
}

/// Lift one `(σ,μ)` boundary edge `a → b` (at thickness level `w`) to a directed 3-D half-edge,
/// deduped: a **horizontal** edge (`μ = const`, σ varies) is a σ-rail Bézier; a **vertical** edge
/// (`σ = const`, μ varies) is a straight radial [`Line`] (`c + μr + wn` is affine in `(μ, w)` at
/// fixed σ). Both endpoints deduplicate by exact coordinate.
fn lift_edge<B: Backend>(
    bld: &mut Builder<B>,
    c: &Vec3Rat<B>,
    r: &Vec3Rat<B>,
    n: &Vec3Rat<B>,
    a: &(Rat<B>, Rat<B>),
    b: &(Rat<B>, Rat<B>),
    w: &Rat<B>,
) -> HalfEdge {
    let va = pt3d(bld, c, r, n, &a.0, &a.1, w);
    let vb = pt3d(bld, c, r, n, &b.0, &b.1, w);
    if req(&a.1, &b.1) {
        // Horizontal: μ = const → σ-rail over [min σ, max σ].
        let (lo, hi) = if a.0.sub(&b.0).sign() <= 0 {
            (a.0.clone(), b.0.clone())
        } else {
            (b.0.clone(), a.0.clone())
        };
        let rv = rail_vec(c, r, n, &a.1, w);
        let eid = bld.rail_edge(&rv, &Interval { lo, hi });
        bld.directed(eid, va, vb)
    } else {
        // Vertical: σ = const → straight radial line.
        let eid = bld.line_edge(va, vb);
        bld.directed(eid, va, vb)
    }
}

/// Lift a whole normalized loop (ordered corners) to a forward wire at thickness level `w`.
fn lift_loop_forward<B: Backend>(
    bld: &mut Builder<B>,
    c: &Vec3Rat<B>,
    r: &Vec3Rat<B>,
    n: &Vec3Rat<B>,
    corners: &[(Rat<B>, Rat<B>)],
    w: &Rat<B>,
) -> Vec<HalfEdge> {
    let m = corners.len();
    (0..m)
        .map(|i| lift_edge(bld, c, r, n, &corners[i], &corners[(i + 1) % m], w))
        .collect()
}

/// Reverse a wire's orientation: reverse the order and flip every half-edge, so it traverses the
/// same edges the opposite way (the bottom lid is the top lid's loop reversed).
fn reversed_wire(mut wire: Vec<HalfEdge>) -> Vec<HalfEdge> {
    wire.reverse();
    for h in &mut wire {
        h.1 = !h.1;
    }
    wire
}

/// Whether a `(σ,μ)` boundary edge `a → b` of a slice is a **hole-rim** edge (gets a tube wall) — as
/// opposed to a panel edge (`μ = mlo/mhi`, shared with a μ-wall) or a σ-station radial (`σ = sk/sk1`,
/// shared with the neighbouring slice's lid).
fn is_rim_edge<B: Backend>(
    a: &(Rat<B>, Rat<B>),
    b: &(Rat<B>, Rat<B>),
    sk: &Rat<B>,
    sk1: &Rat<B>,
    mlo: &Rat<B>,
    mhi: &Rat<B>,
) -> bool {
    if req(&a.1, &b.1) {
        // horizontal (μ = const): rim iff not on a panel μ-boundary.
        !(req(&a.1, mlo) || req(&a.1, mhi))
    } else {
        // vertical (σ = const): rim iff not on a σ-station (slice boundary).
        !(req(&a.0, sk) || req(&a.0, sk1))
    }
}

/// Emit one hole-rim **tube wall** for the `(σ,μ)` rim edge `a → b` (in arrangement/top-lid order),
/// sweeping it through the thickness `[wlo, whi]`. Its bottom/top edges are the same σ-rail/radial
/// the two lids carry (deduped → shared, 2-incident), and its two verticals are shared with the
/// adjacent walls; the wall is a ruled patch for a σ-rim edge (`μ = const`) and planar for a μ-rim
/// edge (`σ = const`). The winding is the reverse of each lid's use of the shared edge (bottom lid =
/// the forward loop reversed, top lid = the forward loop), so every rim edge is once-each-way.
#[allow(clippy::too_many_arguments)]
fn emit_tube<B: Backend>(
    bld: &mut Builder<B>,
    c: &Vec3Rat<B>,
    r: &Vec3Rat<B>,
    n: &Vec3Rat<B>,
    a: &(Rat<B>, Rat<B>),
    b: &(Rat<B>, Rat<B>),
    wlo: &Rat<B>,
    whi: &Rat<B>,
) {
    let ab = pt3d(bld, c, r, n, &a.0, &a.1, wlo);
    let at = pt3d(bld, c, r, n, &a.0, &a.1, whi);
    let bb = pt3d(bld, c, r, n, &b.0, &b.1, wlo);
    let bt = pt3d(bld, c, r, n, &b.0, &b.1, whi);
    let bottom_e = lift_edge(bld, c, r, n, a, b, wlo); // ab → bb
    let top_e = lift_edge(bld, c, r, n, a, b, whi); // at → bt
    let va = bld.line_edge(ab, at);
    let vb = bld.line_edge(bb, bt);
    let wire = vec![
        bottom_e,                 // ab → bb
        bld.directed(vb, bb, bt), // bb → bt
        (top_e.0, !top_e.1),      // bt → at  (top_e runs at → bt)
        bld.directed(va, at, ab), // at → ab
    ];
    let surface = if req(&a.1, &b.1) {
        // σ-rim edge (μ = const): ruled through the thickness.
        let (slo, shi) = if a.0.sub(&b.0).sign() <= 0 {
            (a.0.clone(), b.0.clone())
        } else {
            (b.0.clone(), a.0.clone())
        };
        ruled_panel(
            &rail_vec(c, r, n, &a.1, wlo),
            &rail_vec(c, r, n, &a.1, whi),
            &slo,
            &shi,
        )
    } else {
        // μ-rim edge (σ = const): planar (affine in (μ, w)).
        FaceSurface::Plane
    };
    bld.brep.add_face(surface, wire);
}

/// Emit one panel **μ-wall** (`μ = mu`; `is_hi` selects the μ⁺ vs μ⁻ winding) over slice `[sk, sk1]`:
/// a ruled patch swept through the thickness, its two σ-rails shared with the two lids and its two
/// verticals shared with the neighbouring slice's μ-wall and the σ-caps — all deduped.
#[allow(clippy::too_many_arguments)]
fn add_mu_wall<B: Backend>(
    bld: &mut Builder<B>,
    c: &Vec3Rat<B>,
    r: &Vec3Rat<B>,
    n: &Vec3Rat<B>,
    sk: &Rat<B>,
    sk1: &Rat<B>,
    mu: &Rat<B>,
    wlo: &Rat<B>,
    whi: &Rat<B>,
    is_hi: bool,
) {
    let a = pt3d(bld, c, r, n, sk, mu, wlo); // wlo @ sk
    let b = pt3d(bld, c, r, n, sk, mu, whi); // whi @ sk
    let cc = pt3d(bld, c, r, n, sk1, mu, whi); // whi @ sk1
    let d = pt3d(bld, c, r, n, sk1, mu, wlo); // wlo @ sk1
    let wv_k = bld.line_edge(a, b);
    let wv_k1 = bld.line_edge(d, cc);
    let a_pt = (sk.clone(), mu.clone());
    let b_pt = (sk1.clone(), mu.clone());
    let rail_wlo = lift_edge(bld, c, r, n, &a_pt, &b_pt, wlo); // a → d
    let rail_whi = lift_edge(bld, c, r, n, &a_pt, &b_pt, whi); // b → cc
    let wire = if is_hi {
        vec![
            bld.directed(wv_k, a, b),   // a → b
            rail_whi,                   // b → cc
            bld.directed(wv_k1, cc, d), // cc → d
            (rail_wlo.0, !rail_wlo.1),  // d → a
        ]
    } else {
        vec![
            bld.directed(wv_k, b, a),   // b → a
            rail_wlo,                   // a → d
            bld.directed(wv_k1, d, cc), // d → cc
            (rail_whi.0, !rail_whi.1),  // cc → b
        ]
    };
    let surface = ruled_panel(
        &rail_vec(c, r, n, mu, wlo),
        &rail_vec(c, r, n, mu, whi),
        sk,
        sk1,
    );
    bld.brep.add_face(surface, wire);
}

/// Emit one planar **σ-end cap** (the `(μ,w)` rectangle at σ = `s_end`): its two radials are shared
/// with the adjacent slice's lids and its two verticals with the μ-walls — all deduped. `is_hi`
/// selects the σ⁺ (else σ⁻) outward winding.
#[allow(clippy::too_many_arguments)]
fn add_sigma_cap<B: Backend>(
    bld: &mut Builder<B>,
    c: &Vec3Rat<B>,
    r: &Vec3Rat<B>,
    n: &Vec3Rat<B>,
    s_end: &Rat<B>,
    mlo: &Rat<B>,
    mhi: &Rat<B>,
    wlo: &Rat<B>,
    whi: &Rat<B>,
    is_hi: bool,
) {
    let a0 = pt3d(bld, c, r, n, s_end, mlo, wlo);
    let a1 = pt3d(bld, c, r, n, s_end, mhi, wlo);
    let a2 = pt3d(bld, c, r, n, s_end, mhi, whi);
    let a3 = pt3d(bld, c, r, n, s_end, mlo, whi);
    let radial_wlo = bld.line_edge(a0, a1); // μ⁻ → μ⁺ at wlo
    let radial_whi = bld.line_edge(a3, a2); // μ⁻ → μ⁺ at whi
    let wv_mlo = bld.line_edge(a0, a3); // wlo → whi at μ⁻
    let wv_mhi = bld.line_edge(a1, a2); // wlo → whi at μ⁺
    let wire = if is_hi {
        // σ = σhi: v0→v1→v2→v3.
        vec![
            bld.directed(radial_wlo, a0, a1),
            bld.directed(wv_mhi, a1, a2),
            bld.directed(radial_whi, a2, a3),
            bld.directed(wv_mlo, a3, a0),
        ]
    } else {
        // σ = σlo: v0→v3→v2→v1 (outward).
        vec![
            bld.directed(wv_mlo, a0, a3),
            bld.directed(radial_whi, a3, a2),
            bld.directed(wv_mhi, a2, a1),
            bld.directed(radial_wlo, a1, a0),
        ]
    };
    bld.brep.add_plane(wire);
}

/// [`brep_freeboundary`] specialized to a certified one-joint closure: flank A's chart over the
/// treatment's σ-support (`t.sigma_a`) and thickness window (`t.w`) — the D4.3b fixture path.
pub fn brep_freeboundary_from_closure<B: Backend>(
    joint: &Joint<B>,
    t: &ClosureTreatment<'_, B>,
    mu_lo: &RatFunc<B>,
    mu_hi: &RatFunc<B>,
) -> Brep<B> {
    brep_freeboundary(joint.flank_a().chart(), &t.sigma_a, &t.w, mu_lo, mu_hi)
}

/// The three searcher-proposed positivity margins for [`free_boundary_cert`]. Each is
/// verified by the checker (the searcher only proposes), so each must be strictly below the
/// true infimum of its quantity on the span, or [`free_boundary`](certify_core::free_boundary)
/// refutes it.
pub struct FreeBoundaryMargins<B: Backend> {
    /// Positive-width margin: `μ⁺(σ) − μ⁻(σ) ≥ width`.
    pub width: Rat<B>,
    /// Boundary-regularity margin (both rails): `|â′|² ≥ reg`.
    pub reg: Rat<B>,
    /// σ̂-monotonicity margin: `σ̂′ ≥ mono`.
    pub mono: Rat<B>,
}

/// Build the [`FreeBoundaryCert`] for an authored σ-band boundary `μ⁻(σ), μ⁺(σ)` lifted into
/// `chart` — the geometry→certificate **searcher** that ties
/// [`brep_freeboundary_from_closure`]'s solid to the trusted
/// [`free_boundary`](certify_core::free_boundary) checker (D4.3a). Untrusted: it forms the
/// exact obligation polynomials and their Sturm chains, which the checker re-verifies.
///
/// It clears the three exact-ANCHOR obligations to `RegCert`/`EdgeRegCert` form:
/// - **width** `μ⁺ − μ⁻ = num/den ≥ margins.width` (the reduced difference RatFunc);
/// - **boundary regularity** `|â′|² ≥ margins.reg` for each μ-rail (`|·|²` of the reduced
///   rail `c + μ±·r`'s derivative);
/// - **σ̂-monotonicity** `σ̂′ = num/den ≥ margins.mono` (the caller-supplied σ-projection
///   derivative `sigma_dot` — [`RatFunc::one`] for the σ-graph).
pub fn free_boundary_cert<B: Backend>(
    chart: &Chart<B>,
    mu_lo: &RatFunc<B>,
    mu_hi: &RatFunc<B>,
    sigma: &Interval<B>,
    sigma_dot: &RatFunc<B>,
    margins: &FreeBoundaryMargins<B>,
) -> FreeBoundaryCert<B> {
    // A REG-Q positivity cert `num/den ≥ m` with honest Sturm chains (den + residual).
    let reg = |rf: &RatFunc<B>, m: &Rat<B>| {
        let rf = rf.reduce();
        let (num, den) = (rf.num().clone(), rf.den().clone());
        let res = num.sub(&den.scale(m));
        RegCert {
            den_chain: SturmChain::new(&den),
            res_chain: SturmChain::new(&res),
            num,
            den,
            m: MarginSq(m.clone()),
            span: sigma.clone(),
        }
    };
    // The lifted μ-rail `c + μ·r` (at w = 0) and its squared speed `|â′|²`.
    let speed_sq = |mu: &RatFunc<B>| {
        let base = chart.pedal().add(&chart.ruling().scale(mu)).reduce();
        let d = base.derivative();
        d.dot(&d)
    };

    FreeBoundaryCert {
        span: sigma.clone(),
        width: reg(&mu_hi.sub(mu_lo), &margins.width),
        reg_lo: EdgeRegCert {
            speed_sq: reg(&speed_sq(mu_lo), &margins.reg),
            failure: None,
        },
        reg_hi: EdgeRegCert {
            speed_sq: reg(&speed_sq(mu_hi), &margins.reg),
            failure: None,
        },
        monotone: reg(sigma_dot, &margins.mono),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use certify_core::Verdict;
    use closure::valid::closure_valid;
    use fixtures::closure_joint::{ledge_d24, miter_cap, one_joint, treatment, treatment_miter};

    fn miter_brep() -> Brep<lattice::Bignum> {
        let joint = one_joint();
        let cap = miter_cap();
        let t = treatment_miter(&cap);
        let valid = match closure_valid(&joint, &t) {
            Verdict::Verified(v) => v,
            other => panic!(
                "the miter fold is CLOSURE_VALID: {}",
                matches!(other, Verdict::Verified(_))
            ),
        };
        brep_from_closure(&joint, &t, &valid)
    }

    /// The MITER B-rep is two closed-wire flank faces sharing exactly one crease edge by
    /// identity: `M` is a single 2-incidence edge, nothing is non-manifold, and the free
    /// edges are the honest-open substrate boundary (six rails/far edges + flank A's two
    /// overhang tips = 8).
    #[test]
    fn miter_brep_shares_the_crease_middle_by_identity() {
        let b = miter_brep();
        assert_eq!(b.faces().len(), 2, "two flank faces");
        assert!(b.indices_in_range());
        for f in 0..b.faces().len() {
            assert!(b.wire_is_closed(f), "flank wire {f} closes");
        }
        let inc = b.edge_incidence();
        assert_eq!(
            inc.iter().filter(|&&c| c == 2).count(),
            1,
            "exactly one shared (2-incidence) edge — the crease middle M"
        );
        assert_eq!(b.nonmanifold_edges(), 0, "no non-manifold edge");
        assert_eq!(
            b.free_edges(),
            8,
            "honest-open boundary: 2 rails + 1 far per flank + flank A's two overhang tips"
        );
    }

    /// The shared crease middle is exactly the overlap `x ∈ [−1, 1]` on the line
    /// `L = {(x, 0, 1)}`: flank A's crease spans `x ∈ [−2, 2]`, flank B's `x ∈ [−1, 1]`,
    /// so the shared edge is B's whole crease and A carries the `[−2,−1]` / `[1,2]` tips.
    #[test]
    fn the_shared_middle_is_the_crease_overlap() {
        let b = miter_brep();
        // The single 2-incidence edge runs between (−1,0,1) and (1,0,1).
        let inc = b.edge_incidence();
        let m = inc
            .iter()
            .position(|&c| c == 2)
            .expect("a shared edge exists");
        let e = &b.edges()[m];
        let pt = |v: usize| {
            let p = &b.verts()[v];
            [p[0].clone(), p[1].clone(), p[2].clone()]
        };
        let one = |v: i128| Surd::from_rat(Rat::<lattice::Bignum>::from_i128(v));
        assert_eq!(pt(e.start), [one(-1), one(0), one(1)]);
        assert_eq!(pt(e.end), [one(1), one(0), one(1)]);
    }

    fn ledge_brep() -> Brep<lattice::Bignum> {
        let joint = one_joint();
        let d24 = ledge_d24();
        let t = treatment(&d24);
        let valid = match closure_valid(&joint, &t) {
            Verdict::Verified(v) => v,
            other => panic!(
                "the ledge fold is CLOSURE_VALID: {}",
                matches!(other, Verdict::Verified(_))
            ),
        };
        brep_from_closure(&joint, &t, &valid)
    }

    /// The LEDGE exact §10 body is the **same two certified flank sheets** as MITER — no
    /// cap face. The only available LEDGE cap outline is the CAP-IN-D24 licensing square (a
    /// placeholder, not the real projected cut), whose crease edge overlaps `M`, so no
    /// certificate backs a flank↔cap seam; the exact cap is deferred to the `V_∂` real-cut
    /// slice rather than fabricated. The cap still appears (as two triangles) in the `§11`
    /// mesh path, [`crate::shell`].
    #[test]
    fn ledge_exact_body_is_the_certified_flanks_no_cap() {
        let b = ledge_brep();
        assert_eq!(
            b.faces().len(),
            2,
            "two flank sheets, no exact cap face (deferred to the V_∂ real-cut slice)"
        );
        assert!(b.indices_in_range());
        for f in 0..b.faces().len() {
            assert!(b.wire_is_closed(f), "flank wire {f} closes");
        }
        assert!(
            b.faces()
                .iter()
                .all(|f| matches!(f.surface, FaceSurface::LinearExtrusion { .. })),
            "both faces are ruled flank sheets"
        );
        // Structurally identical to the MITER body: one shared crease edge, manifold.
        let inc = b.edge_incidence();
        assert_eq!(
            inc.iter().filter(|&&c| c == 2).count(),
            1,
            "exactly one shared (2-incidence) edge — the crease middle M"
        );
        assert_eq!(b.nonmanifold_edges(), 0, "no non-manifold edge");
        assert_eq!(
            b.faces().len(),
            miter_brep().faces().len(),
            "same body as MITER"
        );
    }

    /// The single-flank slab is a **certified closed 2-manifold**: a topological box (8
    /// vertices, 12 edges, 6 faces) with every edge shared by exactly two faces, and its
    /// combinatorics pass the trusted `certify_core::shell::closed_shell` checker. This is
    /// the first certified closed solid — proven here without OCCT (the differential oracle
    /// corroborates it separately, `crate::differential`).
    #[test]
    fn the_flank_slab_is_a_certified_closed_2_manifold() {
        use certify_core::shell::{ClosedShell, closed_shell_holed};

        let joint = one_joint();
        let d24 = ledge_d24();
        let t = treatment(&d24);
        let slab = brep_slab_from_closure(&joint, &t);

        assert_eq!(slab.verts().len(), 8, "8 box corners");
        assert_eq!(slab.edges().len(), 12, "12 box edges");
        assert_eq!(slab.faces().len(), 6, "6 box faces");
        assert!(slab.indices_in_range());
        for f in 0..slab.faces().len() {
            assert!(slab.wire_is_closed(f), "slab face {f} wire closes");
        }
        assert_eq!(slab.free_edges(), 0, "a closed slab has no free edge");
        assert_eq!(slab.nonmanifold_edges(), 0, "no non-manifold edge");

        // Two of the six faces are the curved μ-walls (exact rational patches).
        assert_eq!(
            slab.faces()
                .iter()
                .filter(|f| matches!(f.surface, FaceSurface::RationalPatch(_)))
                .count(),
            2,
            "the two μ = const walls are exact rational patches"
        );

        // The trusted checker certifies the combinatorics as a closed oriented 2-manifold.
        let cert = slab.to_shell_certificate();
        assert_eq!(
            closed_shell_holed(
                cert.n_verts,
                &cert.edge_start,
                &cert.edge_end,
                &cert.wire_edge,
                &cert.wire_reversed,
                &cert.loop_start,
                &cert.face_start,
            ),
            Verdict::Verified(ClosedShell {
                verts: 8,
                edges: 12,
                faces: 6,
                loops: 6,
            }),
        );
    }

    /// The **free-boundary** single-flank slab (D4.3) is a certified closed 2-manifold over an
    /// *authored* σ-band — not the rectangular support box. Over flank A with the tapered band
    /// `μ⁻(σ) = −1 + σ`, `μ⁺(σ) = 1 − σ`: (1) the authored boundary is itself certified valid by
    /// the trusted `free_boundary` checker (D4.3a — positive width, regular rails, monotone),
    /// via the `free_boundary_cert` searcher; (2) the emitted solid is the same 8/12/6 box
    /// topology as the slab, but with **four** exact rational-patch sides (the boundary curves in
    /// σ), and its combinatorics pass `closed_shell`. Earned internally; OCCT corroborates in
    /// `crate::differential`.
    #[test]
    fn the_free_boundary_slab_is_a_certified_closed_2_manifold() {
        use certify_core::free_boundary::free_boundary;
        use certify_core::shell::{ClosedShell, closed_shell_holed};
        use lattice::Poly;

        let poly = |cs: &[i128]| {
            Poly::<lattice::Bignum>::from_coeffs(cs.iter().map(|&c| Rat::from_i128(c)).collect())
        };
        let joint = one_joint();
        let d24 = ledge_d24();
        let t = treatment(&d24);
        // Authored tapered band: μ⁻ = −1 + σ, μ⁺ = 1 − σ (width 2 − 2σ, genuinely varying in σ).
        let mu_lo = RatFunc::from_poly(poly(&[-1, 1]));
        let mu_hi = RatFunc::from_poly(poly(&[1, -1]));

        // (1) The authored free boundary is certified valid (the exact-ANCHOR obligation set).
        let cert = free_boundary_cert(
            joint.flank_a().chart(),
            &mu_lo,
            &mu_hi,
            &t.sigma_a,
            &RatFunc::one(), // σ-graph: σ̂ = σ ⇒ σ̂′ = 1
            &FreeBoundaryMargins {
                width: Rat::from_i128(1), // true width ∈ [2, 9/4] on [−1/8, 0]
                reg: Rat::new(1, 100),    // true |â′|² ≈ 8
                mono: Rat::new(1, 2),     // σ̂′ ≡ 1
            },
        );
        assert!(
            matches!(free_boundary(&cert), Verdict::Verified(_)),
            "the authored μ-band is a valid free boundary (positive width, regular rails, monotone)"
        );

        // (2) The emitted solid is a certified closed box over that boundary.
        let solid = brep_freeboundary_from_closure(&joint, &t, &mu_lo, &mu_hi);
        assert_eq!(solid.verts().len(), 8, "8 box corners");
        assert_eq!(solid.edges().len(), 12, "12 box edges");
        assert_eq!(solid.faces().len(), 6, "6 box faces");
        assert!(solid.indices_in_range());
        for f in 0..solid.faces().len() {
            assert!(
                solid.wire_is_closed(f),
                "free-boundary face {f} wire closes"
            );
        }
        assert_eq!(solid.free_edges(), 0, "a closed slab has no free edge");
        assert_eq!(solid.nonmanifold_edges(), 0, "no non-manifold edge");

        // All four side faces are exact rational patches (the curved-in-σ boundary makes even
        // the w = const sheets rational, vs the slab's two straight `LinearExtrusion` sheets).
        assert_eq!(
            solid
                .faces()
                .iter()
                .filter(|f| matches!(f.surface, FaceSurface::RationalPatch(_)))
                .count(),
            4,
            "all four side faces are exact rational patches"
        );

        // The trusted checker certifies the combinatorics as a closed oriented 2-manifold.
        let sc = solid.to_shell_certificate();
        assert_eq!(
            closed_shell_holed(
                sc.n_verts,
                &sc.edge_start,
                &sc.edge_end,
                &sc.wire_edge,
                &sc.wire_reversed,
                &sc.loop_start,
                &sc.face_start,
            ),
            Verdict::Verified(ClosedShell {
                verts: 8,
                edges: 12,
                faces: 6,
                loops: 6,
            }),
        );
    }

    /// The device **cone** as a certified closed solid — the free-boundary machinery generalizes
    /// from the cylinder to a converging-ruling cone (higher-degree rational patches). Over the
    /// exact 42° device cone (`fixtures::devices::cone()`) with an authored slanted boundary
    /// `μ⁻ = 1`, `μ⁺ = 2 + σ`: the authored boundary certifies (D4.3a) and `closed_shell`
    /// certifies the 8/12/6 solid closed. OCCT corroborates the geometry in `crate::differential`.
    #[test]
    fn the_cone_frustum_band_is_a_certified_closed_2_manifold() {
        use certify_core::free_boundary::free_boundary;
        use certify_core::shell::{ClosedShell, closed_shell_holed};
        use fixtures::devices::cone;
        use lattice::Poly;

        let poly = |cs: &[i128]| {
            Poly::<lattice::Bignum>::from_coeffs(cs.iter().map(|&c| Rat::from_i128(c)).collect())
        };
        let chart = cone();
        let sigma = Interval {
            lo: Rat::from_i128(0),
            hi: Rat::from_i128(1),
        };
        let w = Interval {
            lo: Rat::from_i128(0),
            hi: Rat::new(1, 4),
        };
        let mu_lo = RatFunc::from_poly(poly(&[1]));
        let mu_hi = RatFunc::from_poly(poly(&[2, 1]));

        // The authored boundary on the cone is certified valid (exact-ANCHOR obligation set).
        let fbc = free_boundary_cert(
            &chart,
            &mu_lo,
            &mu_hi,
            &sigma,
            &RatFunc::one(),
            &FreeBoundaryMargins {
                width: Rat::new(1, 2),
                reg: Rat::new(1, 10),
                mono: Rat::new(1, 2),
            },
        );
        assert!(
            matches!(free_boundary(&fbc), Verdict::Verified(_)),
            "the cone gore's authored boundary certifies"
        );

        let solid = brep_freeboundary(&chart, &sigma, &w, &mu_lo, &mu_hi);
        assert_eq!(solid.verts().len(), 8);
        assert_eq!(solid.faces().len(), 6);
        assert_eq!(solid.free_edges(), 0, "a closed cone band has no free edge");
        assert_eq!(solid.nonmanifold_edges(), 0);
        assert_eq!(
            solid
                .faces()
                .iter()
                .filter(|f| matches!(f.surface, FaceSurface::RationalPatch(_)))
                .count(),
            4,
            "all four cone-band side faces are exact rational patches"
        );
        let sc = solid.to_shell_certificate();
        assert_eq!(
            closed_shell_holed(
                sc.n_verts,
                &sc.edge_start,
                &sc.edge_end,
                &sc.wire_edge,
                &sc.wire_reversed,
                &sc.loop_start,
                &sc.face_start,
            ),
            Verdict::Verified(ClosedShell {
                verts: 8,
                edges: 12,
                faces: 6,
                loops: 6,
            }),
        );
    }

    /// `brep_trim_solid` on a plain single-piece curved band (μ⁻=1, μ⁺=2+σ over σ∈[0,1]) builds the
    /// same certified closed 8/12/6 solid as `brep_freeboundary` — the generalized builder reduces to
    /// the slab when both boundaries are single pieces.
    #[test]
    fn trim_solid_reduces_to_the_band() {
        use certify_core::shell::{ClosedShell, closed_shell_holed};
        use fixtures::devices::cone;
        use lattice::Poly;
        let poly = |cs: &[i128]| {
            Poly::<lattice::Bignum>::from_coeffs(cs.iter().map(|&c| Rat::from_i128(c)).collect())
        };
        let chart = cone();
        let sigma = Interval {
            lo: Rat::from_i128(0),
            hi: Rat::from_i128(1),
        };
        let w = Interval {
            lo: Rat::from_i128(0),
            hi: Rat::new(1, 4),
        };
        let mu_lo = RatFunc::from_poly(poly(&[1])); // μ⁻ = 1
        let mu_hi = RatFunc::from_poly(poly(&[2, 1])); // μ⁺ = 2 + σ
        let solid = brep_trim_solid(
            &chart,
            &w,
            &[(sigma.clone(), mu_lo)],
            &[(sigma, mu_hi)],
            &[],
        )
        .unwrap();
        assert_eq!(solid.verts().len(), 8);
        assert_eq!(solid.faces().len(), 6);
        assert_eq!(solid.free_edges(), 0);
        assert_eq!(solid.nonmanifold_edges(), 0);
        let sc = solid.to_shell_certificate();
        assert_eq!(
            closed_shell_holed(
                sc.n_verts,
                &sc.edge_start,
                &sc.edge_end,
                &sc.wire_edge,
                &sc.wire_reversed,
                &sc.loop_start,
                &sc.face_start,
            ),
            Verdict::Verified(ClosedShell {
                verts: 8,
                edges: 12,
                faces: 6,
                loops: 6,
            }),
        );
    }

    /// A low-degree `brep_trim_solid` band round-trips through OCCT (isolates the builder's OCCT
    /// path from the high-degree fitted trim rails).
    #[cfg(feature = "step")]
    #[test]
    fn trim_solid_band_exports_via_occt() {
        use crate::step::write_brep;
        use fixtures::devices::cone;
        use lattice::Poly;
        let poly = |cs: &[i128]| {
            Poly::<lattice::Bignum>::from_coeffs(cs.iter().map(|&c| Rat::from_i128(c)).collect())
        };
        let chart = cone();
        let sigma = Interval {
            lo: Rat::from_i128(0),
            hi: Rat::from_i128(1),
        };
        let w = Interval {
            lo: Rat::from_i128(0),
            hi: Rat::new(1, 4),
        };
        let solid = brep_trim_solid(
            &chart,
            &w,
            &[(sigma.clone(), RatFunc::from_poly(poly(&[1])))],
            &[(sigma, RatFunc::from_poly(poly(&[2, 1])))],
            &[],
        )
        .unwrap();
        let path = format!("{}/trim_band.step", std::env::temp_dir().display());
        assert_eq!(
            write_brep(&path, &solid),
            "ok",
            "low-degree trim band → OCCT"
        );
    }

    /// **S3.4b — the BONDED lap seam: two certified solids + a certified bond interface.** The
    /// device seam is emitted as two independent closed solids — the cone body (base, γ = 0) and
    /// the γ≠0 lap flap (the ramped chart, `brep_trim_solid` is `chart.pedal()`-aware) — each
    /// `closed_shell_holed`-certified and OCCT-corroborated (`brepcheck_valid`, `free_edges == 0`),
    /// because a lap is *doubled material* (§6.2), not one self-touching solid. Their bond is
    /// certified by the §14 conjunction `develop::bonded::valid_bonded_seam` (SEP ∧ SLAB ∧ SHEAR ∧
    /// CLEAR): the plateau separation ≡ the gap, the offset slab is regular, the Tier-1
    /// identification collapses (`δ = 18/65 ≈ 0.28 mm`), and the ramp rails keep clear.
    #[cfg(feature = "step")]
    #[test]
    fn bonded_lap_seam_two_certified_solids_plus_a_bond() {
        use crate::step::{audit_brep, write_brep};
        use certify_core::Verdict;
        use certify_core::shell::closed_shell_holed;
        use develop::bonded::{LapRail, clear, sep, shear, slab, valid_bonded_seam};
        use fixtures::devices::{cone_seam, cone_seam_ramp};
        use lattice::{Bignum, Poly};
        let k = |n: i128, d: i128| {
            RatFunc::<Bignum>::from_poly(Poly::from_coeffs(vec![Rat::new(n, d)]))
        };

        // The seam neighbourhood: σ' ∈ [−1/4, 1/4], band µ ∈ [−2, −1], thickness w ∈ [0, 1/8].
        let sig = Interval {
            lo: Rat::new(-1, 4),
            hi: Rat::new(1, 4),
        };
        let w = Interval {
            lo: Rat::from_i128(0),
            hi: Rat::new(1, 8),
        };
        let inner = [(sig.clone(), k(-2, 1))];
        let outer = [(sig.clone(), k(-1, 1))];

        // Two certified solids: the cone body (base, γ = 0) and the γ≠0 lap flap (ramp).
        let body = brep_trim_solid(&cone_seam(), &w, &inner, &outer, &[]).expect("body solid");
        let flap = brep_trim_solid(&cone_seam_ramp(), &w, &inner, &outer, &[]).expect("flap solid");
        for (name, solid) in [("body", &body), ("flap", &flap)] {
            let c = solid.to_shell_certificate();
            assert!(
                matches!(
                    closed_shell_holed(
                        c.n_verts,
                        &c.edge_start,
                        &c.edge_end,
                        &c.wire_edge,
                        &c.wire_reversed,
                        &c.loop_start,
                        &c.face_start,
                    ),
                    Verdict::Verified(_)
                ),
                "{name}: closed_shell_holed certifies a closed 2-manifold"
            );
            let audit = audit_brep(solid).expect("OCC audits the bonded solid");
            assert!(
                audit.brepcheck_valid,
                "{name}: OCC accepts the solid: {audit:?}"
            );
            assert_eq!(audit.free_edges, 0, "{name}: watertight: {audit:?}");
            assert_eq!(audit.nonmanifold_edges, 0, "{name}: manifold: {audit:?}");
        }

        // The bond between the two sheets is certified by the §14 conjunction.
        let w0 = Rat::from_i128(0);
        let neg1 = Rat::from_i128(-1);
        let bond = valid_bonded_seam(
            // SEP: the bonded plateau separation ≡ the gap Δ = g = 1/4 (base h = 0, plateau h = 1/4).
            sep(
                &RatFunc::<Bignum>::zero(),
                &w0,
                &k(1, 4),
                &w0,
                &Rat::new(1, 4),
            ),
            // SLAB: the offset slab stays regular over the seam box at the µ = −1 corner.
            slab(&cone_seam_ramp(), &neg1, &w0, &sig, &Rat::new(1, 1000)),
            // SHEAR: κ_g = −65/72 (−tan β), Δ₀ = 1/4 ⇒ δ = 18/65 ≈ 0.28 mm.
            shear(&k(-65, 72), &k(1, 4), &Rat::new(1, 100)),
            // CLEAR: the base rail and the ramp rail keep clear over the seam box.
            clear(
                &LapRail::from_chart(&cone_seam(), &neg1, &w0),
                &LapRail::from_chart(&cone_seam_ramp(), &neg1, &w0),
                &sig,
                &Rat::new(1, 8),
                2000,
            ),
        );
        assert!(
            matches!(bond, Verdict::Verified(_)),
            "the §14 BONDED conjunction certifies the seam"
        );

        // Emit the two STEP solids (the acceptance artifact).
        let dir = std::env::temp_dir();
        assert_eq!(
            write_brep(&format!("{}/bonded_body.step", dir.display()), &body),
            "ok"
        );
        assert_eq!(
            write_brep(&format!("{}/bonded_flap.step", dir.display()), &flap),
            "ok"
        );
    }

    /// The genus-1 trim solid (one interior hole drilled through a slice) round-trips through OCCT:
    /// the annular lids (`add_face_with_holes`) and the four-wall tube reload through `BRepCheck`
    /// with no free edges — the certified tunnel is a valid OCCT solid.
    #[cfg(feature = "step")]
    #[test]
    fn trim_solid_interior_hole_exports_via_occt() {
        use crate::step::write_brep;
        use fixtures::devices::cone;
        use lattice::Poly;
        let chart = cone();
        let iv = |lo, hi| Interval { lo, hi };
        let konst = |n: i128, d: i128| {
            RatFunc::<lattice::Bignum>::from_poly(Poly::from_coeffs(vec![Rat::new(n, d)]))
        };
        let w = iv(Rat::from_i128(0), Rat::new(1, 4));
        let inner = [(iv(Rat::from_i128(0), Rat::from_i128(1)), konst(1, 1))];
        let outer = [(
            iv(Rat::from_i128(0), Rat::from_i128(1)),
            RatFunc::from_poly(Poly::from_coeffs(vec![
                Rat::from_i128(2),
                Rat::from_i128(1),
            ])),
        )];
        let hole = HoleRail::uniform(konst(4, 3), konst(5, 3), Rat::new(1, 4), Rat::new(3, 4));
        let solid = brep_trim_solid(&chart, &w, &inner, &outer, &[hole]).unwrap();
        let path = format!("{}/trim_hole.step", std::env::temp_dir().display());
        assert_eq!(write_brep(&path, &solid), "ok", "genus-1 trim solid → OCCT");
    }

    /// The **station-crossing** genus-1 trim solid (a hole opening onto the `σ = 0` cross-ring as a
    /// curved notch in two slices) round-trips through OCCT: the notched lids + split rail/σ-cap walls
    /// reload through `BRepCheck` with no free edges.
    #[cfg(feature = "step")]
    #[test]
    fn trim_solid_station_crossing_hole_exports_via_occt() {
        use crate::step::write_brep;
        let (chart, sigma, w, mu_lo, mu_hi) = cone_gore();
        let inner = [(sigma.clone(), mu_lo)];
        let outer = [(sigma, mu_hi)];
        let hole = HoleRail::uniform(
            RatFunc::from_poly(lattice::Poly::from_coeffs(vec![Rat::new(-7, 4)])),
            RatFunc::from_poly(lattice::Poly::from_coeffs(vec![Rat::new(-5, 4)])),
            Rat::new(-1, 4),
            Rat::new(1, 4),
        );
        let solid = brep_trim_solid(&chart, &w, &inner, &outer, &[hole]).unwrap();
        let path = format!("{}/trim_notch.step", std::env::temp_dir().display());
        assert_eq!(
            write_brep(&path, &solid),
            "ok",
            "station-crossing notch trim solid → OCCT"
        );
    }

    /// The **span** genus-1 trim solid (a hole crossing ≥2 stations, so a middle slice's lid splits
    /// into two μ-bands) round-trips through OCCT — the band-split lids + split walls reload valid.
    #[cfg(feature = "step")]
    #[test]
    fn trim_solid_span_multi_station_hole_exports_via_occt() {
        use crate::step::write_brep;
        let (chart, sigma, w, mu_lo, mu_hi) = cone_gore();
        let inner = [(sigma.clone(), mu_lo)];
        let outer = [(sigma, mu_hi)];
        let hole = HoleRail::uniform(
            RatFunc::from_poly(lattice::Poly::from_coeffs(vec![Rat::new(-7, 4)])),
            RatFunc::from_poly(lattice::Poly::from_coeffs(vec![Rat::new(-5, 4)])),
            Rat::from_i128(-2),
            Rat::from_i128(2),
        );
        let solid = brep_trim_solid(&chart, &w, &inner, &outer, &[hole]).unwrap();
        let path = format!("{}/trim_span.step", std::env::temp_dir().display());
        assert_eq!(write_brep(&path, &solid), "ok", "span trim solid → OCCT");
    }

    /// A **piecewise outer boundary** (the D3-notch mechanism): μ⁺ = 2 on [0, 2/5] then σ+8/5 on
    /// [2/5, 1], meeting continuously at σ=2/5. The two outer rails share the kink corner (deduped),
    /// so the two-slice solid is watertight and `closed_shell` certifies it closed.
    #[test]
    fn trim_solid_piecewise_outer_is_closed() {
        use certify_core::shell::closed_shell_holed;
        use fixtures::devices::cone;
        use lattice::Poly;
        let chart = cone();
        let w = Interval {
            lo: Rat::from_i128(0),
            hi: Rat::new(1, 4),
        };
        let iv = |lo, hi| Interval { lo, hi };
        let konst = |cst: i128| {
            RatFunc::<lattice::Bignum>::from_poly(Poly::from_coeffs(vec![Rat::from_i128(cst)]))
        };
        let inner = [(iv(Rat::from_i128(0), Rat::from_i128(1)), konst(1))];
        // outer: 2 on [0, 2/5], then σ + 8/5 on [2/5, 1] (= 2 at σ = 2/5 — a continuous kink).
        let ramp = RatFunc::from_poly(Poly::from_coeffs(vec![Rat::new(8, 5), Rat::from_i128(1)]));
        let outer = [
            (iv(Rat::from_i128(0), Rat::new(2, 5)), konst(2)),
            (iv(Rat::new(2, 5), Rat::from_i128(1)), ramp),
        ];
        let solid = brep_trim_solid(&chart, &w, &inner, &outer, &[]).unwrap();
        assert_eq!(
            solid.free_edges(),
            0,
            "piecewise outer stays watertight (kink corner dedups)"
        );
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
            "piecewise-outer trim solid is a certified closed 2-manifold"
        );
    }

    /// `brep_trim_solid` with **one interior hole within a single slice** drills a through-tunnel:
    /// the slice's two lids become annular (`add_face_with_holes`) and a four-wall tube closes the
    /// bore, so the solid is a certified **genus-1** closed 2-manifold. The band is the single-slice
    /// `μ ∈ [1, 2+σ]` over `σ ∈ [0,1]`; the hole is `μ ∈ [4/3, 5/3]` over `σ ∈ [1/4, 3/4]` (strictly
    /// interior, and its tangent σ don't straddle an interior station).
    #[test]
    fn trim_solid_interior_hole_is_a_certified_genus_1_solid() {
        use certify_core::shell::closed_shell_holed;
        use fixtures::devices::cone;
        use lattice::Poly;
        let chart = cone();
        let iv = |lo, hi| Interval { lo, hi };
        let konst = |n: i128, d: i128| {
            RatFunc::<lattice::Bignum>::from_poly(Poly::from_coeffs(vec![Rat::new(n, d)]))
        };
        let w = iv(Rat::from_i128(0), Rat::new(1, 4));
        let inner = [(iv(Rat::from_i128(0), Rat::from_i128(1)), konst(1, 1))];
        let outer = [(
            iv(Rat::from_i128(0), Rat::from_i128(1)),
            RatFunc::from_poly(Poly::from_coeffs(vec![
                Rat::from_i128(2),
                Rat::from_i128(1),
            ])),
        )];
        let hole = HoleRail::uniform(konst(4, 3), konst(5, 3), Rat::new(1, 4), Rat::new(3, 4));
        let holed = brep_trim_solid(&chart, &w, &inner, &outer, &[hole]).unwrap();
        assert_eq!(holed.free_edges(), 0, "the drilled tube is watertight");
        assert_eq!(holed.nonmanifold_edges(), 0);
        assert_eq!(genus(&holed), 1, "one interior through-hole is genus 1");
        let sc = holed.to_shell_certificate();
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
            "the interior-hole trim solid is a certified closed genus-1 2-manifold"
        );
    }

    /// A hole whose σ-span **straddles an interior positive-weight station** is drilled as a curved
    /// **notch**: the wide gore forces `σ = 0` to be a station, and a hole over `σ ∈ [−1/4, 1/4]`
    /// crosses it, opening onto the `σ = 0` cross-ring in both adjacent slices (no inner loop there),
    /// yet the solid is a certified **genus-1** closed 2-manifold — the same genus the interior hole
    /// gives, however the stations split it (the `slice_footprint` notch case).
    #[test]
    fn trim_solid_station_crossing_hole_is_a_certified_genus_1_solid() {
        use certify_core::shell::closed_shell_holed;
        let (chart, sigma, w, mu_lo, mu_hi) = cone_gore();
        let inner = [(sigma.clone(), mu_lo)];
        let outer = [(sigma, mu_hi)];
        let hole = HoleRail::uniform(
            RatFunc::from_poly(lattice::Poly::from_coeffs(vec![Rat::new(-7, 4)])),
            RatFunc::from_poly(lattice::Poly::from_coeffs(vec![Rat::new(-5, 4)])),
            Rat::new(-1, 4),
            Rat::new(1, 4),
        );
        let holed = brep_trim_solid(&chart, &w, &inner, &outer, &[hole]).unwrap();
        assert_eq!(
            holed.free_edges(),
            0,
            "the station-crossing notch is watertight"
        );
        assert_eq!(holed.nonmanifold_edges(), 0);
        assert_eq!(
            genus(&holed),
            1,
            "a through-hole is genus 1, however the stations split it"
        );
        let sc = holed.to_shell_certificate();
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
            "the station-crossing trim solid is a certified closed genus-1 2-manifold"
        );
    }

    /// A hole **spanning multiple stations** (`σ ∈ [−2, 2]` on the wide gore, whose positive-weight
    /// partition has interior stations inside `(−2, 2)`) exercises the μ-band **split**: a fully
    /// covered middle slice's lid becomes a bottom band `[μ⁻, near]` and a top band `[far, μ⁺]` (no
    /// inner loop), while the end slices notch — still one certified **genus-1** through-hole.
    #[test]
    fn trim_solid_span_multi_station_hole_is_genus_1() {
        use certify_core::shell::closed_shell_holed;
        let (chart, sigma, w, mu_lo, mu_hi) = cone_gore();
        let inner = [(sigma.clone(), mu_lo)];
        let outer = [(sigma, mu_hi)];
        let hole = HoleRail::uniform(
            RatFunc::from_poly(lattice::Poly::from_coeffs(vec![Rat::new(-7, 4)])),
            RatFunc::from_poly(lattice::Poly::from_coeffs(vec![Rat::new(-5, 4)])),
            Rat::from_i128(-2),
            Rat::from_i128(2),
        );
        let holed = brep_trim_solid(&chart, &w, &inner, &outer, &[hole]).unwrap();
        assert_eq!(
            holed.free_edges(),
            0,
            "the multi-station span is watertight"
        );
        assert_eq!(holed.nonmanifold_edges(), 0);
        assert_eq!(
            genus(&holed),
            1,
            "one through-hole is genus 1 however many slices it spans"
        );
        let sc = holed.to_shell_certificate();
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
            "the span trim solid is a certified closed genus-1 2-manifold"
        );
    }

    /// `sigma_splits` subdivides by the **intrinsic positive-weight criterion** — never a
    /// parametrization-specific point. The `1 + σ²`-type denominator (the device cone's) has a
    /// single Bézier span's middle weight go non-positive over a wide σ=0-crossing span, so the wide
    /// `[−15/4, 15/4]` gore genuinely subdivides, and *every* resulting slice has positive weights.
    #[test]
    fn sigma_splits_subdivides_until_positive_weights() {
        use lattice::Poly;
        let den = Poly::<lattice::Bignum>::from_coeffs(vec![
            Rat::from_i128(1),
            Rat::from_i128(0),
            Rat::from_i128(1),
        ]); // 1 + σ²
        let (a, b) = (Rat::new(-15, 4), Rat::new(15, 4));
        // The undivided wide span fails (a non-positive middle weight); the partition fixes it.
        assert!(
            !super::positive_weights(&den, &a, &b),
            "the wide σ=0-crossing span has a non-positive Bézier weight"
        );
        let stations = super::sigma_splits(&den, &a, &b);
        assert!(stations.len() > 2, "the wide gore subdivides: {stations:?}");
        assert_eq!(stations.first(), Some(&a));
        assert_eq!(stations.last(), Some(&b));
        for w in stations.windows(2) {
            assert!(
                super::positive_weights(&den, &w[0], &w[1]),
                "every slice has positive weights"
            );
        }
    }

    /// The **two-sided** device-cone gore forces σ-subdivision (a single Bézier span would carry
    /// non-positive weights across the wide, σ=0-crossing span), and the resulting fused **N-slice**
    /// solid is still a certified closed 2-manifold — all single-span rational patches, no
    /// parametrization-specific split, no B-spline. Over σ ∈ [−15/4, 15/4], band μ ∈ [−2, −1].
    #[test]
    fn the_two_sided_cone_gore_subdivides_and_certifies() {
        use certify_core::shell::{ClosedShell, closed_shell_holed};
        use fixtures::devices::cone;
        use lattice::Poly;

        let muf = |n: i128| {
            RatFunc::from_poly(Poly::<lattice::Bignum>::from_coeffs(vec![Rat::from_i128(
                n,
            )]))
        };
        let chart = cone();
        let sigma = Interval {
            lo: Rat::new(-15, 4),
            hi: Rat::new(15, 4),
        };
        let w = Interval {
            lo: Rat::from_i128(0),
            hi: Rat::new(1, 8),
        };
        let (mu_lo, mu_hi) = (muf(-2), muf(-1));

        let solid = brep_freeboundary(&chart, &sigma, &w, &mu_lo, &mu_hi);
        let (nv, ne, nf) = (
            solid.verts().len(),
            solid.edges().len(),
            solid.faces().len(),
        );
        let big_n = (nf - 2) / 4; // N slices, faces = 4N + 2
        assert!(
            big_n >= 2,
            "the wide two-sided gore subdivides into N ≥ 2 slices: N = {big_n}"
        );
        assert_eq!(nv, 4 * (big_n + 1), "4(N+1) verts");
        assert_eq!(ne, 8 * big_n + 4, "8N+4 edges");
        assert_eq!(nf, 4 * big_n + 2, "4N+2 faces");
        assert_eq!(solid.free_edges(), 0, "a closed solid has no free edge");
        assert_eq!(solid.nonmanifold_edges(), 0);
        assert_eq!(
            solid
                .faces()
                .iter()
                .filter(|f| matches!(f.surface, FaceSurface::RationalPatch(_)))
                .count(),
            4 * big_n,
            "all 4N side faces are single-span rational patches (no B-spline)"
        );
        let sc = solid.to_shell_certificate();
        assert_eq!(
            closed_shell_holed(
                sc.n_verts,
                &sc.edge_start,
                &sc.edge_end,
                &sc.wire_edge,
                &sc.wire_reversed,
                &sc.loop_start,
                &sc.face_start,
            ),
            Verdict::Verified(ClosedShell {
                verts: nv,
                edges: ne,
                faces: nf,
                loops: nf,
            }),
            "the subdivided two-sided cone solid is a certified closed 2-manifold"
        );
    }

    /// A slab with one rectangular through-hole is a certified **genus-1** solid: the two `w=const`
    /// sheets gain the hole as an inner loop (annular faces), a tube closes it through the
    /// thickness, and `closed_shell_holed` certifies the watertight result — `loops = faces + 2`
    /// (one hole cutting both sheets), `free_edges == 0`.
    #[test]
    fn a_through_hole_slab_is_a_certified_genus_1_solid() {
        use certify_core::shell::{ClosedShell, closed_shell_holed};
        use fixtures::closure_joint::one_joint;
        use lattice::Poly;

        let muf = |n: i128| {
            RatFunc::from_poly(Poly::<lattice::Bignum>::from_coeffs(vec![Rat::from_i128(
                n,
            )]))
        };
        let chart = one_joint();
        let chart = chart.flank_a().chart();
        // A single positive-weight σ-slice (N = 1) over a constant-μ band, thickness away from w=0.
        let sigma = Interval {
            lo: Rat::new(-1, 8),
            hi: Rat::from_i128(0),
        };
        let w = Interval {
            lo: Rat::from_i128(1),
            hi: Rat::from_i128(2),
        };
        let (mu_lo, mu_hi) = (muf(-1), muf(1));

        let plain = brep_freeboundary(chart, &sigma, &w, &mu_lo, &mu_hi);
        // A hole strictly inside the slice in σ and inside [μ⁻, μ⁺] in μ.
        let hole = HoleRect {
            sigma: Interval {
                lo: Rat::new(-3, 32),
                hi: Rat::new(-1, 32),
            },
            mu: Interval {
                lo: Rat::new(-1, 4),
                hi: Rat::new(1, 4),
            },
        };
        let holed = brep_freeboundary_holed(chart, &sigma, &w, &mu_lo, &mu_hi, &[hole])
            .expect("the small interior hole fits one positive-weight slice");

        // The hole adds 8 rim vertices, 12 rim edges (4 bottom + 4 top + 4 vertical) and 4 tube
        // faces; the two sheets each gain an inner loop.
        assert_eq!(holed.verts().len(), plain.verts().len() + 8, "8 rim verts");
        assert_eq!(
            holed.edges().len(),
            plain.edges().len() + 12,
            "12 rim edges"
        );
        assert_eq!(holed.faces().len(), plain.faces().len() + 4, "4 tube walls");
        assert!(holed.indices_in_range());
        for f in 0..holed.faces().len() {
            assert!(holed.all_loops_closed(f), "holed face {f} loops all close");
        }
        assert_eq!(
            holed.free_edges(),
            0,
            "a closed through-hole solid is watertight"
        );
        assert_eq!(holed.nonmanifold_edges(), 0);
        // Two tube walls are ruled patches (the μ = const σ-rails), two are planar (σ = const).
        assert_eq!(
            holed
                .faces()
                .iter()
                .filter(|f| matches!(f.surface, FaceSurface::RationalPatch(_)))
                .count(),
            plain
                .faces()
                .iter()
                .filter(|f| matches!(f.surface, FaceSurface::RationalPatch(_)))
                .count()
                + 2,
            "the hole adds two ruled tube walls (its two σ-rail sides)"
        );

        let nf = holed.faces().len();
        let sc = holed.to_shell_certificate();
        assert_eq!(
            closed_shell_holed(
                sc.n_verts,
                &sc.edge_start,
                &sc.edge_end,
                &sc.wire_edge,
                &sc.wire_reversed,
                &sc.loop_start,
                &sc.face_start,
            ),
            Verdict::Verified(ClosedShell {
                verts: holed.verts().len(),
                edges: holed.edges().len(),
                faces: nf,
                loops: nf + 2, // one through-hole = an inner loop on each of the two sheets
            }),
            "the through-hole slab is a certified genus-1 closed 2-manifold"
        );
    }

    /// The genus of a certified closed orientable 2-manifold B-rep, by Euler: a face with `h` holes
    /// is a 2-cell-with-holes contributing `(1 − h)`, so `χ = V − E + (2F − L)` (L = total loops) and
    /// `g = (2 − χ)/2`. Representation-invariant — a through-hole reads as genus 1 whether it is an
    /// interior inner loop, a boundary **notch**, or a **μ-band split**.
    fn genus(b: &Brep<lattice::Bignum>) -> i64 {
        let v = b.verts().len() as i64;
        let e = b.edges().len() as i64;
        let f = b.faces().len() as i64;
        let l: i64 = b.faces().iter().map(|fc| 1 + fc.holes.len() as i64).sum();
        let chi = v - e + (2 * f - l);
        (2 - chi) / 2
    }

    /// The certified device-cone gore fixture: the symmetric two-sided gore `σ ∈ [−15/4, 15/4]`,
    /// constant μ-band `[−2, −1]`, thickness `[0, 1/8]` — whose positive-weight partition forces
    /// `σ = 0` to be a **station** (the parametrization artifact the summary describes).
    #[allow(clippy::type_complexity)]
    fn cone_gore() -> (
        geom::chart::Chart<lattice::Bignum>,
        Interval<lattice::Bignum>,
        Interval<lattice::Bignum>,
        RatFunc<lattice::Bignum>,
        RatFunc<lattice::Bignum>,
    ) {
        use fixtures::devices::cone;
        use lattice::Poly;
        let muf = |n: i128| {
            RatFunc::from_poly(Poly::<lattice::Bignum>::from_coeffs(vec![Rat::from_i128(
                n,
            )]))
        };
        (
            cone(),
            Interval {
                lo: Rat::new(-15, 4),
                hi: Rat::new(15, 4),
            },
            Interval {
                lo: Rat::from_i128(0),
                hi: Rat::new(1, 8),
            },
            muf(-2),
            muf(-1),
        )
    }

    /// The bug the user caught, now fixed: a through-hole **centred on σ = 0** — which the
    /// positive-weight partition forces to be a subdivision **station** — is cut cleanly and
    /// certifies genus 1. The single-slice builder could not place it there (it landed off-centre
    /// in a corner); the arrangement construction opens it as a **notch** into both adjacent slices,
    /// glued along the station, and `closed_shell_holed` certifies the watertight result. The hole
    /// contributes **no** inner loop here (it opens onto the lid boundary), yet the genus is still 1.
    #[test]
    fn a_through_hole_crossing_a_sigma_station_is_a_certified_genus_1_solid() {
        use certify_core::shell::closed_shell_holed;

        let (chart, sigma, w, mu_lo, mu_hi) = cone_gore();
        // Straddles σ = 0 (the station), strictly interior in σ and μ.
        let hole = HoleRect {
            sigma: Interval {
                lo: Rat::new(-1, 4),
                hi: Rat::new(1, 4),
            },
            mu: Interval {
                lo: Rat::new(-7, 4),
                hi: Rat::new(-5, 4),
            },
        };
        let holed = brep_freeboundary_holed(&chart, &sigma, &w, &mu_lo, &mu_hi, &[hole])
            .expect("a station-crossing hole builds via the arrangement");

        assert!(holed.indices_in_range());
        for f in 0..holed.faces().len() {
            assert!(holed.all_loops_closed(f), "holed face {f} loops all close");
        }
        assert_eq!(holed.free_edges(), 0, "a through-hole solid is watertight");
        assert_eq!(holed.nonmanifold_edges(), 0);
        assert_eq!(
            genus(&holed),
            1,
            "a through-hole is genus 1, however the stations split it"
        );

        let sc = holed.to_shell_certificate();
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
            "the station-crossing through-hole is a certified closed 2-manifold"
        );
    }

    /// A through-hole whose σ-range **spans an entire σ-slice** splits that slice's strip into two
    /// μ-bands (the arrangement returns two faces), and the fused solid still certifies genus 1. Over
    /// the asymmetric cone gore `σ ∈ [−15/4, 15/2]` (stations `−15/4, −15/16, 15/32, 15/8, 15/2`), a
    /// hole `σ ∈ [0, 5/2]` fully spans the middle slice `[15/32, 15/8]` while notching the two
    /// neighbours — three different per-slice cell shapes, one uniform arrangement.
    #[test]
    fn a_through_hole_spanning_a_slice_splits_into_mu_bands_and_certifies() {
        use certify_core::shell::closed_shell_holed;
        use fixtures::devices::cone;
        use lattice::Poly;

        let muf = |n: i128| {
            RatFunc::from_poly(Poly::<lattice::Bignum>::from_coeffs(vec![Rat::from_i128(
                n,
            )]))
        };
        let chart = cone();
        let sigma = Interval {
            lo: Rat::new(-15, 4),
            hi: Rat::new(15, 2),
        };
        let w = Interval {
            lo: Rat::from_i128(0),
            hi: Rat::new(1, 8),
        };
        let (mu_lo, mu_hi) = (muf(-2), muf(-1));
        let hole = HoleRect {
            sigma: Interval {
                lo: Rat::from_i128(0),
                hi: Rat::new(5, 2),
            },
            mu: Interval {
                lo: Rat::new(-7, 4),
                hi: Rat::new(-5, 4),
            },
        };
        let holed = brep_freeboundary_holed(&chart, &sigma, &w, &mu_lo, &mu_hi, &[hole])
            .expect("a slice-spanning hole builds via the arrangement");

        assert!(holed.indices_in_range());
        for f in 0..holed.faces().len() {
            assert!(holed.all_loops_closed(f), "holed face {f} loops all close");
        }
        assert_eq!(holed.free_edges(), 0, "a through-hole solid is watertight");
        assert_eq!(holed.nonmanifold_edges(), 0);
        assert_eq!(
            genus(&holed),
            1,
            "the slice-spanning through-hole is genus 1"
        );

        let sc = holed.to_shell_certificate();
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
            "the slice-spanning through-hole is a certified closed 2-manifold"
        );
    }

    /// Two holes in one panel — one straddling the σ = 0 station (a notch), one strictly inside a
    /// slice (an annular inner loop) — compose to a certified **genus-2** solid, each hole raising
    /// the genus independently. The mixed representation (a notch hole with no inner loop + an
    /// annular hole with two) exercises both cell shapes in one build.
    #[test]
    fn two_holes_one_crossing_one_interior_compose_to_genus_2() {
        use certify_core::shell::closed_shell_holed;

        let (chart, sigma, w, mu_lo, mu_hi) = cone_gore();
        let crossing = HoleRect {
            sigma: Interval {
                lo: Rat::new(-1, 4),
                hi: Rat::new(1, 4),
            },
            mu: Interval {
                lo: Rat::new(-7, 4),
                hi: Rat::new(-13, 8),
            },
        };
        // Strictly inside the right slice [0, 15/4]; disjoint from the crossing hole in σ.
        let interior = HoleRect {
            sigma: Interval {
                lo: Rat::from_i128(1),
                hi: Rat::from_i128(2),
            },
            mu: Interval {
                lo: Rat::new(-3, 2),
                hi: Rat::new(-5, 4),
            },
        };
        let holed =
            brep_freeboundary_holed(&chart, &sigma, &w, &mu_lo, &mu_hi, &[crossing, interior])
                .expect("two holes build");

        assert!(holed.indices_in_range());
        assert_eq!(holed.free_edges(), 0, "watertight");
        assert_eq!(holed.nonmanifold_edges(), 0);
        assert_eq!(
            genus(&holed),
            2,
            "two independent through-holes are genus 2"
        );

        let sc = holed.to_shell_certificate();
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
            "the two-hole panel is a certified closed 2-manifold"
        );
    }

    /// The builder **refuses** (returns `None`) rather than silently mis-building a hole that is not
    /// **strictly interior** to the panel — here one touching the σ-support boundary. A through-hole
    /// must raise the genus (a closed void), not open a boundary slot; the honest refusal keeps the
    /// arrangement construction from fabricating a non-manifold cut.
    #[test]
    fn a_hole_touching_the_panel_boundary_is_refused() {
        use fixtures::closure_joint::one_joint;
        use lattice::Poly;

        let muf = |n: i128| {
            RatFunc::from_poly(Poly::<lattice::Bignum>::from_coeffs(vec![Rat::from_i128(
                n,
            )]))
        };
        let chart = one_joint();
        let chart = chart.flank_a().chart();
        let sigma = Interval {
            lo: Rat::new(-1, 8),
            hi: Rat::from_i128(0),
        };
        let w = Interval {
            lo: Rat::from_i128(1),
            hi: Rat::from_i128(2),
        };
        let (mu_lo, mu_hi) = (muf(-1), muf(1));
        // σ.lo == support.lo: the hole touches the panel σ-boundary — not strictly interior.
        let hole = HoleRect {
            sigma: Interval {
                lo: Rat::new(-1, 8),
                hi: Rat::new(-1, 16),
            },
            mu: Interval {
                lo: Rat::new(-1, 4),
                hi: Rat::new(1, 4),
            },
        };
        assert!(
            brep_freeboundary_holed(chart, &sigma, &w, &mu_lo, &mu_hi, &[hole]).is_none(),
            "a hole touching the panel boundary is refused, not silently mis-built"
        );
    }
}
