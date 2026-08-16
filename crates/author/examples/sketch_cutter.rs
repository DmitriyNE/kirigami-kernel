//! The **sketch-extrude cutter** (AUTH.1 / AUTH.2) on the construction facade — drafted holes and
//! non-convex slots through the Stage-1 device gore, both product directions, per stage certified.
//!
//! Three features are cut through the same panel, and the point of running all three is that the
//! certificates cannot tell them apart. `ε` is the max over pipeline stages and this ~296° gore's
//! boundary dominates it, so a drafted hole, an undrafted one and a slot that missed entirely all
//! report the same number. What distinguishes them is **geometry**, which is what this driver
//! measures and prints:
//!
//! - a **drafted disc**, swept from a finite cast point, against the parallel sweep of the same
//!   profile — the taper law `1 − z/z_apex` the apex implies (AUTH.1f);
//! - an **L-slot**, whose footprint no near/far band can express: a ruling meets it twice, and the
//!   flat pattern is bracketed by three metric discs — one it must contain, one it must lie within,
//!   and one, in the notch, it must leave alone (AUTH.2f);
//! - a **keyhole**, whose head is a circle and whose stem is straight, so its saddle joins walls of
//!   different degree — the case no polygon reaches.
//!
//! Both directions run. Direction ① develops the 3-D cut to the flat pattern and writes the SVG;
//! direction ② folds the flat pattern's own vertices back and reports how far they land from the
//! profile they were drawn from. The solid is written as STEP under `--features step`.
//!
//! ```text
//! cargo run --example sketch_cutter                                   # panels + SVG
//! nix develop -c cargo run --example sketch_cutter --features step    # + STEP
//! ```
//!
//! Flags: `--out-dir <dir>` (default `generated-demos/`).

use acceptance::measure;
use arrange2d::profile::Profile;
use author::part::{FlatPattern, Part};
use certify_core::Verdict;
use develop::counters;
use develop::extrude::Apex;
use export::approx::rat_to_f64;
use geom::content::Edge;
use lattice::{Bignum, Rat};

type Q = Rat<Bignum>;

fn q(n: i128, d: i128) -> Q {
    Q::new(n, d)
}
fn qi(n: i128) -> Q {
    Q::from_i128(n)
}

fn parallel() -> Apex<Bignum> {
    Apex::direction([qi(0), qi(0), qi(1)]).expect("a real direction")
}

fn panel(apex: Apex<Bignum>, profile: Vec<Edge<Bignum>>) -> Part<Bignum> {
    acceptance::sketch_panel(Some((apex, profile)))
}

/// Develop, print the per-stage line, and hand back the pattern.
fn develop(name: &str, part: &Part<Bignum>) -> Option<FlatPattern<Bignum>> {
    let t0 = std::time::Instant::now();
    match part.develop() {
        Verdict::Verified(f) => {
            let cut = f.report().ops[3]
                .cut_eps
                .as_ref()
                .map(rat_to_f64)
                .unwrap_or(f64::NAN);
            println!(
                "{name:<9} develop   Verified   ε {:.3e}   cut ε {cut:.3e}   1 face · {} hole(s)   \
                 [{:.1}s]",
                rat_to_f64(f.eps()),
                f.region().faces[0].holes.len(),
                t0.elapsed().as_secs_f64()
            );
            Some(f)
        }
        Verdict::Refuted(fault) => {
            println!("{name:<9} develop   Refuted({fault:?})");
            None
        }
        Verdict::Unresolved(e) => {
            println!(
                "{name:<9} develop   Unresolved at ε {:.3e} — raise clearance/segments",
                rat_to_f64(&e)
            );
            None
        }
    }
}

/// The one interior hole's emitted ring.
fn ring(f: &FlatPattern<Bignum>) -> Vec<[f64; 2]> {
    measure::emitted_hole_rings(f.region())
        .first()
        .and_then(|face| face.first().cloned())
        .unwrap_or_default()
}

fn width(ring: &[[f64; 2]]) -> f64 {
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for p in ring {
        lo = lo.min(p[0]);
        hi = hi.max(p[0]);
    }
    hi - lo
}

fn write_svg(out_dir: &str, name: &str, f: &FlatPattern<Bignum>) {
    let svg = f.svg(720);
    let path = format!("{out_dir}/sketch_{name}.svg");
    std::fs::write(&path, &svg).expect("write the flat pattern");
    println!("{name:<9} SVG       wrote {path}   ({} bytes)", svg.len());
}

#[cfg(feature = "step")]
fn write_step(out_dir: &str, name: &str, part: &Part<Bignum>) {
    counters::reset();
    match part.solid() {
        Verdict::Verified(solid) => {
            let path = format!("{out_dir}/sketch_{name}.step");
            let report = solid.write_step(&path);
            let b = solid.brep();
            println!(
                "{name:<9} solid     {}   {} faces · free {} · non-manifold {} · {} slice clips · \
                 shortest edge {:.2e} → {path}",
                report.summary(),
                b.faces().len(),
                b.free_edges(),
                b.nonmanifold_edges(),
                counters::poly_slice_clips(),
                measure::shortest_edge(b)
            );
        }
        Verdict::Refuted(fault) => println!("{name:<9} solid     Refuted({fault:?})"),
        Verdict::Unresolved(e) => {
            println!("{name:<9} solid     Unresolved at ε {:.3e}", rat_to_f64(&e))
        }
    }
}

#[cfg(not(feature = "step"))]
fn write_step(_out_dir: &str, name: &str, part: &Part<Bignum>) {
    // Without OCCT the shell is still built and audited — only the `.step` file is skipped.
    counters::reset();
    match part.solid() {
        Verdict::Verified(solid) => {
            let b = solid.brep();
            println!(
                "{name:<9} solid     Verified   ε {:.3e}   {} faces · free {} · non-manifold {} · \
                 {} slice clips · shortest edge {:.2e}   (STEP skipped — build under \
                 `nix develop --features step`)",
                rat_to_f64(solid.eps()),
                b.faces().len(),
                b.free_edges(),
                b.nonmanifold_edges(),
                counters::poly_slice_clips(),
                measure::shortest_edge(b)
            );
        }
        Verdict::Refuted(fault) => println!("{name:<9} solid     Refuted({fault:?})"),
        Verdict::Unresolved(e) => {
            println!("{name:<9} solid     Unresolved at ε {:.3e}", rat_to_f64(&e))
        }
    }
}

/// Direction ②: fold the developed hole's own vertices back to 3-D and report the worst distance
/// from the recovered `(x, y)` to the profile it was drawn from. The sweep is parallel to `z`, so
/// a point of the cutter's wall projects onto the authored profile's boundary — a residual neither
/// leg computes, since neither knows about the other.
fn fold_back(name: &str, part: &Part<Bignum>, f: &FlatPattern<Bignum>, corners: &[[f64; 2]]) {
    let Some(hole) = f.holes().first() else {
        return;
    };
    let n = hole.vertices.len();
    let seg_dist = |p: [f64; 2], a: [f64; 2], b: [f64; 2]| {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len2 = dx * dx + dy * dy;
        let t = if len2 > 0.0 {
            (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let (ux, uy) = (a[0] + t * dx - p[0], a[1] + t * dy - p[1]);
        (ux * ux + uy * uy).sqrt()
    };
    let (mut worst, mut folded, mut eps) = (0.0f64, 0usize, 0.0f64);
    for k in (0..n).step_by(n.div_ceil(12).max(1)) {
        let (x, y) = hole.vertices[k].center();
        match part.fold(&[[x, y]], &qi(0)) {
            Verdict::Verified(w) => {
                let p = &w.points[0];
                let xy = [rat_to_f64(&p[0].mid()), rat_to_f64(&p[1].mid())];
                let d = (0..corners.len())
                    .map(|i| seg_dist(xy, corners[i], corners[(i + 1) % corners.len()]))
                    .fold(f64::INFINITY, f64::min);
                worst = worst.max(d);
                eps = eps.max(rat_to_f64(&w.eps));
                folded += 1;
            }
            other => {
                println!(
                    "{name:<9} fold      vertex {k}: {}",
                    match other {
                        Verdict::Refuted(fault) => format!("Refuted({fault:?})"),
                        _ => "Unresolved".into(),
                    }
                );
                return;
            }
        }
    }
    println!(
        "{name:<9} fold      Verified   {folded} vertices   round-trip ε {eps:.3e}   worst \
         profile residual {worst:.3e}"
    );
}

/// The L-slot's six authored corners in world `(x, y)`, mirroring `acceptance::ell_slot`.
fn ell_corners() -> Vec<[f64; 2]> {
    let (cx, cy, a, t) = (-0.1f64, 2.2f64, 0.25f64, 0.125f64);
    let (ux, uy, vx, vy) = (0.8f64, -0.6f64, 0.6f64, 0.8f64);
    let p = |su: f64, sv: f64| [cx + ux * su + vx * sv, cy + uy * su + vy * sv];
    vec![p(0.0, 0.0), p(a, 0.0), p(a, t), p(t, t), p(t, a), p(0.0, a)]
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut out_dir = "generated-demos".to_string();
    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "--out-dir" => {
                out_dir = argv[i + 1].clone();
                i += 2;
            }
            other => panic!("unknown flag {other}"),
        }
    }
    std::fs::create_dir_all(&out_dir).expect("create --out-dir");

    println!("sketch-extrude cutter — device cone (β≈42°), gore σ∈[−7/2,7/2] (~296°)\n");

    // ── AUTH.1f: the draft angle ────────────────────────────────────────────────────────────────
    let disc = |r: Q| Profile::new().circle(qi(0), q(11, 5), r).into_edges();
    let drafted_part = panel(Apex::point([qi(0), q(11, 5), qi(12)]), disc(q(1, 5)));
    let drafted = develop("drafted", &drafted_part);
    let flat_par = develop("parallel", &panel(parallel(), disc(q(1, 5))));
    if let (Some(a), Some(b)) = (&drafted, &flat_par) {
        let (wa, wb) = (width(&ring(a)), width(&ring(b)));
        println!(
            "drafted   faithful  {wa:.4} vs parallel {wb:.4}   ratio {:.3}   (taper law ≈ 0.797)",
            wa / wb
        );
        write_svg(&out_dir, "drafted", a);
    }

    // ── AUTH.2f: the non-convex footprints ──────────────────────────────────────────────────────
    let mut slot: Option<Vec<[f64; 2]>> = None;
    for (name, profile, corners) in [
        ("L-slot", acceptance::ell_slot(), Some(ell_corners())),
        ("keyhole", acceptance::keyhole_slot(), None),
    ] {
        println!();
        let part = panel(parallel(), profile);
        let Some(f) = develop(name, &part) else {
            continue;
        };
        let r = ring(&f);
        if name == "L-slot" {
            slot = Some(r.clone());
        }
        println!(
            "{name:<9} phenom    a ruling meets the cut {} time(s)   (a band gives 1; the \
             development sends rulings to rays from the flat apex)",
            measure::max_ray_crossings(&r) / 2
        );
        println!(
            "{name:<9} golden    longest emitted edge {:.1}% of the hole's extent",
            measure::longest_edge_fraction(&r) * 100.0
        );
        write_svg(&out_dir, name, &f);
        if let Some(c) = corners {
            fold_back(name, &part, &f, &c);
        }
        write_step(&out_dir, name, &part);
    }

    // ── The two-sided differential, all three probes through the metric path ────────────────────
    println!();
    let [inner, outer, notch] = acceptance::ell_probes();
    let probe = |(cx, cy, r2): (Q, Q, Q), name: &str| {
        develop(name, &acceptance::sketch_drill(cx, cy, r2)).map(|f| ring(&f))
    };
    let (pi, po, pn) = (
        probe(inner, "inscribed"),
        probe(outer, "circumscr"),
        probe(notch, "notch"),
    );
    if let (Some(s), Some(a), Some(o), Some(n)) = (&slot, &pi, &po, &pn) {
        println!(
            "\nL-slot    two-sided  contains the inscribed disc: {}   lies within the \
             circumscribing one: {}   leaves the notch alone: {}",
            measure::ring_inside(a, s),
            measure::ring_inside(s, o),
            measure::rings_disjoint(n, s),
        );
        println!(
            "L-slot    areas      {:.6} < {:.6} < {:.6}   (notch {:.6}, disjoint)",
            measure::ring_area(a),
            measure::ring_area(s),
            measure::ring_area(o),
            measure::ring_area(n),
        );
    }

    // ── The scope refusal, by name ──────────────────────────────────────────────────────────────
    println!();
    match panel(parallel(), acceptance::ring_slot()).develop() {
        Verdict::Refuted(fault) => println!(
            "ring      refused   {fault:?}   (§11.8 — an annular \
             through-cut leaves an island of material, which is two parts)"
        ),
        _ => println!("ring      NOT REFUSED — the scope boundary moved"),
    }
}
