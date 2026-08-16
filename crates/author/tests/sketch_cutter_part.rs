//! **AUTH.1f / AUTH.2f — the sketch-extrude cutter, checked for faithfulness rather than for
//! verdicts.**
//!
//! A cut authored with `Cutter::extrude` must do what it was *drawn* to do, and the certificates
//! cannot say whether it did: `ε` is the max over pipeline stages and the panel's boundary
//! dominates it, so a drafted hole and an undrafted one report the **same** `ε`. Only the emitted
//! geometry distinguishes them, which is what these tests measure.
//!
//! The device, the profiles and the measurements all come from the `acceptance` crate, so the demo
//! driver reports the same numbers these assert over the same part.

use acceptance::measure;
use arrange2d::profile::Profile;
use author::part::{OpRole, Part, PartFault};
use certify_core::Verdict;
use develop::counters;
use develop::extrude::Apex;
use export::approx::rat_to_f64;
use lattice::{Bignum, Rat};

type Q = Rat<Bignum>;

fn q(n: i128, d: i128) -> Q {
    Q::new(n, d)
}
fn qi(n: i128) -> Q {
    Q::from_i128(n)
}

/// A disc's boundary, through the shared `Profile` builder — no hand-built arcs.
fn disc(cx: Q, cy: Q, r: Q) -> Vec<geom::content::Edge<Bignum>> {
    Profile::new().circle(cx, cy, r).into_edges()
}

/// The axis-aligned square of half-side `h` about `(cx, cy)`.
fn square(cx: Q, cy: Q, h: Q) -> Vec<geom::content::Edge<Bignum>> {
    Profile::new().rect(cx, cy, h.clone(), h).into_edges()
}

/// A parallel (`w = 0`) sweep along `z` — the apex every AUTH.2 fixture is cut with.
fn parallel() -> Apex<Bignum> {
    Apex::direction([qi(0), qi(0), qi(1)]).expect("a real direction")
}

/// The acceptance panel with `profile` swept from `apex`.
fn panel_with(apex: Apex<Bignum>, profile: Vec<geom::content::Edge<Bignum>>) -> Part<Bignum> {
    acceptance::sketch_panel(Some((apex, profile)))
}

/// The genus of a closed shell: `χ = V − E + (2F − L)` over the doubled faces, `g = (2 − χ)/2`.
fn genus(b: &export::brep::Brep<Bignum>) -> i64 {
    let v = b.verts().len() as i64;
    let e = b.edges().len() as i64;
    let f = b.faces().len() as i64;
    let l: i64 = b.faces().iter().map(|fc| 1 + fc.holes.len() as i64).sum();
    (2 - (v - e + (2 * f - l))) / 2
}

fn develop_or_panic(part: Part<Bignum>, name: &str) -> author::part::FlatPattern<Bignum> {
    match part.develop() {
        Verdict::Verified(f) => f,
        Verdict::Refuted(fault) => panic!("{name}: refuted: {fault:?}"),
        Verdict::Unresolved(e) => panic!("{name}: unresolved at ε ≈ {}", rat_to_f64(&e)),
    }
}

fn solid_or_panic(part: Part<Bignum>, name: &str) -> author::part::PartSolid<Bignum> {
    match part.solid() {
        Verdict::Verified(s) => s,
        Verdict::Refuted(fault) => panic!("{name}: solid refuted: {fault:?}"),
        Verdict::Unresolved(e) => panic!("{name}: solid unresolved at ε ≈ {}", rat_to_f64(&e)),
    }
}

fn developed(apex: Apex<Bignum>) -> author::part::FlatPattern<Bignum> {
    develop_or_panic(
        panel_with(apex, disc(qi(0), q(11, 5), q(1, 5))),
        "the AUTH.1f disc",
    )
}

/// The one interior hole's emitted ring, as the SVG draws it.
fn hole_ring(flat: &author::part::FlatPattern<Bignum>) -> Vec<[f64; 2]> {
    let faces = measure::emitted_hole_rings(flat.region());
    assert_eq!(faces.len(), 1, "one face");
    assert_eq!(faces[0].len(), 1, "one interior hole");
    faces[0][0].clone()
}

/// The developed hole's width, through the **quarantined** exact→`f64` bridge — never a hand-rolled
/// conversion, which returns NaN on large rationals and is then swallowed by `min`/`max`.
fn hole_width(flat: &author::part::FlatPattern<Bignum>) -> f64 {
    let ring = hole_ring(flat);
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for p in &ring {
        assert!(p[0].is_finite(), "the bridge must not emit a non-finite x");
        lo = lo.min(p[0]);
        hi = hi.max(p[0]);
    }
    hi - lo
}

/// **The faithfulness criterion.** The same profile disc, swept from a finite cast point and from a
/// direction, must produce holes whose sizes differ by exactly the taper the cast point implies:
/// a cone from height `z_apex` has narrowed to `1 − z/z_apex` of its profile radius by height `z`.
///
/// This is the check `ε` cannot make. Both variants certify at the *same* `ε` — it is the max over
/// stages and the panel boundary dominates — so a test that only asserted `Verified` would pass
/// just as happily on a cutter that ignored its apex entirely.
#[test]
fn a_drafted_hole_is_smaller_by_exactly_its_taper() {
    let drafted = developed(Apex::point([qi(0), q(11, 5), qi(12)]));
    let parallel = developed(parallel());

    let (a, b) = (hole_width(&drafted), hole_width(&parallel));
    assert!(a > 0.0 && b > 0.0, "both holes must be measurable");
    // The panel's hole sits at z ≈ 2.44 and the cast point at 12, so the cone has narrowed to
    // ≈ 0.797 of the profile radius there.
    let ratio = a / b;
    assert!(
        (ratio - 0.797).abs() < 0.01,
        "the drafted hole should be ≈0.797 of the parallel one, got {ratio:.4} \
         ({a:.4} vs {b:.4}) — the cut is certified but not the shape that was drawn"
    );
}

/// The general cutter **is** the special one it generalizes: the same disc swept along `z` is
/// `Cutter::vertical_cylinder`, and the two develop to the same ε through the whole pipeline —
/// not merely to the same resolved structure, which is all AUTH.1e.2's differential compared.
#[test]
fn a_parallel_extrusion_reproduces_the_metric_cylinder() {
    let extruded = developed(parallel());
    let metric = develop_or_panic(
        acceptance::sketch_drill(qi(0), q(11, 5), q(1, 25)),
        "the metric control",
    );

    let (a, b) = (hole_width(&extruded), hole_width(&metric));
    assert!(
        (a - b).abs() < 1e-6 * b.max(1.0),
        "the extruded disc should cut the same hole as the cylinder it is: {a:.6} vs {b:.6}"
    );
    assert!(
        rat_to_f64(extruded.eps()) - rat_to_f64(metric.eps()) < 1e-12,
        "and certify at the same ε"
    );
}

/// **AUTH.1e.4 — a many-walled profile realizes, and cuts the shape it was drawn as.**
///
/// A square prism's hole must contain the hole of the cylinder inscribed in it and sit inside the
/// hole of the one circumscribing it: `disc(h) ⊂ square(h) ⊂ disc(h√2)` as 3-D solids, and the
/// development is a bijection on the panel, so the same inclusion holds for the developed holes and
/// therefore for their widths. Both bounds come from the **metric** cylinder path, which shares no
/// code with the wall-crossing band builder — a differential, not a restatement.
///
/// This is the check AUTH.1f's disc could not make. Its hole is bounded by *one* wall, so it never
/// exercises the thing 1e.4 adds: a boundary whose governing wall changes at every profile corner.
#[test]
fn a_square_prism_cuts_a_hole_between_its_inscribed_and_circumscribed_discs() {
    let h = q(1, 5);
    let square_hole = develop_or_panic(
        panel_with(parallel(), square(qi(0), q(11, 5), h.clone())),
        "the square prism",
    );
    let inscribed = develop_or_panic(
        acceptance::sketch_drill(qi(0), q(11, 5), h.mul(&h)),
        "the inscribed cylinder",
    );
    let circumscribed = develop_or_panic(
        acceptance::sketch_drill(qi(0), q(11, 5), h.mul(&h).mul(&qi(2))),
        "the circumscribed cylinder",
    );
    let (sq, lo, hi) = (
        hole_width(&square_hole),
        hole_width(&inscribed),
        hole_width(&circumscribed),
    );
    assert!(
        lo < sq && sq < hi,
        "the square's hole must sit strictly between its two discs': {lo:.4} < {sq:.4} < {hi:.4}"
    );
}

/// **The ring is refused by name — and since AUTH.2c, for its own reason.** The tracer has no
/// trouble reading an annular footprint: it is two loops, one inside the other. What makes it a
/// refusal is the geometry that would describe — a through-cut leaving a disc of material floating
/// free, which is two parts rather than one hole. So the part still reports
/// [`PartFault::ProfileNotSimple`], now off `ShadowNested` rather than off a band representation
/// that could not express two stretches.
#[test]
fn a_ring_profile_is_refused_by_name() {
    let part = panel_with(parallel(), acceptance::ring_slot());
    match part.develop() {
        Verdict::Refuted(PartFault::ProfileNotSimple { .. }) => {}
        other => panic!(
            "a ring must be refused as not-a-band, got {}",
            match other {
                Verdict::Verified(_) => "Verified".to_string(),
                Verdict::Refuted(f) => format!("Refuted({f:?})"),
                Verdict::Unresolved(e) => format!("Unresolved({})", rat_to_f64(&e)),
            }
        ),
    }
}

/// **AUTH.2d — a non-convex cutter develops, and the flat pattern keeps its reflex corner.**
///
/// Two things are checked beyond the verdict, because a verdict cannot see either: the topology is
/// what an L should give (one face, one hole — a footprint the tracer split in two, or one it
/// silently closed around the notch, changes this count), and the hole is still non-convex where it
/// matters. A developed L must turn **both ways**; a hole quietly convexified to its bounding band
/// passes every ε gate ever written.
///
/// The **VV.2 ε budget** and the **VV.3 chord golden** for this fixture are pinned here, on the
/// same emitted polygon.
#[test]
fn an_l_slot_develops_and_keeps_its_reflex_corner() {
    let flat = develop_or_panic(panel_with(parallel(), acceptance::ell_slot()), "the L-slot");
    let ring = hole_ring(&flat);
    let n = ring.len();
    assert!(
        n >= 6,
        "an L's developed hole needs at least its six corners"
    );
    let cross = |a: [f64; 2], b: [f64; 2], c: [f64; 2]| {
        (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0])
    };
    // Chords along a curved rail turn very slightly; the reflex corner turns by a right angle, so
    // the test asks for a turn of real size rather than merely a sign.
    let scale: f64 = (0..n)
        .map(|i| cross(ring[i], ring[(i + 1) % n], ring[(i + 2) % n]).abs())
        .fold(0.0, f64::max);
    let (mut left, mut right) = (false, false);
    for i in 0..n {
        let t = cross(ring[i], ring[(i + 1) % n], ring[(i + 2) % n]);
        if t > 0.05 * scale {
            left = true;
        }
        if t < -0.05 * scale {
            right = true;
        }
    }
    assert!(
        left && right,
        "the developed L must turn both ways — a convexified hole turns only one"
    );

    // **VV.2** — measured 4.8792e-1 (2026-08-16) against the DRC ceiling of 1/2, so this ~296° gore
    // certifies at 98% of it: the budget is a ratchet here rather than the flex panel's 55%
    // headroom, and a widening of the boundary bound would surface as `Unresolved` before it
    // surfaced here.
    println!("[budget] L-slot develop {:.4e}", rat_to_f64(flat.eps()));
    assert!(
        flat.eps().cmp(&q(1, 2)) == core::cmp::Ordering::Less,
        "L-slot develop ε {:.4e} is not under the DRC gate 5.0000e-1",
        rat_to_f64(flat.eps())
    );
    // **The σ-midpoint honesty check, read as a number.** The cut's *own* certified bound — the one
    // `eps()` hides, since the panel boundary dominates it — folds in the comparison of every
    // emitted piece against the boundary the exact fill rule reports at its σ-midpoint (§11.5).
    // Measured 2.8439e-4 (2026-08-16) — three orders under the part-level ε, which is the point:
    // the panel boundary is what this device certifies loosely, and the traced cut is not.
    let cut = flat.report().ops[3]
        .cut_eps
        .clone()
        .expect("the slot resolved as a hole and so carries its own cut bound");
    println!("[budget] L-slot cut {:.4e}", rat_to_f64(&cut));
    assert!(
        cut.cmp(&q(1, 1000)) == core::cmp::Ordering::Less,
        "the traced cut certified to {:.4e}, above its 1.0000e-3 budget",
        rat_to_f64(&cut)
    );

    // **VV.3** — the chord golden. Measured 7.7% (2026-08-16); the 20% gate is the structural one,
    // wider than the round holes' 15% because an L's own long straight arm is legitimately a large
    // fraction of its bounding box.
    let frac = measure::longest_edge_fraction(&ring);
    println!(
        "[golden] L-slot hole: longest edge {:.1}% of extent",
        frac * 100.0
    );
    assert!(
        frac < 0.20,
        "the L-slot's longest emitted edge is {:.1}% of its extent",
        frac * 100.0
    );
}

/// **The fixture produces the phenomenon, and the metric probes bound the shape — both measured on
/// the emitted flat pattern.**
///
/// Two independent things, in one test because they read the same four developments.
///
/// *A ruling meets the cutter twice.* The development is an isometry sending each ruling to a ray
/// from the flat apex, so this is a ray meeting the developed hole in **two** intervals — four
/// crossings. Every band footprint gives two however non-convex its flat shape, which is why the
/// reflex corner above proves nothing about the footprint (§11.6); the three metric discs are
/// measured here as the control and give exactly two.
///
/// *The two-sided differential.* AUTH.1e.4 sandwiched a square between two discs; a non-convex
/// footprint needs a third clause, because both containments are satisfied by a slot silently
/// convexified to its bounding band. So: the L **contains** a disc inscribed in one arm,
/// **lies within** a disc circumscribing it, and is **disjoint** from a disc inside the notch it
/// does not cover. All three come from `Cutter::vertical_cylinder` — the metric path, which shares
/// no line of code with the tracer — and all three are compared in the same flat frame, since the
/// four parts differ only in the cutter and so develop identically everywhere else.
#[test]
fn the_traced_slot_is_met_four_times_and_bracketed_by_its_metric_probes() {
    let slot = hole_ring(&develop_or_panic(
        panel_with(parallel(), acceptance::ell_slot()),
        "the L-slot",
    ));
    let [inner, outer, notch] = acceptance::ell_probes();
    let probe = |(cx, cy, r2): (Q, Q, Q), name: &str| {
        hole_ring(&develop_or_panic(
            acceptance::sketch_drill(cx, cy, r2),
            name,
        ))
    };
    let inner = probe(inner, "the inscribed probe");
    let outer = probe(outer, "the circumscribing probe");
    let notch = probe(notch, "the notch probe");

    let (rs, ri, ro, rn) = (
        measure::max_ray_crossings(&slot),
        measure::max_ray_crossings(&inner),
        measure::max_ray_crossings(&outer),
        measure::max_ray_crossings(&notch),
    );
    println!("[phenom] ray crossings: slot {rs}, probes {ri}/{ro}/{rn}");
    assert_eq!(
        rs, 4,
        "some ruling must meet the slot twice — a footprint met once everywhere is a band, and \
         AUTH.2 would not be on this demo's critical path"
    );
    assert_eq!(
        (ri, ro, rn),
        (2, 2, 2),
        "and every metric disc is met once, which is what makes the four a signature"
    );

    let (ai, as_, ao, an) = (
        measure::ring_area(&inner),
        measure::ring_area(&slot),
        measure::ring_area(&outer),
        measure::ring_area(&notch),
    );
    println!("[diff] areas: inner {ai:.6} < slot {as_:.6} < outer {ao:.6}   notch {an:.6}");
    assert!(
        measure::ring_inside(&inner, &slot),
        "the slot must contain the disc inscribed in its arm"
    );
    assert!(
        measure::ring_inside(&slot, &outer),
        "…and lie inside the disc that circumscribes it"
    );
    assert!(
        measure::rings_disjoint(&notch, &slot),
        "…and leave the notch alone: a slot convexified to its bounding band swallows it, and \
         passes both containments while doing so"
    );
    assert!(
        ai < as_ && as_ < ao,
        "the areas order with the containments"
    );
}

/// **AUTH.2 end to end.** A non-convex *footprint* — traced, not authored — becomes a tunnel
/// through the device. Both of the solid path's restrictions are gone:
///
/// 1. `hole_rail` consumed an interior hole as a near/far **band**, which a loop the ruling meets
///    twice is not (2e/1): such a loop now goes to the builder's general channel as the `(σ, µ̂)`
///    polygon it already is.
/// 2. That channel took an arbitrary loop only **within one σ-slice** (2e/2): it now clips the loop
///    per slice with the exact boolean, so a loop crossing stations is ordinary.
///
/// The verdict alone would pass on a cut that missed, so the checks are topological and counted:
/// the slot adds **exactly one** handle to the same panel built without it — it goes all the way
/// through, once — and the builder's general channel ran on **more than one slice**, which is the
/// only thing that distinguishes a hole that crossed a σ-station from one that did not. Both
/// certify, both build; nothing else in the emitted solid tells them apart.
#[test]
fn a_traced_non_convex_loop_builds_a_certified_solid_across_a_station() {
    counters::reset();
    let cut = solid_or_panic(
        panel_with(parallel(), acceptance::ell_slot()),
        "the traced L-slot",
    );
    let clips = counters::poly_slice_clips();
    let brep = cut.brep();
    assert_eq!(brep.free_edges(), 0, "the slotted solid is watertight");
    assert_eq!(brep.nonmanifold_edges(), 0, "…and manifold");
    println!(
        "[work] L-slot solid: {clips} polygon-channel slice clips, {} faces",
        brep.faces().len()
    );
    assert!(
        clips >= 2,
        "the slot is the part's only polygon hole, so {clips} slice clip(s) means it sat inside a \
         single σ-slice — AUTH.2e/2 is then not exercised and the demo proves less than it claims"
    );

    counters::reset();
    let plain = solid_or_panic(acceptance::sketch_panel(None), "the un-slotted panel");
    assert_eq!(
        counters::poly_slice_clips(),
        0,
        "the control has no polygon hole at all — otherwise the counter is measuring the panel"
    );
    assert_eq!(
        genus(brep),
        genus(plain.brep()) + 1,
        "the L-slot is one tunnel through the panel — not a dent, and not two"
    );
}

/// **The keyhole: a profile that mixes a circle with straight edges, end to end.**
///
/// Every wall of the L is affine, so the L never reaches the pairwise resultant's mixed case — and
/// the published quadratic-by-quadratic closed form is *identically zero* on two affine walls, so a
/// differential built only from polygons would have missed a wrong one entirely (§11.2). The
/// keyhole's saddle joins the head's circle to a stem side, which is that case; that it does so is
/// asserted where the stretch structure is visible, in `develop`'s own sweep test.
///
/// Here it is carried through the whole device: resolved, traced, developed and built, met four
/// times by a ruling like the L, and crossing a σ-station in the solid. Its ε budget and chord
/// golden are pinned alongside.
#[test]
fn a_keyhole_profile_develops_and_builds_a_solid() {
    let flat = develop_or_panic(
        panel_with(parallel(), acceptance::keyhole_slot()),
        "the keyhole",
    );
    assert_eq!(
        flat.report().ops[3].role,
        OpRole::Hole,
        "the keyhole pierces the sheet — an `Inactive` here is a green certificate on a cut that \
         did nothing"
    );
    let ring = hole_ring(&flat);
    assert_eq!(
        measure::max_ray_crossings(&ring),
        4,
        "a ruling must cross the head, the notch beside the stem, and the stem"
    );
    // **VV.2 / VV.3** — measured 4.8792e-1 (the shared panel bound), a cut bound of 1.4320e-2 and
    // a 9.1% chord golden (2026-08-16). The cut bound is fifty times the L's: the head's circular
    // wall is chorded, where every wall of the L is straight and fits its rail exactly.
    println!(
        "[budget] keyhole develop {:.4e}   cut {:.4e}",
        rat_to_f64(flat.eps()),
        flat.report().ops[3]
            .cut_eps
            .as_ref()
            .map(rat_to_f64)
            .unwrap_or(f64::NAN)
    );
    assert!(flat.eps().cmp(&q(1, 2)) == core::cmp::Ordering::Less);
    assert!(
        flat.report().ops[3]
            .cut_eps
            .as_ref()
            .is_some_and(|e| e.cmp(&q(3, 100)) == core::cmp::Ordering::Less),
        "the keyhole's cut bound is above its 3.0000e-2 budget"
    );
    let frac = measure::longest_edge_fraction(&ring);
    println!(
        "[golden] keyhole hole: longest edge {:.1}% of extent",
        frac * 100.0
    );
    assert!(frac < 0.20, "keyhole chord golden {:.1}%", frac * 100.0);

    counters::reset();
    let solid = solid_or_panic(
        panel_with(parallel(), acceptance::keyhole_slot()),
        "the keyhole",
    );
    assert_eq!(solid.brep().free_edges(), 0, "watertight");
    assert_eq!(solid.brep().nonmanifold_edges(), 0, "manifold");
    assert!(
        counters::poly_slice_clips() >= 2,
        "the keyhole crosses a σ-station too"
    );
}

/// **Direction ② closes the loop: a developed slot vertex, folded back, lands on the profile it was
/// drawn from.**
///
/// Everything above measures direction ① — 3-D cut to flat pattern. This one runs the certified
/// fold-inversion on the flat pattern's own emitted vertices and asks where they come back to. The
/// sweep is parallel to `z`, so a point of the cutter's wall projects to a point of the **authored
/// profile's boundary**, and the residual is the distance from the recovered `(x, y)` to the L's own
/// polygon — a quantity the certificates never compute, since neither leg knows about the other.
#[test]
fn a_folded_slot_vertex_lands_on_the_profile_it_was_drawn_from() {
    let part = panel_with(parallel(), acceptance::ell_slot());
    let flat = develop_or_panic(panel_with(parallel(), acceptance::ell_slot()), "the L-slot");
    let hole = flat
        .holes()
        .first()
        .expect("the traced slot's developed loop");
    let n = hole.vertices.len();
    assert!(n >= 6, "a developed L has at least its corners");

    // The authored L's boundary, as the segments the fold must land on.
    let seg_dist = |p: [f64; 2], a: [f64; 2], b: [f64; 2]| {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len2 = dx * dx + dy * dy;
        let t = if len2 > 0.0 {
            (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let (qx, qy) = (a[0] + t * dx - p[0], a[1] + t * dy - p[1]);
        (qx * qx + qy * qy).sqrt()
    };
    let corners: Vec<[f64; 2]> = {
        // The L's own six corners, in the same rotated frame `acceptance::ell_slot` draws them in.
        let (cx, cy, a, t) = (-0.1f64, 2.2f64, 0.25f64, 0.125f64);
        let (ux, uy, vx, vy) = (0.8f64, -0.6f64, 0.6f64, 0.8f64);
        let p = |su: f64, sv: f64| [cx + ux * su + vx * sv, cy + uy * su + vy * sv];
        vec![p(0.0, 0.0), p(a, 0.0), p(a, t), p(t, t), p(t, a), p(0.0, a)]
    };

    let mut worst = 0.0f64;
    let mut folded = 0;
    for k in (0..n).step_by(n.div_ceil(8).max(1)) {
        let (x, y) = hole.vertices[k].center();
        let wire = match part.fold(&[[x, y]], &qi(0)) {
            Verdict::Verified(w) => w,
            Verdict::Unresolved(e) => panic!("fold unresolved at ε ≈ {:.3e}", rat_to_f64(&e)),
            Verdict::Refuted(f) => panic!("fold refuted at vertex {k}: {f:?}"),
        };
        let p = &wire.points[0];
        let xy = [rat_to_f64(&p[0].mid()), rat_to_f64(&p[1].mid())];
        let d = (0..corners.len())
            .map(|i| seg_dist(xy, corners[i], corners[(i + 1) % corners.len()]))
            .fold(f64::INFINITY, f64::min);
        worst = worst.max(d);
        folded += 1;
    }
    println!("[round-trip] {folded} folded slot vertices, worst profile residual {worst:.3e}");
    assert!(
        folded >= 6,
        "enough vertices to be a loop, not a spot check"
    );
    // The developed loop's own chords sit a certified ε from the true cut, and the fold adds its
    // own round-trip bound; 5e-3 is loose against both and tight against the L's 1/8 thickness —
    // a vertex that came back onto the *wrong* edge, or onto no edge, is off by that much.
    assert!(
        worst < 5e-3,
        "a folded slot vertex landed {worst:.3e} from the authored profile — direction ② does not \
         return what direction ① emitted"
    );
}
