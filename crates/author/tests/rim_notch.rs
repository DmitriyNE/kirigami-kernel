//! **The device's real drawing against the resolver's boundary model (#291, #293, #294).**
//!
//! The device's real inner cut (`acceptance/data/inner-cut.dxf`) is the Ø 8 bore with a 10° tab
//! reaching in to Ø 4. On the pinned device — where the tab sits under the seam ramp — it is
//! refused, and *which* refusal it earns matters: the resolver models a region as **one
//! µ̂-interval per σ** — a lower rail and an upper rail, both graphs over σ — plus interior holes,
//! and a tab that a ruling crosses sideways makes the material two intervals at one σ. That is
//! [`PartFault::SectionNotSimple`], not an ambiguity and not a degenerate wall. With the ramp
//! moved off the tab the section stays simple, and the drawing **develops** — through the flank
//! splices and the enclosure fixes the third test's doc chronicles.
//!
//! **Measured on the pinned device, and this is why the name matters.** The tab appears *twice* on
//! the 410.7° chart, at the same plan azimuth:
//!
//! | pass | region | `h` | ruling's plan miss | section |
//! |---|---|---|---|---|
//! | σ ≈ −1.079…−0.927 | 0 | `0` | **exactly 0** (radial) | one interval ✓ |
//! | σ ≈ +0.888…+1.049 | 1 (the ramp) | `0 → 1/4` | up to **0.481 mm** | **two intervals** at σ = 0.897, 0.906, 0.915 ✗ |
//!
//! Same cut, same azimuth, same walls traversed in the same order — only the sheet differs. The
//! split stretches are separated by a gap running from the tab's root fillet to its flank, and it
//! is *not* a hole: it opens into the exterior at the low-σ end of its band, so merging the two
//! stretches into a face-with-hole would emit a closed island where the part has an open bay.
//!
//! The tab is 0.35 mm half-wide at its root and the ramp's rulings miss the axis by more than that,
//! which is the one-line statement of the geometry: on the ramp the ruling is wider off-axis than
//! the tab is wide.

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
    let mut spec = self_lapping_spec();
    spec.ccw.ramp_start = acceptance::lapped::Azimuth::Sigma(Q::new(1, 10));
    spec.ccw.ramp_end = acceptance::lapped::Azimuth::Sigma(Q::new(1, 2));
    spec.inner_profile = Some(acceptance::inner_cut_profile());
    let part = self_lapping_cone_from(&spec, 8, 8, false, None);
    let flat = match part.develop() {
        Verdict::Verified(fl) => fl,
        v => panic!("the device drawing must develop, got {}", fault(v)),
    };

    // — The cast, in floats, straight from the spec (`acceptance::lapped::normal_cut`'s own
    //   construction): the profile plane at z_r, the drafted apex below it, the neutral cone
    //   z = −(c/s)·ρ. The gauge radius is its fixed point, which pins the sign conventions. —
    let f = |r: &Q| -> f64 {
        let (n, d) = r.numer_denom_decimal();
        n.parse::<f64>().unwrap() / d.parse::<f64>().unwrap()
    };
    let (c, s) = (f(&spec.apex.0), f(&spec.apex.1));
    let r_gauge = f(spec.inner_r.as_ref().expect("the device has an inner cut"));
    let z_r = -r_gauge * c / s;
    let z_a = z_r - r_gauge * s / c;
    let cast = |x: f64, y: f64| -> [f64; 3] {
        let rho = x.hypot(y);
        let t = -z_a / (z_r - z_a + (c / s) * rho);
        [t * x, t * y, z_a + t * (z_r - z_a)]
    };
    let sketch_of = |p: &[f64; 3]| -> (f64, f64) {
        let u = (z_r - z_a) / (p[2] - z_a);
        (u * p[0], u * p[1])
    };

    // The drawing's two flank segments, and the length each cuts on the sheet.
    let sd = |v: &Surd<Bignum>| -> f64 {
        let (a, b, d) = v.parts();
        f(a) + f(b) * f(d).sqrt()
    };
    let flanks: Vec<([f64; 2], [f64; 2])> = acceptance::inner_cut_profile()
        .iter()
        .filter_map(|e| match e {
            Edge::Seg(seg) => Some((
                [sd(&seg.start.x), sd(&seg.start.y)],
                [sd(&seg.end.x), sd(&seg.end.y)],
            )),
            _ => None,
        })
        .collect();
    assert_eq!(flanks.len(), 2, "the drawing has two flank segments");
    let dist3 = |a: &[f64; 3], b: &[f64; 3]| {
        ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
    };
    let flank_len = {
        let (p, q) = &flanks[0];
        dist3(&cast(p[0], p[1]), &cast(q[0], q[1]))
    };

    // — Find the flat edges of that length, fold their endpoints back, and pull each through the
    //   cast to the sketch plane: they must lie on a flank segment of the drawing. —
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
        let wire = match part.fold(&[[ax, ay], [bx, by]], &Q::from_i128(0)) {
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
        // Tolerance measured, not derived: across the four true flank edges the pulled-back
        // endpoints sit 0.0000–0.051 off the drawing (the offset-sheet pass carries the splice
        // vertices' tangent gaps; the base pass is exact to 4 decimals), while the nearest decoy —
        // a rim chord of accidentally matching length — misses by 2.6. So 0.1 splits signal from
        // decoy by a factor of 26 and stays 15× under the drawing's smallest fillet.
        let near_a_flank = wire.points.iter().all(|p| {
            let p3 = [f(&p[0].mid()), f(&p[1].mid()), f(&p[2].mid())];
            let (sx, sy) = sketch_of(&p3);
            flanks.iter().any(|(a, b)| {
                // Distance from (sx, sy) to the segment a→b.
                let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                let t =
                    (((sx - a[0]) * dx + (sy - a[1]) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
                let (px, py) = (a[0] + t * dx, a[1] + t * dy);
                (sx - px).hypot(sy - py) < 0.1
            })
        });
        if near_a_flank {
            on_flank += 1;
        }
    }
    assert_eq!(
        on_flank, 4,
        "two flanks on each of the tab's two passes: four flank edges in the flat pattern"
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
