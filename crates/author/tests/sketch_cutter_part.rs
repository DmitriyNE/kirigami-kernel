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

/// The Stage-1 gore with its interior hole cut by `profile` swept from `apex`.
fn panel_with(apex: Apex<Bignum>, profile: Vec<Edge<Bignum>>) -> Part<Bignum> {
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
        .subtract(Cutter::extrude(sketch_plane(), apex, profile))
        .clearance(qi(1))
        .thickness(q(1, 8))
        .segments(72)
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

/// **The ring is refused by name.** A profile with a hole of its own shadows every ruling in two
/// stretches, which the band representation cannot express — so the part reports
/// [`PartFault::ProfileNotSimple`] rather than shipping a hole that is not the one drawn. Before
/// 1e.4 this failed closed only by accident, on a window search declining a shape it could not read.
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
