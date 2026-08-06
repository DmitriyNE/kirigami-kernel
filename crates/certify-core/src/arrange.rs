//! Pure arrangement checkers.
//!
//! These are the trusted, formally-verified core of the `arrange2d` boolean engine.
//! The engine itself is an untrusted searcher; it emits a flat index-array
//! certificate, and these checkers re-derive its correctness. Each is pure, total,
//! `no_std`, panic-free, and Kani-proven — they form the extraction surface into
//! Lean. They consume only index arrays and bitmasks, never coordinates.
//!
//! - [`cocycle_ok`] — the ℤ₂² cocycle-closure check (spec §6 step 4): every closed
//!   walk returns its bits, catching a mis-paired-twin / dropped-event defect no
//!   local check sees. Proven by `cocycle_implies_telescoping`.
//! - [`classify_link`] / [`v_boundary`] / [`link_ok`] — CAP-OUT-LINK (spec §8.5):
//!   classify a vertex from its cyclic sector-selected mask and compute `V_∂`
//!   membership. `link_ok` is the frame-invariant **strict-manifold (no-pinch)
//!   predicate**: it is `false` exactly on a pinch, which is why a pinch is `v ∉ V_∂`.
//!   It is a per-vertex *classifier*, not a region gate — a pinched region (e.g. a
//!   transverse `△`) is a valid, correctly-emitted result; whether to refuse one is the
//!   consumer's policy (ultimately SEW-LINK's), not CAP-OUT's. Proven by
//!   `link_ok_iff_no_pinch`.
//! - [`link_iso_ok`] — `Link_emitted ≅ Link_geometric` (spec §8.5): the two cyclic
//!   orders agree as an identity-fixing oriented isomorphism.
//!
//! The DCEL / boolean searcher these certify lives in the `arrange2d` crate.

/// The ℤ₂² cocycle-closure check (spec §6 step 4) over a flat, index-array
/// certificate of the DCEL bit propagation. Pure, total, and **panic-free** (all
/// indexing is checked) — the `certify_core` TCB / Rust→Lean extraction surface.
///
/// A cell's `(A, B)` label packs into a `u8`: bit 0 = A, bit 1 = B. An edge's flip
/// packs the same (bit 0 = ∂F_A, bit 1 = ∂F_B; a coincident edge = `0b11`). The
/// searcher supplies a labeling; this certifies it is a valid ℤ₂² cochain:
/// - the arrays are well-shaped and every index is in range;
/// - every label and every flip is in the ℤ₂² domain (only bits 0–1 set) — a value
///   with any higher bit is a malformed certificate, not a member of ℤ₂², and is
///   rejected rather than trusted (the checker enforces the domain the
///   `cocycle_implies_telescoping` proof assumes, instead of trusting the constructor);
/// - the seed (unbounded) cell is `(0, 0)`;
/// - crossing each undirected edge flips exactly its bits: `labels[a] ^ flip ==
///   labels[b]`.
///
/// Local edge-consistency of a labeling is **equivalent to global cocycle closure**
/// (every closed walk telescopes its flips to `0` — the `cocycle_implies_telescoping`
/// Kani proof), so acceptance certifies there is no frustrated cycle: the
/// mis-paired-twin / dropped-event defect class the spec calls out. It does **not**
/// certify the region (that is CAP-OUT) — only the overlay's ℤ₂² integrity.
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
    // ℤ₂² domain: every label is a two-bit value. A high bit has no meaning in ℤ₂², so
    // its presence is a malformed certificate — reject rather than trust the constructor.
    let mut li = 0;
    while li < labels.len() {
        if labels[li] & !0b11 != 0 {
            return false;
        }
        li += 1;
    }
    // The unbounded cell is anchored at (A, B) = (0, 0).
    match labels.get(seed) {
        Some(&0) => {}
        _ => return false,
    }
    // Every edge is locally consistent: its flip is a two-bit value, and crossing it
    // flips exactly those bits.
    let mut i = 0;
    while i < m {
        if edge_flip[i] & !0b11 != 0 {
            return false;
        }
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
/// §8.5 CAP-OUT-LINK). This is the frame-invariant manifold test: the classification
/// depends only on the geometric sector order, never on the axis-aligned
/// decomposition, so the tangency case is well-defined regardless of frame.
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
    /// internal-tangency witness). CAP-OUT-LINK excludes it from `V_∂` (spec: "π₀ keeps
    /// them separate, CAP-OUT-LINK rejects the vertex" — the vertex, not the region). The
    /// pinched region is still valid and correctly emitted; the manifold requirement it
    /// fails is enforced downstream where a shell is actually sewn (SEW-LINK), not here.
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
/// boundary (`v ∈ V_∂`), two or more ⇒ pinch (`v ∉ V_∂`). Pure, `no_std`, panic-free —
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

/// The strict-manifold (no-pinch) predicate at `v`: `true` iff the link is a
/// 2-manifold-with-boundary — not a pinch (spec §8.5). This is a *classifier* feeding
/// `V_∂` membership and the downstream SEW-LINK gate, **not** a CAP-OUT region gate: the
/// certified boolean (`arrange2d::boolean::ledge_dom_certified`) does not refuse a region
/// on `!link_ok`, because a pinch (e.g. a transverse `△`) is a valid emitted result.
pub fn link_ok(sectors: &[bool]) -> bool {
    !matches!(classify_link(sectors), LinkClass::Pinch)
}

/// Whether `xs` has a repeated element (a nested index scan accumulating a flag — no early
/// return from the inner loop, which the Aeneas lift does not support).
fn has_duplicate(xs: &[usize]) -> bool {
    let n = xs.len();
    let mut found = false;
    let mut i = 0;
    while i < n {
        let mut j = i + 1;
        while j < n {
            if xs[i] == xs[j] {
                found = true;
            }
            j += 1;
        }
        i += 1;
    }
    found
}

/// `Link_emitted(v) ≅ Link_geometric(v)` (spec §8.5): do the two cyclic
/// orderings `a` (the stored face-cycle walk) and `b` (the geometric azimuth sort) of
/// the incident edges around `v` agree as an **identity-fixing oriented cyclic
/// isomorphism** — i.e. `a` is a cyclic rotation of `b` (same elements, same cyclic
/// order and orientation)? This is the *audit* the spec insists on over a mere count:
/// `a → c → b → d` has the right multiset yet crosses, and is rejected here. Pure,
/// `no_std`, panic-free; Kani-**validated for N=4 permutations**
/// (`link_iso_matches_cyclic_adjacency`). Both inputs are validated to be genuine
/// permutations (no repeated element) — the harness's precondition, now enforced by the
/// deployed checker so a malformed link cannot slip through the rotation search. The
/// unbounded (all-N) proof remains a tracked follow-up (`docs/engineering-log.md`).
pub fn link_iso_ok(a: &[usize], b: &[usize]) -> bool {
    let n = a.len();
    if b.len() != n {
        return false;
    }
    if n == 0 {
        return true;
    }
    // Both must be genuine permutations (no repeated incident edge) — the isomorphism is
    // between two orderings of the same set, so a duplicate is malformed. Enforcing the
    // harness's precondition keeps the deployed checker in sync with its proof.
    if has_duplicate(a) || has_duplicate(b) {
        return false;
    }
    // Some rotation `off` of `b` matches `a` position-for-position.
    let mut off = 0;
    while off < n {
        let mut matched = true;
        let mut i = 0;
        while i < n {
            if a[i] != b[(i + off) % n] {
                matched = false;
                break;
            }
            i += 1;
        }
        if matched {
            return true;
        }
        off += 1;
    }
    false
}

/// Whether `xs` contains `v` (a linear scan, extraction-friendly).
fn slice_contains(xs: &[usize], v: usize) -> bool {
    let mut i = 0;
    while i < xs.len() {
        if xs[i] == v {
            return true;
        }
        i += 1;
    }
    false
}

/// The CAP-OUT completeness bijection `{separating edges} ↔ {emitted boundary edges}` (spec
/// §8.5): is `emitted` a **permutation** of `separating`? Both are stable source-edge ids;
/// `separating` is duplicate-free by construction (one id per undirected edge). Stronger than
/// a mere count — a drop-one-and-duplicate-another pair leaves the count unchanged but fails
/// this: `emitted` must have no duplicate and every id must be a separating edge, so with
/// equal length it is exactly the separating set. Pure, `no_std`, panic-free.
pub fn boundary_bijection_ok(separating: &[usize], emitted: &[usize]) -> bool {
    if emitted.len() != separating.len() {
        return false;
    }
    if has_duplicate(emitted) {
        return false;
    }
    // Every emitted id is a separating edge (⊆); equal size + no duplicates ⇒ a permutation.
    let mut i = 0;
    while i < emitted.len() {
        if !slice_contains(separating, emitted[i]) {
            return false;
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

    #[test]
    fn rejects_values_outside_z2xz2() {
        // A high bit (bit 2+) has no meaning in ℤ₂². The XOR equation alone cannot catch
        // it — `0 ^ 4 == 4` holds — so the domain must be checked explicitly. A checker
        // that merely trusts the constructor's two-bit packing would accept this.
        let labels = [0u8, 4]; // cell 1's label carries bit 2 — not a ℤ₂² value
        let ea = [0usize];
        let eb = [1usize];
        let ef = [0b100u8]; // flip carries bit 2; 0 ^ 4 == 4 satisfies the XOR check
        assert!(
            !cocycle_ok(2, &labels, 0, &ea, &eb, &ef),
            "an out-of-domain (non-ℤ₂²) label/flip must be rejected"
        );
        // An out-of-domain flip alone (labels in-domain) is likewise rejected.
        assert!(!cocycle_ok(2, &[0u8, 0], 0, &ea, &eb, &[0b100u8]));
        // The in-domain counterpart of the same shape is accepted.
        assert!(cocycle_ok(2, &[0u8, 1], 0, &ea, &eb, &[0b01u8]));
    }

    // --- CAP-OUT-LINK: V_∂ classification ---

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

    // --- Link_emitted ≅ Link_geometric ---

    #[test]
    fn link_iso_accepts_rotations() {
        assert!(link_iso_ok(&[0, 1, 2, 3], &[0, 1, 2, 3])); // identical
        assert!(link_iso_ok(&[0, 1, 2, 3], &[2, 3, 0, 1])); // rotation
        assert!(link_iso_ok(&[0, 1, 2, 3], &[3, 0, 1, 2])); // rotation
        assert!(link_iso_ok(&[], &[])); // empty
        assert!(link_iso_ok(&[5], &[5])); // singleton
    }

    #[test]
    fn link_iso_rejects_crossings_and_mismatch() {
        // The spec's crossing counterexample: same multiset, different cyclic order.
        assert!(!link_iso_ok(&[0, 1, 2, 3], &[0, 2, 1, 3]));
        // Reversed orientation is not an identity-fixing *oriented* iso.
        assert!(!link_iso_ok(&[0, 1, 2, 3], &[0, 3, 2, 1]));
        // Different length / different elements.
        assert!(!link_iso_ok(&[0, 1, 2], &[0, 1, 2, 3]));
        assert!(!link_iso_ok(&[0, 1, 2], &[0, 1, 4]));
    }

    #[test]
    fn link_iso_rejects_non_permutations() {
        // A repeated element is malformed input — rejected even where a rotation would
        // otherwise "match" (the rotation search assumes genuine permutations).
        assert!(!link_iso_ok(&[0, 1, 1], &[0, 1, 1]));
        assert!(!link_iso_ok(&[0, 1, 2], &[1, 1, 2]));
        // Genuine permutations still round-trip.
        assert!(link_iso_ok(&[0, 1, 2, 3], &[2, 3, 0, 1]));
    }

    #[test]
    fn boundary_bijection_catches_drop_and_duplicate() {
        // A permutation of the separating set is accepted (order irrelevant).
        assert!(boundary_bijection_ok(&[0, 1, 2, 3], &[2, 0, 3, 1]));
        // Drop `1`, duplicate `2`: the COUNT is unchanged (4 vs 4) but the bijection fails —
        // exactly the defect a coverage count misses.
        assert!(!boundary_bijection_ok(&[0, 1, 2, 3], &[0, 2, 2, 3]));
        // An emitted id that is not a separating edge.
        assert!(!boundary_bijection_ok(&[0, 1, 2], &[0, 1, 9]));
        // Length mismatch (a dropped edge with no duplicate).
        assert!(!boundary_bijection_ok(&[0, 1, 2], &[0, 1]));
    }
}
