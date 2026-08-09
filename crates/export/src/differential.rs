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
//! **Two emission paths, two readings (slice 3).** We audit both representations
//! the certified closure can produce and compare each against the internal verdict:
//!
//! - The **exact §10 body** ([`brep_from_closure`] → [`audit_brep`]): the two flank
//!   `w = 0` ruled sheets sharing the fold crease middle `M` **by identity**. Here the
//!   crease is genuinely reconciled — OCCT reports `M` as a single **2-incidence** edge
//!   (neither free nor non-manifold), the certified-seam outcome this slice delivers.
//!   The body is still *honestly open* elsewhere (the uncertified substrate boundary and
//!   the 2:1 overhang tips remain free edges; the LEDGE cap is deferred to the `V_∂`
//!   real-cut slice), so it is not a closed solid — the watertight claim is narrowly the
//!   certified crease seam, not the whole shell.
//! - The **§11 mesh** ([`shell_from_closure`] → [`audit_shell`]): the triangle soup
//!   sampled at the offset band `w = t.w.lo = 1`. The crease coincides only at the
//!   neutral surface `w = 0`, and slice 1's 2:1 ruling-speed overhang
//!   (`docs/vv-guide.md:921-933`) leaves the two flank crease edges collinear on
//!   `L = {(x,0,1)}` but of *different extent* and, being sampled, two coincident-but-
//!   separate boundaries. So OCCT sees the mesh crease as **open** (`free_edges > 0`,
//!   `closed == false`) — the documented divergence, asserted not hidden.
//!
//! The exact path's free-edge count is **strictly below** the mesh path's: the crease
//! is one shared edge there, no longer two open boundaries. The internal certificate —
//! which closes the abstract joint via SEW-LINK over the (currently hand-authored,
//! geometry-decoupled) `SewInput` — reports a manifold closure throughout.

use certify_core::Verdict;
use closure::Joint;
use closure::valid::{CapWitness, ClosureTreatment, ClosureValid, closure_valid};
use fixtures::closure_joint::{ledge_d24, miter_cap, one_joint, treatment, treatment_miter};
use lattice::Backend;

use crate::brep_build::brep_from_closure;
use crate::shell::shell_from_closure;
use crate::step::{ShellAudit, audit_brep, audit_shell};

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

/// The shared oracle-vs-internal comparison across **both** emission paths: the
/// agreement conjuncts (where the kernels speak to the same fact they must agree),
/// the **exact §10 body**'s certified crease seam (watertight `M`, a 2-incidence
/// edge), and the **§11 mesh**'s documented overhang divergence (OCC open while the
/// internal certificate is manifold). `exact` is [`audit_brep`]'s reading of the
/// ruled-flank B-rep; `mesh` is [`audit_shell`]'s reading of the triangle soup.
fn assert_oracle_vs_internal(
    exact: &ShellAudit,
    mesh: &ShellAudit,
    internal: &InternalVerdict,
    branch: &str,
) {
    // — Agreement (both paths, both kernels) —
    assert!(
        internal.manifold,
        "{branch}: the internal certificate closes (CLOSURE_VALID)"
    );
    assert!(
        exact.brepcheck_valid,
        "{branch}: OCC accepts the exact ruled body (BRepCheck): {exact:?}"
    );
    assert!(
        mesh.brepcheck_valid,
        "{branch}: OCC accepts every mesh face (BRepCheck): {mesh:?}"
    );
    // Neither kernel sees a non-manifold locus on either path: the internal CAP-OUT
    // reports no pinch, and OCC finds no edge shared by ≥3 faces.
    assert_eq!(
        internal.cap_pinches, 0,
        "{branch}: internal CAP-OUT reports no pinch: {internal:?}"
    );
    assert_eq!(
        exact.nonmanifold_edges, 0,
        "{branch}: OCC finds no ≥3-incidence edge in the exact body: {exact:?}"
    );
    assert_eq!(
        mesh.nonmanifold_edges, 0,
        "{branch}: OCC finds no ≥3-incidence edge in the mesh: {mesh:?}"
    );

    // The certificate's declared sew incidences (`t.sew.counts`/`links`) are consumed
    // here read-only. They are hand-authored and DECOUPLED from the emitted geometry
    // (deriving `SewInput` from emitted geometry is a later slice). So we assert only
    // that the certificate declares a non-trivial closure: at least one seam incidence
    // and one boundary-vertex link.
    assert!(
        internal.cap_to_flank + internal.flank_to_flank > 0,
        "{branch}: the SewInput declares ≥1 seam incidence: {internal:?}"
    );
    assert!(
        internal.links > 0,
        "{branch}: the SewInput declares ≥1 boundary-vertex link: {internal:?}"
    );

    // — Exact §10 body: the certified crease seam is watertight (slice 3) —
    // The two flanks share the fold crease middle M by identity, so OCC sees exactly
    // one edge that is neither free nor non-manifold: M, incident to both flanks. This
    // is the certified-seam outcome — narrowly the crease, not the whole shell (the
    // body stays honestly open elsewhere, below).
    assert_eq!(
        exact.edges - exact.free_edges - exact.nonmanifold_edges,
        1,
        "{branch}: the exact body has exactly one 2-incidence edge — the shared crease M: {exact:?}"
    );
    // Honest-open, not closed: the uncertified substrate boundary + overhang tips
    // remain free (the LEDGE cap is deferred to the V_∂ real-cut slice), so the exact
    // body is not a closed solid — only the crease seam is reconciled.
    assert!(
        exact.free_edges > 0 && !exact.closed,
        "{branch}: the exact body is honestly open away from the certified seam: {exact:?}"
    );

    // — §11 mesh: the documented overhang divergence (asserted, not hidden) —
    // OCC sees the sampled band's crease as open (two coincident-but-separate free
    // boundaries at w=1≠0, of different extent); the internal certificate closes the
    // abstract joint at w=0 via the (decoupled) SewInput.
    assert!(
        mesh.free_edges > 0,
        "{branch}: OCC sees the mesh band open along the crease (2:1 overhang, w=1≠0): {mesh:?}"
    );
    assert!(
        !mesh.closed,
        "{branch}: OCC reports the mesh band non-closed while the internal certificate \
         is manifold — the documented seam gap: {mesh:?}"
    );

    // The exact path reconciles the crease the mesh path leaves open: its free-edge
    // count is strictly lower — the crease is one shared edge, not two open boundaries.
    assert!(
        exact.free_edges < mesh.free_edges,
        "{branch}: exact body {} free edges < mesh {} — the crease is now shared, not open",
        exact.free_edges,
        mesh.free_edges
    );
}

/// **LEDGE** branch: the physical fold whose cap is deferred to the `V_∂` real-cut
/// slice (Option B — the exact body is the two flanks, like MITER). The OCC oracle
/// agrees on validity/manifoldness, sees the exact crease seam as watertight, and
/// documents the mesh overhang; the internal CAP-OUT still reports a real, pinch-free
/// `V_∂` (the abstract cap the mesh path triangulates).
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
    let exact = audit_brep(&brep_from_closure(&joint, &t, &valid))
        .expect("OCC audits the exact ledge body");
    let mesh =
        audit_shell(&shell_from_closure(&joint, &t, &valid)).expect("OCC audits the ledge mesh");
    let internal = internal_verdict(&valid, &t);
    assert_oracle_vs_internal(&exact, &mesh, &internal, "ledge");
    assert!(
        internal.cap_boundary > 0,
        "ledge: CAP-OUT V_∂ is non-empty (a real cap boundary): {internal:?}"
    );
}

/// **MITER** branch: the same fold with a clean mitered corner (no separate cap
/// face). The OCC oracle agrees on validity/manifoldness, sees the exact crease seam
/// as watertight, and documents the same mesh overhang; there is no cap face, so no
/// CAP-OUT `V_∂`.
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
    let exact = audit_brep(&brep_from_closure(&joint, &t, &valid))
        .expect("OCC audits the exact miter body");
    let mesh =
        audit_shell(&shell_from_closure(&joint, &t, &valid)).expect("OCC audits the miter mesh");
    let internal = internal_verdict(&valid, &t);
    assert_oracle_vs_internal(&exact, &mesh, &internal, "miter");
    assert_eq!(
        internal.cap_boundary, 0,
        "miter: no separate cap face, so no CAP-OUT V_∂: {internal:?}"
    );
}
