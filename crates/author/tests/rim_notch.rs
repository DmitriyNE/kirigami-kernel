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
//! Both features sit in the wedge the 410.7° chart covers **twice** (material azimuth is
//! `270° + 4·arctan σ`, so `az ∈ (64.6°, 115.4°)` is swept on the base cone and again on the
//! lapping sheet), and on the pinned device the second pass lands on the ramp. So each is refused
//! there, and *which* refusal it earns is the measurement:
//!
//! | | the bore's tab (subtract) | the rim's lug (intersect) |
//! |---|---|---|
//! | base pass | σ ≈ −1.079…−0.927, `h′ = 0` ✓ | σ ≈ −1.067…−0.937, `h′ = 0` ✓ |
//! | lapping pass | σ ≈ +0.888…+1.049, on the ramp | σ ≈ +0.937…+1.067, straddling the ramp's end |
//! | refusal | [`PartFault::SectionNotSimple`] — the kept material is two µ̂-intervals at one σ | [`PartFault::TopologyMismatch`] — the section stays simple; the assembled outline does not |
//!
//! The tab's route is understood: a region is modelled as **one µ̂-interval per σ** — a lower rail
//! and an upper rail, both graphs over σ — plus interior holes, and a tab that bays in sideways
//! splits that interval, which the section sampler sees and names. That is #291.
//!
//! The lug's is **not**, and the honest reading is narrower than it looks: `SectionNotSimple` never
//! fires, the outline comes back as two faces, and the coherence gate refuses it — but since the
//! lug's material is not being kept anywhere on this chart (#296), what the two faces are is an
//! open question, not a second face of #291. Stated as measured, and no further.
//!
//! With the ramp moved off the wedge, both features land on flat sheet on both passes. The bore's
//! tab then **develops faithfully** — its four flank edges land on the drawing through the cast,
//! which is the check that distinguishes a green certificate from a right part. The rim's lug
//! develops `Verified` and **is not in the emitted pattern at all** (#296): the boundary is the
//! plain rim, to four decimals the same as the same circle authored without a lug. That is a wrong
//! part with a green certificate, so it is pinned twice here — the criterion `#[ignore]`d, and a
//! tripwire asserting today's behaviour so the day it changes is not silent.

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
/// **This is the acceptance criterion and it does not hold yet — #296.** The develop reports
/// `Verified` and the emitted flat pattern is the **plain rim**: the lug's material is not kept.
/// Measured on this device at `segments = 8` — max `|r|` in the outline `16.3377`, at vertex 28,
/// `(12.2510, −10.8089)`; the *same circle authored as a profile with no lug* gives `16.3377` at
/// vertex 28, `(12.2510, −10.8089)`, identical to four decimals, where the lug's tip must develop
/// to `17.78`. The lug's walls are traced (90 outline points against the control's 54) but they
/// bound nothing. So this finds 0 flank edges where it needs 4.
///
/// It is specific to the wrapping device, which is what makes it a bug and not a scope line: on the
/// narrow gore a kept contour built the same way — a major arc plus a radial lug — carries its lug
/// (flat area `0.5932` against the same circle's `0.4219`), and a non-convex L-shaped kept contour
/// keeps its notch (`0.6587` against its convex hull's `1.4923`). #296 carries the diagnosis and
/// what has already been ruled out.
///
/// Kept as written rather than weakened, because it is the statement that has to become true.
/// `ε` when it runs: `3.478` against this part's `clearance/2 = 3.5` — a **rail-fit chord bound**,
/// `160 → 320 → 640` in `RailFit::subdiv` taking it `3.478 → 1.617 → 0.655` at `51 → 84 → 152` s,
/// while `segments` and the fit's `degree`/`bits` move it not at all.
#[test]
#[ignore = "#296: the lug is silently dropped on the wrapping device — the criterion, not a pass"]
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

/// **The defect itself, pinned so it cannot be forgotten (#296).**
///
/// The test above is the criterion and is `#[ignore]`d. This one runs, and asserts what the kernel
/// *actually does today*: the developed boundary reaches the rim and stops there, where the drawing
/// puts material 1.7 units further out. **When #296 is fixed this test fails**, which is the point
/// — it is a tripwire, not an endorsement.
///
/// Both radii are derived, not restated: the development places a point at its slant distance from
/// the apex, so a sheet radius `ρ` lands at `ρ·√(1 + cot²β)`; the rim's `ρ` is the recipe's gauge
/// (a cast fixes its own gauge radius) and the lug tip's is [`acceptance::lapped::draft_image`] of
/// the profile's own farthest point.
#[test]
fn the_rim_lugs_material_is_dropped_today() {
    let mut spec = ramp_off_the_wedge();
    let profile = acceptance::outer_cut_profile();
    // The profile's farthest point from the axis: an arc reaches `|c| + r`, a segment its ends.
    let mut sketch_max = 0.0f64;
    for e in &profile {
        let d = match e {
            Edge::Arc(a) => f(&a.circle.cx).hypot(f(&a.circle.cy)) + f(&a.circle.r2).sqrt(),
            Edge::Seg(s) => sd(&s.start.x)
                .hypot(sd(&s.start.y))
                .max(sd(&s.end.x).hypot(sd(&s.end.y))),
        };
        sketch_max = sketch_max.max(d);
    }
    let slant = {
        let (c, s) = (f(&spec.apex.0), f(&spec.apex.1));
        (1.0 + (c / s).powi(2)).sqrt()
    };
    let rim = f(&spec.outer_r) * slant;
    let tip = f(&acceptance::lapped::draft_image(
        &spec.apex,
        &spec.outer_r,
        &Q::new((sketch_max * 1_000_000.0) as i128, 1_000_000),
    )) * slant;
    assert!(
        tip > rim + 1.0,
        "the lug must reach well past the rim to be worth checking: rim {rim:.4}, tip {tip:.4}"
    );

    spec.outer_profile = Some(profile);
    let part = self_lapping_cone_from(&spec, 8, 8, false, None);
    let flat = match part.develop() {
        Verdict::Verified(fl) => fl,
        v => panic!(
            "the lug device develops (that is the whole problem), got {}",
            fault(v)
        ),
    };
    let reach = flat
        .outline()
        .vertices
        .iter()
        .map(|v| {
            let (x, y) = v.center();
            f(&x).hypot(f(&y))
        })
        .fold(0.0f64, f64::max);
    assert!(
        reach > rim - 1.0,
        "the pattern must at least reach the rim: {reach:.4} against {rim:.4}"
    );
    assert!(
        reach < (rim + tip) / 2.0,
        "#296 IS FIXED — the lug now reaches {reach:.4} (rim {rim:.4}, tip {tip:.4}). \
         Delete this tripwire and un-ignore the faithfulness test above."
    );
}

/// **The rim lug under the ramp is refused, and by a different name than the tab is.**
///
/// The lug's lapping pass runs `σ ≈ 0.937…1.067`, straddling the pinned ramp's end at `σ = 1`, so
/// part of it meets `h′ ≠ 0` sheet where a ruling crosses the radial flank instead of running along
/// it. [`PartFault::SectionNotSimple`] never fires — an intersect only moves the upper rail, so the
/// section stays one interval — and every certificate passes; the refusal comes at the far end,
/// where the exact flat boolean assembles the outline into **2 faces** and the coherence gate
/// declines to ship it. Sound, and symptom-named.
///
/// **What the ramp does to it is measured** — same file, same device, one parameter:
///
/// | ccw ramp | the lug's lapping pass meets | verdict |
/// |---|---|---|
/// | `[1/10, 1/2]` | plateau only | `Verified`, ε 3.478 |
/// | `[4/7, 9/10]` | plateau only | `Verified`, ε 3.473 |
/// | `[4/7, 1]` (pinned) | ramp end, then plateau | `TopologyMismatch`, **2** faces |
/// | `[9/10, 11/10]` | ramp throughout | `TopologyMismatch`, **6** faces |
///
/// So the refusal tracks `h′ ≠ 0` under the flank, and the face count tracks how much of the wedge
/// is under it. **What it is not yet safe to conclude** is that this is the tab's limit wearing
/// another face: the two `Verified` rows above emit the plain rim (#296), so the lug's walls are not
/// bounding anywhere on this chart, and whatever the extra faces are, they are not the lug's
/// boundary being modelled badly. This is pinned as a refusal that must keep happening, not as a
/// diagnosis.
#[test]
fn the_pinned_devices_ramp_refuses_the_rim_lug() {
    let mut spec = self_lapping_spec();
    spec.outer_profile = Some(acceptance::outer_cut_profile());
    let v = self_lapping_cone_from(&spec, 8, 8, false, None).develop();
    assert!(
        matches!(
            v,
            Verdict::Refuted(PartFault::TopologyMismatch {
                expected_holes: 0,
                faces,
                ..
            }) if faces != 1
        ),
        "the lug under the ramp must refuse as an incoherent outline, got {}",
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
