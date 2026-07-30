# backend-select — throwaway bignum backend selection benchmark

Picks the `lattice` exact-arithmetic backend (M0 task 2) under two criteria:

1. **Hard no_std+alloc gate** (`gate/`) — each candidate is depended on with
   `default-features = false` and compiled for `thumbv7em-none-eabi` (a target
   with no `std`). A candidate that pulls `std` transitively fails to compile.
   Compilation *is* the gate; the crate is a `lib`, so no allocator/binary is
   linked.
2. **Speed yardstick** (`selector/`) — a degree-12 Sturm polynomial-remainder
   sequence over ~240-bit rational coefficients (Sturm's inner loop; no root
   isolation), run identically against each rational-capable backend via a small
   uniform `Rat` trait. Naive Euclidean PRS deliberately triggers Sturm
   coefficient explosion — that is the bignum stress. All backends must agree on
   the computed root count (a free cross-check).

This is a **self-contained workspace** (its own `Cargo.lock`), excluded from the
root workspace so criterion/the candidate crates never enter the kernel's
lockfile or CI. Nothing in `lattice` depends on it.

## Reproduce

```sh
# from repo root, inside `nix develop`
cd benchmarks/backend-select

# (1) no_std gate — expect a clean build (exit 0) for each PASS:
for f in dashu num malachite ibig; do
  cargo build -p gate --no-default-features --features "$f" --target thumbv7em-none-eabi
done

# (2) speed yardstick (criterion; ~3-4 min — num-rational is the slow pole):
cargo bench
```

Candidates: `dashu` (int+rational), `num-bigint`+`num-rational`, `malachite`
(int+rational), `ibig` (**integer-only** — excluded from the rational yardstick).

The decision and recorded results live in
[`../../docs/lattice-backend-benchmark.md`](../../docs/lattice-backend-benchmark.md).
