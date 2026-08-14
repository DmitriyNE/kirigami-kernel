//! The **flex-PCB panel** on the construction facade — the Stage-1 deliverable, declaratively.
//!
//! The 296° device-cone gore is authored as a [`Part`]: one region over `σ ∈ [−7/2, 7/2]`, four
//! **solid cutters** whose roles the evaluator *derives* (the `z ≤ 3` half-space bounds the
//! blank, the eccentric apex cylinder carves the annulus, one cylinder notches the rim, one
//! drills a through-hole), plus a domain-authored quad cut. `develop()` certifies the flat
//! pattern (rails → chord-certified unroll → exact boolean, topology-coherence-gated);
//! `solid()` re-certifies at the STEP profile and sews the watertight solid.
//!
//! ```text
//! cargo run --example flex_panel                                   # panel + SVG
//! nix develop -c cargo run --example flex_panel --features step    # + STEP I/II
//! ```
//!
//! Flags: `--segments <n>` (rail discretization, default 72), `--out-dir <dir>` (default
//! `generated-demos/`). This is the old 415-line hand-wired `export` demo collapsed onto the
//! facade — same geometry, same certificates, roles no longer hand-picked.

use author::construct;
use author::part::{Cutter, Part, SupportFn};
use certify_core::Verdict;
use export::approx::rat_to_f64;
use fixtures::devices::cone;
use lattice::{Bignum, Rat};

type Q = Rat<Bignum>;

fn q(n: i128, d: i128) -> Q {
    Q::new(n, d)
}
fn qi(n: i128) -> Q {
    Q::from_i128(n)
}

/// The panel recipe; `with_cuts` adds the D4 drill + the quad (off = the STEP-I blank).
/// Across the wide gore the four solids genuinely keep material on BOTH sheets of the cone
/// (the antipodal ray crosses the disks too — the old hand-picked `RootPick` hid the choice),
/// so the recipe carries an exact witness: keep the material near a point on the panel.
fn panel(segments: usize, with_cuts: bool) -> Part<Bignum> {
    let witness = cone()
        .surface(&qi(2), &qi(0))
        .eval(&qi(0))
        .expect("the device cone is regular at σ = 0");
    let mut part = construct::from_chart::<Bignum>(&cone())
        .region_sigma(q(-7, 2), q(7, 2), SupportFn::inherit())
        .keep_near(witness)
        .intersect(Cutter::half_space([qi(0), qi(0), qi(1)], qi(3))) // D1: bound the blank
        .subtract(Cutter::vertical_cylinder(qi(0), q(1, 2), qi(2))) // D2: annulus (derived)
        .subtract(Cutter::vertical_cylinder(q(-9, 4), q(9, 4), q(9, 16))) // D3: rim notch (derived)
        .clearance(qi(1))
        .thickness(q(1, 8))
        .segments(segments);
    if with_cuts {
        part = part
            .subtract(Cutter::vertical_cylinder(qi(0), q(11, 5), q(1, 25))) // D4: hole (derived)
            .hole_domain(vec![
                (q(-9, 20), q(43, 20)),
                (q(-6, 20), q(43, 20)),
                (q(-6, 20), q(47, 20)),
                (q(-9, 20), q(47, 20)),
            ]);
    }
    part
}

fn main() {
    // — Arguments —
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut segments = 72usize;
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

    println!(
        "flex-PCB panel — device cone (β≈42°), gore σ∈[−7/2,7/2] (~296°), one Part, roles derived"
    );

    let flat = match panel(segments, true).develop() {
        Verdict::Verified(f) => f,
        Verdict::Refuted(fault) => {
            println!("develop: Refuted({fault:?}) — stopping");
            std::process::exit(1);
        }
        Verdict::Unresolved(e) => {
            println!(
                "develop: Unresolved at ε ≈ {:.3e} — raise clearance/segments",
                rat_to_f64(&e)
            );
            std::process::exit(1);
        }
    };
    for (k, op) in flat.report().ops.iter().enumerate() {
        println!(
            "op {k}:        {}  → {:?}   (derived)",
            if op.subtract {
                "subtract "
            } else {
                "intersect"
            },
            op.role
        );
    }
    println!(
        "develop:     Verified   ε ≈ {:.3e}   1 face · {} holes ({} outer verts)",
        rat_to_f64(flat.eps()),
        flat.region().faces[0].holes.len(),
        flat.region().faces[0].outer.len()
    );

    // — SVG —
    std::fs::create_dir_all(&out_dir).expect("create --out-dir");
    let svg = flat.svg(720);
    let svg_path = format!("{out_dir}/flex_panel.svg");
    std::fs::write(&svg_path, &svg).expect("write flex_panel.svg");
    println!("SVG:         wrote {svg_path}   ({} bytes)", svg.len());

    // — STEP I (the blank) + STEP II (the finished panel) —
    #[cfg(feature = "step")]
    {
        let emit =
            |name: &str, with_cuts: bool, path: String| match panel(segments, with_cuts).solid() {
                Verdict::Verified(solid) => {
                    let report = solid.write_step(&path);
                    println!("{name}   {}   → {path}", report.summary());
                }
                Verdict::Refuted(fault) => println!("{name}   Refuted({fault:?})"),
                Verdict::Unresolved(e) => {
                    println!("{name}   Unresolved at ε ≈ {:.3e}", rat_to_f64(&e));
                }
            };
        emit("STEP I: ", false, format!("{out_dir}/flex_panel_I.step"));
        emit("STEP II:", true, format!("{out_dir}/flex_panel_II.step"));
    }
    #[cfg(not(feature = "step"))]
    println!("STEP:        skipped — build under `nix develop` with `--features step`");
}
