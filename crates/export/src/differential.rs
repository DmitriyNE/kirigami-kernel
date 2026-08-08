//! The OpenCASCADE differential-**oracle** harness (Milestone D slice 2).
//!
//! For each certified one-joint treatment we build the emitted shell, ask OCCT
//! for its own topology facts ([`audit_shell`]), distil the internal certificate's
//! reading ([`InternalVerdict`]), and **compare** the two. OCCT is an *oracle*,
//! never the certificate: manifoldness is certified internally by SEW-EDGES /
//! SEW-LINK / CAP-OUT-LINK; the external kernel only corroborates (agreement) or
//! flags a *documented, spec-aligned* divergence — "oracle ∧ audit, never
//! oracle-instead-of-audit" (`docs/vv-guide.md:854-859`; spec "no kernel CSG").
//! Mirrors `difftest`'s CGAL harness (`crates/difftest/src/differential.rs`).
//!
//! **The documented divergence (slice 1's overhang, now an asserted fact):** the
//! shell is sampled at the offset band `w = t.w.lo = 1`, but the crease coincides
//! only at the neutral surface `w = 0`, and slice 1's 2:1 ruling-speed overhang
//! (`docs/vv-guide.md:921-933`) leaves the two flank crease edges collinear on
//! `L = {(x,0,1)}` but of *different extent*. So OCCT sees the exported band as an
//! **open** shell (`free_edges > 0`, `closed == false`) while the internal
//! certificate — which closes the abstract joint via SEW-LINK over the (currently
//! hand-authored, geometry-decoupled) `SewInput` — reports a manifold closure. The
//! geometry-changing watertight `V_∂` seam that would close this gap is **slice 3**.

use certify_core::Verdict;
use closure::Joint;
use closure::valid::{CapWitness, ClosureTreatment, ClosureValid, closure_valid};
use fixtures::closure_joint::{ledge_d24, miter_cap, one_joint, treatment, treatment_miter};
use lattice::Backend;

use crate::shell::shell_from_closure;
use crate::step::{ShellAudit, audit_shell};

/// The internal certificate's own reading of the sewn shell, distilled from the
/// certified artifacts — the side the OCC oracle is compared against.
#[derive(Debug)]
struct InternalVerdict {
    /// The internal certificate closes: `closure_valid` → Verified, which required
    /// SEW-EDGES (no edge pinch) and SEW-LINK (no link crossing) to pass. Holding a
    /// [`ClosureValid`] *is* that evidence.
    manifold: bool,
    /// `|V_∂|` — CAP-OUT manifold-boundary vertices (Ledge branch); `0` for a Miter
    /// (no separate cap face).
    cap_boundary: usize,
    /// `|pinches|` — CAP-OUT non-manifold self-touch vertices, excluded from `V_∂`.
    cap_pinches: usize,
    /// Declared cap→flank source-side incidences (`t.sew.counts.cap_to_flank`).
    cap_to_flank: usize,
    /// Declared flank→flank source-side incidences (`t.sew.counts.flank_to_flank`).
    flank_to_flank: usize,
    /// Boundary-vertex embedded links (domain `V_∂`), `t.sew.links.len()`.
    links: usize,
}

/// Distil the internal certificate + treatment into an [`InternalVerdict`],
/// consuming `valid.cap.v_boundary()`/`pinches()` (Ledge) and `t.sew.counts`/`links`.
fn internal_verdict<B: Backend>(
    valid: &ClosureValid<B>,
    t: &ClosureTreatment<'_, B>,
) -> InternalVerdict {
    let (cap_boundary, cap_pinches) = match &valid.cap {
        CapWitness::Ledge(cap) => (cap.v_boundary().len(), cap.pinches().len()),
        CapWitness::Miter(_) => (0, 0),
    };
    InternalVerdict {
        // We only ever build this from a `ClosureValid`, obtainable solely from a
        // Verdict::Verified — so SEW-EDGES ∧ SEW-LINK ∧ CAP-OUT-LINK all passed.
        manifold: true,
        cap_boundary,
        cap_pinches,
        cap_to_flank: t.sew.counts.cap_to_flank,
        flank_to_flank: t.sew.counts.flank_to_flank,
        links: t.sew.links.len(),
    }
}

/// Run `closure_valid` and unwrap the certified witness (the fixtures are
/// CLOSURE_VALID by construction; a refutation here is a real regression).
fn expect_valid<B: Backend>(joint: &Joint<B>, t: &ClosureTreatment<'_, B>) -> ClosureValid<B> {
    match closure_valid(joint, t) {
        Verdict::Verified(v) => v,
        _ => panic!("the one-joint fixture is CLOSURE_VALID"),
    }
}

/// The shared oracle-vs-internal comparison: the agreement conjuncts (where both
/// kernels speak to the same fact they must agree) and the documented overhang
/// divergence (OCC open while the internal certificate is manifold).
fn assert_oracle_vs_internal(occ: &ShellAudit, internal: &InternalVerdict, branch: &str) {
    // — Agreement —
    assert!(
        internal.manifold,
        "{branch}: the internal certificate closes (CLOSURE_VALID)"
    );
    assert!(
        occ.brepcheck_valid,
        "{branch}: OCC accepts every emitted face (BRepCheck): {occ:?}"
    );
    // Neither kernel sees a non-manifold locus: the internal CAP-OUT reports no
    // pinch, and OCC finds no edge shared by ≥3 faces.
    assert_eq!(
        internal.cap_pinches, 0,
        "{branch}: internal CAP-OUT reports no pinch: {internal:?}"
    );
    assert_eq!(
        occ.nonmanifold_edges, 0,
        "{branch}: OCC finds no ≥3-incidence (non-manifold) edge: {occ:?}"
    );

    // The certificate's declared sew incidences (`t.sew.counts`/`links`) are consumed
    // here read-only. In slice 2 they are hand-authored and DECOUPLED from the emitted
    // triangle soup — deriving `SewInput` from emitted geometry is slice 3. So we assert
    // only that the certificate declares a non-trivial closure: at least one seam
    // incidence and one boundary-vertex link.
    assert!(
        internal.cap_to_flank + internal.flank_to_flank > 0,
        "{branch}: the SewInput declares ≥1 seam incidence: {internal:?}"
    );
    assert!(
        internal.links > 0,
        "{branch}: the SewInput declares ≥1 boundary-vertex link: {internal:?}"
    );

    // — Documented divergence (the slice-1 overhang, asserted not hidden) —
    // OCC sees the exported band's crease as open; the internal certificate closes
    // the abstract joint at w=0 via the (decoupled) SewInput. The watertight V_∂
    // seam that would reconcile these is slice 3.
    assert!(
        occ.free_edges > 0,
        "{branch}: OCC sees the band open along the crease (2:1 overhang, w=1≠0): {occ:?}"
    );
    assert!(
        !occ.closed,
        "{branch}: OCC reports the exported band non-closed while the internal \
         certificate is manifold — the documented seam gap (→ slice 3): {occ:?}"
    );
}

/// **LEDGE** branch: the physical fold with a spanning cap face. The OCC oracle
/// agrees on validity/manifoldness and documents the overhang; the internal
/// CAP-OUT reports a real, pinch-free `V_∂`.
#[test]
fn ledge_oracle_agrees_and_documents_the_overhang() {
    let joint = one_joint();
    let d24 = ledge_d24();
    let t = treatment(&d24);
    let valid = expect_valid(&joint, &t);
    assert!(
        matches!(valid.cap, CapWitness::Ledge(_)),
        "the ledge treatment certifies via the LEDGE cap branch"
    );
    let shell = shell_from_closure(&joint, &t, &valid);
    let occ = audit_shell(&shell).expect("OCC audits the ledge shell");
    let internal = internal_verdict(&valid, &t);
    assert_oracle_vs_internal(&occ, &internal, "ledge");
    assert!(
        internal.cap_boundary > 0,
        "ledge: CAP-OUT V_∂ is non-empty (a real cap boundary): {internal:?}"
    );
}

/// **MITER** branch: the same fold with a clean mitered corner (no separate cap
/// face). The OCC oracle agrees on validity/manifoldness and documents the same
/// overhang; there is no cap face, so no CAP-OUT `V_∂`.
#[test]
fn miter_oracle_agrees_and_documents_the_overhang() {
    let joint = one_joint();
    let cap_outline = miter_cap();
    let t = treatment_miter(&cap_outline);
    let valid = expect_valid(&joint, &t);
    assert!(
        matches!(valid.cap, CapWitness::Miter(_)),
        "the miter treatment certifies via the MITER cap branch"
    );
    let shell = shell_from_closure(&joint, &t, &valid);
    let occ = audit_shell(&shell).expect("OCC audits the miter shell");
    let internal = internal_verdict(&valid, &t);
    assert_oracle_vs_internal(&occ, &internal, "miter");
    assert_eq!(
        internal.cap_boundary, 0,
        "miter: no separate cap face, so no CAP-OUT V_∂: {internal:?}"
    );
}
