//! Emit the **flex-PCB panel** — the cone trimmed by an *arrangement of vertical cylinders*
//! authored in the physical xy-plane, developed to a certified flat pattern (the Stage-1 flat
//! deliverable, xy-trimming rebuild).
//!
//! The 300°-ish device-cone gore is trimmed by `(D1 − D2) − D3 − D4` (all disks in xy):
//! **D1** concentric outer (an exact `{z=d}` plane cut), **D2** eccentric inner containing the
//! apex (a cone∩cylinder cut → the eccentric annulus), **D3** a boundary **notch** straddling
//! the rim, **D4** an interior circular **hole**. Each disk is pulled back to a certified
//! ruling-rail `μ̂(σ)` (G2: float oracle proposes, `cut_fit` decides), the boundary loops are
//! **unrolled** (`develop::unroll`, ①) to flat polylines, and the panel is stitched together by
//! the exact `arrange2d` boolean (`BoolOp::Diff`), with a polygon (quad) also cut. Every stage
//! prints a certified verdict, so **running it is a stress report**.
//!
//! The physical-xy arrangement itself is certified up front (the same `BoolOp::Diff`): the disks
//! must produce exactly one face with two holes (D2, D4) and a D3 rim notch, else the run stops.
//!
//! Artifacts (under `--out-dir`, default `generated-demos/`, gitignored): `flex_panel.svg` (the
//! developed trimmed panel, even-odd fill). Behind `--features step` under `nix develop`, the
//! **legacy** band+rectangle-hole solids `flex_panel_I.step` / `flex_panel_II.step` are still
//! emitted — the trimmed-geometry STEP export needs the curved-rail B-rep builder (Stage B).
//!
//! ```text
//! cargo run --example flex_panel --features diagnostics                       # panel + SVG
//! nix develop -c cargo run --example flex_panel --features diagnostics,step    # + legacy STEP
//! ```
//!
//! Flags: `--segments <n>` (rail discretization, default 72), `--out-dir <dir>`. The gore is the
//! moderate `σ ∈ [−1, 1]` (~180°) at band scale — a *cut* (circular) boundary is a varying-μ̂
//! rail, so a large radius / a wider gore blows μ̂ up and the interval development goes loose (a
//! logged strain; the constant-μ band is exempt).

use certify_core::Verdict;
use develop::cone::{ConeDevelopment, DevConfig};
use export::approx::rat_to_f64;
use export::cut_oracle::RootPick;
use export::svg::{Bounds, polys_svg, region_to_polys};
use export::trim::{
    RailFit, assemble_flat, concentric_disk, eccentric_disk, flat_to_poly, hole_loop, outer_loop,
    unroll_loop,
};
use fixtures::devices::cone;
use lattice::{Bignum, Interval, Rat};

type Q = Rat<Bignum>;

fn e3(r: &Q) -> f64 {
    rat_to_f64(r)
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

    let chart = cone();
    let dev = ConeDevelopment::new(&chart).expect("the device cone is a canonical arctan cone");
    let cfg = DevConfig::tight();
    let clearance = Q::from_i128(1);
    let span = Interval {
        lo: Q::from_i128(-1),
        hi: Q::from_i128(1),
    };
    let fit = RailFit::default();

    // — The trimming disks (physical xy, around +y — the gore develops the upper half-plane) —
    let d1 = concentric_disk(&chart, &Q::from_i128(3)).expect("concentric plane rail"); // outer
    let d2 = eccentric_disk(
        Q::from_i128(0),
        Q::new(1, 2),
        Q::from_i128(2),
        RootPick::Upper,
    ); // inner
    let d3 = [Q::new(-9, 4), Q::new(9, 4), Q::new(9, 16)]; // boundary notch (straddles D1)
    let d4 = [Q::from_i128(0), Q::new(11, 5), Q::new(1, 25)]; // interior hole

    println!("flex-PCB panel — device cone (β≈42°), gore σ∈[−1,1], trim (D1−D2)−D3−D4 in xy");

    // — Certify the physical-xy arrangement topology (BoolOp::Diff) —
    certify_arrangement(&d1, &d2, &d3, &d4);

    // — Outer boundary: eccentric annulus + D3 notch → unroll —
    let outer = match outer_loop(
        &chart,
        &d1,
        &d2,
        (&d3[0], &d3[1], &d3[2]),
        &span,
        fit,
        &clearance,
        &cfg,
        &Q::new(1, 20),
        segments,
    ) {
        Verdict::Verified(o) => {
            println!(
                "outer rail:  Verified   ε≈{:.3e}   D1∩D3 micro-cap≈{:.2e}",
                e3(&o.eps),
                e3(&o.max_microcap)
            );
            o
        }
        v => bail("outer boundary", &v),
    };
    let outer_flat = match unroll_loop(&dev, &outer.arcs, &cfg, &clearance) {
        Verdict::Verified(o) => {
            println!(
                "outer unroll:Verified   ε≈{:.3e}   ({} flat verts)",
                e3(&o.eps),
                o.vertices.len()
            );
            o
        }
        v => bail("outer unroll", &v),
    };

    // — D4 interior hole → unroll —
    let hole = match hole_loop(
        &chart,
        &d4[0],
        &d4[1],
        &d4[2],
        &span,
        fit,
        &clearance,
        &cfg,
        &Q::new(1, 200),
        segments / 2,
    ) {
        Verdict::Verified(h) => {
            println!(
                "D4 hole:     Verified   ε≈{:.3e}   tangent micro-cap≈{:.2e}",
                e3(&h.eps),
                e3(&h.max_microcap)
            );
            h
        }
        v => bail("D4 hole", &v),
    };
    let hole_flat = match unroll_loop(&dev, &hole.arcs, &cfg, &clearance) {
        Verdict::Verified(o) => o,
        v => bail("D4 hole unroll", &v),
    };

    // — An authored quad cut (developed from (σ,μ), landing in the band left of D4) —
    let quad: Vec<[Q; 2]> = [
        (Q::new(-9, 20), Q::new(43, 20)),
        (Q::new(-6, 20), Q::new(43, 20)),
        (Q::new(-6, 20), Q::new(47, 20)),
        (Q::new(-9, 20), Q::new(47, 20)),
    ]
    .iter()
    .map(|(s, m)| {
        let (x, y) = dev.point(s, m, &cfg).center();
        [x, y]
    })
    .collect();

    // — Assemble the flat panel: outer − (D4 ∪ quad) via the exact BoolOp::Diff —
    let outer_poly = flat_to_poly(&outer_flat);
    let hole_poly = flat_to_poly(&hole_flat);
    let region = match assemble_flat(&outer_poly, &[hole_poly, quad]) {
        Verdict::Verified(r) => {
            println!(
                "assemble:    Verified   1 face · {} holes (D4 + quad)",
                r.faces[0].holes.len()
            );
            r
        }
        v => bail("flat assembly", &v),
    };

    // — SVG (even-odd fill; the holes cut out) —
    let polys = region_to_polys(&region);
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
        "SVG:         wrote {svg_path}   ({} rings, {} bytes)",
        polys.faces.iter().map(|f| f.rings.len()).sum::<usize>(),
        svg.len()
    );

    // — Legacy STEP (band + rectangle hole) — the trimmed-geometry STEP is Stage B (G-C) —
    #[cfg(feature = "step")]
    emit_step_legacy(&chart, &out_dir);
    #[cfg(not(feature = "step"))]
    println!("STEP:        skipped — the trimmed-geometry STEP export is Stage B (curved builder)");
}

/// Certify the physical-xy arrangement `(D1 − D2) − D3 − D4` with the exact `BoolOp::Diff`: the
/// authored disks must resolve to exactly one face with two holes (D2, D4) and a D3 rim notch.
fn certify_arrangement(
    d1: &export::trim::TrimDisk<Bignum>,
    d2: &export::trim::TrimDisk<Bignum>,
    d3: &[Q; 3],
    d4: &[Q; 3],
) {
    use arrange2d::boolean::{BoolOp, OperandId, ledge_dom_certified};
    use geom::content::{Circle, Curve, CurveId, Orient};
    let disks = [
        (d1.cx.clone(), d1.cy.clone(), d1.r2.clone()), // A: D1
        (d2.cx.clone(), d2.cy.clone(), d2.r2.clone()), // B: D2
        (d3[0].clone(), d3[1].clone(), d3[2].clone()), // B: D3
        (d4[0].clone(), d4[1].clone(), d4[2].clone()), // B: D4
    ];
    let mut edges = Vec::new();
    for (i, (cx, cy, r2)) in disks.iter().enumerate() {
        edges.extend(arrange2d::decompose::decompose(&Curve::Circle {
            circle: Circle {
                cx: cx.clone(),
                cy: cy.clone(),
                r2: r2.clone(),
            },
            orient: Orient::Ccw,
            source: CurveId(i as u32),
        }));
    }
    let operand_of = |c: CurveId| {
        if c.0 == 0 { OperandId::A } else { OperandId::B }
    };
    match ledge_dom_certified(&edges, &operand_of, BoolOp::Diff) {
        Verdict::Verified(cap) => {
            let r = cap.region();
            assert_eq!(r.faces.len(), 1, "arrangement is one face");
            assert_eq!(r.faces[0].holes.len(), 2, "two holes (D2, D4)");
            println!("arrangement: Verified   (D1−D2)−D3−D4 = 1 face, 2 holes, D3 rim notch");
        }
        other => {
            println!("arrangement: NOT certified — {}", verdict_tag(&other));
            std::process::exit(1);
        }
    }
}

fn verdict_tag<T, E: core::fmt::Debug, M>(v: &Verdict<T, E, M>) -> String {
    match v {
        Verdict::Verified(_) => "Verified".into(),
        Verdict::Refuted(w) => format!("Refuted({w:?})"),
        Verdict::Unresolved(_) => "Unresolved".into(),
    }
}

fn bail<T, E: core::fmt::Debug, M>(stage: &str, v: &Verdict<T, E, M>) -> ! {
    println!("{stage}: {} — stopping", verdict_tag(v));
    std::process::exit(1);
}

/// Emit the **legacy** band+rectangle-hole STEP solids (STEP I cone slab, STEP II genus-1
/// through-hole) — unchanged from the pre-trimming demo. The trimmed-panel STEP export needs the
/// curved-rail B-rep builder (Stage B / gap G-C); this keeps `--features step` green meanwhile.
#[cfg(feature = "step")]
fn emit_step_legacy(chart: &geom::chart::Chart<Bignum>, out_dir: &str) {
    use export::brep_build::{HoleRect, brep_freeboundary, brep_freeboundary_holed};
    use export::step::write_brep;
    use lattice::{Poly, RatFunc};

    let span = Interval {
        lo: Q::new(-15, 4),
        hi: Q::new(15, 4),
    };
    let mu_lo = RatFunc::<Bignum>::from_poly(Poly::constant(Q::from_i128(-2)));
    let mu_hi = RatFunc::<Bignum>::from_poly(Poly::constant(Q::from_i128(-1)));
    let w_iv = Interval {
        lo: Q::from_i128(0),
        hi: Q::new(1, 8),
    };
    println!("--- legacy STEP (band + rectangle hole; trimmed-geometry STEP is Stage B) ---");
    let solid = brep_freeboundary(chart, &span, &w_iv, &mu_lo, &mu_hi);
    let p1 = format!("{out_dir}/flex_panel_I.step");
    println!("STEP I:      {:<40}   → {p1}", write_brep(&p1, &solid));

    let sigma_mid = Q::from_i128(0);
    let hole = HoleRect {
        sigma: Interval {
            lo: sigma_mid.sub(&Q::new(3, 16)),
            hi: sigma_mid.add(&Q::new(3, 16)),
        },
        mu: Interval {
            lo: Q::new(-25, 16),
            hi: Q::new(-23, 16),
        },
    };
    match brep_freeboundary_holed(chart, &span, &w_iv, &mu_lo, &mu_hi, &[hole]) {
        Some(solid2) => {
            let p2 = format!("{out_dir}/flex_panel_II.step");
            println!("STEP II:     {:<40}   → {p2}", write_brep(&p2, &solid2));
            println!(
                "             (genus-1 through-hole solid: {} faces, {} free edges)",
                solid2.faces().len(),
                solid2.free_edges()
            );
        }
        None => println!("STEP II:     refused — hole not strictly interior"),
    }
}
