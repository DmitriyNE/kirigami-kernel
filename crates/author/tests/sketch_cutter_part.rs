//! **AUTH.1f — the sketch-extrude cutter, checked for faithfulness rather than for verdicts.**
//!
//! A cut authored with [`Cutter::extrude`] must do what it was *drawn* to do, and the certificates
//! cannot say whether it did: `ε` is the max over pipeline stages and the panel's boundary
//! dominates it, so a drafted hole and an undrafted one report the **same** `ε`. Only the emitted
//! geometry distinguishes them, which is what these tests measure.

use arrange2d::profile::Profile;
use author::construct;
use author::part::{Cutter, Part, SupportFn};
use certify_core::Verdict;
use develop::extrude::{Apex, Frame};
use export::approx::rat_to_f64;
use fixtures::devices::cone;
use geom::content::Edge;
use lattice::{Bignum, Rat};

type Q = Rat<Bignum>;

fn q(n: i128, d: i128) -> Q {
    Q::new(n, d)
}
fn qi(n: i128) -> Q {
    Q::from_i128(n)
}

/// A disc's boundary, through the shared `Profile` builder — no hand-built arcs.
fn disc(cx: Q, cy: Q, r: Q) -> Vec<Edge<Bignum>> {
    Profile::new().circle(cx, cy, r).into_edges()
}

/// The axis-aligned square of half-side `h` about `(cx, cy)`.
fn square(cx: Q, cy: Q, h: Q) -> Vec<Edge<Bignum>> {
    Profile::new().rect(cx, cy, h.clone(), h).into_edges()
}

fn sketch_plane() -> Frame<Bignum> {
    Frame::new(
        [qi(0), qi(0), qi(0)],
        [qi(1), qi(0), qi(0)],
        [qi(0), qi(1), qi(0)],
    )
    .expect("the axes are independent")
}

/// The Stage-1 gore, with the sketch cut applied only when `cut` is given — so the same panel builds
/// with and without it and any difference is attributable to that cut alone.
fn panel_maybe(cut: Option<(Apex<Bignum>, Vec<Edge<Bignum>>)>) -> Part<Bignum> {
    let witness = cone()
        .surface(&qi(2), &qi(0))
        .eval(&qi(0))
        .expect("the device cone is regular at σ = 0");
    let base = construct::from_chart::<Bignum>(&cone())
        .region_sigma(q(-7, 2), q(7, 2), SupportFn::inherit())
        .keep_near(witness)
        .intersect(Cutter::half_space([qi(0), qi(0), qi(1)], qi(3)))
        .subtract(Cutter::vertical_cylinder(qi(0), q(1, 2), qi(2)))
        .subtract(Cutter::vertical_cylinder(q(-9, 4), q(9, 4), q(9, 16)));
    let base = match cut {
        Some((apex, profile)) => base.subtract(Cutter::extrude(sketch_plane(), apex, profile)),
        None => base,
    };
    base.clearance(qi(1)).thickness(q(1, 8)).segments(72)
}

/// The Stage-1 gore with its interior hole cut by `profile` swept from `apex`.
fn panel_with(apex: Apex<Bignum>, profile: Vec<Edge<Bignum>>) -> Part<Bignum> {
    panel_maybe(Some((apex, profile)))
}

/// The genus of a closed shell: `χ = V − E + (2F − L)` over the doubled faces, `g = (2 − χ)/2`.
fn genus(b: &export::brep::Brep<Bignum>) -> i64 {
    let v = b.verts().len() as i64;
    let e = b.edges().len() as i64;
    let f = b.faces().len() as i64;
    let l: i64 = b.faces().iter().map(|fc| 1 + fc.holes.len() as i64).sum();
    (2 - (v - e + (2 * f - l))) / 2
}

/// An L-shape with its corner at `(cx, cy)`, arm `a`, thickness `t`, CCW — but laid out on the
/// **rotated** axes `u = (3/5, 4/5)`, `v = (−4/5, 3/5)` rather than on `x`/`y`.
///
/// The rotation is the point, and it is geometry rather than decoration. This cone's rulings
/// project to *radial* rays, so an L whose arms lie along the radius is met by every ray exactly
/// once: its footprint is a band and the notch never appears in `(σ, µ̂)` at all. **Non-convex
/// profile does not imply non-convex footprint** — what AUTH.2 lifts is a restriction on
/// footprints, and a fixture has to produce one. Nor is a reflex corner in the flat pattern
/// evidence: a band `[lo(σ), hi(σ)]` can be a perfectly non-convex *region*. The signature that
/// counts is a ruling meeting the cutter **twice**, which needs the notch to open **across** the
/// rulings rather than along them — hence these axes rather than the first pair tried. The
/// `(3,4,5)` triple keeps every vertex rational, so the frame is exact rather than a rounded 45°.
fn ell(cx: Q, cy: Q, a: Q, t: Q) -> Vec<Edge<Bignum>> {
    let (ux, uy) = (q(4, 5), q(-3, 5));
    let (vx, vy) = (q(3, 5), q(4, 5));
    let p = |su: &Q, sv: &Q| {
        [
            cx.add(&ux.mul(su)).add(&vx.mul(sv)),
            cy.add(&uy.mul(su)).add(&vy.mul(sv)),
        ]
    };
    let z = qi(0);
    Profile::new()
        .polygon(&[
            p(&z, &z),
            p(&a, &z),
            p(&a, &t),
            p(&t, &t),
            p(&t, &a),
            p(&z, &a),
        ])
        .into_edges()
}

/// The AUTH.1f panel: the same gore, its feature a disc of radius `1/5`.
fn panel(apex: Apex<Bignum>) -> Part<Bignum> {
    panel_with(apex, disc(qi(0), q(11, 5), q(1, 5)))
}

/// The same gore with a **metric** cylinder of squared radius `r2` in place of the extrusion —
/// the control both faithfulness tests compare against.
fn metric_panel(r2: Q) -> Part<Bignum> {
    let witness = cone()
        .surface(&qi(2), &qi(0))
        .eval(&qi(0))
        .expect("the device cone is regular at σ = 0");
    construct::from_chart::<Bignum>(&cone())
        .region_sigma(q(-7, 2), q(7, 2), SupportFn::inherit())
        .keep_near(witness)
        .intersect(Cutter::half_space([qi(0), qi(0), qi(1)], qi(3)))
        .subtract(Cutter::vertical_cylinder(qi(0), q(1, 2), qi(2)))
        .subtract(Cutter::vertical_cylinder(q(-9, 4), q(9, 4), q(9, 16)))
        .subtract(Cutter::vertical_cylinder(qi(0), q(11, 5), r2))
        .clearance(qi(1))
        .thickness(q(1, 8))
        .segments(72)
}

fn develop_or_panic(part: Part<Bignum>, name: &str) -> author::part::FlatPattern<Bignum> {
    match part.develop() {
        Verdict::Verified(f) => f,
        Verdict::Refuted(fault) => panic!("{name}: refuted: {fault:?}"),
        Verdict::Unresolved(e) => panic!("{name}: unresolved at ε ≈ {}", rat_to_f64(&e)),
    }
}

fn developed(apex: Apex<Bignum>) -> author::part::FlatPattern<Bignum> {
    match panel(apex).develop() {
        Verdict::Verified(f) => f,
        Verdict::Refuted(fault) => panic!("refuted: {fault:?}"),
        Verdict::Unresolved(e) => panic!("unresolved at ε ≈ {}", rat_to_f64(&e)),
    }
}

/// The developed hole's width, through the **quarantined** exact→`f64` bridge — never a hand-rolled
/// conversion, which returns NaN on large rationals and is then swallowed by `min`/`max`.
fn hole_width(flat: &author::part::FlatPattern<Bignum>) -> f64 {
    let polys = export::svg::region_to_polys(flat.region());
    let face = polys.faces.first().expect("one face");
    let ring = face.rings.get(1).expect("the outer ring, then the hole");
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for p in ring {
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
    let parallel = developed(Apex::direction([qi(0), qi(0), qi(1)]).expect("a real direction"));

    // Same topology either way: one face, one interior hole.
    for (name, f) in [("drafted", &drafted), ("parallel", &parallel)] {
        assert_eq!(f.region().faces.len(), 1, "{name}: one face");
        assert_eq!(f.region().faces[0].holes.len(), 1, "{name}: one hole");
    }

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
    let extruded = developed(Apex::direction([qi(0), qi(0), qi(1)]).expect("a real direction"));
    let metric = develop_or_panic(metric_panel(q(1, 25)), "the metric control");

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
        panel_with(
            Apex::direction([qi(0), qi(0), qi(1)]).expect("a real direction"),
            square(qi(0), q(11, 5), h.clone()),
        ),
        "the square prism",
    );
    // One face, one interior hole — the same topology the disc gives.
    assert_eq!(square_hole.region().faces.len(), 1, "one face");
    assert_eq!(
        square_hole.region().faces[0].holes.len(),
        1,
        "one interior hole"
    );

    let inscribed = develop_or_panic(metric_panel(h.mul(&h)), "the inscribed cylinder");
    let circumscribed = develop_or_panic(
        metric_panel(h.mul(&h).mul(&qi(2))),
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
    let mut profile = disc(qi(0), q(11, 5), q(1, 5));
    profile.extend(disc(qi(0), q(11, 5), q(1, 10)));
    let part = panel_with(
        Apex::direction([qi(0), qi(0), qi(1)]).expect("a real direction"),
        profile,
    );
    match part.develop() {
        Verdict::Refuted(author::part::PartFault::ProfileNotSimple { .. }) => {}
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
/// This is the capability the milestone exists for, end to end: an L-slot authored as a
/// `Cutter::extrude`, resolved, traced, developed and cut into the exact flat boolean. Two things
/// are checked beyond the verdict, because a verdict cannot see either:
///
/// *The topology is what an L should give* — one face, one hole. A footprint the tracer split into
/// two loops, or one it silently closed around the notch, changes this count.
///
/// *The hole is still non-convex where it matters.* A developed L must turn **both ways**: the
/// reflex corner is the whole point, and a hole quietly convexified to its bounding band passes
/// every ε gate ever written. Measured on the emitted polygon, through the quarantined exact→`f64`
/// bridge rather than a hand-rolled conversion.
#[test]
fn an_l_slot_develops_and_keeps_its_reflex_corner() {
    let flat = develop_or_panic(
        panel_with(
            Apex::direction([qi(0), qi(0), qi(1)]).expect("a real direction"),
            ell(q(-1, 10), q(11, 5), q(1, 4), q(1, 8)),
        ),
        "the L-slot",
    );
    assert_eq!(flat.region().faces.len(), 1, "one face");
    assert_eq!(flat.region().faces[0].holes.len(), 1, "one interior hole");

    let polys = export::svg::region_to_polys(flat.region());
    let face = polys.faces.first().expect("one face");
    let ring = face.rings.get(1).expect("the outer ring, then the hole");
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
}

/// **AUTH.2 end to end.** A non-convex *footprint* — traced, not authored — becomes a tunnel through
/// the device. Both of the solid path's restrictions are gone:
///
/// 1. `hole_rail` consumed an interior hole as a near/far **band**, which a loop the ruling meets
///    twice is not (2e/1): such a loop now goes to the builder's general channel as the `(σ, µ̂)`
///    polygon it already is.
/// 2. That channel took an arbitrary loop only **within one σ-slice** (2e/2): it now clips the loop
///    per slice with the exact boolean, so a loop crossing stations is ordinary.
///
/// The verdict alone would pass on a cut that missed, so the check is topological: the slot adds
/// **exactly one** handle to the same panel built without it — it goes all the way through, once.
#[test]
fn a_traced_non_convex_loop_builds_a_certified_solid() {
    let part = panel_with(
        Apex::direction([qi(0), qi(0), qi(1)]).expect("a real direction"),
        ell(q(-1, 10), q(11, 5), q(1, 4), q(1, 8)),
    );
    let name = |v: &Verdict<_, author::part::PartFault, Q>| match v {
        Verdict::Verified(_) => "Verified".to_string(),
        Verdict::Refuted(f) => format!("Refuted({f:?})"),
        Verdict::Unresolved(e) => format!("Unresolved({})", rat_to_f64(e)),
    };
    let cut = match part.solid() {
        Verdict::Verified(s) => s,
        other => panic!(
            "a traced non-convex loop builds a solid, got {}",
            name(&other)
        ),
    };
    let brep = cut.brep();
    assert_eq!(brep.free_edges(), 0, "the slotted solid is watertight");
    assert_eq!(brep.nonmanifold_edges(), 0, "…and manifold");

    let plain = match panel_maybe(None).solid() {
        Verdict::Verified(s) => s,
        other => panic!("the un-slotted panel builds, got {}", name(&other)),
    };
    assert_eq!(
        genus(brep),
        genus(plain.brep()) + 1,
        "the L-slot is one tunnel through the panel — not a dent, and not two"
    );
}
