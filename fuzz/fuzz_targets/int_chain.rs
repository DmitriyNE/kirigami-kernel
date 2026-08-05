#![no_main]
//! Coverage-guided op-chain differential: `dashu` ≡ the *proven* `RefBackend` over
//! large, chained integer arithmetic. libFuzzer mutates the raw bytes; the shared core
//! (`lattice::ratfuzz::run_int_program`) decodes them into a program — size-bucketed seeds
//! (reaching Karatsuba/Toom-Cook/FFT) plus a chain of add/sub/mul/neg — and panics on any
//! divergence. A panic is the bug report; libFuzzer saves the crashing input.
use libfuzzer_sys::fuzz_target;
use std::sync::Once;

static INIT: Once = Once::new();

fuzz_target!(|data: &[u8]| {
    // dashu `tuning`: pull every multiply-algorithm threshold DOWN so small operands route
    // through Karatsuba / Toom-3 / NTT. Same algorithm code as production — only the crossover
    // size changes — so we exercise those paths at sizes where the proven O(n²) `RefBackend`
    // oracle stays cheap. Without this, NTT needs ≥ 4000-limb operands (dashu-int mul/mod.rs).
    //
    // The values must respect each algorithm's own MIN_LEN precondition (dashu asserts them):
    // Karatsuba ≥ 3, Toom-3 ≥ 16 (mul/{karatsuba,toom_3}.rs). So `simple > 1`, `karatsuba ≥ 15`.
    // NTT floor is larger (an FFT needs a real transform size) — 160 keeps it valid yet cheap.
    INIT.call_once(|| {
        std::env::set_var("DASHU_THRESHOLD_SIMPLE_MUL", "2"); // >2 ⇒ Karatsuba (MIN_LEN 3)
        std::env::set_var("DASHU_THRESHOLD_KARATSUBA_MUL", "16"); // >16 ⇒ Toom-3 (MIN_LEN 16)
        std::env::set_var("DASHU_THRESHOLD_NTT_MUL", "160"); // >160 ⇒ NTT
    });
    lattice::ratfuzz::run_int_program(data);
});
