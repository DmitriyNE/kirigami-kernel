//! **IO.3 — the diagnostic dump: the sketch it was cut with, and the body the cut swept.**
//!
//! A cutter's frame is a *search result*: `develop::pick` snaps a picked plane to rationals and
//! certifies the snap. What no certificate can say is whether the **pick** landed where the author
//! meant — that is a question about intent, and a picture is the only instrument for it.
//!
//! So the first half of this file asserts two claims about the **sketch** (IO.3a) that are easy to
//! conflate and must not be:
//!
//! 1. **The face lies in its frame's plane, exactly.** `N·(X − o) = 0` as a rational equality, for
//!    every vertex, with no tolerance. This is invariant under *any* in-plane sampling, which is
//!    why chording the arcs and snapping the `Surd` extrema costs it nothing.
//! 2. **Where that plane sits is not invariant** — perturb the frame and the vertices move by
//!    exactly the perturbation. That is the whole diagnostic value: a mis-picked plane is visible,
//!    and claim 1 keeps holding while it happens, so claim 1 alone would never catch it.
//!
//! The second half is the **body** (IO.3b), and its headline claim is a *differential*: the near
//! cap and the sketch face are the same closed curve reached by two computations that share no
//! code. See the section break below.

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

// ─────────────────────────── IO.3b — the body the cut actually swept ───────────────────────────
//
// `sketch_faces` shows what was *asked for*; `cutter_bodies` shows what was *got*. The pair is
// what makes either worth emitting, and the tests below split the claims the same way the geometry
// does: one about the body's structure, one about the two routes agreeing, one about a cutter that
// reaches two sheets, and one about a cutter that has no sketch plane at all.

use author::dump::cutter_bodies;
use certify_core::Verdict;

/// The chord budget the solid path certifies at — the number the body is drawn from.
const BODY_SEGMENTS: usize = 16;

/// A parallel sweep along `z`, the apex every AUTH.2 fixture is cut with.
fn parallel() -> develop::extrude::Apex<Bignum> {
    develop::extrude::Apex::direction([Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)])
        .expect("a real direction")
}

/// The AUTH.2 L-slot device: a non-convex traced footprint on the Stage-1 gore.
fn ell_device() -> author::part::Part<Bignum> {
    acceptance::sketch_panel(Some((parallel(), acceptance::ell_slot())))
}

/// The authored L outline as float segments, read off the fixture's own edges rather than restated
/// — a duplicated corner list is a golden that drifts.
fn ell_segments() -> Vec<([f64; 2], [f64; 2])> {
    acceptance::ell_slot()
        .iter()
        .map(|e| match e {
            geom::content::Edge::Seg(s) => (
                [surd_to_f64(&s.start.x), surd_to_f64(&s.start.y)],
                [surd_to_f64(&s.end.x), surd_to_f64(&s.end.y)],
            ),
            geom::content::Edge::Arc(_) => panic!("the L-slot is a polygon"),
        })
        .collect()
}

/// The distance from `p` to the segment `a b`.
fn seg_dist(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    ((p[0] - a[0] - t * dx).powi(2) + (p[1] - a[1] - t * dy).powi(2)).sqrt()
}

fn verified<T>(v: Verdict<T, author::part::PartFault, Q>, what: &str) -> T {
    match v {
        Verdict::Verified(t) => t,
        Verdict::Refuted(f) => panic!("{what}: refuted: {f:?}"),
        Verdict::Unresolved(e) => panic!(
            "{what}: unresolved at ε ≈ {}",
            export::approx::rat_to_f64(&e)
        ),
    }
}

/// **The body closes, and that is a check on the tracer.**
///
/// A footprint is a simple closed curve, so the near cap, the walls and the far cap sew into a
/// sphere: every edge falls to exactly two triangles, and `V − E + F = 2`. Nothing here compares a
/// coordinate — the shell is watertight *by identity*, because the caps share one triangulation and
/// the walls share the caps' boundary edges — so a footprint that self-crossed, doubled back, or
/// dropped a vertex could not produce this, whatever ε it certified at.
///
/// And the guard the whole module exists for is asserted from the other side: absorbing the sketch
/// face into the same compound **reopens** it, so what a viewer receives can never pass for a part.
#[test]
fn the_cutter_body_closes_over_its_own_footprint() {
    let part = ell_device();
    let dump = verified(cutter_bodies(&part, BODY_SEGMENTS), "the L-slot body");

    assert_eq!(
        dump.bodies.len(),
        1,
        "one cutter, one footprint on one sheet"
    );
    let body = &dump.bodies[0];
    assert!(
        body.solid,
        "an extruded cutter has a sketch plane to cast back to"
    );
    assert!(
        body.vertices > 20,
        "a traced L-slot footprint, not a bounding box: {} vertices",
        body.vertices
    );

    let (v, e, f) = (
        dump.brep.verts().len(),
        dump.brep.edges().len(),
        dump.brep.faces().len(),
    );
    assert_eq!(
        v,
        2 * body.vertices,
        "one near-cap and one far-cap vertex each"
    );
    assert_eq!(
        f,
        2 * (body.vertices - 2) + 2 * body.vertices,
        "two ear-clipped caps of n−2 triangles, plus two per wall quad"
    );
    assert_eq!(dump.brep.free_edges(), 0, "a closed shell has no free edge");
    assert_eq!(dump.brep.nonmanifold_edges(), 0);
    assert_eq!(
        v as i64 - e as i64 + f as i64,
        2,
        "χ = 2: the shell is a sphere, so the footprint bounded a disc"
    );

    // The near cap is in the sketch plane exactly — the same rational identity `sketch_faces`
    // reports, reached by a completely different route (cast back, not authored).
    assert_eq!(dump.near_residual, Q::from_i128(0));

    // …and the certificate agrees the *body* is closed, where it refuses the sketch face.
    let sc = dump.brep.to_shell_certificate();
    assert!(
        matches!(
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
        "the traced footprint's body must sew into a closed shell"
    );

    // The compound guard: one open sketch face reopens the whole thing.
    let mut compound = dump.brep;
    let sketch = sketch_faces(&part, 20);
    let open = sketch.brep.free_edges();
    assert!(open > 0);
    compound.absorb(sketch.brep);
    assert_eq!(
        compound.free_edges(),
        open,
        "absorbing is a compound, not a sew: the body keeps its seams and the sketch keeps its \
         boundary, so a dump can never pass for a part"
    );
}

/// **The near cap traces the authored profile — and neither computation knows about the other.**
///
/// One route is the sketch: authored profile edges, sampled in frame coordinates, placed by
/// `Frame::point`. The other is the body's near cap: the resolver's traced `(σ, µ̂)` footprint,
/// lifted onto the chart, cast *back* down its own generatrices, and only then read in frame
/// coordinates. They land on the same curve only if the tracer, the chart and the frame all agree,
/// and no certified ε would report it if they did not — ε bounds the cut, not the correspondence.
#[test]
fn the_near_cap_traces_the_authored_profile() {
    let dump = verified(
        cutter_bodies(&ell_device(), BODY_SEGMENTS),
        "the L-slot body",
    );
    let segs = ell_segments();
    let near: Vec<[f64; 2]> = dump
        .brep
        .verts()
        .iter()
        .filter(|v| surd_to_f64(&v[2]).abs() < 1e-12)
        .map(|v| [surd_to_f64(&v[0]), surd_to_f64(&v[1])])
        .collect();
    assert_eq!(
        near.len(),
        dump.bodies[0].vertices,
        "the whole near cap, in the z = 0 plane"
    );

    let dist = |p: [f64; 2], segs: &[([f64; 2], [f64; 2])]| {
        segs.iter()
            .map(|(a, b)| seg_dist(p, *a, *b))
            .fold(f64::INFINITY, f64::min)
    };
    // `hole_poly` snaps the footprint to a 2⁻³⁰ ≈ 9.3e-10 dyadic grid, and that — not the cut's
    // certified ε ≈ 7e-4 — is what the correspondence is good to: the tracer walks the *exact*
    // wall equations, so casting back lands on the profile itself.
    let budget = 1e-8;
    let worst = near.iter().map(|&p| dist(p, &segs)).fold(0.0, f64::max);
    assert!(
        worst < budget,
        "worst near-cap distance to the authored L: {worst:e} (budget {budget:e})"
    );

    // Non-vacuous from two sides. First: the same measurement against the same L shifted by 1/10
    // — a displacement a hundred-thousandth the size of which would already fail above.
    let moved: Vec<_> = segs
        .iter()
        .map(|(a, b)| ([a[0] + 0.1, a[1]], [b[0] + 0.1, b[1]]))
        .collect();
    let off = near.iter().map(|&p| dist(p, &moved)).fold(0.0, f64::max);
    assert!(
        off > budget * 1e5,
        "measured against a displaced outline the distance must blow up, or the check is empty: \
         {off:e}"
    );

    // Second: the near cap *covers* the outline rather than clustering on one edge of it — an
    // extent within a snap of the authored L's own.
    let ext = |f: fn(&[f64; 2]) -> f64, pts: &[[f64; 2]]| {
        pts.iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
                (lo.min(f(p)), hi.max(f(p)))
            })
    };
    let corners: Vec<[f64; 2]> = segs.iter().map(|(a, _)| *a).collect();
    for (axis, name) in [(0usize, "x"), (1, "y")] {
        let (a, b) = match axis {
            0 => (ext(|p| p[0], &near), ext(|p| p[0], &corners)),
            _ => (ext(|p| p[1], &near), ext(|p| p[1], &corners)),
        };
        assert!(
            (a.0 - b.0).abs() < budget && (a.1 - b.1).abs() < budget,
            "the near cap's {name}-extent {a:?} is not the authored outline's {b:?}"
        );
    }
}

/// **One cutter through the lap is one cutter per sheet.** The self-lapping cone passes over
/// itself, so the seam drill pierces the material twice — and the two footprints land on *different
/// regions*, which is exactly why the region travels with the loop instead of being searched for
/// afterwards.
#[test]
fn a_cutter_through_the_lap_gives_a_body_per_sheet() {
    let dump = verified(
        cutter_bodies(&acceptance::self_lapping_cone(16, 8, true), BODY_SEGMENTS),
        "the self-lapping seam drill",
    );
    assert_eq!(
        dump.bodies.len(),
        2,
        "the drill pierces the head and the lapping tail"
    );
    assert_eq!(
        dump.bodies[0].op, dump.bodies[1].op,
        "both footprints belong to the same authored op"
    );
    assert_ne!(
        dump.bodies[0].region, dump.bodies[1].region,
        "…on two different regions of the development — the body and the tail plateau"
    );
}

/// **A metric cutter gets its far cap and no more.** A drill has no sketch plane to cast back to,
/// so there is no near cap to emit and nothing honest to rule walls between. What comes out is the
/// footprint on the sheet, as an open patch that says so.
#[test]
fn a_metric_cutter_yields_an_open_far_cap() {
    let part = acceptance::sketch_drill(Q::from_i128(0), Q::new(11, 5), Q::new(1, 25));
    let dump = verified(cutter_bodies(&part, BODY_SEGMENTS), "the drilled disc");

    assert_eq!(dump.bodies.len(), 1);
    let body = &dump.bodies[0];
    assert!(!body.solid, "a cylinder has no frame, so no near cap");
    assert_eq!(dump.brep.verts().len(), body.vertices, "one cap, not two");
    assert_eq!(
        dump.brep.faces().len(),
        body.vertices - 2,
        "one ear-clipped cap of n−2 triangles"
    );
    assert_eq!(
        dump.brep.free_edges(),
        body.vertices,
        "an open patch: its boundary is the footprint loop itself"
    );
    // No near cap was emitted, so the plane residual has nothing to be nonzero about.
    assert_eq!(dump.near_residual, Q::from_i128(0));
}
