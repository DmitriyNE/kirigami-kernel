//! **AUTH.3 pre-state, pinned** — an intersect op may bound µ̂, but may not terminate the material
//! in σ (`docs/cutter-extrude-design.md` §12).
//!
//! `OpKind::Intersect` ships and every fixture uses it, so the gap reads like a wiring question. It
//! is a region-model one: the material's σ-extent is *required to equal* the authored
//! `region_sigma` band, so a cutter whose footprint ends inside that band leaves the region empty at
//! the samples past its end and the whole sweep refuses. These tests pin each half of that — what
//! must flip when AUTH.3 lands, and what may not move while it does.

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
/// rulings, measured by the same `structure_events` the derivation will use.
/// Declare a band inside that and the part certifies; declare one wider and the *same* cut on the
/// *same* material comes back `EmptyRegion`, because the samples past the footprint's end have no
/// material and the resolver has nowhere to put that fact.
///
/// The wide half is expected to **flip** at AUTH.3b. The narrow half must stay green: it is the
/// lateral-trim intersect that ships today.
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

    // Past the footprint's end: refused, for the σ-extent and for no other reason.
    let wide = panel(q(-1, 8), q(1, 8)).intersect(cutter());
    assert!(
        matches!(wide.develop(), Verdict::Refuted(PartFault::EmptyRegion)),
        "AUTH.3 flips this: a band wider than the cutter's own σ-footprint must stop being a \
         refusal — got {}",
        name(&wide.develop())
    );
}

/// **The same footprint, both senses: a hole today, nothing at all today.**
///
/// A square prism through the panel. Subtracted, it is a certified interior hole — traced,
/// developed, cut into the exact flat boolean — so the *geometry* of a footprint that ends inside
/// the band is shipped machinery (`shadow_cut_loops`, AUTH.2c). Intersected, the identical cutter
/// refuses: keeping what is inside a contour needs the material's σ-extent to be **derived** from
/// the ops, and today it is required to equal the authored band.
///
/// Pinning both senses together is what makes this a measurement rather than a bug report: it
/// isolates the missing piece to the region model, and it rules out the extruded-profile path (a
/// plain metric `vertical_cylinder` at the same place refuses identically — the third assertion).
#[test]
fn the_same_footprint_is_a_certified_hole_and_a_refused_intersect() {
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

    let kept = base().intersect(cutter());
    assert!(
        matches!(kept.develop(), Verdict::Refuted(PartFault::EmptyRegion)),
        "AUTH.3 flips this: the same contour, kept rather than removed — got {}",
        name(&kept.develop())
    );

    // And it is not the extruded path: the metric cutter with the same kind of footprint refuses
    // the same way, which is why AUTH.3 is filed against the resolver and not against AUTH.2.
    let metric = base().intersect(Cutter::vertical_cylinder(qi(0), q(11, 5), qi(1)));
    assert!(
        matches!(metric.develop(), Verdict::Refuted(PartFault::EmptyRegion)),
        "a quadric cutter must refuse identically — got {}",
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
