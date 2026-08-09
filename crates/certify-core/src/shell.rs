//! The **closed-2-manifold shell checker** (Milestone D slice 4 — atlas assembly).
//!
//! The assembly-scale analogue of [`crate::arrange`]'s per-vertex CAP-OUT-LINK: given the
//! *combinatorics* of a shell — a vertex count, an edge table (endpoint-vertex-id pairs),
//! and faces as closed wires of half-edges `(edge_id, reversed)` — [`closed_shell`] decides
//! whether it is a **closed oriented 2-manifold** (the boundary of a solid). It turns
//! whole-solid closedness from an *oracle verdict* (OpenCASCADE `BRepCheck`) into an
//! **earned certificate** proven inside the TCB — "oracle ∧ audit, never oracle-instead"
//! (spec §8.2). The untrusted searcher that builds the shell (`export::brep`) flows its
//! index-array certificate through here; this module never sees a coordinate.
//!
//! A shell is a closed oriented 2-manifold iff all four hold (checked in order, first
//! failure wins):
//!
//! 1. **Shape / range** — the arrays are well-shaped, every id is in range, and every edge
//!    joins two *distinct* vertices.
//! 2. **Wires well-formed** — every face wire is a non-empty loop whose consecutive
//!    half-edges chain end→start (cyclically) and never immediately backtrack along one
//!    edge (the combinatorial analogue of `Brep::wire_is_closed`).
//! 3. **∂² = 0 (oriented)** — every edge is used **exactly twice: once forward, once
//!    reversed**. Incidence 1 is a free (open) edge, ≥ 3 is non-manifold, and two uses in
//!    the *same* direction is a non-orientable seam (a Klein-bottle-style identification) —
//!    all three are refused, so acceptance means "no boundary, manifold edges, orientable".
//! 4. **Vertex link a single cycle** — around each vertex the incident darts, walked by the
//!    rotation-system map `around = rev_dart ∘ next_in_face`, form **one** orbit covering
//!    all of them. This is the manifold-*vertex* condition: two cones sharing an apex pass
//!    1–3 (each edge is still used once each way) yet split into two orbits here, so this is
//!    the check that separates a true 2-manifold from a vertex-pinched pseudomanifold.
//!
//! Pure, total, panic-free, `no_std`, index-arrays-only — the [`crate::arrange`] mold. The
//! bounded soundness proof is the `closed_shell_sound` Kani harness; the unbounded
//! "∂² = 0 ∧ single-cycle links ⇒ closed 2-manifold" theorem is the tracked Lean frontier
//! (the `CertifyCheck/CapOut.lean` assembly analogue), *not* claimed here.
//!
//! # Example
//!
//! ```
//! use certify_core::shell::{closed_shell, ClosedShell};
//! use certify_core::verdict::Verdict;
//!
//! // A tetrahedron: 4 vertices, 6 edges, 4 triangular faces, consistently oriented
//! // (each edge traversed once each way). Edges e0..e5 = (0,1)(1,2)(2,0)(0,3)(1,3)(2,3).
//! let edge_start = [0usize, 1, 2, 0, 1, 2];
//! let edge_end = [1usize, 2, 0, 3, 3, 3];
//! // Faces (0,2,1) (0,1,3) (0,3,2) (1,2,3), each as three half-edges (edge_id, reversed):
//! let wire_edge = [2usize, 1, 0, /**/ 0, 4, 3, /**/ 3, 5, 2, /**/ 1, 5, 4];
//! let wire_reversed = [
//!     true, true, true, /**/ false, false, true, /**/ false, true, false, /**/ false, false, true,
//! ];
//! let face_start = [0usize, 3, 6, 9, 12]; // CSR: face f owns wire[face_start[f]..face_start[f+1]]
//!
//! assert_eq!(
//!     closed_shell(4, &edge_start, &edge_end, &wire_edge, &wire_reversed, &face_start),
//!     Verdict::Verified(ClosedShell { verts: 4, edges: 6, faces: 4 }),
//! );
//! ```

use alloc::vec::Vec;

use crate::verdict::Verdict;

/// The evidence a [`Verified`](Verdict::Verified) closed-shell certificate carries: the
/// shell's element counts. Holding one *is* the proof that the `(verts, edges, faces)`
/// complex is a closed oriented 2-manifold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClosedShell {
    /// Number of vertices.
    pub verts: usize,
    /// Number of edges.
    pub edges: usize,
    /// Number of faces.
    pub faces: usize,
}

/// Which closed-shell condition a shell violated (the leftmost failing check).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClosedShellFault {
    /// Malformed arrays, an out-of-range id, or a degenerate (self-loop) edge — check 1.
    Shape,
    /// Face `face`'s wire is not a well-formed closed loop: empty, does not chain
    /// end→start, or immediately backtracks along one edge — check 2.
    OpenWire {
        /// Index of the offending face.
        face: usize,
    },
    /// Edge `edge` is not used exactly once forward and once reversed — a free edge
    /// (open boundary), a non-manifold edge (≥ 3 uses), or a non-orientable seam (two
    /// same-direction uses) — check 3.
    EdgeCensus {
        /// Index of the offending edge.
        edge: usize,
    },
    /// Vertex `vertex`'s link is not a single cycle: a pinch (≥ 2 fans meeting at the
    /// vertex) or a stray vertex with no incident face — check 4.
    VertexLink {
        /// Index of the offending vertex.
        vertex: usize,
    },
}

/// The directed endpoints `(from, to)` of wire position `i` — its half-edge's edge, taken
/// forward or (if `reversed`) backward. Assumes shape validation has passed, so `i` and the
/// edge id are in range.
fn dart_ends(
    edge_start: &[usize],
    edge_end: &[usize],
    wire_edge: &[usize],
    wire_reversed: &[bool],
    i: usize,
) -> (usize, usize) {
    let e = wire_edge[i];
    if wire_reversed[i] {
        (edge_end[e], edge_start[e])
    } else {
        (edge_start[e], edge_end[e])
    }
}

/// Decide whether a shell's combinatorics describe a **closed oriented 2-manifold** (spec
/// §8: the assembly-scale CAP-OUT-LINK). See the [module docs](self) for the four checks.
///
/// Inputs are flat index arrays (no coordinates): `edge_start`/`edge_end` are the parallel
/// endpoint tables of the `n_edges` edges; `wire_edge`/`wire_reversed` are the parallel
/// half-edge tables (`edge id`, traversed-reversed?) of all face wires concatenated; and
/// `face_start` is the CSR offset table of length `n_faces + 1`, so face `f`'s wire is
/// `wire_edge[face_start[f] .. face_start[f + 1]]` (`face_start[0] == 0`,
/// `face_start[n_faces] == wire_edge.len()`).
///
/// Returns [`Verified`](Verdict::Verified) with the element counts, or
/// [`Refuted`](Verdict::Refuted) naming the leftmost failing check. Total — it never returns
/// [`Unresolved`](Verdict::Unresolved) (a combinatorial fact is always decided).
pub fn closed_shell(
    n_verts: usize,
    edge_start: &[usize],
    edge_end: &[usize],
    wire_edge: &[usize],
    wire_reversed: &[bool],
    face_start: &[usize],
) -> Verdict<ClosedShell, ClosedShellFault, ()> {
    let n_edges = edge_start.len();
    let n_wire = wire_edge.len();

    // ---- Check 1: shape / range integrity ----
    if edge_end.len() != n_edges || wire_reversed.len() != n_wire || face_start.is_empty() {
        return Verdict::Refuted(ClosedShellFault::Shape);
    }
    let n_faces = face_start.len() - 1;
    // A non-empty shell: an empty complex is not a solid boundary.
    if n_verts == 0 || n_edges == 0 || n_faces == 0 {
        return Verdict::Refuted(ClosedShellFault::Shape);
    }
    // CSR offsets: anchored at 0, ending at the wire length, non-decreasing.
    if face_start[0] != 0 || face_start[n_faces] != n_wire {
        return Verdict::Refuted(ClosedShellFault::Shape);
    }
    let mut f = 0;
    while f < n_faces {
        if face_start[f] > face_start[f + 1] {
            return Verdict::Refuted(ClosedShellFault::Shape);
        }
        f += 1;
    }
    // Edge endpoints in range and distinct (a self-loop edge is degenerate).
    let mut e = 0;
    while e < n_edges {
        if edge_start[e] >= n_verts || edge_end[e] >= n_verts || edge_start[e] == edge_end[e] {
            return Verdict::Refuted(ClosedShellFault::Shape);
        }
        e += 1;
    }
    // Wire edge ids in range.
    let mut i = 0;
    while i < n_wire {
        if wire_edge[i] >= n_edges {
            return Verdict::Refuted(ClosedShellFault::Shape);
        }
        i += 1;
    }

    // ---- Check 2: every wire is a well-formed closed loop ----
    let mut f = 0;
    while f < n_faces {
        let lo = face_start[f];
        let hi = face_start[f + 1];
        if lo == hi {
            return Verdict::Refuted(ClosedShellFault::OpenWire { face: f });
        }
        let mut k = lo;
        let mut bad = false;
        while k < hi {
            let next = if k + 1 < hi { k + 1 } else { lo };
            let (_, t_k) = dart_ends(edge_start, edge_end, wire_edge, wire_reversed, k);
            let (s_next, _) = dart_ends(edge_start, edge_end, wire_edge, wire_reversed, next);
            // Chains end→start, and does not immediately backtrack along the same edge
            // (a degenerate spike that would forge a manifold vertex link).
            if t_k != s_next || wire_edge[k] == wire_edge[next] {
                bad = true;
            }
            k += 1;
        }
        if bad {
            return Verdict::Refuted(ClosedShellFault::OpenWire { face: f });
        }
        f += 1;
    }

    // ---- Check 3: ∂² = 0 oriented edge census (each edge: exactly one fwd + one rev) ----
    // Per edge, tally forward/reverse uses and remember the (unique, once accepted) position
    // of each so we can pair reverse darts in check 4. `n_wire` is the "none" sentinel.
    let mut fwd_count: Vec<usize> = Vec::new();
    let mut rev_count: Vec<usize> = Vec::new();
    let mut fwd_pos: Vec<usize> = Vec::new();
    let mut rev_pos: Vec<usize> = Vec::new();
    fwd_count.resize(n_edges, 0);
    rev_count.resize(n_edges, 0);
    fwd_pos.resize(n_edges, n_wire);
    rev_pos.resize(n_edges, n_wire);
    let mut i = 0;
    while i < n_wire {
        let e = wire_edge[i];
        if wire_reversed[i] {
            rev_count[e] += 1;
            rev_pos[e] = i;
        } else {
            fwd_count[e] += 1;
            fwd_pos[e] = i;
        }
        i += 1;
    }
    let mut e = 0;
    while e < n_edges {
        if fwd_count[e] != 1 || rev_count[e] != 1 {
            return Verdict::Refuted(ClosedShellFault::EdgeCensus { edge: e });
        }
        e += 1;
    }

    // ---- Check 4: every vertex link is a single cycle ----
    // The reverse dart of position i (the same edge traversed the other way — unique now).
    let mut rev_dart: Vec<usize> = Vec::new();
    rev_dart.resize(n_wire, n_wire);
    let mut i = 0;
    while i < n_wire {
        let e = wire_edge[i];
        rev_dart[i] = if wire_reversed[i] { fwd_pos[e] } else { rev_pos[e] };
        i += 1;
    }
    // The next half-edge in the same face (cyclic).
    let mut next_in_face: Vec<usize> = Vec::new();
    next_in_face.resize(n_wire, n_wire);
    let mut f = 0;
    while f < n_faces {
        let lo = face_start[f];
        let hi = face_start[f + 1];
        let mut k = lo;
        while k < hi {
            next_in_face[k] = if k + 1 < hi { k + 1 } else { lo };
            k += 1;
        }
        f += 1;
    }
    // Incoming darts at each vertex: `around = rev_dart ∘ next_in_face` maps a dart ending
    // at v to another dart ending at v (wires close ⇒ the next dart starts at v; its reverse
    // ends at v). The vertex is manifold iff those darts are one `around`-orbit.
    let mut deg_in: Vec<usize> = Vec::new();
    let mut some_incoming: Vec<usize> = Vec::new();
    deg_in.resize(n_verts, 0);
    some_incoming.resize(n_verts, n_wire);
    let mut i = 0;
    while i < n_wire {
        let (_, t_i) = dart_ends(edge_start, edge_end, wire_edge, wire_reversed, i);
        deg_in[t_i] += 1;
        some_incoming[t_i] = i;
        i += 1;
    }
    let mut v = 0;
    while v < n_verts {
        if deg_in[v] == 0 {
            // A stray vertex incident to no face is not a manifold point.
            return Verdict::Refuted(ClosedShellFault::VertexLink { vertex: v });
        }
        // Walk the link from one incoming dart; a single cycle visits exactly deg_in[v].
        let start = some_incoming[v];
        let mut cur = rev_dart[next_in_face[start]];
        let mut steps = 1usize;
        // Bounded by the dart count — a runaway (impossible once checks 1–3 pass) is capped.
        while cur != start && steps <= n_wire {
            cur = rev_dart[next_in_face[cur]];
            steps += 1;
        }
        if cur != start || steps != deg_in[v] {
            return Verdict::Refuted(ClosedShellFault::VertexLink { vertex: v });
        }
        v += 1;
    }

    Verdict::Verified(ClosedShell {
        verts: n_verts,
        edges: n_edges,
        faces: n_faces,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    /// A shell certificate as flat arrays: `(n_verts, edge_start, edge_end, wire_edge,
    /// wire_reversed, face_start)`.
    type ShellArrays = (usize, Vec<usize>, Vec<usize>, Vec<usize>, Vec<bool>, Vec<usize>);

    /// A closed, consistently-oriented cube: 8 vertices, 12 edges, 6 quad faces. The
    /// canonical closed 2-manifold the export slab reduces to.
    fn cube() -> ShellArrays {
        // Vertices 0..7: bottom z=0 face 0,1,2,3 (CCW seen from below), top z=1 face 4,5,6,7
        // directly above (4 over 0, 5 over 1, 6 over 2, 7 over 3).
        // Edges: bottom ring 0..3, top ring 4..7, verticals 8..11.
        let edge_start = vec![0, 1, 2, 3, /* top */ 4, 5, 6, 7, /* vert */ 0, 1, 2, 3];
        let edge_end = vec![1, 2, 3, 0, /* top */ 5, 6, 7, 4, /* vert */ 4, 5, 6, 7];
        // Each face is a 4-edge wire; build it so every edge is traversed once each way.
        // bottom (0,3,2,1): outward normal −z. top (4,5,6,7): outward +z.
        // sides: (0,1,5,4) (1,2,6,5) (2,3,7,6) (3,0,4,7).
        // Encode each directed step as (edge_id, reversed) against the tables above.
        // bottom 0→3→2→1→0: e3 rev (3→0 rev = 0... wait build explicitly:
        //   0→3 = e3 reversed (e3 = 3→0), 3→2 = e2 reversed (e2 = 2→3), 2→1 = e1 rev, 1→0 = e0 rev
        let mut wire_edge = vec![3usize, 2, 1, 0];
        let mut wire_reversed = vec![true, true, true, true];
        let mut face_start = vec![0usize, 4];
        // top 4→5→6→7→4: e4,e5,e6,e7 all forward
        wire_edge.extend_from_slice(&[4, 5, 6, 7]);
        wire_reversed.extend_from_slice(&[false, false, false, false]);
        face_start.push(8);
        // side (0→1→5→4→0): e0 fwd, e9 fwd(1→5), e4 rev(4→5 → 5→4), e8 rev(0→4 → 4→0)
        wire_edge.extend_from_slice(&[0, 9, 4, 8]);
        wire_reversed.extend_from_slice(&[false, false, true, true]);
        face_start.push(12);
        // side (1→2→6→5→1): e1 fwd, e10 fwd(2→6), e5 rev(5→6→6→5), e9 rev(1→5→5→1)
        wire_edge.extend_from_slice(&[1, 10, 5, 9]);
        wire_reversed.extend_from_slice(&[false, false, true, true]);
        face_start.push(16);
        // side (2→3→7→6→2): e2 fwd, e11 fwd(3→7), e6 rev, e10 rev
        wire_edge.extend_from_slice(&[2, 11, 6, 10]);
        wire_reversed.extend_from_slice(&[false, false, true, true]);
        face_start.push(20);
        // side (3→0→4→7→3): e3 fwd, e8 fwd(0→4), e7 rev(7→4→4→7), e11 rev(3→7→7→3)
        wire_edge.extend_from_slice(&[3, 8, 7, 11]);
        wire_reversed.extend_from_slice(&[false, false, true, true]);
        face_start.push(24);
        (8, edge_start, edge_end, wire_edge, wire_reversed, face_start)
    }

    fn run(
        nv: usize,
        es: &[usize],
        ee: &[usize],
        we: &[usize],
        wr: &[bool],
        fs: &[usize],
    ) -> Verdict<ClosedShell, ClosedShellFault, ()> {
        closed_shell(nv, es, ee, we, wr, fs)
    }

    #[test]
    fn cube_is_a_closed_2_manifold() {
        let (nv, es, ee, we, wr, fs) = cube();
        assert_eq!(
            run(nv, &es, &ee, &we, &wr, &fs),
            Verdict::Verified(ClosedShell {
                verts: 8,
                edges: 12,
                faces: 6
            })
        );
    }

    #[test]
    fn dropping_a_face_opens_the_shell() {
        // Remove the bottom face: its four edges now have only one use each → EdgeCensus.
        let (nv, es, ee, we, wr, fs) = cube();
        // Faces are [0..4,4..8,...]; drop face 0 by shifting the CSR + wire past it.
        let we2 = we[4..].to_vec();
        let wr2 = wr[4..].to_vec();
        let fs2: Vec<usize> = fs[1..].iter().map(|o| o - 4).collect();
        assert!(matches!(
            run(nv, &es, &ee, &we2, &wr2, &fs2),
            Verdict::Refuted(ClosedShellFault::EdgeCensus { .. })
        ));
    }

    #[test]
    fn a_third_face_on_an_edge_is_non_manifold() {
        // Add a spurious extra face reusing bottom-ring edges → those edges get a 3rd use.
        let (nv, es, ee, mut we, mut wr, mut fs) = cube();
        we.extend_from_slice(&[0, 1, 2, 3]);
        wr.extend_from_slice(&[false, false, false, false]);
        fs.push(28);
        assert!(matches!(
            run(nv, &es, &ee, &we, &wr, &fs),
            Verdict::Refuted(ClosedShellFault::EdgeCensus { .. })
        ));
    }

    #[test]
    fn a_flipped_face_breaks_orientation() {
        // Reverse the top face's wire: its edges are now traversed the *same* way as their
        // partners on the side faces → two same-direction uses → EdgeCensus (non-orientable).
        let (nv, es, ee, mut we, mut wr, fs) = cube();
        // Top face occupies wire positions 4..8.
        we[4..8].reverse();
        for r in wr[4..8].iter_mut() {
            *r = !*r;
        }
        assert!(matches!(
            run(nv, &es, &ee, &we, &wr, &fs),
            Verdict::Refuted(ClosedShellFault::EdgeCensus { .. })
        ));
    }

    #[test]
    fn two_tetrahedra_sharing_one_vertex_is_a_pinch() {
        // Tetra A on vertices 0..3, tetra B on vertices 0,4,5,6 — sharing only vertex 0.
        // Each tetra alone is closed; every edge is used once each way (check 3 passes),
        // but vertex 0's link splits into two 3-cycles → VertexLink.
        // Tetra template (same orientation as the doctest), remapped per tetra.
        let tetra = |a: usize, b: usize, c: usize, d: usize| {
            // edges (a,b)(b,c)(c,a)(a,d)(b,d)(c,d); faces (a,c,b)(a,b,d)(a,d,c)(b,c,d)
            let es = vec![a, b, c, a, b, c];
            let ee = vec![b, c, a, d, d, d];
            let we = vec![2usize, 1, 0, 0, 4, 3, 3, 5, 2, 1, 5, 4];
            let wr = vec![
                true, true, true, false, false, true, false, true, false, false, false, true,
            ];
            (es, ee, we, wr)
        };
        let (mut es, mut ee, mut we, mut wr) = tetra(0, 1, 2, 3);
        let (esb, eeb, web, wrb) = tetra(0, 4, 5, 6);
        let base_e = es.len(); // 6 edges in tetra A; tetra B's edges shift by 6
        es.extend_from_slice(&esb);
        ee.extend_from_slice(&eeb);
        we.extend(web.iter().map(|e| e + base_e));
        wr.extend_from_slice(&wrb);
        // 8 faces total, 3 half-edges each.
        let fs: Vec<usize> = (0..=8).map(|f| f * 3).collect();
        assert!(matches!(
            run(7, &es, &ee, &we, &wr, &fs),
            Verdict::Refuted(ClosedShellFault::VertexLink { vertex: 0 })
        ));
    }

    #[test]
    fn a_broken_wire_is_open() {
        // A single square whose last step does not return to the start: not closed.
        let es = vec![0usize, 1, 2, 3];
        let ee = vec![1usize, 2, 3, 0];
        // 0→1, 1→2, 2→3, and then a bogus 0→1 again instead of 3→0.
        let we = vec![0usize, 1, 2, 0];
        let wr = vec![false, false, false, false];
        let fs = vec![0usize, 4];
        assert!(matches!(
            run(4, &es, &ee, &we, &wr, &fs),
            Verdict::Refuted(ClosedShellFault::OpenWire { face: 0 })
        ));
    }

    #[test]
    fn out_of_range_ids_are_shape_faults() {
        let es = vec![0usize, 1];
        let ee = vec![1usize, 2]; // vertex 2 ≥ n_verts = 2
        let we = vec![0usize, 1];
        let wr = vec![false, false];
        let fs = vec![0usize, 2];
        assert_eq!(
            run(2, &es, &ee, &we, &wr, &fs),
            Verdict::Refuted(ClosedShellFault::Shape)
        );
        // A self-loop edge is degenerate.
        assert_eq!(
            run(3, &[0usize], &[0usize], &[0usize], &[false], &[0usize, 1]),
            Verdict::Refuted(ClosedShellFault::Shape)
        );
    }
}
