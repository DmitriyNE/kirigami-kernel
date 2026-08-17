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
//!
//! # Where the cut actually reached
//!
//! [`sketch_faces`] shows what was *asked for*. [`cutter_bodies`] shows what was *got*: the
//! resolver's own certified footprint — the closed `(σ, µ̂)` loop the solid is cut with — lifted
//! back to three dimensions, with the sketch plane it was cast from and the generatrices between.
//!
//! The two caps are the same loop under two maps, which is what makes the pair worth emitting:
//!
//! - the **far cap** is `chart.surface(µ̂, 0)` evaluated at each footprint σ — on the sheet's
//!   **neutral** surface, `w = 0`. A viewer therefore shows it buried mid-thickness rather than
//!   lying on a lid, and that is the honest place for it: the footprint is a fact about the chart,
//!   which is the neutral surface, not about whichever face of the stackup you happen to see first;
//! - the **near cap** is each of those points cast *back* along its own generatrix,
//!   [`Cast::coords`](develop::extrude::Cast::coords) then
//!   [`Frame::point`](develop::extrude::Frame::point) — in the sketch plane, exactly, by the same
//!   identity [`sketch_faces`] reports;
//! - the **walls** are ruled between corresponding points, so each one *is* a generatrix segment.
//!
//! So the near cap and the sketch face are the same curve computed two entirely different ways —
//! one from the authored profile edges, one from the traced footprint pulled back through the
//! chart. They agree only if the tracer, the chart and the frame all agree, and nothing about the
//! certified ε would tell you if they did not.
//!
//! **Triangles throughout**, and not for want of a better face type: a ruled quad between two
//! generatrices is not coplanar and a footprint lifted onto a curved sheet is not planar, so
//! [`FaceSurface::Plane`] is honest only on a triangle. That a body reads as visibly faceted is a
//! second benefit rather than a cost — it looks like the diagnostic it is.

use crate::part::{Cutter, Part, PartFault};
use certify_core::Verdict;
use export::approx::{rat_to_f64, surd_to_f64};
use export::brep::{Brep, EdgeGeom, FaceSurface, HalfEdge};
use geom::content::{Edge, Point2};
use lattice::{Backend, Rat, Surd};
use std::collections::BTreeMap;

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
        // A closed walk ends where it started, so the last sample repeats the first. The wire is
        // built cyclically below, so keeping it would put a **zero-length edge** in the face —
        // exactly the sub-tolerance edge OCCT refuses (#267). Emitted as a picture only, this went
        // unseen until the dump was actually written out.
        if ring.len() >= 2 && ring[ring.len() - 1] == ring[0] {
            ring.pop();
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

// — (b) the cutter body: where the cut actually reached —

/// A point in the chart's domain coordinates `(σ, µ̂)`.
type P2<B> = (Rat<B>, Rat<B>);

/// A triangulated ring: the cleaned polygon, and index triples into it.
type Triangulation<B> = (Vec<P2<B>>, Vec<[usize; 3]>);

/// `|a| ⊔ |b|`, the running worst residual.
fn worst<B: Backend>(acc: Rat<B>, v: Rat<B>) -> Rat<B> {
    let v = if v.sign() < 0 { v.neg() } else { v };
    if v > acc { v } else { acc }
}

/// The exact 2-D cross product `(a − o) × (b − o)`.
fn cross2<B: Backend>(o: &P2<B>, a: &P2<B>, b: &P2<B>) -> Rat<B> {
    a.0.sub(&o.0)
        .mul(&b.1.sub(&o.1))
        .sub(&a.1.sub(&o.1).mul(&b.0.sub(&o.0)))
}

/// Is `p` inside the closed counter-clockwise triangle `a b c`?
fn in_triangle<B: Backend>(p: &P2<B>, a: &P2<B>, b: &P2<B>, c: &P2<B>) -> bool {
    cross2(a, b, p).sign() >= 0 && cross2(b, c, p).sign() >= 0 && cross2(c, a, p).sign() >= 0
}

/// **Triangulate a simple polygon by ear clipping**, in exact rational arithmetic. Returns the
/// polygon re-wound counter-clockwise with its exactly-collinear vertices dropped, and index
/// triples into *that* ring.
///
/// Ear clipping rather than a fan because a traced footprint is routinely non-convex — that is the
/// whole content of AUTH.2 — and a fan from any one vertex of a non-convex polygon lays triangles
/// outside it. The arithmetic is exact for the same class of reason: every test here is a *sign*
/// question, and a sign decided by a rounded cross product is how a triangulation quietly folds
/// over itself.
///
/// `None` when the ring has no area, or when a pass finds no ear at all. A simple polygon always
/// has one, so that is a refusal — the caller reports it rather than falling back to a fan, since
/// a fan would be wrong on exactly the inputs that got here.
fn triangulate<B: Backend>(poly: &[P2<B>]) -> Option<Triangulation<B>> {
    // A collinear vertex carries no shape and can never be clipped (its ear has zero area), so it
    // would stall the loop below. Dropping one can expose another, hence the stack.
    let mut ring: Vec<P2<B>> = Vec::with_capacity(poly.len());
    for p in poly {
        while ring.len() >= 2 && cross2(&ring[ring.len() - 2], &ring[ring.len() - 1], p).is_zero() {
            ring.pop();
        }
        ring.push(p.clone());
    }
    while ring.len() >= 3
        && cross2(&ring[ring.len() - 2], &ring[ring.len() - 1], &ring[0]).is_zero()
    {
        ring.pop();
    }
    while ring.len() >= 3 && cross2(&ring[ring.len() - 1], &ring[0], &ring[1]).is_zero() {
        ring.remove(0);
    }
    let n = ring.len();
    if n < 3 {
        return None;
    }

    let mut area2 = Rat::from_i128(0);
    for i in 0..n {
        let j = (i + 1) % n;
        area2 = area2.add(&ring[i].0.mul(&ring[j].1).sub(&ring[j].0.mul(&ring[i].1)));
    }
    if area2.is_zero() {
        return None;
    }
    if area2.sign() < 0 {
        ring.reverse();
    }

    let mut live: Vec<usize> = (0..n).collect();
    let mut tris: Vec<[usize; 3]> = Vec::with_capacity(n - 2);
    while live.len() > 3 {
        let m = live.len();
        let mut clipped = None;
        for k in 0..m {
            let (ia, ib, ic) = (live[(k + m - 1) % m], live[k], live[(k + 1) % m]);
            let (a, b, c) = (&ring[ia], &ring[ib], &ring[ic]);
            // Reflex or collinear: not an ear.
            if cross2(a, b, c).sign() <= 0 {
                continue;
            }
            // Any other live vertex in the closed ear blocks it — closed rather than open, so a
            // vertex sitting *on* the ear's edge cannot leave a T-junction behind.
            if live
                .iter()
                .any(|&v| v != ia && v != ib && v != ic && in_triangle(&ring[v], a, b, c))
            {
                continue;
            }
            tris.push([ia, ib, ic]);
            clipped = Some(k);
            break;
        }
        live.remove(clipped?);
    }
    tris.push([live[0], live[1], live[2]]);
    Some((ring, tris))
}

/// One body's edge table: the map from an unordered vertex pair to the edge id that carries it.
///
/// Two triangles meet along an edge exactly when both wires name the same id, so this is the whole
/// of the watertightness — nothing here compares a coordinate. It is per-body deliberately: two
/// cutter bodies that happen to touch must stay two bodies.
#[derive(Default)]
struct Wires {
    ids: BTreeMap<(usize, usize), usize>,
}

impl Wires {
    /// The directed use of the edge `a → b`, creating the edge on first sight.
    fn half<B: Backend>(&mut self, brep: &mut Brep<B>, a: usize, b: usize) -> HalfEdge {
        let key = if a <= b { (a, b) } else { (b, a) };
        let id = match self.ids.get(&key) {
            Some(&id) => id,
            None => {
                let id = brep.add_edge(key.0, key.1, EdgeGeom::Line);
                self.ids.insert(key, id);
                id
            }
        };
        (id, a > b)
    }

    /// Emit the planar triangle `a b c`, sharing every edge it has in common with what came before.
    fn triangle<B: Backend>(&mut self, brep: &mut Brep<B>, a: usize, b: usize, c: usize) {
        let wire = vec![
            self.half(brep, a, b),
            self.half(brep, b, c),
            self.half(brep, c, a),
        ];
        brep.add_face(FaceSurface::Plane, wire);
    }
}

/// What one emitted body is, and which op it belongs to.
pub struct BodyReport {
    /// The material op that cut it — an index into the part's ops, the order
    /// [`Part::cutters`](crate::part::Part::cutters) reports.
    pub op: usize,
    /// The region whose chart carries its far cap.
    pub region: usize,
    /// Footprint vertices, after the collinear ones are dropped. The far cap has this many, and so
    /// does the near cap when there is one.
    pub vertices: usize,
    /// Whether the near cap and the walls were emitted — true exactly for an extruded cutter,
    /// which is the only kind with a sketch plane to cast back to. `false` leaves the far cap on
    /// its own: an honest open patch showing where a metric cutter reached, and no more.
    pub solid: bool,
}

/// What a cutter dump produced, alongside the geometry.
pub struct CutterDump<B: Backend> {
    /// The bodies, as one compound — closed shells for the extruded cutters, open far-cap patches
    /// for the metric ones.
    pub brep: Brep<B>,
    /// One entry per emitted body, in footprint order.
    pub bodies: Vec<BodyReport>,
    /// The largest certified cut bound over the footprints this was built from — the same ε the
    /// part's own report carries, restated here because it is what the picture's *shape* is good
    /// to.
    pub eps: Rat<B>,
    /// The largest `|N·(X − o)|` over every near-cap vertex — **exactly zero**, since a cast-back
    /// point is by construction `Frame::point` of its own frame coordinates. Measured rather than
    /// asserted, for the same reason [`SketchDump::plane_residual`] is.
    pub near_residual: Rat<B>,
}

impl<B: Backend> CutterDump<B> {
    /// One line, the format the demos print.
    pub fn summary(&self) -> String {
        let closed = self.bodies.iter().filter(|b| b.solid).count();
        format!(
            "{} bodies ({closed} closed, {} far-cap only) → {} faces, {} vertices   \
             near-cap plane residual {}   ε {:.3e}",
            self.bodies.len(),
            self.bodies.len() - closed,
            self.brep.faces().len(),
            self.brep.verts().len(),
            if self.near_residual.is_zero() {
                "exactly 0"
            } else {
                "NONZERO — a near cap is not in its own sketch plane"
            },
            rat_to_f64(&self.eps),
        )
    }
}

/// **Every cutter's traced footprint, as a body between the sheet it reached and the plane it was
/// drawn in.**
///
/// `segments` is the chord budget the footprint loops are certified at; `16` is the number the
/// solid path itself uses, so the default picture is the geometry the part was built from rather
/// than a finer one drawn alongside it.
///
/// An extruded cutter yields a **closed** shell — near cap, walls, far cap, every edge shared by
/// exactly two triangles. A metric cutter (a drill, a half-space) has no sketch plane to cast back
/// to, so it yields its far cap alone, and [`BodyReport::solid`] says which is which rather than
/// leaving the caller to infer it from a face count.
///
/// Diagnostic geometry, so: write it with [`write_brep`](export::step::write_brep) and **never**
/// with [`emit_certified_step`](export::step::emit_certified_step). That a body's shell happens to
/// close is a fact about the tracer — a footprint is a simple closed curve — and not a warrant for
/// any of the geometry inside it.
///
/// Refuses whatever the resolution refuses ([`Part::develop`](crate::part::Part::develop)'s faults,
/// identically, since it runs the same prelude), and is `Unresolved` on the same loose ε.
///
/// ```no_run
/// use certify_core::Verdict;
/// use lattice::{Bignum, Rat};
///
/// let apex = develop::extrude::Apex::direction([
///     Rat::<Bignum>::from_i128(0),
///     Rat::from_i128(0),
///     Rat::from_i128(1),
/// ])
/// .unwrap();
/// let part = acceptance::sketch_panel(Some((apex, acceptance::ell_slot())));
///
/// let Verdict::Verified(dump) = author::dump::cutter_bodies(&part, 16) else {
///     panic!("the L-slot resolves")
/// };
/// assert_eq!(dump.brep.free_edges(), 0, "an extruded cutter's body is a closed shell");
/// assert!(dump.near_residual.is_zero(), "the near cap is in the sketch plane, exactly");
///
/// // Package it with the folded sheet and write it **raw** — never `emit_certified_step`.
/// let Verdict::Verified(solid) = part.solid() else { panic!("the part resolves") };
/// let mut compound = solid.into_brep();
/// compound.absorb(author::dump::sketch_faces(&part, 20).brep);
/// compound.absorb(dump.brep);
/// # #[cfg(feature = "step")]
/// export::step::write_brep("cutter_dump.step", &compound);
/// ```
pub fn cutter_bodies<B: Backend>(
    part: &Part<B>,
    segments: usize,
) -> Verdict<CutterDump<B>, PartFault, Rat<B>> {
    use crate::realize::RErr;
    let built = match part.build_regions() {
        Ok(b) => b,
        Err(f) => return Verdict::Refuted(f),
    };
    let structure = match crate::resolve::sweep(part, &built) {
        Ok(s) => s,
        Err(f) => return Verdict::Refuted(f),
    };
    let prints = match crate::realize::footprints(part, &built, &structure, segments.max(8)) {
        Ok(p) => p,
        Err(RErr::Fault(f)) => return Verdict::Refuted(f),
        Err(RErr::Loose(e)) => return Verdict::Unresolved(e),
    };

    let zero = Rat::from_i128(0);
    let mut brep = Brep::new();
    let mut bodies = Vec::with_capacity(prints.len());
    let mut eps = Rat::from_i128(0);
    let mut near_residual = Rat::from_i128(0);

    for fp in &prints {
        let Some((ring, tris)) = triangulate(&fp.poly) else {
            return Verdict::Refuted(PartFault::LoopBroken);
        };
        eps = worst(eps, fp.eps.clone());

        // The far cap: the footprint on the sheet. `µ̂` **is** the chart's ruling parameter, so
        // this is the surface point the tracer's own predicate was evaluated at.
        let chart = &built.charts[fp.region];
        let mut far = Vec::with_capacity(ring.len());
        for (s, m) in &ring {
            match chart.surface(m, &zero).eval(s) {
                Some(x) => far.push(x),
                None => return Verdict::Refuted(PartFault::Pole),
            }
        }

        // The near cap: each far-cap point cast *back* along its own generatrix. Casting back is
        // what makes the two caps an exact bijection; matching traced vertices against profile
        // corners would not be one, because the tracer's vertices sit where the *events* are.
        let near = match &part.ops[fp.op].1 {
            Cutter::Extrude(e) => {
                let Ok(cast) = e.cast() else {
                    return Verdict::Refuted(PartFault::CutUnresolved { op: fp.op });
                };
                let mut near = Vec::with_capacity(far.len());
                for x in &far {
                    // `None` only where the generatrix runs parallel to the sketch plane, which no
                    // point of a footprint does — the cut it bounds came down that very ray.
                    let Some((a, b)) = cast.coords(x) else {
                        return Verdict::Refuted(PartFault::CutUnresolved { op: fp.op });
                    };
                    let p = e.frame.point(&a, &b);
                    near_residual = worst(near_residual, plane_residual(&e.frame, &p));
                    near.push(p);
                }
                Some(near)
            }
            _ => None,
        };

        let push = |brep: &mut Brep<B>, pts: &[[Rat<B>; 3]]| -> Vec<usize> {
            pts.iter()
                .map(|p| {
                    brep.add_vertex([
                        Surd::from_rat(p[0].clone()),
                        Surd::from_rat(p[1].clone()),
                        Surd::from_rat(p[2].clone()),
                    ])
                })
                .collect()
        };
        let far_ids = push(&mut brep, &far);
        let near_ids = near.as_ref().map(|n| push(&mut brep, n));

        let mut w = Wires::default();
        for t in &tris {
            w.triangle(&mut brep, far_ids[t[0]], far_ids[t[1]], far_ids[t[2]]);
        }
        if let Some(near_ids) = &near_ids {
            // The near cap carries the same triangulation wound the other way, and each wall quad
            // splits on the diagonal `near_i → far_{i+1}`. Every edge then falls to exactly two
            // triangles traversing it in opposite directions, which is what closes the shell.
            for t in &tris {
                w.triangle(&mut brep, near_ids[t[2]], near_ids[t[1]], near_ids[t[0]]);
            }
            let n = ring.len();
            for i in 0..n {
                let j = (i + 1) % n;
                w.triangle(&mut brep, near_ids[i], near_ids[j], far_ids[j]);
                w.triangle(&mut brep, near_ids[i], far_ids[j], far_ids[i]);
            }
        }

        bodies.push(BodyReport {
            op: fp.op,
            region: fp.region,
            vertices: ring.len(),
            solid: near_ids.is_some(),
        });
    }

    Verdict::Verified(CutterDump {
        brep,
        bodies,
        eps,
        near_residual,
    })
}
