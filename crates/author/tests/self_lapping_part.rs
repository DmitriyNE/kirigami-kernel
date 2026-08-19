//! The **self-lapping cone on the facade** — the construction-API phase's acceptance geometry:
//! the wrapping chart (`ψ = (260/97)·arctan σ` — more than one turn in a finite window), three
//! piecewise-support regions (body / smoothstep ramp / offset tail), solid cutters with derived
//! roles, the **per-window seam drill** (one cylinder, two derived holes — head and tail flap),
//! a flat-authored hexagon, and the certified fold closing the round trip.

use author::part::{OpRole, Part};
use certify_core::Verdict;
use export::approx::rat_to_f64;
use lattice::{Bignum, Rat};

use acceptance::measure as common;

type Q = Rat<Bignum>;

fn q(n: i128, d: i128) -> Q {
    Q::new(n, d)
}
fn qi(n: i128) -> Q {
    Q::from_i128(n)
}

/// The acceptance device at the suite's lean budget — the same recipe the demo runs at higher
/// fidelity, from the one shared definition (see the `acceptance` crate).
fn device(with_drill: bool) -> Part<Bignum> {
    acceptance::self_lapping_cone(16, 8, with_drill)
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

    // **The chord golden (VV.3).** The holes must not just exist, they must be *round*. This is
    // the metric that caught the reported defect: while a closed cut was represented as two
    // µ̂ = f(σ) graphs bridged by a straight radial chord, each hole's longest emitted edge spanned
    // 30–48% of its own diameter. Measured on the polylines the SVG actually draws.
    let faces = common::emitted_hole_rings(flat.region());
    for (h, ring) in faces[0].iter().enumerate() {
        let frac = common::longest_edge_fraction(ring);
        // Measured 9.4% and 10.1% (2026-08-14); the graph model gave 30–48% on this very drill.
        // 15% is a *structural* gate, not a ratchet: it separates "chord spacing" from "a bridge
        // across the tangent rulings" without being brittle to a resolution change (the metric
        // scales as ~1/n).
        println!(
            "[golden] self-lapping hole {h}: longest edge {:.1}% of diameter",
            frac * 100.0
        );
        assert!(
            frac < 0.15,
            "hole {h}: longest emitted edge is {:.1}% of the hole diameter — a chord that large is \
             the graph-model tangent bridge coming back, not chord spacing",
            frac * 100.0
        );
    }
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

/// **The chord golden detects the defect it was built for.** A quality gate nobody has seen fail
/// is a guess. This reconstructs the shape the graph model actually emitted — a round hole with a
/// run of samples missing, so one straight edge bridges what used to be the tangent gap — and
/// checks the metric both scores it in the historically observed 30–48% band and rejects it at the
/// 15% gate. Pure geometry, no pipeline: it pins the *instrument*, not the kernel.
#[test]
fn the_chord_golden_rejects_a_bridged_hole() {
    let (n, r) = (32usize, 0.5f64);
    // A unit-diameter circle missing samples 1..4 — the gap spans 5/32 of a turn, so the bridging
    // chord is 2r·sin(5π/32) ≈ 0.48 of the diameter.
    let bridged: Vec<[f64; 2]> = (0..n)
        .filter(|i| !(1..4).contains(i))
        .map(|i| {
            let t = 2.0 * std::f64::consts::PI * (i as f64) / (n as f64);
            [r * t.cos(), r * t.sin()]
        })
        .collect();
    let frac = common::longest_edge_fraction(&bridged);
    assert!(
        (0.30..0.50).contains(&frac),
        "the reconstructed defect must land in the observed 30–48% band, got {:.1}%",
        frac * 100.0
    );
    assert!(frac >= 0.15, "and the 15% gate must reject it");

    // The same circle intact scores as ordinary chord spacing and passes.
    let intact: Vec<[f64; 2]> = (0..n)
        .map(|i| {
            let t = 2.0 * std::f64::consts::PI * (i as f64) / (n as f64);
            [r * t.cos(), r * t.sin()]
        })
        .collect();
    let clean = common::longest_edge_fraction(&intact);
    assert!(
        clean < 0.15,
        "an evenly sampled circle must pass the gate, got {:.1}%",
        clean * 100.0
    );
}

/// **The ε budget (VV.2).** Every other test here asks only whether a stage *certifies* — but a
/// `Verified` verdict means "ε is under the clearance", and the clearance is 1, so a change that
/// made every bound ten times worse would still pass the whole suite. These are the quality
/// bounds: each stage's certified ε, pinned just above its measured value. **A failure here is
/// not unsoundness** — the geometry is still certified — it means an edit moved a bound, and the
/// budget line says by how much. Tighten the constants whenever a change legitimately improves
/// one; that is the ratchet.
///
/// Measured on this device (`segments(16)`, `support_panels(8)`, 2026-08-14):
///
/// | stage   | measured  | budget | headroom |
/// |---------|-----------|--------|----------|
/// | develop | 1.5266e0  | 13/8   | 1.06×    |
/// | fold    | 3.3975e-1 | 7/20   | 1.03×    |
/// | refold  | 7.57e-2   | 1/8    | 1.65×    |
/// | solid   | 1.6214e-1 | 1/5    | 1.23×    |
///
/// `develop` gets the least headroom because it has the least to give: the DRC gate is
/// `clearance/2 = 7/2`, so at 1.53e0 this device certifies at **44% of its ceiling** — the
/// re-proportioning to Ø 21.5 halved the ε while the clearance stayed put, so what used to be the
/// binding constraint now has room. `fold` is the tight one at 1.03×, and it is the bound to watch
/// before optimizing anything the fold leg touches.
#[test]
fn the_certified_bounds_stay_within_budget() {
    // Pinned bounds. Raise ONLY with a recorded reason; lower freely when a change earns it.
    //
    // Re-pinned 2026-08-17 with the outer diameter's correction, Ø 43 → Ø 21.5 mm. Three of the
    // four moved, each for one reason:
    //
    //   * `develop 13/4 → 13/8`. Exactly halved, because the measurement did: 3.053e0 → 1.527e0
    //     against an outer radius of 21.5 → 10.75. It is a length on a shorter part and nothing
    //     more, which is what one wants a re-scaling to look like. The headroom is deliberately the
    //     same 1.06× it was — `develop` has the least room to give (see the table above).
    //   * `solid 1/3 → 1/5`. 2.715e-1 → 1.621e-1, a factor 0.60 rather than 0.50, because the
    //     solid's ε is set by the σ-slice chords as much as by the radial extent and the azimuthal
    //     sampling did not change.
    //   * `refold 1/20 → 1/8`, and it is now a **radius**, not a squared radius. The old metric was
    //     `|ρ² − r²|`, which is `≈ 2r` times the quantity anyone reading it would assume, and a
    //     budget on a squared residual scales with the square of everything. As a length it went
    //     2.28e-2 → 7.57e-2, a factor 3.3: the drill rode inward with the annulus (`ρ = 12.72 →
    //     7.63`) and the same 1.5 mm hole now sits on a patch with 1.67× the curvature, which the
    //     sagitta squares to ≈2.8.
    //
    // `fold` does not track the radius: the fold works in the flat pattern's own frame. It is
    // also the *tightest* of the four, which is worth knowing before optimizing anything it
    // touches. `7/20 → 21/50` (2026-08-20, #310): the share-proportional rail chords sample the
    // long body arcs the old 8-per-piece grid skipped, and the fold's sup over the outline moved
    // 3.398e-1 → 4.002e-1 by *measuring more points* — same geometry, a more complete sup (the
    // other three budgets did not move). 21/50 restores the documented ≈1.05× headroom.
    let develop_max = q(13, 8);
    let solid_max = q(1, 5);
    let fold_max = q(21, 50);
    let refold_max = q(1, 8);

    let part = device(true);
    let flat = match part.develop() {
        Verdict::Verified(f) => f,
        Verdict::Unresolved(e) => panic!("develop unresolved at ε ≈ {:.3e}", rat_to_f64(&e)),
        Verdict::Refuted(f) => panic!("develop refuted: {f:?}"),
    };
    let develop_eps = flat.eps().clone();

    // The fold leg, sampled across all three support regions (body γ ≡ 0, ramp and tail γ ≠ 0):
    // the worst certified round-trip bound over the sample.
    let verts = &flat.outline().vertices;
    let n = verts.len();
    let mut fold_eps = q(0, 1);
    for k in 0..6 {
        let i = (k * n) / 6;
        let (x, y) = verts[i].center();
        match part.fold(&[[x, y]], &qi(0)) {
            Verdict::Verified(w) => {
                if w.eps.cmp(&fold_eps) == core::cmp::Ordering::Greater {
                    fold_eps = w.eps.clone();
                }
            }
            Verdict::Unresolved(e) => panic!("fold unresolved at ε ≈ {:.3e}", rat_to_f64(&e)),
            Verdict::Refuted(f) => panic!("fold refuted at outline vertex {i}: {f:?}"),
        }
    }

    // The round trip that actually matters to the device: the two flat drill holes, far apart in
    // the pattern, must fold back onto the ONE drill cylinder they were cut from.
    // Read from the part's own recipe, never restated: a copied constant here silently measures
    // the round trip against a cylinder the device is no longer cut with.
    let (dcx, dcy, dr2) = {
        let (x, y, r2) = acceptance::seam_drill_axis();
        (rat_to_f64(&x), rat_to_f64(&y), rat_to_f64(&r2))
    };
    let r_drill = dr2.sqrt();
    let mut refold = 0.0f64;
    for hole in flat.holes() {
        let hv = &hole.vertices;
        for j in (0..hv.len()).step_by(8) {
            let (x, y) = hv[j].center();
            match part.fold(&[[x, y]], &qi(0)) {
                Verdict::Verified(w) => {
                    let p = &w.points[0];
                    let (px, py) = (rat_to_f64(&p[0].mid()), rat_to_f64(&p[1].mid()));
                    // How far off the drill's *surface* the folded point landed — a length in the
                    // part's own unit, so it is comparable to every other ε here. (It was
                    // `|ρ² − r²|`, which is `≈ 2r` times this and scales as a square.)
                    let d = ((px - dcx).hypot(py - dcy) - r_drill).abs();
                    refold = refold.max(d);
                }
                Verdict::Unresolved(e) => panic!("refold unresolved at ε ≈ {:.3e}", rat_to_f64(&e)),
                Verdict::Refuted(f) => panic!("refold refuted: {f:?}"),
            }
        }
    }

    let solid = match part.solid() {
        Verdict::Verified(s) => s,
        Verdict::Unresolved(e) => panic!("solid unresolved at ε ≈ {:.3e}", rat_to_f64(&e)),
        Verdict::Refuted(f) => panic!("solid refuted: {f:?}"),
    };
    let solid_eps = solid.eps().clone();

    // The device's topology, pinned where its solid is already built: the ONE seam drill bores
    // through both sheets of the lap, so the shell has two tunnels. This is the control
    // `self_lapping_slot.rs` reads its genus against — a traced slot there must take it to four.
    let brep = solid.brep();
    let (v, e, f) = (
        brep.verts().len() as i64,
        brep.edges().len() as i64,
        brep.faces().len() as i64,
    );
    let l: i64 = brep
        .faces()
        .iter()
        .map(|fc| 1 + fc.holes.len() as i64)
        .sum();
    let genus = (2 - (v - e + (2 * f - l))) / 2;
    println!("[topology] drilled device: {f} faces, genus {genus}");
    assert_eq!(brep.free_edges(), 0, "watertight");
    assert_eq!(
        genus, 2,
        "one drill, two sheets, two tunnels — a genus other than 2 means the drill bored one sheet \
         or neither"
    );

    println!(
        "[budget] develop {:.4e}/{:.4e}  fold {:.4e}/{:.4e}  refold {:.4e}/{:.4e}  solid {:.4e}/{:.4e}",
        rat_to_f64(&develop_eps),
        rat_to_f64(&develop_max),
        rat_to_f64(&fold_eps),
        rat_to_f64(&fold_max),
        refold,
        rat_to_f64(&refold_max),
        rat_to_f64(&solid_eps),
        rat_to_f64(&solid_max),
    );

    let within = |got: &Q, max: &Q| got.cmp(max) != core::cmp::Ordering::Greater;
    assert!(
        within(&develop_eps, &develop_max),
        "develop ε {:.4e} exceeds its budget {:.4e}",
        rat_to_f64(&develop_eps),
        rat_to_f64(&develop_max)
    );
    assert!(
        within(&fold_eps, &fold_max),
        "fold ε {:.4e} exceeds its budget {:.4e}",
        rat_to_f64(&fold_eps),
        rat_to_f64(&fold_max)
    );
    assert!(
        refold < rat_to_f64(&refold_max),
        "refold defect {refold:.4e} exceeds its budget {:.4e}",
        rat_to_f64(&refold_max)
    );
    assert!(
        within(&solid_eps, &solid_max),
        "solid ε {:.4e} exceeds its budget {:.4e}",
        rat_to_f64(&solid_eps),
        rat_to_f64(&solid_max)
    );
}
