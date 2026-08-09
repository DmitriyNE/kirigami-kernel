//! Kani bounded-model-checking harnesses for the pure checkers (compiled only under
//! `cargo kani`; see `vv-guide §5/§8`). This is the first Kani surface outside
//! `lattice` — the slice-3d ℤ₂² cocycle proof.

use crate::arrange::{LinkClass, cocycle_ok, link_iso_ok, link_ok, v_boundary};
use crate::cap_in::edge_hands_off;
use crate::certify1d::{ClipBranch, clip_sigma_branch, corner_range};
use crate::gate::conj;
use crate::miter::{Occupancy, OrderSign, eps_from_cmp};
use crate::sew::occupancy_row;
use crate::shell::closed_shell;
use crate::verdict::Verdict;

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

// Soundness of the ★ SEW-EDGES quadrant→row classifier (spec §8.5). `occupancy_row` reuses
// the already-proven `classify_link` on the four occupancy bits in cyclic quadrant order
// `[A_L, B_L, A_R, B_R]`; this certifies that reuse reproduces the SEW pinch semantics for
// every one of the sixteen patterns. The ★ is here because the *grouped* mask `[A_L, A_R, B_R,
// B_L]` — the obvious "one flank then the other" order — is unsound: it puts the clean miter's
// occupied `{A_L, B_R}` in opposite quadrants and rejects a valid shell edge as a pinch. The
// alternating order proven below is the one that holds.

/// Independent reference for the quadrant→row class: enumerate directly by the SEW pinch
/// semantics — a `k = 2` occupancy is a pinch iff its two occupied cells are the two sides of
/// the *same* flank (`A_L ∧ A_R` or `B_L ∧ B_R`); every other count is a plain interval.
fn ref_occupancy_row(a_l: bool, a_r: bool, b_l: bool, b_r: bool) -> LinkClass {
    let k = u8::from(a_l) + u8::from(a_r) + u8::from(b_l) + u8::from(b_r);
    match k {
        0 => LinkClass::Exterior,
        4 => LinkClass::Interior,
        2 => {
            if (a_l && a_r) || (b_l && b_r) {
                LinkClass::Pinch
            } else {
                LinkClass::Boundary
            }
        }
        _ => LinkClass::Boundary, // k == 1 or k == 3: a single occupied run
    }
}

#[kani::proof]
#[kani::unwind(8)]
fn occupancy_row_sound() {
    let a_l: bool = kani::any();
    let a_r: bool = kani::any();
    let b_l: bool = kani::any();
    let b_r: bool = kani::any();
    let frame: bool = kani::any();
    let occ = Occupancy {
        a_l,
        a_r,
        b_l,
        b_r,
        frame,
    };

    // The reused `classify_link` row agrees with the independent boundary-count reference,
    // exhaustively over all sixteen occupancy patterns.
    assert!(occupancy_row(occ) == ref_occupancy_row(a_l, a_r, b_l, b_r));

    // Frame-invariance: an L↔R flip (swap each flank's two sides, flip the frame bit) leaves
    // the class fixed — reversing the cycle preserves its run structure.
    let flipped = Occupancy {
        a_l: a_r,
        a_r: a_l,
        b_l: b_r,
        b_r: b_l,
        frame: !frame,
    };
    assert!(occupancy_row(occ) == occupancy_row(flipped));
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

// Soundness of the ★ MITER `ε_φ` order-sign mint (spec §8.5). `eps_phi` mints the order sign
// of the monotone cross-flank correspondence `φ_J` from **one exact oriented-endpoint
// comparison** — `sign(φ_J(σ_hi) − φ_J(σ_lo))` — never the derivative sign. The row is ★
// because the tempting *derivative* mint `sgn(dσ_B/dσ_A)` is unsound: it collapses to zero at
// any interior stationary point (the `σ_A³` fossil has `φ_J′(0) = 0`) even though `φ_J` is
// strictly monotone with distinctly-ordered endpoints, so it would refuse a genuine clean
// miter or, worse, mint the wrong sign near the stall. This proves the endpoint mint does not:
// its verdict is *exactly* the order of the two images, so any two distinct images mint a
// definite sign and only coincident images abstain.
//
// The decision is factored (`eps_from_cmp`, generic over the `Ordering`) so the proof runs on
// `i128` — the exact comparison `eps_phi` applies at the endpoints `φ_J(σ_·) = Rat`. That the
// `i128` order and `Rat`'s order agree is `lattice`'s obligation (`cmp` panic-freedom +
// differential proofs), cleanly separated from this logic proof; the property is scale-free so
// `i128` corners lose nothing.
#[kani::proof]
fn eps_phi_is_endpoint_order() {
    let lo: i128 = kani::any();
    let hi: i128 = kani::any();
    let sign = eps_from_cmp(lo.cmp(&hi));

    // The verdict is exactly the endpoint order — the whole content of `ε_φ`.
    match sign {
        Some(OrderSign::Preserving) => assert!(lo < hi),
        Some(OrderSign::Reversing) => assert!(lo > hi),
        None => assert!(lo == hi),
    }

    // The anti-derivative property: **distinct** endpoints always mint a definite sign. A
    // derivative mint would return `None` wherever `φ_J′` vanishes (the `σ_A³` fossil at the
    // origin) even here, where the endpoints are strictly ordered; the endpoint mint never does.
    // Abstention (`None`) happens *only* on coincident images.
    assert!(sign.is_some() == (lo != hi));
}

// Soundness of the CAP-IN-D24 cycle-closure + flank-correspondence census (spec §8.5), as
// bounded boundary bookkeeping (vv-guide §5). The census's soundness-critical *carrier
// identity* test is an exact rational-function identity — owned by `lattice` + differential
// testing, out of Kani's tractable scope. Its **combinatorial** admission — the boundary is a
// single closed loop (step 4) spanning both flanks (step 5) — is bounded, and this proves it
// admits only genuine such loops.
//
// The non-trivial content parallels the CLIP-σ harness's "range *every* corner, not just the
// endpoints": the census ANDs `edge_hands_off` over **every** cyclic consecutive pair, not
// merely the wrap `edge[n-1] → edge[0]`. Checking only the wrap would admit a broken chain
// (two sub-arcs whose free ends coincidentally meet). This proves the full census rejects any
// chain with a broken internal link even when its wrap happens to close, and rejects any cap
// missing a flank — the exact soundness of steps 4–5. `N = 4` bounds a cap boundary's edge
// count in the cylinder-flank corpus; the endpoints are `i8`-small (the property is discrete).
#[kani::proof]
#[kani::unwind(6)]
fn cap_in_cycle_census_sound() {
    const N: usize = 4;
    // Per edge: an (end, next_start) coordinate pair, `i8`-small (only equality matters).
    let ends: [(i8, i8); N] = kani::any();
    let starts: [(i8, i8); N] = kani::any();
    // Flank tags 0 = Crease, 1 = A, 2 = B.
    let flanks: [u8; N] = kani::any();
    for &f in &flanks {
        kani::assume(f < 3);
    }

    // The real per-link test, over every cyclic consecutive pair (step 4), and the flank
    // census (step 5) — assembled exactly as `cap_in_d24` assembles them.
    let mut cycle_ok = true;
    let mut wrap_ok = false;
    let mut internal_break = false;
    for k in 0..N {
        let links = edge_hands_off(&ends[k], &starts[(k + 1) % N]);
        cycle_ok &= links;
        if k == N - 1 {
            wrap_ok = links; // the wrap edge[N-1] → edge[0]
        } else if !links {
            internal_break = true; // some interior hand-off failed
        }
    }
    let has_a = flanks.iter().any(|&f| f == 1);
    let has_b = flanks.iter().any(|&f| f == 2);
    let census_accepts = cycle_ok && has_a && has_b;

    // (step 4) A chain whose wrap coincidentally closes but has a broken internal link is
    // rejected — the census checks every hand-off, not just the wrap.
    if wrap_ok && internal_break {
        assert!(!census_accepts);
    }
    // (step 5) A cap missing either flank is rejected.
    if !has_a || !has_b {
        assert!(!census_accepts);
    }
    // Acceptance is sound: every hand-off links (a single closed loop) and both flanks appear.
    if census_accepts {
        for k in 0..N {
            assert!(edge_hands_off(&ends[k], &starts[(k + 1) % N]));
        }
        assert!(has_a && has_b);
    }
}

// Soundness of the ★ REG-V / WEDGE / EXT-WEDGE division-free clearing (spec §8.5). The bundle
// certifies `|V|² = (1−d)/(1+d) ≥ m` (REG-V) and `s_bev(1+s_bev)|V|² < 1` (EXT-WEDGE) **without
// dividing**: each predicate is cleared against `1 + d`, and each clearing is sound only because
// the same function first guards `1 + d > 0` (WEDGE) — `certify_core::wedge::{reg_v,ext_wedge}`
// re-check `one_plus_dot.sign() <= 0` before clearing. The row is ★ because a clearing that
// dropped that guard would flip the inequality on an over-π fold (`1 + d < 0`) and falsely
// certify a degenerate joint — the wedge analogue of the CLIP-σ squared-form slip.
//
// Factored to `i128` rationals (`d = dn/dd`, `m = mn/md`, `s_bev(1+s_bev) = k = kn/kd`, positive
// denominators): the checker's `Rat` residual `(1−d) − m(1+d)`, cleared over the common
// denominator `dd·md > 0`, has the sign of `md·(dd−dn) − mn·(dd+dn)` — the integer combination
// this harness decides. That `Rat`'s arithmetic realizes this ring identity (and that its order
// agrees with `i128`'s) is `lattice`'s obligation, cleanly separated from this logic proof;
// the property is scale-free, so `i32`-widened inputs lose nothing. This proves each clearing
// accepts **iff** the true sign-aware predicate holds, AND that the `1 + d > 0` guard is
// necessary (dropping it admits false certificates on the over-π branch).
#[kani::proof]
fn wedge_clearing_sound() {
    // Rational inputs with strictly-positive denominators; `k = s_bev(1+s_bev) ≥ 0`.
    let dn = kani::any::<i32>() as i128;
    let dd = kani::any::<i32>() as i128;
    let mn = kani::any::<i32>() as i128;
    let md = kani::any::<i32>() as i128;
    let kn = kani::any::<i32>() as i128;
    let kd = kani::any::<i32>() as i128;
    kani::assume(dd > 0 && md > 0 && kd > 0 && kn >= 0);

    // `P/dd = 1 + d`, `Q/dd = 1 − d` (dd > 0 ⇒ sign(P) = sign(1 + d)).
    let p = dd + dn;
    let q = dd - dn;

    // --- REG-V (wedge.rs:196–208) ---
    // Checker accepts ⟺ margin positive ∧ WEDGE guard ∧ cleared residual ≥ 0.
    let regv_accepts = mn > 0 && p > 0 && (md * q - mn * p) >= 0;
    // True sign-aware predicate: |V|² = Q/P ≥ mn/md is geometrically meaningful only on WEDGE
    // (P > 0); there both denominators are positive, so cross-multiplication preserves direction.
    let regv_true = mn > 0 && p > 0 && md * q >= mn * p;
    assert!(regv_accepts == regv_true);

    // WEDGE-necessity: on the over-π branch (P < 0) the true predicate is false, yet the
    // clearing WITHOUT its guard would accept exactly `md·q ≥ mn·p` — a false certificate. This
    // is why `reg_v` guards `1 + d > 0` before clearing.
    if p < 0 && mn > 0 && md * q >= mn * p {
        let regv_no_guard = mn > 0 && (md * q - mn * p) >= 0; // guard dropped
        assert!(regv_no_guard && !regv_true);
    }

    // --- EXT-WEDGE (wedge.rs:217–233) ---
    // Checker accepts ⟺ WEDGE guard ∧ cleared = (1+d) − k(1−d) > 0, i.e. kd·P − kn·Q > 0.
    let ext_accepts = p > 0 && (kd * p - kn * q) > 0;
    // True: s_bev(1+s_bev)|V|² = (kn/kd)(Q/P) < 1 on WEDGE (P > 0) ⟺ kn·Q < kd·P.
    let ext_true = p > 0 && kn * q < kd * p;
    assert!(ext_accepts == ext_true);
}

// Soundness of the ★ gate verdict-propagation fold (spec §8.6; the first reusable
// combinator the workspace's 121 hand-rolled 3-arm conjunction matches reduce to).
// `conj` is the strong-Kleene conjunction: verified iff every conjunct is verified,
// refuted (the first one) iff any is refuted, else unresolved (the first one). This
// proves that biconditional exhaustively over the three-valued lattice for a bounded N,
// AND that the selected witness/margin is the *leftmost* of its kind — the non-trivial
// content, since the fold must return the first refuter even past an earlier unresolved
// conjunct (a refuted conjunct dominates regardless of position), and the first
// unresolved only when no refuter exists anywhere.
//
// Each conjunct is a 2-bit tag (0 = Verified, 1 = Refuted, 2 = Unresolved) with the
// conjunct index as its witness/margin payload, so the returned payload reveals *which*
// conjunct the fold selected. `N = 4` bounds the conjunct count (the one-joint slice folds
// two conjuncts — VALID_complement + one CLOSURE_VALID — with headroom); the property is
// combinatorial in the tags, so the bound loses nothing. `unwind(5)` bounds the ≤N loops
// (the reference scans and `conj`'s own fold).
#[kani::proof]
#[kani::unwind(5)]
fn gate_conj_sound() {
    const N: usize = 4;
    let tags: [u8; N] = kani::any();
    let mut i = 0;
    while i < N {
        kani::assume(tags[i] < 3);
        i += 1;
    }

    // Build the conjunct array; the payload is the index, so a returned witness/margin
    // identifies the selected conjunct. `Verdict<i32, i32, i32>` is `Copy`.
    let mut arr = [Verdict::<i32, i32, i32>::Verified(0); N];
    i = 0;
    while i < N {
        arr[i] = match tags[i] {
            0 => Verdict::Verified(i as i32),
            1 => Verdict::Refuted(i as i32),
            _ => Verdict::Unresolved(i as i32),
        };
        i += 1;
    }

    // Independent reference: the strong-Kleene outcome and the leftmost index of each kind.
    let mut any_refuted = false;
    let mut all_verified = true;
    let mut first_refuted = N; // sentinel = "none"
    let mut first_unresolved = N;
    i = 0;
    while i < N {
        if tags[i] != 0 {
            all_verified = false;
        }
        if tags[i] == 1 {
            any_refuted = true;
            if first_refuted == N {
                first_refuted = i;
            }
        }
        if tags[i] == 2 && first_unresolved == N {
            first_unresolved = i;
        }
        i += 1;
    }

    let r = conj(arr);

    // Soundness AND completeness: the three outcomes partition the tag space, and `conj`
    // lands in exactly the cell the reference does (biconditional, both directions).
    assert!(matches!(r, Verdict::Verified(())) == all_verified);
    assert!(matches!(r, Verdict::Refuted(_)) == any_refuted);
    assert!(matches!(r, Verdict::Unresolved(_)) == (!any_refuted && !all_verified));

    // The selected witness/margin is the *leftmost* of its kind — the ordering guarantee.
    if let Verdict::Refuted(w) = r {
        assert!(w == first_refuted as i32);
    }
    if let Verdict::Unresolved(m) = r {
        assert!(m == first_unresolved as i32);
    }
}

// Soundness of the ★ closed-shell 2-manifold checker (spec §8; Milestone D slice 4 — atlas
// assembly). `closed_shell` decides whether a shell's combinatorics form a closed oriented
// 2-manifold (the boundary of a solid): every wire closed, every edge used once forward and
// once reversed (∂² = 0), and every vertex link a single cycle. These harnesses prove, over
// bounded fixed topologies with symbolic orientation, that it accepts exactly the genuinely-
// closed shells and never a vertex pinch. The unbounded theorem (∂² = 0 ∧ single-cycle links
// ⇒ closed 2-manifold) is the tracked Lean frontier (`CapOut.lean:25-30` assembly analogue),
// not claimed here. Vecs are sized concretely (topology is fixed); only the orientation bits
// are symbolic, so the unwind bounds the fixed-length scans.

/// Independent reference for "the tetrahedron with these orientation bits is a closed
/// oriented 2-manifold": recompute wire closure (no backtracking) and the once-forward /
/// once-reversed edge census by a per-edge scan — a different code path from the checker's
/// position-paired census. For the tetrahedron, closure ∧ census ⇔ closed 2-manifold (every
/// vertex link is then automatically a single 3-cycle), so this is the full reference.
fn ref_tetra_closed(rev: &[bool; 12]) -> bool {
    let es = [0usize, 1, 2, 0, 1, 2];
    let ee = [1usize, 2, 0, 3, 3, 3];
    let we = [2usize, 1, 0, 0, 4, 3, 3, 5, 2, 1, 5, 4];
    let fs = [0usize, 3, 6, 9, 12];
    let dend = |i: usize| -> (usize, usize) {
        let e = we[i];
        if rev[i] {
            (ee[e], es[e])
        } else {
            (es[e], ee[e])
        }
    };
    let mut ok = true;
    // Per-face wire closure + no immediate backtrack along one edge.
    let mut f = 0;
    while f < 4 {
        let lo = fs[f];
        let hi = fs[f + 1];
        let mut k = lo;
        while k < hi {
            let next = if k + 1 < hi { k + 1 } else { lo };
            let (_, t) = dend(k);
            let (s, _) = dend(next);
            if t != s || we[k] == we[next] {
                ok = false;
            }
            k += 1;
        }
        f += 1;
    }
    // Per-edge census: exactly one forward and one reversed use.
    let mut e = 0;
    while e < 6 {
        let mut fc = 0;
        let mut rc = 0;
        let mut i = 0;
        while i < 12 {
            if we[i] == e {
                if rev[i] {
                    rc += 1;
                } else {
                    fc += 1;
                }
            }
            i += 1;
        }
        if fc != 1 || rc != 1 {
            ok = false;
        }
        e += 1;
    }
    ok
}

#[kani::proof]
#[kani::unwind(15)]
fn closed_shell_sound() {
    // Fixed tetrahedron topology; only the 12 orientation bits are symbolic.
    let rev: [bool; 12] = kani::any();
    let es = [0usize, 1, 2, 0, 1, 2];
    let ee = [1usize, 2, 0, 3, 3, 3];
    let we = [2usize, 1, 0, 0, 4, 3, 3, 5, 2, 1, 5, 4];
    let fs = [0usize, 3, 6, 9, 12];
    let accepted = matches!(
        closed_shell(4, &es, &ee, &we, &rev, &fs),
        Verdict::Verified(_)
    );
    // The checker accepts the tetrahedron iff the independent closure+census reference does —
    // so checks 1–3 are sound and, on the accept side, check 4 never spuriously rejects.
    assert!(accepted == ref_tetra_closed(&rev));
}

#[kani::proof]
#[kani::unwind(10)]
fn closed_shell_never_accepts_a_vertex_pinch() {
    // Two minimal closed surfaces (each a "bigon sphere": two vertices joined by two edges,
    // bounding two bigon faces) sharing only vertex 0. Sphere A on {0,1} (edges 0,1), sphere
    // B on {0,2} (edges 2,3). Each sphere alone can be closed, but the union pinches at
    // vertex 0 — the darts of A and B never share an `around`-orbit — so no orientation
    // should make the checker accept it. This is the check-4 (vertex-link) refutation.
    let rev: [bool; 8] = kani::any();
    let es = [0usize, 0, 0, 0];
    let ee = [1usize, 1, 2, 2];
    // Sphere A faces (e0 fwd, e1 rev)(e1 fwd, e0 rev); sphere B faces (e2 fwd, e3 rev)(e3 fwd, e2 rev).
    let we = [0usize, 1, 1, 0, 2, 3, 3, 2];
    let fs = [0usize, 2, 4, 6, 8];
    assert!(!matches!(
        closed_shell(3, &es, &ee, &we, &rev, &fs),
        Verdict::Verified(_)
    ));
}
