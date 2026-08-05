#![no_main]
//! Coverage-guided op-chain differential: `dashu` ≡ the *proven* `RefBackend` over
//! large, chained integer arithmetic. libFuzzer mutates the raw bytes; the shared core
//! (`lattice::ratfuzz::run_int_program`) decodes them into a program — size-bucketed seeds
//! (reaching Karatsuba/Toom-Cook/FFT) plus a chain of add/sub/mul/neg — and panics on any
//! divergence. A panic is the bug report; libFuzzer saves the crashing input.
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    lattice::ratfuzz::run_int_program(data);
});
