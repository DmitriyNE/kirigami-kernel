# M0 bignum backend selection — benchmark & decision

Status: decided 2026-07-30. Resolves the M0 task-2 backend choice (`vv-guide §8`,
`environment-and-crate-layout.md §1/§6`). The benchmark is a self-contained,
throwaway workspace at `benchmarks/backend-select/` (excluded from the root
workspace; not built by CI). Reproduce per its `README.md`.

## Decision

**`dashu` is the `lattice` bignum backend** (`dashu::integer::IBig` /
`dashu::rational::RBig`), behind `lattice::backend::Backend`. **`num-bigint` +
`num-rational` is the differential second backend** (test-only, `vv-guide §3/§6`).

Rationale, against the two criteria:

- **no_std + alloc gate (hard):** dashu passes. (So do all candidates at the
  tested revs — see below; the gate did not eliminate anyone, contrary to the
  going-in worry that malachite might require `std`.)
- **speed:** dashu ties malachite for fastest and is ~47× faster than
  num-rational on the yardstick. malachite is *not* decisively faster than dashu,
  so the tiebreak goes to the pre-declared bias (dashu) — chosen for its native
  integer **and** rational types, clean `no_std` story, and ergonomics.
- **differential oracle:** picking dashu leaves num-rational (an independent
  implementation) as the second backend for the `lattice cmp/sign` differential
  row. Its slowness is irrelevant for a correctness oracle exercised only in
  tests.

ibig is **integer-only** (no native rational; its author's successor is dashu),
so it is disqualified for the rational yardstick — it would need a hand-built
rational layer that dashu already provides.

## Criterion 1 — no_std + alloc gate

`cargo build -p gate --no-default-features --features <cand> --target thumbv7em-none-eabi`
(a target with no `std`; compilation is the gate).

| candidate | tested rev | `--target thumbv7em-none-eabi` | notes |
|---|---|---|---|
| dashu | 0.4.4 | ✅ compiles | int + rational, `default-features = false` |
| num-bigint + num-rational | 0.4.8 / 0.4.2 | ✅ compiles | `Ratio<BigInt>` (BigRational alias needs the num-bigint feature) |
| malachite | 0.4.22 | ✅ compiles | features `naturals_and_integers`, `rationals`; `std` off |
| ibig | 0.3.6 | ✅ compiles | **integer-only** — no rational type |

## Criterion 2 — speed yardstick

Degree-12 Sturm polynomial-remainder-sequence over ~240-bit rational coefficients
(naive Euclidean PRS → deliberate Sturm coefficient explosion; the bignum stress).
9 runs/backend, `aarch64-darwin`, rustc 1.96.0, `--release`, under `caffeinate -i`
(no idle-sleep). All backends compute the identical chain (see the fingerprint
below) and agree on the root count.

| backend | min (ms) | median (ms) | max (ms) | relative (min) |
|---|---:|---:|---:|---:|
| dashu 0.4.4 | 362.6 | 363.6 | 365.6 | 1.00× |
| malachite 0.4.22 | 363.8 | 364.7 | 366.0 | 1.00× |
| num-rational 0.4.2 | 16924.9 | 16932.9 | 16938.9 | 46.7× |

Cross-check: all three agree, root count = 0 (this fixed pseudo-random degree-12
polynomial has no real roots in (−1000, 1000]).

**Workload fingerprint (identical across all three backends).** The Sturm chain
is 13 entries; the leading-coefficient decimal-digit count per entry explodes:

```
(degree, leading-coeff digits):
(12,147) (11,146) (10,583) (9,1877) (8,4188) (7,8356) (6,14400)
(5,23294) (4,34207) (3,48392) (2,64478) (1,83693) (0,104225)
Σ = 387986 digits
```

i.e. the last chain entries carry coefficients of ~100 000 decimal digits
(~330 000 bits, ~40 KB integers). This fingerprint is asserted equal for all
three backends, so they do the *same* work — the timing gap is pure backend
arithmetic speed on very large integers.

**On the dashu ≈ malachite tie (was it suspicious?).** No — with 9 runs it is a
genuine near-tie, not an identical value: dashu median 363.6 ms vs malachite
364.7 ms (~0.3% apart), each with ~1% run-to-run spread and no outliers. Two
FFT-/Toom-grade bignum libraries land within noise of each other on this
multiply-and-GCD-heavy workload. dashu is marginally ahead.

**On num-rational's 47× slowness (real, not a harness bug).** The chain reaches
~100 000-digit coefficients, and num-rational reduces to lowest terms after every
operation via num-bigint's GCD. num-bigint uses schoolbook/Karatsuba
multiplication and a comparatively slow big-integer GCD/division; dashu and
malachite use asymptotically faster large-integer algorithms. On tens-of-
thousands-of-digit operands that compounds into the ~47× gap. It is a property of
num-bigint on huge integers, *not* of the benchmark — and it is irrelevant to
num-rational's role as the differential oracle, which runs on modest test-sized
rationals, never this exploded workload.

## Notes / caveats

- Newer majors exist (dashu 0.5.1, malachite 0.10, num-bigint 0.5). The tested
  0.4.x revs are recent and the decision is structural (native rationals + no_std
  + speed tier), so it is stable across the bump; adopting dashu 0.5 in `lattice`
  later is a trivial follow-up and its `RBig` surface we use (`from_parts`,
  arithmetic ops, `cmp`) is unchanged.
- The speed harness uses `std::time::Instant` min-of-N rather than criterion — a
  throwaway selector does not need distribution statistics to separate a 47× gap
  and a dead heat.
- Standing gate after selection: `lattice` is `#![no_std]`, so
  `cargo build -p lattice --target thumbv7em-none-eabi` (wired into CI) is the
  durable no_std+alloc regression guard on the real `lattice` + dashu graph.
