# Two-tier (`Fast(i128)`/`Slow(dashu)`) vs dashu-only — benchmark & decision

Status: measured 2026-08-04. Answers a previously-unmeasured question.

The M0 backend benchmark (`docs/lattice-backend-benchmark.md`) picked dashu on a
degree-12 Sturm PRS that explodes to ~100 000-digit coefficients — a workload where the
i128 fast path *never fires*. So it never measured whether the two-tier overlay
(`lattice::Rat/Int = Fast(i128) | Slow(dashu)`) beats **dashu-only** in the
small-coordinate regime the fast path actually targets. And since dashu already stores
values up to ~two machine words **inline** (no heap allocation below ~128 bits), the
overlay is *not* buying allocation-avoidance — the open question was whether skipping
dashu's per-op dispatch (and a native i128 gcd) is worth the two-tier's complexity and its
`fast≡slow` Kani obligation.

Benchmark: `benchmarks/two-tier-vs-dashu/` (throwaway workspace, its own lockfile, not in
CI). `lattice::Rat<Bignum>`/`Int<Bignum>` (two-tier) vs `Backend::rat_*`/`int_*` on
`BigRat`/`BigInt` (dashu-only) — **identical exact ℚ arithmetic**, the only difference
being the i128 fast path. criterion, aarch64-darwin, rustc 1.96.0.

## Results

| workload (small-coordinate) | two-tier | dashu-only | two-tier speedup |
|---|---:|---:|---:|
| integer dot `Σ aᵢ·bᵢ`                     | 8.50 µs  | 27.60 µs | **3.2×** |
| rational comparison `a < b`               | 11.70 µs | 19.51 µs | **1.67×** |
| rational 2×2 determinant `a·d − b·c`      | 638 µs   | 665 µs   | 1.04× (tie) |
| product overflowing i128 (crossover)      | 731 ns   | 1212 ns  | **1.66×** |

## Reading

- **Integer arithmetic — the fast path wins big (3.2×).** Native `i128` add/mul skips
  dashu's per-op representation-check → general add-with-carry → normalize *entirely*.
  dashu's inline-small-value optimization removes the *allocation*, but not the *dispatch*
  — and the dispatch is what the fast path removes.
- **Rational comparison — 1.67×.** A cheap op (cross-multiply + sign, no reduction), so the
  skipped dispatch is a large fraction of the total cost.
- **Rational 2×2 determinant — a tie (~4%).** This is the *most expensive* op and the core
  geometry predicate. It is dominated by the gcd **reduction**, where the native i128 gcd is
  not meaningfully faster than dashu's on these ~18-bit operands — the dispatch savings are
  swamped by the reduction cost.
- **Crossover (values exceed i128) — still 1.66× faster, no penalty.** Even when the running
  product grows past i128, the early small ops run fast-path while dashu-only runs bignum
  throughout; the overflow-check + promotion cost is negligible. The two-tier never loses.

## Decision — keep the two-tier

Empirically justified: **1.6–3.2× on integer- and comparison-heavy code, neutral (never
worse) on heavy rational reduction and at the i128 crossover.** It is *not* dead weight —
dashu's inlining does not eliminate the per-op dispatch the fast path skips.

Caveat, stated honestly: the single most expensive rational op — the 2×2 determinant, the
core predicate — is a wash. A kernel whose time is dominated by rational determinants gains
little in aggregate; but it never regresses, and the integer (resultant/Bareiss) and
comparison-heavy sections clearly benefit.

Orthogonal to the algebra-lift direction: modeling `Int = ℤ` / `Rat = ℚ` abstracts the
`Fast`/`Slow` tiers away, so keeping the two-tier does not complicate the
extraction/trust story.
