//! Backend speed yardstick. Builds a fixed degree-12 polynomial with ~240-bit
//! rational coefficients and times a full Sturm PRS + sign-count per backend.
//!
//! Instrumented to answer two fair suspicions:
//!  - is the dashu≈malachite tie real? -> print min/median/max + every raw run.
//!  - is the workload real & identical?  -> print the Sturm chain profile
//!    (degree + leading-coeff size per entry) and assert it matches across
//!    backends. (Run under `caffeinate -i` so the laptop can't idle-sleep.)

mod backends;
mod prs;

use backends::{Dashu, Malachite, Num, Rat};
use std::io::Write;
use std::time::Instant;

const DEG: u64 = 12;
const ITERS: u32 = 9;

fn make_poly<R: Rat>() -> Vec<R> {
    (0..=DEG)
        .map(|i| prs::big256::<R>(0x1234_5678u64 ^ i.wrapping_mul(0x9e37_79b9)))
        .collect()
}

/// (degree, leading-coeff size in chars) per Sturm-chain entry — the workload
/// fingerprint. Identical across backends because they compute identical values.
fn profile<R: Rat>() -> Vec<(usize, usize)> {
    prs::sturm_chain(&make_poly::<R>())
        .iter()
        .map(|p| (p.len() - 1, p.last().unwrap().size_chars()))
        .collect()
}

fn run<R: Rat>() -> (u32, Vec<f64>) {
    let poly = make_poly::<R>();
    let checksum = prs::sturm_root_count(&poly); // warmup
    let mut times = Vec::with_capacity(ITERS as usize);
    for _ in 0..ITERS {
        let t = Instant::now();
        let c = prs::sturm_root_count(&poly);
        times.push(t.elapsed().as_secs_f64());
        assert_eq!(c, checksum);
    }
    (checksum, times)
}

fn summarize(times: &[f64]) -> (f64, f64, f64) {
    let mut s = times.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (s[0], s[s.len() / 2], *s.last().unwrap())
}

fn main() {
    let started = Instant::now();
    println!("deg-{DEG} Sturm PRS over ~240-bit rationals · {ITERS} runs/backend (aarch64-darwin)\n");

    // Workload fingerprint — proves the chain is real (big coeffs) and that all
    // three backends do the SAME work.
    let (pd, pn, pm) = (profile::<Dashu>(), profile::<Num>(), profile::<Malachite>());
    assert_eq!(pd, pn, "chain profile differs: dashu vs num");
    assert_eq!(pd, pm, "chain profile differs: dashu vs malachite");
    println!("  Sturm chain: {} entries; (degree, leading-coeff chars) per entry:", pd.len());
    println!("    {pd:?}");
    let total_chars: usize = pd.iter().map(|(_, c)| c).sum();
    println!("    Σ leading-coeff chars = {total_chars}  (identical across all 3 backends)\n");

    let rows: Vec<(&str, u32, Vec<f64>)> = vec![
        {
            let (rc, t) = run::<Dashu>();
            (Dashu::NAME, rc, t)
        },
        {
            let (rc, t) = run::<Num>();
            (Num::NAME, rc, t)
        },
        {
            let (rc, t) = run::<Malachite>();
            (Malachite::NAME, rc, t)
        },
    ];

    println!("  {:<22} {:>9} {:>9} {:>9}   raw runs (ms)", "backend", "min", "median", "max");
    let mut mins = Vec::new();
    for (name, rc, times) in &rows {
        let (mn, md, mx) = summarize(times);
        mins.push(mn);
        let raw: Vec<String> = times.iter().map(|t| format!("{:.0}", t * 1e3)).collect();
        println!(
            "  {name:<22} {:>7.1}ms {:>7.1}ms {:>7.1}ms   [{}]  (roots={rc})",
            mn * 1e3,
            md * 1e3,
            mx * 1e3,
            raw.join(", ")
        );
        let _ = std::io::stdout().flush();
    }

    let fastest = mins.iter().cloned().fold(f64::INFINITY, f64::min);
    println!("\n  relative (min × fastest):");
    for ((name, _, _), mn) in rows.iter().zip(&mins) {
        println!("    {name:<22} {:>6.2}×", mn / fastest);
    }

    let rc0 = rows[0].1;
    assert!(rows.iter().all(|r| r.1 == rc0), "root-count disagreement");
    println!("\n  cross-check OK: all backends agree, root count = {rc0}");
    println!("  wall time: {:.1}s", started.elapsed().as_secs_f64());
}
