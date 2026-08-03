//! Pure arrangement checkers.
//!
//! The **ℤ₂² cocycle-closure check** ([`cocycle_ok`], spec §6 step 4 — every closed
//! walk returns its bits, a kernel-defect detector no local check sees) lands at
//! slice 3d; it is proven by Kani (bounded DCEL bookkeeping, `vv-guide §5`) over the
//! flat index-array certificate the `arrange2d` searcher emits.
//!
//! **CAP-OUT-LINK** ([`classify_link`] / [`v_boundary`] / [`link_ok`], spec §8.5,
//! slice 3e.2) classifies a vertex from its cyclic sector-selected mask and computes
//! `V_∂` membership — the frame-invariant manifold/pinch test, Kani-proven
//! (`link_ok_iff_no_pinch`). Still to come in slice 3e: `Link_emitted ≅
//! Link_geometric` as an identity-fixing oriented isomorphism (3e.3) and the CAP-OUT
//! completeness bijections (components ↔ faces, separating edges ↔ boundary edges,
//! V_∂ ↔ emitted vertices). The DCEL / boolean searcher lives in the `arrange2d` crate.

/// The ℤ₂² cocycle-closure check (spec §6 step 4) over a flat, index-array
/// certificate of the DCEL bit propagation. Pure, total, and **panic-free** (all
/// indexing is checked) — the `certify_core` TCB / Rust→Lean extraction surface.
///
/// A cell's `(A, B)` label packs into a `u8`: bit 0 = A, bit 1 = B. An edge's flip
/// packs the same (bit 0 = ∂F_A, bit 1 = ∂F_B; a coincident edge = `0b11`). The
/// searcher supplies a labeling; this certifies it is a valid ℤ₂² cochain:
/// - the arrays are well-shaped and every index is in range;
/// - the seed (unbounded) cell is `(0, 0)`;
/// - crossing each undirected edge flips exactly its bits: `labels[a] ^ flip ==
///   labels[b]`.
///
/// Local edge-consistency of a labeling is **equivalent to global cocycle closure**
/// (every closed walk telescopes its flips to `0` — the `cocycle_implies_telescoping`
/// Kani proof), so acceptance certifies there is no frustrated cycle: the
/// mis-paired-twin / dropped-event defect class the spec calls out. It does **not**
/// certify the region (that is CAP-OUT, 3e) — only the overlay's ℤ₂² integrity.
pub fn cocycle_ok(
    n_cells: usize,
    labels: &[u8],
    seed: usize,
    edge_a: &[usize],
    edge_b: &[usize],
    edge_flip: &[u8],
) -> bool {
    // Shape: one label per cell, and the three edge arrays share a length.
    if labels.len() != n_cells {
        return false;
    }
    let m = edge_a.len();
    if edge_b.len() != m || edge_flip.len() != m {
        return false;
    }
    // The unbounded cell is anchored at (A, B) = (0, 0).
    match labels.get(seed) {
        Some(&0) => {}
        _ => return false,
    }
    // Every edge is locally consistent: crossing it flips exactly its bits.
    let mut i = 0;
    while i < m {
        match (labels.get(edge_a[i]), labels.get(edge_b[i])) {
            (Some(&la), Some(&lb)) => {
                if la ^ edge_flip[i] != lb {
                    return false;
                }
            }
            _ => return false,
        }
        i += 1;
    }
    true
}

/// The local classification of a vertex's link from its **cyclic sector-selected
/// mask** — the selection bits of the faces incident to `v`, in azimuth order (spec
/// §8.5 CAP-OUT-LINK, slice 3e). This is the frame-invariant manifold test: the
/// classification depends only on the geometric sector order, never on the axis-aligned
/// decomposition, so it resolves the tangency case 3d left frame-dependent.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LinkClass {
    /// No selected sector — `v` is exterior to the selected region (`v ∉ V_∂`).
    Exterior,
    /// Every sector selected — `v` is interior (`v ∉ V_∂`).
    Interior,
    /// Exactly one *proper* cyclic interval of selected sectors — a manifold boundary
    /// vertex (`v ∈ V_∂`).
    Boundary,
    /// Two or more disjoint selected intervals — a non-manifold **pinch** (the
    /// internal-tangency witness). CAP-OUT-LINK rejects it (spec: "disconnected ⇒
    /// reject"; "π₀ keeps them separate, CAP-OUT-LINK rejects the vertex").
    Pinch,
}

/// The number of maximal cyclic runs of `true` in `sectors` (a full ring counts as one
/// run, an empty ring as zero). Pure, total, panic-free.
fn cyclic_true_runs(sectors: &[bool]) -> usize {
    let n = sectors.len();
    if n == 0 {
        return 0;
    }
    let mut runs = 0usize;
    let mut i = 0;
    while i < n {
        // A run starts at each `false → true` transition (cyclically).
        if sectors[i] && !sectors[(i + n - 1) % n] {
            runs += 1;
        }
        i += 1;
    }
    // An all-`true` ring has no transition but is one full run.
    if runs == 0 && sectors[0] { 1 } else { runs }
}

/// Classify a vertex's link from its cyclic sector-selected mask (spec §8.5
/// CAP-OUT-LINK): `0` runs ⇒ exterior, a full ring ⇒ interior, one proper interval ⇒
/// boundary (`v ∈ V_∂`), two or more ⇒ pinch (reject). Pure, `no_std`, panic-free —
/// the `certify_core` TCB / extraction surface; Kani-proven (`link_ok_iff_no_pinch`).
pub fn classify_link(sectors: &[bool]) -> LinkClass {
    match cyclic_true_runs(sectors) {
        0 => LinkClass::Exterior,
        1 => {
            if all_true(sectors) {
                LinkClass::Interior
            } else {
                LinkClass::Boundary
            }
        }
        _ => LinkClass::Pinch,
    }
}

/// Every sector selected (checked without an iterator adaptor, for the extraction path).
fn all_true(sectors: &[bool]) -> bool {
    let mut i = 0;
    while i < sectors.len() {
        if !sectors[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// `v ∈ V_∂`: the selected sectors form exactly one proper cyclic interval (spec §8.5:
/// "selected sectors form one proper cyclic interval ⇒ v ∈ V_∂").
pub fn v_boundary(sectors: &[bool]) -> bool {
    matches!(classify_link(sectors), LinkClass::Boundary)
}

/// CAP-OUT-LINK acceptance at `v`: a 2-manifold-with-boundary link — not a pinch (spec
/// §8.5: "disconnected ⇒ reject", manifoldness is a property of the *selected region*).
pub fn link_ok(sectors: &[bool]) -> bool {
    !matches!(classify_link(sectors), LinkClass::Pinch)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A valid labeling of the two-overlapping-disks cells: outside (0,0)=0,
    /// A-lune (1,0)=1, B-lune (0,1)=2, lens (1,1)=3. Edges: outside|A-lune (flip A),
    /// outside|B-lune (flip B), A-lune|lens (flip B), B-lune|lens (flip A).
    #[test]
    fn accepts_consistent_labeling() {
        let labels = [0u8, 1, 2, 3]; // outside, A, B, lens
        let edge_a = [0usize, 0, 1, 2];
        let edge_b = [1usize, 2, 3, 3];
        let edge_flip = [0b01u8, 0b10, 0b10, 0b01];
        assert!(cocycle_ok(4, &labels, 0, &edge_a, &edge_b, &edge_flip));
    }

    /// A frustrated triangle (flips 1,1,1 around a 3-cycle) admits no consistent
    /// labeling — whatever labels the searcher offers, some edge fails.
    #[test]
    fn rejects_frustrated_cycle() {
        // any labeling of a triangle whose three edges each flip A once: the parity
        // around the cycle is odd, so no assignment is consistent.
        let edge_a = [0usize, 1, 2];
        let edge_b = [1usize, 2, 0];
        let edge_flip = [0b01u8, 0b01, 0b01];
        for l0 in 0..4u8 {
            for l1 in 0..4u8 {
                for l2 in 0..4u8 {
                    let labels = [l0, l1, l2];
                    // seed must be (0,0); only labelings with a (0,0) cell qualify,
                    // and none of those can also satisfy the odd cycle.
                    let seed = 0;
                    if labels[seed] == 0 {
                        assert!(
                            !cocycle_ok(3, &labels, seed, &edge_a, &edge_b, &edge_flip),
                            "frustrated cycle must be rejected: {:?}",
                            labels
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn rejects_bad_seed_and_shapes() {
        let labels = [0u8, 1];
        let ea = [0usize];
        let eb = [1usize];
        let ef = [0b01u8];
        // wrong seed label (cell 1 is (1,0), not (0,0))
        assert!(!cocycle_ok(2, &labels, 1, &ea, &eb, &ef));
        // seed out of range
        assert!(!cocycle_ok(2, &labels, 9, &ea, &eb, &ef));
        // labels length mismatch
        assert!(!cocycle_ok(3, &labels, 0, &ea, &eb, &ef));
        // edge index out of range
        assert!(!cocycle_ok(2, &labels, 0, &[0], &[5], &ef));
        // mismatched edge array lengths
        assert!(!cocycle_ok(2, &labels, 0, &[0, 1], &[1], &ef));
    }

    // --- CAP-OUT-LINK: V_∂ classification (3e.2) ---

    #[test]
    fn link_exterior_and_interior() {
        assert_eq!(
            classify_link(&[false, false, false, false]),
            LinkClass::Exterior
        );
        assert_eq!(
            classify_link(&[true, true, true, true]),
            LinkClass::Interior
        );
        assert_eq!(classify_link(&[]), LinkClass::Exterior);
        assert!(!v_boundary(&[false, false]));
        assert!(!v_boundary(&[true, true]));
        assert!(link_ok(&[false; 4]) && link_ok(&[true; 4]));
    }

    #[test]
    fn link_boundary_one_interval() {
        // One contiguous cyclic run of selected sectors ⇒ a manifold boundary vertex.
        assert_eq!(
            classify_link(&[true, true, false, false]),
            LinkClass::Boundary
        );
        assert_eq!(
            classify_link(&[false, true, true, false]),
            LinkClass::Boundary
        );
        // The run may wrap around the end of the array (still one cyclic interval).
        assert_eq!(
            classify_link(&[true, false, false, true]),
            LinkClass::Boundary
        );
        assert!(v_boundary(&[true, true, false, false]));
        assert!(link_ok(&[true, false, false, true]));
    }

    #[test]
    fn link_pinch_two_intervals() {
        // Two disjoint selected runs separated by unselected on both sides — the
        // internal-tangency pinch. Rejected (v ∉ V_∂, not manifold).
        assert_eq!(classify_link(&[true, false, true, false]), LinkClass::Pinch);
        assert_eq!(
            classify_link(&[true, true, false, true, true, false]),
            LinkClass::Pinch
        );
        assert!(!link_ok(&[true, false, true, false]));
        assert!(!v_boundary(&[true, false, true, false]));
    }
}
