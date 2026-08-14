//! **The scale probe** — a fold-heavy workload, the shape the product actually runs.
//!
//! The acceptance demo is dominated by `develop` and `solid`, which run *once* per part. The
//! product's hot path is the opposite: an atlas built once, then ECAD geometry — traces, drills,
//! a FEM mesh — folded through it, point after point. This probe isolates that.
//!
//! It folds one ring in a single `fold` call (regions are built once, as the real path does —
//! folding point-by-point measures `build_regions`, not the transform), repeated `--rounds` times
//! so a sampling profiler has a steady state to look at.
//!
//! ```text
//! cargo run --release -p author --example scale_probe -- --points 40 --rounds 5
//! ```

use author::part::Part;
use certify_core::Verdict;
use export::approx::rat_to_f64;
use lattice::{Bignum, Rat};

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let (mut points, mut rounds, mut segments, mut panels) = (40usize, 1usize, 24usize, 20usize);
    let mut i = 0;
    while i < argv.len() {
        let v = || argv[i + 1].parse().expect("numeric flag value");
        match argv[i].as_str() {
            "--points" => points = v(),
            "--rounds" => rounds = v(),
            "--segments" => segments = v(),
            "--panels" => panels = v(),
            other => panic!("unknown flag {other}"),
        }
        i += 2;
    }

    let part: Part<Bignum> = acceptance::self_lapping_cone(segments, panels, true);
    let t = std::time::Instant::now();
    let flat = match part.develop() {
        Verdict::Verified(f) => f,
        Verdict::Unresolved(e) => panic!("develop unresolved at ε ≈ {:.3e}", rat_to_f64(&e)),
        Verdict::Refuted(f) => panic!("develop refuted: {f:?}"),
    };
    println!("develop      {:8.2}s (once)", t.elapsed().as_secs_f64());

    let verts = &flat.outline().vertices;
    let n = points.min(verts.len());
    let ring: Vec<[Rat<Bignum>; 2]> = verts
        .iter()
        .take(n)
        .map(|v| {
            let (x, y) = v.center();
            [x, y]
        })
        .collect();

    let t = std::time::Instant::now();
    let mut worst = 0.0f64;
    for _ in 0..rounds {
        match part.fold(&ring, &Rat::from_i128(0)) {
            Verdict::Verified(w) => worst = worst.max(rat_to_f64(&w.eps)),
            Verdict::Unresolved(e) => panic!("fold unresolved at ε ≈ {:.3e}", rat_to_f64(&e)),
            Verdict::Refuted(f) => panic!("fold refuted: {f:?}"),
        }
    }
    let el = t.elapsed().as_secs_f64();
    let total = (n * rounds) as f64;
    println!(
        "fold         {el:8.2}s  {total:.0} pts  {:.1} ms/pt  worst ε {worst:.4e}",
        el / total * 1000.0
    );
}
