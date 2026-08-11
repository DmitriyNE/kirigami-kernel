//! Emit the **Stage-1 flex-PCB panel** end-to-end — the certified pipeline finale (G7).
//!
//! Drives the whole Milestone-E development chain over a *wide two-sided* device-cone gore and
//! prints a per-stage certified verdict, so **running it is a stress report**:
//!
//! 1. **unroll** (`develop::unroll`, ①) — develop the μ-band boundary loop to a flat polyline;
//! 2. **hole** (`develop::flat::cut_hole`, exact `arrange2d` boolean) — cut the interior hole (the
//!    development of the authored `(σ,μ)` rectangle, the *same* one STEP II drills);
//! 3. **fold** (`develop::fold`, ②) — fold the flat outline *and* the hole back to 3-D;
//! 4. **STEP** (`export::brep_build` + the OCCT bridge) — write the input cone solid (STEP I) and
//!    the folded panel carrying the hole as a real interior wire (STEP II).
//!
//! Artifacts (written under `--out-dir`, default `generated-demos/`, gitignored): `flex_panel.svg`
//! (the flat pattern with the hole cut out, even-odd fill), and — behind `--features step`, under
//! `nix develop` — `flex_panel_I.step` (cone solid) and `flex_panel_II.step` (folded holed panel).
//!
//! ```text
//! cargo run --example flex_panel --features diagnostics                 # pipeline + A2 SVG
//! nix develop -c cargo run --example flex_panel --features diagnostics,step   # + STEP I/II
//! ```
//!
//! Flags: `--sigma S` (gore half-span σ∈[−S,S], default `15/4` ≈ 300° 3-D sweep — the wide target;
//! accepts `n` or `n/d`), `--segments <n>` (rail discretization, default 96), `--iters <n>` (fold
//! bisection depth, default 64), `--out-dir <dir>`. A rational cone chart sweeps `< 2π`, so the gore
//! widens with `S`; this is deliberately a *strain* case (see the engineering-log G7 note).

use certify_core::Verdict;
use develop::cone::{ConeDevelopment, DevConfig};
use develop::flat::cut_hole;
use develop::fold::{FoldedWire, fold_outline};
use develop::unroll::unroll_freeboundary;
use export::approx::rat_to_f64;
use export::svg::{Bounds, polys_svg, region_to_polys};
use fixtures::devices::cone;
use lattice::{Bignum, Interval, Poly, Rat, RatFunc};

fn parse_rat(s: &str) -> Rat<Bignum> {
    match s.split_once('/') {
        Some((n, d)) => Rat::new(
            n.trim().parse().expect("--sigma numerator"),
            d.trim().parse().expect("--sigma denominator"),
        ),
        None => Rat::from_i128(s.trim().parse().expect("--sigma integer")),
    }
}

fn main() {
    // — Arguments —
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut sigma = "15/4".to_string();
    let mut lo: Option<String> = None;
    let mut segments = 96usize;
    let mut iters = 64usize;
    let mut out_dir = "generated-demos".to_string();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--sigma" => {
                i += 1;
                sigma = args.get(i).cloned().expect("--sigma expects a value");
            }
            "--lo" => {
                i += 1;
                lo = Some(args.get(i).cloned().expect("--lo expects a value"));
            }
            "--segments" => {
                i += 1;
                segments = args
                    .get(i)
                    .and_then(|s| s.parse().ok())
                    .expect("--segments expects an integer");
            }
            "--iters" => {
                i += 1;
                iters = args
                    .get(i)
                    .and_then(|s| s.parse().ok())
                    .expect("--iters expects an integer");
            }
            "--out-dir" => {
                i += 1;
                out_dir = args.get(i).cloned().expect("--out-dir expects a path");
            }
            other => {
                eprintln!("unknown argument `{other}`");
                std::process::exit(1);
            }
        }
        i += 1;
    }

    let s = parse_rat(&sigma);
    let lo = match &lo {
        Some(l) => parse_rat(l),
        None => Rat::from_i128(0).sub(&s), // default: symmetric two-sided gore [−S, S]
    };
    let span = Interval {
        lo: lo.clone(),
        hi: s.clone(),
    };
    let sigma_mid = span.lo.add(&span.hi).mul(&Rat::new(1, 2)); // hole placed at the σ-midpoint
    let chart = cone();
    let dev = ConeDevelopment::new(&chart).expect("the device cone is a canonical arctan cone");
    let cfg = DevConfig::tight();
    // The retained band: outer rail μ⁻ ≡ −2, inner rail μ⁺ ≡ −1 (negative side ⇒ no apex μ̂=0
    // crossing). Both constant RatFuncs.
    let mu_lo = RatFunc::<Bignum>::from_poly(Poly::constant(Rat::from_i128(-2)));
    let mu_hi = RatFunc::<Bignum>::from_poly(Poly::constant(Rat::from_i128(-1)));
    // A generous fab clearance so the demo certifies; the report shows the *achieved* ε per stage.
    let clearance = Rat::from_i128(1);

    // One hole authored in the sheet's `(σ, μ)` domain, centred on the σ-midline at mid-band μ = −3/2.
    // The *same* rectangle drives both the flat cut (via its development) and the STEP-II drill, so the
    // two stay coherent — and on a symmetric gore its centre σ = 0 is a positive-weight **station**,
    // the exact placement the single-slice builder mis-drilled (off-centre, distorted). The general
    // arrangement construction cuts it per slice and splits its tube at the station.
    let hole_sigma_half = span.hi.sub(&span.lo).mul(&Rat::new(1, 40));
    let hole_sigma = Interval {
        lo: sigma_mid.sub(&hole_sigma_half),
        hi: sigma_mid.add(&hole_sigma_half),
    };
    let hole_mu = Interval {
        lo: Rat::new(-25, 16),
        hi: Rat::new(-23, 16),
    };

    println!(
        "flex-PCB Stage-1 panel — device cone (β≈42°), gore σ∈[{:.4},{:.4}], band μ∈[−2,−1]\n\
         segments={segments} iters={iters}",
        rat_to_f64(&span.lo),
        rat_to_f64(&span.hi)
    );

    // — Stage 1: unroll (develop ①) —
    let outline = match unroll_freeboundary(&dev, &span, &mu_lo, &mu_hi, segments, &cfg, &clearance)
    {
        Verdict::Verified(o) => {
            println!(
                "unroll:     Verified    ε≈{:.3e}   ({} flat verts)",
                rat_to_f64(&o.eps),
                o.vertices.len()
            );
            o
        }
        Verdict::Unresolved(e) => {
            println!(
                "unroll:     Unresolved  ε≈{:.3e} ≥ clearance/2 — raise --segments (finer rails)",
                rat_to_f64(&e)
            );
            std::process::exit(1);
        }
        Verdict::Refuted(f) => {
            println!("unroll:     Refuted     {f:?}");
            std::process::exit(1);
        }
    };

    // — Stage 2: cut the interior hole (develop::flat + exact arrange2d) —
    // The flat hole is the **development of the authored (σ,μ) rectangle**: its four corners mapped
    // through the exact cone development, so the flat cut and the STEP-II drill are one hole.
    let square: Vec<[Rat<Bignum>; 2]> = [
        (&hole_sigma.lo, &hole_mu.lo),
        (&hole_sigma.hi, &hole_mu.lo),
        (&hole_sigma.hi, &hole_mu.hi),
        (&hole_sigma.lo, &hole_mu.hi),
    ]
    .iter()
    .map(|&(s, m)| {
        let (x, y) = dev.point(s, m, &cfg).center();
        [x, y]
    })
    .collect();
    let holed = match cut_hole(&outline, &square) {
        Verdict::Verified(h) => {
            println!("hole:       Verified    (1 face · 1 hole · no pinch)");
            h
        }
        Verdict::Unresolved(()) => {
            println!("hole:       Unresolved");
            std::process::exit(1);
        }
        Verdict::Refuted(f) => {
            println!("hole:       Refuted     {f:?}");
            std::process::exit(1);
        }
    };

    // — A2: SVG of the flat pattern with the hole cut out (even-odd fill) —
    let polys = region_to_polys(&holed.region);
    let frame = Bounds::of_points(
        polys
            .faces
            .iter()
            .flat_map(|f| f.rings.iter().flatten().copied()),
    );
    let svg = polys_svg(&polys, &frame, 720);
    std::fs::create_dir_all(&out_dir).expect("create --out-dir");
    let svg_path = format!("{out_dir}/flex_panel.svg");
    std::fs::write(&svg_path, &svg).expect("write flex_panel.svg");
    println!(
        "A2 SVG:     wrote {svg_path}   ({} rings, {} bytes)",
        polys.faces.iter().map(|f| f.rings.len()).sum::<usize>(),
        svg.len()
    );

    // — Stage 3: fold the flat outline + the hole back to 3-D (develop ②, σ=0 split) —
    let flat_outer: Vec<[Rat<Bignum>; 2]> = outline
        .vertices
        .iter()
        .map(|b| {
            let (x, y) = b.center();
            [x, y]
        })
        .collect();
    let w0 = Rat::from_i128(0);
    let do_fold = |label: &str, flat: &[[Rat<Bignum>; 2]]| -> FoldedWire<Bignum> {
        match fold_outline(&chart, flat, &w0, &span, iters, true, &cfg, &clearance) {
            Verdict::Verified(w) => {
                println!(
                    "{label} Verified    ε≈{:.3e}   ({} wire verts)",
                    rat_to_f64(&w.eps),
                    w.points.len()
                );
                w
            }
            Verdict::Unresolved(e) => {
                println!(
                    "{label} Unresolved  ε≈{:.3e} ≥ clearance/2 — raise --iters",
                    rat_to_f64(&e)
                );
                std::process::exit(1);
            }
            Verdict::Refuted(f) => {
                println!("{label} Refuted     {f:?}");
                std::process::exit(1);
            }
        }
    };
    let fw_outer = do_fold("fold-outer:", &flat_outer);
    let fw_hole = do_fold("fold-hole: ", &square);

    // — Stage 4: STEP I (cone solid) + STEP II (folded holed panel) —
    #[cfg(feature = "step")]
    emit_step(
        &chart,
        &span,
        &mu_lo,
        &mu_hi,
        &hole_sigma,
        &hole_mu,
        &fw_outer,
        &fw_hole,
        &out_dir,
    );
    #[cfg(not(feature = "step"))]
    {
        let _ = (&fw_outer, &fw_hole);
        println!("STEP:       skipped — build `--features step` under `nix develop` for A4/A5");
    }
}

/// Emit STEP I — the input cone gore as a thin closed solid via [`brep_freeboundary`] — and STEP II
/// — the **same slab with a real through-hole** via [`brep_freeboundary_holed`]. Both auto-subdivide
/// σ into positive-weight single-span-Bézier slices (a wide two-sided gore needs it). STEP II drills
/// the *same* `(σ, μ)` rectangle the flat cut used, at its authored location — on a symmetric gore
/// that centre σ = 0 is a subdivision station, so the general arrangement construction cuts the hole
/// per slice (a **notch** into each) and splits its tube at the station. The result is a certified
/// **genus-1** solid, exported through OCCT `MakeFace` inner wires and written through the OCCT bridge
/// ([`write_brep`] = write-then-reload-through-BRepCheck), which prints `ok` or `error: <what>` — a
/// rejection is reported, not hidden.
#[cfg(feature = "step")]
#[allow(clippy::too_many_arguments)]
fn emit_step(
    chart: &geom::chart::Chart<Bignum>,
    span: &Interval<Bignum>,
    mu_lo: &RatFunc<Bignum>,
    mu_hi: &RatFunc<Bignum>,
    hole_sigma: &Interval<Bignum>,
    hole_mu: &Interval<Bignum>,
    fw_outer: &FoldedWire<Bignum>,
    fw_hole: &FoldedWire<Bignum>,
    out_dir: &str,
) {
    use export::brep_build::{HoleRect, brep_freeboundary, brep_freeboundary_holed};
    use export::step::write_brep;
    // The folded wires are the 3-D preview of the flat pattern + hole (Stage 3); the exact solids
    // below are reconstructed from the chart in (σ, μ), so the wires are only reported here.
    let _ = (fw_outer, fw_hole);

    // A thin thickness window w ∈ [0, 1/8]; the hole runs through it.
    let w_iv = Interval {
        lo: Rat::from_i128(0),
        hi: Rat::new(1, 8),
    };

    // STEP I — the input cone gore as a thin closed solid. `brep_freeboundary` auto-subdivides σ so
    // every ruled Bézier patch has positive weights (a wide two-sided gore needs it), assembling one
    // watertight N-slice solid — no σ=0 special case.
    let solid = brep_freeboundary(chart, span, &w_iv, mu_lo, mu_hi);
    let p1 = format!("{out_dir}/flex_panel_I.step");
    println!("STEP I:     {:<40}   → {p1}", write_brep(&p1, &solid));

    // STEP II — the same slab with the hole drilled at its authored `(σ, μ)` rectangle (the one the
    // flat cut developed). On the symmetric gore its centre σ = 0 is a station; the arrangement cuts
    // it per slice and splits its tube there — the hole lands exactly where the SVG shows it.
    let hole = HoleRect {
        sigma: hole_sigma.clone(),
        mu: hole_mu.clone(),
    };
    match brep_freeboundary_holed(chart, span, &w_iv, mu_lo, mu_hi, &[hole]) {
        Some(solid2) => {
            let p2 = format!("{out_dir}/flex_panel_II.step");
            println!("STEP II:    {:<40}   → {p2}", write_brep(&p2, &solid2));
            println!(
                "            (genus-1 through-hole solid: {} faces, {} free edges)",
                solid2.faces().len(),
                solid2.free_edges()
            );
        }
        None => println!(
            "STEP II:    refused — the authored hole is not strictly interior to the panel"
        ),
    }
}
