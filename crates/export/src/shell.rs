//! The neutral **shell record** — the exact triangulated boundary of a certified
//! one-joint closure, ready to hand to a CAD writer.
//!
//! [`shell_from_closure`] reconstructs the joint's watertight shell from a
//! [`ClosureValid`](closure::valid::ClosureValid) witness and the geometry it was
//! certified against: the two **flank faces** (each chart's ruled strip, sampled on
//! a σ-grid across its retained support at the certified normal offset) and the
//! **cap face** (a Ledge cap's [`CapOut`] boundary loops, lifted from the cap plane
//! back into 3-space through a joint-derived frame). The record is a plain list of
//! [`Tri`]s with exact `a + b√d` vertices — no float appears here; the exact→`f64`
//! cast happens once, later, in the [`step`](crate::step) writer.
//!
//! This module is **always compiled** and float-free — it is the exact geometry
//! layer between the certified witness and the (feature-gated) OCCT bridge. Nothing
//! keys on the flank *type*: the same chart-field walk serves a cylinder or a cone
//! (the M4 slice is cylinder-first; see `docs/vv-guide.md §8`).
//!
//! # A degenerate offset is avoided by construction
//!
//! The cylinder fold's charts have a pedal that collapses onto the fold axis at `w = 0`
//! (the ruled patch degenerates to a line). The flanks are therefore sampled at
//! `w = t.w.lo` — the **low end of the certified normal-offset box** `[w⁻, w⁺]` — where
//! the patch is a genuine 2D face, faithful to the same interval the closure was
//! certified over.
//!
//! # The fixture is a physical fold (M-D slice 1)
//!
//! The `fixtures::closure_joint` builders render a device-recognizable **90° cylinder
//! self-fold** — the three Milestone-C fixture warts are discharged, so the assembled
//! shell both certifies and reads as a folded panel:
//! - **flank shape (D.1):** each flank is a true `h ≠ 0` cylinder (pedal `c = h·n ≠ 0`),
//!   so its rulings stay parallel to the fold axis — not the `h ≡ 0` cone whose rulings
//!   converge on an apex;
//! - **shared crease (D.1):** the two flanks are *distinct* charts (B is A rigidly
//!   translated ⊥ the rulings) whose crease neutral edges coincide on one line — the
//!   strips abut with no gap, a real crease;
//! - **metric cap (D.2):** the Ledge cap is lifted through the **orthonormal** crease
//!   frame `{r₀/√s, n₀}` (see [`lift`]) — a unit cap square lifts to a unit world
//!   square, no stretch. Its 2D outline is still the CAP-IN-D24 licensing square (a real
//!   projected cut awaits the `V_∂`-guided seam, a later M-D slice).
//!
//! # Example
//!
//! ```
//! use certify_core::Verdict;
//! use closure::valid::closure_valid;
//! use export::shell::shell_from_closure;
//! use fixtures::closure_joint::{ledge_d24, one_joint, treatment};
//!
//! let joint = one_joint();
//! let d24 = ledge_d24();
//! let t = treatment(&d24);
//! let valid = match closure_valid(&joint, &t) {
//!     Verdict::Verified(v) => v,
//!     other => panic!("the 90° fold is CLOSURE_VALID: {}", matches!(other, Verdict::Verified(_))),
//! };
//! let shell = shell_from_closure(&joint, &t, &valid);
//! // Two flank strips (a σ-grid of quads → 2 triangles each) plus a cap fan.
//! assert!(!shell.is_empty());
//! ```

use arrange2d::boolean::CapOut;
use closure::cap_in::PiFrame;
use closure::valid::{CapWitness, ClosureTreatment, ClosureValid};
use closure::{Joint, MuRange};
use geom::chart::Chart;
use geom::content::{Edge, Point2};
use lattice::{Backend, Bignum, Interval, Rat, Surd};

/// The number of σ-subdivisions per flank strip — `N` quads (`2N` triangles) across
/// each flank's retained support. Small (the ruled patch is exactly captured by the
/// two μ-samples; σ carries the only curvature), but enough that a viewer reads the
/// developable's bend.
const FLANK_STEPS: usize = 8;

/// One shell vertex: an exact `a + b√d` point in 3-space. Rational vertices (every
/// vertex the M4 slice mints) are the `b = 0` degenerate case, kept cheap by [`Surd`].
pub type Vertex<B> = [Surd<B>; 3];

/// One triangle of the shell boundary — three exact vertices, in boundary-consistent
/// winding within a face (the writer re-derives orientation from the sewn shell).
pub struct Tri<B: Backend = Bignum> {
    /// The triangle's three corners.
    pub v: [Vertex<B>; 3],
}

impl<B: Backend> Clone for Tri<B> {
    fn clone(&self) -> Self {
        Tri {
            v: [self.v[0].clone(), self.v[1].clone(), self.v[2].clone()],
        }
    }
}

/// The exact triangulated boundary of a certified one-joint shell: a flat list of
/// [`Tri`]s (the two flank faces + the cap face). Assemble with [`shell_from_closure`];
/// read the triangles back through [`tris`](ShellRecord::tris).
pub struct ShellRecord<B: Backend = Bignum> {
    tris: Vec<Tri<B>>,
}

impl<B: Backend> ShellRecord<B> {
    /// The shell's triangles, in assembly order (flank A, flank B, then the cap).
    pub fn tris(&self) -> &[Tri<B>] {
        &self.tris
    }
    /// The number of triangles in the shell.
    pub fn len(&self) -> usize {
        self.tris.len()
    }
    /// Whether the shell carries no triangles.
    pub fn is_empty(&self) -> bool {
        self.tris.is_empty()
    }
}

/// One flank σ-station's ruled edge: the two ruling-endpoint points `(surface(μ⁻),
/// surface(μ⁺))` at that σ, in exact rational coordinates.
type FlankEdge<B> = ([Rat<B>; 3], [Rat<B>; 3]);

/// A rational vertex lifted into [`Surd`] components (the `b = 0` case).
fn vert_from_rat<B: Backend>(p: &[Rat<B>; 3]) -> Vertex<B> {
    [
        Surd::from_rat(p[0].clone()),
        Surd::from_rat(p[1].clone()),
        Surd::from_rat(p[2].clone()),
    ]
}

/// `n + 1` evenly spaced σ-samples across `[iv.lo, iv.hi]` — `iv.lo + (i/n)·(iv.hi − iv.lo)`.
fn sigma_samples<B: Backend>(iv: &Interval<B>, n: usize) -> Vec<Rat<B>> {
    let span = iv.hi.sub(&iv.lo);
    (0..=n)
        .map(|i| iv.lo.add(&span.mul(&Rat::new(i as i128, n as i128))))
        .collect()
}

/// Triangulate one flank's ruled strip: the chart's `surface(μ, w)` sampled at the
/// two μ-endpoints across a σ-grid over the retained support `sigma`, at the certified
/// offset `w`. Each σ-cell is a quad (`prev_lo → cur_lo → cur_hi → prev_hi`) split into
/// two triangles. A σ-station where the surface is singular breaks the strip there
/// (its cell is dropped) rather than fabricating a point.
fn flank_tris<B: Backend>(
    chart: &Chart<B>,
    sigma: &Interval<B>,
    mu: &MuRange<B>,
    w: &Rat<B>,
    n: usize,
) -> Vec<Tri<B>> {
    let surf_lo = chart.surface(&mu.lo, w);
    let surf_hi = chart.surface(&mu.hi, w);
    let mut tris = Vec::new();
    let mut prev: Option<FlankEdge<B>> = None;
    for s in sigma_samples(sigma, n) {
        match (surf_lo.eval(&s), surf_hi.eval(&s)) {
            (Some(cur_lo), Some(cur_hi)) => {
                if let Some((prev_lo, prev_hi)) = &prev {
                    tris.push(Tri {
                        v: [
                            vert_from_rat(prev_lo),
                            vert_from_rat(&cur_lo),
                            vert_from_rat(&cur_hi),
                        ],
                    });
                    tris.push(Tri {
                        v: [
                            vert_from_rat(prev_lo),
                            vert_from_rat(&cur_hi),
                            vert_from_rat(prev_hi),
                        ],
                    });
                }
                prev = Some((cur_lo, cur_hi));
            }
            _ => prev = None,
        }
    }
    tris
}

/// The starting point of a cap-boundary edge (segment or arc).
fn edge_start<B: Backend>(e: &Edge<B>) -> &Point2<B> {
    match e {
        Edge::Seg(s) => &s.start,
        Edge::Arc(a) => &a.start,
    }
}

/// The ending point of a cap-boundary edge (segment or arc).
fn edge_end<B: Backend>(e: &Edge<B>) -> &Point2<B> {
    match e {
        Edge::Seg(s) => &s.end,
        Edge::Arc(a) => &a.end,
    }
}

/// Exact equality of two cap-plane points (`Surd` coordinates compare exactly).
fn same_point<B: Backend>(a: &Point2<B>, b: &Point2<B>) -> bool {
    a.x == b.x && a.y == b.y
}

/// Chain a face's boundary `edges` into a **consistently-ordered** vertex ring.
///
/// The arrangement stores each face's `outer` edges as an *unordered, arbitrarily
/// oriented* set — e.g. the D24 cap square arrives as four segments whose starts are
/// `(2,0),(0,2),(0,0),(0,0)`, no two consecutive. Walking `edge_start` alone would
/// drop the `(2,2)` corner and double `(0,0)`, fanning a half-covered cap with a
/// zero-area triangle. This walks the incidence graph instead: seed with the first
/// edge oriented start→end, then at each step append the far endpoint of an unused
/// edge sharing the current tail (matching either orientation), stopping when the ring
/// closes on its head. The result is the ordered corner loop, ready to fan. A face
/// whose edges do not chain into a single closed loop yields the partial walk (the fan
/// then covers what connects) — the certified caps this slice emits are simple loops.
fn ordered_ring<B: Backend>(edges: &[Edge<B>]) -> Vec<Point2<B>> {
    if edges.is_empty() {
        return Vec::new();
    }
    let mut used = vec![false; edges.len()];
    used[0] = true;
    let mut ring = vec![edge_start(&edges[0]).clone(), edge_end(&edges[0]).clone()];
    loop {
        let tail = ring[ring.len() - 1].clone();
        if same_point(&tail, &ring[0]) {
            ring.pop(); // the walk returned to its head — drop the duplicate
            break;
        }
        let mut extended = false;
        for (i, e) in edges.iter().enumerate() {
            if used[i] {
                continue;
            }
            if same_point(edge_start(e), &tail) {
                ring.push(edge_end(e).clone());
            } else if same_point(edge_end(e), &tail) {
                ring.push(edge_start(e).clone());
            } else {
                continue;
            }
            used[i] = true;
            extended = true;
            break;
        }
        if !extended {
            break; // the edges do not chain into a single closed loop
        }
    }
    ring
}

/// A [`Surd`] that is in fact rational (`b = 0` or `d = 0`), else `None`. The M4-slice
/// cap boundary is rational; a genuinely irrational (single-radical) cap coordinate
/// would need the Surd-arithmetic lift, deferred with the curved-crease atlas.
fn surd_to_rat<B: Backend>(s: &Surd<B>) -> Option<Rat<B>> {
    let (a, b, d) = s.parts();
    if b.sign() == 0 || d.sign() == 0 {
        Some(a.clone())
    } else {
        None
    }
}

/// Lift a cap-plane point `(x, y)` back into 3-space through the **orthonormal** crease
/// frame, isometrically. With `s = |r₀|² = |n′₀|²` (one rational, `chart.normal_deriv_sq`
/// at the crease station), the unit ruling is `û = r₀/√s` and the unit normal is `n₀`
/// (`|n₀| = 1`, `û·n₀ = r₀·n₀/√s = 0`, since `n·n′ ≡ 0`). So
///
/// ```text
/// world = origin + x·(r₀/√s) + y·n₀ = (origin + y·n₀) + (x·r₀/s)·√s,
/// ```
///
/// and each world coordinate is a clean [`Surd`] `a + b√s` with `a = origin + y·n₀`,
/// `b = x·r₀/s`, sharing the **single radical `d = s`** across the whole cap — no
/// cross-radical arithmetic. The lift is metric-faithful: a unit cap square lifts to a
/// unit (not stretched) world square. `None` if the cap coordinate is irrational — the
/// cap-plane outline of this slice (the CAP-IN-D24 square) is rational by construction; a
/// genuinely irrational cap coordinate (curved crease) escalates cross-radical and stays
/// deferred with the curved-crease atlas.
fn lift<B: Backend>(pt: &Point2<B>, frame: &PiFrame<B>, s: &Rat<B>) -> Option<Vertex<B>> {
    let x = surd_to_rat(&pt.x)?;
    let y = surd_to_rat(&pt.y)?;
    let comp = |k: usize| {
        let a = frame.origin[k].add(&y.mul(&frame.v[k]));
        let b = x.mul(&frame.u[k]).div(s);
        Surd::new(a, b, s.clone())
    };
    Some([comp(0), comp(1), comp(2)])
}

/// Fan-triangulate the cap: each face's boundary chained into an ordered corner loop
/// (via [`ordered_ring`] — the arrangement does not store `outer` loop-ordered), lifted
/// into 3-space through `frame`, and fanned from the first corner. A loop that lifts to
/// fewer than three points (an irrational coordinate this slice cannot represent) is
/// skipped. Ordering the loop is what keeps the fan faithful: an unordered `edge_start`
/// walk would drop a corner and emit a degenerate (zero-area) triangle the CAD kernel
/// then rejects.
fn cap_tris<B: Backend>(cap: &CapOut<B>, frame: &PiFrame<B>, s: &Rat<B>) -> Vec<Tri<B>> {
    let mut tris = Vec::new();
    for face in &cap.region().faces {
        let ring: Vec<Vertex<B>> = ordered_ring(&face.outer)
            .iter()
            .filter_map(|p| lift(p, frame, s))
            .collect();
        for i in 1..ring.len().saturating_sub(1) {
            tris.push(Tri {
                v: [ring[0].clone(), ring[i].clone(), ring[i + 1].clone()],
            });
        }
    }
    tris
}

/// The oriented cap plane at the joint: origin at the crease point offset along the
/// normal (`c₀ + w·n₀`), `u` the ruling `r₀`, `v` the normal `n₀` — the frame the cap
/// boundary was projected through. `None` if the chart is singular at the crease station.
fn cap_frame<B: Backend>(chart: &Chart<B>, sigma_star: &Rat<B>, w: &Rat<B>) -> Option<PiFrame<B>> {
    let c0 = chart.pedal().eval(sigma_star)?;
    let r0 = chart.ruling().eval(sigma_star)?;
    let n0 = chart.normal().eval(sigma_star)?;
    let origin = [
        c0[0].add(&n0[0].mul(w)),
        c0[1].add(&n0[1].mul(w)),
        c0[2].add(&n0[2].mul(w)),
    ];
    Some(PiFrame {
        origin,
        u: r0,
        v: n0,
    })
}

/// Assemble the exact shell record of a certified one-joint closure.
///
/// Reconstructs the sewn shell's boundary from the [`Joint`]'s charts and the authored
/// [`ClosureTreatment`], guided by the [`ClosureValid`] witness: the two flank faces
/// (ruled from each chart over its retained σ-support at the offset `t.w.lo`, sampled on
/// a σ-grid), and — for a [`Ledge`](CapWitness::Ledge) cap — the cap face (the
/// [`CapOut`] region's outer loops lifted into the joint's cap plane). A
/// [`Miter`](CapWitness::Miter) cap contributes no separate planar face (the flanks meet
/// directly), so the shell is the two flank strips.
///
/// The result is exact: every vertex is a [`Surd`], and no float is produced here.
pub fn shell_from_closure<B: Backend>(
    joint: &Joint<B>,
    t: &ClosureTreatment<'_, B>,
    valid: &ClosureValid<B>,
) -> ShellRecord<B> {
    let w = &t.w.lo;
    let mut tris = Vec::new();
    tris.extend(flank_tris(
        joint.flank_a().chart(),
        &t.sigma_a,
        &t.mu,
        w,
        FLANK_STEPS,
    ));
    tris.extend(flank_tris(
        joint.flank_b().chart(),
        &t.sigma_b,
        &t.mu,
        w,
        FLANK_STEPS,
    ));
    if let CapWitness::Ledge(cap) = &valid.cap {
        // Emit the cap face only when the certificate reports no pinch vertices.
        // A pinched cap boundary is non-manifold — CAP-OUT-LINK excludes the pinch
        // from `V_∂`, so fanning `face.outer` across it would emit a non-manifold
        // face. `pinches().is_empty()` is that certificate precondition (read-only).
        if cap.pinches().is_empty() {
            let chart = joint.flank_a().chart();
            let sigma_star = &joint.crease().sigma_a;
            // `s = |r₀|² = |n′₀|²` at the crease station — the single radical the whole cap
            // shares. A singular crease (`s = 0`) has no cap plane, so the cap is skipped.
            if let (Some(frame), Some(s)) = (
                cap_frame(chart, sigma_star, w),
                chart.normal_deriv_sq().eval(sigma_star),
            ) {
                if s.sign() != 0 {
                    tris.extend(cap_tris(cap, &frame, &s));
                }
            }
        }
    }
    ShellRecord { tris }
}

#[cfg(test)]
mod tests {
    use super::*;
    use certify_core::Verdict;
    use closure::valid::closure_valid;
    use fixtures::closure_joint::{ledge_d24, one_joint, treatment};

    /// The one-joint ledge fold assembles a non-empty shell: two flank strips
    /// (`2·FLANK_STEPS` triangles each) plus the square cap's fan (a 4-corner loop → 2).
    #[test]
    fn ledge_fold_assembles_the_expected_shell() {
        let joint = one_joint();
        let d24 = ledge_d24();
        let t = treatment(&d24);
        let valid = match closure_valid(&joint, &t) {
            Verdict::Verified(v) => v,
            other => panic!(
                "the fold is CLOSURE_VALID: {}",
                matches!(other, Verdict::Verified(_))
            ),
        };
        let shell = shell_from_closure(&joint, &t, &valid);
        // 2·FLANK_STEPS per flank + a 4-vertex cap loop fanned into 2 triangles.
        assert_eq!(shell.len(), 4 * FLANK_STEPS + 2);
        assert!(matches!(valid.cap, CapWitness::Ledge(_)));
    }

    /// [`ordered_ring`] chains the arrangement's *unordered, mixed-orientation* cap
    /// edges into the full corner loop. The D24 square arrives as four segments whose
    /// starts are `(2,0),(0,2),(0,0),(0,0)` — walking `edge_start` alone would drop the
    /// `(2,2)` corner and double `(0,0)`, fanning a half cap plus a zero-area triangle
    /// (which OCCT's `BRepCheck` rejects). Ordering recovers all four distinct corners.
    #[test]
    fn ordered_ring_chains_scrambled_cap_edges_into_the_full_loop() {
        use geom::content::{CurveId, Line, Orient, SegPiece};
        let seg = |x0: i128, y0: i128, x1: i128, y1: i128| -> Edge<Bignum> {
            Edge::Seg(Box::new(SegPiece {
                line: Line {
                    a: Rat::from_i128(0),
                    b: Rat::from_i128(0),
                    c: Rat::from_i128(0),
                },
                start: Point2::from_rat(Rat::from_i128(x0), Rat::from_i128(y0)),
                end: Point2::from_rat(Rat::from_i128(x1), Rat::from_i128(y1)),
                orient: Orient::Ccw,
                source: CurveId(0),
            }))
        };
        // Exactly the D24 square as the arrangement stores `face.outer`.
        let edges = vec![
            seg(2, 0, 2, 2),
            seg(0, 2, 2, 2),
            seg(0, 0, 0, 2),
            seg(0, 0, 2, 0),
        ];
        let ring = ordered_ring(&edges);
        assert_eq!(
            ring.len(),
            4,
            "the square has four distinct corners: {ring:?}"
        );
        // Every corner appears exactly once — in particular the `(2,2)` an unordered
        // start-walk would have dropped, and no doubled `(0,0)`.
        for corner in [(0, 0), (2, 0), (2, 2), (0, 2)] {
            let p = Point2::from_rat(Rat::from_i128(corner.0), Rat::from_i128(corner.1));
            assert_eq!(
                ring.iter().filter(|q| **q == p).count(),
                1,
                "corner {corner:?} appears exactly once: {ring:?}"
            );
        }
        // Consecutive corners (cyclically) are distinct — no zero-area fan triangle.
        for i in 0..ring.len() {
            assert_ne!(
                ring[i],
                ring[(i + 1) % ring.len()],
                "adjacent corners differ"
            );
        }
    }

    /// The cap lift is **metric-faithful** (D.2): the unit cap square lifts to a unit
    /// world square — no stretch. Asserted on exact `Surd` coordinates. Between two
    /// same-radical vertices `p, q` (shared `d = s`), the squared world distance is
    /// `Σₖ (Δaₖ + Δbₖ√s)² = (Σ Δaₖ² + s·Σ Δbₖ²) + (2 Σ ΔaₖΔbₖ)·√s`; the identity
    /// `|n₀| = 1`, `|r₀|² = s`, `r₀·n₀ = 0` forces the radical coefficient to `0` and the
    /// rational part to the cap-plane `Δx² + Δy²`. (The pre-D.2 raw-ruling lift gave a
    /// squared x-edge of `s = 4`, a 2× stretch — this asserts `1`.)
    #[test]
    fn the_cap_lift_is_metric_faithful_a_unit_square_stays_unit() {
        let joint = one_joint();
        let chart = joint.flank_a().chart();
        let sigma_star = &joint.crease().sigma_a;
        let w = Rat::from_i128(1);
        let frame = cap_frame(chart, sigma_star, &w).expect("regular crease frame");
        let s = chart
            .normal_deriv_sq()
            .eval(sigma_star)
            .expect("regular normal derivative at the crease");
        // The physical crease station has |r₀|² = 4 ≠ 1 — the raw-ruling lift stretched by 2×.
        assert_eq!(
            s,
            Rat::from_i128(4),
            "the crease-station ruling speed² is 4"
        );

        let corner = |x: i128, y: i128| {
            let pt = Point2 {
                x: Surd::from_rat(Rat::from_i128(x)),
                y: Surd::from_rat(Rat::from_i128(y)),
            };
            lift(&pt, &frame, &s).expect("a rational cap corner lifts")
        };
        let (p00, p10, p11, p01) = (corner(0, 0), corner(1, 0), corner(1, 1), corner(0, 1));

        // Exact squared world distance between two same-radical (d = s) Surd vertices,
        // returned as `(rational_part, radical_coeff)` of `rational + radical·√s`.
        let dist2 = |p: &Vertex<Bignum>, q: &Vertex<Bignum>| {
            let (mut da2, mut db2, mut dadb) =
                (Rat::from_i128(0), Rat::from_i128(0), Rat::from_i128(0));
            for k in 0..3 {
                let (pa, pb, _) = p[k].parts();
                let (qa, qb, _) = q[k].parts();
                let da = qa.sub(pa);
                let db = qb.sub(pb);
                da2 = da2.add(&da.mul(&da));
                db2 = db2.add(&db.mul(&db));
                dadb = dadb.add(&da.mul(&db));
            }
            (da2.add(&s.mul(&db2)), dadb.add(&dadb))
        };
        // Each unit edge of the square lifts to a unit world edge (length² = 1, no √s term).
        for (p, q) in [(&p00, &p10), (&p10, &p11), (&p11, &p01), (&p01, &p00)] {
            let (rational, radical) = dist2(p, q);
            assert_eq!(radical, Rat::from_i128(0), "world edge is rational (no √s)");
            assert_eq!(
                rational,
                Rat::from_i128(1),
                "unit cap edge stays unit in world"
            );
        }
        // The diagonal lifts isometrically too (length² = 2), pinning both axes at once.
        let (diag_rational, diag_radical) = dist2(&p00, &p11);
        assert_eq!(diag_radical, Rat::from_i128(0));
        assert_eq!(
            diag_rational,
            Rat::from_i128(2),
            "unit diagonal stays √2 in world"
        );
    }

    /// Every flank triangle is non-degenerate at `w = t.w.lo`: the cylinder patch is a
    /// genuine 2D face there (it would collapse onto the fold axis at `w = 0`).
    #[test]
    fn flank_triangles_are_non_degenerate_at_the_certified_offset() {
        let joint = one_joint();
        let d24 = ledge_d24();
        let t = treatment(&d24);
        // Flank A alone, so we know these are all flank triangles.
        let tris = flank_tris(
            joint.flank_a().chart(),
            &t.sigma_a,
            &t.mu,
            &t.w.lo,
            FLANK_STEPS,
        );
        assert_eq!(tris.len(), 2 * FLANK_STEPS);
        // A triangle is non-degenerate iff its three vertices are pairwise distinct.
        for tri in &tris {
            let eq = |p: &Vertex<Bignum>, q: &Vertex<Bignum>| {
                (0..3).all(|k| p[k].cmp(&q[k]) == core::cmp::Ordering::Equal)
            };
            assert!(
                !eq(&tri.v[0], &tri.v[1]) && !eq(&tri.v[1], &tri.v[2]) && !eq(&tri.v[0], &tri.v[2])
            );
        }
    }
}
