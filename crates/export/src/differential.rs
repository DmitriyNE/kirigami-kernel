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
use certify_core::gate::{ClosedSolid, SolidClosure, SolidClosureFault, valid_closed_solid};
use certify_core::shell::{ClosedShell, closed_shell};
use closure::Joint;
use closure::valid::{CapWitness, ClosureTreatment, ClosureValid, closure_valid};
use fixtures::closure_joint::{ledge_d24, miter_cap, one_joint, treatment, treatment_miter};
use lattice::Backend;

use crate::brep_build::{
    brep_freeboundary_from_closure, brep_from_closure, brep_slab_from_closure,
};
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

/// **The certified closed solid (Milestone D slice 4 — atlas assembly).** The single-flank
/// slab is certified closed *internally*: `closed_shell` verifies its combinatorics are a
/// closed oriented 2-manifold, and `valid_closed_solid` conjoins that with the joint's
/// CLOSURE_VALID. The OCCT oracle then *corroborates* the emitted geometry — including the
/// two rational-patch μ-walls — reporting no free edge, no non-manifold edge, and a valid
/// shell. Closedness is **earned** by the internal checker, never delegated to the kernel
/// ("oracle ∧ audit, never oracle-instead"). This is the first genuinely closed solid.
#[test]
fn the_flank_slab_is_a_certified_closed_solid() {
    let joint = one_joint();
    let d24 = ledge_d24();
    let t = treatment(&d24);
    // The joint closes (CLOSURE_VALID) — the leftmost conjunct of the atlas gate.
    let _valid = expect_valid(&joint, &t);

    let slab = brep_slab_from_closure(&joint, &t);

    // — Internal certificate: closed_shell over the slab's combinatorics —
    let cert = slab.to_shell_certificate();
    let shell = closed_shell(
        cert.n_verts,
        &cert.edge_start,
        &cert.edge_end,
        &cert.wire_edge,
        &cert.wire_reversed,
        &cert.face_start,
    );
    assert_eq!(
        shell,
        Verdict::Verified(ClosedShell {
            verts: 8,
            edges: 12,
            faces: 6
        }),
        "closed_shell certifies the slab a closed oriented 2-manifold"
    );

    // The atlas gate: the joint closes AND the assembled shell is closed.
    let solid_closure: Verdict<SolidClosure, SolidClosureFault<&str>, ()> =
        Verdict::Verified(SolidClosure {
            joints_certified: 1,
        });
    assert_eq!(
        valid_closed_solid(&solid_closure, &shell),
        Verdict::Verified(ClosedSolid {
            joints_certified: 1,
            verts: 8,
            edges: 12,
            faces: 6
        }),
        "valid_closed_solid conjoins the joint closure with whole-solid closedness"
    );

    // — OCCT oracle corroborates the geometry (compared, never trusted as the certificate) —
    let audit = audit_brep(&slab).expect("OCC audits the exact slab");
    assert!(
        audit.brepcheck_valid,
        "OCC accepts the exact slab (incl. the rational-patch μ-walls): {audit:?}"
    );
    assert_eq!(audit.faces, 6, "six faces: {audit:?}");
    assert_eq!(
        audit.free_edges, 0,
        "OCC finds no free edge — the slab is watertight: {audit:?}"
    );
    assert_eq!(
        audit.nonmanifold_edges, 0,
        "OCC finds no non-manifold edge: {audit:?}"
    );
}

/// **The free-boundary certified closed solid (Milestone D slice D4.3).** The single-flank
/// slab over an *authored substrate free boundary* — the tapered σ-band `μ⁻(σ) = −1 + σ`,
/// `μ⁺(σ) = 1 − σ`, not the rectangular support box. Certified closed **internally** by
/// `closed_shell` (conjoined with the joint's `CLOSURE_VALID` via `valid_closed_solid`), and
/// the OCCT oracle *corroborates* the emitted geometry — now **four** exact rational-patch
/// sides (the boundary curves in σ, so even the `w = const` sheets are rational, not the
/// slab's straight extrusions) — reporting no free edge, no non-manifold edge, a valid shell.
/// Closedness is **earned** by the internal checker, never delegated to the kernel. This is
/// the first certified closed solid over a real material outline rather than a box.
#[test]
fn the_free_boundary_solid_is_a_certified_closed_solid() {
    use lattice::{Poly, Rat, RatFunc};

    let poly = |cs: &[i128]| {
        Poly::<lattice::Bignum>::from_coeffs(cs.iter().map(|&c| Rat::from_i128(c)).collect())
    };
    let joint = one_joint();
    let d24 = ledge_d24();
    let t = treatment(&d24);
    // The joint closes (CLOSURE_VALID) — the leftmost conjunct of the atlas gate.
    let _valid = expect_valid(&joint, &t);

    // The authored free boundary: a tapered μ-band, genuinely varying in σ.
    let mu_lo = RatFunc::from_poly(poly(&[-1, 1]));
    let mu_hi = RatFunc::from_poly(poly(&[1, -1]));
    let solid = brep_freeboundary_from_closure(&joint, &t, &mu_lo, &mu_hi);

    // — Internal certificate: closed_shell over the free-boundary solid's combinatorics —
    let cert = solid.to_shell_certificate();
    let shell = closed_shell(
        cert.n_verts,
        &cert.edge_start,
        &cert.edge_end,
        &cert.wire_edge,
        &cert.wire_reversed,
        &cert.face_start,
    );
    assert_eq!(
        shell,
        Verdict::Verified(ClosedShell {
            verts: 8,
            edges: 12,
            faces: 6
        }),
        "closed_shell certifies the free-boundary solid a closed oriented 2-manifold"
    );

    // The atlas gate: the joint closes AND the assembled shell is closed.
    let solid_closure: Verdict<SolidClosure, SolidClosureFault<&str>, ()> =
        Verdict::Verified(SolidClosure {
            joints_certified: 1,
        });
    assert_eq!(
        valid_closed_solid(&solid_closure, &shell),
        Verdict::Verified(ClosedSolid {
            joints_certified: 1,
            verts: 8,
            edges: 12,
            faces: 6
        }),
        "valid_closed_solid conjoins the joint closure with whole-solid closedness"
    );

    // — OCCT oracle corroborates the geometry (compared, never trusted as the certificate) —
    let audit = audit_brep(&solid).expect("OCC audits the exact free-boundary solid");
    assert!(
        audit.brepcheck_valid,
        "OCC accepts the exact free-boundary solid (incl. the four rational-patch sides): {audit:?}"
    );
    assert_eq!(audit.faces, 6, "six faces: {audit:?}");
    assert_eq!(
        audit.free_edges, 0,
        "OCC finds no free edge — the free-boundary solid is watertight: {audit:?}"
    );
    assert_eq!(
        audit.nonmanifold_edges, 0,
        "OCC finds no non-manifold edge: {audit:?}"
    );
}

/// **The device cone as a certified closed solid — the machinery generalizes past the
/// cylinder.** A frustum band (gore) of the exact 42° rational device cone
/// (`fixtures::devices::cone()`, `n·ẑ ≡ 65/97`, apex at the origin) over an authored slanted
/// boundary `μ⁻(σ) = 1`, `μ⁺(σ) = 2 + σ` — a genuinely different chart than the cylinder slab
/// (converging rulings ⇒ higher-degree rational patches). (1) the D4.3a `free_boundary` checker
/// certifies the authored boundary valid; (2) `closed_shell` certifies the 8/12/6 solid a
/// closed oriented 2-manifold; (3) the OCCT oracle corroborates the (higher-degree) geometry
/// (`brepcheck_valid`, `free_edges == 0`, `nonmanifold_edges == 0`). Exact throughout — the
/// cone chart's splines and the boundary are the *hand-authored* stand-in for the DEV layer;
/// everything downstream is exact ruled geometry.
#[test]
fn the_cone_frustum_band_is_a_certified_closed_solid() {
    use crate::brep_build::{FreeBoundaryMargins, brep_freeboundary, free_boundary_cert};
    use certify_core::free_boundary::free_boundary;
    use lattice::{Interval, Poly, Rat, RatFunc};

    let poly = |cs: &[i128]| {
        Poly::<lattice::Bignum>::from_coeffs(cs.iter().map(|&c| Rat::from_i128(c)).collect())
    };
    let chart = fixtures::devices::cone();
    let sigma = Interval {
        lo: Rat::from_i128(0),
        hi: Rat::from_i128(1),
    };
    let w = Interval {
        lo: Rat::from_i128(0),
        hi: Rat::new(1, 4),
    };
    let mu_lo = RatFunc::from_poly(poly(&[1])); // inner edge (μ ≡ 1)
    let mu_hi = RatFunc::from_poly(poly(&[2, 1])); // outer edge μ = 2 + σ (authored slant)

    // (1) The authored free boundary is certified valid (positive width, regular rails, monotone).
    let fbc = free_boundary_cert(
        &chart,
        &mu_lo,
        &mu_hi,
        &sigma,
        &RatFunc::one(), // σ-graph: σ̂ = σ ⇒ σ̂′ = 1
        &FreeBoundaryMargins {
            width: Rat::new(1, 2), // width 1 + σ ∈ [1, 2]
            reg: Rat::new(1, 10),  // |â′|² comfortably above 1/10 across the gore
            mono: Rat::new(1, 2),
        },
    );
    assert!(
        matches!(free_boundary(&fbc), Verdict::Verified(_)),
        "the cone gore's authored boundary is a valid free boundary"
    );

    // (2) closed_shell certifies the assembled solid a closed oriented 2-manifold.
    let solid = brep_freeboundary(&chart, &sigma, &w, &mu_lo, &mu_hi);
    let cert = solid.to_shell_certificate();
    assert_eq!(
        closed_shell(
            cert.n_verts,
            &cert.edge_start,
            &cert.edge_end,
            &cert.wire_edge,
            &cert.wire_reversed,
            &cert.face_start,
        ),
        Verdict::Verified(ClosedShell {
            verts: 8,
            edges: 12,
            faces: 6
        }),
        "closed_shell certifies the cone frustum band closed"
    );

    // (3) OCCT corroborates the higher-degree cone geometry.
    let audit = audit_brep(&solid).expect("OCC audits the exact cone frustum band");
    assert!(
        audit.brepcheck_valid,
        "OCC accepts the exact cone frustum band: {audit:?}"
    );
    assert_eq!(audit.faces, 6, "six faces: {audit:?}");
    assert_eq!(
        audit.free_edges, 0,
        "OCC finds no free edge — the cone band is watertight: {audit:?}"
    );
    assert_eq!(
        audit.nonmanifold_edges, 0,
        "OCC finds no non-manifold edge: {audit:?}"
    );
}

/// **G6b interior-hole mechanism (the gate) — a planar holed panel through OCCT.** The
/// widened bridge (N-loop faces + the shim's `mf.Add(holeWire)` + `ShapeFix_Face`
/// orientation) must emit a `TopoDS_Face` with a real inner wire that BRepCheck accepts.
/// A 6×6 outer square with a 2×2 interior hole (both authored CCW, so `ShapeFix` genuinely
/// reverses the hole to a proper inner boundary) is an honestly *open* sheet — all eight
/// boundary edges (4 outer + 4 hole) are free — but a single valid holed face, matching the
/// pure-IR incidence exactly (mirrors `brep.rs`'s `a_face_with_a_hole_…` combinatorial test).
#[test]
fn a_planar_holed_panel_audits_as_one_valid_holed_face() {
    use crate::brep::FaceSurface;
    use crate::brep_build::brep_holed_panel;
    use lattice::{Bignum, Rat};

    let q = |n: i128| Rat::<Bignum>::from_i128(n);
    let p = |x: i128, y: i128| [q(x), q(y), q(0)];
    let outer = [p(0, 0), p(6, 0), p(6, 6), p(0, 6)];
    let hole = [p(2, 2), p(4, 2), p(4, 4), p(2, 4)];
    let brep = brep_holed_panel(FaceSurface::Plane, &outer, &[&hole]);

    // Pure-IR precondition: both loops close, 8 free edges (open holed sheet).
    assert!(brep.all_loops_closed(0), "outer + hole loops close");
    assert_eq!(brep.free_edges(), 8, "4 outer + 4 hole edges, all free");

    let audit = audit_brep(&brep).expect("OCC audits the planar holed panel");
    assert_eq!(audit.faces, 1, "one holed face: {audit:?}");
    assert_eq!(audit.edges, 8, "4 outer + 4 hole edges: {audit:?}");
    assert_eq!(
        audit.free_edges, 8,
        "every boundary edge is free (open sheet): {audit:?}"
    );
    assert_eq!(
        audit.nonmanifold_edges, 0,
        "no non-manifold edge: {audit:?}"
    );
    assert!(
        audit.brepcheck_valid,
        "OCC accepts the planar face with an interior hole: {audit:?}"
    );
}

/// One *open* ruled cone panel (one `brep_freeboundary` side face, opened out as a standalone
/// sheet): a frustum sector of the device cone at `w = 0`, `μ ∈ [1, 3]`, `σ ∈ [0, 1]`, its four
/// boundary edges **exactly on-surface** — two straight `σ = const` rulings and two
/// rational-Bézier `σ`-rails — on a `RationalPatch` ruled between the two μ-rails. With
/// `with_hole`, an interior rectangle in `(σ, μ)` whose four corners lie **on** the cone
/// (`chart.surface`) but whose edges are straight `Line` **chords** that cut across it — the
/// faithful STEP-II interior hole. Mirrors the `brep_freeboundary` reduction so the two rails
/// share a denominator (`RatBezierSurface::ruled_from_rails`' precondition).
fn cone_panel_brep(with_hole: bool) -> crate::brep::Brep<lattice::Bignum> {
    use crate::bezier::{RatBezier, RatBezierSurface};
    use crate::brep::{Brep, EdgeGeom, FaceSurface};
    use lattice::{Bignum, Poly, Rat, RatFunc, Surd};

    let chart = fixtures::devices::cone();
    let q = |n: i128| Rat::<Bignum>::from_i128(n);
    let qn = |n: i128, d: i128| Rat::<Bignum>::new(n, d);
    let rf = |n: i128| RatFunc::<Bignum>::from_poly(Poly::from_coeffs(vec![Rat::from_i128(n)]));
    let vert = |p: &[Rat<Bignum>; 3]| {
        [
            Surd::<Bignum>::from_rat(p[0].clone()),
            Surd::from_rat(p[1].clone()),
            Surd::from_rat(p[2].clone()),
        ]
    };
    let (sig_lo, sig_hi, w0) = (q(0), q(1), q(0));

    // The two σ-rails μ⁻ = 1, μ⁺ = 3 at w = 0, built exactly as brep_freeboundary does so both
    // rails share a Bernstein denominator.
    let c = chart.pedal().reduce();
    let r = chart.ruling().reduce();
    let n = chart.normal().reduce();
    let surf_lo = c.add(&r.scale(&rf(1))).reduce().add(&n.scale_rat(&w0));
    let surf_hi = c.add(&r.scale(&rf(3))).reduce().add(&n.scale_rat(&w0));

    let a0 = surf_lo.eval(&sig_lo).expect("A0 on cone"); // μ⁻, σlo
    let a1 = surf_hi.eval(&sig_lo).expect("A1 on cone"); // μ⁺, σlo
    let b0 = surf_lo.eval(&sig_hi).expect("B0 on cone"); // μ⁻, σhi
    let b1 = surf_hi.eval(&sig_hi).expect("B1 on cone"); // μ⁺, σhi

    let mut brep = Brep::<Bignum>::new();
    let va0 = brep.add_vertex(vert(&a0));
    let va1 = brep.add_vertex(vert(&a1));
    let vb0 = brep.add_vertex(vert(&b0));
    let vb1 = brep.add_vertex(vert(&b1));

    // Two straight σ = const rulings + two rational-Bézier σ-rails — all on-surface.
    let ruling_lo = brep.add_edge(va0, va1, EdgeGeom::Line); // A0 → A1
    let rail_hi = brep.add_edge(
        va1,
        vb1,
        EdgeGeom::RationalBezier(RatBezier::from_vec3rat(&surf_hi, &sig_lo, &sig_hi)),
    ); // A1 → B1
    let ruling_hi = brep.add_edge(vb1, vb0, EdgeGeom::Line); // B1 → B0
    let rail_lo = brep.add_edge(
        va0,
        vb0,
        EdgeGeom::RationalBezier(RatBezier::from_vec3rat(&surf_lo, &sig_lo, &sig_hi)),
    ); // A0 → B0
    let wire = vec![
        (ruling_lo, false),
        (rail_hi, false),
        (ruling_hi, false),
        (rail_lo, true), // B0 → A0
    ];
    let surface = FaceSurface::RationalPatch(RatBezierSurface::ruled_from_rails(
        &surf_lo, &surf_hi, &sig_lo, &sig_hi,
    ));

    let holes = if with_hole {
        // A rectangle interior to (σ, μ), corners on the cone; μ-parallel sides are chords.
        let on = |mu: &Rat<Bignum>, sig: &Rat<Bignum>| {
            chart
                .surface(mu, &w0)
                .eval(sig)
                .expect("hole corner on cone")
        };
        let (mlo, mhi) = (qn(9, 5), qn(11, 5)); // μ ∈ (1, 3)
        let (slo, shi) = (qn(2, 5), qn(3, 5)); // σ ∈ (0, 1)
        let hp = [
            on(&mlo, &slo),
            on(&mhi, &slo),
            on(&mhi, &shi),
            on(&mlo, &shi),
        ];
        let hv: Vec<usize> = hp.iter().map(|p| brep.add_vertex(vert(p))).collect();
        let hole: Vec<_> = (0..4)
            .map(|i| (brep.add_edge(hv[i], hv[(i + 1) % 4], EdgeGeom::Line), false))
            .collect();
        vec![hole]
    } else {
        Vec::new()
    };

    brep.add_face_with_holes(surface, wire, holes);
    brep
}

/// **Sanity: the hole-free ruled cone panel is a valid open face through OCCT.** Isolates the
/// off-surface-hole risk (below) from the panel construction itself: with the four boundary
/// edges exactly on-surface, OCC must accept the standalone ruled sheet — one face, four free
/// boundary edges, BRepCheck-valid.
#[test]
fn a_ruled_cone_panel_audits_as_one_valid_open_face() {
    let brep = cone_panel_brep(false);
    let audit = audit_brep(&brep).expect("OCC audits the ruled cone panel");
    assert_eq!(audit.faces, 1, "one ruled sheet: {audit:?}");
    assert_eq!(
        audit.free_edges, 4,
        "open panel: all four boundary edges free: {audit:?}"
    );
    assert_eq!(audit.nonmanifold_edges, 0, "{audit:?}");
    assert!(
        audit.brepcheck_valid,
        "OCC accepts the on-surface ruled cone panel: {audit:?}"
    );
}

/// **The STEP-II risk: a curved cone panel with an off-surface-chord interior hole through
/// OCCT.** The outer boundary is on-surface (isolated by the sanity test above); the hole's
/// corners lie on the cone but its edges are straight chords cutting across the curved surface,
/// so OCC's `ShapeFix` must project pcurves onto the `RationalPatch` and reconcile the
/// chord→surface gap within tolerance. This test *observes* OCC and pins the outcome (the
/// pure-IR incidence holds regardless); the assertion below records the behavior verified under
/// `nix develop` — see the G6b engineering-log note.
#[test]
fn a_ruled_cone_panel_with_an_interior_hole_through_occt() {
    let brep = cone_panel_brep(true);
    // Pure-IR precondition, independent of what OCC makes of the off-surface chords.
    assert!(brep.all_loops_closed(0), "outer + hole loops close");
    assert_eq!(
        brep.free_edges(),
        8,
        "4 on-surface outer + 4 chord hole edges, all free"
    );

    let audit = audit_brep(&brep).expect("OCC audits the holed cone panel");
    assert_eq!(audit.faces, 1, "one holed cone sheet: {audit:?}");
    assert_eq!(audit.nonmanifold_edges, 0, "{audit:?}");
    assert!(
        audit.brepcheck_valid,
        "OCC accepts the cone face with an off-surface-chord hole: {audit:?}"
    );
}

/// **G9 — the two-sided cone gore as a robust subdivided closed solid.** The wide σ=0-crossing gore
/// (σ ∈ [−2, 2], band μ ∈ [−2, −1]) is exactly the case that (a) a single rational Bézier patch
/// cannot carry with positive weights and (b) SIGSEGV'd OCCT when forced through a B-spline closed
/// shell. `brep_freeboundary` now subdivides σ by the intrinsic **positive-weight** criterion into
/// single-span Bézier slices (no σ=0 special case), so the fused N-slice solid is certified closed
/// internally by `closed_shell` **and** OCCT corroborates it: `brepcheck_valid`, `free_edges == 0`,
/// `nonmanifold_edges == 0`, no abort. This is the robust replacement for the abandoned B-spline.
#[test]
fn the_two_sided_cone_gore_is_a_robust_subdivided_solid() {
    use crate::brep_build::brep_freeboundary;
    use lattice::{Interval, Poly, Rat, RatFunc};

    let muf =
        |n: i128| RatFunc::<lattice::Bignum>::from_poly(Poly::from_coeffs(vec![Rat::from_i128(n)]));
    let chart = fixtures::devices::cone();
    let sigma = Interval {
        lo: Rat::from_i128(-2),
        hi: Rat::from_i128(2),
    };
    let w = Interval {
        lo: Rat::from_i128(0),
        hi: Rat::new(1, 8),
    };
    let solid = brep_freeboundary(&chart, &sigma, &w, &muf(-2), &muf(-1));

    // The two-sided gore genuinely subdivided (faces = 4N+2 > 6), and the fused solid is a certified
    // closed 2-manifold with the (4(N+1), 8N+4, 4N+2) counts.
    let (nv, ne, nf) = (
        solid.verts().len(),
        solid.edges().len(),
        solid.faces().len(),
    );
    let big_n = (nf - 2) / 4;
    assert!(big_n >= 2, "subdivided into N ≥ 2 slices: N = {big_n}");
    let cert = solid.to_shell_certificate();
    assert_eq!(
        closed_shell(
            cert.n_verts,
            &cert.edge_start,
            &cert.edge_end,
            &cert.wire_edge,
            &cert.wire_reversed,
            &cert.face_start,
        ),
        Verdict::Verified(ClosedShell {
            verts: nv,
            edges: ne,
            faces: nf
        }),
        "closed_shell certifies the subdivided two-sided cone solid"
    );

    // OCCT corroborates the exact single-span-Bézier geometry — cleanly, no crash.
    let audit = audit_brep(&solid).expect("OCC audits the subdivided two-sided cone solid");
    assert!(
        audit.brepcheck_valid,
        "OCC accepts the subdivided two-sided cone solid: {audit:?}"
    );
    assert_eq!(audit.faces, nf, "{audit:?}");
    assert_eq!(audit.free_edges, 0, "watertight — no free edge: {audit:?}");
    assert_eq!(audit.nonmanifold_edges, 0, "{audit:?}");
}
