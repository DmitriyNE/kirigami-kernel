//! Kani bounded-model-checking harnesses for the pure checkers (compiled only under
//! `cargo kani`; see `vv-guide §5/§8`). This is the first Kani surface outside
//! `lattice` — the slice-3d ℤ₂² cocycle proof.

use crate::arrange::{cocycle_ok, link_iso_ok, link_ok, v_boundary};
use crate::certify1d::{ClipBranch, clip_sigma_branch, corner_range};

// Soundness of the ★ CLIP-σ signed disjunction (spec §8.5). `clip_sigma` ranges the
// **signed** affine `∂_σG` over four box corners and certifies a single sign; the row
// exists because the tempting *squared* `|∂_σG|² ≥ m` test is unsound — an affine form
// minimizes in the box interior, so `G = σμ` (whose `∂_σG = μ` vanishes on `μ = 0`)
// passes the corner test with margin while the crossing is singular. This proves the
// signed test does not: a certified verdict forces **every** corner strictly single-
// signed and separated, so any mixed-sign corner set (the `σμ` class) is rejected.
//
// The decision is factored (`corner_range` ∘ `clip_sigma_branch`, both generic over the
// order) so the proof runs on `i128` — the exact functions `clip_sigma` applies at
// `T = Rat`. Running Kani through `Rat<Bignum>` instead is a trap: the two-tier fast/slow
// dispatch's dead `Slow` branch drags in dashu's unbounded `gcd` loop, which CBMC unwinds
// forever. That the `i128` order and `Rat`'s order agree is `lattice`'s obligation (its
// `cmp` panic-freedom + differential proofs), cleanly separated from this logic proof.
// `i32` corners bound the state; the property is scale-free, so the bound loses nothing.
#[kani::proof]
#[kani::unwind(6)]
fn clip_sigma_signed_disjunction_sound() {
    let cs32: [i32; 4] = kani::any();
    let m32: i32 = kani::any();
    // Widen to i128 so the margin negation `-m` never overflows (i32::MIN as i128 negates
    // cleanly); this mirrors the exact rationals `clip_sigma` builds via `Rat::from_i128`.
    let cs: [i128; 4] = [
        cs32[0] as i128,
        cs32[1] as i128,
        cs32[2] as i128,
        cs32[3] as i128,
    ];
    let m = m32 as i128;

    // The exact body of `clip_sigma`, at T = i128.
    let (lo, hi) = corner_range(&cs).expect("four corners is non-empty");
    let neg_m = -m;
    let branch = clip_sigma_branch(&lo, &hi, &m, &neg_m, m > 0);

    // Soundness: a certified sign is the true single sign of *all* corners, separated by a
    // positive margin.
    match branch {
        Some(ClipBranch::Positive) => {
            assert!(m > 0);
            for &c in &cs {
                assert!(c >= m);
            }
        }
        Some(ClipBranch::Negative) => {
            assert!(m > 0);
            for &c in &cs {
                assert!(c <= -m);
            }
        }
        None => {} // the honest three-valued middle carries no obligation
    }

    // The `σμ` falsely-certifying class, stated directly: a mixed-sign corner set — some
    // `∂_σG > 0` and some `∂_σG < 0` — is never certified.
    let has_pos = cs.iter().any(|&c| c > 0);
    let has_neg = cs.iter().any(|&c| c < 0);
    if has_pos && has_neg {
        assert!(branch.is_none());
    }
}

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

// Soundness of the CAP-OUT-LINK classifier (spec §8.5): the streaming O(n)
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

// Soundness of the `Link_emitted ≅ Link_geometric` checker (spec §8.5):
// over all permutations of `N` incident edges, the rotation-search `link_iso_ok` agrees
// with the independent **cyclic-adjacency** characterization — two cyclic orderings are
// an identity-fixing oriented iso iff every element has the same successor in both. This
// is exactly what distinguishes a rotation from a crossing (`a→c→b→d`), so the proof
// certifies the checker audits order, not just the multiset.

/// The element cyclically after the first occurrence of `x` in a permutation `seq`.
fn succ_of<const N: usize>(seq: &[usize; N], x: usize) -> usize {
    let mut i = 0;
    while i < N {
        if seq[i] == x {
            return seq[(i + 1) % N];
        }
        i += 1;
    }
    x // unreachable when `seq` is a permutation containing `x`
}

#[kani::proof]
#[kani::unwind(6)]
fn link_iso_matches_cyclic_adjacency() {
    const N: usize = 4;
    let a: [usize; N] = kani::any();
    let b: [usize; N] = kani::any();

    // Both are permutations of 0..N (distinct, in range) — the incident-edge set of a
    // vertex, ordered two ways.
    let mut i = 0;
    while i < N {
        kani::assume(a[i] < N && b[i] < N);
        let mut j = i + 1;
        while j < N {
            kani::assume(a[i] != a[j] && b[i] != b[j]);
            j += 1;
        }
        i += 1;
    }

    // Reference: identical cyclic successor for every value ⟺ same cyclic order.
    let mut adj_same = true;
    let mut x = 0;
    while x < N {
        if succ_of(&a, x) != succ_of(&b, x) {
            adj_same = false;
        }
        x += 1;
    }

    assert!(link_iso_ok(&a, &b) == adj_same);
}
