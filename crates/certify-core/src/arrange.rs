//! Pure arrangement checkers.
//!
//! The **ℤ₂² cocycle-closure check** ([`cocycle_ok`], spec §6 step 4 — every closed
//! walk returns its bits, a kernel-defect detector no local check sees) lands at
//! slice 3d; it is proven by Kani (bounded DCEL bookkeeping, `vv-guide §5`) over the
//! flat index-array certificate the `arrange2d` searcher emits.
//!
//! The CAP-OUT completeness bijections (components ↔ faces, separating edges ↔
//! boundary edges, V_∂ ↔ emitted vertices), CAP-OUT-LINK with V_∂-membership
//! computation, and `Link_emitted ≅ Link_geometric` as an identity-fixing oriented
//! isomorphism are the **region** certificate — slice 3e. The DCEL / boolean
//! searcher lives in the `arrange2d` crate.

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
}
