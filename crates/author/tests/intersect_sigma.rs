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
//! - a **quadric** contour is a certified part too, and its σ-ends are tangent rulings no graph rail
//!   can reach — so the boundary carries a **p-curve turn arc** there. Where the contour bounds the
//!   part alone the whole boundary is its traced loop; where it shares the boundary with other ops,
//!   the arc is spliced into the graph chain at the junctions the corner refinement locates — one
//!   arc per end where it takes over near each, and a single arc wrapping **both** tangents where it
//!   bounds one whole side;
//! - the **solid** follows the flat pattern in two of those three shapes (AUTH.3c): where the
//!   boundary is still a rail band the region bands clip to the derived extent and the Bézier
//!   weights normalize to their positive representative; where it is one traced loop, that loop
//!   goes to the builder as a general `(σ,µ̂)` **outer wire**, kept *inside* rather than subtracted.
//!   The mixed whole-side boundary — a rail out and an arc back — still refuses;
//! - the shipped lateral trim is unmoved, vertex for vertex.

use author::construct;
use author::part::{Cutter, OpRole, Part, SupportFn};
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
/// Declare a band inside that and the part certifies. Declare one **wider** — so the contour now
/// terminates the material rather than merely trimming it — and it certifies too. That is the whole
/// of AUTH.3 in one fixture, and the route it took is the milestone: `EmptyRegion` (σ was authored,
/// so a σ the ops left empty was a contradiction) → `RailSpanShort` (the extent became derived, and
/// a graph rail cannot reach a tangent ruling) → certified, once the boundary could carry a p-curve.
///
/// This is the **whole-side** shape, the hardest of the three: the contour bounds the entire lower
/// side, so its chain is a *single* segment and the two tangents are joined by one continuous run of
/// contour boundary. The boundary is therefore the panel's `z ≤ 3` rail out and **one** turn arc
/// wrapping *both* tangents all the way back — not one arc per end.
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

    // Past the footprint's end: the contour terminates the material, and that is a part now. Here
    // it bounds one **whole** side, so the boundary is the panel's `z ≤ 3` rail out and a single
    // turn arc wrapping **both** tangents all the way back — one arc, not one per end.
    let wide = panel(q(-1, 8), q(1, 8)).intersect(cutter());
    let flat = match wide.develop() {
        Verdict::Verified(f) => f,
        v => panic!(
            "a band wider than the contour's own σ-footprint must certify, got {}",
            name(&v)
        ),
    };
    assert_eq!(flat.region().faces.len(), 1, "one face");
    assert_eq!(flat.region().faces[0].holes.len(), 0, "no holes");
    let roles: Vec<OpRole> = flat.report().ops.iter().map(|o| o.role).collect();
    assert!(
        roles[0] != OpRole::Inactive && roles[2] != OpRole::Inactive,
        "the plane and the contour must BOTH bound — else this is not the shared-boundary shape \
         at all. roles {roles:?}"
    );

    // **Cost, not only correctness.** `segments` is the tracer's *piece* count, and a traced piece
    // is one chord — so a boundary made of a rail plus an arc over that loop is O(segments), never
    // O(segments²). It was the square once (#281): each already-traced piece was re-sampled
    // `segments` times, giving 6386 points here against 192 for the same contour bounding alone,
    // and 175s to develop against 3s. Every faithfulness assertion below passed throughout, which
    // is exactly why the budget is its own check — a correct boundary can still be an unusable one.
    let n_out = flat.outline().vertices.len();
    assert!(
        n_out < 8 * 48,
        "the outline must stay proportional to the tracer's resolution, not its square: {n_out} \
         points at segments = 48"
    );

    // Faithfulness: every boundary vertex folded back lies on the authored cylinder OR on the
    // authored plane — the two surfaces that bound it — and the arc really wraps, so the folded
    // boundary spans the contour's full diameter rather than stopping at a tangent.
    let verts: Vec<[Q; 2]> = flat
        .outline()
        .vertices
        .iter()
        .map(|b| {
            let (x, y) = b.center();
            [x, y]
        })
        .collect();
    let wire = match wide.fold(&verts, &qi(0)) {
        Verdict::Verified(w) => w,
        v => panic!("the emitted boundary must fold back, got {}", name(&v)),
    };
    let (cyf, rf) = (2.5, 0.5);
    let (mut worst, mut lo_x, mut hi_x) = (0.0f64, f64::MAX, f64::MIN);
    for p in &wire.points {
        let (x, y, z) = (
            to_f64(&p[0].mid()),
            to_f64(&p[1].mid()),
            to_f64(&p[2].mid()),
        );
        let on_cyl = ((x * x + (y - cyf).powi(2)).sqrt() - rf).abs();
        let on_plane = (z - 3.0).abs();
        worst = worst.max(on_cyl.min(on_plane));
        lo_x = lo_x.min(x);
        hi_x = hi_x.max(x);
    }
    assert!(
        worst < 5e-3,
        "every boundary vertex must lie on the cylinder or the plane: worst is {worst:.3e} off both"
    );
    assert!(
        (hi_x - lo_x) > 1.9 * rf,
        "the arc must wrap the contour, not stop at a tangent — folded x-span {:.4} against a \
         diameter {:.4}",
        hi_x - lo_x,
        2.0 * rf
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

    // A quadric contour at the same place, sized so it pokes through the panel's annulus carve —
    // so the carve still bounds part of the boundary and the contour takes over only near its ends.
    // That is the mixed shape: graph rails with a p-curve turn arc spliced in at each tangent
    // ruling. A polygon needs none of it (every wall affine, `plane_cut_rail` exact, ε = 0 above).
    let metric = base().intersect(Cutter::vertical_cylinder(qi(0), q(11, 5), qi(1)));
    let flat = match metric.develop() {
        Verdict::Verified(f) => f,
        v => panic!(
            "a quadric contour must certify through its tangent ends, got {}",
            name(&v)
        ),
    };
    assert_eq!(flat.region().faces.len(), 1, "one face");
    let roles: Vec<OpRole> = flat.report().ops.iter().map(|o| o.role).collect();
    assert!(
        roles[0] != OpRole::Inactive && roles[2] != OpRole::Inactive,
        "this is the MIXED boundary: the panel's plane and the contour must BOTH bound, else the \
         fixture is the sole-contour case and proves nothing about the splice. roles {roles:?}"
    );
}

/// **The solid over a derived σ-extent (AUTH.3c).**
///
/// AUTH.3b taught the *flat pattern* to close in σ; the solid builder still swept the authored
/// region **band**, so a part whose extent is narrower than its band asked the builder to place
/// geometry over σ the boundary chains do not cover — and it refused, correctly, by failing to find
/// a rail piece there. Clipping each band to `structure.domain` is the whole of the mechanical part.
///
/// It was not the whole of the fix, and the second half is the interesting one: with the extent
/// right, `sigma_splits` still refused this part, because the anchor's denominator came out
/// **uniformly negative** over it. `(N, D)` and `(−N, −D)` are the same curve — which one arrives is
/// a convention of the cutter's wall orientation, flipped again by `reduce()` — so demanding the
/// positive sign refused a part every emitted patch is perfectly well-conditioned over. The gate is
/// now sign-*definiteness* (a weight passing through zero is a genuine pole, and that is what
/// subdividing is for) and the Bernstein constructors pick the positive representative.
///
/// A watertight genus-0 shell is the claim, not merely `Verified`: a solid built over the wrong
/// σ-range would still report a bound.
#[test]
fn the_solid_closes_at_the_derived_extent_too() {
    let kept = panel(qi(-1), qi(1)).intersect(drilled(square(qi(0), q(11, 5), q(1, 4))));
    let solid = match kept.solid() {
        Verdict::Verified(s) => s,
        v => panic!(
            "a contour that terminates the material in σ must build a solid, got {}",
            name(&v)
        ),
    };
    let brep = solid.brep();
    let (v, e, f) = (
        brep.verts().len() as i64,
        brep.edges().len() as i64,
        brep.faces().len() as i64,
    );
    let l: i64 = brep
        .faces()
        .iter()
        .map(|fc| 1 + fc.holes.len() as i64)
        .sum();
    let genus = (2 - (v - e + (2 * f - l))) / 2;
    assert_eq!(brep.free_edges(), 0, "watertight: {f} faces");
    assert_eq!(
        genus, 0,
        "a solid square contour is a plain slab: {f} faces"
    );

    // The control that keeps the above from being vacuous *and* pins the no-regression half: the
    // shipped lateral trim, whose extent IS its band, must build exactly as before — the sign
    // normalization above touches every rational patch in the crate, so "the σ-stock case now
    // works" is only half the claim.
    let lateral =
        panel(qi(-1), qi(1)).intersect(Cutter::half_space([qi(0), qi(0), qi(1)], q(5, 2)));
    let plain = match lateral.solid() {
        Verdict::Verified(s) => s,
        v => panic!(
            "the shipped lateral trim must still build, got {}",
            name(&v)
        ),
    };
    assert_eq!(plain.brep().free_edges(), 0, "watertight");
    assert!(
        plain.brep().faces().len() < brep.faces().len(),
        "and the contour's solid must be the richer one — a σ-terminating boundary carries more \
         faces than a band-wide trim, so equal counts would mean one of them is not what it says"
    );
}

/// **The solid of a part bounded by nothing but a traced contour (AUTH.3c, the outer wire).**
///
/// This is the shape a rail band cannot carry. Not because the topology is exotic — the footprint in
/// `(σ, µ̂)` is a closed oval, and an oval swept through the thickness is an ordinary prism — but
/// because at the two σ-ends the wall's branches meet with **unbounded slope**, and a fitted
/// polynomial rail is clamped away from exactly there. `certify_boundary` says so by name
/// (`RailSpanShort`), which is why the solid evaluator forks *before* it and hands the loop to the
/// builder as a general `(σ,µ̂)` outer wire instead — the same currency the polygon-hole channel
/// already takes, intersected rather than subtracted.
///
/// **Faithfulness, not just a verdict**, per the flat path's own standard: the solid is folded back
/// onto the authored cylinder `x² + (y − 11/5)² = 1/25`. Every vertex is a boundary vertex here (no
/// holes), so each sits on that cylinder at `w = 0` or a thickness off it along the surface normal —
/// so the whole shell must lie within a thickness of the contour, and the `w = 0` half must lie
/// *on* it. A solid built over the wrong loop would pass a face count and fail this.
#[test]
fn a_traced_contour_bounds_a_solid_where_no_rail_band_can() {
    let (cy, r2) = (q(11, 5), q(1, 25));
    let kept = panel(qi(-1), qi(1)).intersect(Cutter::vertical_cylinder(qi(0), cy.clone(), r2));
    let solid = match kept.solid() {
        Verdict::Verified(s) => s,
        v => panic!(
            "a contour that bounds the part alone must build a solid, got {}",
            name(&v)
        ),
    };
    let brep = solid.brep();
    let (v, e, f) = (
        brep.verts().len() as i64,
        brep.edges().len() as i64,
        brep.faces().len() as i64,
    );
    let l: i64 = brep
        .faces()
        .iter()
        .map(|fc| 1 + fc.holes.len() as i64)
        .sum();
    assert_eq!(brep.free_edges(), 0, "watertight: {f} faces");
    assert_eq!(
        (2 - (v - e + (2 * f - l))) / 2,
        0,
        "a disc swept through the thickness is a plain slab: {f} faces"
    );

    // The shell hugs the authored cylinder: within a thickness everywhere, ON it for the `w = 0`
    // lid. `1/8` is the part's thickness; `5e-3` is the same tolerance the flat leg asserts at.
    let (cyf, rf, th) = (2.2, 0.2, 0.125);
    // A brep vertex is a `Surd` `a + b√d`; every vertex of a rational patch's trim is rational.
    let surd_f64 = |s: &lattice::Surd<Bignum>| {
        let (a, b, d) = s.parts();
        to_f64(a) + to_f64(b) * to_f64(d).sqrt()
    };
    let (mut worst, mut on_it) = (0.0f64, 0usize);
    for p in brep.verts() {
        let (x, y) = (surd_f64(&p[0]), surd_f64(&p[1]));
        let d = ((x * x + (y - cyf).powi(2)).sqrt() - rf).abs();
        worst = worst.max(d);
        if d < 5e-3 {
            on_it += 1;
        }
    }
    assert!(
        worst < th + 5e-3,
        "no vertex may sit more than a thickness off the authored contour: worst {worst:.3e}"
    );
    assert!(
        on_it * 4 >= brep.verts().len(),
        "the w = 0 lid must lie ON the contour, and it is half the vertices: only {on_it} of {} \
         are within 5e-3",
        brep.verts().len()
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

/// **A quadric contour is a part, and its boundary is a fully parametric curve.**
///
/// "Keep only what is inside this circle." The material is the disc's footprint on the cone, and
/// both derived σ-ends are the cylinder's **tangent rulings** — where its two µ̂-branches meet with
/// unbounded slope. No chain of graph rails `µ̂ = f(σ)` can reach such an end: `certified_rail_surface`
/// clamps its fit away from the tangent for exactly that reason, which is what `RailSpanShort`
/// refuses. The boundary here is the wall's own traced loop, parametric in its own parameter and
/// passing *through* both tangents — PC.3's construction used as an outline rather than as a hole.
///
/// **Faithfulness**: the emitted boundary is folded back and every vertex must land on the authored
/// cylinder, `x² + (y − 11/5)² = 1/25`. The develop leg never sees the cutter's metric form and the
/// fold leg never sees the resolver, so agreeing on it is not two halves of one mistake.
#[test]
fn a_quadric_contour_is_a_part_with_a_parametric_boundary() {
    let (cy, r2) = (q(11, 5), q(1, 25));
    let kept =
        panel(qi(-1), qi(1)).intersect(Cutter::vertical_cylinder(qi(0), cy.clone(), r2.clone()));
    let flat = match kept.develop() {
        Verdict::Verified(f) => f,
        v => panic!(
            "keeping what is inside the circle must certify, got {}",
            name(&v)
        ),
    };
    assert_eq!(flat.region().faces.len(), 1, "one face");
    assert_eq!(flat.region().faces[0].holes.len(), 0, "and no holes");
    let roles: Vec<OpRole> = flat.report().ops.iter().map(|o| o.role).collect();
    assert!(
        roles[0] == OpRole::Inactive && roles[1] == OpRole::Inactive,
        "the contour must bound the part ALONE — roles {roles:?}"
    );

    // Direction ②: every boundary vertex, folded back, on the authored cylinder.
    let verts: Vec<[Q; 2]> = flat
        .outline()
        .vertices
        .iter()
        .map(|b| {
            let (x, y) = b.center();
            [x, y]
        })
        .collect();
    assert!(
        verts.len() > 32,
        "a curved boundary should carry many vertices, got {}",
        verts.len()
    );
    let wire = match kept.fold(&verts, &qi(0)) {
        Verdict::Verified(w) => w,
        v => panic!("the emitted boundary must fold back, got {}", name(&v)),
    };
    let (cyf, rf) = (to_f64(&cy), to_f64(&r2).sqrt());
    let mut worst = 0.0f64;
    let (mut lo_s, mut hi_s) = (f64::MAX, f64::MIN);
    for p in &wire.points {
        let (x, y) = (to_f64(&p[0].mid()), to_f64(&p[1].mid()));
        worst = worst.max((((x * x + (y - cyf).powi(2)).sqrt()) - rf).abs());
        lo_s = lo_s.min(x);
        hi_s = hi_s.max(x);
    }
    assert!(
        worst < 5e-3,
        "the developed boundary must BE the authored circle: worst vertex sits {worst:.3e} off it"
    );
    // And it wraps the whole contour rather than stopping at one side of it: the folded boundary
    // spans the cylinder's full diameter in x, which a boundary truncated at a tangent could not.
    assert!(
        (hi_s - lo_s) > 1.9 * rf,
        "the boundary must wrap the contour — folded x-span {:.4} against a diameter {:.4}",
        hi_s - lo_s,
        2.0 * rf
    );
}
