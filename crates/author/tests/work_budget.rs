//! **The work budget (VV.1)** — a perf regression gate that cannot flake.
//!
//! Until now there was no performance regression detection for the geometry pipeline at all: the
//! only benchmarks in the tree measure algebra backends, and the 10× slowdown the p-curve
//! milestone introduced was found by accident, months of work later.
//!
//! This gate counts **operations**, not seconds. A committed wall-clock baseline moves with the
//! machine and has to be either loose enough to miss a real regression or tight enough to cry
//! wolf; and the regression that motivated all of this was a *complexity* change — `N × panels`
//! work where `N + cells` was available — which a count catches exactly and identically on every
//! machine. See the OPT.0 / OPT.1 entries in `docs/engineering-log.md`.
//!
//! Wall-clock is deliberately **not** asserted here. The stage timings live in the demo driver
//! (`[time]` lines on stderr) for when a human wants them.

use author::part::Part;
use certify_core::Verdict;
use develop::counters;
use export::approx::rat_to_f64;
use lattice::Bignum;

/// Measured on the acceptance device at `segments(16)`, `support_panels(8)` (2026-08-14, after
/// OPT.1): `develop` integrates **2 256 γ cells** and performs **4 096 cut-certificate
/// evaluations**.
///
/// The budgets sit ~1.4× above measurement. They are **complexity** gates, not ratchets: what they
/// exist to catch is a change of *shape* — reintroducing a per-query re-integration would multiply
/// the γ count by the panel budget, which blows through this by an order of magnitude and cannot
/// be mistaken for noise. Small drifts from a resolution or geometry change are expected; update
/// the constants with a note when that happens.
const GAMMA_CELLS_MAX: u64 = 3_200;
const CUT_EVALS_MAX: u64 = 5_800;

fn device() -> Part<Bignum> {
    acceptance::self_lapping_cone(16, 8, true)
}

#[test]
fn the_development_stays_within_its_work_budget() {
    counters::reset();
    let flat = match device().develop() {
        Verdict::Verified(f) => f,
        Verdict::Unresolved(e) => panic!("develop unresolved at ε ≈ {:.3e}", rat_to_f64(&e)),
        Verdict::Refuted(f) => panic!("develop refuted: {f:?}"),
    };
    let (gamma, cuts) = (counters::gamma_cells(), counters::cut_evals());
    println!("[work] develop  γ cells {gamma}/{GAMMA_CELLS_MAX}  cut evals {cuts}/{CUT_EVALS_MAX}");
    // The part really was built — a gate over a stage that silently did nothing would pass.
    assert_eq!(flat.region().faces[0].holes.len(), 2);
    assert!(
        gamma > 0 && cuts > 0,
        "the counters must actually observe the pipeline (γ {gamma}, cuts {cuts})"
    );
    assert!(
        gamma <= GAMMA_CELLS_MAX,
        "γ cells {gamma} exceeds the work budget {GAMMA_CELLS_MAX} — the flat directrix is being \
         re-integrated per query again (see OPT.1)"
    );
    assert!(
        cuts <= CUT_EVALS_MAX,
        "cut-certificate evaluations {cuts} exceeds the work budget {CUT_EVALS_MAX}"
    );
}

/// The property OPT.1 actually installed, asserted directly rather than inferred from a total:
/// **γ work is independent of how many times γ is asked for.** A second development of the same
/// part costs the same as the first; before the prefix table, every query paid the whole integral
/// again. This is what would fail first if the memoization were removed or keyed wrongly.
#[test]
fn asking_for_gamma_again_costs_nothing() {
    let dev = develop::cone::ConeDevelopment::<Bignum>::new_developable(
        &fixtures::devices::cone_seam_ramp(),
        16,
    )
    .expect("the ramp is a curved-support developable");
    let cfg = develop::cone::DevConfig::tight();
    let lo = lattice::Rat::from_i128(0);

    // Walk a boundary's worth of σ once, building the grid.
    counters::reset();
    for k in 1..=32 {
        let s = lattice::Rat::new(k, 64);
        dev.directrix_between(&lo, &s, &cfg).expect("γ is regular");
    }
    let first = counters::gamma_cells();

    // Walk it again. Every σ is now inside the grid, so only the partial cells are integrated.
    counters::reset();
    for k in 1..=32 {
        let s = lattice::Rat::new(k, 64);
        dev.directrix_between(&lo, &s, &cfg).expect("γ is regular");
    }
    let second = counters::gamma_cells();

    println!("[work] γ cells: first sweep {first}, repeat sweep {second}");
    assert!(
        second < first,
        "the repeat sweep must reuse the grid: first {first}, second {second}"
    );
    // 32 queries against one origin must not cost 32 × panels cells — the shape OPT.1 removed.
    assert!(
        first < 32 * 16,
        "the first sweep already amortizes: {first} cells for 32 queries at 16 panels"
    );
}
