//! The **self-lapping cone** on the construction facade — the flex-PCB spine's acceptance demo:
//! ONE connected development that, cut from a single sheet and folded, becomes a cone whose
//! offset tail laps over its head.
//!
//! The wrapping chart (`ψ = (260/97)·arctan σ` — the Gauss circle traversed twice, so one
//! turn-plus-lap fits the finite window `σ ∈ [−5/4, 5/4]`) carries three piecewise-support
//! regions: the body (`h ≡ 0`), a §8 smoothstep ramp (`h: 0 → D`), and the lap plateau
//! (`h ≡ D`). Everything else is **derived**: the concentric outer and eccentric inner cylinders
//! resolve to the bounding rails of an offset annulus, and the ONE vertical **seam drill** over
//! the lap resolves to TWO interior holes — the head and the tail flap it laps — one per
//! disc-positive window.
//!
//! Two authored features ride along, one per direction of the round trip. A hexagon drawn **in flat
//! ECAD coordinates** goes 2-D → 3-D: [`develop`](author::part::Part::develop) cuts it as-is,
//! [`fold`](author::part::Part::fold) certifies it back onto the cone, and
//! [`solid`](author::part::Part::solid) drills it through the STEP shell. An **L-shaped sketch
//! extrusion** (`acceptance::lap_slot`) goes the other way: drawn in the `z = 0` plane and swept, it
//! is *traced* into the domain as a footprint some ruling meets twice, and because it sits in the
//! lap wedge it pierces both sheets at once — one hole in the body at `γ ≡ 0`, its twin in the
//! smoothstep ramp at `γ ≠ 0`. The featureless recipe (`acceptance::self_lapping_cone`) stays the
//! V&V baseline; `self_lapping_slot.rs` pins this one.
//!
//! ```text
//! cargo run --example self_lapping_cone                                 # flat + folded SVGs
//! nix develop -c cargo run --example self_lapping_cone --features step  # + the STEP solid
//! ```
//!
//! Flags: `--segments <n>` (rail discretization, default 24), `--out-dir <dir>` (default
//! `generated-demos/`). This is the old 917-line hand-wired demo collapsed onto the facade —
//! same device, same certificates, structure no longer hand-picked.

use author::part::Part;
use certify_core::Verdict;
use develop::cone::{ConeDevelopment, DevConfig};
use develop::extrude::Apex;
use export::approx::{rat_to_f64, surd_to_f64};
use fixtures::devices::cone_wrap;
use geom::content::Edge;
use lattice::{Bignum, Rat};

type Q = Rat<Bignum>;

fn q(n: i128, d: i128) -> Q {
    Q::new(n, d)
}
fn qi(n: i128) -> Q {
    Q::from_i128(n)
}

/// The device at the demo's fidelity, from the one shared definition (see the `acceptance`
/// crate) — the same recipe the V&V suite pins at a leaner budget, carrying the lap slot.
fn device(segments: usize) -> Part<Bignum> {
    let apex = Apex::direction([qi(0), qi(0), qi(1)]).expect("a real sweep direction");
    acceptance::self_lapping_cone_with(segments, 20, true, Some((apex, acceptance::lap_slot())))
}

/// The authored L's boundary segments in the `z = 0` sketch plane, as floats.
///
/// The sweep is parallel to `z`, so a point of the cutter's wall projects to a point of *this*
/// polygon — which is what lets a folded slot vertex be checked against the shape it was drawn as,
/// rather than against a restatement of it.
fn slot_profile() -> Vec<([f64; 2], [f64; 2])> {
    acceptance::lap_slot()
        .iter()
        .filter_map(|e| match e {
            Edge::Seg(s) => Some((
                [surd_to_f64(&s.start.x), surd_to_f64(&s.start.y)],
                [surd_to_f64(&s.end.x), surd_to_f64(&s.end.y)],
            )),
            Edge::Arc(_) => None,
        })
        .collect()
}

/// The distance from `p` to the nearest point of `segs`.
fn profile_residual(p: [f64; 2], segs: &[([f64; 2], [f64; 2])]) -> f64 {
    segs.iter()
        .map(|(a, b)| {
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let len2 = dx * dx + dy * dy;
            let t = if len2 > 0.0 {
                (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            (a[0] + t * dx - p[0]).hypot(a[1] + t * dy - p[1])
        })
        .fold(f64::INFINITY, f64::min)
}

/// The flat-authored hexagon (rational ECAD coordinates) around the flat image of a mid-annulus
/// body point — the 2-D → 3-D leg's feature.
fn hexagon() -> Vec<[Q; 2]> {
    let body = ConeDevelopment::new(&cone_wrap()).expect("wrapping cone develops");
    let rz = cone_wrap()
        .ruling()
        .comp(2)
        .eval(&q(-1, 4))
        .expect("regular");
    let mu = q(-29, 6).div(&rz); // z = −29/6: mid-annulus at σ = −1/4 (the old −29/10, scaled 5/3)
    let (cx, cy) = body
        .point_signed(&q(-1, 4), &mu, &DevConfig::tight())
        .center();
    let r = q(2, 3); // the old 2/5, on the same 5/3 as the rest of the device
    [
        (qi(1), qi(0)),
        (q(1, 2), q(7, 8)),
        (q(-1, 2), q(7, 8)),
        (qi(-1), qi(0)),
        (q(-1, 2), q(-7, 8)),
        (q(1, 2), q(-7, 8)),
    ]
    .iter()
    .map(|(u, v)| [cx.add(&u.mul(&r)), cy.add(&v.mul(&r))])
    .collect()
}

/// A minimal SVG of 2-D rings (each an `evenodd` sub-path, math-up y-flip).
fn rings_svg(rings: &[Vec<[f64; 2]>], px: f64) -> String {
    let (mut minx, mut miny, mut maxx, mut maxy) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for r in rings {
        for p in r {
            minx = minx.min(p[0]);
            miny = miny.min(p[1]);
            maxx = maxx.max(p[0]);
            maxy = maxy.max(p[1]);
        }
    }
    let pad = 0.06 * (maxx - minx).max(maxy - miny);
    let (minx, miny) = (minx - pad, miny - pad);
    let (w, h) = (maxx - minx + pad, maxy - miny + pad);
    let hpx = (px * h / w).round().max(1.0);
    let flip = 2.0 * miny + h;
    let sw = 0.004 * w.max(h);
    let mut d = String::new();
    for r in rings {
        for (i, p) in r.iter().enumerate() {
            d.push_str(if i == 0 { "M" } else { "L" });
            d.push_str(&format!("{:.5} {:.5} ", p[0], p[1]));
        }
        d.push_str("Z ");
    }
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{px:.0}\" height=\"{hpx:.0}\" \
         viewBox=\"{minx:.5} {miny:.5} {w:.5} {h:.5}\">\
         <g transform=\"matrix(1 0 0 -1 0 {flip:.5})\">\
         <path d=\"{d}\" fill=\"#5b8def\" fill-opacity=\"0.35\" fill-rule=\"evenodd\" \
         stroke=\"#1f3b8c\" stroke-width=\"{sw:.5}\" stroke-linejoin=\"round\"/></g></svg>"
    )
}

/// Every `stride`-th vertex center of a flat loop, as rational fold input.
fn subsample(loop_: &[[Q; 2]], stride: usize) -> Vec<[Q; 2]> {
    loop_
        .iter()
        .step_by(stride.max(1))
        .cloned()
        .collect::<Vec<_>>()
}

fn main() {
    // — Arguments —
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut segments = 24usize;
    let mut out_dir = "generated-demos".to_string();
    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "--segments" => {
                segments = argv[i + 1].parse().expect("--segments <n>");
                i += 2;
            }
            "--out-dir" => {
                out_dir = argv[i + 1].clone();
                i += 2;
            }
            other => panic!("unknown flag {other}"),
        }
    }
    std::fs::create_dir_all(&out_dir).expect("create --out-dir");

    let c = 260.0 / 97.0;
    let sector = c * 2.0 * (1.25f64).atan() * 180.0 / std::f64::consts::PI;
    println!("self-lapping cone — one connected development, one Part, structure derived");
    println!("  flat sector      : {sector:.1}°  (one turn ≈ 240.9°, the excess is the lap)");

    let part = device(segments).hole_flat(hexagon());

    // — develop: the certified flat pattern —
    let t_develop = std::time::Instant::now();
    let develop_verdict = part.develop();
    eprintln!(
        "[time] develop           {:8.2}s",
        t_develop.elapsed().as_secs_f64()
    );
    let flat = match develop_verdict {
        Verdict::Verified(f) => f,
        Verdict::Refuted(fault) => {
            println!("develop: Refuted({fault:?}) — stopping");
            std::process::exit(1);
        }
        Verdict::Unresolved(e) => {
            println!("develop: Unresolved at ε ≈ {:.3e}", rat_to_f64(&e));
            std::process::exit(1);
        }
    };
    for (k, op) in flat.report().ops.iter().enumerate() {
        let kind = if op.subtract {
            "subtract "
        } else {
            "intersect"
        };
        println!("  op {k}:  {kind}  → {:?}   (derived)", op.role);
    }
    println!(
        "  develop          : Verified  ε ≈ {:.3e}   1 face · {} holes (drill×2 + slot×2 + hex)",
        rat_to_f64(flat.eps()),
        flat.region().faces[0].holes.len()
    );
    if let Some(cut) = flat.report().ops[3].cut_eps.as_ref() {
        println!(
            "  lap slot         : traced footprint, own cut bound ε ≈ {:.3e}",
            rat_to_f64(cut)
        );
    }
    let svg_path = format!("{out_dir}/self_lapping_cone.svg");
    let t_svg = std::time::Instant::now();
    let svg = flat.svg(900);
    eprintln!(
        "[time] flat svg          {:8.2}s",
        t_svg.elapsed().as_secs_f64()
    );
    std::fs::write(&svg_path, svg).expect("write flat svg");
    println!("  wrote {svg_path}");

    // — fold: the certified 2-D → 3-D leg, and the folded top-down view —
    let flat_loop = |o: &develop::unroll::FlatOutline<Bignum>| -> Vec<[Q; 2]> {
        o.vertices
            .iter()
            .map(|b| {
                let (x, y) = b.center();
                [x, y]
            })
            .collect()
    };
    let mut folded_rings: Vec<Vec<[f64; 2]>> = Vec::new();
    let mut fold_eps = 0.0f64;
    let mut rings: Vec<Vec<[Q; 2]>> = vec![subsample(&flat_loop(flat.outline()), 2)];
    for h in flat.holes() {
        rings.push(subsample(&flat_loop(h), 2));
    }
    rings.push(hexagon());
    for (r, ring) in rings.iter().enumerate() {
        let t_ring = std::time::Instant::now();
        let verdict = part.fold(ring, &qi(0));
        eprintln!(
            "[time] fold ring {r}        {:8.2}s   ({} pts)",
            t_ring.elapsed().as_secs_f64(),
            ring.len()
        );
        match verdict {
            Verdict::Verified(wire) => {
                fold_eps = fold_eps.max(rat_to_f64(&wire.eps));
                folded_rings.push(
                    wire.points
                        .iter()
                        .map(|p| [rat_to_f64(&p[0].mid()), rat_to_f64(&p[1].mid())])
                        .collect(),
                );
            }
            Verdict::Refuted(fault) => {
                println!("fold: Refuted({fault:?}) — stopping");
                std::process::exit(1);
            }
            Verdict::Unresolved(e) => {
                println!("fold: Unresolved at ε ≈ {:.3e}", rat_to_f64(&e));
                std::process::exit(1);
            }
        }
    }
    println!(
        "  fold             : Verified  ε ≈ {fold_eps:.3e}   (outline + 4 derived holes + hex, direction ②)"
    );
    // The refold check: both folded drill rings land on the ONE drill cylinder — the two flat
    // holes, far apart in the pattern, coincide through the sheet when rolled up. `rings` is
    // [outline, hole₀…hole₃, hex] and `FlatPattern::holes()` comes in op order, so 1..3 are the
    // drill's two windows and 3..5 the slot's.
    let (dcx, dcy, dr2) = {
        let (x, y, r2) = acceptance::seam_drill_axis();
        (rat_to_f64(&x), rat_to_f64(&y), rat_to_f64(&r2))
    };
    let refold = folded_rings[1..3]
        .iter()
        .flatten()
        .map(|p| ((p[0] - dcx).powi(2) + (p[1] - dcy).powi(2) - dr2).abs())
        .fold(0.0f64, f64::max);
    println!("  refold defect    : {refold:.3e}   (folded drill holes land on the drill cylinder)");
    // …and the traced slot's two loops, folded, land on the L that was drawn in the sketch plane —
    // the round trip closed on a feature that went 3-D → flat, not the hexagon's flat → 3-D.
    let segs = slot_profile();
    let slot_residual = folded_rings[3..5]
        .iter()
        .flatten()
        .map(|p| profile_residual([p[0], p[1]], &segs))
        .fold(0.0f64, f64::max);
    println!(
        "  slot residual    : {slot_residual:.3e}   (folded slot loops land on the authored L)"
    );
    let folded_path = format!("{out_dir}/self_lapping_cone_folded.svg");
    std::fs::write(&folded_path, rings_svg(&folded_rings, 900.0)).expect("write folded svg");
    println!("  wrote {folded_path}   (top-down: the tail laps the head)");

    // — solid: the certified watertight STEP shell —
    #[cfg(feature = "step")]
    let t_solid = std::time::Instant::now();
    #[cfg(feature = "step")]
    let solid_verdict = part.solid();
    #[cfg(feature = "step")]
    eprintln!(
        "[time] solid             {:8.2}s",
        t_solid.elapsed().as_secs_f64()
    );
    #[cfg(feature = "step")]
    match solid_verdict {
        Verdict::Verified(solid) => {
            let path = format!("{out_dir}/self_lapping_cone.step");
            let t_step = std::time::Instant::now();
            let report = solid.write_step(&path);
            eprintln!(
                "[time] write_step        {:8.2}s",
                t_step.elapsed().as_secs_f64()
            );
            println!("  STEP             : {}   → {path}", report.summary());
        }
        Verdict::Refuted(fault) => println!("  STEP             : Refuted({fault:?})"),
        Verdict::Unresolved(e) => {
            println!(
                "  STEP             : Unresolved at ε ≈ {:.3e}",
                rat_to_f64(&e)
            );
        }
    }
    #[cfg(not(feature = "step"))]
    println!("  STEP             : skipped — build under `nix develop` with `--features step`");
}
