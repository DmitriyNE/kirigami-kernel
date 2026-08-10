//! The exact **boundary-representation IR** slice 3 emits — a shared vertex table, a
//! shared edge table, and faces whose wires reference edges *by index*.
//!
//! This is the neutral exact geometry the STEP surface bridge consumes, the ruled-face
//! analogue of [`crate::shell`]'s triangle soup. Its defining property is **watertight
//! by construction through identity**: two faces meet along an edge iff their wires
//! reference the *same* [`edge id`](Brep::add_edge). There is no float tolerance and no
//! coincidence test — a shared seam is a shared index, so the incidence of an edge is a
//! pure combinatorial fact ([`edge_incidence`](Brep::edge_incidence)), computable here
//! with no CAD kernel. This is the internal precondition the D3.2 OCCT audit is a
//! differential *oracle* for ("oracle ∧ audit, never oracle-instead").
//!
//! Geometry is exact: vertices are `a + b√d` [`Surd`] points (rational vertices are the
//! `b = 0` case, as in [`crate::shell`]); a curved edge carries an exact rational
//! [`RatBezier`] (`crate::bezier`); a ruled face names its base edge and an exact
//! rational extrusion direction. No float appears — the exact→`f64` cast happens once,
//! later, in the feature-gated STEP bridge.
//!
//! The IR is intentionally minimal (no full DCEL): faces carry only their boundary
//! wire, and topology facts are recomputed from the edge references on demand. That is
//! enough for the certified-seam / honest-open export — the certificate backs the seam
//! edges, not a globally closed shell, so a free edge on the substrate boundary is an
//! honest, expected outcome, not a defect.
//!
//! # Example
//!
//! ```
//! use export::brep::{Brep, EdgeGeom};
//! use lattice::{Bignum, Rat, Surd};
//!
//! let r = |v: i128| Surd::<Bignum>::from_rat(Rat::from_i128(v));
//! let mut b = Brep::<Bignum>::new();
//! // A shared seam edge between two triangular faces meeting along it.
//! let v0 = b.add_vertex([r(0), r(0), r(0)]);
//! let v1 = b.add_vertex([r(1), r(0), r(0)]);
//! let v2 = b.add_vertex([r(0), r(1), r(0)]);
//! let v3 = b.add_vertex([r(1), r(1), r(0)]);
//! let seam = b.add_edge(v1, v2, EdgeGeom::Line); // the shared edge
//! let a0 = b.add_edge(v0, v1, EdgeGeom::Line);
//! let a1 = b.add_edge(v2, v0, EdgeGeom::Line);
//! let b0 = b.add_edge(v1, v3, EdgeGeom::Line);
//! let b1 = b.add_edge(v3, v2, EdgeGeom::Line);
//! b.add_plane(vec![(a0, false), (seam, false), (a1, false)]);
//! b.add_plane(vec![(b0, false), (b1, false), (seam, true)]); // reuses `seam` by identity
//! // The seam is shared (incidence 2); the outer edges are free (incidence 1).
//! assert_eq!(b.edge_incidence()[seam], 2);
//! assert_eq!(b.free_edges(), 4);
//! assert_eq!(b.nonmanifold_edges(), 0);
//! ```

use crate::bezier::{RatBezier, RatBezierSurface};
use lattice::{Backend, Bignum, Rat, Surd};

/// One B-rep vertex: an exact `a + b√d` point in 3-space. Rational vertices (the `b = 0`
/// case) are kept cheap by [`Surd`]. Same shape as [`crate::shell::Vertex`].
pub type Vertex<B> = [Surd<B>; 3];

/// The geometric carrier of an edge between its two endpoint vertices.
pub enum EdgeGeom<B: Backend = Bignum> {
    /// A straight segment — the two endpoint vertices determine it fully.
    Line,
    /// A curved edge carrying an exact rational Bézier (from [`crate::bezier`]). Its
    /// endpoints are expected to match the edge's endpoint vertices.
    RationalBezier(RatBezier<B>),
}

/// One edge of the B-rep: an ordered pair of endpoint vertex indices and a geometric
/// carrier. An edge's **index in the [`Brep`] edge table is its identity** — sharing an
/// edge across faces means referencing this index, never comparing coordinates.
pub struct BEdge<B: Backend = Bignum> {
    /// Index of the start vertex in the [`Brep`] vertex table.
    pub start: usize,
    /// Index of the end vertex in the [`Brep`] vertex table.
    pub end: usize,
    /// The edge's geometric carrier.
    pub geom: EdgeGeom<B>,
}

/// One directed use of an edge in a face wire: the edge id and whether it is traversed
/// against its stored orientation.
pub type HalfEdge = (usize, bool);

/// The surface a face lies on.
pub enum FaceSurface<B: Backend = Bignum> {
    /// A planar face — its bounding wire is coplanar (exact by construction); the plane
    /// is inferred from the wire by the CAD bridge.
    Plane,
    /// A ruled face = the `base` edge's carrier swept along the exact direction `dir`
    /// (an OCCT `Geom_SurfaceOfLinearExtrusion`). `base` is an edge id in the [`Brep`]
    /// edge table; it need not appear in this face's own wire.
    LinearExtrusion {
        /// Edge id of the base curve that is extruded.
        base: usize,
        /// The exact rational extrusion direction.
        dir: [Rat<B>; 3],
    },
    /// An exact rational tensor-product Bézier patch (an OCCT rational
    /// `Geom_BSplineSurface`), carrying its own control net. The surface a *curved* wall
    /// needs when no constant-direction extrusion expresses it — e.g. the flank slab's
    /// `μ = const` wall, ruled along the rotating normal `n(σ)`. The bounding wire trims
    /// it.
    RationalPatch(RatBezierSurface<B>),
}

/// One face of the B-rep: a surface, the outer boundary wire that trims it, and any interior
/// **hole** wires (an ordered list of directed edge uses per loop).
pub struct Face<B: Backend = Bignum> {
    /// The surface this face lies on.
    pub surface: FaceSurface<B>,
    /// The outer boundary wire — directed edge uses, in traversal order.
    pub wire: Vec<HalfEdge>,
    /// Interior hole wires — one directed loop per hole, wound opposite to `wire`. Empty for a
    /// plain (hole-free) face. A hole loop shares no vertex or edge with the outer wire (it is
    /// strictly interior), so it contributes its own free edges to [`edge_incidence`](Brep::edge_incidence).
    pub holes: Vec<Vec<HalfEdge>>,
}

/// An exact boundary representation: a shared vertex table, a shared edge table (index =
/// identity), and faces whose wires reference edges by index. Watertight-by-identity:
/// two faces meet along an edge iff both wires reference the same edge id.
pub struct Brep<B: Backend = Bignum> {
    verts: Vec<Vertex<B>>,
    edges: Vec<BEdge<B>>,
    faces: Vec<Face<B>>,
}

/// The flat index-array certificate a [`Brep`] hands to the trusted
/// `certify_core::shell::closed_shell` checker — the untrusted-searcher → proven-checker
/// bridge. It carries only combinatorics (no coordinates, no surface types): the vertex
/// count, the edge endpoint tables, and the face wires in CSR form. Produced by
/// [`Brep::to_shell_certificate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellCertificate {
    /// Number of vertices.
    pub n_verts: usize,
    /// Per-edge start-vertex ids (parallel to [`edge_end`](Self::edge_end)).
    pub edge_start: Vec<usize>,
    /// Per-edge end-vertex ids.
    pub edge_end: Vec<usize>,
    /// All face wires' edge ids, concatenated (indexed by [`face_start`](Self::face_start)).
    pub wire_edge: Vec<usize>,
    /// Whether each half-edge is traversed reversed, parallel to
    /// [`wire_edge`](Self::wire_edge).
    pub wire_reversed: Vec<bool>,
    /// CSR offsets of length `faces + 1`: face `f`'s wire is
    /// `wire_edge[face_start[f] .. face_start[f + 1]]`.
    pub face_start: Vec<usize>,
}

impl<B: Backend> Brep<B> {
    /// An empty B-rep.
    pub fn new() -> Self {
        Brep {
            verts: Vec::new(),
            edges: Vec::new(),
            faces: Vec::new(),
        }
    }

    /// Append a vertex, returning its identity (its index in the vertex table).
    pub fn add_vertex(&mut self, v: Vertex<B>) -> usize {
        self.verts.push(v);
        self.verts.len() - 1
    }

    /// Append an edge between two existing vertex indices, returning its identity (its
    /// index in the edge table) — the handle callers reuse to share the edge.
    pub fn add_edge(&mut self, start: usize, end: usize, geom: EdgeGeom<B>) -> usize {
        self.edges.push(BEdge { start, end, geom });
        self.edges.len() - 1
    }

    /// Append a hole-free face on the given surface, bounded by `wire` (directed edge uses).
    /// Returns the face index. Convenience for [`add_face_with_holes`](Self::add_face_with_holes)
    /// with no interior wires.
    pub fn add_face(&mut self, surface: FaceSurface<B>, wire: Vec<HalfEdge>) -> usize {
        self.add_face_with_holes(surface, wire, Vec::new())
    }

    /// Append a face on the given surface, bounded by the outer `wire` with interior `holes`
    /// (each a directed loop wound opposite to `wire`). Returns the face index.
    pub fn add_face_with_holes(
        &mut self,
        surface: FaceSurface<B>,
        wire: Vec<HalfEdge>,
        holes: Vec<Vec<HalfEdge>>,
    ) -> usize {
        self.faces.push(Face {
            surface,
            wire,
            holes,
        });
        self.faces.len() - 1
    }

    /// Convenience: append a planar face bounded by `wire`.
    pub fn add_plane(&mut self, wire: Vec<HalfEdge>) -> usize {
        self.add_face(FaceSurface::Plane, wire)
    }

    /// The vertex table.
    pub fn verts(&self) -> &[Vertex<B>] {
        &self.verts
    }
    /// The edge table (index = identity).
    pub fn edges(&self) -> &[BEdge<B>] {
        &self.edges
    }
    /// The faces.
    pub fn faces(&self) -> &[Face<B>] {
        &self.faces
    }

    /// The number of faces each edge is used by, indexed by edge id — the pure
    /// combinatorial precursor to OCCT's edge→face incidence. An edge is *free* at
    /// incidence 1 (an open boundary), *manifold-shared* at 2, *non-manifold* at ≥ 3.
    pub fn edge_incidence(&self) -> Vec<usize> {
        let mut inc = vec![0usize; self.edges.len()];
        for face in &self.faces {
            for &(e, _) in face.wire.iter().chain(face.holes.iter().flatten()) {
                if let Some(c) = inc.get_mut(e) {
                    *c += 1;
                }
            }
        }
        inc
    }

    /// The number of **free edges** — used by exactly one face. On the certified-seam /
    /// honest-open export these are the honestly-open substrate boundary; the certified
    /// Π-seam must *not* be among them.
    pub fn free_edges(&self) -> usize {
        self.edge_incidence().iter().filter(|&&c| c == 1).count()
    }

    /// The number of **non-manifold edges** — used by three or more faces. Must be zero.
    pub fn nonmanifold_edges(&self) -> usize {
        self.edge_incidence().iter().filter(|&&c| c >= 3).count()
    }

    /// Structural integrity: every wire edge id is in range, and every edge's endpoint
    /// vertex ids are in range (plus every `LinearExtrusion` base id). A `false` return
    /// means a dangling index — a builder bug, not a geometry fault.
    pub fn indices_in_range(&self) -> bool {
        let nv = self.verts.len();
        let ne = self.edges.len();
        self.edges.iter().all(|e| e.start < nv && e.end < nv)
            && self.faces.iter().all(|f| {
                let base_ok = match &f.surface {
                    FaceSurface::Plane | FaceSurface::RationalPatch(_) => true,
                    FaceSurface::LinearExtrusion { base, .. } => *base < ne,
                };
                base_ok
                    && f.wire
                        .iter()
                        .chain(f.holes.iter().flatten())
                        .all(|&(e, _)| e < ne)
            })
    }

    /// Whether a single directed edge loop is **closed**: consecutive directed edges chain
    /// endpoint-to-startpoint (respecting each half-edge's `reversed` flag) all the way around.
    /// Returns `false` for an empty loop or an out-of-range edge id.
    fn loop_is_closed(&self, loop_: &[HalfEdge]) -> bool {
        if loop_.is_empty() {
            return false;
        }
        // The directed endpoints of half-edge (e, reversed).
        let ends = |&(e, rev): &HalfEdge| -> Option<(usize, usize)> {
            let ed = self.edges.get(e)?;
            Some(if rev {
                (ed.end, ed.start)
            } else {
                (ed.start, ed.end)
            })
        };
        let n = loop_.len();
        for i in 0..n {
            let Some((_, cur_end)) = ends(&loop_[i]) else {
                return false;
            };
            let Some((next_start, _)) = ends(&loop_[(i + 1) % n]) else {
                return false;
            };
            if cur_end != next_start {
                return false;
            }
        }
        true
    }

    /// Whether a face's **outer** wire is a closed loop (see [`loop_is_closed`](Self::loop_is_closed)).
    /// Returns `false` for an out-of-range face id.
    pub fn wire_is_closed(&self, face: usize) -> bool {
        match self.faces.get(face) {
            Some(f) => self.loop_is_closed(&f.wire),
            None => false,
        }
    }

    /// Whether a face's `hole_idx`-th interior hole wire is a closed loop. Returns `false` for an
    /// out-of-range face or hole id.
    pub fn hole_is_closed(&self, face: usize, hole_idx: usize) -> bool {
        match self.faces.get(face).and_then(|f| f.holes.get(hole_idx)) {
            Some(h) => self.loop_is_closed(h),
            None => false,
        }
    }

    /// Whether a face's outer wire **and every** interior hole wire are closed loops. Returns
    /// `false` for an out-of-range face id.
    pub fn all_loops_closed(&self, face: usize) -> bool {
        match self.faces.get(face) {
            Some(f) => {
                self.loop_is_closed(&f.wire) && f.holes.iter().all(|h| self.loop_is_closed(h))
            }
            None => false,
        }
    }

    /// Emit the flat index-array [`ShellCertificate`] for the trusted
    /// `certify_core::shell::closed_shell` checker: the vertex count, the edge endpoint
    /// tables, and the face wires in CSR form. This is the one point where the exact B-rep
    /// hands its *combinatorics* (no coordinates) to the TCB — the searcher/checker split.
    ///
    /// Scope: the certificate covers **outer** wires only. Interior [`holes`](Face::holes) are
    /// excluded — a face with holes is an honestly *open* sheet (its hole edges are free), not a
    /// closed solid, so it is outside `closed_shell`'s remit; a hole-free `Brep` (every
    /// `holes` empty) is certified exactly as before.
    pub fn to_shell_certificate(&self) -> ShellCertificate {
        let edge_start = self.edges.iter().map(|e| e.start).collect();
        let edge_end = self.edges.iter().map(|e| e.end).collect();
        let mut wire_edge = Vec::new();
        let mut wire_reversed = Vec::new();
        let mut face_start = Vec::with_capacity(self.faces.len() + 1);
        face_start.push(0);
        for f in &self.faces {
            for &(eid, reversed) in &f.wire {
                wire_edge.push(eid);
                wire_reversed.push(reversed);
            }
            face_start.push(wire_edge.len());
        }
        ShellCertificate {
            n_verts: self.verts.len(),
            edge_start,
            edge_end,
            wire_edge,
            wire_reversed,
            face_start,
        }
    }
}

impl<B: Backend> Default for Brep<B> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(v: i128) -> Surd<Bignum> {
        Surd::from_rat(Rat::from_i128(v))
    }

    /// A hand-built two-face shell sharing one edge by identity round-trips: index
    /// integrity holds, both wires close, the shared edge has incidence 2, and the four
    /// outer edges are free — the pure-IR precursor to the D3.2 2-incidence seam gate.
    #[test]
    fn two_faces_share_a_seam_edge_by_identity() {
        let mut b = Brep::<Bignum>::new();
        // A unit square split into two triangles across the diagonal v1–v2.
        let v0 = b.add_vertex([r(0), r(0), r(0)]);
        let v1 = b.add_vertex([r(1), r(0), r(0)]);
        let v2 = b.add_vertex([r(0), r(1), r(0)]);
        let v3 = b.add_vertex([r(1), r(1), r(0)]);
        let seam = b.add_edge(v1, v2, EdgeGeom::Line);
        let a0 = b.add_edge(v0, v1, EdgeGeom::Line);
        let a1 = b.add_edge(v2, v0, EdgeGeom::Line);
        let bb0 = b.add_edge(v1, v3, EdgeGeom::Line);
        let bb1 = b.add_edge(v3, v2, EdgeGeom::Line);
        let f0 = b.add_plane(vec![(a0, false), (seam, false), (a1, false)]);
        let f1 = b.add_plane(vec![(bb0, false), (bb1, false), (seam, true)]);

        assert!(b.indices_in_range());
        assert!(b.wire_is_closed(f0));
        assert!(b.wire_is_closed(f1));
        let inc = b.edge_incidence();
        assert_eq!(
            inc[seam], 2,
            "the seam is shared by both faces (by identity)"
        );
        assert_eq!(b.free_edges(), 4, "the four outer edges are open");
        assert_eq!(b.nonmanifold_edges(), 0);
    }

    /// A wire whose edges do not chain end-to-start is not closed.
    #[test]
    fn a_broken_wire_is_not_closed() {
        let mut b = Brep::<Bignum>::new();
        let v0 = b.add_vertex([r(0), r(0), r(0)]);
        let v1 = b.add_vertex([r(1), r(0), r(0)]);
        let v2 = b.add_vertex([r(0), r(1), r(0)]);
        let e0 = b.add_edge(v0, v1, EdgeGeom::Line);
        let e1 = b.add_edge(v2, v0, EdgeGeom::Line); // gap: v1 → v2 missing
        let f = b.add_plane(vec![(e0, false), (e1, false)]);
        assert!(!b.wire_is_closed(f));
    }

    /// A ruled face carries its base edge and an exact rational extrusion direction; the
    /// base id and wire ids are range-checked.
    #[test]
    fn a_ruled_face_names_its_base_and_direction() {
        let mut b = Brep::<Bignum>::new();
        let v0 = b.add_vertex([r(0), r(0), r(0)]);
        let v1 = b.add_vertex([r(0), r(0), r(1)]);
        let v2 = b.add_vertex([r(1), r(0), r(1)]);
        let v3 = b.add_vertex([r(1), r(0), r(0)]);
        let base = b.add_edge(v0, v1, EdgeGeom::Line);
        let top = b.add_edge(v1, v2, EdgeGeom::Line);
        let far = b.add_edge(v2, v3, EdgeGeom::Line);
        let ret = b.add_edge(v3, v0, EdgeGeom::Line);
        let f = b.add_face(
            FaceSurface::LinearExtrusion {
                base,
                dir: [Rat::from_i128(1), Rat::from_i128(0), Rat::from_i128(0)],
            },
            vec![(base, false), (top, false), (far, false), (ret, false)],
        );
        assert!(b.indices_in_range());
        assert!(b.wire_is_closed(f));
        assert!(matches!(
            b.faces()[f].surface,
            FaceSurface::LinearExtrusion { base: bb, .. } if bb == base
        ));
    }

    /// An out-of-range base id fails the range check (a builder bug is caught here, not
    /// deferred to the CAD kernel).
    #[test]
    fn a_dangling_base_id_is_out_of_range() {
        let mut b = Brep::<Bignum>::new();
        let v0 = b.add_vertex([r(0), r(0), r(0)]);
        let v1 = b.add_vertex([r(1), r(0), r(0)]);
        let e = b.add_edge(v0, v1, EdgeGeom::Line);
        b.add_face(
            FaceSurface::LinearExtrusion {
                base: 99, // no such edge
                dir: [Rat::from_i128(1), Rat::from_i128(0), Rat::from_i128(0)],
            },
            vec![(e, false)],
        );
        assert!(!b.indices_in_range());
    }

    /// A closed square loop of `Line` edges through `pts`, tagged forward.
    fn square_loop(b: &mut Brep<Bignum>, pts: &[[Surd<Bignum>; 3]; 4]) -> Vec<HalfEdge> {
        let v: Vec<usize> = pts.iter().map(|p| b.add_vertex(p.clone())).collect();
        (0..4)
            .map(|i| (b.add_edge(v[i], v[(i + 1) % 4], EdgeGeom::Line), false))
            .collect()
    }

    /// A plane face with one interior square hole: both loops close, they reference disjoint edge
    /// ids, and every edge is free (incidence 1) — the open-sheet holed-panel precursor to STEP-II.
    #[test]
    fn a_face_with_a_hole_closes_both_loops_and_frees_every_edge() {
        let mut b = Brep::<Bignum>::new();
        let outer = square_loop(
            &mut b,
            &[
                [r(0), r(0), r(0)],
                [r(4), r(0), r(0)],
                [r(4), r(4), r(0)],
                [r(0), r(4), r(0)],
            ],
        );
        let hole = square_loop(
            &mut b,
            &[
                [r(1), r(1), r(0)],
                [r(3), r(1), r(0)],
                [r(3), r(3), r(0)],
                [r(1), r(3), r(0)],
            ],
        );
        let outer_ids: Vec<usize> = outer.iter().map(|&(e, _)| e).collect();
        let hole_ids: Vec<usize> = hole.iter().map(|&(e, _)| e).collect();
        let f = b.add_face_with_holes(FaceSurface::Plane, outer, vec![hole]);

        assert!(b.indices_in_range());
        assert!(b.wire_is_closed(f), "outer wire closes");
        assert!(b.hole_is_closed(f, 0), "hole wire closes");
        assert!(b.all_loops_closed(f));
        assert_eq!(
            b.free_edges(),
            8,
            "4 outer + 4 hole edges, all free on an open sheet"
        );
        assert_eq!(b.nonmanifold_edges(), 0);
        assert!(
            hole_ids.iter().all(|h| !outer_ids.contains(h)),
            "the hole references edge ids disjoint from the outer wire (identity separation)"
        );
    }

    /// A hole loop whose edges do not chain end-to-start is detected as not closed.
    #[test]
    fn an_open_hole_wire_is_detected() {
        let mut b = Brep::<Bignum>::new();
        let outer = square_loop(
            &mut b,
            &[
                [r(0), r(0), r(0)],
                [r(4), r(0), r(0)],
                [r(4), r(4), r(0)],
                [r(0), r(4), r(0)],
            ],
        );
        // A broken hole: two disjoint edges that do not form a loop.
        let hv = [
            b.add_vertex([r(1), r(1), r(0)]),
            b.add_vertex([r(3), r(1), r(0)]),
            b.add_vertex([r(3), r(3), r(0)]),
        ];
        let h0 = b.add_edge(hv[0], hv[1], EdgeGeom::Line);
        let h1 = b.add_edge(hv[2], hv[0], EdgeGeom::Line); // gap: hv[1] → hv[2] missing
        let f = b.add_face_with_holes(
            FaceSurface::Plane,
            outer,
            vec![vec![(h0, false), (h1, false)]],
        );
        assert!(b.wire_is_closed(f), "outer still closes");
        assert!(
            !b.hole_is_closed(f, 0),
            "the broken hole loop is not closed"
        );
        assert!(!b.all_loops_closed(f));
    }

    /// `add_face` / `add_plane` produce a hole-free face (the delegation contract).
    #[test]
    fn add_face_keeps_holes_empty() {
        let mut b = Brep::<Bignum>::new();
        let v0 = b.add_vertex([r(0), r(0), r(0)]);
        let v1 = b.add_vertex([r(1), r(0), r(0)]);
        let v2 = b.add_vertex([r(0), r(1), r(0)]);
        let e0 = b.add_edge(v0, v1, EdgeGeom::Line);
        let e1 = b.add_edge(v1, v2, EdgeGeom::Line);
        let e2 = b.add_edge(v2, v0, EdgeGeom::Line);
        let f = b.add_plane(vec![(e0, false), (e1, false), (e2, false)]);
        assert!(b.faces()[f].holes.is_empty());
    }

    /// The shell certificate covers outer wires only: a holed face's CSR span equals its outer
    /// wire length (the documented open-sheet scope).
    #[test]
    fn to_shell_certificate_excludes_holes() {
        let mut b = Brep::<Bignum>::new();
        let outer = square_loop(
            &mut b,
            &[
                [r(0), r(0), r(0)],
                [r(4), r(0), r(0)],
                [r(4), r(4), r(0)],
                [r(0), r(4), r(0)],
            ],
        );
        let hole = square_loop(
            &mut b,
            &[
                [r(1), r(1), r(0)],
                [r(3), r(1), r(0)],
                [r(3), r(3), r(0)],
                [r(1), r(3), r(0)],
            ],
        );
        b.add_face_with_holes(FaceSurface::Plane, outer, vec![hole]);
        let cert = b.to_shell_certificate();
        // One face, its CSR span = the 4 outer edges (holes excluded).
        assert_eq!(cert.face_start, vec![0, 4]);
        assert_eq!(cert.wire_edge.len(), 4);
    }
}
