//! Kani bounded-model-checking harnesses for the pure checkers (compiled only under
//! `cargo kani`; see `vv-guide §5/§8`). This is the first Kani surface outside
//! `lattice` — the slice-3d ℤ₂² cocycle proof.

use crate::arrange::cocycle_ok;

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
