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
//! The M4 cylinder fold's charts have a pedal that collapses onto the fold axis at
//! `w = 0` (the ruled patch degenerates to a line). The flanks are therefore
//! sampled at `w = t.w.lo` — the **low end of the certified normal-offset box**
//! `[w⁻, w⁺]` — where the patch is a genuine 2D face, faithful to the same interval
//! the closure was certified over.
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

/// Lift a cap-plane point `(x, y)` back into 3-space through `frame`:
/// `origin + x·u + y·v`, in pure rational arithmetic. `None` if either coordinate is
/// irrational (not represented in this slice).
fn lift<B: Backend>(pt: &Point2<B>, frame: &PiFrame<B>) -> Option<Vertex<B>> {
    let x = surd_to_rat(&pt.x)?;
    let y = surd_to_rat(&pt.y)?;
    let comp = |k: usize| {
        frame.origin[k]
            .add(&x.mul(&frame.u[k]))
            .add(&y.mul(&frame.v[k]))
    };
    Some([
        Surd::from_rat(comp(0)),
        Surd::from_rat(comp(1)),
        Surd::from_rat(comp(2)),
    ])
}

/// Fan-triangulate the cap: each face's outer loop, its edge-start points lifted into
/// 3-space through `frame`, fanned from the first vertex. A loop that lifts to fewer
/// than three points (an irrational coordinate this slice cannot represent) is skipped.
fn cap_tris<B: Backend>(cap: &CapOut<B>, frame: &PiFrame<B>) -> Vec<Tri<B>> {
    let mut tris = Vec::new();
    for face in &cap.region().faces {
        let ring: Vec<Vertex<B>> = face
            .outer
            .iter()
            .filter_map(|e| lift(edge_start(e), frame))
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
        if let Some(frame) = cap_frame(joint.flank_a().chart(), &joint.crease().sigma_a, w) {
            tris.extend(cap_tris(cap, &frame));
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
