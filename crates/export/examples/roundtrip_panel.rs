//! Emit the **round-trip flex-PCB panel** (DD.1): a cone gore whose **boundaries are cut in 3D**
//! (an eccentric annulus), developed to a certified flat pattern, on which an **interior ECAD
//! feature is authored in 2-D and folded back** onto the cone — the fold-back leg the Stage-1
//! `flex_panel` demo skips (it only lifts cuts *forward*).
//!
//! The pipeline, every stage a certified verdict (running it is a stress report):
//! **3D-cut boundary** (`export::trim`: a concentric plane cut `D1` + an eccentric cone∩cylinder
//! cut `D2` → an annulus) → **develop** (`develop::unroll`, direction ①) → **author an interior
//! feature on the flat pattern** (a rectangle in developed `(x,y)`) → **fold back** onto the cone
//! (`develop::fold::fold_outline`, direction ②) → a certified 3-D wire `C(σ, μ̂, w)`.
//!
//! Artifacts (under `--out-dir`, default `generated-demos/`, gitignored): `roundtrip_panel.svg`
//! (the developed annulus with the interior feature cut, even-odd fill). Behind `--features step`
//! under `nix develop`, the annulus is also written as a certified curved-rail cone solid
//! `roundtrip_panel.step` (`brep_trim_solid`, `closed_shell_holed`-certified + OCCT-corroborated).
//! Cutting the *folded* interior feature through the B-rep (fitting its recovered `(σ, μ̂)` to hole
//! rails) is the DD.4 device-assembly step; here the folded feature is the certified 3-D wire.
//!
//! ```text
//! cargo run --example roundtrip_panel --features diagnostics                    # panel + SVG
//! nix develop -c cargo run --example roundtrip_panel --features diagnostics,step # + annulus STEP
//! ```
//!
//! Flags: `--segments <n>` (rail discretization, default 48), `--out-dir <dir>`. The gore is
//! `σ ∈ [−1, 1]` ≈ **180°** (`φ = 2·arctan σ`), a comfortable well-conditioned band for the fold.

use certify_core::Verdict;
use develop::cone::{ConeDevelopment, DevConfig};
use develop::fold::fold_outline;
use export::approx::rat_to_f64;
use export::cut_oracle::RootPick;
use export::svg::{Bounds, polys_svg, region_to_polys};
use export::trim::{
    RailFit, annulus_loop, assemble_flat, certified_rail, concentric_disk, eccentric_disk,
    flat_to_poly, unroll_loop,
};
use fixtures::devices::cone;
use lattice::{Bignum, Interval, Rat};

type Q = Rat<Bignum>;

fn e3(r: &Q) -> f64 {
    rat_to_f64(r)
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

fn main() {
    // — Arguments —
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut segments = 48usize;
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

    println!("round-trip flex-PCB panel — device cone (β≈42°), gore σ∈[−1,1] (~180°)");
    println!("  boundaries cut in 3D · developed · interior feature authored in 2D + folded back");

    // — 3D-cut boundary: an eccentric annulus (D1 concentric plane cut, D2 eccentric inner cut) —
    let d1 = concentric_disk(&chart, &Q::from_i128(3)).expect("concentric plane rail");
    let d2 = eccentric_disk(
        Q::from_i128(0),
        Q::new(1, 2),
        Q::from_i128(2),
        RootPick::Upper,
    );
    let (mu_out, e_out) = match certified_rail(&chart, &d1, &span, fit, &clearance, &cfg) {
        Verdict::Verified(x) => x,
        v => bail("D1 outer rail", &v),
    };
    let (mu_in, e_in) = match certified_rail(&chart, &d2, &span, fit, &clearance, &cfg) {
        Verdict::Verified(x) => x,
        v => bail("D2 inner rail", &v),
    };
    println!(
        "boundary:    Verified   D1 ε≈{:.3e}   D2 ε≈{:.3e}   (eccentric annulus, cut in 3D)",
        e3(&e_out),
        e3(&e_in)
    );
    let arcs = annulus_loop(&mu_in, &mu_out, &span, segments).expect("annulus rails have no pole");

    // — Develop the boundary to the flat pattern (direction ①) —
    let panel = match unroll_loop(&dev, &arcs, &cfg, &clearance) {
        Verdict::Verified(o) => {
            println!(
                "develop:     Verified   ε≈{:.3e}   ({} flat verts)",
                e3(&o.eps),
                o.vertices.len()
            );
            o
        }
        v => bail("develop", &v),
    };

    // — Author an interior ECAD feature on the FLAT pattern: a rectangle placed around the
    //   developed gore centre (σ=0, mid-band). The fold (below) is the DD.1 leg under test. —
    let mid = mu_in
        .eval(&Q::from_i128(0))
        .and_then(|a| {
            mu_out
                .eval(&Q::from_i128(0))
                .map(|b| a.add(&b).mul(&Q::new(1, 2)))
        })
        .expect("mid-band μ̂ at σ=0");
    let (cx, cy) = dev.point(&Q::from_i128(0), &mid, &cfg).center();
    let h = Q::new(3, 20); // half-size, in flat units
    let feature: Vec<[Q; 2]> = [
        (cx.sub(&h), cy.sub(&h)),
        (cx.add(&h), cy.sub(&h)),
        (cx.add(&h), cy.add(&h)),
        (cx.sub(&h), cy.add(&h)),
    ]
    .into_iter()
    .map(|(x, y)| [x, y])
    .collect();
    println!(
        "feature:     authored on the flat pattern at ({:.3}, {:.3}), half-size {:.3}",
        e3(&cx),
        e3(&cy),
        e3(&h)
    );

    // — Fold the flat-authored feature back onto the cone (direction ②) → a certified 3-D wire —
    let wire = match fold_outline(
        &chart,
        &feature,
        &Q::from_i128(0),
        &span,
        60,
        true, // the device band is on the −μ̂ side
        &cfg,
        &clearance,
    ) {
        Verdict::Verified(w) => {
            println!(
                "fold-back:   Verified   ε≈{:.3e}   ({} 3-D verts on the cone)",
                e3(&w.eps),
                w.points.len()
            );
            w
        }
        v => bail("fold-back", &v),
    };
    let p0 = &wire.points[0];
    println!(
        "  folded vertex 0 → C ≈ ({:.3}, {:.3}, {:.3})",
        e3(&p0[0].mid()),
        e3(&p0[1].mid()),
        e3(&p0[2].mid())
    );

    // — SVG: the developed annulus with the interior feature cut (even-odd fill) —
    let panel_poly = flat_to_poly(&panel);
    let region = match assemble_flat(&panel_poly, std::slice::from_ref(&feature)) {
        Verdict::Verified(r) => {
            println!(
                "assemble:    Verified   1 face · {} hole (the interior feature)",
                r.faces[0].holes.len()
            );
            r
        }
        v => bail("flat assembly", &v),
    };
    let polys = region_to_polys(&region);
    let frame = Bounds::of_points(
        polys
            .faces
            .iter()
            .flat_map(|f| f.rings.iter().flatten().copied()),
    );
    let svg = polys_svg(&polys, &frame, 720);
    std::fs::create_dir_all(&out_dir).expect("create --out-dir");
    let svg_path = format!("{out_dir}/roundtrip_panel.svg");
    std::fs::write(&svg_path, &svg).expect("write roundtrip_panel.svg");
    println!("SVG:         wrote {svg_path}   ({} bytes)", svg.len());

    // — STEP: the annulus solid (the certified surface the feature was folded onto) —
    #[cfg(feature = "step")]
    emit_annulus_step(&chart, &span, &d1, &d2, &clearance, &cfg, &out_dir);
    #[cfg(not(feature = "step"))]
    println!("STEP:        skipped — build under `nix develop` with `--features diagnostics,step`");
}

/// Emit the annulus as a certified curved-rail cone solid (`brep_trim_solid`). The rails are
/// re-fitted at **low degree** (4) for OCCT's `f64` edge tolerance (the SVG keeps the tighter fit).
#[cfg(feature = "step")]
#[allow(clippy::too_many_arguments)]
fn emit_annulus_step(
    chart: &geom::chart::Chart<Bignum>,
    span: &Interval<Bignum>,
    d1: &export::trim::TrimDisk<Bignum>,
    d2: &export::trim::TrimDisk<Bignum>,
    clearance: &Q,
    cfg: &DevConfig<Bignum>,
    out_dir: &str,
) {
    use certify_core::shell::closed_shell_holed;
    use export::brep_build::brep_trim_solid;
    use export::step::write_brep;

    let lowfit = RailFit {
        degree: 4,
        subdiv: 256,
        bits: 44,
    };
    let w = Interval {
        lo: Q::from_i128(0),
        hi: Q::new(1, 8),
    };
    let (mu_out_lo, _) = match certified_rail(chart, d1, span, lowfit, clearance, cfg) {
        Verdict::Verified(x) => x,
        v => {
            println!("STEP:        D1 low-fit {} — skipping", verdict_tag(&v));
            return;
        }
    };
    let (mu_in_lo, _) = match certified_rail(chart, d2, span, lowfit, clearance, cfg) {
        Verdict::Verified(x) => x,
        v => {
            println!("STEP:        D2 low-fit {} — skipping", verdict_tag(&v));
            return;
        }
    };
    let outer_ch = vec![(span.clone(), mu_out_lo)];
    let inner_ch = vec![(span.clone(), mu_in_lo)];
    match brep_trim_solid(chart, &w, &inner_ch, &outer_ch, &[]) {
        Some(solid) => {
            let sc = solid.to_shell_certificate();
            let cert = matches!(
                closed_shell_holed(
                    sc.n_verts,
                    &sc.edge_start,
                    &sc.edge_end,
                    &sc.wire_edge,
                    &sc.wire_reversed,
                    &sc.loop_start,
                    &sc.face_start,
                ),
                Verdict::Verified(_)
            );
            let path = format!("{out_dir}/roundtrip_panel.step");
            println!(
                "STEP:        cert={}   {:<20}   → {path}   ({} faces, {} free)",
                if cert { "Verified" } else { "REFUTED" },
                write_brep(&path, &solid),
                solid.faces().len(),
                solid.free_edges(),
            );
        }
        None => println!("STEP:        refused — degenerate annulus band"),
    }
}
