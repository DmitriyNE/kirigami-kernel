//! Kani bounded-model-checking harnesses for the pure checkers (compiled only under
//! `cargo kani`; see `vv-guide §5/§8`). This is the first Kani surface outside
//! `lattice` — the slice-3d ℤ₂² cocycle proof.

use crate::arrange::{cocycle_ok, link_ok, v_boundary};

// The soundness of the cocycle checker, as **bounded DCEL bookkeeping** (vv-guide
// §5): if `cocycle_ok` accepts a labeling, then bit propagation is *path
// independent* — any walk accumulates flips telescoping to `labels[start] ^
// labels[end]`, so **every closed walk returns its bits** (acc = 0). That is exactly
// the spec §6 step-4 cocycle-closure property ("every closed walk returns its bits —
// a kernel-defect detector no local check sees"). Proven exhaustively over all
// arrangements up to `NC` cells / `NE` edges: since any simple closed walk in such a
// graph has ≤ `NC` edges, `NW = NC` steps cover every cycle, so the bounded proof is
// complete for graphs of that size. `unwind(NE + 1)` bounds the checker's edge loop.
#[kani::proof]
#[kani::unwind(6)]
fn cocycle_implies_telescoping() {
    const NC: usize = 4; // cells
    const NE: usize = 5; // edges
    const NW: usize = 4; // walk steps (≥ NC covers every simple cycle)

    // An arbitrary labeling (2-bit labels) and edge set (in-range, 2-bit flips).
    let labels: [u8; NC] = kani::any();
    for &l in &labels {
        kani::assume(l < 4);
    }
    let ea: [usize; NE] = kani::any();
    let eb: [usize; NE] = kani::any();
    let ef: [u8; NE] = kani::any();
    for i in 0..NE {
        kani::assume(ea[i] < NC && eb[i] < NC && ef[i] < 4);
    }
    let seed: usize = kani::any();
    kani::assume(seed < NC);

    // Precondition: the checker accepts this labeling.
    kani::assume(cocycle_ok(NC, &labels, seed, &ea, &eb, &ef));

    // An arbitrary walk: from `start`, each step crosses an incident edge (in either
    // direction) to the other endpoint, accumulating its flip.
    let start: usize = kani::any();
    kani::assume(start < NC);
    let steps: [usize; NW] = kani::any();
    let dir: [bool; NW] = kani::any();

    let mut cur = start;
    let mut acc: u8 = 0;
    for i in 0..NW {
        let e = steps[i];
        kani::assume(e < NE);
        let (from, to) = if dir[i] {
            (ea[e], eb[e])
        } else {
            (eb[e], ea[e])
        };
        kani::assume(from == cur); // the edge is incident to the current cell
        acc ^= ef[e];
        cur = to;
    }

    // Telescoping ⇒ (closed walk: cur == start) ⇒ acc == 0. The general form:
    assert!(acc == (labels[start] ^ labels[cur]));
}

// Soundness of the CAP-OUT-LINK classifier (spec §8.5, slice 3e): the streaming O(n)
// run-counter `link_ok`/`v_boundary` agrees with the exhaustive O(n²) pinch definition
// over **all** cyclic sector masks up to `N` sectors. The pinch definition is
// independent (a pairwise search for two selected sectors separated by an unselected
// one on *both* cyclic arcs — a non-manifold link), so this is a genuine equivalence,
// not a restatement. `N` = 6 bounds a vertex's incident-face count in the D24 corpus
// (`vv-guide §5`, bounded link bookkeeping); the unwind bounds the ≤N loops.

/// Is there an unselected sector strictly between `i` and `j` on the forward cyclic arc?
fn arc_has_false<const N: usize>(s: &[bool; N], i: usize, j: usize) -> bool {
    let mut k = (i + 1) % N;
    while k != j {
        if !s[k] {
            return true;
        }
        k = (k + 1) % N;
    }
    false
}

/// The reference pinch predicate: two selected sectors separated by an unselected one
/// on both arcs ⇒ ≥2 disjoint selected intervals ⇒ a non-manifold pinch.
fn ref_has_pinch<const N: usize>(s: &[bool; N]) -> bool {
    let mut i = 0;
    while i < N {
        let mut j = 0;
        while j < N {
            if i != j && s[i] && s[j] && arc_has_false(s, i, j) && arc_has_false(s, j, i) {
                return true;
            }
            j += 1;
        }
        i += 1;
    }
    false
}

#[kani::proof]
#[kani::unwind(8)]
fn link_ok_iff_no_pinch() {
    const N: usize = 6;
    let s: [bool; N] = kani::any();

    // CAP-OUT-LINK accepts iff the link is not a pinch.
    assert!(link_ok(&s) == !ref_has_pinch(&s));

    // v ∈ V_∂ iff exactly one *proper* interval: no pinch, and a genuine boundary
    // (some selected sector and some unselected).
    let mut has_t = false;
    let mut has_f = false;
    let mut i = 0;
    while i < N {
        has_t |= s[i];
        has_f |= !s[i];
        i += 1;
    }
    assert!(v_boundary(&s) == (!ref_has_pinch(&s) && has_t && has_f));
}
