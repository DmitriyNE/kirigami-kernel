//! **The lapped cone, as parameters** — a seam with a ramp on *each* side.
//!
//! The acceptance device is the one-ramp case: its seam centreline sits exactly `t/2 + g/2` off the
//! base sheet, so one end never leaves the base cone. This driver runs the other configuration —
//! `c = 0`, the seam straddling the base symmetrically — which costs a ramp on both sides and is
//! what a board wants when neither end may bulge more than the other.
//!
//! Both are the same recipe type. What changes is one number.
//!
//! ```text
//! cargo run --example lapped_cone                                   # develop + SVG + clearance
//! nix develop -c cargo run --example lapped_cone --features step    # + the .step solid
//! ```
//!
//! Flags: `--out-dir <dir>` (default `generated-demos/`), `--segments N`, `--panels N`.

use acceptance::lapped::{self, Azimuth, GapPolicy, LappedCone, OnTop, RampProfile, SideAngles};
use author::part::Part;
use certify_core::Verdict;
use develop::cone::DevConfig;
use export::approx::rat_to_f64;
use export::trim::RailFit;
use lattice::{Bignum, Rat};

type Q = Rat<Bignum>;

fn q(n: i128, d: i128) -> Q {
    Q::new(n, d)
}
fn qi(n: i128) -> Q {
    Q::from_i128(n)
}
fn sigma(n: i128, d: i128) -> Azimuth {
    Azimuth::Sigma(q(n, d))
}

/// The two-ramp recipe: the same 42° cone and the same **physical** stack as the acceptance
/// device — 240 µm of 4-layer flex, a 10 µm ACF bondline, inner Ø 5 mm, all in millimetres — with
/// the seam centred on the base sheet instead of offset onto one side of it.
///
/// [`acceptance::self_lapping_spec`] carries the table of where each number comes from; the only
/// one that differs here is `seam_offset`.
fn spec() -> LappedCone {
    LappedCone {
        // The Pythagorean (72, 65, 97) — sin β = 65/97, exact.
        apex: (qi(72), qi(65)),
        thickness: q(6, 25),
        gap: q(1, 100),
        on_top: OnTop::Cw,
        // c = 0: the seam straddles the base cone, so BOTH ends ramp, by ∓(t/2 + g/2) = ∓1/8.
        // seam_offset: q(-25, 200),
        seam_offset: q(0, 200),
        // Both ramps span Δσ = 7/20, symmetrically. A ramp's edge of regression sweeps ≈0.9·h/Δσ²
        // along the ruling and must stay inside the inner bound's µ̂ ≈ 1.81 or it crosses the
        // sheet: 0.9·(1/8)/(7/20)² ≈ 0.92, clear by ~2×. Narrow them and the part is refused —
        // soundly, because the sheet would have to crease. See docs/engineering-log.md.
        ccw: SideAngles {
            ramp_start: sigma(50, 80),
            ramp_end: sigma(7, 8),
            sheet_end: sigma(9, 8),
        },
        cw: SideAngles {
            ramp_start: sigma(-50, 80),
            ramp_end: sigma(-7, 8),
            sheet_end: sigma(-9, 8),
        },
        // ccw: SideAngles::flat(sigma(5, 4)),
        // cw: SideAngles::flat(sigma(-9, 8)),
        // The annulus, concentric here (the acceptance device's inner bound is off-axis):
        // inner Ø 5 mm, outer ≈ 5.115 mm.
        outer_r2: q(157, 6),
        inner_r2: Some(q(25, 4)),
        neutral: q(1, 2),
        // The even ramp: `h''` constant in magnitude, so the bend is spread across the ramp
        // instead of piling up at its two joins. Measured 1.5x less fold-line swing.
        ramp_profile: RampProfile::EvenCurvature,
        // Both ramps finish before the overlap starts (ramp_end 3/4 against the lap's 4/5), so the
        // gap really is `g` across the whole seam and the strict policy holds.
        policy: GapPolicy::Constant,
        pick: None,
    }
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let (mut out_dir, mut segments, mut panels) = ("generated-demos".to_string(), 16usize, 8usize);
    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "--out-dir" => {
                out_dir = argv[i + 1].clone();
                i += 2;
            }
            "--segments" => {
                segments = argv[i + 1].parse().expect("--segments N");
                i += 2;
            }
            "--panels" => {
                panels = argv[i + 1].parse().expect("--panels N");
                i += 2;
            }
            other => panic!("unknown flag {other}"),
        }
    }
    std::fs::create_dir_all(&out_dir).expect("create --out-dir");

    let spec = spec();
    let lap = match lapped::lapped_cone(&spec) {
        Ok(l) => l,
        Err(f) => {
            println!("recipe    Refused({f:?})");
            return;
        }
    };

    println!(
        "lapped cone — two-ramp seam (c = {:.4}), 42° cone, t = {:.4} mm, g = {:.4} mm\n",
        rat_to_f64(&spec.seam_offset),
        rat_to_f64(&spec.thickness),
        rat_to_f64(&spec.gap),
    );
    println!(
        "recipe    h_ccw {:+.4}   h_cw {:+.4}   {} regions   lap σ ∈ [{:.4}, {:.4}] ∪ [{:.4}, {:.4}]",
        rat_to_f64(&lap.h_ccw),
        rat_to_f64(&lap.h_cw),
        lap.regions.len(),
        rat_to_f64(&lap.lap_cw.lo),
        rat_to_f64(&lap.lap_cw.hi),
        rat_to_f64(&lap.lap_ccw.lo),
        rat_to_f64(&lap.lap_ccw.hi),
    );
    for (band, h) in &lap.regions {
        println!(
            "region    σ ∈ [{:+.4}, {:+.4}]   h {:+.4}",
            rat_to_f64(&band.lo),
            rat_to_f64(&band.hi),
            rat_to_f64(h),
        );
    }

    // — what BONDED certifies the seam clears —
    let t0 = std::time::Instant::now();
    // BONDED *proves* `≥ keep_out`; it does not measure. Ask for half the authored gap — a
    // keep-out at or above `g` is exactly what the ramp's intrusion makes unprovable.
    match lap.seam_clearance(&spec.gap.div(&qi(2)), 4_000) {
        Verdict::Verified(c) => println!(
            "seam      BONDED certifies ≥ {:.4} between the facing faces over {} rail pair(s), \
             {} nodes   [{:.1}s]   (authored gap {:.4})",
            c.min_dist(),
            c.rails,
            c.nodes,
            t0.elapsed().as_secs_f64(),
            rat_to_f64(&spec.gap),
        ),
        Verdict::Unresolved(d2) => println!(
            "seam      BONDED unresolved — closest it reached was d² ≈ {:.3e}",
            rat_to_f64(&d2)
        ),
        Verdict::Refuted(f) => println!("seam      BONDED refuted({f:?})"),
    }

    // — the resolution knobs are the caller's, exactly as `self_lapping_cone` sets them —
    let part: Part<Bignum> = lap
        .part
        // Matches the acceptance device: the DRC keep-out is a length in the part's own unit.
        .clearance(q(5, 3))
        .fit(RailFit {
            degree: 4,
            subdiv: 160,
            bits: 44,
        })
        .segments(segments)
        .support_panels(panels)
        .budget(DevConfig {
            terms: 14,
            sqrt_eps: q(1, 1_000_000_000),
        });

    // — direction ①: develop to the flat pattern —
    let t0 = std::time::Instant::now();
    match part.develop() {
        Verdict::Verified(flat) => {
            println!(
                "develop   Verified   ε {:.3e}   {} face(s) · {} hole(s)   [{:.1}s]",
                rat_to_f64(flat.eps()),
                flat.region().faces.len(),
                flat.region().faces[0].holes.len(),
                t0.elapsed().as_secs_f64(),
            );
            let path = format!("{out_dir}/lapped_cone_two_ramp.svg");
            let svg = flat.svg(900);
            std::fs::write(&path, &svg).expect("write the flat pattern");
            println!("develop   SVG       wrote {path}   ({} bytes)", svg.len());
        }
        Verdict::Refuted(f) => println!("develop   Refuted({f:?})"),
        Verdict::Unresolved(e) => {
            println!("develop   Unresolved at ε {:.3e}", rat_to_f64(&e))
        }
    }

    // — the solid —
    let t0 = std::time::Instant::now();
    match part.solid() {
        Verdict::Verified(solid) => {
            let b = solid.brep();
            println!(
                "solid     Verified   ε {:.3e}   {} faces · free {} · non-manifold {}   [{:.1}s]",
                rat_to_f64(solid.eps()),
                b.faces().len(),
                b.free_edges(),
                b.nonmanifold_edges(),
                t0.elapsed().as_secs_f64(),
            );
            write_step(&out_dir, &solid);
        }
        Verdict::Refuted(f) => println!("solid     Refuted({f:?})"),
        Verdict::Unresolved(e) => println!("solid     Unresolved at ε {:.3e}", rat_to_f64(&e)),
    }
}

#[cfg(feature = "step")]
fn write_step(out_dir: &str, solid: &author::part::PartSolid<Bignum>) {
    let path = format!("{out_dir}/lapped_cone_two_ramp.step");
    println!(
        "solid     STEP      {} → {path}",
        solid.write_step(&path).summary()
    );
}

#[cfg(not(feature = "step"))]
fn write_step(_out_dir: &str, _solid: &author::part::PartSolid<Bignum>) {
    println!(
        "solid     STEP      skipped — build under \
         `nix develop -c cargo run --example lapped_cone --features step`"
    );
}
