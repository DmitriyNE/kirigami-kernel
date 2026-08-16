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

/// A derived hole **spanning σ-stations** still builds a solid. Interior cuts are p-curve loops
/// that pass through their tangent rulings, so a hole is no longer two fitted graphs; the builder
/// consumes it as a near/far band of contiguous rail *chains*, and each chain's piece boundaries
/// join the station partition. This is the case that a single-slice polygon cut cannot express —
/// the device's holes straddle stations — and it is why the band survived the p-curve rewrite
/// rather than being replaced by polygons.
#[test]
fn a_derived_hole_spanning_stations_still_builds_a_solid() {
    match panel().solid() {
        Verdict::Verified(_) => {}
        Verdict::Refuted(f) => panic!("the hole band must still sew a solid, got {f:?}"),
        Verdict::Unresolved(e) => panic!("expected Verified, got Unresolved({e:?})"),
    }
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

/// A verdict's name, for panic messages (the payloads are not `Debug`).
fn verdict_name<E, W: core::fmt::Debug, M: core::fmt::Debug>(v: &Verdict<E, W, M>) -> String {
    match v {
        Verdict::Verified(_) => "Verified".to_string(),
        Verdict::Unresolved(m) => format!("Unresolved({m:?})"),
        Verdict::Refuted(f) => format!("Refuted({f:?})"),
    }
}

/// An L-shaped — deliberately **non-convex** — `(σ, µ̂)` loop, CCW, as a domain hole.
fn l_slot(s0: Q, s1: Q, m0: Q, m1: Q) -> Vec<(Q, Q)> {
    let smid = s0.add(&s1).mul(&q(1, 2));
    let mmid = m0.add(&m1).mul(&q(1, 2));
    vec![
        (s0.clone(), m0.clone()),
        (s1.clone(), m0),
        (s1, mmid.clone()),
        (smid.clone(), mmid),
        (smid, m1.clone()),
        (s0, m1),
    ]
}

/// **AUTH.2 pre-state, pinned.** A non-convex loop is *already* general on both paths, so long as it
/// stays inside one σ-station slice: it develops through the exact flat boolean and it builds a
/// certified solid through the authored-polygon channel (a lid inner wire plus a wall per edge).
///
/// This is the measurement that sized AUTH.2 (`docs/cutter-extrude-design.md` §11.1): the band in
/// `HoleRail` is *not* what stands between the kernel and non-convex profiles. What refuses today is
/// the loop **tracer** — `develop::cut` cannot yet read a footprint the ruling meets twice — so the
/// only way to hand the downstream a non-convex loop is to author it, as here. Must stay green
/// through the milestone: it is the leg AUTH.2 is *not* allowed to regress.
#[test]
fn a_non_convex_domain_loop_inside_one_slice_already_builds_a_solid() {
    // σ ∈ [0.3, 0.45] is clear of the derived drill (which sits at σ ≈ 0), and µ̂ ∈ [2.2, 2.6] is
    // strictly inside the kept band there — an authored hole that touches either boundary is
    // dropped by the flat difference and reads as a topology mismatch, not as this claim failing.
    let part = panel().hole_domain(l_slot(q(3, 10), q(45, 100), q(11, 5), q(13, 5)));
    let flat = match part.develop() {
        Verdict::Verified(f) => f,
        Verdict::Refuted(f) => panic!("the flat path takes a non-convex loop today, got {f:?}"),
        Verdict::Unresolved(e) => panic!("expected Verified, got Unresolved({e:?})"),
    };
    // `Verified` alone would pass just as happily on a hole that had been silently convexified, so
    // check the reflex corner survived into the emitted geometry: the developed loop must turn both
    // ways. (The develop map is smooth and orientation-preserving here, so a reflex corner in
    // (σ, µ̂) is a reflex corner in the flat pattern.)
    let poly = &flat.domain_hole_polys()[0];
    let cross = |a: &[Q; 2], b: &[Q; 2], c: &[Q; 2]| {
        b[0].sub(&a[0])
            .mul(&c[1].sub(&b[1]))
            .sub(&b[1].sub(&a[1]).mul(&c[0].sub(&b[0])))
            .sign()
    };
    let n = poly.len();
    let turns: Vec<i8> = (0..n)
        .map(|i| cross(&poly[i], &poly[(i + 1) % n], &poly[(i + 2) % n]))
        .collect();
    assert!(
        turns.iter().any(|&t| t > 0) && turns.iter().any(|&t| t < 0),
        "the developed hole must still be non-convex — turns {turns:?}"
    );

    match part.solid() {
        Verdict::Verified(s) => {
            // The hole is really in the shell: its six edges are swept into six walls, on top of
            // the same solid the hole-free panel builds.
            let plain_faces = match panel().solid() {
                Verdict::Verified(p) => p.brep().faces().len(),
                v => panic!("the hole-free panel must build: {}", verdict_name(&v)),
            };
            assert_eq!(
                s.brep().faces().len(),
                plain_faces + poly.len(),
                "one wall per hole edge, and nothing else moved"
            );
        }
        Verdict::Refuted(f) => {
            panic!("the solid path takes a non-convex loop within one slice today, got {f:?}")
        }
        Verdict::Unresolved(e) => panic!("expected Verified, got Unresolved({e:?})"),
    }
}

/// **AUTH.2e — the flip.** The same non-convex loop, moved so it crosses a σ-station, used to be
/// refused by the solid builder while the flat path took it. Now the builder **clips it per slice**
/// (`brep_trim_solid_regions` → `slice_poly_footprint`) and both paths take it.
///
/// The station is not a neutral place to land: this L's step is a `σ = const` edge sitting *on*
/// σ = 0, so the two slices keep different material on that ruling. The cross-ring between them is
/// shared only where both lids reach, and the step itself is a wall — which is what the watertight
/// assertion here is really checking (skipping it returned a `Verified` shell with four free edges).
#[test]
fn a_domain_loop_crossing_a_station_is_clipped_per_slice() {
    // Without the drill, so the σ = 0 station is clear and what is exercised is unambiguously the
    // station crossing rather than a collision with a derived hole.
    let panel = || {
        construct::from_chart::<Bignum>(&cone())
            .region_sigma(qi(-1), qi(1), SupportFn::inherit())
            .intersect(Cutter::half_space([qi(0), qi(0), qi(1)], qi(3)))
            .subtract(Cutter::vertical_cylinder(qi(0), q(1, 2), qi(2)))
    };
    let slot = l_slot(q(-3, 10), q(3, 10), q(23, 10), q(13, 5));
    let plain = panel().hole_domain(slot.clone());
    match plain.develop() {
        Verdict::Verified(_) => {}
        Verdict::Refuted(f) => panic!("the flat path is indifferent to stations, got {f:?}"),
        Verdict::Unresolved(e) => panic!("expected Verified, got Unresolved({e:?})"),
    }
    let solid = match plain.solid() {
        Verdict::Verified(s) => s,
        v => panic!(
            "a station-crossing loop now clips per slice, got {}",
            verdict_name(&v)
        ),
    };
    let brep = solid.brep();
    assert_eq!(brep.free_edges(), 0, "the clipped solid is watertight");
    assert_eq!(brep.nonmanifold_edges(), 0, "…and manifold");

    // The hole is in the shell, edge for edge: a wall per authored edge (6), plus one more for the
    // edge σ = 0 cuts in two (the bottom run at µ̂ = 23/10 crosses the station). The step edge — the
    // authored `σ = 0` one — is a wall too, contributed once by the slice that keeps material there;
    // twice would show up as a non-manifold edge above. Lid faces are unchanged: the hole opens onto
    // the station as a notch in each slice rather than splitting either lid.
    let plain_faces = match panel().solid() {
        Verdict::Verified(p) => p.brep().faces().len(),
        v => panic!("the hole-free panel must build: {}", verdict_name(&v)),
    };
    assert_eq!(
        brep.faces().len(),
        plain_faces + slot.len() + 1,
        "one wall per hole edge, plus the one the station splits"
    );
}
