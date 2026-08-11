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
//! 2. **Loops well-formed** — every boundary loop is a non-empty cycle whose consecutive
//!    half-edges chain end→start (cyclically) and never immediately backtrack along one
//!    edge (the combinatorial analogue of `Brep::wire_is_closed`).
//! 3. **∂² = 0 (oriented)** — every edge is used **exactly twice: once forward, once
//!    reversed**. Incidence 1 is a free (open) edge, ≥ 3 is non-manifold, and two uses in
//!    the *same* direction is a non-orientable seam (a Klein-bottle-style identification) —
//!    all three are refused, so acceptance means "no boundary, manifold edges, orientable".
//! 4. **Vertex link a single cycle** — around each vertex the incident darts, walked by the
//!    rotation-system map `around = rev_dart ∘ next_in_loop`, form **one** orbit covering
//!    all of them. This is the manifold-*vertex* condition: two cones sharing an apex pass
//!    1–3 (each edge is still used once each way) yet split into two orbits here, so this is
//!    the check that separates a true 2-manifold from a vertex-pinched pseudomanifold.
//!
//! # Faces with holes (genus > 0)
//!
//! [`closed_shell_holed`] is the general entry point: a face is given as one *or more*
//! boundary loops (an outer wire plus zero or more interior hole wires), so the input adds a
//! second CSR level (faces → loops → half-edges) and check 2 / the check-4 rotation run **per
//! loop**. The `∂² = 0` census (check 3) never saw face shape and is unchanged. Declaring two
//! loops one *annular* face — rather than two disk faces — is exactly "replace two disks by a
//! tube" = drilling one handle: it preserves closed-orientable-manifoldness and only raises the
//! genus, and manifoldness never depended on the loop→face grouping (the checks read only local
//! dart data). That each face's loops actually bound an orientable patch is the per-face
//! *realizability* the CAD oracle owns (`BRepCheck`) — the **same** "oracle ∧ audit" split a
//! disk face already relies on (spec §8.2), not a new axis of trust. [`closed_shell`] is the
//! disk-face special case (one loop per face), kept as a thin wrapper.
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
//!     Verdict::Verified(ClosedShell { verts: 4, edges: 6, faces: 4, loops: 4 }),
//! );
//! ```

use alloc::vec::Vec;

use crate::verdict::Verdict;

/// The evidence a [`Verified`](Verdict::Verified) closed-shell certificate carries: the
/// shell's element counts. Holding one *is* the proof that the complex is a closed oriented
/// 2-manifold. `loops` counts boundary loops across all faces — it equals `faces` for a
/// disk-face shell (one loop per face) and exceeds it by one per interior hole, so
/// `loops − faces` is the total number of holes drilled (each raising the genus by one).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClosedShell {
    /// Number of vertices.
    pub verts: usize,
    /// Number of edges.
    pub edges: usize,
    /// Number of faces.
    pub faces: usize,
    /// Number of boundary loops across all faces (`≥ faces`; the excess is the hole count).
    pub loops: usize,
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

/// Decide whether a shell whose faces may carry **interior hole loops** describes a **closed
/// oriented 2-manifold** — the genus-`g` generalization of [`closed_shell`] (see the
/// [module docs](self#faces-with-holes-genus--0)).
///
/// Inputs are flat index arrays (no coordinates). `edge_start`/`edge_end` are the parallel
/// endpoint tables of the `n_edges` edges. `wire_edge`/`wire_reversed` are the parallel
/// half-edge tables (`edge id`, traversed-reversed?) of **all loops of all faces** concatenated.
/// Two CSR levels index them:
///
/// - `loop_start` — length `n_loops + 1`: loop `ℓ`'s half-edges are
///   `wire_edge[loop_start[ℓ] .. loop_start[ℓ + 1]]` (`loop_start[0] == 0`,
///   `loop_start[n_loops] == wire_edge.len()`).
/// - `face_start` — length `n_faces + 1`, indexing **loops**: face `f` owns loops
///   `face_start[f] .. face_start[f + 1]` (`face_start[0] == 0`, `face_start[n_faces] == n_loops`).
///
/// A hole-free face is just a face with one loop; [`closed_shell`] is that special case.
/// Returns [`Verified`](Verdict::Verified) with the element counts (including the loop count),
/// or [`Refuted`](Verdict::Refuted) naming the leftmost failing check. Total — it never returns
/// [`Unresolved`](Verdict::Unresolved) (a combinatorial fact is always decided).
pub fn closed_shell_holed(
    n_verts: usize,
    edge_start: &[usize],
    edge_end: &[usize],
    wire_edge: &[usize],
    wire_reversed: &[bool],
    loop_start: &[usize],
    face_start: &[usize],
) -> Verdict<ClosedShell, ClosedShellFault, ()> {
    let n_edges = edge_start.len();
    let n_wire = wire_edge.len();

    // ---- Check 1: shape / range integrity ----
    if edge_end.len() != n_edges
        || wire_reversed.len() != n_wire
        || loop_start.is_empty()
        || face_start.is_empty()
    {
        return Verdict::Refuted(ClosedShellFault::Shape);
    }
    let n_loops = loop_start.len() - 1;
    let n_faces = face_start.len() - 1;
    // A non-empty shell: an empty complex is not a solid boundary.
    if n_verts == 0 || n_edges == 0 || n_loops == 0 || n_faces == 0 {
        return Verdict::Refuted(ClosedShellFault::Shape);
    }
    // Loop CSR: anchored at 0, ending at the wire length, non-decreasing.
    if loop_start[0] != 0 || loop_start[n_loops] != n_wire {
        return Verdict::Refuted(ClosedShellFault::Shape);
    }
    let mut l = 0;
    while l < n_loops {
        if loop_start[l] > loop_start[l + 1] {
            return Verdict::Refuted(ClosedShellFault::Shape);
        }
        l += 1;
    }
    // Face CSR: anchored at 0, ending at the loop count, non-decreasing.
    if face_start[0] != 0 || face_start[n_faces] != n_loops {
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

    // ---- Check 2: every loop is a well-formed closed cycle (per loop; faults named by face) ----
    let mut f = 0;
    while f < n_faces {
        let mut l = face_start[f];
        while l < face_start[f + 1] {
            let lo = loop_start[l];
            let hi = loop_start[l + 1];
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
            l += 1;
        }
        f += 1;
    }

    // ---- Check 3: ∂² = 0 oriented edge census (each edge: exactly one fwd + one rev) ----
    // Per edge, tally forward/reverse uses and remember the (unique, once accepted) position
    // of each so we can pair reverse darts in check 4. `n_wire` is the "none" sentinel. This
    // is over *all* darts and never saw the loop/face nesting — unchanged from the disk case.
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
        rev_dart[i] = if wire_reversed[i] {
            fwd_pos[e]
        } else {
            rev_pos[e]
        };
        i += 1;
    }
    // The next half-edge in the same **loop** (cyclic). A face's outer and hole loops are
    // separate cycles here — exactly the "drill a handle" identification — but the rotation
    // walk below only ever reads local dart data, so a pinch still splits the link regardless.
    let mut next_in_loop: Vec<usize> = Vec::new();
    next_in_loop.resize(n_wire, n_wire);
    let mut l = 0;
    while l < n_loops {
        let lo = loop_start[l];
        let hi = loop_start[l + 1];
        let mut k = lo;
        while k < hi {
            next_in_loop[k] = if k + 1 < hi { k + 1 } else { lo };
            k += 1;
        }
        l += 1;
    }
    // Incoming darts at each vertex: `around = rev_dart ∘ next_in_loop` maps a dart ending
    // at v to another dart ending at v (loops close ⇒ the next dart starts at v; its reverse
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
        let mut cur = rev_dart[next_in_loop[start]];
        let mut steps = 1usize;
        // Bounded by the dart count — a runaway (impossible once checks 1–3 pass) is capped.
        while cur != start && steps <= n_wire {
            cur = rev_dart[next_in_loop[cur]];
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
        loops: n_loops,
    })
}

/// Decide whether a **disk-face** shell's combinatorics describe a **closed oriented
/// 2-manifold** (spec §8: the assembly-scale CAP-OUT-LINK). See the [module docs](self) for
/// the four checks.
///
/// Inputs are flat index arrays (no coordinates): `edge_start`/`edge_end` are the parallel
/// endpoint tables of the `n_edges` edges; `wire_edge`/`wire_reversed` are the parallel
/// half-edge tables (`edge id`, traversed-reversed?) of all face wires concatenated; and
/// `face_start` is the CSR offset table of length `n_faces + 1`, so face `f`'s wire is
/// `wire_edge[face_start[f] .. face_start[f + 1]]` (`face_start[0] == 0`,
/// `face_start[n_faces] == wire_edge.len()`).
///
/// This is exactly [`closed_shell_holed`] with **one loop per face** (the identity face→loop
/// nesting), so the disk case is a literal special case of the holed one. Returns
/// [`Verified`](Verdict::Verified) with the element counts (`loops == faces` here), or
/// [`Refuted`](Verdict::Refuted) naming the leftmost failing check.
pub fn closed_shell(
    n_verts: usize,
    edge_start: &[usize],
    edge_end: &[usize],
    wire_edge: &[usize],
    wire_reversed: &[bool],
    face_start: &[usize],
) -> Verdict<ClosedShell, ClosedShellFault, ()> {
    // Each face is exactly one loop: the per-face wire CSR *is* the loop CSR, and the face→loop
    // map is the identity 0,1,…,n_faces. (A malformed empty `face_start` falls through to the
    // holed checker's shape check.)
    if face_start.is_empty() {
        return Verdict::Refuted(ClosedShellFault::Shape);
    }
    let n_faces = face_start.len() - 1;
    // Identity face→loop nesting: face f owns loop f only (one loop per face).
    let identity: Vec<usize> = (0..=n_faces).collect();
    closed_shell_holed(
        n_verts,
        edge_start,
        edge_end,
        wire_edge,
        wire_reversed,
        face_start,
        &identity,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    /// A shell certificate as flat arrays: `(n_verts, edge_start, edge_end, wire_edge,
    /// wire_reversed, face_start)`.
    type ShellArrays = (
        usize,
        Vec<usize>,
        Vec<usize>,
        Vec<usize>,
        Vec<bool>,
        Vec<usize>,
    );

    /// A holed shell certificate: [`ShellArrays`] plus the loop-level CSR — `(n_verts, edge_start,
    /// edge_end, wire_edge, wire_reversed, loop_start, face_start)`.
    type HoledShellArrays = (
        usize,
        Vec<usize>,
        Vec<usize>,
        Vec<usize>,
        Vec<bool>,
        Vec<usize>,
        Vec<usize>,
    );

    /// A closed, consistently-oriented cube: 8 vertices, 12 edges, 6 quad faces. The
    /// canonical closed 2-manifold the export slab reduces to.
    fn cube() -> ShellArrays {
        // Vertices 0..7: bottom z=0 face 0,1,2,3 (CCW seen from below), top z=1 face 4,5,6,7
        // directly above (4 over 0, 5 over 1, 6 over 2, 7 over 3).
        // Edges: bottom ring 0..3, top ring 4..7, verticals 8..11.
        let edge_start = vec![
            0, 1, 2, 3, /* top */ 4, 5, 6, 7, /* vert */ 0, 1, 2, 3,
        ];
        let edge_end = vec![
            1, 2, 3, 0, /* top */ 5, 6, 7, 4, /* vert */ 4, 5, 6, 7,
        ];
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
        (
            8,
            edge_start,
            edge_end,
            wire_edge,
            wire_reversed,
            face_start,
        )
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
                faces: 6,
                loops: 6,
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

    /// A square slab with a square hole drilled through it — a **genus-1** solid whose
    /// boundary is a torus. Its two `z = const` sheets are **annular faces** (an outer wire +
    /// an interior hole wire, 2 loops each); four outer walls and four tube walls close it.
    /// 16 vertices, 24 edges, 10 faces, 12 loops; every edge is used once each way and every
    /// vertex link (including the three-face inner-rim corners) is a single cycle.
    ///
    /// Vertices: outer bottom 0–3 / top 4–7 (7 above 3, …); inner-rim bottom 8–11 / top 12–15.
    fn holed_box() -> HoledShellArrays {
        // Edges e0..e23: outer bottom ring 0..3, outer top ring 4..7, outer verticals 8..11,
        // inner bottom ring 12..15, inner top ring 16..19, inner verticals 20..23.
        let edge_start = vec![
            0, 1, 2, 3, /* outer top */ 4, 5, 6, 7, /* outer vert */ 0, 1, 2, 3,
            /* inner bot */ 8, 9, 10, 11, /* inner top */ 12, 13, 14, 15,
            /* inner vert */ 8, 9, 10, 11,
        ];
        let edge_end = vec![
            1, 2, 3, 0, /* outer top */ 5, 6, 7, 4, /* outer vert */ 4, 5, 6, 7,
            /* inner bot */ 9, 10, 11, 8, /* inner top */ 13, 14, 15, 12,
            /* inner vert */ 12, 13, 14, 15,
        ];
        // Loops, four darts each (edge id, reversed), in face order.
        let loops: [[(usize, bool); 4]; 12] = [
            // F0 bottom (normal −z): outer 0→3→2→1, hole 8→9→10→11.
            [(3, true), (2, true), (1, true), (0, true)],
            [(12, false), (13, false), (14, false), (15, false)],
            // F1 top (normal +z): outer 4→5→6→7, hole 12→15→14→13.
            [(4, false), (5, false), (6, false), (7, false)],
            [(19, true), (18, true), (17, true), (16, true)],
            // Outer walls W0..W3 (outward normal).
            [(0, false), (9, false), (4, true), (8, true)],
            [(1, false), (10, false), (5, true), (9, true)],
            [(2, false), (11, false), (6, true), (10, true)],
            [(3, false), (8, false), (7, true), (11, true)],
            // Tube walls T0..T3 (normal toward the hole axis — opposite an outer wall).
            [(20, false), (16, false), (21, true), (12, true)],
            [(21, false), (17, false), (22, true), (13, true)],
            [(22, false), (18, false), (23, true), (14, true)],
            [(23, false), (19, false), (20, true), (15, true)],
        ];
        let mut wire_edge = Vec::new();
        let mut wire_reversed = Vec::new();
        let mut loop_start = vec![0usize];
        for lp in &loops {
            for &(e, r) in lp {
                wire_edge.push(e);
                wire_reversed.push(r);
            }
            loop_start.push(wire_edge.len());
        }
        // F0,F1 own two loops each; the eight walls own one loop each.
        let face_start = vec![0usize, 2, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        (
            16,
            edge_start,
            edge_end,
            wire_edge,
            wire_reversed,
            loop_start,
            face_start,
        )
    }

    #[test]
    fn a_square_slab_with_a_through_hole_is_a_closed_torus() {
        let (nv, es, ee, we, wr, ls, fs) = holed_box();
        assert_eq!(
            closed_shell_holed(nv, &es, &ee, &we, &wr, &ls, &fs),
            Verdict::Verified(ClosedShell {
                verts: 16,
                edges: 24,
                faces: 10,
                loops: 12,
            }),
            "the drilled slab is a genus-1 closed 2-manifold (loops − faces = 2 = one hole × 2 sheets)"
        );
    }

    #[test]
    fn an_unpaired_hole_edge_is_a_census_fault() {
        // Flip F0's hole loop (reverse order and direction): it stays a closed cycle
        // (11→10→9→8→11), so check 2 passes — but now the four inner-bottom edges are used
        // *reversed* by both the bottom sheet and their tube walls (two same-direction uses),
        // which check 3 refuses. The hole must be wound opposite its tube to pair.
        let (nv, es, ee, mut we, mut wr, ls, fs) = holed_box();
        // F0's hole loop is loop 1 → wire positions 4..8.
        we[4..8].reverse();
        wr[4..8].reverse();
        for r in wr[4..8].iter_mut() {
            *r = !*r;
        }
        assert!(matches!(
            closed_shell_holed(nv, &es, &ee, &we, &wr, &ls, &fs),
            Verdict::Refuted(ClosedShellFault::EdgeCensus { .. })
        ));
    }

    #[test]
    fn a_broken_hole_loop_is_open() {
        // Corrupt one dart of F0's hole loop so it no longer chains end→start.
        let (nv, es, ee, mut we, wr, ls, fs) = holed_box();
        // Position 4 is the hole loop's first dart (e12, 8→9); repoint it to an edge that
        // does not start where the previous dart ended.
        we[4] = 14; // e14 = (10,11), breaking the 11→8→9 chain
        assert!(matches!(
            closed_shell_holed(nv, &es, &ee, &we, &wr, &ls, &fs),
            Verdict::Refuted(ClosedShellFault::OpenWire { face: 0 })
        ));
    }

    #[test]
    fn the_disk_wrapper_is_the_one_loop_per_face_special_case() {
        // `closed_shell` must agree with `closed_shell_holed` fed the identity face→loop
        // nesting — the wrapper is faithful, so every disk-face caller rides the general path.
        let (nv, es, ee, we, wr, fs) = cube();
        let identity: Vec<usize> = (0..=fs.len() - 1).collect();
        assert_eq!(
            closed_shell(nv, &es, &ee, &we, &wr, &fs),
            closed_shell_holed(nv, &es, &ee, &we, &wr, &fs, &identity),
        );
    }
}
