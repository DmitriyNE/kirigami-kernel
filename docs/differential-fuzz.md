# Differential fuzzing — `dashu` ≡ the proven `RefBackend`

How the default runtime bignum backend (`dashu`) is continuously checked against a *proven*
reference. Companion to [`algebra-trust.md`](algebra-trust.md) (the trust doctrine) and
[`refbackend-lift.md`](refbackend-lift.md) (the `RefBackend = ℤ/ℚ` proof). Adopted 2026-08-06.

## Why

`lattice::Int`/`Rat` are a two-tier `Fast(i128) | Slow(dashu)` type; the slow leaves are `dashu`.
`dashu`'s correctness is *tested*, not proven — the confidence is **transferred** to it by a
differential against `RefBackend`, which *is* proven `= ℤ/ℚ` in Lean. So any `dashu` ↔ `RefBackend`
disagreement is a genuine `dashu` (or two-tier) bug, not oracle ambiguity — a **proof-backed
oracle**, not a cross-check against a trusted model.

The op-chain fuzzer (`lattice::ratfuzz`, `crates/lattice/src/ratfuzz.rs`) closes two gaps the old
single-op `rat::differential` had:

- **Op-chains.** A value is walked across the two-tier boundary through a *sequence* of ops (the
  intermediate results accumulate), which is where two-tier canonicalization bugs live — a single
  op over fresh operands never gets there. The fuzzer decodes its input into a little program over
  a 4-register file and runs it in lockstep through both backends, asserting agreement at each step.
- **Large operands.** Seeds are built from a byte string of *chosen length*, so `dashu`'s multiply
  ladder actually runs. The old `from_i128` seeds are ≤ 2 limbs, so `dashu` never left the
  schoolbook base case — Karatsuba / Toom-Cook / NTT went 100 % unexercised.

## How it works

- **Core:** `ratfuzz::run_int_program(&[u8])` — byte-driven so *one* implementation serves both the
  always-on proptest and the coverage-guided `cargo-fuzz` target. It seeds the registers (same bytes
  into both backends, seed equality asserted), then runs a chain of `add/sub/mul/neg`, comparing via
  an **O(n) little-endian-bytes** canonical form (decimal is O(n²) and throttles at large operands).
- **Oracle + metamorphic.** `RefBackend`'s `mul` is schoolbook O(n²) — correct, simple, *proven* — so
  it is a live oracle up to ~10⁵ bits (a growth guard, `MAX_LIMBS`, keeps operands there). On top,
  size-independent **metamorphic** identities (`a·b = b·a`, `(a+b)·c = a·c + b·c`) catch multiply
  bugs with no reference at all.
- **Size buckets, pinned to `dashu`'s thresholds.** `dashu-int` 0.4.3 dispatches on the *smaller*
  operand's limb length (`mul/mod.rs`): schoolbook ≤ 24 · Karatsuba 25–96 · Toom-3 97–4000 · NTT >
  4000. The seed buckets straddle each crossover ±1 so limb-splitting off-by-ones get hit.
- **NTT at cheap sizes via `tuning`.** Reaching NTT with production thresholds needs ≥ 4000-limb
  operands (slow for the O(n²) oracle). Instead the `cargo-fuzz` build enables `dashu`'s `tuning`
  feature (`lattice` feature `fuzzing = ["dashu/tuning"]`) and the target lowers the thresholds by
  env var (`SIMPLE=2 / KARATSUBA=16 / NTT=160`), so tiny operands route through Karatsuba/Toom-3/**NTT**
  — same algorithm code, smaller crossover — while the oracle stays cheap. The lowered values MUST
  respect each algorithm's own `MIN_LEN` (Karatsuba 3, Toom-3 16); `dashu` asserts them.

## Running it

```sh
# (re)generate the threshold-straddling seed corpus from the authoritative encoder
cd fuzz && cargo run --bin gen_corpus

# coverage-guided campaign (needs `cargo install cargo-fuzz` + a nightly with libFuzzer)
cargo +nightly fuzz run int_chain -- -max_total_time=300
```

## CI cadence — determinism is the axis

Fuzzing is a nondeterministic, time-boxed **search** whose value compounds with a persisted corpus,
so it is a soak, not a gate. The split:

- **Per-PR (stable, no libFuzzer):** the *deterministic replay* — the `replay_seed_corpus` unit test
  (in `cargo nextest`) replays the curated seed programs, and `tests/fuzz_replay.rs` replays the
  committed crash corpus (`crates/lattice/tests/fuzz-corpus/int_chain/`) under the fuzzer's tuning
  (ci.yml step `cargo test -p lattice --features fuzzing --test fuzz_replay`). Guarantees *no known
  input regresses* — which needs neither nightly nor libFuzzer.
- **Nightly (the search):** `.github/workflows/fuzz-nightly.yml` — `cargo-fuzz` on the runner's rustup
  (outside nix, like the dylint step), corpus persisted via `actions/cache`, crash inputs uploaded as
  artifacts.

**Triaging a crash:** minimize the uploaded artifact (`cargo fuzz tmin int_chain <artifact>`) and drop
the minimized `.bin` into `crates/lattice/tests/fuzz-corpus/int_chain/` — it is then replayed on every
PR forever.

## Scope & caveats

- `RefBackend` is **never** production (a slow O(n²)/bit-serial oracle) — the two-tier keeps `dashu`
  as the default. `RefBackend`'s only job is to be the *proven* oracle.
- **Integer surface only** (`add/sub/mul/neg`). A rational op-chain variant is deferred: `RefBackend`'s
  bit-serial `divrem`/`gcd` are too slow as a large-operand oracle — use metamorphic identities there.
- The unproven seed constructor `RefBackend::int_from_le_bytes` is TEST/FUZZ-ONLY
  (`#[cfg(any(test, feature = "fuzzing"))]`, banner on the fn); if it ever enters the `Backend` trait
  or the Aeneas lift it must first be proven `den = value`, like `from_i128`.
