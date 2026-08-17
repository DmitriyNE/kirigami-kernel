//! **IO.3a — the authored sketch, at the plane it cuts from.**
//!
//! A cutter's frame is a *search result*: `develop::pick` snaps a picked plane to rationals and
//! certifies the snap. What no certificate can say is whether the **pick** landed where the author
//! meant — that is a question about intent, and a picture is the only instrument for it.
//!
//! So this file asserts two claims that are easy to conflate and must not be:
//!
//! 1. **The face lies in its frame's plane, exactly.** `N·(X − o) = 0` as a rational equality, for
//!    every vertex, with no tolerance. This is invariant under *any* in-plane sampling, which is
//!    why chording the arcs and snapping the `Surd` extrema costs it nothing.
//! 2. **Where that plane sits is not invariant** — perturb the frame and the vertices move by
//!    exactly the perturbation. That is the whole diagnostic value: a mis-picked plane is visible,
//!    and claim 1 keeps holding while it happens, so claim 1 alone would never catch it.

use author::dump::{plane_residual, sketch_faces};
use author::part::Cutter;
use export::approx::surd_to_f64;
use lattice::{Bignum, Rat};

type Q = Rat<Bignum>;

/// The signed distance from `p` to the boundary of a rounded rectangle — zero on the outline.
fn rounded_box_sdf(p: [f64; 2], c: [f64; 2], half: [f64; 2], r: f64) -> f64 {
    let qx = (p[0] - c[0]).abs() - (half[0] - r);
    let qy = (p[1] - c[1]).abs() - (half[1] - r);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    outside + qx.max(qy).min(0.0) - r
}

fn f(q: &Q) -> f64 {
    let (n, d) = q.numer_denom_decimal();
    n.parse::<f64>().unwrap() / d.parse::<f64>().unwrap()
}

/// **Claim 1, on the real acceptance device.** Every emitted vertex satisfies its own frame's plane
/// equation as an exact rational identity — the residual is `0`, not "small".
#[test]
fn every_sketch_vertex_is_exactly_in_the_plane_it_was_built_from() {
    let part = acceptance::contour_panel(48, None);
    let dump = sketch_faces(&part, 20);

    assert_eq!(dump.cutters, 1, "the panel has one extruded cutter");
    assert_eq!(dump.brep.faces().len(), 1, "one closed profile loop");
    assert!(
        dump.brep.verts().len() > 60,
        "four sides and four chorded corners, not a bounding box"
    );
    assert_eq!(
        dump.plane_residual,
        Q::from_i128(0),
        "a vertex is off its own frame's plane: {}",
        dump.summary()
    );

    // …and the residual really is being computed against the frame, not trivially zero: the same
    // vertices measured against a *different* plane are nowhere near it.
    let Some((_, Cutter::Extrude(e))) = part
        .cutters()
        .find(|(_, c)| matches!(c, Cutter::Extrude(_)))
    else {
        panic!("expected an extruded cutter")
    };
    let elsewhere = develop::extrude::Frame::new(
        [Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)],
        e.frame.u().clone(),
        e.frame.v().clone(),
    )
    .expect("a parallel plane one unit away");
    let mut worst = Q::from_i128(0);
    for v in dump.brep.verts() {
        let p = [
            Q::from_i128(0).add(&rat_of(&v[0])),
            rat_of(&v[1]),
            rat_of(&v[2]),
        ];
        let r = plane_residual(&elsewhere, &p);
        let r = if r.sign() < 0 { r.neg() } else { r };
        if r > worst {
            worst = r;
        }
    }
    assert!(
        worst.sign() > 0,
        "measured against another plane the residual must NOT be zero, or the metric is vacuous"
    );
}

/// The dump's vertices are rational (`b = 0` surds) by construction.
fn rat_of(s: &lattice::Surd<Bignum>) -> Q {
    export::approx::f64_to_rat::<Bignum>(surd_to_f64(s), 40)
}

/// **Claim 2 — a mis-picked plane is visible, and by exactly how much.**
///
/// The same profile on a frame shifted one unit along its normal emits the same outline one unit
/// away. Claim 1 holds for *both*, which is the point: a certificate on the snap, and a residual
/// against the frame, are both blind to the frame being in the wrong place. Only the picture is not.
#[test]
fn perturbing_the_frame_moves_the_sketch_by_exactly_the_perturbation() {
    let (cx, cy, w, h, r) = acceptance::contour_outline_geometry();
    let profile = acceptance::rounded_outline(cx, cy, w, h, r);
    let unit = |k: usize| {
        let mut v = [Q::from_i128(0), Q::from_i128(0), Q::from_i128(0)];
        v[k] = Q::from_i128(1);
        v
    };
    let build = |origin: [Q; 3]| {
        let frame = develop::extrude::Frame::new(origin, unit(0), unit(1)).expect("a frame");
        let apex = develop::extrude::Apex::direction(unit(2)).expect("a direction");
        author::construct::cone::<Bignum>(30.0)
            .region_sigma(
                Q::from_i128(-1),
                Q::from_i128(1),
                author::part::SupportFn::inherit(),
            )
            .intersect(Cutter::extrude(frame, apex, profile.clone()))
    };

    let flat = build([Q::from_i128(0), Q::from_i128(0), Q::from_i128(0)]);
    let lifted = build([Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)]);
    let (a, b) = (sketch_faces(&flat, 20), sketch_faces(&lifted, 20));

    // Both are exactly in their own planes — the invariant claim survives the perturbation.
    assert_eq!(a.plane_residual, Q::from_i128(0));
    assert_eq!(b.plane_residual, Q::from_i128(0));

    // …and the perturbation is visible, vertex for vertex, at exactly its own size.
    let va = a.brep.verts();
    let vb = b.brep.verts();
    assert_eq!(va.len(), vb.len(), "the same outline, sampled the same way");
    assert!(!va.is_empty());
    for (p, q) in va.iter().zip(vb) {
        assert!(
            (surd_to_f64(&p[0]) - surd_to_f64(&q[0])).abs() < 1e-12,
            "x unmoved"
        );
        assert!(
            (surd_to_f64(&p[1]) - surd_to_f64(&q[1])).abs() < 1e-12,
            "y unmoved"
        );
        let dz = surd_to_f64(&q[2]) - surd_to_f64(&p[2]);
        assert!(
            (dz - 1.0).abs() < 1e-12,
            "z moved by {dz}, not by the unit shift"
        );
    }
}

/// **The importer's faithfulness echo.** The emitted outline is the authored rounded rectangle —
/// not its bounding box, not a convexification, not a shape that lost its radii. The frame here is
/// orthonormal, so in-plane distance is metric distance and the check is direct.
#[test]
fn the_sketch_outline_is_the_authored_shape_and_not_its_bounding_box() {
    let (cx, cy, w, h, r) = acceptance::contour_outline_geometry();
    let part = acceptance::contour_panel(48, None);
    let dump = sketch_faces(&part, 20);
    let (c, half, rr) = ([f(&cx), f(&cy)], [f(&w), f(&h)], f(&r));

    // Chording a 90° corner into 16 pieces leaves a sagitta of ~1.2e-4; the 2⁻²⁰ snap adds ~1e-6.
    let budget = 3e-4;
    let mut worst = 0.0f64;
    for v in dump.brep.verts() {
        let d = rounded_box_sdf([surd_to_f64(&v[0]), surd_to_f64(&v[1])], c, half, rr).abs();
        worst = worst.max(d);
    }
    assert!(
        worst < budget,
        "worst deviation from the authored outline: {worst}"
    );

    // Non-vacuous: the bounding *rectangle* of the same outline would fail, because the corners are
    // where a lost radius shows up and the sdf is `r` away from a square corner.
    let square_corner = rounded_box_sdf([f(&cx) + f(&w), f(&cy) + f(&h)], c, half, rr).abs();
    assert!(
        square_corner > budget * 10.0,
        "the check must be able to tell a rounded corner from a square one"
    );
}

/// **The dump is structurally incapable of being mistaken for a certified solid.** It is an open
/// shell — every sketch edge is free — so the closed-shell certificate refuses it outright. That is
/// the property, not a convention: a picture that could pass for a part is the failure this guards.
#[test]
fn the_dump_is_an_open_shell_that_no_certificate_would_accept() {
    let part = acceptance::contour_panel(48, None);
    let dump = sketch_faces(&part, 20);
    assert!(
        dump.brep.free_edges() > 0,
        "a lone planar face is all boundary"
    );
    assert_eq!(
        dump.brep.free_edges(),
        dump.brep.verts().len(),
        "a single closed wire: as many free edges as vertices"
    );

    use certify_core::Verdict;
    let sc = dump.brep.to_shell_certificate();
    assert!(
        !matches!(
            certify_core::shell::closed_shell_holed(
                sc.n_verts,
                &sc.edge_start,
                &sc.edge_end,
                &sc.wire_edge,
                &sc.wire_reversed,
                &sc.loop_start,
                &sc.face_start,
            ),
            Verdict::Verified(_)
        ),
        "the closed-shell certificate must REFUSE a diagnostic sketch"
    );
}
