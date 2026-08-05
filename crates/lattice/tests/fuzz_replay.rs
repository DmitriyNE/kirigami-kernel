//! Per-PR **regression replay** (stable toolchain, no libFuzzer): re-runs every checked-in
//! fuzz crash input through the differential — faithfully, under the *same* lowered dashu
//! thresholds the cargo-fuzz target uses — so a crash pinned here stays fixed forever. This is
//! the deterministic per-PR half of the fuzzing story; the nondeterministic *search* is the
//! nightly cron (`.github/workflows/fuzz-nightly.yml`).
//!
//! Workflow: when the nightly fuzzer finds a divergence, minimize it
//! (`cargo fuzz tmin int_chain <artifact>`) and drop the minimized `.bin` into
//! `tests/fuzz-corpus/int_chain/` — it is then replayed here on every PR.
//!
//! Gated on `feature = "fuzzing"` (needed for `lattice::ratfuzz` + dashu `tuning`); under a plain
//! `cargo test` the whole file compiles to nothing, so it never breaks the main test run. CI:
//!   cargo test -p lattice --features fuzzing --test fuzz_replay
#![cfg(feature = "fuzzing")]

use std::fs;
use std::path::Path;

#[test]
fn replay_committed_regressions() {
    // Match the cargo-fuzz target's tuning (fuzz/fuzz_targets/int_chain.rs) so a tuned crash
    // reproduces here. SAFETY: set once at the start of this single test, before any dashu op.
    unsafe {
        std::env::set_var("DASHU_THRESHOLD_SIMPLE_MUL", "2");
        std::env::set_var("DASHU_THRESHOLD_KARATSUBA_MUL", "16");
        std::env::set_var("DASHU_THRESHOLD_NTT_MUL", "160");
    }

    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fuzz-corpus/int_chain");
    let mut replayed = 0usize;
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|x| x == "bin") {
                let data = fs::read(&path).expect("read regression input");
                // A divergence panics inside `run_int_program` — i.e. this test fails.
                lattice::ratfuzz::run_int_program(&data);
                replayed += 1;
            }
        }
    }
    eprintln!(
        "replayed {replayed} committed fuzz regression(s) from {}",
        dir.display()
    );
}
