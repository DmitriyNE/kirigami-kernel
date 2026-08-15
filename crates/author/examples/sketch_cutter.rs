//! The **sketch-extrude cutter** (AUTH.1) on the construction facade — a *drafted* hole.
//!
//! The Stage-1 device-cone gore, authored as a [`Part`] exactly as `flex_panel` does, but with the
//! interior feature cut by [`Cutter::extrude`]: a profile drawn in a rational frame and swept from
//! a homogeneous apex. Here the apex is a finite **cast point**, so the wall is a cone and the hole
//! narrows with depth — the draft angle the milestone is named for. Pushing the cast point outward
//! degrades it continuously to the parallel drill; there is no discontinuity at "parallel", because
//! parallel is `w = 0`.
//!
//! For comparison the same panel is emitted with the hole authored as a **direction** sweep, which
//! is exactly `Cutter::vertical_cylinder` — the two agree to the digit, which is the point: the
//! general cutter reproduces the special one it generalizes.
//!
//! **Not yet.** A profile of several walls — a polygonal slot, a ring — resolves correctly but
//! cannot be *realized*: the hole loop is built from one surface's two µ̂-branches, and a
//! multi-wall footprint's boundary switches between walls at the profile's corners. See AUTH.1e.4.
//!
//! ```text
//! cargo run --example sketch_cutter                                   # panel + SVG
//! nix develop -c cargo run --example sketch_cutter --features step    # + STEP
//! ```
//!
//! Flags: `--segments <n>` (rail discretization, default 72), `--out-dir <dir>` (default
//! `generated-demos/`).

use arrange2d::profile::Profile;
use author::construct;
use author::part::{Cutter, Part, SupportFn};
use certify_core::Verdict;
use develop::extrude::{Apex, Frame};
use export::approx::rat_to_f64;
use fixtures::devices::cone;
use geom::content::Edge;
use lattice::{Bignum, Rat};

type Q = Rat<Bignum>;

fn q(n: i128, d: i128) -> Q {
    Q::new(n, d)
}
fn qi(n: i128) -> Q {
    Q::from_i128(n)
}

/// A disc's boundary, through the shared [`Profile`] builder.
///
/// This used to be twenty lines of hand-built `ArcPiece`s — the ergonomics gap this demo made
/// visible. `Profile` closed it: each constructor builds a `Curve` and hands it to `arrange2d`'s
/// own `decompose`, so nothing here re-derives an arc.
fn disc(cx: Q, cy: Q, r: Q) -> Vec<Edge<Bignum>> {
    Profile::new().circle(cx, cy, r).into_edges()
}

/// The `z = 0` sketch plane in world coordinates, orthonormal so a profile circle is a true circle.
fn sketch_plane() -> Frame<Bignum> {
    Frame::new(
        [qi(0), qi(0), qi(0)],
        [qi(1), qi(0), qi(0)],
        [qi(0), qi(1), qi(0)],
    )
    .expect("the axes are independent")
}

/// The panel recipe. `with_cuts` adds the two extruded features (off = the blank).
fn panel(segments: usize, with_cuts: bool, apex: Apex<Bignum>) -> Part<Bignum> {
    let witness = cone()
        .surface(&qi(2), &qi(0))
        .eval(&qi(0))
        .expect("the device cone is regular at σ = 0");
    let mut part = construct::from_chart::<Bignum>(&cone())
        .region_sigma(q(-7, 2), q(7, 2), SupportFn::inherit())
        .keep_near(witness)
        .intersect(Cutter::half_space([qi(0), qi(0), qi(1)], qi(3))) // bound the blank
        .subtract(Cutter::vertical_cylinder(qi(0), q(1, 2), qi(2))) // the annulus
        .subtract(Cutter::vertical_cylinder(q(-9, 4), q(9, 4), q(9, 16))) // rim notch
        .clearance(qi(1))
        .thickness(q(1, 8))
        .segments(segments);
    if with_cuts {
        // The interior hole, swept from the given apex. With a finite cast point the wall is a
        // cone, so the cut narrows the further the sheet sits from the sketch plane.
        part = part.subtract(Cutter::extrude(
            sketch_plane(),
            apex,
            disc(qi(0), q(11, 5), q(1, 5)),
        ));
    }
    part
}

/// The widest extent of the first interior hole in the developed pattern — the measurable the
/// faithfulness check compares, since a drafted cut must come out smaller than a parallel one.
///
/// Goes through `export::svg::region_to_polys`, the **quarantined** exact→`f64` bridge, rather than
/// converting coordinates by hand: a profile boundary's endpoints can be algebraic and their
/// rational brackets large enough that a naive `rat_to_f64` returns NaN — which `min`/`max` then
/// swallow, turning a real measurement into a silent "could not measure".
fn hole_width(flat: &author::part::FlatPattern<Bignum>) -> Option<f64> {
    let polys = export::svg::region_to_polys(flat.region());
    // The outer ring comes first, so the first interior ring is the hole.
    let ring = polys.faces.first()?.rings.get(1)?;
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for p in ring {
        if p[0].is_finite() {
            lo = lo.min(p[0]);
            hi = hi.max(p[0]);
        }
    }
    (hi > lo).then_some(hi - lo)
}

fn main() {
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

    println!("sketch-extrude cutter — device cone (β≈42°), gore σ∈[−7/2,7/2] (~296°)");
    println!("  a DRAFTED hole: a profile disc swept from the cast point (0, 11/5, 12)");

    let drafted = Apex::point([qi(0), q(11, 5), qi(12)]);
    let parallel = Apex::direction([qi(0), qi(0), qi(1)]).expect("a real direction");
    let flat = match panel(segments, true, drafted.clone()).develop() {
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

    std::fs::create_dir_all(&out_dir).expect("create --out-dir");
    let svg = flat.svg(720);
    let svg_path = format!("{out_dir}/sketch_cutter.svg");
    std::fs::write(&svg_path, &svg).expect("write sketch_cutter.svg");
    println!("SVG:         wrote {svg_path}   ({} bytes)", svg.len());

    // The same hole swept PARALLEL is the metric cylinder — the general cutter reproducing the
    // special one. But ε cannot tell the two apart: it is the max over stages and the boundary
    // dominates, so both report the same number. The faithfulness check is therefore GEOMETRIC —
    // the drafted hole must come out SMALLER, by the taper its cast point implies.
    match panel(segments, true, parallel).develop() {
        Verdict::Verified(par) => {
            println!(
                "parallel:    Verified   ε ≈ {:.3e}   (= Cutter::vertical_cylinder)",
                rat_to_f64(par.eps())
            );
            match (hole_width(&flat), hole_width(&par)) {
                (Some(a), Some(b)) if b > 0.0 => println!(
                    "faithful:    drafted {a:.4} vs parallel {b:.4}   ratio {:.3}   (taper law ≈ 0.80)",
                    a / b
                ),
                _ => println!("faithful:    could not measure the hole"),
            }
        }
        other => println!(
            "parallel:    not Verified ({})",
            match other {
                Verdict::Refuted(f) => format!("Refuted({f:?})"),
                _ => "Unresolved".into(),
            }
        ),
    }

    #[cfg(feature = "step")]
    {
        let emit = |name: &str, with_cuts: bool, path: String| match panel(
            segments,
            with_cuts,
            drafted.clone(),
        )
        .solid()
        {
            Verdict::Verified(solid) => {
                let report = solid.write_step(&path);
                println!("{name}   {}   → {path}", report.summary());
            }
            Verdict::Refuted(fault) => println!("{name}   Refuted({fault:?})"),
            Verdict::Unresolved(e) => {
                println!("{name}   Unresolved at ε ≈ {:.3e}", rat_to_f64(&e));
            }
        };
        emit("STEP I: ", false, format!("{out_dir}/sketch_cutter_I.step"));
        emit("STEP II:", true, format!("{out_dir}/sketch_cutter_II.step"));
    }
    #[cfg(not(feature = "step"))]
    println!("STEP:        skipped — build under `nix develop` with `--features step`");
}
