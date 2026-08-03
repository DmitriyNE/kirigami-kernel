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
//! **Scope note (regime handled).** Cells are the traced DCEL cycles, labeled by
//! **exact point-location** — the horizontal-slab decomposition of [`label_cells`]
//! (3e.1), which seeds every cell independently (not a single BFS). This is exact on
//! **transverse-crossing** overlaps, **identical/coincident** operands, **disjoint**
//! (disconnected) operands, and **nested** (annulus) operands — the cocycle closes on
//! all of them, so `ledge_dom` and [`ledge_dom_checked`] return the correct labeling.
//! Two follow-ups remain:
//! - **Face-with-holes nesting** (3e.1b): the emitted [`Face`] is still a flat edge
//!   bag, so an annulus `△` emits its outer and hole boundaries as two faces rather
//!   than one face with a counter-oriented hole. The labels/selection are correct; the
//!   *structural* nesting is pending.
//! - **tangency** (internal/external — the operands touch at a point): the cocycle
//!   closes (connected), but the emitted face count can still be **frame-dependent** —
//!   after a rotation the tangent point may land on the axis-aligned decomposition's
//!   x-extremum, changing the piece split. CAP-OUT-LINK (3e.2) is the frame-invariant
//!   net; until then the property/differential invariants stay scoped to the
//!   transverse regime (see `crosses_twice`).

use certify_core::Verdict;
use core::cmp::Ordering;
use geom::content::{Circle, CurveId, Edge, Half};
use lattice::{Backend, Rat, Surd};

use crate::dcel::{Dcel, SubEdge};
use crate::locate::{
    rational_above, rational_below, rational_between, ray_x_arc, ray_x_seg, winding_parity,
};

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

/// Propagate the ℤ₂² cell labels over the DCEL and self-check the cocycle (spec §6
/// steps 2–4). Labels every cell by exact point-location (3e.1), anchors the checker
/// at an unbounded `(0,0)` cell, then certifies consistency by flowing the labeling
/// through the pure, Kani-proven `certify_core::arrange::cocycle_ok` (3d.3). Returns
/// the flat certificate (`labels`, `adj`, `seed`) that checker consumes.
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

    // Label every cycle by exact point-location (the horizontal-slab decomposition,
    // 3e.1). This replaces the single-seed BFS, which could not reach cycles in a
    // disconnected cell-adjacency graph — one geometric face (the unbounded one
    // especially, and every hole) is bounded by several traced cycles that share no
    // edge, so BFS left them mislabeled and the cocycle failed on disjoint/nested
    // inputs. Point-location seeds every cycle independently and consistently; the
    // cocycle check below now certifies that consistency instead of masking a gap.
    let labels = label_all_cycles(d, operand_of);

    // The checker's anchor is any unbounded `(A,B) = (0,0)` cell — the region outside
    // every operand always exists. (The old `unbounded_cycle` searched the leftmost
    // vertex's rotation, but the most-CCW outgoing half-edge there bounds an *interior*
    // cell, not the unbounded one; 3d masked that by force-seeding it `(0,0)`.)
    let seed = labels
        .iter()
        .position(|&l| l == (false, false))
        .unwrap_or(0);

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

/// The distinct circles carrying the sub-edges — the arcs whose centre height and
/// apexes (`cy`, `cy ± √r2`) are structural critical heights for the slab decomposition.
fn circles_of<B: Backend>(d: &Dcel<B>) -> Vec<Circle<B>> {
    let mut cs: Vec<Circle<B>> = Vec::new();
    for se in &d.edges {
        if let Edge::Arc(a) = &se.edge {
            let dup = cs.iter().any(|c| {
                c.cx.cmp(&a.circle.cx) == Ordering::Equal
                    && c.cy.cmp(&a.circle.cy) == Ordering::Equal
                    && c.r2.cmp(&a.circle.r2) == Ordering::Equal
            });
            if !dup {
                cs.push(a.circle.clone());
            }
        }
    }
    cs
}

/// The sorted, deduped **critical heights** where the arrangement's horizontal
/// cross-section changes: every vertex `y`, plus each circle's centre height `cy`
/// and its two apexes `cy ± √r2` (an arc piece starts/stops being crossed there).
/// A generic ray strictly between two consecutive criticals sees a constant set of
/// simple x-intervals (a horizontal slab), so one such ray per gap labels every cell.
fn critical_ys<B: Backend>(d: &Dcel<B>) -> Vec<Surd<B>> {
    let mut ys: Vec<Surd<B>> = d.verts.iter().map(|p| p.y.clone()).collect();
    for c in circles_of(d) {
        ys.push(Surd::from_rat(c.cy.clone()));
        ys.push(Surd::new(c.cy.clone(), Rat::from_i128(1), c.r2.clone()));
        ys.push(Surd::new(c.cy.clone(), Rat::from_i128(-1), c.r2.clone()));
    }
    ys.sort();
    ys.dedup();
    ys
}

/// The sub-edges bounding operand `want` (any covering source maps to it) — the
/// boundary curve set whose rightward ray-cast parity is that operand's enclosure bit.
fn operand_edges<B: Backend>(
    d: &Dcel<B>,
    operand_of: &impl Fn(CurveId) -> OperandId,
    want: OperandId,
) -> Vec<Edge<B>> {
    d.edges
        .iter()
        .filter(|se| se.sources.iter().any(|(s, _)| operand_of(*s) == want))
        .map(|se| se.edge.clone())
        .collect()
}

/// One crossing of a slab ray with a sub-edge: the crossing x and the cycle ids of
/// the cells immediately **below** (`down`, the left face of the downward-traversing
/// half-edge) and **above** (`up`, its twin) it. Left-of-directed-half-edge is the
/// `+y` side iff the half-edge runs in `+x`, so the cell directly above a crossing is
/// the left face of the `+x`-traversing (`t_y` from `−x` ⇒ upward) half-edge, and the
/// cell directly below (equivalently, the cell to `+x` along the ray) is the left face
/// of the downward one. That is the whole cell↔cycle map, tangent-free for segments
/// and needing only the arc branch (`sign(x − cx)`) for arcs.
struct Crossing<B: Backend> {
    x: Surd<B>,
    down_cycle: usize,
    up_cycle: usize,
}

/// The downward half-edge (`t_y < 0`) of a straddling **segment** edge `k`: the one
/// traversing from the higher-`y` endpoint to the lower.
fn seg_down_he<B: Backend>(d: &Dcel<B>, k: usize, se: &SubEdge<B>) -> usize {
    if d.verts[se.va].y.cmp(&d.verts[se.vb].y) == Ordering::Greater {
        2 * k // va (higher) → vb (lower)
    } else {
        2 * k + 1
    }
}

/// The downward half-edge (`t_y < 0`) of an **arc** edge `k` at crossing `x`. The arc
/// tangent's y-sign is `x_dir · dy/dx`; `dy/dx` on the Upper half is `−sign(x − cx)`,
/// on the Lower `+sign(x − cx)`. The `+x` half-edge (`x_dir > 0`) runs from the
/// smaller-x endpoint to the larger; the downward one has `x_dir · (dy/dx) < 0`.
fn arc_down_he<B: Backend>(
    d: &Dcel<B>,
    k: usize,
    se: &SubEdge<B>,
    half: Half,
    x: &Surd<B>,
    cx: &Rat<B>,
) -> usize {
    let branch_gt = x.cmp(&Surd::from_rat(cx.clone())) == Ordering::Greater; // x > cx
    let dydx_pos = matches!(
        (half, branch_gt),
        (Half::Upper, false) | (Half::Lower, true)
    );
    let xdir_pos = d.verts[se.vb].x.cmp(&d.verts[se.va].x) == Ordering::Greater; // vb.x > va.x
    // t_y(2k) = x_dir · dy/dx; negative iff the two signs differ.
    if xdir_pos != dydx_pos {
        2 * k
    } else {
        2 * k + 1
    }
}

/// Label **every** cycle by exact point-location over the horizontal-slab
/// decomposition (3e.1): for each gap between consecutive critical heights, cast one
/// generic rational ray, and for each cell the ray crosses set its `(A, B)` label
/// from the even-odd `winding_parity` of a rational interior sample against the A- and
/// B-boundary edges. Robust to disconnected / nested arrangements (unlike the BFS it
/// replaces); the cocycle check downstream certifies the result.
fn label_all_cycles<B: Backend>(
    d: &Dcel<B>,
    operand_of: &impl Fn(CurveId) -> OperandId,
) -> Vec<(bool, bool)> {
    let a_edges = operand_edges(d, operand_of, OperandId::A);
    let b_edges = operand_edges(d, operand_of, OperandId::B);
    let mut labels: Vec<Option<(bool, bool)>> = vec![None; d.n_cycles];

    let crit = critical_ys(d);
    if crit.is_empty() {
        return vec![(false, false); d.n_cycles];
    }
    // A generic rational height per gap: below the lowest, between each pair, above
    // the highest. Each is strictly between (or beyond) criticals, so it avoids every
    // vertex y and every circle cy — the ray-cast genericity precondition.
    let mut band_ys: Vec<Rat<B>> = Vec::with_capacity(crit.len() + 1);
    band_ys.push(rational_below(&crit[0]));
    for w in crit.windows(2) {
        band_ys.push(rational_between(&w[0], &w[1]));
    }
    band_ys.push(rational_above(&crit[crit.len() - 1]));

    let label_at = |xm: &Rat<B>, y0: &Rat<B>| -> (bool, bool) {
        (
            winding_parity(xm, y0, &a_edges),
            winding_parity(xm, y0, &b_edges),
        )
    };

    for y0 in &band_ys {
        // All crossings of this ray with the sub-edges.
        let mut xs: Vec<Crossing<B>> = Vec::new();
        for (k, se) in d.edges.iter().enumerate() {
            match &se.edge {
                Edge::Seg(s) => {
                    if let Some(xr) = ray_x_seg(y0, s) {
                        let down = seg_down_he(d, k, se);
                        xs.push(Crossing {
                            x: Surd::from_rat(xr),
                            down_cycle: d.halfedges[down].cycle,
                            up_cycle: d.halfedges[down ^ 1].cycle,
                        });
                    }
                }
                Edge::Arc(a) => {
                    for x in ray_x_arc(y0, a) {
                        let down = arc_down_he(d, k, se, a.half, &x, &a.circle.cx);
                        xs.push(Crossing {
                            x,
                            down_cycle: d.halfedges[down].cycle,
                            up_cycle: d.halfedges[down ^ 1].cycle,
                        });
                    }
                }
            }
        }
        if xs.is_empty() {
            continue; // a slab above/below the whole arrangement — the unbounded cell
        }
        xs.sort_by(|p, q| p.x.cmp(&q.x));
        let m = xs.len();

        // Leftmost cell: −x of the first crossing = the upward half-edge's face.
        let xm = rational_below(&xs[0].x);
        labels[xs[0].up_cycle] = Some(label_at(&xm, y0));
        // Cell between consecutive crossings: +x of the left one = the downward face.
        for j in 0..m - 1 {
            let xm = rational_between(&xs[j].x, &xs[j + 1].x);
            labels[xs[j].down_cycle] = Some(label_at(&xm, y0));
        }
        // Rightmost cell: +x of the last crossing.
        let xm = rational_above(&xs[m - 1].x);
        labels[xs[m - 1].down_cycle] = Some(label_at(&xm, y0));
    }

    labels
        .into_iter()
        .map(|l| l.unwrap_or((false, false)))
        .collect()
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

    // --- properties: cocycle closure + metamorphic invariants over disk pairs ---

    use crate::testgen::{rigid, rigid_circle, scale_circle};
    use proptest::prelude::*;

    fn disk(cx: i128, cy: i128, r2: i128) -> Circle<Bignum> {
        Circle {
            cx: Q::from_i128(cx),
            cy: Q::from_i128(cy),
            r2: Q::from_i128(r2),
        }
    }
    /// Do the two circles (squared radii `r1`, `r2`) cross in **two transverse
    /// points** — `|r1−r2| < dist < r1+r2`, i.e. `(dist² − r1 − r2)² < 4·r1·r2`?
    /// This is the connected, non-degenerate regime the boolean handles robustly.
    /// Tangency (1 point) and nested/disjoint (0 points) are the deferred degenerate
    /// cases (see the module scope note) — and internal tangency in particular is not
    /// frame-independent, because after a rotation the tangent point can land on the
    /// axis-aligned decomposition's x-extremum where a piece splits.
    fn crosses_twice(x1: i128, y1: i128, r1: i128, x2: i128, y2: i128, r2: i128) -> bool {
        let d = (x1 - x2).pow(2) + (y1 - y2).pow(2);
        (d - r1 - r2).pow(2) < 4 * r1 * r2
    }
    fn two_disk_edges(c1: &Circle<Bignum>, c2: &Circle<Bignum>) -> Vec<Edge<Bignum>> {
        let mut e = crate::decompose::decompose(&Curve::Circle {
            circle: c1.clone(),
            orient: Orient::Ccw,
            source: CurveId(0),
        });
        e.extend(crate::decompose::decompose(&Curve::Circle {
            circle: c2.clone(),
            orient: Orient::Ccw,
            source: CurveId(1),
        }));
        e
    }
    /// `(△, ∩, ∪)` output face counts.
    fn face_counts(edges: &[Edge<Bignum>]) -> (usize, usize, usize) {
        (
            ledge_dom(edges, &ab, BoolOp::Xor).faces.len(),
            ledge_dom(edges, &ab, BoolOp::And).faces.len(),
            ledge_dom(edges, &ab, BoolOp::Or).faces.len(),
        )
    }

    /// The ℤ₂² cocycle closes (the proven `certify_core::arrange::cocycle_ok`
    /// accepts) on **connected** two-operand arrangements — boundaries that meet, so
    /// the cell-adjacency graph is connected and bit propagation reaches every cell.
    #[test]
    fn cocycle_closes_on_connected_configs() {
        // (overlap, internal tangency, external tangency, identical, overlap).
        let configs = [
            (disk(0, 0, 25), disk(8, 0, 25)),
            (disk(0, 0, 4), disk(1, 0, 1)),
            (disk(0, 0, 4), disk(4, 0, 4)),
            (disk(0, 0, 25), disk(0, 0, 25)),
            (disk(0, 0, 25), disk(6, 0, 16)),
        ];
        for (c1, c2) in &configs {
            let e = two_disk_edges(c1, c2);
            let d = Dcel::build(&e);
            assert!(
                label_cells(&d, &ab).cocycle_ok,
                "cocycle must close on the connected config ({:?},{:?})",
                c1.cx,
                c2.cx
            );
        }
    }

    /// **Disjoint operands are now handled exactly** (3e.1): point-location seeds each
    /// connected component of the cell-adjacency graph independently, so the two
    /// separate disks' cells are labeled correctly and the cocycle closes — where 3d
    /// could only self-detect the disconnection as `Unresolved`. Two separate unit
    /// disks (centres 3 apart): outside `(0,0)`, disk-A `(1,0)`, disk-B `(0,1)`; no
    /// `(1,1)` overlap cell.
    #[test]
    fn disjoint_operands_now_correct() {
        let e = two_disk_edges(&disk(0, 0, 1), &disk(0, 3, 1));
        let d = Dcel::build(&e);
        let cl = label_cells(&d, &ab);
        assert!(
            cl.cocycle_ok,
            "disjoint arrangement now labels consistently"
        );
        let mut seen = cl.labels.clone();
        seen.sort();
        seen.dedup();
        assert_eq!(
            seen,
            vec![(false, false), (false, true), (true, false)],
            "outside, disk-B, disk-A — no overlap cell"
        );
        // ∪ = two disks, ∩ = empty, △ = two disks (symmetric difference of disjoint).
        assert!(matches!(
            ledge_dom_checked(&e, &ab, BoolOp::Or),
            Verdict::Verified(_)
        ));
        assert_eq!(
            ledge_dom(&e, &ab, BoolOp::Or).faces.len(),
            2,
            "∪ = two disks"
        );
        assert_eq!(ledge_dom(&e, &ab, BoolOp::And).faces.len(), 0, "∩ = empty");
        assert_eq!(
            ledge_dom(&e, &ab, BoolOp::Xor).faces.len(),
            2,
            "△ = two disks"
        );
    }

    /// **Nested operands (annulus) are now handled exactly** (3e.1): concentric disks,
    /// the inner strictly inside the outer. The three cells — outside `(0,0)`, the
    /// annulus between them `(1,0)` (in A = outer, not B = inner), and the inner disk
    /// `(1,1)` — label consistently. ∩ = the inner disk, △ = the annulus, ∪ = the
    /// outer disk.
    #[test]
    fn nested_operands_now_correct() {
        // A = outer (source 0, r²=9), B = inner (source 1, r²=1), concentric.
        let e = two_disk_edges(&disk(0, 0, 9), &disk(0, 0, 1));
        let d = Dcel::build(&e);
        let cl = label_cells(&d, &ab);
        assert!(cl.cocycle_ok, "nested arrangement now labels consistently");
        let mut seen = cl.labels.clone();
        seen.sort();
        seen.dedup();
        assert_eq!(
            seen,
            vec![(false, false), (true, false), (true, true)],
            "outside, annulus (A only), inner (A∧B)"
        );
        assert_eq!(
            ledge_dom(&e, &ab, BoolOp::And).faces.len(),
            1,
            "∩ = inner disk"
        );
        assert_eq!(
            ledge_dom(&e, &ab, BoolOp::Or).faces.len(),
            1,
            "∪ = outer disk"
        );
        // △ = the annulus: one face with a hole (checked structurally in 3e.1b).
        assert!(matches!(
            ledge_dom_checked(&e, &ab, BoolOp::Xor),
            Verdict::Verified(_)
        ));
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(48))]

        /// The boolean's output face count (△/∩/∪) is invariant under a rational
        /// rigid motion — the whole DCEL + eight-step pipeline is frame-independent.
        #[test]
        fn boolean_face_count_rigid_invariant(
            x1 in -3i128..=3, y1 in -3i128..=3, r1 in 1i128..=6,
            x2 in -3i128..=3, y2 in -3i128..=3, r2 in 1i128..=6,
            u in -3i128..=3, v in -3i128..=3, tx in -4i128..=4, ty in -4i128..=4,
        ) {
            prop_assume!(u != 0 || v != 0);
            prop_assume!(crosses_twice(x1, y1, r1, x2, y2, r2));
            let (c1, c2) = (disk(x1, y1, r1), disk(x2, y2, r2));
            let m = rigid(u, v, tx, ty);
            let e0 = two_disk_edges(&c1, &c2);
            let e1 = two_disk_edges(&rigid_circle(&c1, &m), &rigid_circle(&c2, &m));
            prop_assert_eq!(face_counts(&e0), face_counts(&e1));
        }

        /// Invariant under lattice rescaling `p ↦ k·p` (`k > 0`): scaling preserves
        /// the arrangement's combinatorics, hence the output face counts.
        #[test]
        fn boolean_face_count_scale_invariant(
            x1 in -3i128..=3, y1 in -3i128..=3, r1 in 1i128..=6,
            x2 in -3i128..=3, y2 in -3i128..=3, r2 in 1i128..=6,
            k in 1i128..=5,
        ) {
            prop_assume!(crosses_twice(x1, y1, r1, x2, y2, r2));
            let (c1, c2) = (disk(x1, y1, r1), disk(x2, y2, r2));
            let kk = Q::from_i128(k);
            let e0 = two_disk_edges(&c1, &c2);
            let e1 = two_disk_edges(&scale_circle(&c1, &kk), &scale_circle(&c2, &kk));
            prop_assert_eq!(face_counts(&e0), face_counts(&e1));
        }
    }
}
