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
/// device's own `0`, `0 → 1/10`, `1/10` — derived here from `t`, `g` and `c` rather than written
/// down, which is the whole point.
#[test]
fn the_parametrized_recipe_is_the_hand_written_device() {
    let lap = lapped_cone(&device_spec()).expect("the device is a valid recipe");

    assert_eq!(
        lap.h_cw,
        qi(0),
        "c = t/2 + g/2 puts the CW end on the base cone"
    );
    assert_eq!(
        lap.h_ccw,
        q(1, 10),
        "…and the CCW end at the device's D = 1/10"
    );

    let bands: Vec<(Q, Q, Q)> = lap
        .regions
        .iter()
        .map(|(b, h)| (b.lo.clone(), b.hi.clone(), h.clone()))
        .collect();
    assert_eq!(
        bands,
        vec![
            (q(-5, 4), q(1, 2), qi(0)),
            (q(1, 2), qi(1), q(1, 10)),
            (qi(1), q(5, 4), q(1, 10)),
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

    let step = q(1, 40).add(&q(1, 40)); // t/2 + g/2
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
        bad(&|s| s.outer_r2 = qi(0)),
        Some(LapFault::RadiiNotAnAnnulus)
    );
    assert_eq!(
        bad(&|s| s.inner_r2 = Some(qi(100))),
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
/// The device's ramp descends inside the lap, so the gap is *not* the authored `1/20` everywhere:
/// at the tight end of the overlap the tail is still climbing. Where the tail is at `σ = 4/5` the
/// smoothstep has reached `h ≈ 0.0648`, and the head beneath it is a `1/20`-thick sheet on the base
/// cone, so the normal gap there is only `≈ 0.0148` — against `0.05` at the loose end.
///
/// `seam_clearance` proves `≥ keep_out`; it does not measure. So the useful shape is a bracket: a
/// keep-out under the true minimum certifies, one above it does not, and bisecting between them
/// tightens the number as far as a caller cares to pay for.
#[test]
fn bonded_reports_what_the_seam_actually_clears() {
    use certify_core::Verdict;

    let lap = lapped_cone(&device_spec()).expect("valid");
    let nodes = 4_000;

    // Below the true minimum: certified clear, and the witness is at least the keep-out.
    match lap.seam_clearance(&q(1, 100), nodes) {
        Verdict::Verified(c) => {
            assert!(
                c.min_dist() >= 0.01,
                "the certified bound must be at least the keep-out: {}",
                c.min_dist()
            );
            assert!(c.rails >= 1, "at least one rail pair was certified");
        }
        other => panic!(
            "the seam clears 1/100: {}",
            match other {
                Verdict::Unresolved(d2) => format!("unresolved at d² ≈ {}", rat_to_f64(&d2)),
                _ => "refuted".into(),
            }
        ),
    }

    // At the *authored* gap the ramp's intrusion shows: the sheets come closer than 1/20 somewhere
    // in the overlap, so the same certificate cannot establish it. That is the feedback — the
    // nominal gap is not what the seam achieves once a ramp runs into it.
    assert!(
        !matches!(lap.seam_clearance(&q(1, 20), nodes), Verdict::Verified(_)),
        "the authored gap 1/20 is NOT cleared everywhere — the ramp descends inside the lap"
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
