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
| [Fast-path gcd](fast-path.md#gcd) | `small::gcd_u128` | `GcdReduce.lean` | ✅ `[propext, Classical.choice, Quot.sound]` — **re-proven for the OPT.3 strip-twos + `u64`-narrowed implementation** (2026-08-15), same axiom footprint as before. `gcd_two_pow_mul` (`gcd(2^i·m, 2^j·n) = 2^min(i,j)·gcd(m,n)`) carries the new algorithm; the Euclidean loop argument is reused at both widths. `trailing_zeros` is a *faithful `def`*, not an axiom, so nothing leaked into the footprint |
| [Fast-path reduce](fast-path.md#reduce) | `small::SmallRat::reduce` | `Reduce.lean` | ✅ `[propext, Classical.choice, Quot.sound]` |
| ℤ₂² cocycle closure (§6 step 4) | `arrange::cocycle_ok` | Kani (bounded) | ✅ `cocycle_implies_telescoping` — bounded DCEL ≤4 cells / 5 edges (vv-guide §5:73); accept ⇒ every closed walk returns its bits |
| CAP-OUT-LINK / V_∂ membership (§8.5) | `arrange::{classify_link,link_ok,v_boundary}` | Kani (bounded) + Aeneas lift (Lean) | ✅ `link_ok_iff_no_pinch` (Kani, ≤6 masks) **and** ✅ `CertifyCheck.link_ok_spec` (Lean, axiom-clean): the Aeneas-lifted `cyclic_true_runs` provably computes the cyclic-run count (`cyclic_true_runs_spec` via `loop.spec_decr_nat`), so `link_ok ↔ ≤1 run` deductively — the unbounded Lean analogue of the bounded Kani proof. Also `CertifyCheck.CapOut` dispatch soundness. 2-manifold thm = research frontier |
| Link_emitted ≅ Link_geometric (§8.5) | `arrange::link_iso_ok` | Kani (bounded) + Aeneas lift | ✅ `link_iso_matches_cyclic_adjacency` — over all permutations N=4, the rotation-search matches the cyclic-adjacency reference. Also Aeneas-lifted (axiom-clean, sorry-free); full iso-spec refinement = frontier |
| Sylvester criterion (strict) | *(shell tier — M4+)* | — | ⬜ TODO |
