# V&V matrix (stub — CI gates on this)

Row per certificate/kernel operation; cell per method. Status: ✅ done · 🚧 partial · ⬜ todo · N/A.
CI fails a milestone gate if a **soundness-critical** row (marked ★) has empty {Kani ∨ Lean ∨ runtime-checked-hypothesis}.

| Item | crate | unit | property | differential | Kani | Lean | validation |
|---|---|---|---|---|---|---|---|
| lattice cmp/sign ★ | lattice | ✅ | ✅ (randomized diff) | ✅ (num-rational) | ✅ (panic-free full i128; fast≡slow via exhaustive+diff) | ⬜ (spike GO; gcd/reduce Lean gated Phase 5) | — |
| Sturm isolate ★ | lattice | ✅ | ✅ (proptest) | ✅ (constructed roots) | ✅ (sign-count) | 🚧 `SturmChecker.lean` (checker formalized; Sturm = 1 cited axiom) · `sign_variations` proven axiom-clean, incl. the **Aeneas-lifted model** end-to-end (`Refine.lean`) · rc-hyp ✅ | — |
| resultant ★ | lattice | ✅ | ✅ | ✅ (vs Poly::gcd) | — | ✅ `verify_common_factor_sound` (`Resultant.lean`): witness ⟹ `¬IsCoprime f g` ⟹ `resultant f g = 0`, **axiom-clean, NO cited axiom** (Mathlib `resultant_eq_zero_iff` closed the gap) · rc-hyp ✅ | — |
| CLIP-σ signed ★ | certify1d | ⬜ | ⬜ | — | — | ⬜ | — |
| strict Sylvester ★ | certify1d | ⬜ | ⬜ | — | ⬜ | ⬜ | — |
| occupancy→row ★ | sew | ⬜ | ⬜ | — | ⬜ (≤6 bits) | ⬜ | — |
| quotient emission ★ | arrange2d | ⬜ | ⬜ (Euler) | ⬜ (CGAL) | — | ⬜ (research) | — |
| CAP-OUT-LINK ★ | arrange2d | ⬜ | ⬜ | — | — | ⬜ (research) | — |
| Link_emitted≅geom ★ | sew | ⬜ | ⬜ | — | ⬜ (bounded) | ⬜ | — |
| completeness bijections | arrange2d | ⬜ | ⬜ | — | ⬜ (bounded) | — | — |
| device cone chart | geom | ⬜ | — | — | — | — | ⬜ (golden) |
| STEP shell | export | ⬜ | — | ⬜ (OCC) | — | — | ⬜ (kernels + hw) |
| (extend per implementation-plan §1) | | | | | | | |

**Notes.** (`rc-hyp` = runtime-checked-hypothesis cell, tracked in `docs/proofs/ledger.md`.)
- `lattice cmp/sign` (slice 1): unit = corpus seeds + `Bignum` (>i128) + the exhaustive
  ±24 grid; property = the proptest differential; differential = num-rational (2nd backend).
  **Kani** = `neg`/`sign`/`cmp` panic-/overflow-freedom over the full i128 domain (incl
  `i128::MIN`); fast≡slow *correctness* is carried by the exhaustive grid + differential.
  The 128-bit gcd loop is CBMC-expensive, so **gcd/reduce correctness is owned by Lean at
  the §7 spike** — the settled tool-fit decision: Kani keeps the gcd-free bridge +
  panic-freedom, no binary-GCD bandage; the `*_correct_i16` harnesses are authored for that.
- `Sturm isolate` (slice 2): the variation theorem is a **runtime-checked hypothesis**
  (`SturmChain::verify_chain`) filling the soundness cell; **Kani** proves the finite
  `sign_variations` counter; property/differential = the constructed-roots proptest. Lean at
  the spike.
- `resultant` (slice 2): resultant⇔common-root is a **runtime-checked hypothesis**
  (`verify_common_factor` — the spec §5.3 divisibility check) filling the soundness cell; the
  value is differentially cross-checked vs the independent `Poly::gcd`. Out of Kani scope
  (vv-guide §5). Lean at the spike.
