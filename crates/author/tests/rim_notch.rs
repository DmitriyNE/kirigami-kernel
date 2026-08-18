//! **The device's real drawings against the resolver's boundary model (#291, #293, #294, #295,
//! #296).**
//!
//! Both of the device's boundaries come out of a file, and both are **radially flanked**: the bore
//! (`acceptance/data/inner-cut.dxf`) is Ø 8 with a 10° tab reaching in to Ø 4, and the rim
//! (`data/outer-cut.dxf`) is Ø 21.5 with a 15° lug reaching out to Ø 27.5. One is subtracted and
//! one is intersected, and they are otherwise the same shape problem — which is what makes them
//! worth pinning side by side.
//!
//! **A radial flank is the whole story.** Cast from a point on the axis, a radial sketch line
//! sweeps a plane through the axis, and a ruling either lies *in* that plane or crosses it. Which
//! one it does is decided by `h′` and by nothing else: where the support is flat the ruling's plan
//! projection passes exactly through the axis and runs *along* the flank; on a ramp it misses by up
//! to 0.481 mm and runs *across* it.
//!
//! Both features sit in the wedge the 410.7° chart covers **twice** — material azimuth is
//! `270° + 4·arctan σ`, so `az ∈ (64.6°, 115.4°)` is swept on the base cone and again on the
//! lapping sheet:
//!
//! | | the bore's tab (subtract) | the rim's lug (intersect) |
//! |---|---|---|
//! | base pass | σ ≈ −1.079…−0.927, `h′ = 0` | σ ≈ −1.067…−0.937, `h′ = 0` |
//! | lapping pass | σ ≈ +0.888…+1.049, on the pinned ramp | σ ≈ +0.937…+1.067, straddling its end |
//!
//! On the pinned device the tab's second pass lands on the ramp and it refuses
//! [`PartFault::SectionNotSimple`] — the kept material is two µ̂-intervals at one σ.
//!
//! The tab's route is understood: a region is modelled as **one µ̂-interval per σ** — a lower rail
//! and an upper rail, both graphs over σ — plus interior holes, and a tab that bays in sideways
//! splits that interval, which the section sampler sees and names. That is #291.
//!
//! With the ramp moved off the wedge, both features land on flat sheet on both passes. The bore's
//! tab then **develops faithfully** — its four flank edges land on the drawing through the cast,
//! which is the check that distinguishes a green certificate from a right part.
//!
//! The rim's lug is **kept but not yet buildable** (#296). It used to come back `Verified` with the
//! tab cut *inward* as a bite; that was the shadow fragmenting a non-convex kept region at every
//! wall crossing, and coalescing the patches fixed it. What stops it now is a named refusal —
//! `RailSpanShort { op: 0 }`, §12.4's p-curve end at the lug's **mixed corner**: its flank is
//! tangent to the nose arc at one end and meets the rim transversally at the other, so #294's
//! `flank_splice`, which wants a tangency at both, does not fire.

use acceptance::{self_lapping_cone_from, self_lapping_spec};
use arrange2d::profile::Profile;
use author::part::{Part, PartFault};
use certify_core::Verdict;
use geom::content::Edge;
use lattice::{Bignum, Rat, Surd};

type Q = Rat<Bignum>;

fn qi(n: i128) -> Q {
    Q::from_i128(n)
}

/// The device carrying `profile` as its inner cut, at the demo's own resolution knobs.
fn device_with(profile: Vec<Edge<Bignum>>) -> Part<Bignum> {
    let mut spec = self_lapping_spec();
    spec.inner_profile = Some(profile);
    self_lapping_cone_from(&spec, 8, 8, false, None)
}

fn fault(v: Verdict<author::part::FlatPattern<Bignum>, PartFault, Q>) -> String {
    match v {
        Verdict::Verified(_) => "Verified".to_string(),
        Verdict::Unresolved(e) => {
            let (n, d) = e.numer_denom_decimal();
            let fl = n.parse::<f64>().unwrap_or(f64::NAN) / d.parse::<f64>().unwrap_or(f64::NAN);
            format!("Unresolved({fl:.4e})")
        }
        Verdict::Refuted(f) => format!("{f:?}"),
    }
}

/// The device with the ccw ramp slid **off** the features' azimuth wedge, so both the bore's tab
/// and the rim's lug meet flat sheet on each of their two passes. Everything else is the pinned
/// recipe, which is what makes the difference attributable to the ramp.
fn ramp_off_the_wedge() -> acceptance::lapped::LappedCone {
    let mut spec = self_lapping_spec();
    spec.ccw.ramp_start = acceptance::lapped::Azimuth::Sigma(Q::new(1, 10));
    spec.ccw.ramp_end = acceptance::lapped::Azimuth::Sigma(Q::new(1, 2));
    spec
}

fn f(r: &Q) -> f64 {
    let (n, d) = r.numer_denom_decimal();
    n.parse::<f64>().unwrap() / d.parse::<f64>().unwrap()
}

fn sd(v: &Surd<Bignum>) -> f64 {
    let (a, b, d) = v.parts();
    f(a) + f(b) * f(d).sqrt()
}

/// **The drafted cast, in floats, straight from the recipe** — `acceptance::lapped`'s own
/// construction at gauge radius `r`: the profile plane at `z_r = −r·cot β`, the apex `r·tan β`
/// below it, the neutral cone `z = −(c/s)·ρ`. The gauge radius is the cast's fixed point, which is
/// what pins the sign conventions.
///
/// Returns the sketch→sheet cast and its inverse projection: `sketch_of` sends **any** point of a
/// cast wall back to the sketch plane, whatever `h` the sheet carries there, because the wall is a
/// cone through the same apex. That is what lets one check cover both passes of a feature.
#[allow(clippy::type_complexity)]
fn drafted_cast(
    apex: &(Q, Q),
    r: &Q,
) -> (
    impl Fn(f64, f64) -> [f64; 3],
    impl Fn(&[f64; 3]) -> (f64, f64),
) {
    let (c, s) = (f(&apex.0), f(&apex.1));
    let r = f(r);
    let z_r = -r * c / s;
    let z_a = z_r - r * s / c;
    (
        move |x: f64, y: f64| -> [f64; 3] {
            let rho = x.hypot(y);
            let t = -z_a / (z_r - z_a + (c / s) * rho);
            [t * x, t * y, z_a + t * (z_r - z_a)]
        },
        move |p: &[f64; 3]| -> (f64, f64) {
            let u = (z_r - z_a) / (p[2] - z_a);
            (u * p[0], u * p[1])
        },
    )
}

/// A drawing's straight **flanks**, as endpoint pairs. Both cut files are arcs joined by exactly
/// two of these, and they are the radial walls the whole module is about.
fn flanks_of(profile: &[Edge<Bignum>]) -> Vec<([f64; 2], [f64; 2])> {
    profile
        .iter()
        .filter_map(|e| match e {
            Edge::Seg(seg) => Some((
                [sd(&seg.start.x), sd(&seg.start.y)],
                [sd(&seg.end.x), sd(&seg.end.y)],
            )),
            _ => None,
        })
        .collect()
}

/// **How many of the flat pattern's edges are the drawing's flanks** — the faithfulness check both
/// features get, and the reason a green certificate is not the end of it.
///
/// A flank crossing's splice ends with a cap along the flank's own ruling, so the flat pattern must
/// carry one edge per flank per pass, at the length the cast dictates. Each candidate is found by
/// that length (±10%), folded back to 3-D through the part's own `fold`, and pulled through the
/// cast onto the sketch plane, where it must lie within `tol` of one of the drawing's flank lines.
///
/// Nothing here is restated: the length comes from the spec's apex and the profile's own segments,
/// and the fold is the part's.
fn flank_edges_on_the_drawing(
    part: &Part<Bignum>,
    flat: &author::part::FlatPattern<Bignum>,
    profile: &[Edge<Bignum>],
    gauge: &Q,
    apex: &(Q, Q),
    tol: f64,
) -> usize {
    let (cast, sketch_of) = drafted_cast(apex, gauge);
    let flanks = flanks_of(profile);
    assert_eq!(flanks.len(), 2, "the drawing has two flank segments");
    let dist3 = |a: &[f64; 3], b: &[f64; 3]| {
        ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
    };
    let flank_len = {
        let (p, q) = &flanks[0];
        dist3(&cast(p[0], p[1]), &cast(q[0], q[1]))
    };

    let verts = &flat.outline().vertices;
    let n = verts.len();
    let mut on_flank = 0usize;
    for i in 0..n {
        let (ax, ay) = verts[i].center();
        let (bx, by) = verts[(i + 1) % n].center();
        let len = (f(&ax) - f(&bx)).hypot(f(&ay) - f(&by));
        if (len - flank_len).abs() > flank_len / 10.0 {
            continue;
        }
        let wire = match part.fold(&[[ax, ay], [bx, by]], &qi(0)) {
            Verdict::Verified(w) => w,
            v => panic!(
                "a candidate flank edge's endpoints must fold back, got {}",
                match v {
                    Verdict::Unresolved(_) => "Unresolved".to_string(),
                    Verdict::Refuted(fa) => format!("{fa:?}"),
                    Verdict::Verified(_) => unreachable!(),
                }
            ),
        };
        let near_a_flank = wire.points.iter().all(|p| {
            let p3 = [f(&p[0].mid()), f(&p[1].mid()), f(&p[2].mid())];
            let (sx, sy) = sketch_of(&p3);
            flanks.iter().any(|(a, b)| {
                // Distance from (sx, sy) to the segment a→b.
                let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                let t =
                    (((sx - a[0]) * dx + (sy - a[1]) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
                let (px, py) = (a[0] + t * dx, a[1] + t * dy);
                (sx - px).hypot(sy - py) < tol
            })
        });
        if near_a_flank {
            on_flank += 1;
        }
    }
    on_flank
}

/// **The drawing itself.** `data/inner-cut.dxf` reads exactly and the recipe carries it; what the
/// resolver cannot yet place is the tab, and it must say which of its refusals this is.
///
/// Op 1 is the inner cut — the op whose gap splits the section. Op 0 is the outer trim.
#[test]
fn the_device_drawing_splits_the_material_section_and_is_refused_by_name() {
    let v = device_with(acceptance::inner_cut_profile()).develop();
    assert!(
        matches!(v, Verdict::Refuted(PartFault::SectionNotSimple { op: 1 })),
        "the drawing's tab must refuse as a split section on op 1, got {}",
        fault(v)
    );
}

/// **#293 → #294 — the drawing develops.** Move the ccw ramp off the tab and the section stops
/// splitting, so the drawing gets past [`PartFault::SectionNotSimple`] — and then the whole chain
/// of certificate walls behind it, each step pinned by a measurement rather than inferred:
///
/// | | the drawing reported |
/// |---|---|
/// | before any of this | `Refuted(NappeCrossed)` — a DRC cushion used as a soundness gate |
/// | after the per-ball nappe check | `Unresolved ε 1.1220e1` |
/// | after #293's rail-per-run | `Unresolved ε 3.5000e0` — exactly `clearance/2`, the ball bound's "nothing certified" sentinel on eight tab rails |
/// | after the centred discriminant | `Refuted(RailSpanShort)` — the rails certify, and the boundary needs them past their windows |
/// | after the flank splice (§12.4 mid-chain) | `Unresolved ε 1.2e1` — the splice's own fillet loops could not be certified |
/// | after the nested-centred chart fields + the ball floor | `Unresolved ε 8.9e0` — the loops certify, the steep fillet rails' unroll chords do not |
/// | after the per-edge subdiv ladder | **`Verified`** |
///
/// Each wall was the same disease at a different layer: an interval enclosure blind to the scale
/// of a sub-millimetre feature — the µ̂-discriminant's cancellation, the chart fields' cancellation,
/// a ball floor as large as the fillet itself, a fixed chord subdivision on a rail that dives half
/// a µ̂-unit across its window.
///
/// **And the result is checked against the drawing, not just `Verified`.** A flank crossing's
/// splice ends with a cap along the flank's own ruling, so the flat pattern must contain the four
/// flank edges (two per tab pass) at the length the cast dictates — computed here from the spec's
/// apex and the profile's own flank segments, not restated — and folding each such edge's
/// endpoints back to 3-D must land, through the drafted-apex cast, on the drawing's flank lines
/// (the cut-file-is-the-cutter's-sketch ruling: the cast's radial displacement is the tool).
#[test]
fn the_drawings_tab_develops_and_its_flanks_land_on_the_drawing() {
    let mut spec = ramp_off_the_wedge();
    spec.inner_profile = Some(acceptance::inner_cut_profile());
    let part = self_lapping_cone_from(&spec, 8, 8, false, None);
    let flat = match part.develop() {
        Verdict::Verified(fl) => fl,
        v => panic!("the device drawing must develop, got {}", fault(v)),
    };

    // Tolerance measured, not derived: across the four true flank edges the pulled-back endpoints
    // sit 0.0000–0.051 off the drawing (the offset-sheet pass carries the splice vertices' tangent
    // gaps; the base pass is exact to 4 decimals), while the nearest decoy — a rim chord of
    // accidentally matching length — misses by 2.6. So 0.1 splits signal from decoy by a factor of
    // 26 and stays 15× under the drawing's smallest fillet.
    let on_flank = flank_edges_on_the_drawing(
        &part,
        &flat,
        &acceptance::inner_cut_profile(),
        spec.inner_r.as_ref().expect("the device has an inner cut"),
        &spec.apex,
        0.1,
    );
    assert_eq!(
        on_flank, 4,
        "two flanks on each of the tab's two passes: four flank edges in the flat pattern"
    );
}

/// **The rim comes out of a file too (#295).** `data/outer-cut.dxf` replaces the outer *disc*: the
/// same Ø 21.5 circle — the drawing states `r² = 1849/16` exactly, so the recipe's `outer_r` and
/// the file agree at `δ = 0` — interrupted over 15° about `+y` by a lug reaching out to Ø 27.5 on
/// two radial flanks and a 195° nose arc tangent to both.
///
/// It is the bore's tab inverted, and the inversion is the point. The tab is **subtracted** and
/// bays into the material; the lug is **intersected** and pushes the material out. Same wall kinds,
/// same radial flanks, opposite role — and the flat pattern must carry the same evidence: one edge
/// per flank per pass, folding back onto the drawing's own flank lines through the cast.
///
/// **Why this is not the σ-stock refusal** (`docs/cutter-extrude-design.md` §12.5): a kept contour
/// on this chart is refused by name because a ruling is a *line* and a swept profile a *prism*, so
/// the far nappe is kept too wherever an azimuth and its antipode are both swept. This outline
/// bounds **µ̂ alone** — it is cast from a point on the device's own axis and it encloses that axis
/// — so it moves the upper rail and closes nothing in σ. The exclusion is about contours that have
/// to say where material starts and stops, and a rim is not one.
///
/// **This is the acceptance criterion and it does not hold yet — #296.** The lug is now *kept*
/// (the shadow's abutting patches are coalesced, so an `Intersect` no longer fragments a non-convex
/// kept region — see `resolve::extruded_shadow`), and what stops it is a named refusal:
/// `RailSpanShort { op: 0 }`, §12.4's p-curve end at the lug's **mixed corner**. The bore's tab has
/// a *tangency at each end of its flank*, which is the shape #294's `flank_splice` detects; the
/// lug's flank is tangent to the nose arc at one end and meets the rim **transversally** at the
/// other, so the two windows never overlap and the splice does not fire. The boundary genuinely
/// jumps in µ̂ at the flank azimuth — a `Cap` along the flank wall, the same emission #294 already
/// builds — and it is the *detection* that has to grow.
///
/// Until then this refuses rather than lying, which is the whole difference from before: it used to
/// come back `Verified` with the tab cut *inward* as a bite, the emitted rim dipping to `15.884`
/// where the drawing wants `17.78`.
///
/// Kept as written rather than weakened, because it is the statement that has to become true.
#[test]
#[ignore = "#296: the lug's mixed corner needs the §12.4 splice — the criterion, not a pass"]
fn the_drawings_rim_lug_develops_and_its_flanks_land_on_the_drawing() {
    let mut spec = ramp_off_the_wedge();
    spec.outer_profile = Some(acceptance::outer_cut_profile());
    let part = self_lapping_cone_from(&spec, 8, 8, false, None);
    let flat = match part.develop() {
        Verdict::Verified(fl) => fl,
        v => panic!("the drawing's rim must develop, got {}", fault(v)),
    };

    let on_flank = flank_edges_on_the_drawing(
        &part,
        &flat,
        &acceptance::outer_cut_profile(),
        &spec.outer_r,
        &spec.apex,
        0.1,
    );
    assert_eq!(
        on_flank, 4,
        "two flanks on each of the lug's two passes: four flank edges in the flat pattern"
    );
}

/// **What the lug does today, pinned so the next change to it is not silent (#296).**
///
/// The criterion above is `#[ignore]`d. This one runs and asserts the current, *honest* behaviour:
/// the lug is kept, and the boundary it needs cannot be built yet, so the part is refused by name —
/// `RailSpanShort { op: 0 }`, §12.4's p-curve end at the lug's mixed corner. **When #296 is fixed
/// this test fails**, which is the point.
///
/// It is worth knowing what this replaced. Before the shadow's patches were coalesced, every lug
/// configuration came back `Verified` with the tab cut *inward* as a bite, and the ramp's position
/// appeared to matter — `TopologyMismatch` with 2 faces at the pinned ramp, 6 with the ramp over
/// the whole wedge, `Verified` either side. All of that was downstream of the fragmentation: the
/// kept region was split at every wall crossing, the component pick kept whichever piece held the
/// witness, and which piece that was moved with the geometry. None of it was about the ramp. The
/// ramp variants are gone from this file for that reason — they measured an artifact.
#[test]
fn the_rim_lug_refuses_by_name_pending_its_corner_splice() {
    let mut spec = ramp_off_the_wedge();
    spec.outer_profile = Some(acceptance::outer_cut_profile());
    let v = self_lapping_cone_from(&spec, 8, 8, false, None).develop();
    assert!(
        matches!(v, Verdict::Refuted(PartFault::RailSpanShort { op: 0 })),
        "the lug must refuse as the outer op's p-curve end, got {}",
        fault(v)
    );
}

/// **The flank, not the sheet, on its own is enough.** A straight-sided tab — vertical flanks, a
/// chord tip — is crossed sideways by a *radial* ruling too, because its flanks are not radial. So
/// this one splits the section on the base cone, where the drawing's radial-flanked tab does not.
///
/// Widening it does not help and that is the point: the two-interval band lives between the tab's
/// corner azimuth and its flank azimuth, which every width has. Measured across half-widths 0.347
/// through 2.400 mm, all refused.
#[test]
fn a_straight_flanked_tab_splits_the_section_at_every_width() {
    // A Pythagorean (23, 264, 265) puts both top corners exactly on r = 4: half-width 92/265.
    let (w, ytop) = (Q::new(92, 265), Q::new(1056, 265));
    let (a, b) = ([w.clone(), ytop.clone()], [w.neg(), ytop]);
    let tab = Profile::new()
        .arc(qi(0), qi(0), qi(16), b.clone(), a.clone())
        .polyline(&[a, [w.clone(), qi(2)], [w.neg(), qi(2)], b])
        .into_edges();

    let v = device_with(tab).develop();
    assert!(
        matches!(v, Verdict::Refuted(PartFault::SectionNotSimple { op: 1 })),
        "a straight-flanked tab must refuse as a split section on op 1, got {}",
        fault(v)
    );
}
