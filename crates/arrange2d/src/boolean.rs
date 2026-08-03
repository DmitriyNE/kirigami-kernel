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
//! **Certified vs plain entry.** [`ledge_dom`] emits a [`Region`] unconditionally (fast,
//! trusted). [`ledge_dom_certified`] is the real CAP-OUT gate (spec §8.5): it computes
//! the labeling **once** and runs *every* proven checker over the **emitted** region —
//! substrate-link, `cocycle_ok`, per-vertex `link_iso_ok`, and the separating↔boundary
//! bijection — returning a real [`CapOutFault`] `Refuted` on any defect (so a searcher
//! bug is loud, not silent) plus the CAP-OUT-LINK `V_∂`/pinch classification.
//!
//! **Scope note (regime handled).** Cells are the traced DCEL cycles, labeled by
//! **exact point-location** — the horizontal-slab decomposition of [`slab_locate`]
//! (3e.1), which seeds every cell independently (not a single BFS). The boolean is now
//! exact and **frame-invariant across the full regime**: transverse-crossing overlaps,
//! identical/coincident, disjoint, nested (annulus), and **tangency** (internal /
//! external). The cocycle closes on all of them; [`ledge_dom`] emits [`Face`]s carrying
//! an outer loop plus counter-oriented holes (an annulus `△` is one face with one
//! hole), and its face counts are invariant under rational rigid motion + rescaling
//! over the whole regime — the boundary-loop [`emit_region`] (3e.1b) removed the
//! tangency frame-dependence 3d had (the crescent traces as one face regardless of
//! where the decomposition splits). Pinch points (a △ touching itself at a crossing or
//! tangent point) are classified frame-invariantly by CAP-OUT-LINK ([`has_pinch`] /
//! [`link_classes`], 3e.2, over the Kani-proven `certify_core::arrange::classify_link`):
//! spec §6 "π₀ keeps them separate, CAP-OUT-LINK rejects the vertex".

use certify_core::Verdict;
use certify_core::arrange::{LinkClass, classify_link, link_iso_ok};
use core::cmp::Ordering;
use geom::content::{Circle, CurveId, Edge, Half};
use lattice::{Backend, Rat, Surd};

use crate::dcel::{Dcel, SubEdge};
use crate::locate::{
    rational_above, rational_below, rational_between, ray_x_arc, ray_x_seg, winding_parity,
};

/// An `(A, B)` ℤ₂² cell label.
type Label = (bool, bool);
/// A rational sample point `(x, y)` — an interior witness of a cell (3e point-location).
type Pt<B> = (Rat<B>, Rat<B>);

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

/// One emitted output face (spec §6 step 8 — one face per π₀ component of the selected
/// region, never per cell): its **outer** boundary loop (CCW) and its **holes** (CW,
/// counter-oriented inner loops — the unselected regions strictly enclosed by it).
/// Together an outer + holes describe a `General_polygon_with_holes` (the CGAL oracle).
pub struct Face<B: Backend> {
    pub outer: Vec<Edge<B>>,
    pub holes: Vec<Vec<Edge<B>>>,
}

/// The emitted region: the connected components of the selected cells.
pub struct Region<B: Backend> {
    pub faces: Vec<Face<B>>,
}

/// A **kernel-defect** the CAP-OUT certificate refutes on: the searcher produced a
/// region that fails an internal-consistency clause a correct build always satisfies
/// (spec §8.5 — "absence is the silent failure"). These are `Refuted`, not `Unresolved`:
/// in a correct build they never fire, so a fire is a real bug, not an unhandled input.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CapOutFault {
    /// The DCEL overlay is malformed (twin pairing / dangling half-edge — spec §6
    /// substrate-link, [`Dcel::substrate_link_ok`]).
    SubstrateLink,
    /// The ℤ₂² labeling is not a valid cochain (a frustrated cycle ⇒ a mis-paired
    /// twin / dropped event — the Kani-proven `cocycle_ok` rejected it).
    Cocycle,
    /// `Link_emitted(v) ≇ Link_geometric(v)`: the stored rotation ≠ the azimuth sort at
    /// vertex `v` (the Kani-proven `link_iso_ok` rejected it — `link_rotation` is wrong).
    Link { vertex: usize },
    /// The `{separating edges} ↔ {emitted boundary edges}` completeness bijection failed
    /// (emission dropped or duplicated a boundary edge — spec §8.5 CAP-OUT completeness).
    Bijection,
}

/// A **certified** boolean output (spec §8.5 CAP-OUT): the emitted [`Region`] together
/// with the CAP-OUT-LINK classification of its arrangement vertices — `v_boundary` = the
/// manifold shell vertices `V_∂`, `pinches` = the non-manifold pinch points (valid, but
/// excluded from `V_∂`: spec "π₀ keeps them separate, CAP-OUT-LINK rejects the vertex").
pub struct CapOut<B: Backend> {
    pub region: Region<B>,
    pub v_boundary: Vec<usize>,
    pub pinches: Vec<usize>,
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

/// Is `y0` a **generic** ray height — strictly avoiding every arrangement vertex `y`
/// and every circle centre `cy` (the `winding_parity` genericity precondition, `locate`)?
/// A slab band height is generic by construction *iff* [`critical_ys`] is complete; this
/// is the self-check that makes an incomplete critical set (a dropped vertex / circle
/// upstream) a **detected** fault rather than a silent-wrong label. See the
/// `debug_assert!` in [`slab_locate`] and `slab_heights_generic` (proptest).
fn generic_height<B: Backend>(d: &Dcel<B>, y0: &Rat<B>) -> bool {
    let y0s = Surd::from_rat(y0.clone());
    if d.verts.iter().any(|p| p.y.cmp(&y0s) == Ordering::Equal) {
        return false;
    }
    circles_of(d)
        .iter()
        .all(|c| c.cy.cmp(y0) != Ordering::Equal)
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
    slab_locate(d, operand_of).0
}

/// The slab decomposition, returning both the per-cycle `(A, B)` labels **and** a
/// rational interior sample point of each cycle's face — the latter reused by 3e.1b's
/// boundary-loop orientation / hole-nesting (a point known to be inside a given cell).
fn slab_locate<B: Backend>(
    d: &Dcel<B>,
    operand_of: &impl Fn(CurveId) -> OperandId,
) -> (Vec<Label>, Vec<Pt<B>>) {
    let a_edges = operand_edges(d, operand_of, OperandId::A);
    let b_edges = operand_edges(d, operand_of, OperandId::B);
    let mut labels: Vec<Option<Label>> = vec![None; d.n_cycles];
    let mut reps: Vec<Option<Pt<B>>> = vec![None; d.n_cycles];

    let crit = critical_ys(d);
    if crit.is_empty() {
        return (
            vec![(false, false); d.n_cycles],
            vec![(Rat::from_i128(0), Rat::from_i128(0)); d.n_cycles],
        );
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

    // Genericity self-check (#4): every band ray must strictly avoid all vertex y's and
    // circle centres. This holds by construction iff `critical_ys` is complete; a fire
    // here is a dropped-vertex / missing-circle defect that would otherwise mis-count a
    // parity and silently mislabel a cell. Debug-only (zero release cost); exercised by
    // the whole property/differential suite.
    debug_assert!(
        band_ys.iter().all(|y0| generic_height(d, y0)),
        "slab band height grazed a vertex y or circle centre — critical_ys is incomplete"
    );

    let a_edges = &a_edges;
    let b_edges = &b_edges;
    let assign = |labels: &mut Vec<Option<(bool, bool)>>,
                  reps: &mut Vec<Option<(Rat<B>, Rat<B>)>>,
                  cyc: usize,
                  xm: Rat<B>,
                  y0: &Rat<B>| {
        let lab = (
            winding_parity(&xm, y0, a_edges),
            winding_parity(&xm, y0, b_edges),
        );
        labels[cyc] = Some(lab);
        reps[cyc] = Some((xm, y0.clone()));
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
        assign(&mut labels, &mut reps, xs[0].up_cycle, xm, y0);
        // Cell between consecutive crossings: +x of the left one = the downward face.
        for j in 0..m - 1 {
            let xm = rational_between(&xs[j].x, &xs[j + 1].x);
            assign(&mut labels, &mut reps, xs[j].down_cycle, xm, y0);
        }
        // Rightmost cell: +x of the last crossing.
        let xm = rational_above(&xs[m - 1].x);
        assign(&mut labels, &mut reps, xs[m - 1].down_cycle, xm, y0);
    }

    (
        labels
            .into_iter()
            .map(|l| l.unwrap_or((false, false)))
            .collect(),
        reps.into_iter()
            .map(|r| r.unwrap_or((Rat::from_i128(0), Rat::from_i128(0))))
            .collect(),
    )
}

/// The next boundary half-edge after `h` that keeps the selected region on the left:
/// follow the arrangement `next`, and whenever it lands on an **internal** edge (both
/// sides selected — a suppressed merge edge, spec §6 step 7) cross through its twin
/// into the neighbouring selected cell and continue, until a **separating** edge
/// (selected on the left, unselected on the right) is reached. This walks the boundary
/// of the *merged* selected region, so overlapping selected cells share one loop.
fn boundary_succ<B: Backend>(d: &Dcel<B>, sel: &[bool], h: usize) -> usize {
    let mut g = d.halfedges[h].next;
    loop {
        let twin = d.halfedges[g].twin;
        if !sel[d.halfedges[twin].cycle] {
            return g; // separating: unselected on the right — a boundary edge
        }
        g = d.halfedges[twin].next; // internal edge: cross into the neighbour
    }
}

/// A traced output boundary loop: its edge geometry, a rational interior point of the
/// selected cell on its **left**, and one of the unselected cell on its **right**.
struct BoundaryLoop<B: Backend> {
    edges: Vec<Edge<B>>,
    left_rep: Pt<B>,
    right_rep: Pt<B>,
}

/// Assemble the output region (spec §6 step 8) from the per-cell selection: trace the
/// selected region's boundary loops, classify each as an outer boundary (the selected
/// cell to its left is *inside* it) or a hole (it is *outside*), and nest each hole
/// into its immediate containing outer loop — one [`Face`] (outer + holes) per π₀
/// component. `reps[c]` is a rational interior point of cell `c` (from [`slab_locate`]).
fn emit_region<B: Backend>(d: &Dcel<B>, sel: &[bool], reps: &[(Rat<B>, Rat<B>)]) -> Region<B> {
    // The selected-side half-edge of each separating edge is a boundary half-edge.
    let mut is_boundary = vec![false; d.halfedges.len()];
    for k in 0..d.edges.len() {
        let (ca, cb) = (d.halfedges[2 * k].cycle, d.halfedges[2 * k + 1].cycle);
        if sel[ca] != sel[cb] {
            is_boundary[if sel[ca] { 2 * k } else { 2 * k + 1 }] = true;
        }
    }

    // Trace the boundary half-edges into closed loops.
    let mut visited = vec![false; d.halfedges.len()];
    let mut loops: Vec<BoundaryLoop<B>> = Vec::new();
    for start in 0..d.halfedges.len() {
        if !is_boundary[start] || visited[start] {
            continue;
        }
        let mut edges = Vec::new();
        let mut h = start;
        loop {
            visited[h] = true;
            edges.push(d.edges[d.halfedges[h].edge].edge.clone());
            h = boundary_succ(d, sel, h);
            if h == start {
                break;
            }
        }
        let left = d.halfedges[start].cycle;
        let right = d.halfedges[d.halfedges[start].twin].cycle;
        loops.push(BoundaryLoop {
            edges,
            left_rep: reps[left].clone(),
            right_rep: reps[right].clone(),
        });
    }

    // Classify: a loop is an outer boundary iff its (selected) left cell is inside it;
    // otherwise it is a hole, whose interior is the (unselected) right cell.
    let mut outers: Vec<(Vec<Edge<B>>, Pt<B>)> = Vec::new();
    let mut holes: Vec<(Vec<Edge<B>>, Pt<B>)> = Vec::new();
    for lp in loops {
        if winding_parity(&lp.left_rep.0, &lp.left_rep.1, &lp.edges) {
            outers.push((lp.edges, lp.left_rep));
        } else {
            holes.push((lp.edges, lp.right_rep));
        }
    }

    // One face per outer loop; nest each hole into its immediate containing outer (the
    // deepest candidate — the one whose own interior point lies inside every other
    // candidate). Containers are totally ordered by nesting, so this is well-defined.
    let mut faces: Vec<Face<B>> = outers
        .iter()
        .map(|(e, _)| Face {
            outer: e.clone(),
            holes: Vec::new(),
        })
        .collect();
    for (hedges, hpt) in holes {
        let cands: Vec<usize> = (0..outers.len())
            .filter(|&oi| winding_parity(&hpt.0, &hpt.1, &outers[oi].0))
            .collect();
        let container = cands.iter().copied().find(|&oi| {
            cands.iter().all(|&oj| {
                oj == oi || winding_parity(&outers[oi].1.0, &outers[oi].1.1, &outers[oj].0)
            })
        });
        if let Some(oi) = container {
            faces[oi].holes.push(hedges);
        }
    }

    Region { faces }
}

/// The full eight-step boolean: build the DCEL, label the cells by exact point-location
/// (3e.1), select by `op`, then emit the region as outer + hole loops (spec §6 steps
/// 1–8). Never `Refuted` (searcher); a cocycle failure is surfaced through
/// [`ledge_dom_checked`], not here.
pub fn ledge_dom<B: Backend>(
    edges: &[Edge<B>],
    operand_of: &impl Fn(CurveId) -> OperandId,
    op: BoolOp,
) -> Region<B> {
    let d = Dcel::build(edges);
    let (labels, reps) = slab_locate(&d, operand_of);
    let sel: Vec<bool> = labels.iter().map(|&l| op.select(l)).collect();
    emit_region(&d, &sel, &reps)
}

/// **The certified boolean (spec §8.5 CAP-OUT).** Unlike [`ledge_dom`] (which emits
/// unconditionally) and the old cocycle-only `ledge_dom_checked`, this runs *every*
/// proven checker over the **emitted** region and returns a real [`CapOutFault`] on any
/// defect — so a searcher bug becomes a loud `Refuted`, not a silent wrong region.
///
/// Critically, the labeling is computed **once** and the region is emitted from the
/// *same* labeling that the cocycle checker certified (the old checked path re-ran the
/// point-location, certifying a recomputation rather than the emitted artifact).
///
/// The gates, in order: the DCEL substrate-link integrity; the Kani-proven ℤ₂²
/// `cocycle_ok` over the emitted labeling; `Link_emitted ≅ Link_geometric` at every
/// vertex (Kani-proven `link_iso_ok`); and the `{separating}↔{boundary}` edge bijection
/// over the emitted region. On success it also returns the CAP-OUT-LINK classification
/// (`V_∂` + the pinch vertices) — pinches are *valid* (a △ pinches at its crossings), so
/// they are reported, not refuted.
pub fn ledge_dom_certified<B: Backend>(
    edges: &[Edge<B>],
    operand_of: &impl Fn(CurveId) -> OperandId,
    op: BoolOp,
) -> Verdict<CapOut<B>, CapOutFault, ()> {
    let d = Dcel::build(edges);
    if !d.substrate_link_ok() {
        return Verdict::Refuted(CapOutFault::SubstrateLink);
    }
    let (labels, reps) = slab_locate(&d, operand_of);
    certify_from_labels(&d, operand_of, op, labels, reps)
}

/// The certification core, taking the labeling explicitly so the gates can be exercised
/// on a deliberately-corrupted labeling in tests (a correct searcher never trips them).
fn certify_from_labels<B: Backend>(
    d: &Dcel<B>,
    operand_of: &impl Fn(CurveId) -> OperandId,
    op: BoolOp,
    labels: Vec<Label>,
    reps: Vec<Pt<B>>,
) -> Verdict<CapOut<B>, CapOutFault, ()> {
    // (1) ℤ₂² cocycle over THIS labeling (spec §6 step 4) — the Kani-proven checker.
    let adj: Vec<(usize, usize, bool, bool)> = (0..d.edges.len())
        .map(|k| {
            let (fa, fb) = edge_flips(&d.edges[k], operand_of);
            (
                d.halfedges[2 * k].cycle,
                d.halfedges[2 * k + 1].cycle,
                fa,
                fb,
            )
        })
        .collect();
    let seed = labels
        .iter()
        .position(|&l| l == (false, false))
        .unwrap_or(0);
    let labels_u8: Vec<u8> = labels.iter().map(|&(a, b)| pack(a, b)).collect();
    let (ea, eb, ef) = flat_edges(&adj);
    if !certify_core::arrange::cocycle_ok(d.n_cycles, &labels_u8, seed, &ea, &eb, &ef) {
        return Verdict::Refuted(CapOutFault::Cocycle);
    }

    // (2) Link_emitted ≅ Link_geometric at every vertex (spec §8.5) — Kani-proven.
    for v in 0..d.verts.len() {
        if !link_iso_ok(&link_emitted(d, v), &outgoing_sorted(d, v)) {
            return Verdict::Refuted(CapOutFault::Link { vertex: v });
        }
    }

    // (3) select + emit from THIS labeling (spec §6 steps 6–8).
    let sel: Vec<bool> = labels.iter().map(|&l| op.select(l)).collect();
    let region = emit_region(d, &sel, &reps);

    // (4) {separating edges} ↔ {emitted boundary edges} completeness (spec §8.5).
    if separating_count(d, &sel) != region_boundary_count(&region) {
        return Verdict::Refuted(CapOutFault::Bijection);
    }

    // CAP-OUT-LINK classification (informational): V_∂ = manifold shell vertices,
    // pinches = non-manifold touch points (valid, but not in V_∂).
    let classes = link_classes(d, &sel);
    let v_boundary = (0..classes.len())
        .filter(|&v| classes[v] == LinkClass::Boundary)
        .collect();
    let pinches = (0..classes.len())
        .filter(|&v| classes[v] == LinkClass::Pinch)
        .collect();
    Verdict::Verified(CapOut {
        region,
        v_boundary,
        pinches,
    })
}

/// Back-compat convenience: the region as a `Verdict`, now backed by the full
/// [`ledge_dom_certified`] gate (any [`CapOutFault`] collapses to `Unresolved`, since
/// this signature's `Refuted` is [`core::convert::Infallible`]).
pub fn ledge_dom_checked<B: Backend>(
    edges: &[Edge<B>],
    operand_of: &impl Fn(CurveId) -> OperandId,
    op: BoolOp,
) -> Verdict<Region<B>, core::convert::Infallible, ()> {
    match ledge_dom_certified(edges, operand_of, op) {
        Verdict::Verified(c) => Verdict::Verified(c.region),
        Verdict::Refuted(_) | Verdict::Unresolved(()) => Verdict::Unresolved(()),
    }
}

// ---------------------------------------------------------------------------
// CAP-OUT-LINK (spec §8.5, slice 3e.2b) — the searcher side of the V_∂ / manifold
// classifier. At each vertex the incident faces, taken in azimuth order, give a cyclic
// sector-selected mask; `certify_core::arrange::classify_link` classifies it. The order
// is `dir_cmp` (geometric), so the per-vertex class is **frame-invariant** — the net
// for the tangency case whose raw face count is not.
// ---------------------------------------------------------------------------

/// The outgoing half-edges at vertex `v`, in the rotation-system (azimuth) order.
fn outgoing_sorted<B: Backend>(d: &Dcel<B>, v: usize) -> Vec<usize> {
    let mut outs: Vec<usize> = (0..d.halfedges.len())
        .filter(|&h| d.halfedges[h].origin == v)
        .collect();
    let tan = |h: usize| {
        crate::tangent::outgoing_tangent(&d.edges[d.halfedges[h].edge].edge, d.halfedges[h].dir)
    };
    outs.sort_by(|&h1, &h2| crate::tangent::dir_cmp(&tan(h1), &tan(h2)));
    outs
}

/// The cyclic sector-selected mask at `v`: for each outgoing half-edge (azimuth order),
/// the selection bit of the face on its left (the sector CCW-adjacent to it).
fn sector_mask<B: Backend>(d: &Dcel<B>, sel: &[bool], v: usize) -> Vec<bool> {
    outgoing_sorted(d, v)
        .iter()
        .map(|&h| sel[d.halfedges[h].cycle])
        .collect()
}

/// The CAP-OUT-LINK class of every vertex for the selection `sel` (spec §8.5), via the
/// Kani-proven `certify_core::arrange::classify_link`. `V_∂ = { v : Boundary }`; any
/// `Pinch` is a non-manifold internal-tangency vertex.
pub fn link_classes<B: Backend>(d: &Dcel<B>, sel: &[bool]) -> Vec<LinkClass> {
    (0..d.verts.len())
        .map(|v| classify_link(&sector_mask(d, sel, v)))
        .collect()
}

/// Does the boolean `op` of the operands produce a non-manifold **pinch** (an internal
/// tangency where the selected region touches itself at a point)? Frame-invariant — the
/// CAP-OUT-LINK net over geometric sectors, where the emitted face count is not.
pub fn has_pinch<B: Backend>(
    edges: &[Edge<B>],
    operand_of: &impl Fn(CurveId) -> OperandId,
    op: BoolOp,
) -> bool {
    let d = Dcel::build(edges);
    let (labels, _reps) = slab_locate(&d, operand_of);
    let sel: Vec<bool> = labels.iter().map(|&l| op.select(l)).collect();
    link_classes(&d, &sel).contains(&LinkClass::Pinch)
}

// ---------------------------------------------------------------------------
// Link_emitted ≅ Link_geometric + completeness bijections (spec §8.5, slice 3e.3).
// ---------------------------------------------------------------------------

/// `Link_emitted(v)`: the incident outgoing half-edges in the **stored** rotation order
/// (the CCW orbit of `o ↦ twin(prev(o))`, as wired by `Dcel::link_rotation`).
fn link_emitted<B: Backend>(d: &Dcel<B>, v: usize) -> Vec<usize> {
    let start = match (0..d.halfedges.len()).find(|&h| d.halfedges[h].origin == v) {
        Some(h) => h,
        None => return Vec::new(),
    };
    let mut order = vec![start];
    let mut o = start;
    loop {
        // twin(prev(o)) is the next outgoing half-edge CCW around `v`.
        o = d.halfedges[d.halfedges[o].prev].twin;
        if o == start {
            break;
        }
        order.push(o);
    }
    order
}

/// Audit `Link_emitted(v) ≅ Link_geometric(v)` at every vertex (spec §8.5 SEW-LINK): the
/// stored face-cycle rotation equals the geometric azimuth sort ([`outgoing_sorted`]) as
/// an identity-fixing oriented cyclic isomorphism (via the Kani-proven
/// `certify_core::arrange::link_iso_ok`). A searcher-integrity audit of `link_rotation`.
pub fn links_consistent<B: Backend>(d: &Dcel<B>) -> bool {
    (0..d.verts.len()).all(|v| link_iso_ok(&link_emitted(d, v), &outgoing_sorted(d, v)))
}

/// The number of **separating** edges (selected | unselected) for a selection — the
/// emitted boundary edges (spec §6 step 7: exactly one selected and one unselected side).
pub fn separating_count<B: Backend>(d: &Dcel<B>, sel: &[bool]) -> usize {
    (0..d.edges.len())
        .filter(|&k| sel[d.halfedges[2 * k].cycle] != sel[d.halfedges[2 * k + 1].cycle])
        .count()
}

/// Total boundary edges a region emits (outer loops + holes) — the `{separating edges} ↔
/// {emitted boundary edges}` side of the CAP-OUT completeness bijection (spec §8.5).
pub fn region_boundary_count<B: Backend>(r: &Region<B>) -> usize {
    r.faces
        .iter()
        .map(|f| f.outer.len() + f.holes.iter().map(Vec::len).sum::<usize>())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use geom::content::{Circle, Curve, Line, Orient, Point2, SegPiece};
    use lattice::{Bignum, Rat};

    type Q = Rat<Bignum>;

    /// A closed polygon operand: the CCW loop of segments through `verts`, tagged `src`.
    fn polygon(verts: &[(i128, i128)], src: u32) -> Vec<Edge<Bignum>> {
        let n = verts.len();
        (0..n)
            .map(|i| {
                let (sx, sy) = verts[i];
                let (ex, ey) = verts[(i + 1) % n];
                let (a, b) = (Q::from_i128(-(ey - sy)), Q::from_i128(ex - sx));
                let c = a
                    .mul(&Q::from_i128(sx))
                    .add(&b.mul(&Q::from_i128(sy)))
                    .neg();
                Edge::Seg(Box::new(SegPiece {
                    line: Line { a, b, c },
                    start: Point2::from_rat(Q::from_i128(sx), Q::from_i128(sy)),
                    end: Point2::from_rat(Q::from_i128(ex), Q::from_i128(ey)),
                    orient: Orient::Ccw,
                    source: CurveId(src),
                }))
            })
            .collect()
    }
    /// The number of certified output faces of `op`, panicking on any CAP-OUT refutation.
    fn certified_faces(
        edges: &[Edge<Bignum>],
        operand_of: &impl Fn(CurveId) -> OperandId,
        op: BoolOp,
    ) -> usize {
        match ledge_dom_certified(edges, operand_of, op) {
            Verdict::Verified(c) => c.region.faces.len(),
            Verdict::Refuted(f) => panic!("CAP-OUT refuted a valid boolean: {f:?}"),
            Verdict::Unresolved(()) => panic!("unresolved"),
        }
    }

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
        assert_eq!(r.faces[0].outer.len(), 4, "bounded by the four outer arcs");
        assert!(r.faces[0].holes.is_empty(), "no holes");
    }

    #[test]
    fn two_disks_intersection_one_face() {
        let r = ledge_dom(&two_disks(), &ab, BoolOp::And);
        assert_eq!(r.faces.len(), 1, "∩ is the single lens");
        assert_eq!(r.faces[0].outer.len(), 4, "bounded by the four inner arcs");
        assert!(r.faces[0].holes.is_empty(), "no holes");
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
            assert_eq!(f.outer.len(), 4, "each lune: two outer + two inner arcs");
            assert!(f.holes.is_empty(), "a lune has no hole");
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
        let inter = ledge_dom(&e, &ab, BoolOp::And);
        assert_eq!(inter.faces.len(), 1, "∩ = inner disk");
        assert!(
            inter.faces[0].holes.is_empty(),
            "the inner disk has no hole"
        );
        let uni = ledge_dom(&e, &ab, BoolOp::Or);
        assert_eq!(uni.faces.len(), 1, "∪ = outer disk");
        assert!(
            uni.faces[0].holes.is_empty(),
            "the filled outer disk has no hole"
        );

        // △ = the annulus: ONE face (the ring) with ONE hole (the inner disk) — the
        // Face-with-holes nesting (3e.1b), where flat π₀ would have emitted two faces.
        let xor = ledge_dom(&e, &ab, BoolOp::Xor);
        assert_eq!(xor.faces.len(), 1, "△ = the annulus is one face");
        assert_eq!(
            xor.faces[0].outer.len(),
            2,
            "outer boundary = the two outer arcs"
        );
        assert_eq!(xor.faces[0].holes.len(), 1, "one hole (the inner disk)");
        assert_eq!(
            xor.faces[0].holes[0].len(),
            2,
            "the hole = the two inner arcs"
        );
        assert!(matches!(
            ledge_dom_checked(&e, &ab, BoolOp::Xor),
            Verdict::Verified(_)
        ));
    }

    /// **CAP-OUT-LINK detects the internal-tangency pinch** (3e.2): c1=(0,0,4) r=2 and
    /// c2=(1,0,1) r=1 are internally tangent at (2,0). Their △ (the crescent) pinches to
    /// a point there — CAP-OUT-LINK classifies that vertex as a non-manifold `Pinch`,
    /// while ∪ (the outer disk) and ∩ (the inner disk) are smooth at the touch (no pinch).
    #[test]
    fn internal_tangency_pinch_detected() {
        let e = two_disk_edges(&disk(0, 0, 4), &disk(1, 0, 1));
        assert!(
            has_pinch(&e, &ab, BoolOp::Xor),
            "△ pinches at the internal tangency"
        );
        assert!(!has_pinch(&e, &ab, BoolOp::Or), "∪ = outer disk, smooth");
        assert!(!has_pinch(&e, &ab, BoolOp::And), "∩ = inner disk, smooth");
    }

    /// The tangency pinch is **frame-invariant** — the net CAP-OUT-LINK provides where
    /// the raw face count is not (3d.4b): whether △ pinches survives every rational
    /// rigid motion, even though the motion moves the tangent point off the x-extremum
    /// and re-splits the decomposition. External tangency likewise pinches under △.
    #[test]
    fn tangency_pinch_rigid_invariant() {
        let cfgs = [
            (disk(0, 0, 4), disk(1, 0, 1)), // internal tangency at (2,0)
            (disk(0, 0, 4), disk(4, 0, 4)), // external tangency at (2,0)
        ];
        for (c1, c2) in &cfgs {
            for &(u, v, tx, ty) in &[(1, 0, 0, 0), (2, 1, 0, 0), (1, 2, 3, -1), (3, 1, -2, 4)] {
                let m = rigid(u, v, tx, ty);
                let e = two_disk_edges(&rigid_circle(c1, &m), &rigid_circle(c2, &m));
                assert!(
                    has_pinch(&e, &ab, BoolOp::Xor),
                    "△ pinch is frame-invariant: c=({:?},{:?}) motion=({u},{v},{tx},{ty})",
                    c1.cx,
                    c2.cx
                );
            }
        }
    }

    /// The number of `Pinch` (non-manifold) vertices in the △ of two disks.
    fn pinch_count(edges: &[Edge<Bignum>]) -> usize {
        let d = Dcel::build(edges);
        let (labels, _) = slab_locate(&d, &ab);
        let sel: Vec<bool> = labels.iter().map(|&l| BoolOp::Xor.select(l)).collect();
        link_classes(&d, &sel)
            .iter()
            .filter(|c| **c == LinkClass::Pinch)
            .count()
    }

    /// **CAP-OUT-LINK is the frame-invariant net** (3e.2): the pinch count of a △ is the
    /// same in every frame, even where the raw vertex count is not — a rigid motion moves
    /// a tangent point off the x-extremum (adding smooth extremum vertices) but never
    /// changes how many vertices are genuine non-manifold pinches.
    #[test]
    fn pinch_count_rigid_invariant() {
        let cfgs = [
            (disk(0, 0, 4), disk(1, 0, 1)),   // internal tangency
            (disk(0, 0, 25), disk(8, 0, 25)), // transverse (2 crossings)
            (disk(0, 0, 4), disk(4, 0, 4)),   // external tangency
        ];
        for (c1, c2) in &cfgs {
            let base = pinch_count(&two_disk_edges(c1, c2));
            for &(u, v, tx, ty) in &[(2, 1, 0, 0), (1, 2, 3, -1), (3, 1, -2, 4)] {
                let m = rigid(u, v, tx, ty);
                let e = two_disk_edges(&rigid_circle(c1, &m), &rigid_circle(c2, &m));
                assert_eq!(
                    pinch_count(&e),
                    base,
                    "pinch count is frame-invariant: c=({:?},{:?})",
                    c1.cx,
                    c2.cx
                );
            }
        }
    }

    /// The four representative corpus configs (transverse, disjoint, annulus, tangency).
    fn corpus() -> Vec<Vec<Edge<Bignum>>> {
        vec![
            two_disk_edges(&disk(0, 0, 25), &disk(8, 0, 25)), // transverse
            two_disk_edges(&disk(0, 0, 1), &disk(0, 3, 1)),   // disjoint
            two_disk_edges(&disk(0, 0, 9), &disk(0, 0, 1)),   // nested annulus
            two_disk_edges(&disk(0, 0, 4), &disk(1, 0, 1)),   // internal tangency
        ]
    }

    /// **Link_emitted ≅ Link_geometric** (3e.3): at every vertex of every corpus
    /// arrangement, the stored rotation order equals the geometric azimuth sort as an
    /// identity-fixing oriented cyclic isomorphism — the `link_rotation` integrity audit.
    #[test]
    fn links_consistent_on_corpus() {
        for e in corpus() {
            let d = Dcel::build(&e);
            assert!(
                links_consistent(&d),
                "Link_emitted ≅ Link_geometric at every vertex"
            );
        }
    }

    /// **{separating edges} ↔ {emitted boundary edges}** (3e.3, CAP-OUT completeness):
    /// for every op on every corpus config, the number of separating (selected|unselected)
    /// edges equals the total edges the region emits across its outer loops and holes.
    #[test]
    fn separating_boundary_bijection() {
        for e in corpus() {
            let d = Dcel::build(&e);
            let (labels, _) = slab_locate(&d, &ab);
            for op in [BoolOp::Xor, BoolOp::And, BoolOp::Or] {
                let sel: Vec<bool> = labels.iter().map(|&l| op.select(l)).collect();
                let r = ledge_dom(&e, &ab, op);
                assert_eq!(
                    separating_count(&d, &sel),
                    region_boundary_count(&r),
                    "separating ↔ boundary edge bijection ({op:?})"
                );
            }
        }
    }

    /// **The certified entry accepts the whole corpus** (transverse, disjoint, annulus,
    /// tangency) for every op — all four CAP-OUT gates (substrate-link, cocycle,
    /// Link≅geom, bijection) pass over the *emitted* region.
    #[test]
    fn certified_verified_on_corpus() {
        for e in corpus() {
            for op in [BoolOp::Xor, BoolOp::And, BoolOp::Or] {
                assert!(
                    matches!(ledge_dom_certified(&e, &ab, op), Verdict::Verified(_)),
                    "CAP-OUT must certify a valid boolean ({op:?})"
                );
            }
        }
    }

    /// The CAP-OUT-LINK classification carried by the certificate: a △ pinches at every
    /// crossing/tangent point (2 for transverse overlap, 1 for internal tangency), while
    /// ∪/∩ are manifold (no pinch). These vertices are reported, not refuted.
    #[test]
    fn certified_pinch_classification() {
        let got = |e: &[Edge<Bignum>], op| match ledge_dom_certified(e, &ab, op) {
            Verdict::Verified(c) => c.pinches.len(),
            _ => panic!("expected Verified"),
        };
        let transverse = two_disk_edges(&disk(0, 0, 25), &disk(8, 0, 25));
        assert_eq!(got(&transverse, BoolOp::Xor), 2, "transverse △: 2 pinches");
        assert_eq!(got(&transverse, BoolOp::Or), 0, "∪ is manifold");
        assert_eq!(got(&transverse, BoolOp::And), 0, "∩ is manifold");
        let tangency = two_disk_edges(&disk(0, 0, 4), &disk(1, 0, 1));
        assert_eq!(
            got(&tangency, BoolOp::Xor),
            1,
            "internal tangency △: 1 pinch"
        );
    }

    /// **The gate is real, not decorative:** a deliberately-corrupted labeling (one
    /// cell's A-bit flipped, breaking the ℤ₂² cochain) is *refuted* with `Cocycle` —
    /// where the plain `ledge_dom` would have silently emitted a wrong region.
    #[test]
    fn certified_refutes_corrupted_labeling() {
        let e = two_disks();
        let d = Dcel::build(&e);
        let (mut labels, reps) = slab_locate(&d, &ab);
        labels[0].0 = !labels[0].0; // break the cochain around cell 0
        assert!(matches!(
            certify_from_labels(&d, &ab, BoolOp::Or, labels, reps),
            Verdict::Refuted(CapOutFault::Cocycle)
        ));
    }

    /// The slab genericity self-check (#4) distinguishes a grazing height (equal to a
    /// vertex y or a circle centre — the silent-wrong risk) from a generic one. Two disks
    /// (0,0,25),(8,0,25): vertices at y ∈ {−3,0,3}, centre cy = 0.
    #[test]
    fn generic_height_detects_grazing() {
        let d = Dcel::build(&two_disks());
        assert!(
            !generic_height(&d, &Q::from_i128(0)),
            "y=0 = cy and the extrema y"
        );
        assert!(
            !generic_height(&d, &Q::from_i128(3)),
            "y=3 = the crossing vertices"
        );
        assert!(generic_height(&d, &Q::new(1, 2)), "y=1/2 is generic");
    }

    /// **Boolean over polygon (segment) operands** (#3) — the disks-only corpus never
    /// exercised line-bounded regions. Two overlapping 4×4 squares: ∪ = one face, ∩ =
    /// the 2×2 overlap, △ certifies (two L-shapes pinched at the crossings).
    #[test]
    fn boolean_over_polygons() {
        let mut e = polygon(&[(0, 0), (4, 0), (4, 4), (0, 4)], 0);
        e.extend(polygon(&[(2, 2), (6, 2), (6, 6), (2, 6)], 1));
        assert_eq!(
            certified_faces(&e, &ab, BoolOp::Or),
            1,
            "∪ of overlapping squares"
        );
        assert_eq!(certified_faces(&e, &ab, BoolOp::And), 1, "∩ is the overlap");
        assert!(matches!(
            ledge_dom_certified(&e, &ab, BoolOp::Xor),
            Verdict::Verified(_)
        ));
    }

    /// **Mixed line+circle operands** (#3): a 6×6 square A with a radius-2 disk B fully
    /// inside it. ∩ = the disk, ∪ = the square, △ = the square with a disk-shaped hole
    /// (one face, one hole) — the polygon analogue of the annulus.
    #[test]
    fn boolean_mixed_line_circle() {
        let mut e = polygon(&[(0, 0), (6, 0), (6, 6), (0, 6)], 0);
        e.extend(circle_edges(3, 3, 4, 1));
        assert_eq!(
            certified_faces(&e, &ab, BoolOp::And),
            1,
            "∩ = the inner disk"
        );
        assert_eq!(certified_faces(&e, &ab, BoolOp::Or), 1, "∪ = the square");
        match ledge_dom_certified(&e, &ab, BoolOp::Xor) {
            Verdict::Verified(c) => {
                assert_eq!(c.region.faces.len(), 1, "△ = one face");
                assert_eq!(c.region.faces[0].holes.len(), 1, "with a disk hole");
            }
            Verdict::Refuted(f) => panic!("△ refuted: {f:?}"),
            Verdict::Unresolved(()) => panic!("△ unresolved"),
        }
    }

    /// **Degree-6 arrangement vertex** (#3): three circles through the common point
    /// (0,0) — (1,0,1),(0,1,1),(1,1,2) — where the corpus never went past degree 4.
    /// Operands A = {two circles}, B = {one}. The certified entry must Verify for every
    /// op (CAP-OUT-LINK is proven up to ≤6 sectors).
    #[test]
    fn boolean_degree6_vertex() {
        let mut e = circle_edges(1, 0, 1, 0);
        e.extend(circle_edges(0, 1, 1, 1));
        e.extend(circle_edges(1, 1, 2, 2));
        let op3 = |s: CurveId| if s.0 <= 1 { OperandId::A } else { OperandId::B };
        for op in [BoolOp::Xor, BoolOp::And, BoolOp::Or] {
            assert!(
                matches!(ledge_dom_certified(&e, &op3, op), Verdict::Verified(_)),
                "degree-6 boolean must certify ({op:?})"
            );
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(128))]

        /// The boolean's output face count (△/∩/∪) is invariant under a rational rigid
        /// motion — the whole DCEL + eight-step + point-location pipeline is
        /// frame-independent across the **full** regime: transverse, tangent, disjoint,
        /// nested, identical. (3d scoped this to `crosses_twice` because the tangency
        /// count was frame-dependent under the old π₀-over-cells emission; the 3e.1b
        /// boundary-loop emission fixed that, so the restriction is lifted.)
        #[test]
        fn boolean_face_count_rigid_invariant(
            x1 in -3i128..=3, y1 in -3i128..=3, r1 in 1i128..=6,
            x2 in -3i128..=3, y2 in -3i128..=3, r2 in 1i128..=6,
            u in -3i128..=3, v in -3i128..=3, tx in -4i128..=4, ty in -4i128..=4,
        ) {
            prop_assume!(u != 0 || v != 0);
            let (c1, c2) = (disk(x1, y1, r1), disk(x2, y2, r2));
            let m = rigid(u, v, tx, ty);
            let e0 = two_disk_edges(&c1, &c2);
            let e1 = two_disk_edges(&rigid_circle(&c1, &m), &rigid_circle(&c2, &m));
            prop_assert_eq!(face_counts(&e0), face_counts(&e1));
        }

        /// Invariant under lattice rescaling `p ↦ k·p` (`k > 0`) across the full regime:
        /// scaling preserves the arrangement's combinatorics, hence the face counts.
        #[test]
        fn boolean_face_count_scale_invariant(
            x1 in -3i128..=3, y1 in -3i128..=3, r1 in 1i128..=6,
            x2 in -3i128..=3, y2 in -3i128..=3, r2 in 1i128..=6,
            k in 1i128..=5,
        ) {
            let (c1, c2) = (disk(x1, y1, r1), disk(x2, y2, r2));
            let kk = Q::from_i128(k);
            let e0 = two_disk_edges(&c1, &c2);
            let e1 = two_disk_edges(&scale_circle(&c1, &kk), &scale_circle(&c2, &kk));
            prop_assert_eq!(face_counts(&e0), face_counts(&e1));
        }
    }
}
