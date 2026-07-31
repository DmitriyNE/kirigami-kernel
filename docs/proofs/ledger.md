# Proof ledger — index

The theorems the `certify-core` checkers rely on. See [`README.md`](README.md) for
the field semantics and how this relates to `certify-check/` (the actual proofs)
and `../../vv-matrix.md`. Status: ✅ proven in Lean · 📌 proven modulo one labelled
cited axiom · ⬜ not yet formalized. All ✅/📌 rows are in the CI `#print axioms`
gate (`.github/workflows/ci.yml`).

| Obligation | Rust checker | Lean | Status · axioms |
|---|---|---|---|
| [Sturm variation](sturm.md) | `sturm::SturmChain::verify_chain` | `SturmChecker.lean` | 📌 `[propext, sturm_root_count, Classical.choice, Quot.sound]` — Sturm cited (not in Mathlib) |
| [Resultant ⇔ common root](resultant.md) | `resultant::verify_common_factor` | `Resultant.lean` | ✅ `[propext, Classical.choice, Quot.sound]` — no cited axiom |
| [Fast-path gcd](fast-path.md#gcd) | `small::gcd_u128` | `GcdReduce.lean` | ✅ `[propext, Classical.choice, Quot.sound]` |
| [Fast-path reduce](fast-path.md#reduce) | `small::SmallRat::reduce` | `Reduce.lean` | ✅ `[propext, Classical.choice, Quot.sound]` |
| Sylvester criterion (strict) | *(shell tier — M4+)* | — | ⬜ TODO |
