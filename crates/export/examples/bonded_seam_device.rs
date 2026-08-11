//! Emit the **bonded seam device** (DD.4) — the culminating acceptance demo the flex-PCB spine
//! builds toward: the self-lapping cone-with-ramp as **body gore (γ = 0) + ramp flap (γ ≠ 0) + a
//! certified bond** (§6.2 — a lap is doubled material, not one self-touching solid). Running it is
//! the acceptance report; every stage prints a certified verdict.
//!
//! The full bidirectional round-trip on both sheets:
//! - **ramp flap** (`cone_seam_ramp`, γ ≠ 0): develop the band to a certified flat pattern (DD.2
//!   directrix integrator) → fold a flat point back onto it (DD.3, the signed-µ̂ residual);
//! - **body gore** (`cone_seam`, γ = 0): develop → fold back (DD.1 / DEV.2d/2e);
//! - the seam **bond**: the Stage-2 §14 `valid_bonded_seam` (SEP ∧ SLAB ∧ SHEAR ∧ CLEAR).
//!
//! The flat pattern is sampled at the band corners via the certified `dev.point` (DD.2) — the
//! boundary-loop `unroll` composes the *same* `point_on` and rides unchanged, but for a `γ ≠ 0`
//! chart its anchor subdivision re-integrates γ per sub-interval (slow), so the demo samples corners.
//!
//! Artifacts (`--out-dir`, default `generated-demos/`): `seam_body.svg` + `seam_flap.svg`. Behind
//! `--features step` under `nix develop`: `seam_body.step` + `seam_flap.step` — two certified
//! curved-rail cone solids (`closed_shell_holed` + OCCT `audit_brep`).
//!
//! ```text
//! cargo run --example bonded_seam_device --features diagnostics
//! nix develop -c cargo run --example bonded_seam_device --features diagnostics,step
//! ```

use certify_core::Verdict;
use develop::bonded::{LapRail, clear, sep, shear, slab, valid_bonded_seam};
use develop::cone::{ConeDevelopment, DevConfig};
use develop::fold::fold_point;
use export::approx::rat_to_f64;
use export::svg::{Bounds, polys_svg, region_to_polys};
use export::trim::assemble_flat;
use fixtures::devices::{cone_seam, cone_seam_ramp};
#[cfg(feature = "step")]
use geom::chart::Chart;
use lattice::{Bignum, Interval, Poly, Rat, RatFunc};

type Q = Rat<Bignum>;

fn e3(r: &Q) -> f64 {
    rat_to_f64(r)
}
fn ratf(n: i128, d: i128) -> RatFunc<Bignum> {
    RatFunc::from_poly(Poly::from_coeffs(vec![Rat::new(n, d)]))
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

/// Develop the band `σ' ∈ [0, 1/2], µ̂ ∈ [−2, −1]` at its four corners via the certified `dev.point`
/// (DD.2), report the max backward error, and write the flat quad as SVG. Returns the max ε.
fn develop_and_draw(dev: &ConeDevelopment<Bignum>, cfg: &DevConfig<Bignum>, path: &str) -> Q {
    let corners = [
        (Q::from_i128(0), Q::from_i128(-2)),
        (Q::new(1, 2), Q::from_i128(-2)),
        (Q::new(1, 2), Q::from_i128(-1)),
        (Q::from_i128(0), Q::from_i128(-1)),
    ];
    let mut poly: Vec<[Q; 2]> = Vec::new();
    let mut max_eps = Q::from_i128(0);
    for (s, m) in &corners {
        let b = dev.point(s, m, cfg);
        if b.backward_error().cmp(&max_eps) == core::cmp::Ordering::Greater {
            max_eps = b.backward_error();
        }
        let (x, y) = b.center();
        poly.push([x, y]);
    }
    if let Verdict::Verified(region) = assemble_flat(&poly, &[]) {
        let polys = region_to_polys(&region);
        let frame = Bounds::of_points(
            polys
                .faces
                .iter()
                .flat_map(|f| f.rings.iter().flatten().copied()),
        );
        let svg = polys_svg(&polys, &frame, 720);
        std::fs::write(path, &svg).expect("write flat-pattern SVG");
        println!("  SVG → {path}   ({} bytes)", svg.len());
    }
    max_eps
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

    let cfg = DevConfig::tight();
    let clearance = Q::from_i128(1);
    let w0 = Q::from_i128(0);
    let band = Interval {
        lo: Q::from_i128(0),
        hi: Q::new(1, 2),
    };

    println!("bonded seam device — self-lapping cone-with-ramp (body γ=0 + ramp flap γ≠0 + bond)");

    // ---- THE RAMP FLAP (γ ≠ 0) ----
    println!("--- ramp flap (γ ≠ 0) ---");
    let flap = cone_seam_ramp();
    let flap_dev = ConeDevelopment::new_developable(&flap, 64).expect("flap developable");
    let feps = develop_and_draw(&flap_dev, &cfg, &format!("{out_dir}/seam_flap.svg"));
    println!(
        "develop:     Verified   ε≈{:.3e}   (γ≠0 directrix)",
        e3(&feps)
    );
    let (s0, m0) = (Q::new(1, 4), Q::new(-3, 2));
    let (fx, fy) = flap_dev.point(&s0, &m0, &cfg).center();
    match fold_point(&flap, &fx, &fy, &w0, &band, 40, true, &cfg, &clearance) {
        Verdict::Verified(f) => println!(
            "fold-back:   Verified   ε≈{:.3e}   recovered (σ′≈1/4, µ̂≈−3/2) on the flap",
            e3(&f.eps)
        ),
        v => bail("flap fold", &v),
    }

    // ---- THE BODY GORE (γ = 0) ----
    println!("--- body gore (γ = 0) ---");
    let body = cone_seam();
    let body_dev = ConeDevelopment::new(&body).expect("body apex cone");
    let beps = develop_and_draw(&body_dev, &cfg, &format!("{out_dir}/seam_body.svg"));
    println!("develop:     Verified   ε≈{:.3e}   (γ=0)", e3(&beps));
    let (bx, by) = body_dev.point(&Q::new(1, 4), &Q::new(-3, 2), &cfg).center();
    match fold_point(&body, &bx, &by, &w0, &band, 40, true, &cfg, &clearance) {
        Verdict::Verified(f) => println!(
            "fold-back:   Verified   ε≈{:.3e}   recovered (σ′≈1/4, µ̂≈−3/2) on the body",
            e3(&f.eps)
        ),
        v => bail("body fold", &v),
    }

    // ---- THE CERTIFIED BOND (Stage 2 §14 BONDED) ----
    println!("--- the bond (§14 BONDED) ---");
    let sig = Interval {
        lo: Q::new(-1, 4),
        hi: Q::new(1, 4),
    };
    let neg1 = Q::from_i128(-1);
    let bond = valid_bonded_seam(
        sep(
            &RatFunc::<Bignum>::zero(),
            &w0,
            &ratf(1, 4),
            &w0,
            &Q::new(1, 4),
        ),
        slab(&cone_seam_ramp(), &neg1, &w0, &sig, &Q::new(1, 1000)),
        shear(&ratf(-65, 72), &ratf(1, 4), &Q::new(1, 100)),
        clear(
            &LapRail::from_chart(&cone_seam(), &neg1, &w0),
            &LapRail::from_chart(&cone_seam_ramp(), &neg1, &w0),
            &sig,
            &Q::new(1, 8),
            2000,
        ),
    );
    match bond {
        Verdict::Verified(_) => {
            println!("bond:        Verified   SEP ∧ SLAB ∧ SHEAR (δ=18/65≈0.28mm) ∧ CLEAR")
        }
        _ => {
            println!("bond:        NOT certified — stopping");
            std::process::exit(1);
        }
    }

    // ---- THE TWO CERTIFIED SOLIDS (STEP) ----
    #[cfg(feature = "step")]
    emit_step_solids(&body, &flap, &out_dir);
    #[cfg(not(feature = "step"))]
    println!("STEP:        skipped — build under `nix develop` with `--features diagnostics,step`");
}

/// Emit the two certified curved-rail cone solids (body γ=0 + flap γ≠0) as STEP, each
/// `closed_shell_holed`-certified and OCCT-corroborated.
#[cfg(feature = "step")]
fn emit_step_solids(body: &Chart<Bignum>, flap: &Chart<Bignum>, out_dir: &str) {
    use certify_core::shell::closed_shell_holed;
    use export::brep_build::brep_trim_solid;
    use export::step::{audit_brep, write_brep};
    println!("--- the two certified solids (STEP) ---");
    let sig = Interval {
        lo: Q::new(-1, 4),
        hi: Q::new(1, 4),
    };
    let w = Interval {
        lo: Q::from_i128(0),
        hi: Q::new(1, 8),
    };
    let inner = [(sig.clone(), ratf(-2, 1))];
    let outer = [(sig.clone(), ratf(-1, 1))];
    for (name, chart) in [("body", body), ("flap", flap)] {
        let Some(solid) = brep_trim_solid(chart, &w, &inner, &outer, &[]) else {
            println!("STEP {name}:   refused — degenerate band");
            continue;
        };
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
        let path = format!("{out_dir}/seam_{name}.step");
        let ok = write_brep(&path, &solid);
        let audit = audit_brep(&solid);
        println!(
            "STEP {name}:   cert={}   write={ok}   → {path}   audit={}",
            if cert { "Verified" } else { "REFUTED" },
            match &audit {
                Ok(a) => format!(
                    "valid={} free={} nonmanifold={}",
                    a.brepcheck_valid, a.free_edges, a.nonmanifold_edges
                ),
                Err(e) => format!("audit-error: {e}"),
            },
        );
    }
}
