# V&V matrix (stub — CI gates on this)

Row per certificate/kernel operation; cell per method. Status: ✅ done · 🚧 partial · ⬜ todo · N/A.
CI fails a milestone gate if a **soundness-critical** row (marked ★) has empty {Kani ∨ Lean ∨ runtime-checked-hypothesis}.

| Item | crate | unit | property | differential | Kani | Lean | validation |
|---|---|---|---|---|---|---|---|
| lattice cmp/sign ★ | lattice | ⬜ | ⬜ | ⬜ (2nd backend) | ⬜ (fast≡slow) | ⬜ | — |
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
