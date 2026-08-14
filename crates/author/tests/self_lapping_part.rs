//! The **self-lapping cone on the facade** — the construction-API phase's acceptance geometry:
//! the wrapping chart (`ψ = (260/97)·arctan σ` — more than one turn in a finite window), three
//! piecewise-support regions (body / smoothstep ramp / offset tail), solid cutters with derived
//! roles, the **per-window seam drill** (one cylinder, two derived holes — head and tail flap),
//! a flat-authored hexagon, and the certified fold closing the round trip.

use author::construct;
use author::part::{Cutter, OpRole, Part, SupportFn};
use certify_core::Verdict;
use export::approx::rat_to_f64;
use export::trim::RailFit;
use fixtures::devices::cone_wrap;
use lattice::{Bignum, Rat};

type Q = Rat<Bignum>;

fn q(n: i128, d: i128) -> Q {
    Q::new(n, d)
}
fn qi(n: i128) -> Q {
    Q::from_i128(n)
}

/// The device: `D = 1/10` lap offset, regions body `[−5/4, 1/2]` (`h ≡ 0`), ramp `[1/2, 1]`
/// (smoothstep `0 → D`), tail `[1, 5/4]` (`h ≡ D`); concentric outer D1 (intersect), eccentric
/// apex-containing inner D2 (subtract), and the seam drill over the lap (subtract — pierces the
/// head at σ ≈ −0.9 and the tail flap at σ ≈ 1.1: one cutter, two derived holes).
fn device(with_drill: bool) -> Part<Bignum> {
    let d = q(1, 10);
    // A witness on the kept sheet: the σ = 0 ruling's point at z = −3 (mid-annulus). The wrap
    // chart keeps material on both sheets of the double cover (the antipodal ray crosses the
    // disks too), so the recipe must designate the component — exactly the PR 2 finding.
    let rz0 = cone_wrap().ruling().comp(2).eval(&qi(0)).unwrap();
    let mu_w = q(-3, 1).div(&rz0);
    let witness = cone_wrap().surface(&mu_w, &qi(0)).eval(&qi(0)).unwrap();
    let mut part = construct::from_chart::<Bignum>(&cone_wrap())
        .region_sigma(q(-5, 4), q(1, 2), SupportFn::constant(qi(0)))
        .region_sigma(q(1, 2), qi(1), SupportFn::smoothstep(qi(0), d.clone()))
        .region_sigma(qi(1), q(5, 4), SupportFn::constant(d))
        .keep_near(witness)
        .intersect(Cutter::vertical_cylinder(qi(0), qi(0), q(471, 50)))
        .subtract(Cutter::vertical_cylinder(qi(0), q(1, 2), qi(4)))
        .clearance(qi(1))
        .thickness(q(1, 20))
        .fit(RailFit {
            degree: 4,
            subdiv: 160,
            bits: 44,
        })
        .segments(16)
        .support_panels(8)
        .budget(develop::cone::DevConfig {
            terms: 14,
            sqrt_eps: q(1, 1_000_000_000),
        });
    if with_drill {
        part = part.subtract(Cutter::vertical_cylinder(q(-1, 2), q(27, 10), q(1, 40)));
    }
    part
}

/// The flat sector sweeps more than one full 3-D turn (2π·sinβ ≈ 240.9°) yet stays under 360° —
/// the excess IS the lap, on a still-cuttable single sheet.
#[test]
fn the_flat_sector_exceeds_one_turn_by_the_lap() {
    let c = 260.0 / 97.0;
    let s = c * (2.0 * (1.25f64).atan()) * 180.0 / std::f64::consts::PI;
    assert!(
        s > 240.9 && s < 360.0,
        "sector {s:.1}° must lap (> 240.9°) yet stay < 360°"
    );
}

/// The facade derives the whole wrap structure: D1/D2 bound, the one seam drill yields **two**
/// holes (one per disc-positive window — head and tail flap), and the exact boolean reproduces
/// it (one face, two holes).
#[test]
fn the_seam_drill_derives_two_holes_across_the_lap() {
    let flat = match device(true).develop() {
        Verdict::Verified(f) => f,
        Verdict::Unresolved(e) => panic!("develop unresolved at ε ≈ {:.3e}", rat_to_f64(&e)),
        Verdict::Refuted(f) => panic!("develop refuted: {f:?}"),
    };
    let roles: Vec<OpRole> = flat.report().ops.iter().map(|o| o.role).collect();
    assert_eq!(
        roles,
        vec![OpRole::LowerBound, OpRole::UpperBound, OpRole::Hole],
        "derived roles: D1 bounds below, D2 above, the drill holes"
    );
    assert_eq!(flat.region().faces.len(), 1);
    assert_eq!(
        flat.region().faces[0].holes.len(),
        2,
        "ONE drill cutter, TWO derived holes: the head and the lapping tail flap"
    );
}

/// The certified round trip: outline vertices developed by `develop()` fold back through
/// `fold()` onto the 3-D cone — flat chords match 3-D chords (the isometry corroboration, now
/// through the certified piecewise fold rather than a hand-built oracle).
#[test]
fn the_flat_pattern_folds_back_isometrically() {
    let part = device(false);
    let flat = match part.develop() {
        Verdict::Verified(f) => f,
        Verdict::Unresolved(e) => panic!("develop unresolved at ε ≈ {:.3e}", rat_to_f64(&e)),
        Verdict::Refuted(f) => panic!("develop refuted: {f:?}"),
    };
    let verts = &flat.outline().vertices;
    let n = verts.len();
    // Consecutive-vertex pairs spread across the loop: the body far end, mid-body, and the
    // second rail (which walks the tail/ramp bands, exercising the γ ≠ 0 fold).
    let picks = [1usize, n / 4, n / 2 + 1, (3 * n) / 4];
    let mut worst = 0.0f64;
    for &i in &picks {
        let pair: Vec<[Q; 2]> = (i..=i + 1)
            .map(|k| {
                let (x, y) = verts[k % n].center();
                [x, y]
            })
            .collect();
        let flat_chord = {
            let (dx, dy) = (
                rat_to_f64(&pair[1][0]) - rat_to_f64(&pair[0][0]),
                rat_to_f64(&pair[1][1]) - rat_to_f64(&pair[0][1]),
            );
            (dx * dx + dy * dy).sqrt()
        };
        let wire = match part.fold(&pair, &qi(0)) {
            Verdict::Verified(w) => w,
            Verdict::Unresolved(e) => panic!("fold unresolved at ε ≈ {:.3e}", rat_to_f64(&e)),
            Verdict::Refuted(f) => panic!("fold refuted at vertex {i}: {f:?}"),
        };
        let d3 = {
            let a = &wire.points[0];
            let b = &wire.points[1];
            let mut acc = 0.0;
            for j in 0..3 {
                let d = rat_to_f64(&a[j].mid()) - rat_to_f64(&b[j].mid());
                acc += d * d;
            }
            acc.sqrt()
        };
        // The chord defect must sit inside the fold's OWN certified round-trip bound (plus the
        // outline vertices' enclosure slop) — the oracle is tied to the certificate, not to a
        // magic constant. At this lean budget the γ-quadrature dominates the tail pairs' ε.
        let defect = (flat_chord - d3).abs();
        assert!(
            defect < rat_to_f64(&wire.eps) + 1e-3,
            "pair {i}: chord defect {defect:.3e} exceeds the certified fold ε {:.3e}",
            rat_to_f64(&wire.eps)
        );
        worst = worst.max(defect);
    }
    assert!(
        worst < 1e-1,
        "gross isometry breakage: chord defect {worst:.3e}"
    );
}
