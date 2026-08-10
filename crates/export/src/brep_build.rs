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

use crate::bezier::{RatBezier, RatBezierSurface};
use crate::brep::{Brep, EdgeGeom, FaceSurface, HalfEdge};
use certify_core::MarginSq;
use certify_core::certify1d::{EdgeRegCert, RegCert};
use certify_core::free_boundary::FreeBoundaryCert;
use closure::valid::{CapWitness, ClosureTreatment, ClosureValid};
use closure::{Joint, MuRange};
use geom::chart::Chart;
use lattice::{Backend, Interval, Rat, RatFunc, SturmChain, Surd, Vec3Rat};

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

impl<B: Backend> Builder<B> {
    fn new() -> Self {
        Builder {
            brep: Brep::new(),
            verts: Vec::new(),
        }
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
            FaceSurface::RationalPatch(RatBezierSurface::ruled_from_rails(
                &surf(j, 0),
                &surf(j, 1),
                &sigmas[0],
                &sigmas[1],
            ))
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
    let mut bld = Builder::new();
    let supp = sigma;
    let sigmas = [supp.lo.clone(), supp.hi.clone()];
    let ws = [w.lo.clone(), w.hi.clone()];

    // The chart fields, reduced once (like the slab's `base`/`dir`): `c + μ±(σ)·r` for the two
    // authored boundary splines — `Vec3Rat::scale` by the `RatFunc` μ±, the generalization of
    // the slab's scalar `scale_rat`. Reducing pedal/ruling/normal first keeps the true low
    // degree AND collapses both μ-bases to one shared denominator, so — since `n·w` is added
    // via `scale_rat` (denominator-preserving) — all four σ-rails share `base.den · n.den`,
    // exactly the shared-weights condition `ruled_from_rails` needs.
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

    // 8 corner vertices: a[m] at σ = σlo, b[m] at σ = σhi, over ring corner m (deduped).
    let mut a = [0usize; 4];
    let mut b = [0usize; 4];
    for (m, &(j, k)) in ring.iter().enumerate() {
        let s = surf(j, k);
        a[m] = bld.vertex(
            &s.eval(&sigmas[0])
                .expect("free-boundary corner (σlo) finite"),
        );
        b[m] = bld.vertex(
            &s.eval(&sigmas[1])
                .expect("free-boundary corner (σhi) finite"),
        );
    }

    // 12 edges: the two straight cross-section rings (σlo, σhi) and the four curved σ-rails.
    let mut ring_a = [0usize; 4];
    let mut ring_b = [0usize; 4];
    let mut rails = [0usize; 4];
    for m in 0..4 {
        let mp = (m + 1) % 4;
        ring_a[m] = bld.brep.add_edge(a[m], a[mp], EdgeGeom::Line);
        ring_b[m] = bld.brep.add_edge(b[m], b[mp], EdgeGeom::Line);
        let (j, k) = ring[m];
        rails[m] = bld.add_rail(&surf(j, k), supp);
    }

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

    // The four side faces: A_m → A_{m+1} → B_{m+1} → B_m, sharing rings and rails by identity.
    // Every side is now an exact rational patch ruled between its two adjacent σ-rails — the
    // authored boundary curves in σ, so even the w = const sheets are no longer straight
    // extrusions (the slab's cylinder-only case). Ring corners m and m+1 supply the rails.
    for m in 0..4 {
        let mp = (m + 1) % 4;
        let wire = vec![
            bld.directed(ring_a[m], a[m], a[mp]),
            bld.directed(rails[mp], a[mp], b[mp]),
            bld.directed(ring_b[m], b[mp], b[m]),
            bld.directed(rails[m], b[m], a[m]),
        ];
        let (jm, km) = ring[m];
        let (jn, kn) = ring[mp];
        let surface = FaceSurface::RationalPatch(RatBezierSurface::ruled_from_rails(
            &surf(jm, km),
            &surf(jn, kn),
            &sigmas[0],
            &sigmas[1],
        ));
        bld.brep.add_face(surface, wire);
    }

    bld.into_brep()
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
        use certify_core::shell::{ClosedShell, closed_shell};

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
            closed_shell(
                cert.n_verts,
                &cert.edge_start,
                &cert.edge_end,
                &cert.wire_edge,
                &cert.wire_reversed,
                &cert.face_start,
            ),
            Verdict::Verified(ClosedShell {
                verts: 8,
                edges: 12,
                faces: 6
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
        use certify_core::shell::{ClosedShell, closed_shell};
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
            closed_shell(
                sc.n_verts,
                &sc.edge_start,
                &sc.edge_end,
                &sc.wire_edge,
                &sc.wire_reversed,
                &sc.face_start,
            ),
            Verdict::Verified(ClosedShell {
                verts: 8,
                edges: 12,
                faces: 6
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
        use certify_core::shell::{ClosedShell, closed_shell};
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
            closed_shell(
                sc.n_verts,
                &sc.edge_start,
                &sc.edge_end,
                &sc.wire_edge,
                &sc.wire_reversed,
                &sc.face_start,
            ),
            Verdict::Verified(ClosedShell {
                verts: 8,
                edges: 12,
                faces: 6
            }),
        );
    }
}
