//! **AUTH.3d — the contour panel: a part whose boundary is authored, not banded.**
//!
//! Every other demo here draws a *band*: `region_sigma` says where the material starts and stops,
//! and the cutters only trim µ̂. This one keeps what is inside a **radiused rectangle** — four plane
//! sides and four corner arcs, the outline a flex fabricator accepts — so the panel's σ-extent is
//! *derived* from the contour's own corners. That is what a flex circuit's boundary actually is,
//! and it is the one thing `intersect` could not express before AUTH.3.
//!
//! Both product directions, on one part:
//!
//! * **3-D → flat.** The contour is cut in 3-D and developed to a certified flat pattern (SVG).
//! * **flat → 3-D.** A feature is authored in the *developed* panel's own ECAD coordinates and
//!   folded back onto the surface, then drilled through the certified solid (STEP).
//!
//! ```text
//! cargo run --release --example contour_panel                                  # SVGs
//! nix develop -c cargo run --release --example contour_panel --features step   # + STEP
//! ```

use certify_core::Verdict;
use lattice::{Bignum, Rat};

type Q = Rat<Bignum>;

fn f(r: &Q) -> f64 {
    let (n, d) = r.numer_denom_decimal();
    n.parse::<f64>().unwrap() / d.parse::<f64>().unwrap()
}

fn main() {
    let mut out_dir = "generated-demos".to_string();
    let mut segments = 96usize;
    let argv: Vec<String> = std::env::args().collect();
    for i in 0..argv.len() {
        match argv[i].as_str() {
            "--out-dir" if i + 1 < argv.len() => out_dir = argv[i + 1].clone(),
            "--segments" if i + 1 < argv.len() => {
                segments = argv[i + 1].parse().expect("--segments takes a number");
            }
            _ => {}
        }
    }
    std::fs::create_dir_all(&out_dir).expect("create --out-dir");
    let (cx, cy, w, h, r) = acceptance::contour_outline_geometry();
    println!(
        "[recipe] outline: rounded rect c=({:.3},{:.3}) half=({:.3},{:.3}) r={:.3}  segments={segments}",
        f(&cx),
        f(&cy),
        f(&w),
        f(&h),
        f(&r)
    );

    // — Direction ①: the authored 3-D contour, developed. —
    let part = acceptance::contour_panel(segments, None);
    let t = std::time::Instant::now();
    let flat = match part.develop() {
        Verdict::Verified(fl) => fl,
        v => {
            println!("[refused] develop: {}", verdict_name(&v));
            return;
        }
    };
    println!(
        "[time] develop           {:8.2}s   {} outline points, eps {:.3e}",
        t.elapsed().as_secs_f64(),
        flat.outline().vertices.len(),
        f(flat.eps())
    );
    let roles: Vec<_> = flat.report().ops.iter().map(|o| o.role).collect();
    println!("[roles] {roles:?}   (the outline bounds the part alone — the rest derive Inactive)");

    let svg_path = format!("{out_dir}/contour_panel.svg");
    std::fs::write(&svg_path, flat.svg(900)).expect("write flat svg");
    println!("  wrote {svg_path}");

    // — Direction ②: a feature authored on the DEVELOPED panel, folded back. —
    //
    // Its coordinates come from the flat pattern above, which is the whole point: an ECAD author
    // draws on the panel that will be manufactured, not on the cone.
    let verts: Vec<[Q; 2]> = flat
        .outline()
        .vertices
        .iter()
        .map(|b| {
            let (x, y) = b.center();
            [x, y]
        })
        .collect();
    let n = verts.len() as f64;
    let (gx, gy) = verts.iter().fold((0.0, 0.0), |(sx, sy), v| {
        (sx + f(&v[0]) / n, sy + f(&v[1]) / n)
    });
    let snap = |v: f64| export::approx::f64_to_rat::<Bignum>(v, 20);
    let e = 0.02;
    let feature = vec![
        [snap(gx - e), snap(gy - e)],
        [snap(gx + e), snap(gy - e)],
        [snap(gx + e), snap(gy + e)],
        [snap(gx - e), snap(gy + e)],
    ];
    println!("[ecad] feature authored at flat ({gx:.4}, {gy:.4}), half-side {e}");

    let drilled = acceptance::contour_panel(segments, Some(feature));
    let t = std::time::Instant::now();
    let flat2 = match drilled.develop() {
        Verdict::Verified(fl) => fl,
        v => {
            println!("[refused] develop with feature: {}", verdict_name(&v));
            return;
        }
    };
    println!(
        "[time] develop + feature {:8.2}s   {} holes in the flat pattern",
        t.elapsed().as_secs_f64(),
        flat2.region().faces[0].holes.len()
    );
    let svg2 = format!("{out_dir}/contour_panel_drilled.svg");
    std::fs::write(&svg2, flat2.svg(900)).expect("write drilled svg");
    println!("  wrote {svg2}");

    // — The certified solid, with the flat-authored feature folded through it. —
    let t = std::time::Instant::now();
    let solid = match drilled.solid() {
        Verdict::Verified(s) => s,
        v => {
            println!("[refused] solid: {}", verdict_name(&v));
            return;
        }
    };
    let brep = solid.brep();
    let (nv, ne, nf) = (
        brep.verts().len() as i64,
        brep.edges().len() as i64,
        brep.faces().len() as i64,
    );
    let nl: i64 = brep
        .faces()
        .iter()
        .map(|fc| 1 + fc.holes.len() as i64)
        .sum();
    println!(
        "[time] solid             {:8.2}s   {nf} faces, genus {}, free edges {}, eps {:.3e}",
        t.elapsed().as_secs_f64(),
        (2 - (nv - ne + (2 * nf - nl))) / 2,
        brep.free_edges(),
        f(solid.eps())
    );

    #[cfg(feature = "step")]
    {
        let path = format!("{out_dir}/contour_panel.step");
        let report = solid.write_step(&path);
        println!("[step] {}   → {path}", report.summary());
    }
    #[cfg(not(feature = "step"))]
    println!("  STEP             : skipped — build under `nix develop` with `--features step`");
}

fn verdict_name<E, W: core::fmt::Debug, M: core::fmt::Debug>(v: &Verdict<E, W, M>) -> String {
    match v {
        Verdict::Verified(_) => "Verified".to_string(),
        Verdict::Unresolved(m) => format!("Unresolved({m:?})"),
        Verdict::Refuted(fa) => format!("Refuted({fa:?})"),
    }
}
