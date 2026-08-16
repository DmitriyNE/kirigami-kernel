//! Differential-fuzz core (algebra-rehaul follow-up): op-**chains** of `dashu` ≡ the proven
//! `RefBackend`, over **size-bucketed large operands**.
//!
//! Two gaps in the old single-op `rat::differential`, both addressed here:
//!  - **op-chains** — a value is walked across the two-tier `Fast(i128)|Slow(bignum)` boundary
//!    through a sequence of ops (the intermediate results accumulate), where two-tier
//!    canonicalization bugs actually live; a single op over fresh operands never gets there.
//!  - **large operands** — seeds are drawn from a byte string of *chosen length*, bucketed to
//!    straddle every multiply-algorithm threshold (base-case, Karatsuba, Toom-Cook, FFT). The
//!    old `from_i128` seeds are ≤ 2 limbs, so dashu never leaves the schoolbook base case — its
//!    most intricate code (Toom/FFT) went 100% unexercised.
//!
//! `RefBackend` is *proven* `= ℤ` (`certify-check/CertifyCheck/RefBackend.lean`), so any
//! divergence is a genuine dashu bug, not oracle ambiguity. Its `mul` is schoolbook O(n²)
//! (correct, simple, proven) — a live oracle up to ~10⁵ bits; the growth guard keeps operands
//! there. Beyond that, metamorphic identities (commutativity / distributivity) catch multiply
//! bugs with no reference at all.
//!
//! Byte-driven so **one core** serves two frontends: the `#[cfg(test)]` proptest below (always-on,
//! no new toolchain) and the out-of-tree `cargo-fuzz` target (`fuzz/fuzz_targets/int_chain.rs`),
//! which mutates the same bytes under coverage guidance.

use crate::backend::Backend; // brings `int_add`/`int_sub`/`int_mul`/`int_neg` into scope
use crate::bignum::{BigInt, Bignum};
use crate::refbackend::{RefBackend, RefInt};
use alloc::vec::Vec;
use dashu::integer::{IBig, UBig};

/// Registers in the little program.
const NREG: usize = 4;
/// Cap on chain length (bounds worst-case runtime per input).
const MAX_STEPS: usize = 64;
/// Operand-growth ceiling (limbs) so `RefBackend`'s O(n²) `mul` stays a *live* oracle. The
/// cargo-fuzz build lowers dashu's NTT threshold to ~160 limbs (`int_chain.rs`), so 512 limbs
/// (~32 Kbit) comfortably exercises every algorithm — including NTT — at oracle-cheap sizes.
const MAX_LIMBS: usize = 512;

/// A dashu `BigInt` as (sign, minimal little-endian magnitude bytes) — the O(n) canonical
/// form we compare on (decimal is O(n²) and would throttle the fuzzer at large operands).
fn dashu_le_bytes(a: &BigInt) -> (bool, Vec<u8>) {
    let neg = a.0 < IBig::ZERO;
    let bytes = a.0.clone().into_parts().1.to_le_bytes(); // (Sign, UBig) → magnitude Box<[u8]>
    (neg && !bytes.is_empty(), bytes.to_vec())
}

/// The `dashu` counterpart of `RefBackend::int_from_le_bytes`, from the *same* bytes — so a
/// seed disagreement (a constructor bug on either side) is caught before the chain runs.
fn dashu_from_le_bytes(neg: bool, bytes: &[u8]) -> BigInt {
    let mag = IBig::from(UBig::from_le_bytes(bytes));
    BigInt(if neg { -mag } else { mag })
}

/// Byte cursor over the fuzzer's input: reads a program out of it, **clamping** (not wrapping)
/// at the end — running out just yields `0` bytes / empty slices, so *any* input is a valid
/// program (good for coverage-guided mutation).
struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}
impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Cursor { data, pos: 0 }
    }
    fn byte(&mut self) -> u8 {
        let b = self.data.get(self.pos).copied().unwrap_or(0);
        self.pos = self.pos.saturating_add(1);
        b
    }
    fn take(&mut self, n: usize) -> &'a [u8] {
        let start = self.pos.min(self.data.len());
        let end = self.pos.saturating_add(n).min(self.data.len());
        self.pos = end;
        &self.data[start..end]
    }
    fn done(&self) -> bool {
        self.pos >= self.data.len()
    }
}

/// Seed sizes (in 64-bit **limbs**), pinned to dashu-int 0.4.3's multiply-algorithm thresholds
/// (dispatch keys on the *smaller* operand's limb length, `mul/mod.rs`):
///   schoolbook ≤ 24 · Karatsuba 25..96 · Toom-3 97..4000 · NTT > 4000.
/// We straddle each crossover with ±1 so off-by-one limb-splitting bugs get hit. The always-on
/// (production-dashu) proptest reaches Toom-3; the cargo-fuzz build lowers the thresholds via the
/// `tuning` env vars so *these same small sizes* route through NTT too.
const LIMB_BUCKETS: [usize; 16] = [0, 1, 2, 4, 8, 16, 23, 24, 25, 48, 64, 95, 96, 97, 160, 256];

/// Byte → seed length (bytes) = a limb-bucket plus a few bytes of jitter (so the top limb is
/// sometimes partially filled). Actual size is also clamped by how much input remains.
fn seed_bytes_len(sel: u8, jit: u8) -> usize {
    let limbs = LIMB_BUCKETS[(sel as usize) % LIMB_BUCKETS.len()];
    (limbs * 8)
        .saturating_add((jit % 15) as usize)
        .saturating_sub(7) // ⇒ ±7 bytes of jitter around the 8-byte limb boundary
}

/// Run `data` as an integer op-chain through both backends, asserting agreement at every step
/// (plus metamorphic multiply identities on dashu). A divergence **panics** — that panic is the
/// bug report (proptest shrinks the input; cargo-fuzz saves the crashing case).
///
/// The cargo-fuzz entry point: uses the full [`MAX_LIMBS`] cap (release build → reaches FFT).
pub fn run_int_program(data: &[u8]) {
    run_int_program_capped(data, MAX_LIMBS)
}

/// As [`run_int_program`], but with an explicit operand-growth cap so the always-on (debug)
/// proptest can stay in the Karatsuba/Toom range while the cargo-fuzz soak goes to FFT scale.
fn run_int_program_capped(data: &[u8], max_limbs: usize) {
    let mut cur = Cursor::new(data);

    // Seed the registers with size-bucketed operands — the *same* bytes into both backends.
    let mut rr: Vec<RefInt> = Vec::with_capacity(NREG);
    let mut rd: Vec<BigInt> = Vec::with_capacity(NREG);
    for _ in 0..NREG {
        let neg = cur.byte() & 1 == 1;
        let n = seed_bytes_len(cur.byte(), cur.byte());
        let bytes = cur.take(n);
        let a = RefBackend::int_from_le_bytes(neg, bytes);
        let b = dashu_from_le_bytes(neg, bytes);
        assert_eq!(
            RefBackend::int_le_bytes(&a),
            dashu_le_bytes(&b),
            "seed constructor divergence"
        );
        rr.push(a);
        rd.push(b);
    }

    // The op chain: dashu vs proven RefBackend, step-by-step.
    let mut steps = 0;
    while !cur.done() && steps < MAX_STEPS {
        steps += 1;
        let opcode = cur.byte();
        let x = (cur.byte() as usize) % NREG;
        let y = (cur.byte() as usize) % NREG;
        let d = (cur.byte() as usize) % NREG;
        match opcode % 4 {
            0 => {
                rr[d] = RefBackend::int_add(&rr[x], &rr[y]);
                rd[d] = Bignum::int_add(&rd[x], &rd[y]);
            }
            1 => {
                rr[d] = RefBackend::int_sub(&rr[x], &rr[y]);
                rd[d] = Bignum::int_sub(&rd[x], &rd[y]);
            }
            2 => {
                // Guard runaway growth so the O(n²) reference stays fast enough to be a live
                // oracle; when we'd blow the cap, do an add instead (both backends decide from
                // the same limb counts, so they stay in lockstep).
                if RefBackend::int_limbs(&rr[x]) + RefBackend::int_limbs(&rr[y]) <= max_limbs {
                    let dm = Bignum::int_mul(&rd[x], &rd[y]);
                    // metamorphic: multiply commutes (dashu-only — needs no reference).
                    assert_eq!(
                        dashu_le_bytes(&dm),
                        dashu_le_bytes(&Bignum::int_mul(&rd[y], &rd[x])),
                        "mul not commutative"
                    );
                    rr[d] = RefBackend::int_mul(&rr[x], &rr[y]);
                    rd[d] = dm;
                } else {
                    rr[d] = RefBackend::int_add(&rr[x], &rr[y]);
                    rd[d] = Bignum::int_add(&rd[x], &rd[y]);
                }
            }
            // `opcode % 4` is `0..=3`, so this arm *is* case 3. Written as the catch-all rather
            // than `3 => … , _ => unreachable!()` because the pure tier forbids panic-capable
            // constructs (`docs/trusted-invariants.md`), and removing the panic path outright is
            // better than discharging one that can never be taken.
            _ => {
                rr[d] = RefBackend::int_neg(&rr[x]);
                rd[d] = Bignum::int_neg(&rd[x]);
            }
        }
        assert_eq!(
            RefBackend::int_le_bytes(&rr[d]),
            dashu_le_bytes(&rd[d]),
            "op-chain divergence (opcode {})",
            opcode % 4
        );
    }

    // Final metamorphic sweep: distributivity (a+b)·c = a·c + b·c on dashu — a multiply check
    // that holds at *any* size, no reference needed.
    let sz = RefBackend::int_limbs(&rr[0]).max(RefBackend::int_limbs(&rr[1]))
        + RefBackend::int_limbs(&rr[2]);
    if sz <= max_limbs {
        let lhs = Bignum::int_mul(&Bignum::int_add(&rd[0], &rd[1]), &rd[2]);
        let rhs = Bignum::int_add(
            &Bignum::int_mul(&rd[0], &rd[2]),
            &Bignum::int_mul(&rd[1], &rd[2]),
        );
        assert_eq!(
            dashu_le_bytes(&lhs),
            dashu_le_bytes(&rhs),
            "mul not distributive"
        );
    }
}

/// A small **seed corpus** of encoded programs — one per interesting operand size, each seeding
/// all registers at that size and then running a multiply chain — so libFuzzer starts already
/// hitting every algorithm (schoolbook / Karatsuba / Toom-3, and NTT under the fuzz build's
/// lowered thresholds) instead of rediscovering them from scratch. Written to
/// `fuzz/corpus/int_chain/` by the `gen_corpus` bin; the fuzzer mutates onward from these.
pub fn corpus_seeds() -> Vec<Vec<u8>> {
    // A program: `NREG` seeds of `limbs` words each, then a chain of muls (opcode % 4 == 2).
    fn program(limbs: usize, fill: u8) -> Vec<u8> {
        let sel = LIMB_BUCKETS.iter().position(|&b| b == limbs).unwrap_or(0) as u8;
        let mut p = Vec::new();
        for r in 0..NREG as u8 {
            p.push(r & 1); // neg
            p.push(sel); // size bucket → `limbs`
            p.push(7); // jitter 7 ⇒ exactly limbs*8 magnitude bytes
            for i in 0..(limbs * 8) {
                p.push(
                    fill.wrapping_add(i as u8)
                        .wrapping_mul(3)
                        .wrapping_add(r + 1),
                );
            }
        }
        for k in 0..8u8 {
            p.push(2); // opcode ⇒ mul
            p.push(k % NREG as u8); // x
            p.push((k + 1) % NREG as u8); // y
            p.push((k + 2) % NREG as u8); // d
        }
        p
    }
    let mut out = Vec::new();
    // straddle the schoolbook/Karatsuba (24) and Karatsuba/Toom-3 (96) crossovers, plus a
    // couple larger sizes (→ NTT once the fuzz build lowers the thresholds).
    for &limbs in &[2usize, 8, 24, 25, 96, 97, 256] {
        out.push(program(limbs, 0x5a));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        // Always-on op-chain differential (no new toolchain). Runs in debug, so it stays in the
        // Karatsuba/Toom range (cap 384 limbs ≈ 24 Kbit) to keep the O(n²) reference fast; the
        // cargo-fuzz soak (release) drives the same core to FFT scale. The cheap 80% that gates
        // every PR — coverage-guided depth is the fuzzer's job.
        #![proptest_config(ProptestConfig::with_cases(48))]

        #[test]
        fn int_chain_dashu_matches_ref(program in proptest::collection::vec(any::<u8>(), 0..2048)) {
            run_int_program_capped(&program, 384);
        }
    }

    #[test]
    fn replay_seed_corpus() {
        // Deterministic per-PR regression gate: the curated threshold-straddling seed programs
        // must never diverge. Runs under *production* dashu thresholds (no `tuning`), so it
        // exercises schoolbook / Karatsuba / Toom-3; the NTT path is the nightly cargo-fuzz cron's
        // job (`.github/workflows/fuzz-nightly.yml`). No libFuzzer, no nightly — plain `cargo test`.
        for prog in corpus_seeds() {
            run_int_program(&prog);
        }
    }

    #[test]
    fn seed_ctor_roundtrips_against_dashu() {
        // A directed smoke test of the seed constructor at a few sizes across the buckets.
        for &len in &[0usize, 1, 7, 8, 9, 64, 520] {
            let bytes: Vec<u8> = (0..len)
                .map(|i| (i as u8).wrapping_mul(37).wrapping_add(1))
                .collect();
            for neg in [false, true] {
                let a = RefBackend::int_from_le_bytes(neg, &bytes);
                let b = dashu_from_le_bytes(neg, &bytes);
                assert_eq!(RefBackend::int_le_bytes(&a), dashu_le_bytes(&b));
            }
        }
    }
}
