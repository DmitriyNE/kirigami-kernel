# V&V matrix (stub — CI gates on this)

Row per certificate/kernel operation; cell per method. Status: ✅ done · 🚧 partial · ⬜ todo · N/A.
CI fails a milestone gate if a **soundness-critical** row (marked ★) has empty {Kani ∨ Lean ∨ runtime-checked-hypothesis}.

| Item | crate | unit | property | differential | Kani | Lean | validation |
|---|---|---|---|---|---|---|---|
| lattice cmp/sign ★ | lattice | ✅ | ✅ (randomized diff) | ✅ (num-rational) | ✅ (panic-free full i128; fast≡slow via exhaustive+diff) | ⬜ (spike) | — |
| Sturm isolate ★ | lattice | ⬜ | ⬜ | ⬜ | ⬜ (sign-count) | ⬜ (hyp-checked) | — |
| resultant ★ | lattice | ⬜ | ⬜ | ⬜ | — | ⬜ (hyp-checked) | — |
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

**Notes.**
- `lattice cmp/sign` (M0 task-2 slice 1): unit = corpus seeds + `Bignum` (>i128) + the
  exhaustive ±24 grid sweep; property = the boundary-weighted randomized differential;
  differential = num-rational (independent 2nd backend). **Kani** = `neg`/`sign`/`cmp`
  panic-/overflow-freedom proven over the full i128 domain (incl `i128::MIN`); fast≡slow
  *correctness* is carried by the exhaustive grid + differential (the 128-bit gcd loop is
  CBMC-expensive, so its symbolic `*_correct_i16` harnesses are authored but hardened —
  loop-bounded gcd — at the `§7` spike). Lean deferred to the spike. `Sturm`/`resultant`
  rows land with slice 2 (ℚ[x] → Sturm → resultants → interval-plus-separation).
