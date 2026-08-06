//! Boolean operations (∪ / ∩ / △) over two regions bounded by lines and circular arcs.
//!
//! Given the decomposed input edges and a map from each source curve to operand `A` or
//! `B`, [`ledge_dom`] computes the boolean and returns a [`Region`]: one [`Face`] per
//! connected component, each with an outer boundary loop (CCW) and counter-oriented hole
//! loops (CW). An annulus, for example, is one face with one hole.
//!
//! # How it works
//!
//! 1. Build the arrangement half-edge structure ([`super::dcel`]).
//! 2. Label every cell with its `(inside-A, inside-B)` membership by exact
//!    point-location (a horizontal-slab ray-cast; see [`super::locate`]).
//! 3. Select the cells the operation keeps: `△ = A⊕B`, `∩ = A∧B`, `∪ = A∨B` ([`BoolOp`]).
//! 4. Trace the boundary between kept and dropped cells into the output face loops.
//!
//! It is exact and invariant under rigid motion and rescaling across the full input
//! regime: transversely crossing, tangent, disjoint, nested, and identical/coincident
//! operands.
//!
//! # Certified vs plain entry
//!
//! - [`ledge_dom`] emits a [`Region`] unconditionally — fast, and correct for valid
//!   input, but performs no self-check.
//! - [`ledge_dom_certified`] returns the same region wrapped in a `Verdict`: it runs the
//!   formally verified [`certify_core::arrange`] checkers over the *emitted* region and
//!   reports any internal inconsistency as a [`CapOutFault`] (`Refuted`) rather than a
//!   silently-wrong region. On success it also classifies the arrangement vertices into
//!   `V_∂` (manifold shell vertices) and pinch points.
//!
//! # Pinch points
//!
//! Where a symmetric difference touches itself at a single point — the crossings of two
//! overlapping disks, or a tangency — the two lobes meet only at that vertex. This is a
//! valid result: the lobes are emitted as separate faces meeting at the point, and the
//! vertex is reported as a *pinch* (excluded from `V_∂`), not treated as an error.

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
/// A rational point `(x, y)` known to lie in the interior of a cell.
type Pt<B> = (Rat<B>, Rat<B>);

/// Which of the two operands a source curve belongs to. Every input curve is assigned
/// `A` or `B`; the boolean combines the region bounded by the `A` curves with the region
/// bounded by the `B` curves.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OperandId {
    A,
    B,
}

/// The boolean operation to compute.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BoolOp {
    /// Symmetric difference, `A △ B` — in exactly one operand.
    Xor,
    /// Intersection, `A ∩ B` — in both operands.
    And,
    /// Union, `A ∪ B` — in either operand.
    Or,
}

impl BoolOp {
    /// Whether a cell with membership `(inside_a, inside_b)` is kept by this operation.
    pub fn select(self, label: (bool, bool)) -> bool {
        let (a, b) = label;
        match self {
            BoolOp::Xor => a ^ b,
            BoolOp::And => a && b,
            BoolOp::Or => a || b,
        }
    }
}

/// One face of the output region: an `outer` boundary loop (counterclockwise) and zero
/// or more `holes` (clockwise inner loops — regions strictly enclosed by `outer` but not
/// part of the face). Each loop is the sequence of boundary edges, in order.
pub struct Face<B: Backend> {
    pub outer: Vec<Edge<B>>,
    pub holes: Vec<Vec<Edge<B>>>,
}

/// A boolean result: one [`Face`] per connected component of the region.
pub struct Region<B: Backend> {
    pub faces: Vec<Face<B>>,
}

/// Why [`ledge_dom_certified`] refused a region: an internal-consistency check that a
/// correctly-built region always passes. A fault therefore indicates a bug in the
/// constructor, not an unsupported input — each variant names the failed check and the
/// class of defect it catches.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CapOutFault {
    /// The half-edge structure is malformed — a broken twin pairing or a dangling
    /// half-edge ([`Dcel::substrate_link_ok`]).
    SubstrateLink,
    /// The cell membership labeling is inconsistent — some closed walk does not return
    /// to its starting label, indicating a mis-paired twin or a dropped intersection
    /// event (rejected by the verified `certify_core::arrange::cocycle_ok`).
    Cocycle,
    /// At vertex `v`, the stored edge rotation disagrees with the geometric azimuth order
    /// (rejected by the verified `certify_core::arrange::link_iso_ok`).
    Link { vertex: usize },
    /// The number of emitted boundary edges does not equal the number of edges separating
    /// a kept cell from a dropped one — emission dropped or added a boundary edge. This is
    /// a **coverage count**, not a full bijection: it certifies every separating edge is
    /// emitted exactly once (given the tracer emits each boundary half-edge at most once),
    /// but does not check per-edge source identity, loop closure, or orientation. The
    /// stronger source-ID permutation certificate is a tracked follow-up (see
    /// `docs/engineering-log.md`).
    BoundaryEdgeCount,
    /// The slab point-location did not cover every arrangement cell — some cycle got no
    /// label, or a band height grazed a vertex `y` / circle centre (`critical_ys` incomplete).
    /// A correctly-built arrangement covers every cell, so this is a decomposition defect; the
    /// certified path refuses it rather than certify a silently-defaulted `(false, false)`
    /// label (rejected by `slab_decomposition_generic` + the `all_assigned` flag).
    Incomplete,
}

/// A certified boolean result: the emitted [`Region`] plus a classification of the
/// arrangement vertices. Opaque — its fields are private and it is minted only by
/// [`ledge_dom_certified`] after every CAP-OUT checker passes, so a `CapOut` cannot be
/// forged by assembling one from arbitrary parts. Read it through the accessors:
/// [`region`](CapOut::region), the manifold shell vertices [`v_boundary`](CapOut::v_boundary)
/// (`V_∂`), and the [`pinches`](CapOut::pinches) — points where the region touches itself
/// (see the module-level "Pinch points" note), valid but not manifold boundary vertices.
pub struct CapOut<B: Backend> {
    region: Region<B>,
    v_boundary: Vec<usize>,
    pinches: Vec<usize>,
}

impl<B: Backend> CapOut<B> {
    /// The emitted region (one [`Face`] per connected component: outer loop + holes).
    pub fn region(&self) -> &Region<B> {
        &self.region
    }
    /// The manifold shell vertices `V_∂`.
    pub fn v_boundary(&self) -> &[usize] {
        &self.v_boundary
    }
    /// The pinch points — non-manifold self-touch vertices (valid, but not in `V_∂`).
    pub fn pinches(&self) -> &[usize] {
        &self.pinches
    }
    /// Consume the certificate into its parts `(region, v_boundary, pinches)`.
    pub fn into_parts(self) -> (Region<B>, Vec<usize>, Vec<usize>) {
        (self.region, self.v_boundary, self.pinches)
    }
}

/// The cell membership labeling in the flat, index-array form the
/// `certify_core::arrange::cocycle_ok` checker consumes. `labels[c]` is cell `c`'s
/// `(inside_a, inside_b)`; `adj[k] = (cyc_a, cyc_b, flip_a, flip_b)` is the k-th edge —
/// crossing it flips `A` iff `flip_a` and `B` iff `flip_b`; `seed` is an outer `(0,0)`
/// cell; `cocycle_ok` is the checker's verdict on this labeling.
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

/// Compute the `(inside_a, inside_b)` membership of every arrangement cell and run the
/// consistency check on it. Membership is found by exact point-location; the result is a
/// [`CellLabeling`] whose `cocycle_ok` field is the verified checker's verdict.
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

    // Label every cell by exact point-location. Each cell (a traced boundary cycle) is
    // labeled independently, so the result is correct even when the cell-adjacency graph
    // is disconnected — as it is for disjoint operands and for holes, where one region is
    // bounded by several cycles that share no edge.
    let labels = label_all_cycles(d, operand_of);

    // The checker anchors at an unbounded `(A,B) = (0,0)` cell — the region outside every
    // operand, which always exists. Any such cell works.
    let seed = labels
        .iter()
        .position(|&l| l == (false, false))
        .unwrap_or(0);

    // Run the verified consistency checker on the computed labeling.
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

/// Label every cell by exact point-location. For each horizontal slab between consecutive
/// critical heights, cast one generic rational ray; for each cell the ray crosses, set its
/// `(A, B)` label from the even-odd `winding_parity` of an interior sample against the A-
/// and B-boundary edges. Labels every cell independently, so it is correct even when the
/// arrangement is disconnected or nested (disjoint operands, holes).
fn label_all_cycles<B: Backend>(
    d: &Dcel<B>,
    operand_of: &impl Fn(CurveId) -> OperandId,
) -> Vec<(bool, bool)> {
    slab_locate(d, operand_of).0
}

/// The generic band heights: one strictly below the lowest critical, one strictly between
/// each consecutive pair, one above the highest. Empty when there are no criticals (an empty
/// arrangement). Each is strictly between (or beyond) criticals, so — *iff* `critical_ys` is
/// complete — it avoids every vertex `y` and circle `cy` (the ray-cast genericity precondition).
fn slab_band_ys<B: Backend>(d: &Dcel<B>) -> Vec<Rat<B>> {
    let crit = critical_ys(d);
    if crit.is_empty() {
        return Vec::new();
    }
    let mut band_ys: Vec<Rat<B>> = Vec::with_capacity(crit.len() + 1);
    band_ys.push(rational_below(&crit[0]));
    for w in crit.windows(2) {
        band_ys.push(rational_between(&w[0], &w[1]));
    }
    band_ys.push(rational_above(&crit[crit.len() - 1]));
    band_ys
}

/// Whether every slab band height is **generic** — strictly avoids every vertex `y` and
/// circle centre `cy` — i.e. [`critical_ys`] is complete. A `false` here is a
/// dropped-vertex / missing-circle decomposition defect that would otherwise mis-count a
/// parity and silently mislabel a cell. This is `O(bands × arrangement)`, too costly for the
/// fast [`ledge_dom`] path, so [`ledge_dom_certified`] runs it as an explicit gate (a
/// [`debug_assert`] covers the fast path in dev builds).
pub(crate) fn slab_decomposition_generic<B: Backend>(d: &Dcel<B>) -> bool {
    slab_band_ys(d).iter().all(|y0| generic_height(d, y0))
}

/// The slab decomposition, returning the per-cell `(A, B)` labels, a rational interior point
/// per cell, and whether **every** cycle was assigned a label. Boundary-loop orientation and
/// hole-nesting use the interior points to decide which cell a traced loop bounds. An
/// unassigned cycle (the `all_assigned = false` flag) is a decomposition defect: on the fast
/// path it falls back to a default `(false, false)` label / origin rep, but
/// [`ledge_dom_certified`] refuses it ([`CapOutFault::Incomplete`]) rather than certify a
/// silently-manufactured label.
fn slab_locate<B: Backend>(
    d: &Dcel<B>,
    operand_of: &impl Fn(CurveId) -> OperandId,
) -> (Vec<Label>, Vec<Pt<B>>, bool) {
    let a_edges = operand_edges(d, operand_of, OperandId::A);
    let b_edges = operand_edges(d, operand_of, OperandId::B);
    let mut labels: Vec<Option<Label>> = vec![None; d.n_cycles];
    let mut reps: Vec<Option<Pt<B>>> = vec![None; d.n_cycles];

    let band_ys = slab_band_ys(d);
    if band_ys.is_empty() {
        // No criticals — an empty arrangement; the unbounded cell(s) are correctly outside.
        return (
            vec![(false, false); d.n_cycles],
            vec![(Rat::from_i128(0), Rat::from_i128(0)); d.n_cycles],
            true,
        );
    }
    // Genericity holds by construction iff `critical_ys` is complete; the certified path
    // gates on `slab_decomposition_generic`, this only guards dev builds of the fast path.
    debug_assert!(
        slab_decomposition_generic(d),
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

    let all_assigned = labels.iter().all(Option::is_some);
    (
        labels
            .into_iter()
            .map(|l| l.unwrap_or((false, false)))
            .collect(),
        reps.into_iter()
            .map(|r| r.unwrap_or((Rat::from_i128(0), Rat::from_i128(0))))
            .collect(),
        all_assigned,
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

/// Compute the boolean `op` of the two operands and return the result region.
///
/// Builds the arrangement, labels every cell by exact point-location, keeps the cells
/// `op` selects, and traces the kept region into faces (outer loop + holes). This is the
/// plain entry point: it always emits a region. To have the emitted region checked by the
/// verified checkers, use [`ledge_dom_certified`].
///
/// ```
/// use arrange2d::boolean::{ledge_dom, BoolOp, OperandId};
/// use arrange2d::decompose::decompose;
/// use geom::content::{Circle, Curve, CurveId, Orient};
/// use lattice::{Bignum, Rat};
///
/// let disk = |cx, cy, r2, src| decompose(&Curve::Circle {
///     circle: Circle {
///         cx: Rat::<Bignum>::from_i128(cx),
///         cy: Rat::from_i128(cy),
///         r2: Rat::from_i128(r2),
///     },
///     orient: Orient::Ccw,
///     source: CurveId(src),
/// });
/// let mut edges = disk(0, 0, 25, 0);
/// edges.extend(disk(8, 0, 25, 1));
/// let operand_of = |c: CurveId| if c.0 == 0 { OperandId::A } else { OperandId::B };
///
/// // Intersection of the two overlapping disks is the single lens.
/// let lens = ledge_dom(&edges, &operand_of, BoolOp::And);
/// assert_eq!(lens.faces.len(), 1);
/// ```
pub fn ledge_dom<B: Backend>(
    edges: &[Edge<B>],
    operand_of: &impl Fn(CurveId) -> OperandId,
    op: BoolOp,
) -> Region<B> {
    let d = Dcel::build(edges);
    let (labels, reps, _) = slab_locate(&d, operand_of);
    let sel: Vec<bool> = labels.iter().map(|&l| op.select(l)).collect();
    emit_region(&d, &sel, &reps)
}

/// Compute the boolean `op` and certify the result (spec §8.5 CAP-OUT).
///
/// Same result as [`ledge_dom`], but every output runs through the verified checkers, and
/// any defect is reported as a [`CapOutFault`] instead of a silently-wrong region. The
/// region is emitted from the *same* labeling the checkers certify, so what is verified is
/// exactly what is returned.
///
/// The gates, in order:
/// - the DCEL's twin-pairing integrity ([`CapOutFault::SubstrateLink`]);
/// - the ℤ₂² `cocycle_ok` consistency of the cell labeling ([`CapOutFault::Cocycle`]);
/// - `Link_emitted ≅ Link_geometric` at every vertex ([`CapOutFault::Link`]);
/// - the separating-edge / boundary-edge coverage count ([`CapOutFault::BoundaryEdgeCount`]).
///
/// The middle two are Kani-proven. On success it returns the [`CapOut`], which carries the
/// region together with the CAP-OUT-LINK classification of the arrangement vertices (the
/// boundary set `V_∂` and the pinch points). Pinches are valid — a symmetric difference
/// pinches at its crossings — so they are reported, not refused.
///
/// ```
/// use arrange2d::boolean::{ledge_dom_certified, BoolOp, CapOut, OperandId};
/// use arrange2d::decompose::decompose;
/// use certify_core::Verdict;
/// use geom::content::{Circle, Curve, CurveId, Orient};
/// use lattice::{Bignum, Rat};
///
/// let disk = |cx, cy, r2, src| decompose(&Curve::Circle {
///     circle: Circle {
///         cx: Rat::<Bignum>::from_i128(cx),
///         cy: Rat::from_i128(cy),
///         r2: Rat::from_i128(r2),
///     },
///     orient: Orient::Ccw,
///     source: CurveId(src),
/// });
/// let mut edges = disk(0, 0, 25, 0);
/// edges.extend(disk(8, 0, 25, 1));
/// let operand_of = |c: CurveId| if c.0 == 0 { OperandId::A } else { OperandId::B };
///
/// match ledge_dom_certified(&edges, &operand_of, BoolOp::Or) {
///     // The region passed every verified checker; `v_boundary()` / `pinches()` classify
///     // the arrangement vertices.
///     Verdict::Verified(cap) => assert_eq!(cap.region().faces.len(), 1),
///     // A fault means a constructor bug, not unsupported input.
///     Verdict::Refuted(fault) => panic!("CAP-OUT refuted: {fault:?}"),
///     Verdict::Unresolved(()) => unreachable!(),
/// }
/// ```
pub fn ledge_dom_certified<B: Backend>(
    edges: &[Edge<B>],
    operand_of: &impl Fn(CurveId) -> OperandId,
    op: BoolOp,
) -> Verdict<CapOut<B>, CapOutFault, ()> {
    let d = Dcel::build(edges);
    if !d.substrate_link_ok() {
        return Verdict::Refuted(CapOutFault::SubstrateLink);
    }
    let (labels, reps, all_assigned) = slab_locate(&d, operand_of);
    // Refuse a silently-defaulted labeling: every cell must have been located, and the slab
    // decomposition must be generic (critical_ys complete) — else a manufactured outside-label
    // would poison the certificate.
    if !all_assigned || !slab_decomposition_generic(&d) {
        return Verdict::Refuted(CapOutFault::Incomplete);
    }
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

    // (4) separating-edge / boundary-edge coverage count (spec §8.5): every separating
    // edge is emitted exactly once. A count, not a full source-ID bijection (see the
    // `BoundaryEdgeCount` doc + `docs/engineering-log.md`).
    if separating_count(d, &sel) != region_boundary_count(&region) {
        return Verdict::Refuted(CapOutFault::BoundaryEdgeCount);
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

/// The certified region as a plain `Verdict<Region, _, _>` — a thin wrapper over
/// [`ledge_dom_certified`] that drops the [`CapOut`] classification and collapses any
/// [`CapOutFault`] to `Unresolved` (this signature's `Refuted` type is
/// [`core::convert::Infallible`]). Use when you only want the region and a pass/fail.
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
// CAP-OUT-LINK (spec §8.5) — the searcher side of the V_∂ / manifold classifier. At each
// vertex the incident faces, taken in azimuth order, give a cyclic sector-selected mask;
// `certify_core::arrange::classify_link` classifies it. The order is `dir_cmp`
// (geometric), so the per-vertex class is frame-invariant — which is what makes the
// tangency case, whose raw face count is not frame-invariant, well-defined.
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
    let (labels, _reps, _) = slab_locate(&d, operand_of);
    let sel: Vec<bool> = labels.iter().map(|&l| op.select(l)).collect();
    link_classes(&d, &sel).contains(&LinkClass::Pinch)
}

// ---------------------------------------------------------------------------
// Link_emitted ≅ Link_geometric + completeness bijections (spec §8.5).
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

/// Check `Link_emitted(v) ≅ Link_geometric(v)` at every vertex (spec §8.5): the stored
/// face-cycle rotation equals the geometric azimuth sort as an identity-fixing
/// oriented cyclic isomorphism (via the Kani-proven
/// `certify_core::arrange::link_iso_ok`). Audits that the DCEL's rotation wiring matches
/// the true geometry at each vertex.
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

/// Total boundary edges a region emits (outer loops + holes) — the emitted side of the
/// CAP-OUT separating-edge / boundary-edge coverage count (spec §8.5).
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

    /// Disjoint operands: two separate unit disks (centres 3 apart). Because every cell is
    /// point-located independently, the disconnected arrangement still labels consistently
    /// and the cocycle closes. Cells: outside `(0,0)`, disk-A `(1,0)`, disk-B `(0,1)` — no
    /// `(1,1)` overlap cell. ∪ = two disks, ∩ = empty, △ = two disks.
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

    /// Nested operands (annulus): concentric disks, the inner strictly inside the outer.
    /// The three cells — outside `(0,0)`, the annulus between them `(1,0)` (in A = outer,
    /// not B = inner), and the inner disk `(1,1)` — label consistently. ∩ = the inner
    /// disk, △ = the annulus, ∪ = the outer disk.
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

        // △ = the annulus: ONE face (the ring) with ONE hole (the inner disk) — hole
        // nesting collapses what would otherwise be two separate boundary loops.
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

    /// CAP-OUT-LINK detects the internal-tangency pinch: c1=(0,0,4) r=2 and c2=(1,0,1) r=1
    /// are internally tangent at (2,0). Their △ (the crescent) pinches to a point there —
    /// CAP-OUT-LINK classifies that vertex as a non-manifold `Pinch`, while ∪ (the outer
    /// disk) and ∩ (the inner disk) are smooth at the touch (no pinch).
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

    /// The tangency pinch is frame-invariant: whether △ pinches survives every rational
    /// rigid motion, even though the motion moves the tangent point off the x-extremum and
    /// re-splits the decomposition (the raw face count is not frame-invariant, but the
    /// pinch classification is). External tangency likewise pinches under △.
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
        let (labels, _, _) = slab_locate(&d, &ab);
        let sel: Vec<bool> = labels.iter().map(|&l| BoolOp::Xor.select(l)).collect();
        link_classes(&d, &sel)
            .iter()
            .filter(|c| **c == LinkClass::Pinch)
            .count()
    }

    /// The pinch count of a △ is the same in every frame, even where the raw vertex count
    /// is not — a rigid motion moves a tangent point off the x-extremum (adding smooth
    /// extremum vertices) but never changes how many vertices are genuine non-manifold
    /// pinches.
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

    /// Link_emitted ≅ Link_geometric: at every vertex of every corpus arrangement, the
    /// stored rotation order equals the geometric azimuth sort as an identity-fixing
    /// oriented cyclic isomorphism.
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

    /// Separating-edge / boundary-edge coverage count (CAP-OUT): for every op on every
    /// corpus config, the number of separating (selected|unselected) edges equals the
    /// total edges the region emits across its outer loops and holes.
    #[test]
    fn separating_boundary_edge_count() {
        for e in corpus() {
            let d = Dcel::build(&e);
            let (labels, _, _) = slab_locate(&d, &ab);
            for op in [BoolOp::Xor, BoolOp::And, BoolOp::Or] {
                let sel: Vec<bool> = labels.iter().map(|&l| op.select(l)).collect();
                let r = ledge_dom(&e, &ab, op);
                assert_eq!(
                    separating_count(&d, &sel),
                    region_boundary_count(&r),
                    "separating / boundary edge coverage count ({op:?})"
                );
            }
        }
    }

    /// The certified entry accepts the whole corpus (transverse, disjoint, annulus,
    /// tangency) for every op — all four CAP-OUT gates (substrate-link, cocycle, Link≅geom,
    /// boundary-edge count) pass over the emitted region.
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
        let (mut labels, reps, _) = slab_locate(&d, &ab);
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

    /// The certified-path completeness gate does not false-fire on a valid arrangement: the
    /// slab decomposition is generic and `slab_locate` assigns every cycle a label (so
    /// `CapOutFault::Incomplete` never triggers for correctly-built input — the corpus
    /// certifies).
    #[test]
    fn slab_decomposition_complete_on_valid_arrangement() {
        let d = Dcel::build(&two_disks());
        assert!(slab_decomposition_generic(&d), "critical_ys is complete");
        let (_labels, _reps, all_assigned) = slab_locate(&d, &ab);
        assert!(all_assigned, "every cycle located");
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
        /// motion — the whole pipeline is frame-independent across the full regime:
        /// transverse, tangent, disjoint, nested, identical.
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
