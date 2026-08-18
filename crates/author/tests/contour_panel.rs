//! **AUTH.3d — acceptance: a panel whose boundary is an authored contour**
//! (`docs/cutter-extrude-design.md` §12).
//!
//! Every other acceptance device is a **band**: `region_sigma` says where the material starts and
//! stops and the cutters only trim µ̂. `acceptance::contour_panel` is the first whose σ-extent is
//! *derived* — it keeps what is inside a radiused rectangle, and where the material ends is a
//! consequence of the contour's own corners. That is what a flex circuit's boundary actually is,
//! and it is the one thing `intersect` could not express before this milestone.
//!
//! Three claims, and the third is as much the milestone as the first two:
//!
//! 1. **The round-trip closes on it.** 3-D contour → certified flat pattern; a feature authored in
//!    flat (ECAD) coordinates → folded back onto the surface → drilled through the certified solid.
//!    Both product directions, one part.
//! 2. **It is faithful, not merely `Verified`.** The developed boundary folded back lands on the
//!    authored rounded rectangle, with *both* wall kinds carrying it — a contour that lost its radii
//!    or was convexified to a disc fails there while passing every count.
//! 3. **§12.5's exclusions refuse by name on the device where they arise.** The σ-stock is
//!    well-posed only where no azimuth *and its antipode* are both swept, because a ruling is a line
//!    through the apex and a swept profile is a prism. The self-lapping cone's 410.7° does not
//!    qualify, and says so.

use acceptance::{contour_outline_geometry, contour_panel};
use author::part::{Cutter, OpRole};
use certify_core::Verdict;
use develop::extrude::Apex;
use lattice::{Bignum, Rat};

type Q = Rat<Bignum>;

fn q(n: i128, d: i128) -> Q {
    Q::new(n, d)
}
fn qi(n: i128) -> Q {
    Q::from_i128(n)
}
fn f(r: &Q) -> f64 {
    let (n, d) = r.numer_denom_decimal();
    n.parse::<f64>().unwrap() / d.parse::<f64>().unwrap()
}

fn name<E, W: core::fmt::Debug, M: core::fmt::Debug>(v: &Verdict<E, W, M>) -> String {
    match v {
        Verdict::Verified(_) => "Verified".to_string(),
        Verdict::Unresolved(m) => format!("Unresolved({m:?})"),
        Verdict::Refuted(fa) => format!("Refuted({fa:?})"),
    }
}

/// Distance from `(x, y)` to the boundary of the rounded rectangle — zero exactly on that outline
/// and on no other shape, which is what makes it a faithfulness test rather than a bounding check.
fn rounded_box_distance(x: f64, y: f64, c: (f64, f64), half: (f64, f64), r: f64) -> f64 {
    let (ax, ay) = ((x - c.0).abs(), (y - c.1).abs());
    let (dx, dy) = ((ax - (half.0 - r)).max(0.0), (ay - (half.1 - r)).max(0.0));
    (dx.hypot(dy) - r).abs()
}

/// **The acceptance part: developed, folded back, and built — the whole σ-stock in one device.**
///
/// `segments = 48` is the measured floor for this outline. The tracer spends its budget over the
/// *whole* loop, so the corner radius sets it: at `r = w/5` nothing certifies below 384, at
/// `r = 2w/5` (this outline) 48 suffices and ε converges `5.4e-2 → 4.0e-2 → 1.7e-2` over
/// `48 → 96 → 192`. The test takes the floor; the demo driver runs it finer.
#[test]
fn a_panel_bounded_by_its_own_authored_outline_round_trips() {
    let part = contour_panel(48, None);
    let flat = match part.develop() {
        Verdict::Verified(fl) => fl,
        v => panic!("the contour panel must develop, got {}", name(&v)),
    };
    assert_eq!(flat.region().faces.len(), 1, "one face");
    assert_eq!(flat.region().faces[0].holes.len(), 0, "no holes yet");

    // **The contour bounds it ALONE, and that is derived rather than arranged.** The panel's own
    // `z ≤ 3` bound and annulus carve are still in the recipe; both must come back `Inactive`.
    let roles: Vec<OpRole> = flat.report().ops.iter().map(|o| o.role).collect();
    assert!(
        roles[0] == OpRole::Inactive && roles[1] == OpRole::Inactive,
        "the outline must bound the part alone — a recipe pruned to force that would prove \
         nothing. roles {roles:?}"
    );
    assert!(
        roles[2] != OpRole::Inactive,
        "…and the outline must actually bound it. roles {roles:?}"
    );

    // — Faithfulness: direction ②, onto the authored rounded rectangle. —
    let (cx, cy, w, h, r) = contour_outline_geometry();
    let verts: Vec<[Q; 2]> = flat
        .outline()
        .vertices
        .iter()
        .map(|b| {
            let (x, y) = b.center();
            [x, y]
        })
        .collect();
    let wire = match part.fold(&verts, &qi(0)) {
        Verdict::Verified(wi) => wi,
        v => panic!("the emitted boundary must fold back, got {}", name(&v)),
    };
    let (c, half, rf) = ((f(&cx), f(&cy)), (f(&w), f(&h)), f(&r));
    let (mut worst, mut on_arc, mut on_side) = (0.0f64, 0usize, 0usize);
    for p in &wire.points {
        let (x, y) = (f(&p[0].mid()), f(&p[1].mid()));
        worst = worst.max(rounded_box_distance(x, y, c, half, rf));
        let (ax, ay) = ((x - c.0).abs(), (y - c.1).abs());
        if ax > half.0 - rf + 1e-9 && ay > half.1 - rf + 1e-9 {
            on_arc += 1;
        } else {
            on_side += 1;
        }
    }
    assert!(
        worst < 5e-2,
        "the developed boundary must BE the authored outline: worst vertex {worst:.3e} off it"
    );
    assert!(
        on_arc > 0 && on_side > 0,
        "both wall kinds must carry boundary — {on_arc} on a corner radius, {on_side} on a \
         straight side, of {}",
        wire.points.len()
    );

    // — The other direction: a feature authored in FLAT coordinates, folded back and drilled. —
    //
    // Its coordinates come from the flat pattern just certified, which is the point: an ECAD author
    // draws on the developed panel, not on the cone. A small square about the flat centroid is
    // inside the outline by construction, since the outline is convex and the centroid is interior.
    let n = verts.len() as f64;
    let (gx, gy) = verts.iter().fold((0.0, 0.0), |(sx, sy), v| {
        (sx + f(&v[0]) / n, sy + f(&v[1]) / n)
    });
    let snap = |v: f64| export::approx::f64_to_rat::<Bignum>(v, 20);
    let e = 0.02;
    let feature = vec![
        [snap(gx - e), snap(gy - e)],
        [snap(gx + e), snap(gy - e)],
        [snap(gx + e), snap(gy + e)],
        [snap(gx - e), snap(gy + e)],
    ];
    let drilled = contour_panel(48, Some(feature));
    let flat2 = match drilled.develop() {
        Verdict::Verified(fl) => fl,
        v => panic!("the flat-authored feature must certify, got {}", name(&v)),
    };
    assert_eq!(
        flat2.region().faces[0].holes.len(),
        1,
        "the authored feature is one hole in the flat pattern"
    );

    // — And the solid, with that feature folded back through it. —
    let solid = match drilled.solid() {
        Verdict::Verified(s) => s,
        v => panic!("the contour panel's solid must build, got {}", name(&v)),
    };
    let brep = solid.brep();
    let (v_, e_, f_) = (
        brep.verts().len() as i64,
        brep.edges().len() as i64,
        brep.faces().len() as i64,
    );
    let l: i64 = brep
        .faces()
        .iter()
        .map(|fc| 1 + fc.holes.len() as i64)
        .sum();
    assert_eq!(brep.free_edges(), 0, "watertight: {f_} faces");
    assert_eq!(
        (2 - (v_ - e_ + (2 * f_ - l))) / 2,
        1,
        "an outline-bounded slab with one through-feature is genus 1: {f_} faces"
    );
    println!(
        "[contour-panel] flat {} pts  eps {:.3e}  solid {f_} faces  genus 1",
        flat.outline().vertices.len(),
        f(flat.eps())
    );
}

/// **§12.5's exclusions, on the device where they arise (AUTH.3d.3).**
///
/// The σ-stock is well-posed only where a ruling meets the contour once. A ruling is a **line**
/// through the apex and a swept profile is a **prism**, so a contour is met on the far nappe too
/// whenever the chart sweeps an azimuth *and* its antipode. The self-lapping cone sweeps 410.7°, so
/// it never qualifies — and the refusal is by name, not a crash or a silently smaller part.
///
/// The non-biting control is what stops this being a test that anything refuses: the same device
/// with a contour that contains it whole must come back **`Inactive` and unmoved**.
#[test]
fn the_wrapping_device_refuses_a_kept_contour_by_name() {
    use author::part::PartFault;
    let device = || acceptance::self_lapping_cone_with(16, 8, false, None);
    let drilled = |profile| {
        Cutter::extrude(
            acceptance::sketch_plane(),
            Apex::direction([qi(0), qi(0), qi(1)]).expect("a real direction"),
            profile,
        )
    };
    let square = |cx: Q, cy: Q, hw: Q, hh: Q| {
        arrange2d::profile::Profile::new()
            .rect(cx, cy, hw, hh)
            .into_edges()
    };

    // Both squares are stated as multiples of the device's **own** outer radius, not as absolutes:
    // they only mean anything relative to the panel they contain or bite, and an absolute pair
    // silently stops doing either when the device is re-proportioned (it did, at Ø 8 → Ø 43).
    let r_out = acceptance::self_lapping_spec().outer_r;
    let (hw, cy, hh) = (
        r_out.mul(&q(8, 5)),
        r_out.mul(&q(1, 2)),
        r_out.mul(&q(3, 10)),
    );

    // The control: a contour containing the whole panel restricts nothing.
    let plain = match device().develop() {
        Verdict::Verified(fl) => fl,
        v => panic!("the device itself must certify, got {}", name(&v)),
    };
    let wrapped = match device()
        .intersect(drilled(square(qi(0), qi(0), hw.clone(), hw.clone())))
        .develop()
    {
        Verdict::Verified(fl) => fl,
        v => panic!("a non-biting contour must certify, got {}", name(&v)),
    };
    let roles: Vec<OpRole> = wrapped.report().ops.iter().map(|o| o.role).collect();
    assert_eq!(roles[2], OpRole::Inactive, "roles {roles:?}");
    let (a, b) = (&plain.outline().vertices, &wrapped.outline().vertices);
    assert_eq!(a.len(), b.len(), "and the outline must not move");

    // And a contour that *bites*: refused, by name. Which of the two names it gets depends on how
    // the far-nappe material reaches the resolver — as a second σ-run, or as one op that both holes
    // and bounds — and both are this exclusion.
    let bitten = device()
        .intersect(drilled(square(qi(0), cy, hw, hh)))
        .develop();
    match bitten {
        Verdict::Refuted(PartFault::DisconnectedRegion)
        | Verdict::Refuted(PartFault::SectionNotSimple { .. }) => {}
        v => panic!(
            "a kept contour on a 410.7° sweep must refuse by name (§12.5), got {}",
            name(&v)
        ),
    }
}
