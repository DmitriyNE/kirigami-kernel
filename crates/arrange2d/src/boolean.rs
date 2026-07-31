//! The eight-step boolean (M3 slice 3d, spec §6:289 steps 2–8) — the general D24
//! boolean engine over the [`super::dcel`] arrangement. Seeds the unbounded cell
//! `(A,B) = (0,0)`, propagates the ℤ₂² operand bits across edges (∂F_A flips A,
//! ∂F_B flips B, a coincident edge flips both), self-checks the ℤ₂² cocycle
//! (every closed walk returns its bits), selects by a pluggable boolean
//! (△ = A⊕B, ∧, ∨), emits only separating edges (the three-way law), and takes
//! faces = π₀ of the selected cells along selected|selected edges.
//!
//! This is the untrusted **searcher**. The cocycle check is done here as the §6
//! step-4 self-diagnostic ([`CellLabeling::cocycle_ok`]); its *proof* — the pure
//! checker in `certify_core::arrange` over the flat certificate this module exposes
//! — is slice 3d.3.
//!
//! **Scope note (holes).** Faces are taken as the traced DCEL cycles: correct when
//! every region is bounded by a single cycle (the disk ∪/∩/△ corpus and the
//! Milestone-A operands). A selected region with an *unselected hole* (e.g. an
//! annulus) would need cycle→face nesting, deferred; the 3d.4 CGAL differential
//! (which handles holes) is the loud safety net that would catch it.

use certify_core::Verdict;
use geom::content::{CurveId, Edge};
use lattice::Backend;

use crate::dcel::{Dcel, SubEdge};

/// Which operand a source curve bounds (the two-operand ℤ₂² model).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OperandId {
    A,
    B,
}

/// The pluggable selection over the propagated `(A, B)` cell label (spec §6 step 6).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BoolOp {
    /// Symmetric difference `A ⊕ B` (the closure △).
    Xor,
    /// Intersection `A ∧ B`.
    And,
    /// Union `A ∨ B`.
    Or,
}

impl BoolOp {
    /// Apply the selection to a cell's `(A, B)` label.
    pub fn select(self, label: (bool, bool)) -> bool {
        let (a, b) = label;
        match self {
            BoolOp::Xor => a ^ b,
            BoolOp::And => a && b,
            BoolOp::Or => a || b,
        }
    }
}

/// One emitted output face: the separating boundary edges of a π₀ component of the
/// selected region (spec §6 step 8 — one face per component, never per cell).
pub struct Face<B: Backend> {
    pub boundary: Vec<Edge<B>>,
}

/// The emitted region: the connected components of the selected cells.
pub struct Region<B: Backend> {
    pub faces: Vec<Face<B>>,
}

/// The **flat certificate** of the cell labeling — a flattened, index-array view of
/// the DCEL bit propagation, exactly what the 3d.3 `certify_core::arrange` cocycle
/// checker (Kani-harnessable / Charon-extractable) consumes. `adj[k] = (cyc_a,
/// cyc_b, flip_a, flip_b)` is the k-th undirected edge: crossing it between its two
/// incident cells flips `A` iff `flip_a`, `B` iff `flip_b`.
pub struct CellLabeling {
    pub n_cycles: usize,
    pub labels: Vec<(bool, bool)>,
    pub adj: Vec<(usize, usize, bool, bool)>,
    pub seed: usize,
    pub cocycle_ok: bool,
}

/// The operand-boundary flips an edge carries: `A` flips iff an odd number of its
/// covering sources bound operand A, likewise `B` (a coincident edge carrying one
/// of each flips both — spec §6 step 4/5).
fn edge_flips<B: Backend>(
    se: &SubEdge<B>,
    operand_of: &impl Fn(CurveId) -> OperandId,
) -> (bool, bool) {
    let (mut ca, mut cb) = (0u32, 0u32);
    for (src, _orient) in &se.sources {
        match operand_of(*src) {
            OperandId::A => ca += 1,
            OperandId::B => cb += 1,
        }
    }
    (ca % 2 == 1, cb % 2 == 1)
}

/// The cycle bounding the unbounded face, seeded `(0,0)`: at the lexicographically
/// smallest vertex (leftmost-lowest, on the outer boundary) the unbounded face lies
/// to the left of the most-CCW outgoing half-edge (nothing is further left/below).
fn unbounded_cycle<B: Backend>(d: &Dcel<B>) -> usize {
    let vmin = (0..d.verts.len())
        .min_by(|&i, &j| d.verts[i].cmp(&d.verts[j]))
        .expect("non-empty arrangement");
    let o_max = d
        .halfedges
        .iter()
        .enumerate()
        .filter(|(_, he)| he.origin == vmin)
        .map(|(h, _)| h)
        .max_by(|&h1, &h2| crate::tangent::dir_cmp(&tangent(d, h1), &tangent(d, h2)))
        .expect("a boundary vertex has an outgoing half-edge");
    d.halfedges[o_max].cycle
}

fn tangent<B: Backend>(d: &Dcel<B>, h: usize) -> crate::tangent::Outgoing<B> {
    let he = &d.halfedges[h];
    crate::tangent::outgoing_tangent(&d.edges[he.edge].edge, he.dir)
}

/// Propagate the ℤ₂² cell labels over the DCEL and self-check the cocycle (spec §6
/// steps 2–4). Seeds the unbounded cell `(0,0)`, BFS-crosses edges flipping bits to
/// compute the labeling, then certifies consistency by flowing it through the pure,
/// Kani-proven `certify_core::arrange::cocycle_ok` (3d.3). Returns the flat
/// certificate (`labels`, `adj`, `seed`) that checker consumes.
pub fn label_cells<B: Backend>(
    d: &Dcel<B>,
    operand_of: &impl Fn(CurveId) -> OperandId,
) -> CellLabeling {
    // Per-edge incident cycles + flips.
    let adj: Vec<(usize, usize, bool, bool)> = (0..d.edges.len())
        .map(|k| {
            let cyc_a = d.halfedges[2 * k].cycle;
            let cyc_b = d.halfedges[2 * k + 1].cycle;
            let (fa, fb) = edge_flips(&d.edges[k], operand_of);
            (cyc_a, cyc_b, fa, fb)
        })
        .collect();

    let seed = unbounded_cycle(d);
    let mut labels: Vec<Option<(bool, bool)>> = vec![None; d.n_cycles];
    labels[seed] = Some((false, false));

    // BFS over the cell-adjacency graph (nodes = cycles, edges = DCEL edges) to
    // *compute* the labeling; consistency (the cocycle) is then certified below by
    // the pure `certify_core::arrange` checker, not asserted here.
    let mut queue = vec![seed];
    while let Some(c) = queue.pop() {
        let here = labels[c].unwrap();
        for &(ca, cb, fa, fb) in &adj {
            let other = if ca == c {
                Some(cb)
            } else if cb == c {
                Some(ca)
            } else {
                None
            };
            if let Some(o) = other {
                if labels[o].is_none() {
                    labels[o] = Some((here.0 ^ fa, here.1 ^ fb));
                    queue.push(o);
                }
            }
        }
    }
    let labels: Vec<(bool, bool)> = labels
        .into_iter()
        .map(|l| l.unwrap_or((false, false)))
        .collect();

    // The ℤ₂² cocycle self-diagnostic (spec §6 step 4): the searcher's computed
    // labeling flows through the *proven* pure checker (Kani-verified, 3d.3).
    let labels_u8: Vec<u8> = labels.iter().map(|&(a, b)| pack(a, b)).collect();
    let (ea, eb, ef) = flat_edges(&adj);
    let cocycle_ok = certify_core::arrange::cocycle_ok(d.n_cycles, &labels_u8, seed, &ea, &eb, &ef);

    CellLabeling {
        n_cycles: d.n_cycles,
        labels,
        adj,
        seed,
        cocycle_ok,
    }
}

/// Pack an `(A, B)` label into the checker's `u8` (bit 0 = A, bit 1 = B).
fn pack(a: bool, b: bool) -> u8 {
    (a as u8) | ((b as u8) << 1)
}

/// The flat edge certificate `certify_core::arrange::cocycle_ok` consumes:
/// `(edge_a, edge_b, edge_flip)`, `edge_flip` packed like a label.
fn flat_edges(adj: &[(usize, usize, bool, bool)]) -> (Vec<usize>, Vec<usize>, Vec<u8>) {
    let ea = adj.iter().map(|&(a, ..)| a).collect();
    let eb = adj.iter().map(|&(_, b, ..)| b).collect();
    let ef = adj.iter().map(|&(_, _, fa, fb)| pack(fa, fb)).collect();
    (ea, eb, ef)
}

/// Union-find over cells (for the π₀ quotient).
struct Uf {
    parent: Vec<usize>,
}
impl Uf {
    fn new(n: usize) -> Self {
        Uf {
            parent: (0..n).collect(),
        }
    }
    fn find(&mut self, x: usize) -> usize {
        let mut r = x;
        while self.parent[r] != r {
            r = self.parent[r];
        }
        let mut c = x;
        while self.parent[c] != c {
            let n = self.parent[c];
            self.parent[c] = r;
            c = n;
        }
        r
    }
    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            self.parent[ra] = rb;
        }
    }
}

/// The full eight-step boolean: build the DCEL, label the cells, select by `op`,
/// emit only separating edges, and quotient to π₀ faces (spec §6 steps 1–8).
/// Returns `Refuted` [`Verdict`] never (searcher); a cocycle failure is surfaced as
/// an `Unresolved`-free `Verified` whose labeling carries `cocycle_ok = false` for
/// the checker — but in a correct build the cocycle always closes.
pub fn ledge_dom<B: Backend>(
    edges: &[Edge<B>],
    operand_of: &impl Fn(CurveId) -> OperandId,
    op: BoolOp,
) -> Region<B> {
    let d = Dcel::build(edges);
    let cl = label_cells(&d, operand_of);
    let sel: Vec<bool> = cl.labels.iter().map(|&l| op.select(l)).collect();

    // π₀: union selected cells joined by a selected|selected edge (the merge edges).
    let mut uf = Uf::new(d.n_cycles);
    for &(ca, cb, _, _) in &cl.adj {
        if sel[ca] && sel[cb] {
            uf.union(ca, cb);
        }
    }

    // Emit only separating edges (selected|unselected), grouped by the π₀ component
    // of their selected side → one face per component.
    let mut roots: Vec<usize> = Vec::new();
    let mut faces: Vec<Face<B>> = Vec::new();
    for (k, &(ca, cb, _, _)) in cl.adj.iter().enumerate() {
        if sel[ca] == sel[cb] {
            continue; // selected|selected (merge, deleted) or unselected|unselected
        }
        let sel_cycle = if sel[ca] { ca } else { cb };
        let root = uf.find(sel_cycle);
        let fi = match roots.iter().position(|&r| r == root) {
            Some(i) => i,
            None => {
                roots.push(root);
                faces.push(Face {
                    boundary: Vec::new(),
                });
                roots.len() - 1
            }
        };
        faces[fi].boundary.push(d.edges[k].edge.clone());
    }

    Region { faces }
}

/// Convenience: the boolean as a `Verdict` carrying the labeling's cocycle verdict
/// (the searcher self-diagnostic). `Refuted` is [`core::convert::Infallible`].
pub fn ledge_dom_checked<B: Backend>(
    edges: &[Edge<B>],
    operand_of: &impl Fn(CurveId) -> OperandId,
    op: BoolOp,
) -> Verdict<Region<B>, core::convert::Infallible, ()> {
    let d = Dcel::build(edges);
    let cl = label_cells(&d, operand_of);
    if !cl.cocycle_ok {
        return Verdict::Unresolved(());
    }
    Verdict::Verified(ledge_dom(edges, operand_of, op))
}

#[cfg(test)]
mod tests {
    use super::*;
    use geom::content::{Circle, Curve, Orient};
    use lattice::{Bignum, Rat};

    type Q = Rat<Bignum>;

    fn circle_edges(cx: i128, cy: i128, r2: i128, src: u32) -> Vec<Edge<Bignum>> {
        crate::decompose::decompose(&Curve::Circle {
            circle: Circle {
                cx: Q::from_i128(cx),
                cy: Q::from_i128(cy),
                r2: Q::from_i128(r2),
            },
            orient: Orient::Ccw,
            source: CurveId(src),
        })
    }

    /// Source 0 → operand A, source 1 → operand B.
    fn ab(src: CurveId) -> OperandId {
        if src.0 == 0 {
            OperandId::A
        } else {
            OperandId::B
        }
    }

    fn two_disks() -> Vec<Edge<Bignum>> {
        // (0,0,25) and (8,0,25): overlap, meeting at (4,±3).
        let mut e = circle_edges(0, 0, 25, 0);
        e.extend(circle_edges(8, 0, 25, 1));
        e
    }

    #[test]
    fn cocycle_closes_on_two_disks() {
        let d = Dcel::build(&two_disks());
        let cl = label_cells(&d, &ab);
        assert!(
            cl.cocycle_ok,
            "ℤ₂² cocycle must close on a valid arrangement"
        );
        // four cells: outside (0,0), A-only lune (1,0), B-only lune (0,1), lens (1,1).
        let mut seen: Vec<(bool, bool)> = cl.labels.clone();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), 4, "the four ℤ₂² labels are all present");
    }

    #[test]
    fn two_disks_union_one_face() {
        let r = ledge_dom(&two_disks(), &ab, BoolOp::Or);
        assert_eq!(r.faces.len(), 1, "∪ of two overlapping disks is one face");
        assert_eq!(
            r.faces[0].boundary.len(),
            4,
            "bounded by the four outer arcs"
        );
    }

    #[test]
    fn two_disks_intersection_one_face() {
        let r = ledge_dom(&two_disks(), &ab, BoolOp::And);
        assert_eq!(r.faces.len(), 1, "∩ is the single lens");
        assert_eq!(
            r.faces[0].boundary.len(),
            4,
            "bounded by the four inner arcs"
        );
    }

    #[test]
    fn two_disks_xor_two_faces() {
        let r = ledge_dom(&two_disks(), &ab, BoolOp::Xor);
        assert_eq!(
            r.faces.len(),
            2,
            "△ is the two lunes (pinched at the crossings)"
        );
        for f in &r.faces {
            assert_eq!(f.boundary.len(), 4, "each lune: two outer + two inner arcs");
        }
    }

    /// Two *identical* disks (distinct sources → coincident edges flipping BOTH
    /// operands): △ vanishes completely (the clean-miter empty case, spec §6 step 7),
    /// while ∪ = ∩ = the disk.
    #[test]
    fn identical_disks_xor_empty() {
        let mut e = circle_edges(0, 0, 25, 0);
        e.extend(circle_edges(0, 0, 25, 1));
        let d = Dcel::build(&e);
        let cl = label_cells(&d, &ab);
        assert!(cl.cocycle_ok);
        // two cells only: outside (0,0), inside (1,1) — the coincident boundary flips both.
        let mut seen = cl.labels.clone();
        seen.sort();
        seen.dedup();
        assert_eq!(seen, vec![(false, false), (true, true)]);

        assert_eq!(
            ledge_dom(&e, &ab, BoolOp::Xor).faces.len(),
            0,
            "△ of identical operands is empty"
        );
        assert_eq!(
            ledge_dom(&e, &ab, BoolOp::Or).faces.len(),
            1,
            "∪ is the disk"
        );
        assert_eq!(
            ledge_dom(&e, &ab, BoolOp::And).faces.len(),
            1,
            "∩ is the disk"
        );
    }
}
