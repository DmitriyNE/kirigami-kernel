//! **Diagnostic geometry** — what a picture can check that a certificate cannot.
//!
//! Everything in this module is a *picture*. None of it carries a verdict, none of it routes
//! through [`emit_certified_step`](export::step::emit_certified_step), and the one way to write it
//! out is the raw [`write_brep`](export::step::write_brep). A diagnostic that arrived with a
//! certificate attached would be a lie about what was checked.
//!
//! # The sketch occupies the plane it cuts from
//!
//! An extruded cutter's frame is a **search result** — `develop::pick`'s ray pick snaps a picked
//! plane to rationals and reports a backward error (AUTH.1c). The certificate says the snapped
//! plane is close to the picked one. It cannot say the *pick* landed where the author meant, and no
//! certificate can: that is a question about intent, and the only instrument for it is a picture.
//!
//! So the sketch face is emitted at its true three-dimensional position, and every vertex of it is
//! [`Frame::point(a, b)`](develop::extrude::Frame::point) — the same exact map the wall equations
//! are built from, not a plane re-derived for the picture and not the profile drawn at the origin.
//! A sketch rendered skew to the surface it cut says the pick is wrong. A sketch rendered anywhere
//! else says nothing at all.
//!
//! That claim is **metric, not visual**, and [`plane_residual`] is how it is checked: every emitted
//! vertex satisfies the frame's own plane equation `N·(X − o) = 0` **exactly**, for any rational
//! `(a, b)` whatsoever. Which means the in-plane sampling below — arcs chorded, `Surd` extrema
//! bracketed to rationals — costs the *shape* of the outline a little and costs the *plane* nothing.
//! The two are separate claims and the test asserts them separately.

use crate::part::{Cutter, Part};
use export::approx::{rat_to_f64, surd_to_f64};
use export::brep::{Brep, EdgeGeom, FaceSurface};
use geom::content::{Edge, Point2};
use lattice::{Backend, Rat, Surd};

/// Interior chords per arc when a profile arc is sampled into the sketch wire.
const ARC_CHORDS: usize = 16;

/// The 2-D endpoints of a profile edge, as `(start, end)`.
fn ends<B: Backend>(e: &Edge<B>) -> (&Point2<B>, &Point2<B>) {
    match e {
        Edge::Seg(s) => (&s.start, &s.end),
        Edge::Arc(a) => (&a.start, &a.end),
    }
}

/// A float view of a frame coordinate — sampling only, never a predicate.
fn f(p: &Point2<impl Backend>) -> [f64; 2] {
    [surd_to_f64(&p.x), surd_to_f64(&p.y)]
}

/// The frame-coordinate samples of one profile edge, from its `start` to its `end`, reversed when
/// the walk traverses it backwards.
///
/// A segment contributes its two endpoints; an arc contributes [`ARC_CHORDS`] chords along the true
/// circle. The samples are *snapped rationals*: an arc's interior points and its `Surd` extrema
/// alike. See the module docs for why that is a statement about the outline and not about the plane.
fn edge_samples<B: Backend>(e: &Edge<B>, forward: bool, bits: u32) -> Vec<[Rat<B>; 2]> {
    let snap = |v: f64| export::approx::f64_to_rat::<B>(v, bits);
    let exact =
        |p: &Point2<B>| -> [Rat<B>; 2] { [snap(surd_to_f64(&p.x)), snap(surd_to_f64(&p.y))] };
    let mut out = match e {
        Edge::Seg(s) => vec![exact(&s.start), exact(&s.end)],
        Edge::Arc(a) => {
            use core::f64::consts::PI;
            let (cx, cy) = (rat_to_f64(&a.circle.cx), rat_to_f64(&a.circle.cy));
            let r = rat_to_f64(&a.circle.r2).max(0.0).sqrt();
            let [sx, sy] = f(&a.start);
            let [ex, ey] = f(&a.end);
            let (mut t0, mut t1) = ((sy - cy).atan2(sx - cx), (ey - cy).atan2(ex - cx));
            if let geom::content::Half::Lower = a.half {
                if t0 > 0.0 {
                    t0 -= 2.0 * PI;
                }
                if t1 > 0.0 {
                    t1 -= 2.0 * PI;
                }
            }
            let mut pts = vec![exact(&a.start)];
            for i in 1..ARC_CHORDS {
                let t = t0 + (t1 - t0) * (i as f64 / ARC_CHORDS as f64);
                pts.push([snap(cx + r * t.cos()), snap(cy + r * t.sin())]);
            }
            pts.push(exact(&a.end));
            pts
        }
    };
    if !forward {
        out.reverse();
    }
    out
}

/// Chain a profile's arrangement edges into closed loops of frame-coordinate samples.
///
/// The edges arrive as an unordered bag whose stored `start`/`end` may run either way (they are
/// post-decomposition x-monotone pieces), so the walk chains on **exact** [`Point2`] equality — the
/// same discipline `export::svg` uses, and for the same reason: two edges are joined only where
/// they truly meet, so a pinched outline splits into its cycles instead of being bridged.
fn profile_loops<B: Backend>(edges: &[Edge<B>], bits: u32) -> Vec<Vec<[Rat<B>; 2]>> {
    let n = edges.len();
    let mut used = vec![false; n];
    let mut loops = Vec::new();

    for seed in 0..n {
        if used[seed] {
            continue;
        }
        used[seed] = true;
        let (anchor, _) = ends(&edges[seed]);
        let anchor = anchor.clone();
        let mut ring = edge_samples(&edges[seed], true, bits);
        let mut tail = ends(&edges[seed]).1.clone();

        while tail != anchor {
            let next = (0..n).find(|&j| {
                !used[j] && {
                    let (a, b) = ends(&edges[j]);
                    *a == tail || *b == tail
                }
            });
            let Some(j) = next else { break };
            used[j] = true;
            let (a, b) = ends(&edges[j]);
            let forward = *a == tail;
            tail = if forward { b.clone() } else { a.clone() };
            let pts = edge_samples(&edges[j], forward, bits);
            // `pts[0]` is the running tail, already in the ring.
            ring.extend_from_slice(&pts[1..]);
        }
        if ring.len() >= 3 {
            loops.push(ring);
        }
    }
    loops
}

/// What a sketch dump produced, alongside the geometry.
pub struct SketchDump<B: Backend> {
    /// One planar face per profile loop, each at its cutter's true 3-D frame position.
    pub brep: Brep<B>,
    /// How many extruded cutters contributed.
    pub cutters: usize,
    /// The largest `|N·(X − o)|` over every emitted vertex — **exactly zero** when the faces really
    /// do lie in the planes they were built from, which is the point of measuring it.
    pub plane_residual: Rat<B>,
}

impl<B: Backend> SketchDump<B> {
    /// One line, the format the demos print.
    pub fn summary(&self) -> String {
        format!(
            "{} cutters → {} sketch faces, {} vertices   plane residual {}",
            self.cutters,
            self.brep.faces().len(),
            self.brep.verts().len(),
            if self.plane_residual.is_zero() {
                "exactly 0".to_string()
            } else {
                "NONZERO — a face is not in its own frame's plane".to_string()
            },
        )
    }
}

/// `N·(X − o)` for a point against a frame — zero exactly when `X` lies in the frame's plane.
pub fn plane_residual<B: Backend>(frame: &develop::extrude::Frame<B>, x: &[Rat<B>; 3]) -> Rat<B> {
    let n = frame.normal();
    let o = frame.origin();
    (0..3)
        .map(|i| n[i].mul(&x[i].sub(&o[i])))
        .fold(Rat::from_i128(0), |a, b| a.add(&b))
}

/// **The authored sketch of every extruded cutter, as planar faces at their true 3-D positions.**
///
/// One [`FaceSurface::Plane`] per closed profile loop — no new surface kind, no CAD-bridge change.
/// `bits` is the dyadic precision the in-plane samples are snapped to (20 is ample for a picture).
///
/// This is the importer's faithfulness echo and the frame's placement check in one artifact: if the
/// file was read wrong the outline is the wrong shape, and if the plane was picked wrong the
/// outline is in the wrong place. The two failures look completely different, which is what makes
/// the picture worth emitting.
pub fn sketch_faces<B: Backend>(part: &Part<B>, bits: u32) -> SketchDump<B> {
    let mut brep = Brep::new();
    let mut cutters = 0usize;
    let mut residual = Rat::from_i128(0);

    for (_, cutter) in part.cutters() {
        let Cutter::Extrude(e) = cutter else { continue };
        cutters += 1;
        for ring in profile_loops(&e.profile, bits) {
            let pts: Vec<[Rat<B>; 3]> = ring
                .iter()
                .map(|[a, b]| {
                    let v = e.frame.point(a, b);
                    [v[0].clone(), v[1].clone(), v[2].clone()]
                })
                .collect();
            for p in &pts {
                let r = plane_residual(&e.frame, p);
                let r = if r.sign() < 0 { r.neg() } else { r };
                if r > residual {
                    residual = r;
                }
            }
            let ids: Vec<usize> = pts
                .iter()
                .map(|p| {
                    brep.add_vertex([
                        Surd::from_rat(p[0].clone()),
                        Surd::from_rat(p[1].clone()),
                        Surd::from_rat(p[2].clone()),
                    ])
                })
                .collect();
            let m = ids.len();
            let wire: Vec<(usize, bool)> = (0..m)
                .map(|i| {
                    (
                        brep.add_edge(ids[i], ids[(i + 1) % m], EdgeGeom::Line),
                        false,
                    )
                })
                .collect();
            brep.add_face(FaceSurface::Plane, wire);
        }
    }

    SketchDump {
        brep,
        cutters,
        plane_residual: residual,
    }
}
