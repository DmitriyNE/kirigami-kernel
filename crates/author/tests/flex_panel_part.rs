//! The facade's acceptance suite: the Stage-1 flex panel authored as a declarative [`Part`] —
//! four solid cutters, roles derived, everything certified — validated against the legacy
//! hand-wired `outer_loop`/`hole_loop` pipeline it replaces, plus the typed-fault paths.

use author::construct;
use author::part::{Cutter, OpRole, SupportFn};
use certify_core::Verdict;
use export::approx::rat_to_f64;
use fixtures::devices::cone;
use lattice::{Bignum, Interval, Rat};

type Q = Rat<Bignum>;

fn q(n: i128, d: i128) -> Q {
    Q::new(n, d)
}
fn qi(n: i128) -> Q {
    Q::from_i128(n)
}

/// The Stage-1 demo cutter set on the `[−1, 1]` gore (the trim-test geometry): D1 the `z ≤ 3`
/// half-space bound, D2 the eccentric apex cylinder, D3 the rim notch, D4 the interior drill.
fn flex_part() -> author::Part<Bignum> {
    construct::from_chart::<Bignum>(&cone())
        .region_sigma(qi(-1), qi(1), SupportFn::inherit())
        .intersect(Cutter::half_space([qi(0), qi(0), qi(1)], qi(3)))
        .subtract(Cutter::vertical_cylinder(qi(0), q(1, 2), qi(2)))
        .subtract(Cutter::vertical_cylinder(q(-9, 4), q(9, 4), q(9, 16)))
        .subtract(Cutter::vertical_cylinder(qi(0), q(11, 5), q(1, 25)))
        .clearance(qi(1))
}

#[test]
fn the_flex_panel_develops_with_derived_roles() {
    let flat = match flex_part().develop() {
        Verdict::Verified(f) => f,
        Verdict::Refuted(f) => panic!("refuted: {f:?}"),
        Verdict::Unresolved(e) => panic!("unresolved at ε ≈ {}", rat_to_f64(&e)),
    };
    // The exact assembly reproduces the resolved topology: one face, one interior hole (D4).
    assert_eq!(flat.region().faces.len(), 1);
    assert_eq!(flat.region().faces[0].holes.len(), 1);
    // Roles are DERIVED: D1 bounds, D2 bounds the other side, D3 notches the rim, D4 holes.
    let roles: Vec<OpRole> = flat.report().ops.iter().map(|o| o.role).collect();
    assert!(matches!(roles[0], OpRole::UpperBound | OpRole::LowerBound));
    assert!(matches!(roles[1], OpRole::UpperBound | OpRole::LowerBound));
    assert!(roles[0] != roles[1], "D1 and D2 bound opposite sides");
    assert_eq!(roles[2], OpRole::Notch);
    assert_eq!(roles[3], OpRole::Hole);
    // Certified end to end under the DRC.
    assert!(flat.eps().cmp(&q(1, 2)) == core::cmp::Ordering::Less);
    // The SVG renders.
    let svg = flat.svg(400);
    assert!(svg.starts_with("<svg") && svg.len() > 200);
}

/// The facade's flat panel corroborates the legacy hand-wired pipeline: same outer area (the
/// exact shoelace over the developed outer ring) within a resolution-level tolerance.
#[test]
fn the_facade_corroborates_the_legacy_pipeline() {
    use develop::cone::{ConeDevelopment, DevConfig};
    use export::cut_oracle::RootPick;
    use export::trim::{
        RailFit, concentric_disk, eccentric_disk, flat_to_poly, outer_loop, unroll_loop,
    };

    let flat = match flex_part().segments(48).develop() {
        Verdict::Verified(f) => f,
        Verdict::Refuted(f) => panic!("facade refuted: {f:?}"),
        Verdict::Unresolved(e) => panic!("facade unresolved at ε ≈ {}", rat_to_f64(&e)),
    };
    let facade_outer = flat_to_poly(flat.outline());

    // The legacy path (the old flex_panel demo wiring, minus D4/quad).
    let chart = cone();
    let dev = ConeDevelopment::new(&chart).unwrap();
    let cfg = DevConfig::tight();
    let clearance = qi(1);
    let span = Interval {
        lo: qi(-1),
        hi: qi(1),
    };
    let d1 = concentric_disk(&chart, &qi(3)).unwrap();
    let d2 = eccentric_disk(qi(0), q(1, 2), qi(2), RootPick::Upper);
    let d3 = [q(-9, 4), q(9, 4), q(9, 16)];
    let outer = match outer_loop(
        &chart,
        &d1,
        &d2,
        (&d3[0], &d3[1], &d3[2]),
        &span,
        RailFit::default(),
        &clearance,
        &cfg,
        &q(1, 20),
        48,
    ) {
        Verdict::Verified(o) => o,
        other => panic!("legacy outer: {:?}", verdict_tag(&other)),
    };
    let legacy_flat = match unroll_loop(&dev, &outer.arcs, &cfg, &clearance) {
        Verdict::Verified(o) => o,
        other => panic!("legacy unroll: {:?}", verdict_tag(&other)),
    };
    let legacy_outer = flat_to_poly(&legacy_flat);

    let (a_facade, a_legacy) = (shoelace(&facade_outer), shoelace(&legacy_outer));
    let rel = ((a_facade - a_legacy) / a_legacy).abs();
    assert!(
        rel < 5e-3,
        "outer areas agree: facade {a_facade:.6} vs legacy {a_legacy:.6} (rel {rel:.2e})"
    );
}

#[test]
fn typed_faults_replace_the_demo_panics() {
    use author::part::PartFault;
    // No regions declared.
    let v = construct::from_chart::<Bignum>(&cone())
        .intersect(Cutter::half_space([qi(0), qi(0), qi(1)], qi(3)))
        .develop();
    assert!(matches!(v, Verdict::Refuted(PartFault::NoRegions)));
    // No bounding op — the stock discipline.
    let v = construct::from_chart::<Bignum>(&cone())
        .region_sigma(qi(-1), qi(1), SupportFn::inherit())
        .develop();
    assert!(matches!(v, Verdict::Refuted(PartFault::UnboundedRegion)));
    // A gap between region bands.
    let v = construct::from_chart::<Bignum>(&cone())
        .region_sigma(qi(-1), qi(0), SupportFn::inherit())
        .region_sigma(q(1, 4), qi(1), SupportFn::inherit())
        .intersect(Cutter::half_space([qi(0), qi(0), qi(1)], qi(3)))
        .subtract(Cutter::vertical_cylinder(qi(0), q(1, 2), qi(2)))
        .develop();
    assert!(matches!(v, Verdict::Refuted(PartFault::RegionGap(1))));
}

/// A two-region part (γ=0 body + γ≠0 smoothstep ramp on the same frame) develops through the
/// connected piecewise frame: the region joins are exact, the boundary is one certified loop.
/// (The γ≠0 side runs the verified quadrature per unroll edge — the fab-plausible budget and
/// modest segment count keep the certified suite fast; the per-edge γ recomputation is a logged
/// perf item for the optimization pass.)
#[test]
fn a_piecewise_support_part_develops() {
    use develop::cone::DevConfig;
    let flat = match construct::from_chart::<Bignum>(&cone())
        .region_sigma(q(-1, 4), q(1, 4), SupportFn::constant(qi(0)))
        .region_sigma(q(1, 4), q(3, 4), SupportFn::smoothstep(qi(0), q(1, 10)))
        .intersect(Cutter::half_space([qi(0), qi(0), qi(1)], qi(3)))
        .subtract(Cutter::vertical_cylinder(qi(0), q(1, 2), qi(2)))
        .clearance(qi(2))
        .segments(6)
        .support_panels(8)
        .fit(export::trim::RailFit {
            degree: 4,
            subdiv: 256,
            bits: 44,
        })
        .budget(DevConfig {
            terms: 14,
            sqrt_eps: q(1, 1_000_000_000),
        })
        .develop()
    {
        Verdict::Verified(f) => f,
        Verdict::Refuted(f) => panic!("refuted: {f:?}"),
        Verdict::Unresolved(e) => panic!("unresolved at ε ≈ {}", rat_to_f64(&e)),
    };
    assert_eq!(flat.region().faces.len(), 1);
    assert!(flat.report().regions.len() == 2);
    // The coarse budget certifies under the declared (unitless) fab clearance of 2.
    assert!(flat.eps().cmp(&qi(1)) == core::cmp::Ordering::Less);
}

/// Azimuth-authored regions snap to exact rational σ and echo both back.
#[test]
fn azimuth_regions_snap_and_echo() {
    let part =
        construct::cone::<Bignum>(42.0).region_azimuth(-90.0..90.0, SupportFn::constant(qi(0)));
    // Snap: tan(±45°) = ±1 exactly on the dyadic grid. (The snapped 42° cone has its own scale,
    // so the borrowed demo cutters certify under a generous unitless clearance — this test is
    // about the exact echo, not tightness.)
    let v = part
        .intersect(Cutter::half_space([qi(0), qi(0), qi(1)], qi(3)))
        .subtract(Cutter::vertical_cylinder(qi(0), q(1, 2), qi(2)))
        .clearance(qi(3))
        .segments(16)
        .develop();
    let flat = match v {
        Verdict::Verified(f) => f,
        Verdict::Refuted(f) => panic!("refuted: {f:?}"),
        Verdict::Unresolved(e) => panic!("unresolved at ε ≈ {}", rat_to_f64(&e)),
    };
    let echo = &flat.report().regions[0];
    assert_eq!(echo.requested_deg, Some((-90.0, 90.0)));
    assert_eq!(echo.band.lo, qi(-1), "tan(−45°) snapped exactly");
    assert_eq!(echo.band.hi, qi(1));
}

fn shoelace(poly: &[[Q; 2]]) -> f64 {
    let n = poly.len();
    let mut acc = 0.0f64;
    for i in 0..n {
        let (p, r) = (&poly[i], &poly[(i + 1) % n]);
        acc += rat_to_f64(&p[0]) * rat_to_f64(&r[1]) - rat_to_f64(&r[0]) * rat_to_f64(&p[1]);
    }
    (acc / 2.0).abs()
}

fn verdict_tag<T, E: core::fmt::Debug, M>(v: &Verdict<T, E, M>) -> String {
    match v {
        Verdict::Verified(_) => "Verified".into(),
        Verdict::Refuted(w) => format!("Refuted({w:?})"),
        Verdict::Unresolved(_) => "Unresolved".into(),
    }
}
