//! The facade fold surface: `Part::fold` (direction ② — flat-authored features back onto the
//! surface, the µ̂-side derived from the resolution) and `Part::hole_flat` (ECAD 2-D cutouts in
//! the exact flat boolean).

use author::construct;
use author::part::{Cutter, Part, PartFault, SupportFn};
use certify_core::Verdict;
use develop::cone::{ConeDevelopment, DevConfig};
use fixtures::devices::cone;
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

/// The doctest panel: a device-cone gore with a derived bound, an annulus carve, and a drill.
/// Its kept material is the µ̂ > 0 sheet — the side `fold` must derive on its own.
fn panel() -> Part<Bignum> {
    construct::from_chart::<Bignum>(&cone())
        .region_sigma(qi(-1), qi(1), SupportFn::inherit())
        .intersect(Cutter::half_space([qi(0), qi(0), qi(1)], qi(3)))
        .subtract(Cutter::vertical_cylinder(qi(0), q(1, 2), qi(2)))
        .subtract(Cutter::vertical_cylinder(qi(0), q(11, 5), q(1, 25)))
}

/// The exact signed flat image of a chart coordinate (the pw frame is the signed development).
fn forward(sigma: Q, mu: Q) -> [Q; 2] {
    let dev = ConeDevelopment::new(&cone()).unwrap();
    let (x, y) = dev.point_signed(&sigma, &mu, &DevConfig::tight()).center();
    [x, y]
}

/// `Part::fold` inverts flat-authored feature vertices back onto the cone: each folded 3-D box
/// recovers the exact surface point its flat vertex developed from — with the µ̂-side derived
/// from the resolution (never passed in).
#[test]
fn the_fold_evaluator_round_trips_a_flat_feature() {
    // Vertices on the kept annulus (µ̂ ∈ ≈[1.92, 2.72] at σ = 0 — between the D2 carve and the
    // z = 3 bound).
    let coords = [
        (q(-1, 4), qi(2)),
        (q(1, 4), qi(2)),
        (q(1, 4), q(9, 4)),
        (q(-1, 4), q(9, 4)),
    ];
    let feature: Vec<[Q; 2]> = coords
        .iter()
        .map(|(s, m)| forward(s.clone(), m.clone()))
        .collect();
    match panel().fold(&feature, &qi(0)) {
        Verdict::Verified(wire) => {
            assert_eq!(wire.points.len(), 4);
            for (b, (s, m)) in wire.points.iter().zip(coords.iter()) {
                let orig = cone().surface(m, &qi(0)).eval(s).unwrap();
                for i in 0..3 {
                    assert!(
                        (to_f64(&b[i].mid()) - to_f64(&orig[i])).abs() < 1e-3,
                        "folded vertex must recover the 3-D point (axis {i})"
                    );
                }
            }
        }
        Verdict::Unresolved(e) => panic!("fold unresolved at ε ≈ {}", to_f64(&e)),
        Verdict::Refuted(f) => panic!("fold refuted: {f:?}"),
    }
}

/// A feature vertex whose direction lies outside the declared gore is refused with the typed
/// fault, and an empty feature is refused as such.
#[test]
fn out_of_gore_and_empty_features_are_refused() {
    let p = panel();
    let far = forward(qi(5), q(3, 2)); // ψ(5) is beyond the σ ∈ [−1, 1] gore
    assert!(matches!(
        p.fold(&[far], &qi(0)),
        Verdict::Refuted(PartFault::OutOfGore)
    ));
    let empty: [[Q; 2]; 0] = [];
    assert!(matches!(
        p.fold(&empty, &qi(0)),
        Verdict::Refuted(PartFault::EmptyFeature)
    ));
}

/// A flat-authored hole (`hole_flat`) is cut into the exact flat boolean alongside the derived
/// drill — the topology-coherence gate counts it — and echoes back as authored.
#[test]
fn a_flat_authored_hole_is_cut_into_the_pattern() {
    // A rational diamond around the flat image of (σ = 1/2, µ̂ = 23/10) — mid-annulus, and clear
    // in flat angle of the derived drill (which develops around ψ ≈ 0: the σ = 0 ruling passes
    // its cylinder).
    let c = forward(q(1, 2), q(23, 10));
    let r = q(1, 10);
    let diamond = vec![
        [c[0].add(&r), c[1].clone()],
        [c[0].clone(), c[1].add(&r)],
        [c[0].sub(&r), c[1].clone()],
        [c[0].clone(), c[1].sub(&r)],
    ];
    let flat = match panel().hole_flat(diamond.clone()).develop() {
        Verdict::Verified(f) => f,
        Verdict::Unresolved(e) => panic!("develop unresolved at ε ≈ {}", to_f64(&e)),
        Verdict::Refuted(f) => panic!("develop refuted: {f:?}"),
    };
    // One face, two holes: the derived drill + the flat-authored diamond.
    assert_eq!(flat.region().faces.len(), 1);
    assert_eq!(flat.region().faces[0].holes.len(), 2);
    assert_eq!(flat.flat_hole_polys(), &[diamond]);
}

/// An overlapping authored hole is refused by **both** evaluators: `develop` counts it in the
/// exact flat boolean, and `solid` runs the same gate before building — an overlap XORs an
/// island face, and drilled blind it would sew a self-intersecting shell.
#[test]
fn an_overlapping_flat_hole_refuses_the_solid_too() {
    // The diamond centered on the derived drill's flat window (the σ = 0 ruling pierces the
    // drill cylinder, so its hole develops around ψ ≈ 0 — right here).
    let c = forward(qi(0), q(23, 10));
    let r = q(1, 10);
    let diamond = vec![
        [c[0].add(&r), c[1].clone()],
        [c[0].clone(), c[1].add(&r)],
        [c[0].sub(&r), c[1].clone()],
        [c[0].clone(), c[1].sub(&r)],
    ];
    let part = panel().hole_flat(diamond);
    assert!(matches!(
        part.develop(),
        Verdict::Refuted(PartFault::TopologyMismatch { .. })
    ));
    assert!(matches!(
        part.solid(),
        Verdict::Refuted(PartFault::TopologyMismatch { .. })
    ));
}

/// A degenerate authored polygon (here: empty) is refused as a typed fault on the solid path —
/// never handed to the builder's unchecked vertex indexing.
#[test]
fn a_degenerate_domain_hole_is_refused_not_built() {
    assert!(matches!(
        panel().hole_domain(vec![]).solid(),
        Verdict::Refuted(_)
    ));
}
