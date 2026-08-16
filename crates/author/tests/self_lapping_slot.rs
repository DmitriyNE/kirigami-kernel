//! **#269 — the sketch-extrude cutter on the hardest chart there is.**
//!
//! Everything AUTH.1/AUTH.2 had been exercised on lived on `acceptance::sketch_panel`: one region,
//! `SupportFn::inherit` (so `γ ≡ 0`), no wrap. The self-lapping device is the other end of the
//! range — a chart that sweeps 410.7° of azimuth so the tail passes over the head, three
//! piecewise-support regions, and a smoothstep ramp whose flat directrix does not vanish. Until now
//! every cut on it was a **metric** cylinder, so the tracer, the resolver's window derivation and
//! the per-slice solid clipping had never met either the lap or `γ ≠ 0`.
//!
//! `acceptance::lap_slot` puts one L-shaped extrusion in the lap wedge, where a vertical sweep
//! pierces **both sheets** at once: the near hole lands on the body at `γ ≡ 0`, the far one on the
//! ramp at `γ ≠ 0`. One cutter, two traced footprints, both development tiers — which is what makes
//! the differences between the two holes attributable to the thing under test and to nothing else.
//!
//! The pinned device (`acceptance::self_lapping_cone`) is untouched: it carries the VV.1 work
//! budgets, VV.2 ε bounds and VV.3 goldens of `self_lapping_part.rs`, and a baseline that moves
//! whenever a fixture is added is not a baseline. This is its sibling, with its own pins.

use acceptance::measure;
use author::part::{OpRole, Part};
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

/// A parallel (`w = 0`) sweep along `z` — the apex the fixture is pinned with.
fn parallel() -> Apex<Bignum> {
    Apex::direction([qi(0), qi(0), qi(1)]).expect("a real direction")
}

/// The device carrying the lap slot, at the suite's lean budget.
fn slotted(segments: usize, apex: Apex<Bignum>) -> Part<Bignum> {
    acceptance::self_lapping_cone_with(segments, 8, true, Some((apex, acceptance::lap_slot())))
}

fn develop_or_panic(part: &Part<Bignum>, name: &str) -> author::part::FlatPattern<Bignum> {
    match part.develop() {
        Verdict::Verified(f) => f,
        Verdict::Refuted(fault) => panic!("{name}: refuted: {fault:?}"),
        Verdict::Unresolved(e) => panic!("{name}: unresolved at ε ≈ {:.3e}", rat_to_f64(&e)),
    }
}

/// The genus of a closed shell: `χ = V − E + (2F − L)` over the doubled faces, `g = (2 − χ)/2`.
fn genus(b: &export::brep::Brep<Bignum>) -> i64 {
    let v = b.verts().len() as i64;
    let e = b.edges().len() as i64;
    let f = b.faces().len() as i64;
    let l: i64 = b.faces().iter().map(|fc| 1 + fc.holes.len() as i64).sum();
    (2 - (v - e + (2 * f - l))) / 2
}

/// Fold one flat point back onto the surface and report `(x, y, z)`.
fn folded(part: &Part<Bignum>, p: [Q; 2]) -> [f64; 3] {
    match part.fold(&[p], &qi(0)) {
        Verdict::Verified(w) => {
            let v = &w.points[0];
            [
                rat_to_f64(&v[0].mid()),
                rat_to_f64(&v[1].mid()),
                rat_to_f64(&v[2].mid()),
            ]
        }
        Verdict::Unresolved(e) => panic!("refold unresolved at ε ≈ {:.3e}", rat_to_f64(&e)),
        Verdict::Refuted(f) => panic!("refold refuted: {f:?}"),
    }
}

/// **Which sheet a folded point came back to**, as a number: its signed offset from the *base* cone
/// `72·√(x² + y²) + 65·z = 0`, along that cone's own unit normal `(72/97 radial, 65/97 axial)`.
///
/// The device's three regions ride one cone: the body sits **on** it (`h ≡ 0`), the ramp climbs
/// `0 → 1/10` across the lap and the tail plateau stays there. So this reads off the artifact which
/// region a hole was cut in, without asking the recipe.
fn sheet_offset(p: [f64; 3]) -> f64 {
    (72.0 * p[0].hypot(p[1]) + 65.0 * p[2]) / 97.0
}

/// One derived hole, seen three ways: the ring the SVG draws for it, the loop `develop()` certified,
/// and the sheet a fold of its first vertex comes back to.
struct Hole {
    /// Index into [`FlatPattern::holes`](author::part::FlatPattern::holes).
    idx: usize,
    ring: Vec<[f64; 2]>,
    area: f64,
    /// The folded first vertex — its sheet offset and its height.
    offset: f64,
    z: f64,
    /// Does that folded vertex lie on the **seam drill's** cylinder? The device has two cutters over
    /// the lap and this is which one made the hole, asked of the cutter itself
    /// ([`acceptance::seam_drill_axis`]) rather than inferred from a size or a position in a list.
    on_drill: bool,
}

/// The device's derived holes, each ring paired with the loop it came from.
///
/// The two orders are **not** the same one: `FlatPattern::holes()` is in op order, and the assembled
/// region's rings come in the exact boolean's own order. Pairing them by flat centroid is what makes
/// a shape measured on a ring and a sheet measured by folding its loop refer to one object — the
/// four loops sit ~1.3 apart in the pattern and each is ~0.4 across, so the match is unambiguous.
fn derived_holes(part: &Part<Bignum>, flat: &author::part::FlatPattern<Bignum>) -> Vec<Hole> {
    let faces = measure::emitted_hole_rings(flat.region());
    assert_eq!(faces.len(), 1, "the device is one face");
    let rings = faces.into_iter().next().unwrap();
    assert_eq!(rings.len(), flat.holes().len(), "one ring per derived loop");
    let centroid = |pts: &[[f64; 2]]| -> [f64; 2] {
        let n = pts.len() as f64;
        let s = pts
            .iter()
            .fold([0.0f64; 2], |a, p| [a[0] + p[0], a[1] + p[1]]);
        [s[0] / n, s[1] / n]
    };
    flat.holes()
        .iter()
        .enumerate()
        .map(|(idx, h)| {
            let pts: Vec<[f64; 2]> = h
                .vertices
                .iter()
                .map(|b| {
                    let (x, y) = b.center();
                    [rat_to_f64(&x), rat_to_f64(&y)]
                })
                .collect();
            let c = centroid(&pts);
            let ring = rings
                .iter()
                .min_by(|a, b| {
                    let d = |r: &Vec<[f64; 2]>| {
                        let m = centroid(r);
                        (m[0] - c[0]).hypot(m[1] - c[1])
                    };
                    d(a).partial_cmp(&d(b)).unwrap()
                })
                .expect("a ring per loop")
                .clone();
            let (x, y) = h.vertices[0].center();
            let p = folded(part, [x, y]);
            let (dcx, dcy, dr2) = acceptance::seam_drill_axis();
            let on_axis = (p[0] - rat_to_f64(&dcx)).powi(2) + (p[1] - rat_to_f64(&dcy)).powi(2)
                - rat_to_f64(&dr2);
            Hole {
                idx,
                area: measure::ring_area(&ring),
                ring,
                offset: sheet_offset(p),
                z: p[2],
                on_drill: on_axis.abs() < 5e-2,
            }
        })
        .collect()
}

/// The **mean height** the sweep met a sheet at, over eight of the loop's vertices.
///
/// The taper varies across a footprint, so the ratio of two holes' areas reflects a mean rather than
/// any one point — and on this device a single vertex is off by enough to matter (the body sheet's
/// vertices span `z ∈ [−3.09, −2.98]`).
fn mean_sheet_z(part: &Part<Bignum>, hole: &develop::unroll::FlatOutline<Bignum>) -> f64 {
    let v = &hole.vertices;
    let step = v.len().div_ceil(8).max(1);
    let (mut acc, mut n) = (0.0f64, 0.0f64);
    for j in (0..v.len()).step_by(step) {
        let (x, y) = v[j].center();
        acc += folded(part, [x, y])[2];
        n += 1.0;
    }
    acc / n
}

/// The slot's two holes as `(near, far)`: the one cut in the **body**, which lies on the base cone,
/// and the one cut in the **flap** lapping over it.
///
/// Both classifications are physical rather than positional. Folding one vertex answers *which
/// cutter* made a hole — the drill's loops come back onto the drill cylinder, the slot's do not —
/// and *which sheet* it was made in. Neither appeals to op order or to the boolean's ring order, and
/// neither uses size: under draft the slot's holes grow to within 5% of the drill's, so an area
/// threshold silently reclassifies them (it did, until this test failed).
fn slot_pair(holes: &[Hole]) -> (&Hole, &Hole) {
    let slot: Vec<&Hole> = holes.iter().filter(|h| !h.on_drill).collect();
    assert_eq!(
        slot.len(),
        2,
        "two of the four loops must fold back off the drill cylinder; areas {:?}",
        holes.iter().map(|h| h.area).collect::<Vec<_>>()
    );
    let (a, b) = (slot[0], slot[1]);
    if a.offset.abs() < b.offset.abs() {
        (a, b)
    } else {
        (b, a)
    }
}

/// **One cutter, two sheets — and the two footprints are traced on opposite sides of `γ = 0`.**
///
/// The lap wedge is covered twice, so a vertical extrusion placed in it reaches the body *and* the
/// flap that passes over it. Everything here is read off the emitted pattern and its fold-back:
///
/// * the recipe's one `Cutter::extrude` derives **two** holes, so the device carries four (the seam
///   drill already derives two of its own);
/// * one slot hole folds back **onto** the base cone and the other onto a sheet lifted `0.081…0.093`
///   along its normal — the smoothstep ramp's own `h`, still *climbing* across the footprint, which
///   is what `γ ≠ 0` means here;
/// * and the two developed holes are nevertheless the **same shape**, because development is an
///   isometry and a prism cuts congruent patches from two parallel sheets of one cone.
///
/// The last clause is the one with teeth. A `γ` that were quietly dropped, or accumulated into the
/// wrong region's frame, would still certify — `ε` is the max over stages and this device's panel
/// boundary dominates it — but the flap's traced hole would come out the wrong size or shape.
#[test]
fn the_lap_slot_pierces_both_sheets_of_the_wrap() {
    let part = slotted(16, parallel());
    counters::reset();
    let flat = develop_or_panic(&part, "the lap slot");
    // **VV.1** — read before anything else touches the pipeline: the fold-backs below integrate γ
    // too, and a work budget that includes them is measuring the test rather than the development.
    let (gamma, vel, cuts) = (
        counters::gamma_cells(),
        counters::gamma_velocity(),
        counters::cut_evals(),
    );

    let roles: Vec<OpRole> = flat.report().ops.iter().map(|o| o.role).collect();
    assert_eq!(
        roles,
        vec![
            OpRole::LowerBound,
            OpRole::UpperBound,
            OpRole::Hole,
            OpRole::Hole
        ],
        "derived roles: D1 bounds below, D2 above, the seam drill holes, the slot holes"
    );
    assert_eq!(flat.region().faces.len(), 1);
    assert_eq!(
        flat.region().faces[0].holes.len(),
        4,
        "two cutters over the lap, two derived holes each"
    );
    assert_eq!(flat.holes().len(), 4);

    // Which sheet each of the four came back to.
    let holes = derived_holes(&part, &flat);
    for (i, h) in holes.iter().enumerate() {
        println!(
            "[sheet] hole {i}: area {:.6}  normal offset {:+.5}  z {:.5}",
            h.area, h.offset, h.z
        );
    }
    assert_eq!(
        holes.iter().filter(|h| h.offset.abs() < 1e-3).count(),
        2,
        "two of the four holes are cut in the body, which lies on the base cone"
    );
    let (near, far) = slot_pair(&holes);
    assert!(
        near.offset.abs() < 1e-3,
        "the slot's near hole is cut in the body, on the base cone: offset {:+.5}",
        near.offset
    );
    assert!(
        far.offset > 0.06 && far.offset < 0.098,
        "the slot's far hole must land on the *ramp*, between the body (0) and the plateau (1/10): \
         offset {:+.5}",
        far.offset
    );

    // The ramp hole's lift still *varies* across its own footprint. The tail plateau's drill hole is
    // lifted too, but sits at a constant `h ≡ 1/10` — so the spread is what separates "the support
    // is nonzero here" from "the support is still curving here", and the ramp is the only place on
    // this device where the flat directrix is integrated under a moving support.
    let far_loop = &flat.holes()[far.idx];
    let n = far_loop.vertices.len();
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for j in (0..n).step_by(n.div_ceil(8).max(1)) {
        let (x, y) = far_loop.vertices[j].center();
        let o = sheet_offset(folded(&part, [x, y]));
        lo = lo.min(o);
        hi = hi.max(o);
    }
    println!("[sheet] the ramp hole's support climbs over [{lo:.5}, {hi:.5}] across its footprint");
    assert!(
        hi - lo > 5e-3,
        "the ramp's support must still be climbing across the footprint — a spread of {:.2e} means \
         the far hole landed on the constant-support plateau instead, and γ ≠ 0 is not exercised",
        hi - lo
    );

    // …and the two traced footprints are congruent. Measured areas 0.069953 (body) vs 0.070007
    // (ramp), 0.08% apart, and perimeters 1.269143 vs 1.268027, 0.09% apart (2026-08-16).
    let perim = |r: &[[f64; 2]]| -> f64 {
        (0..r.len())
            .map(|i| {
                let (a, b) = (r[i], r[(i + 1) % r.len()]);
                ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt()
            })
            .sum()
    };
    let (pn, pf) = (perim(&near.ring), perim(&far.ring));
    println!(
        "[congruence] slot areas body {:.6} / ramp {:.6}   perimeters {pn:.6} / {pf:.6}",
        near.area, far.area
    );
    assert!(
        (near.area - far.area).abs() < 0.01 * near.area,
        "the two traced footprints must enclose the same area: {:.6} vs {:.6}",
        near.area,
        far.area
    );
    assert!(
        (pn - pf).abs() < 0.01 * pn,
        "…and have the same perimeter: {pn:.6} vs {pf:.6}"
    );

    // **VV.2** — measured 4.1481e-1 develop (identical to the featureless device: the panel
    // boundary dominates ε, which is exactly why the checks above are shape measurements and not
    // verdicts) and a slot cut bound of 4.1712e-3 (2026-08-16).
    let cut = flat.report().ops[3]
        .cut_eps
        .clone()
        .expect("the slot resolved as a hole and so carries its own cut bound");
    println!(
        "[budget] lap slot develop {:.4e}   cut {:.4e}",
        rat_to_f64(flat.eps()),
        rat_to_f64(&cut)
    );
    assert!(
        flat.eps().cmp(&q(1, 2)) == core::cmp::Ordering::Less,
        "develop ε {:.4e} is not under the DRC gate 5.0000e-1",
        rat_to_f64(flat.eps())
    );
    assert!(
        cut.cmp(&q(1, 100)) == core::cmp::Ordering::Less,
        "the traced cut certified to {:.4e}, above its 1.0000e-2 budget",
        rat_to_f64(&cut)
    );

    // Measured γ cells 4 336, γ′ evaluations 5 136 and cut-certificate evaluations 17 408
    // (2026-08-16); the featureless device sits at 2 256 / 2 640 / 4 096, so tracing one L-shaped
    // footprint through two sheets roughly doubles the γ work and quadruples the cut work. These are
    // **complexity** gates like their siblings in `work_budget.rs`: what they catch is a change of
    // shape, not a drift.
    println!("[work] slotted develop  γ cells {gamma}  γ′ {vel}  cut evals {cuts}");
    assert!(
        gamma <= 6_100 && vel <= 7_200 && cuts <= 24_000,
        "the slotted development's work moved shape: γ {gamma}/6100, γ′ {vel}/7200, cuts \
         {cuts}/24000"
    );
}

/// **The four-crossing signature, read against the ruling family the development actually
/// produces.**
///
/// A cone develops by an isometry that sends each ruling to a straight line, so "a ruling meets the
/// cutter twice" — the property AUTH.2 exists for — is four crossings of the developed hole. On a
/// constant-support chart every one of those lines passes through the flat apex, and sampling rays
/// from the origin *is* sampling the family; that is what `measure::max_ray_crossings` does and what
/// the AUTH.2f fixtures use.
///
/// This device breaks that. Its ramp carries a nonzero flat directrix, so the ruling images are
/// offset by `γ(σ)` and the family stops being a pencil — measured below at `|γ| ≈ 0.16` where the
/// body's is exactly 0. A ray from the origin is then simply not a ruling, and the signature has to
/// be read against `Part::flat_rulings`. The three metric holes are the control: the seam drill's
/// two are ordinary bands and give two crossings, on both sheets.
#[test]
fn a_ruling_meets_the_traced_footprint_twice_on_each_sheet() {
    let part = slotted(16, parallel());
    let flat = develop_or_panic(&part, "the lap slot");

    // The ruling family over the whole declared domain σ ∈ [−5/4, 5/4], finely enough that several
    // land inside each footprint (whose σ-extent is ≈ 0.05).
    const N: i128 = 400;
    let sigmas: Vec<Q> = (0..=N).map(|k| Q::new(5 * (2 * k - N), 4 * N)).collect();
    let developed = part.flat_rulings(&sigmas).expect("the rulings develop");
    let rulings: Vec<[[f64; 2]; 2]> = developed
        .iter()
        .map(|[a, b]| {
            let pt = |p: &develop::cone::FlatBox<Bignum>| {
                let (x, y) = p.center();
                [rat_to_f64(&x), rat_to_f64(&y)]
            };
            [pt(a), pt(b)]
        })
        .collect();

    // The instrument is needed, not merely available: on the body the family IS a pencil through
    // the flat apex, and on the ramp it is not.
    let gamma = |s: &Q| -> f64 {
        let i = sigmas
            .iter()
            .position(|t| t.cmp(s) == core::cmp::Ordering::Equal);
        let [g, _] = &developed[i.expect("a sampled σ")];
        let (x, y) = g.center();
        rat_to_f64(&x).hypot(rat_to_f64(&y))
    };
    let (body, ramp) = (gamma(&q(-1, 2)), gamma(&q(7, 8)));
    println!("[pencil] |γ| at σ = −1/2 (body): {body:.5}   at σ = 7/8 (ramp): {ramp:.5}");
    assert!(
        body < 1e-9,
        "the body's support is constant, so its rulings must be concurrent at the flat apex"
    );
    assert!(
        ramp > 1e-2,
        "the ramp's rulings must NOT be concurrent at the flat apex — if |γ| ≈ {ramp:.2e} the \
         fixture is not exercising the case this measurement exists for"
    );

    let holes = derived_holes(&part, &flat);
    for (i, h) in holes.iter().enumerate() {
        println!(
            "[phenom] hole {i} (area {:.6}, offset {:+.5}): rulings {}, rays {}",
            h.area,
            h.offset,
            measure::max_ruling_crossings(&h.ring, &rulings),
            measure::max_ray_crossings(&h.ring),
        );
    }
    let (near, far) = slot_pair(&holes);
    assert_eq!(
        measure::max_ruling_crossings(&near.ring, &rulings),
        4,
        "some ruling must meet the slot's near footprint twice"
    );
    assert_eq!(
        measure::max_ruling_crossings(&far.ring, &rulings),
        4,
        "…and its far one, the traced footprint over γ ≠ 0 — that is the whole point of the fixture"
    );
    for h in holes.iter().filter(|h| h.on_drill) {
        assert_eq!(
            measure::max_ruling_crossings(&h.ring, &rulings),
            2,
            "the seam drill's holes are bands, which is what makes the four a signature"
        );
    }
}

/// **The taper tells the two sheets apart — a faithfulness check no single-sheet gore can make.**
///
/// One cutter, two holes, at two different heights: measured `z = −3.059` on the body and `z =
/// −2.939` on the flap that laps over it. Swept **parallel** the cutter is a prism, so the two holes
/// come out the same size. Swept from a **cast point** at `z = 12` it is a cone, so the higher sheet
/// — nearer the apex — gets the smaller hole, by exactly the ratio the two folded heights predict:
///
/// ```text
/// (12 − z_flap)² / (12 − z_body)²  =  0.9842        measured  0.9837
/// ```
///
/// Everything else cancels. The two holes ride the same panel, the same rails and the same `ε`, and
/// differ only in which sheet they were cut on — so the residual after dividing them is the draft
/// and nothing else. A cutter that ignored its apex, or applied the taper at one nominal height for
/// the whole part, passes every certificate and fails this by 1.6%.
#[test]
fn the_taper_tells_the_two_sheets_apart() {
    let z_apex = 12.0;
    let run = |apex: Apex<Bignum>, name: &str| {
        let part = slotted(16, apex);
        let flat = develop_or_panic(&part, name);
        (part, flat)
    };
    let (part_p, flat_p) = run(parallel(), "the parallel slot");
    let (part_d, flat_d) = run(
        Apex::point([q(27, 40), q(27, 10), qi(12)]),
        "the drafted slot",
    );
    let (hp, hd) = (
        derived_holes(&part_p, &flat_p),
        derived_holes(&part_d, &flat_d),
    );
    let ((p_near, p_far), (d_near, d_far)) = (slot_pair(&hp), slot_pair(&hd));
    let (pb, pf, db, df) = (p_near.area, p_far.area, d_near.area, d_far.area);

    // The heights come from the drafted run, which is the one whose prediction they enter.
    let (zb, zf) = (
        mean_sheet_z(&part_d, &flat_d.holes()[d_near.idx]),
        mean_sheet_z(&part_d, &flat_d.holes()[d_far.idx]),
    );
    println!("[taper] sheet heights: body z {zb:.5}, flap z {zf:.5}");
    assert!(
        zf > zb,
        "the flap laps *over* the head, so it must sit higher: body {zb:.5}, flap {zf:.5}"
    );

    // Parallel: a prism cuts congruent holes on two parallel sheets.
    let flat_ratio = pf / pb;
    println!("[taper] parallel area ratio flap/body {flat_ratio:.5}");
    assert!(
        (flat_ratio - 1.0).abs() < 5e-3,
        "swept parallel the two holes must match: {flat_ratio:.5}"
    );

    // Drafted: the ratio the two heights predict, against the one the pattern shows.
    let predicted = ((z_apex - zf) / (z_apex - zb)).powi(2);
    let drafted_ratio = df / db;
    println!("[taper] drafted area ratio flap/body {drafted_ratio:.5}   predicted {predicted:.5}");
    assert!(
        (1.0 - drafted_ratio) > 5e-3,
        "the draft must actually resolve the two sheets — a ratio of {drafted_ratio:.5} is the \
         parallel answer, so the apex is not reaching the cut"
    );
    assert!(
        (drafted_ratio - predicted).abs() < 3e-3,
        "the drafted holes differ by {drafted_ratio:.5} where the folded heights predict \
         {predicted:.5}"
    );
    // …and both drafted holes are larger than their parallel twins: this surface sits *below* the
    // sketch plane, so a cone from `z = 12` has widened by the time it reaches it (the flat panel's
    // own draft test measures the same effect with the opposite sign, at z ≈ 2.4).
    println!(
        "[taper] drafted/parallel: body {:.5}, flap {:.5}",
        db / pb,
        df / pf
    );
    assert!(
        db > pb && df > pf,
        "the cut is below the sketch plane, so the drafted holes must be the larger ones"
    );
}

/// **The chord golden on a traced footprint: chord spacing, not a bridge.**
///
/// The VV.3 metric — longest emitted edge as a fraction of the ring's own size — scores this slot at
/// **28.6%** at `segments(16)`, inside the 30–48% band that the metric was built to catch a real
/// defect in (a closed cut split into two `µ̂ = f(σ)` graphs and bridged across the tangent rulings).
/// A gate alone could not tell the two apart, so this measures the property that does: **a bridge is
/// structural and a chord is not**. Doubling the resolution leaves a bridge where it was and halves
/// a chord.
///
/// Measured 28.6% → 18.0% → 9.0% at `segments` 16 → 32 → 64 (2026-08-16), against the seam drill's
/// 9.4% → 4.7% → 2.4% on the same runs. The slot scores worse than the drill at every resolution for
/// a reason that is also not a defect: an L's own straight sides are legitimately a large fraction of
/// its bounding box, and where the drill's ring is a p-curve chorded uniformly by `segments`, the
/// traced loop's vertices come from the σ-event partition and are not evenly spread.
#[test]
fn the_slots_chord_golden_is_spacing_and_not_a_bridge() {
    let golden = |segments: usize| -> (f64, f64) {
        let part = slotted(segments, parallel());
        let flat = develop_or_panic(&part, "the lap slot");
        let holes = derived_holes(&part, &flat);
        let worst = |drill: bool| {
            holes
                .iter()
                .filter(|h| h.on_drill == drill)
                .map(|h| measure::longest_edge_fraction(&h.ring))
                .fold(0.0, f64::max)
        };
        let (slot, drill) = (worst(false), worst(true));
        println!(
            "[golden] segments {segments}: slot {:.1}%, drill {:.1}%",
            slot * 100.0,
            drill * 100.0
        );
        (slot, drill)
    };
    let (coarse, coarse_drill) = golden(16);
    let (fine, fine_drill) = golden(32);

    assert!(
        coarse < 0.35,
        "the slot's longest emitted edge is {:.1}% of its extent even before refinement",
        coarse * 100.0
    );
    assert!(
        fine < 0.75 * coarse,
        "doubling the resolution took the slot's chord golden {:.1}% → {:.1}%, barely moving it — \
         an edge that survives refinement is a bridge across the tangent rulings, not chord spacing",
        coarse * 100.0,
        fine * 100.0
    );
    // The metric control on the same runs: the drill's p-curve ring chords at exactly 1/n, which is
    // what "refinement works here" looks like when the sampling is uniform.
    assert!(
        fine_drill < 0.6 * coarse_drill,
        "the drill's golden {:.1}% → {:.1}% did not halve, so the comparison above is measuring the \
         resolution knob rather than the loop",
        coarse_drill * 100.0,
        fine_drill * 100.0
    );
}

/// **The slotted device is still a solid a CAD kernel can read.**
///
/// Two traced loops become two tunnels through a shell that already had two (the seam drill's), so
/// the genus goes 2 → 4 — the un-slotted device's 2 is pinned in `self_lapping_part.rs`. Beyond the
/// verdict: the polygon channel ran on **both** footprints, the shell is watertight and manifold,
/// and its shortest emitted edge clears the vertex tolerance a floating-point consumer reads it with
/// (#267 — every certificate passes on a shell OCCT then refuses to write).
#[test]
fn the_slotted_device_builds_a_certified_solid() {
    counters::reset();
    let solid = match slotted(16, parallel()).solid() {
        Verdict::Verified(s) => s,
        Verdict::Refuted(f) => panic!("the slotted solid was refused: {f:?}"),
        Verdict::Unresolved(e) => panic!("the slotted solid: ε ≈ {:.3e}", rat_to_f64(&e)),
    };
    let clips = counters::poly_slice_clips();
    let brep = solid.brep();
    println!(
        "[solid] slotted: {} faces, genus {}, {clips} polygon-channel slice clips, ε {:.4e}",
        brep.faces().len(),
        genus(brep),
        rat_to_f64(solid.eps()),
    );
    assert_eq!(brep.free_edges(), 0, "watertight");
    assert_eq!(brep.nonmanifold_edges(), 0, "manifold");
    assert_eq!(
        genus(brep),
        4,
        "the slot must add exactly two tunnels to the drilled device's two — one per sheet of the \
         lap, neither a dent nor a doubling"
    );
    assert!(
        clips >= 2,
        "both traced loops must reach the builder's polygon channel; {clips} clip(s) means one of \
         them never became a hole in the shell"
    );

    let shortest = measure::shortest_edge(brep);
    println!("[solid] slotted: shortest emitted edge {shortest:.3e}");
    assert!(
        shortest > measure::CAD_VERTEX_TOL,
        "the shortest emitted edge is {shortest:.3e}, under the {:.0e} vertex tolerance a CAD kernel \
         reads the shell with",
        measure::CAD_VERTEX_TOL
    );
}
