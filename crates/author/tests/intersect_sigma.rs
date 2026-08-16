//! **AUTH.3 — an intersect that terminates the material in σ, not only µ̂**
//! (`docs/cutter-extrude-design.md` §12).
//!
//! `OpKind::Intersect` always shipped, but in one shape only: a lateral trim, bounding µ̂ on every
//! ruling and never ending the material along σ. "Keep what is inside this contour" refused, and the
//! gap was in the region model — the σ-extent was *defined* to be the authored `region_sigma` band,
//! so a σ the ops left empty was a contradiction rather than a point outside the part.
//!
//! Where the milestone stands, as these tests pin it:
//!
//! - the extent is **derived** (AUTH.3a) and the boundary **closes at it** (AUTH.3b);
//! - a **polygonal** contour is a certified part — its walls are affine, so `plane_cut_rail` is
//!   exact and the rails reach the corner with no fit and no clamp;
//! - a **quadric** contour still refuses, by name: it ends at a *tangent ruling*, where the rail is
//!   a fitted graph of unbounded slope, and the fit's certified span cannot reach the end. That is
//!   §12.4's p-curve, the remaining half of AUTH.3b;
//! - the shipped lateral trim is unmoved, vertex for vertex.

use author::construct;
use author::part::{Cutter, OpRole, Part, PartFault, SupportFn};
use certify_core::Verdict;
use develop::extrude::{Apex, Frame};
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
fn to_f64(r: &Q) -> f64 {
    let (n, d) = r.numer_denom_decimal();
    n.parse::<f64>().unwrap() / d.parse::<f64>().unwrap()
}

/// The doctest cone panel over a chosen σ-band: a `z ≤ 3` bound above, an annulus carve below, and
/// a witness on the kept sheet. Everything below adds exactly one op to this.
fn panel(lo: Q, hi: Q) -> Part<Bignum> {
    construct::from_chart::<Bignum>(&cone())
        .region_sigma(lo, hi, SupportFn::inherit())
        .keep_near(
            cone()
                .surface(&qi(2), &qi(0))
                .eval(&qi(0))
                .expect("the cone is regular at σ = 0"),
        )
        .intersect(Cutter::half_space([qi(0), qi(0), qi(1)], qi(3)))
        .subtract(Cutter::vertical_cylinder(qi(0), q(1, 2), qi(2)))
}

/// A cutter extruding `profile` straight down `z` from the `z = 0` sketch plane.
fn drilled(profile: Vec<Edge<Bignum>>) -> Cutter<Bignum> {
    let frame = Frame::new(
        [qi(0), qi(0), qi(0)],
        [qi(1), qi(0), qi(0)],
        [qi(0), qi(1), qi(0)],
    )
    .expect("an orthonormal frame");
    Cutter::extrude(
        frame,
        Apex::direction([qi(0), qi(0), qi(1)]).expect("a real direction"),
        profile,
    )
}

/// The axis-aligned square of half-side `h` about `(cx, cy)`.
fn square(cx: Q, cy: Q, h: Q) -> Vec<Edge<Bignum>> {
    arrange2d::profile::Profile::new()
        .rect(cx, cy, h.clone(), h)
        .into_edges()
}

/// A verdict's name, for panic messages (the payloads are not `Debug`).
fn name<E, W: core::fmt::Debug, M: core::fmt::Debug>(v: &Verdict<E, W, M>) -> String {
    match v {
        Verdict::Verified(_) => "Verified".to_string(),
        Verdict::Unresolved(m) => format!("Unresolved({m:?})"),
        Verdict::Refuted(f) => format!("Refuted({f:?})"),
    }
}

/// **The band decides, and nothing else does — this is the whole gap in one fixture.**
///
/// One part, one cutter, one placement: a cylinder of radius ½ about `(0, 5/2)`, intersected in. It
/// bites for real — it takes over as the part's **lower** bound and pushes the annulus carve out of
/// the structure entirely — so this is not the "a cutter containing the whole panel verifies
/// trivially" reading. Its footprint on the cone subtends `|σ| ≤ 0.101020514` — its two tangent
/// rulings, measured by the same `structure_events` the derivation uses.
///
/// Declare a band inside that and the part certifies. Declare one wider and the *same* cut on the
/// *same* material still refuses — but the refusal has moved, and where it moved to is the
/// measurement. It was `EmptyRegion`, a region-model gap: σ was an authored quantity and a σ with no
/// material was a contradiction. AUTH.3a made the extent derived and AUTH.3b closed the boundary at
/// it, and what is left is one thing — a **quadric** contour ends at a *tangent ruling*, where its
/// rail is a fitted graph with unbounded slope, so the fit's certified span stops short of the
/// derived end and the rail is refused rather than extrapolated (`RailSpanShort`). A polygonal
/// contour has no such end and now certifies (see the next test): every wall is affine, so
/// `plane_cut_rail` is exact and reaches the corner.
///
/// Expected to flip when the pinch end becomes a p-curve — §12.4's remaining half of AUTH.3b.
#[test]
fn only_the_declared_band_separates_a_working_intersect_from_a_refused_one() {
    let cutter = || Cutter::vertical_cylinder(qi(0), q(5, 2), q(1, 4));

    // Inside the footprint: certified, and the cut is load-bearing.
    let narrow = panel(q(-1, 16), q(1, 16)).intersect(cutter());
    let flat = match narrow.develop() {
        Verdict::Verified(f) => f,
        v => panic!("a band inside the footprint must certify, got {}", name(&v)),
    };
    let roles: Vec<OpRole> = flat.report().ops.iter().map(|o| o.role).collect();
    assert_eq!(
        roles[2],
        OpRole::LowerBound,
        "the intersect must actually bound the material — else this fixture proves nothing about \
         biting cuts. roles {roles:?}"
    );
    assert_eq!(
        roles[1],
        OpRole::Inactive,
        "and it must bound it *instead of* the annulus carve. roles {roles:?}"
    );

    // Past the footprint's end: still refused, but now by the tangent-ruling fit and nothing else.
    let wide = panel(q(-1, 8), q(1, 8)).intersect(cutter());
    assert!(
        matches!(
            wide.develop(),
            Verdict::Refuted(PartFault::RailSpanShort { op: 2 })
        ),
        "the σ-extent is derived now, so the refusal must name the pinch rather than the region — \
         got {}",
        name(&wide.develop())
    );
}

/// **The same footprint, both senses — and the kept one is now a part.**
///
/// A square prism through the panel. *Subtracted*, it is a certified interior hole. *Intersected*,
/// the identical cutter is the whole part: the material's σ-extent is derived from the contour's own
/// corners and the boundary closes there. Both senses of one cutter, and every other op has gone
/// `Inactive` — the panel's `z ≤ 3` bound and its annulus carve are both outside the footprint, so
/// what is left is the contour and nothing else.
///
/// **Faithfulness, not just a verdict.** A `Verified` flat pattern of the wrong shape would pass a
/// count of faces and holes, so the outline is folded **back** and checked against the authored
/// profile: every vertex of the developed boundary must land on the square's own boundary in the
/// sketch plane, `max(|x|, |y − 11/5|) = 1/4`. That is a quantity neither leg computes — the develop
/// leg never sees the sketch frame and the fold leg never sees the resolver — so agreeing on it is
/// not two halves of one mistake.
#[test]
fn the_same_footprint_is_a_certified_hole_and_a_kept_part() {
    let cutter = || drilled(square(qi(0), q(11, 5), q(1, 4)));
    let base = || panel(qi(-1), qi(1));

    let holed = base().subtract(cutter());
    let flat = match holed.develop() {
        Verdict::Verified(f) => f,
        v => panic!(
            "the subtract sense is shipped and must stay green, got {}",
            name(&v)
        ),
    };
    assert_eq!(flat.holes().len(), 1, "the prism drills exactly one hole");
    let roles: Vec<OpRole> = flat.report().ops.iter().map(|o| o.role).collect();
    assert_eq!(roles[2], OpRole::Hole, "roles {roles:?}");

    // The same cutter, kept rather than removed.
    let kept = base().intersect(cutter());
    let flat = match kept.develop() {
        Verdict::Verified(f) => f,
        v => panic!(
            "keeping what is inside the contour must certify, got {}",
            name(&v)
        ),
    };
    assert_eq!(flat.region().faces.len(), 1, "one face");
    assert_eq!(flat.region().faces[0].holes.len(), 0, "and no holes");
    let roles: Vec<OpRole> = flat.report().ops.iter().map(|o| o.role).collect();
    assert!(
        matches!(
            roles[2],
            OpRole::LowerBound | OpRole::UpperBound | OpRole::Notch
        ),
        "the contour must bound the part — roles {roles:?}"
    );
    assert!(
        roles[0] == OpRole::Inactive && roles[1] == OpRole::Inactive,
        "and it must bound it ALONE: the panel's own ops lie outside the footprint, so a part \
         still carrying them would not be the contour's. roles {roles:?}"
    );

    // Direction ②: fold the emitted boundary back and land it on the authored square.
    let verts: Vec<[Q; 2]> = flat
        .outline()
        .vertices
        .iter()
        .map(|b| {
            let (x, y) = b.center();
            [x, y]
        })
        .collect();
    let wire = match kept.fold(&verts, &qi(0)) {
        Verdict::Verified(w) => w,
        v => panic!("the emitted boundary must fold back, got {}", name(&v)),
    };
    let mut worst = 0.0f64;
    for p in &wire.points {
        let (x, y) = (to_f64(&p[0].mid()), to_f64(&p[1].mid()));
        // Distance to the square's boundary: the Chebyshev radius about its centre, minus ¼.
        let r = x.abs().max((y - 2.2).abs());
        worst = worst.max((r - 0.25).abs());
    }
    assert!(
        worst < 5e-3,
        "the developed boundary must BE the authored square: worst vertex sits {worst:.3e} off it"
    );

    // The quadric contour is the half AUTH.3b has not finished: its rails are a *fit*, and at a
    // tangent ruling the fit's certified span stops short of the derived end, so it refuses by name
    // rather than extrapolating into a √-branch. A polygon has no such end — every wall is affine,
    // so `plane_cut_rail` is exact and reaches the corner (the flat pattern above certifies at
    // ε = 0 on the contour's own rails). Expected to flip when the pinch end becomes a p-curve.
    let metric = base().intersect(Cutter::vertical_cylinder(qi(0), q(11, 5), qi(1)));
    assert!(
        matches!(
            metric.develop(),
            Verdict::Refuted(PartFault::RailSpanShort { op: 2 })
        ),
        "a quadric contour pinches at a tangent ruling, which needs a p-curve end — got {}",
        name(&metric.develop())
    );
}

/// **The leg AUTH.3 may not regress.** An intersect whose inside contains the whole panel restricts
/// nothing, and the part must come out exactly as it does without it — not "also Verified", but the
/// same outline, vertex for vertex, and the same certified bound.
///
/// Worth pinning because it is the reading that makes the feature look present: `intersect(<a big
/// enough cutter>)` has always verified, and it verifies for the reason that a cut which never
/// terminates the material never meets the gap.
#[test]
fn an_intersect_that_does_not_bite_leaves_the_part_untouched() {
    let plain = match panel(qi(-1), qi(1)).develop() {
        Verdict::Verified(f) => f,
        v => panic!("the bare panel must certify, got {}", name(&v)),
    };
    // A cylinder of radius 20 about the axis — every ruling meets it, and it clips none of them.
    let wrapped = match panel(qi(-1), qi(1))
        .intersect(Cutter::vertical_cylinder(qi(0), qi(0), qi(400)))
        .develop()
    {
        Verdict::Verified(f) => f,
        v => panic!("a non-biting intersect must certify, got {}", name(&v)),
    };
    let roles: Vec<OpRole> = wrapped.report().ops.iter().map(|o| o.role).collect();
    assert_eq!(roles[2], OpRole::Inactive, "roles {roles:?}");

    let (a, b) = (&plain.outline().vertices, &wrapped.outline().vertices);
    assert_eq!(a.len(), b.len(), "the outline must not change shape");
    for (i, (p, r)) in a.iter().zip(b.iter()).enumerate() {
        let (px, py) = p.center();
        let (rx, ry) = r.center();
        assert!(
            px.cmp(&rx) == core::cmp::Ordering::Equal && py.cmp(&ry) == core::cmp::Ordering::Equal,
            "vertex {i} moved: an inactive op must be exactly inactive"
        );
    }
    assert!(
        plain.eps().cmp(wrapped.eps()) == core::cmp::Ordering::Equal,
        "and the certified bound must not move either"
    );

    // The control that keeps the equality above from being vacuous: an intersect that *does* bite
    // — laterally, so it stays inside today's model — must move the very vertices just compared.
    let bitten = match panel(qi(-1), qi(1))
        .intersect(Cutter::half_space([qi(0), qi(0), qi(1)], q(5, 2)))
        .develop()
    {
        Verdict::Verified(f) => f,
        v => panic!(
            "a lateral trim is the shipped intersect and must certify, got {}",
            name(&v)
        ),
    };
    let c = &bitten.outline().vertices;
    let moved = a.len() != c.len()
        || a.iter().zip(c.iter()).any(|(p, r)| {
            let ((px, py), (rx, ry)) = (p.center(), r.center());
            px.cmp(&rx) != core::cmp::Ordering::Equal || py.cmp(&ry) != core::cmp::Ordering::Equal
        });
    assert!(
        moved,
        "a biting intersect must change the outline — otherwise the equality above proves nothing"
    );
}
