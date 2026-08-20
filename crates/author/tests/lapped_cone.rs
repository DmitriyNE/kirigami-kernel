//! **The lapped cone as a validated parameter set (LAP).**
//!
//! The self-lapping device used to be a hand-written recipe; it is now one point in a parameter
//! space. These tests hold that space to two things it would be easy to lose:
//!
//! 1. **The re-expression is exact.** The parametrized recipe must produce the *same* σ bands, the
//!    same supports and the same surface as the device did when it was written by hand — otherwise
//!    the VV.1 work budgets, VV.2 ε bounds and VV.3 chord goldens are all quietly guarding new
//!    geometry, which is worse than guarding nothing.
//! 2. **The validation is exact and non-vacuous.** Every precondition is a sign or an ordering over
//!    ℚ — no `arctan`, no tolerance — and each refusal is provoked by a recipe that differs from a
//!    good one in exactly the way the fault names.

use acceptance::lapped::{
    Azimuth, GapPolicy, LapFault, LappedCone, OnTop, SideAngles, lapped_cone,
};
use export::approx::{rat_to_f64, surd_to_f64};
use lattice::{Bignum, Rat};

type Q = Rat<Bignum>;

fn q(n: i128, d: i128) -> Q {
    Q::new(n, d)
}
fn qi(n: i128) -> Q {
    Q::from_i128(n)
}
fn sigma(n: i128, d: i128) -> Azimuth {
    Azimuth::Sigma(q(n, d))
}

/// Today's acceptance device, as parameters — **the real one**, not a copy of it. A test that
/// restated the numbers would pass while the fixture drifted, which is exactly the failure the
/// `acceptance` crate exists to prevent.
fn device_spec() -> LappedCone {
    acceptance::self_lapping_spec()
}

/// **The re-expression reproduces the hand-written device, band for band.**
///
/// Three regions and not five: the CW end's offset is exactly zero, so it has neither a ramp nor a
/// plateau distinct from the base, and its two empty bands never appear. The supports are the
/// device's own `0`, `0 → Δ`, `Δ` — derived here from `t`, `g` and `c` rather than written down,
/// which is the whole point, so the ramp's height is `t + g` and nothing else.
#[test]
fn the_parametrized_recipe_is_the_hand_written_device() {
    let spec = device_spec();
    let lap = lapped_cone(&spec).expect("the device is a valid recipe");

    assert_eq!(
        lap.h_cw,
        qi(0),
        "c = t/2 + g/2 puts the CW end on the base cone"
    );
    let step = spec.thickness.add(&spec.gap);
    assert_eq!(
        lap.h_ccw, step,
        "…and the CCW end at the device's ramp step Δ = t + g"
    );

    let bands: Vec<(Q, Q, Q)> = lap
        .regions
        .iter()
        .map(|(b, h)| (b.lo.clone(), b.hi.clone(), h.clone()))
        .collect();
    assert_eq!(
        bands,
        vec![
            (q(-5, 4), q(4, 7), qi(0)),
            (q(4, 7), qi(1), step.clone()),
            (qi(1), q(5, 4), step.clone()),
        ],
        "the device's own three bands"
    );

    // The surface is the device's surface. The generator comes out `(234, 104) = 26·(9, 4)`, and
    // the Hopf map is invariant under `q ↦ λq`, so the scale is gauge: every derived field matches.
    let dev = fixtures::devices::cone_wrap();
    assert_eq!(lap.chart.normal(), dev.normal());
    assert_eq!(lap.chart.ruling(), dev.ruling());
    assert_eq!(lap.chart.pedal(), dev.pedal());
}

/// **The lap is a sign over ℚ, and the overlap windows are the Möbius partner.**
///
/// `φ = 4·arctan σ`, so two azimuths differ by exactly `2π` iff `1 + σ₁σ₀ = 0` — the shift is
/// `σ ↦ −1/σ`. The device's ends are `±5/4`, so the windows are `[4/5, 5/4]` and `[−5/4, −4/5]`,
/// and `1 + (5/4)(−5/4) = −9/16 < 0` is the whole "does it lap" test.
#[test]
fn the_overlap_windows_are_exact_mobius_partners() {
    let lap = lapped_cone(&device_spec()).expect("valid");
    assert_eq!(
        (lap.lap_ccw.lo.clone(), lap.lap_ccw.hi.clone()),
        (q(4, 5), q(5, 4))
    );
    assert_eq!(
        (lap.lap_cw.lo.clone(), lap.lap_cw.hi.clone()),
        (q(-5, 4), q(-4, 5))
    );

    // Non-vacuous: pull both ends in until they no longer wrap past a full turn, and the recipe is
    // refused by name. `1 + (4/5)(−4/5) = 9/25 > 0`.
    let mut spec = device_spec();
    spec.ccw = SideAngles {
        ramp_start: sigma(1, 4),
        ramp_end: sigma(1, 2),
        sheet_end: sigma(4, 5),
    };
    spec.cw = SideAngles::flat(sigma(-4, 5));
    assert_eq!(lapped_cone(&spec).err(), Some(LapFault::NoLap));
}

/// **A zero offset means both ends ramp, symmetrically.** The two-ramp seam is `c = 0`: the seam
/// straddles the base cone and each end steps off by `t/2 + g/2` in opposite directions.
#[test]
fn a_centred_seam_gives_a_ramp_on_each_side() {
    let mut spec = device_spec();
    spec.seam_offset = qi(0);
    spec.cw = SideAngles {
        ramp_start: sigma(-1, 2),
        ramp_end: sigma(-1, 1),
        sheet_end: sigma(-5, 4),
    };
    let lap = lapped_cone(&spec).expect("a symmetric two-ramp seam");

    let step = spec.thickness.add(&spec.gap).div(&qi(2)); // t/2 + g/2
    assert_eq!(lap.h_ccw, step);
    assert_eq!(
        lap.h_cw,
        step.neg(),
        "the two ends step off in opposite directions"
    );
    assert_eq!(lap.regions.len(), 5, "plateau, ramp, base, ramp, plateau");
    assert_eq!(
        lap.h_ccw.sub(&lap.h_cw),
        spec.thickness.add(&spec.gap),
        "facing faces stay `g` apart: the offsets differ by exactly t + g"
    );
}

/// **The gap policy is the caller's, and the strict one really does refuse today's device.**
///
/// The acceptance part lets its ramp descend inside the lap — the ramp ends at `σ = 1`, while the
/// overlap starts at `σ = 4/5` — so the gap closes over part of the seam. Under
/// [`GapPolicy::Constant`] that is a refusal by name; under `MinDistance` it is allowed and BONDED
/// reports what the gap actually reaches.
#[test]
fn the_constant_gap_policy_refuses_a_ramp_that_runs_into_the_lap() {
    let mut spec = device_spec();
    spec.policy = GapPolicy::Constant;
    assert_eq!(
        lapped_cone(&spec).err(),
        Some(LapFault::RampInsideLap(OnTop::Ccw)),
        "ramp_end = 1 is past the overlap start 4/5"
    );

    // Pull the ramp back so it finishes before the overlap and the same recipe is accepted.
    spec.ccw.ramp_end = sigma(3, 4);
    assert!(
        lapped_cone(&spec).is_ok(),
        "a ramp that clears the lap is constant-gap"
    );
}

/// **Each refusal is provoked by exactly its own defect.** A validation suite that only ever sees
/// good input proves nothing, and one whose bad inputs are bad in several ways at once cannot say
/// which check fired.
#[test]
fn every_precondition_refuses_by_name() {
    let bad = |f: &dyn Fn(&mut LappedCone)| {
        let mut spec = device_spec();
        f(&mut spec);
        lapped_cone(&spec).err()
    };

    assert_eq!(
        bad(&|s| s.thickness = qi(0)),
        Some(LapFault::ThicknessNotPositive)
    );
    assert_eq!(bad(&|s| s.gap = qi(-1)), Some(LapFault::GapNegative));
    assert_eq!(
        bad(&|s| s.outer_r = qi(0)),
        Some(LapFault::RadiiNotAnAnnulus)
    );
    assert_eq!(
        bad(&|s| s.inner_r = Some(qi(100))),
        Some(LapFault::RadiiNotAnAnnulus),
        "an inner radius outside the outer one is not an annulus"
    );
    assert_eq!(
        bad(&|s| s.apex = (qi(1), qi(0))),
        Some(LapFault::ApexNotACone),
        "a zero half-angle is a cylinder, not a cone"
    );
    assert_eq!(
        bad(&|s| s.ccw.ramp_end = sigma(1, 4)),
        Some(LapFault::AngleOrder(OnTop::Ccw)),
        "ramp_end before ramp_start does not run outward"
    );
    assert_eq!(
        bad(&|s| {
            s.cw = SideAngles::flat(sigma(3, 4));
            s.ccw.ramp_start = sigma(1, 2);
        }),
        Some(LapFault::SidesCross),
        "the CW base past the CCW base leaves no base band"
    );
    // A zero-offset side handed a real ramp — a ramp from the base cone to the base cone.
    assert_eq!(
        bad(&|s| {
            s.cw = SideAngles {
                ramp_start: sigma(-1, 2),
                ramp_end: sigma(-1, 1),
                sheet_end: sigma(-5, 4),
            }
        }),
        Some(LapFault::RampOffsetMismatch(OnTop::Cw)),
        "the CW end's offset is zero here, so it has no ramp to place"
    );
    // …and the other direction: a real offset handed a zero-width ramp, which is a support *step*.
    assert_eq!(
        bad(&|s| {
            s.seam_offset = qi(0);
            s.cw = SideAngles::flat(sigma(-5, 4));
        }),
        Some(LapFault::RampOffsetMismatch(OnTop::Cw)),
        "a nonzero offset with no ramp width is a discontinuous surface"
    );
}

/// **What BONDED says the seam actually clears.**
///
/// The device's ramp descends inside the lap, so the gap is *not* the authored `g` everywhere: at
/// the tight end of the overlap the tail is still climbing, while the head beneath it is a
/// `t`-thick sheet on the base cone.
///
/// `seam_clearance` proves `≥ keep_out`; it does not measure. So the useful shape is a bracket: a
/// keep-out under the true minimum certifies, one above it does not, and bisecting between them
/// tightens the number as far as a caller cares to pay for. Both ends are taken from the spec, so
/// the bracket follows the device instead of pinning a stale pair of constants.
#[test]
fn bonded_reports_what_the_seam_actually_clears() {
    use certify_core::Verdict;

    let spec = device_spec();
    let lap = lapped_cone(&spec).expect("valid");
    let nodes = 4_000;

    // Below the true minimum: certified clear, and the witness is at least the keep-out.
    let under = spec.gap.div(&qi(2));
    match lap.seam_clearance(&under, nodes) {
        Verdict::Verified(c) => {
            assert!(
                c.min_dist() >= rat_to_f64(&under),
                "the certified bound must be at least the keep-out: {}",
                c.min_dist()
            );
            assert!(c.rails >= 1, "at least one rail pair was certified");
        }
        other => panic!(
            "the seam clears g/2: {}",
            match other {
                Verdict::Unresolved(d2) => format!("unresolved at d² ≈ {}", rat_to_f64(&d2)),
                _ => "refuted".into(),
            }
        ),
    }

    // At the *authored* gap the ramp's intrusion shows: the sheets come closer than `g` somewhere
    // in the overlap, so the same certificate cannot establish it. That is the feedback — the
    // nominal gap is not what the seam achieves once a ramp runs into it.
    assert!(
        !matches!(lap.seam_clearance(&spec.gap, nodes), Verdict::Verified(_)),
        "the authored gap g is NOT cleared everywhere — the ramp descends inside the lap"
    );
}

/// **Where the stack sits relative to the developed surface, and why the default is the middle.**
///
/// The chart surface is what `develop` unrolls *isometrically*, so it is the surface the flat
/// pattern is true for — which for a bent laminate is its bending-neutral axis, mid-stack. The
/// default `neutral = 1/2` puts it there; `0` and `1` put it on a face, and the emitted solid moves
/// by half a thickness along the normal between them, which is the whole content of the knob.
#[test]
fn the_stack_straddles_the_developed_surface_by_default() {
    use author::part::{Part, PartFault};
    use certify_core::Verdict;

    // The acceptance gore, which is a known-good solid — only the neutral knob varies.
    let build = |f: Option<Q>| -> Part<Bignum> {
        let p = acceptance::sketch_panel(None);
        match f {
            Some(f) => p.neutral(f),
            None => p,
        }
    };
    let span = |part: &Part<Bignum>| -> (f64, f64) {
        let Verdict::Verified(s) = part.solid() else {
            panic!("the annulus is a solid")
        };
        s.brep()
            .verts()
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
                let z = surd_to_f64(&v[2]);
                (lo.min(z), hi.max(z))
            })
    };

    let (mid_lo, mid_hi) = span(&build(None)); // the default
    let (face_lo, face_hi) = span(&build(Some(qi(0))));
    let t = 0.125;
    // `neutral = 0` puts the whole stack on the `+n` side, so the solid rides half a thickness
    // higher than the centred one — measured along z, that is `(t/2)·(n·ẑ)`.
    let nz = rat_to_f64(
        &fixtures::devices::cone()
            .normal()
            .comp(2)
            .eval(&qi(0))
            .expect("regular at σ = 0"),
    );
    let want = t / 2.0 * nz;
    assert!(
        (face_lo - mid_lo - want).abs() < 1e-9 && (face_hi - mid_hi - want).abs() < 1e-9,
        "neutral 0 must lift the solid by exactly (t/2)·(n·ẑ) = {want:.6}: \
         mid [{mid_lo:.6}, {mid_hi:.6}] vs face [{face_lo:.6}, {face_hi:.6}]"
    );
    // …and the stack is the same thickness either way — the knob moves it, it does not stretch it.
    assert!(
        ((mid_hi - mid_lo) - (face_hi - face_lo)).abs() < 1e-9,
        "the thickness is unchanged by where the neutral surface sits"
    );

    // Outside [0, 1] the developed surface leaves the material, and that is refused by name rather
    // than silently developing a surface the part does not have.
    assert!(matches!(
        build(Some(q(3, 2))).solid(),
        Verdict::Refuted(PartFault::NeutralOutsideStack)
    ));
}

/// **An even ramp buys ramp angle, and that is the whole point of the knob.**
///
/// The support enters the geometry through `R₁ + w = det J / |n′|²`, so material at `µ̂` has
/// `R₁ ∝ (µ̂ − µ̂_fold)` and its bending strain goes as `w/(µ̂ − µ̂_fold)`. Peak strain and the
/// largest usable ramp angle are therefore *one* quantity: how far the fold line swings while the
/// support climbs. The cubic `3u² − 2u³` spends that swing badly — `h″` is linear, so the bend
/// piles up at the two joins and the middle of the ramp does no work — while two parabolic halves
/// hold `|h″|` constant, the smallest peak available at `h′ = 0` on both ends.
///
/// Measured on the acceptance ramp, peak `|µ̂_fold|` is 2.474 against 1.641 — **1.507×**, versus
/// the 1.500 the two peaks predict. This test pins the *consequence* rather than that ratio, which
/// no public API exposes: at `Δσ = 3/16` the cubic's ε has already run past the part's DRC gate —
/// 7.4e0 against a gate of 3.5 — while the even profile still certifies at 1.8e0, on a recipe
/// differing in nothing else.
///
/// The width at which the two part ways is a property of the *device*, not of the profiles: it is
/// where the fold line's swing starts to reach material, so it moves whenever the annulus does.
/// (It was `Δσ = 11/32` on the Ø 43 device; the correction to Ø 21.5 narrowed the kept band and
/// took it to `3/16`. One step coarser, at `Δσ = 1/4`, the cubic fails by breaking the region into
/// three faces instead of by ε — the same fold line, reported by topology; see #287.)
#[test]
fn an_even_ramp_certifies_a_ramp_the_cubic_cannot() {
    use acceptance::RampProfile;
    use certify_core::Verdict;

    let narrowed = |p: RampProfile| {
        let mut spec = device_spec();
        spec.ramp_profile = p;
        spec.ccw.ramp_start = sigma(13, 16); // Δσ = 3/16, where the two part ways
        acceptance::self_lapping_cone_from(&spec, 16, 8, false, None)
    };

    assert!(
        !matches!(
            narrowed(RampProfile::Smoothstep).develop(),
            Verdict::Verified(_)
        ),
        "the cubic cannot hold this ramp — that is the limit the even profile lifts"
    );
    let even = narrowed(RampProfile::EvenCurvature);
    // Half the part's own DRC keep-out, read from the part rather than restated.
    let gate = even.drc_clearance().div(&qi(2));
    let Verdict::Verified(flat) = even.develop() else {
        panic!("the even ramp certifies the same seam at the same width");
    };
    assert!(
        flat.eps().cmp(&gate) == core::cmp::Ordering::Less,
        "…and under the device's own DRC gate {:.3e}, not merely somewhere: ε {:.3e}",
        rat_to_f64(&gate),
        rat_to_f64(flat.eps())
    );
}

/// **The profile changes how the ramp climbs, not what the seam is.**
///
/// Same supports, same lap windows, same azimuths — only the band count differs, because
/// `EvenCurvature` needs two of them per ramp to hold `|h″|` constant.
#[test]
fn the_ramp_profile_leaves_the_seam_alone() {
    use acceptance::RampProfile;

    let build = |p: RampProfile| {
        let mut spec = device_spec();
        spec.ramp_profile = p;
        lapped_cone(&spec).expect("valid either way")
    };
    let (cubic, even) = (
        build(RampProfile::Smoothstep),
        build(RampProfile::EvenCurvature),
    );

    assert_eq!(
        cubic.h_ccw, even.h_ccw,
        "the sheet offsets are the recipe's"
    );
    assert_eq!(cubic.h_cw, even.h_cw);
    assert_eq!(cubic.lap_ccw.lo, even.lap_ccw.lo, "and so is the lap");
    assert_eq!(cubic.lap_ccw.hi, even.lap_ccw.hi);
    assert_eq!(
        (cubic.regions.len(), even.regions.len()),
        (3, 4),
        "one ramp, split in two: three bands become four"
    );
    // The ramp's two halves meet at its midpoint and cover exactly the band the cubic spanned.
    let cubic_ramp = &cubic.regions[1].0;
    assert_eq!(even.regions[1].0.lo, cubic_ramp.lo);
    assert_eq!(even.regions[2].0.hi, cubic_ramp.hi);
    assert_eq!(
        even.regions[1].0.hi,
        cubic_ramp.lo.add(&cubic_ramp.hi).div(&qi(2)),
        "split at the midpoint, which is where |h″| flips sign"
    );
    assert_eq!(even.regions[1].0.hi, even.regions[2].0.lo, "and they join");
}

/// **The two-ramp device develops, and its two gauge circles stay concentric (#302).**
///
/// This is the configuration that carries *both* cut files — `acceptance::two_ramp_spec`, the
/// device `examples/lapped_cone.rs` emits — and until this test existed nothing in the suite ever
/// called `develop()` on it. The recipe tests above certify the *parameters*; the geometry went
/// uncovered, and a boundary the rail oracle could not fit sat there refusing while everything
/// stayed green.
///
/// **Why the second assertion, and not just `Verified`.** A distance certificate bounds how far the
/// emitted polyline is from the true development; it says nothing about *which* curve that is. The
/// bug it missed was a rail clamped into a stretch of σ where its wall had no real intersection at
/// all — a wrong curve, not a loose one. So the check here is shape-valued: both drawings' main
/// arcs (`inner-cut.dxf`'s Ø 8 bore and `outer-cut.dxf`'s Ø 21.5 rim) are **coaxial with the cone
/// axis**, so each develops to a circular arc about the *developed apex* — one point, shared. Two
/// arcs at different radii sharing a centre is a fact about the part's shape that no ε can fake,
/// and it is exactly what a rail off its own wall would break.
///
/// The radius ratio is left loose (5%) on purpose: the trim is a `NormalCut`, cast from a drafted
/// apex, so the map from sketch radius to developed radius is projective rather than linear and
/// the two circles are displaced by different amounts. That displacement is the tool, not an error
/// (`docs/engineering-log.md`, the cut-file ruling) — what must hold is the concentricity.
#[test]
fn the_two_ramp_device_develops_and_its_gauge_circles_stay_concentric() {
    use certify_core::Verdict;

    let part = acceptance::self_lapping_cone_from(&acceptance::two_ramp_spec(), 8, 1, false, None);
    let flat = match part.develop() {
        Verdict::Verified(f) => f,
        Verdict::Unresolved(e) => panic!(
            "the two-ramp device must develop; Unresolved at ε {:.3e}",
            rat_to_f64(&e)
        ),
        Verdict::Refuted(f) => panic!("the two-ramp device must develop; Refuted({f:?})"),
    };
    let eps = rat_to_f64(flat.eps());
    assert!(
        eps < 3.5,
        "the DRC bar is clearance/2 = 3.5; got ε {eps:.4}"
    );

    let pts: Vec<(f64, f64)> = flat
        .outline()
        .vertices
        .iter()
        .map(|v| {
            let (x, y) = v.center();
            (rat_to_f64(&x), rat_to_f64(&y))
        })
        .collect();

    // The two longest stretches of outline that lie on *some* circle, by **arc length**. Each
    // drawing's main arc is one of them; nothing else on this outline is circular for anything like
    // as far (the flanks are straight, the fillets and caps are R 0.3). Ranking by *point count*
    // instead looks equivalent and is not — raise `--segments` and a R 0.39 cap carries more
    // vertices than the Ø 21.5 rim, and the check silently starts measuring the wrong two curves.
    let runs = circular_runs(&pts, 1e-2);
    assert!(
        runs.len() >= 2,
        "the bore and the rim must each develop to a circular run; found {}",
        runs.len()
    );
    let (bore, rim) = if runs[0].2 < runs[1].2 {
        (&runs[0], &runs[1])
    } else {
        (&runs[1], &runs[0])
    };
    // The bar is the *shape* claim, not the fit's own precision: 5e-2 flat units is ~70× tighter
    // than the ε this development certifies, so a rail wandering off its wall cannot slip under it,
    // while the greedy run-grower's slack (it stops at `tol`, not at the tightest sub-run) does.
    let apex_gap = (bore.0 - rim.0).hypot(bore.1 - rim.1);
    assert!(
        apex_gap < 5e-2,
        "two coaxial drawing circles must develop about ONE centre — the developed apex; \
         the fitted centres are {apex_gap:.2e} apart (bore R {:.4}, rim R {:.4})",
        bore.2,
        rim.2
    );
    let ratio = rim.2 / bore.2;
    let drawn = 10.75 / 4.0;
    assert!(
        (ratio - drawn).abs() < 0.05 * drawn,
        "the developed radii must keep the drawing's Ø 21.5 : Ø 8 proportion up to the normal-cut \
         cast; got {ratio:.4} against {drawn:.4}"
    );
}

/// The maximal runs of consecutive points that fit a circle to within `tol`, as
/// `(centre_x, centre_y, radius)` — the two of **greatest arc length**.
///
/// Greedy and deliberately simple: grow a run while the algebraic fit still holds, then start the
/// next one after it.
///
/// Arc length, not point count, is what makes the ranking mean "the two big arcs". The outline is
/// sampled per σ-station, so raising `--segments` multiplies every feature's vertices alike and a
/// R 0.39 cap overtakes the Ø 21.5 rim on count while remaining two orders shorter — the check
/// would then compare two fillets and call them the gauge circles.
fn circular_runs(pts: &[(f64, f64)], tol: f64) -> Vec<(f64, f64, f64)> {
    const MIN: usize = 12;
    let mut runs: Vec<(f64, (f64, f64, f64))> = Vec::new();
    let mut i = 0;
    while i + MIN <= pts.len() {
        let mut j = i + MIN;
        let mut last = match fit_circle(&pts[i..j]) {
            Some(c) if c.3 < tol => Some(c),
            _ => None,
        };
        if last.is_some() {
            while j < pts.len() {
                match fit_circle(&pts[i..j + 1]) {
                    Some(c) if c.3 < tol => {
                        last = Some(c);
                        j += 1;
                    }
                    _ => break,
                }
            }
            let c = last.expect("the seed fit held");
            let arc = pts[i..j]
                .windows(2)
                .map(|w| (w[0].0 - w[1].0).hypot(w[0].1 - w[1].1))
                .sum();
            runs.push((arc, (c.0, c.1, c.2)));
            i = j;
        } else {
            i += 1;
        }
    }
    runs.sort_by(|a, b| b.0.total_cmp(&a.0));
    runs.into_iter().take(2).map(|(_, c)| c).collect()
}

/// Kåsa's algebraic circle fit: `(cx, cy, r, max |‖p − c‖ − r|)`, or `None` where the points are
/// collinear enough that the normal equations are singular.
fn fit_circle(p: &[(f64, f64)]) -> Option<(f64, f64, f64, f64)> {
    let n = p.len() as f64;
    let (mut sx, mut sy, mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0, 0.0, 0.0);
    let (mut sxxx, mut syyy, mut sxyy, mut sxxy) = (0.0, 0.0, 0.0, 0.0);
    for &(x, y) in p {
        sx += x;
        sy += y;
        sxx += x * x;
        syy += y * y;
        sxy += x * y;
        sxxx += x * x * x;
        syyy += y * y * y;
        sxyy += x * y * y;
        sxxy += x * x * y;
    }
    let c = n * sxx - sx * sx;
    let d = n * sxy - sx * sy;
    let e = n * sxxx + n * sxyy - (sxx + syy) * sx;
    let g = n * syy - sy * sy;
    let h = n * sxxy + n * syyy - (sxx + syy) * sy;
    let den = 2.0 * (c * g - d * d);
    if den.abs() < 1e-12 {
        return None;
    }
    let (a, b) = ((e * g - h * d) / den, (c * h - d * e) / den);
    let r = p.iter().map(|&(x, y)| (x - a).hypot(y - b)).sum::<f64>() / n;
    let res = p
        .iter()
        .map(|&(x, y)| ((x - a).hypot(y - b) - r).abs())
        .fold(0.0, f64::max);
    Some((a, b, r, res))
}
