# V&V matrix (stub — CI gates on this)

Row per certificate/kernel operation; cell per method. Status: ✅ done · 🚧 partial · ⬜ todo · N/A.
Each Item carries its milestone tag `[Mx]`. CI fails the milestone gate
(`scripts/lint/vv_matrix_gate.sh`) if a **soundness-critical** row (marked ★) whose
milestone has **landed** (`[M0]`, `[M3a]`, `[M3c]`) has empty {Kani ∨ Lean ∨ runtime-checked-hypothesis}.
Not-yet-landed ★ rows (`[M2]`/`[M3d]`/`[M3e]`/`[M4]`/…) are out of scope until their milestone ships.
(The `arrange2d`/`certify1d` searcher slices `[M3a]`/`[M3c]` are non-★: Kani/Lean = N/A, soundness
deferred to the M3e checkers. CLIP-σ/strict-Sylvester are `[M2]`/`[M4]` CLIP-DOM checkers, not the 3c
arrangement lattice — the "1D" name is overloaded.)

| Item | crate | unit | property | differential | Kani | Lean | validation |
|---|---|---|---|---|---|---|---|
| lattice cmp/sign ★ [M0] | lattice | ✅ | ✅ (randomized diff) | ✅ (num-rational) | ✅ (panic-free full i128; fast≡slow via exhaustive+diff) | ⬜ (spike GO; gcd/reduce Lean gated Phase 5) | — |
| Sturm isolate ★ [M0] | lattice | ✅ | ✅ (proptest) | ✅ (constructed roots) | ✅ (sign-count) | 🚧 `SturmChecker.lean` (checker formalized; Sturm = 1 cited axiom) · `sign_variations` proven axiom-clean, incl. the **Aeneas-lifted model** end-to-end (`Refine.lean`) · rc-hyp ✅ | — |
| resultant ★ [M0] | lattice | ✅ | ✅ | ✅ (vs Poly::gcd) | — | ✅ `verify_common_factor_sound` (`Resultant.lean`): witness ⟹ `¬IsCoprime f g` ⟹ `resultant f g = 0`, **axiom-clean, NO cited axiom** (Mathlib `resultant_eq_zero_iff` closed the gap) · rc-hyp ✅ | — |
| decomposition [M3a] | arrange2d | ✅ | ✅ | ✅ (CGAL + resultant) | N/A | N/A | — |
| carrier + predicates [M3a] | arrange2d | ✅ | ✅ | ✅ (CGAL + resultant) | N/A | N/A | — |
| membership [M3a] | arrange2d | ✅ | ✅ | ✅ (CGAL) | N/A | N/A | — |
| event spine + classify [M3a] | arrange2d | ✅ | ✅ | ✅ (CGAL) | N/A | N/A | — |
| 1D coincidence lattice [M3c] | arrange2d | ✅ | ✅ | ✅ (CGAL overlap-edge) | N/A | N/A | — |
| CLIP-σ signed ★ [M2] | certify1d | ⬜ | ⬜ | — | — | ⬜ | — |
| strict Sylvester ★ [M4] | certify1d | ⬜ | ⬜ | — | ⬜ | ⬜ | — |
| occupancy→row ★ [M4] | sew | ⬜ | ⬜ | — | ⬜ (≤6 bits) | ⬜ | — |
| quotient emission ★ [M3d] | arrange2d | ⬜ | ⬜ (Euler) | ⬜ (CGAL) | — | ⬜ (research) | — |
| CAP-OUT-LINK ★ [M3e] | arrange2d | ⬜ | ⬜ | — | — | ⬜ (research) | — |
| Link_emitted≅geom ★ [M3e] | sew | ⬜ | ⬜ | — | ⬜ (bounded) | ⬜ | — |
| completeness bijections [M3e] | arrange2d | ⬜ | ⬜ | — | ⬜ (bounded) | — | — |
| device cone chart [M1] | geom | ⬜ | — | — | — | — | ⬜ (golden) |
| STEP shell [export] | export | ⬜ | — | ⬜ (OCC) | — | — | ⬜ (kernels + hw) |
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
- **M3a `arrange2d` rows** (front-half searcher: decomposition, carrier/predicates, membership,
  event-spine/classification): unit = the `cx_*` corpus + inline units; property = proptest
  (rigid-motion + lattice-rescaling metamorphic invariants, reassembly, residual-zero);
  differential = the CGAL `Arrangement_2` circular-kernel oracle **up to the quotient** (exact
  `a+b√d`, no tolerance) **plus** the in-crate `resultant_bivariate` count oracle. **Kani / Lean
  = N/A**: a ℚ-arrangement searcher is out of Kani scope (vv-guide §5, :76), and its soundness is
  deferred to the `certify_core::arrange` checkers at M3e (the ★ `[M3d]`/`[M3e]` rows) — so these
  searcher rows are **not** soundness-critical and the gate does not demand a proof cell.
