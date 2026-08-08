# V&V matrix (CI gates on this)

Row per certificate/kernel operation; cell per method. Status: ✅ done · 🚧 partial · ⬜ todo · N/A.
Each Item carries its milestone tag `[Mx]`. CI fails the milestone gate
(`cargo xtask lint`'s vv-matrix gate) if a **soundness-critical** row (marked ★) whose
milestone has **landed** (`[M0]`, `[M1]`, `[M2]`, `[M3a]`, `[M3c]`, `[M3d]`, `[M3e]`) has empty {Kani ∨ Lean ∨ runtime-checked-hypothesis}.
Not-yet-landed ★ rows (`[M4]`/…) are out of scope until their milestone ships.
(The gate reads the table columns with `FS="|"`; before slice 3d it split on whitespace, so `$2`
matched a mid-cell word instead of the ★ Item and passed vacuously — fixed with the M3d landing.)
(The `arrange2d`/`certify1d` searcher slices `[M3a]`/`[M3c]` are non-★: Kani/Lean = N/A, soundness
deferred to the M3e checkers. CLIP-σ/strict-Sylvester are `[M2]`/`[M4]` CLIP-DOM checkers, not the 3c
arrangement lattice — the "1D" name is overloaded.)

| Item | crate | unit | property | differential | Kani | Lean | validation |
|---|---|---|---|---|---|---|---|
| lattice cmp/sign ★ [M0] | lattice | ✅ | ✅ (randomized diff) | ✅ (num-rational + `RefBackend` limb backend, R.4 — two independent oracles; the whole `RefBackend` `Backend` trait is Aeneas-lifted + **proven = ℤ/ℚ** over `den`/`iden`/`qden` denotations, R.4b, axiom-clean — public target `refBackend_eq_ZQ`, so the differential is a **proof-backed oracle**; extended by the `ratfuzz` **op-chain fuzzer** — chained arith over size-bucketed operands that exercise dashu's Karatsuba/Toom-3/NTT ladder, deterministic replay per-PR + coverage-guided nightly, `docs/differential-fuzz.md`) | ✅ (panic-free full i128; fast≡slow via exhaustive+diff) | ⬜ (spike GO; gcd/reduce Lean gated Phase 5) | — |
| Sturm isolate ★ [M0] | lattice | ✅ | ✅ (proptest) | ✅ (constructed roots) | ✅ (sign-count) | 🚧 `SturmChecker.lean` (checker formalized; Sturm = 1 cited axiom) · `sign_variations` proven axiom-clean, incl. the **Aeneas-lifted model** end-to-end (`Refine.lean`) · rc-hyp ✅ | — |
| resultant ★ [M0] | lattice | ✅ | ✅ | ✅ (vs Poly::gcd) | — | ✅ `verify_common_factor_sound` (`Resultant.lean`): witness ⟹ `¬IsCoprime f g` ⟹ `resultant f g = 0`, **axiom-clean, NO cited axiom** (Mathlib `resultant_eq_zero_iff` closed the gap) · rc-hyp ✅ | — |
| decomposition [M3a] | arrange2d | ✅ | ✅ | ✅ (CGAL + resultant) | N/A | N/A | — |
| carrier + predicates [M3a] | arrange2d | ✅ | ✅ | ✅ (CGAL + resultant) | N/A | N/A | — |
| membership [M3a] | arrange2d | ✅ | ✅ | ✅ (CGAL) | N/A | N/A | — |
| event spine + classify [M3a] | arrange2d | ✅ | ✅ | ✅ (CGAL) | N/A | N/A | — |
| 1D coincidence lattice [M3c] | arrange2d | ✅ | ✅ | ✅ (CGAL overlap-edge) | N/A | N/A | — |
| CLIP-σ signed ★ [M2] | certify1d | ✅ | ✅ (`cx-sigma-mu-crossing` → Unresolved) | — | ✅ (`clip_sigma_signed_disjunction_sound`: corner-range signed disjunction sound over i128; rejects the σμ class) | ✅ `ClipSigma.lean` — **Aeneas-lifted over ℚ** (algebra-rehaul R.3c): `clip_sigma_branch_eq` + `corner_range_eq` prove the extracted `clip_sigma`'s two cores (decision + affine range) EQUAL their spec, so the predicate is derived from the running Rust, not mirrored; soundness + σμ-rejection axiom-clean, NO cited axiom | — |
| certify1d checkers [M2] | certify-core | ✅ | ✅ (corpus + cone fields) | — | N/A | N/A | — |
| strict Sylvester ★ [M4] | certify1d | ⬜ | ⬜ | — | ⬜ | ⬜ | — |
| CAP-IN-D24 input license ★ [M4] | closure → certify-core | ✅ (`cap_in_d24` census + `on_carrier` exact identity: carrier/interval/endpoint/cycle/flank-correspondence — 10 tests; searcher `ruling_edge`/`sigma_edge` project cylinder ruling→line ✅, cone σ-cut→conic refused `OffCarrier` — 4 tests) | ⬜ | — | ✅ (`cap_in_cycle_census_sound`: cycle-closure ANDed over **every** cyclic hand-off + both-flank census, bounded N=4 — rejects a broken internal link whose wrap coincidentally closes, and a cap missing a flank) | ⬜ | — |
| REG-V / WEDGE / EXT-WEDGE bundle ★ [M4] | closure → certify-core | ✅ (`certify_core::wedge::{wedge,reg_v,ext_wedge,regularity}` — division-free ring clearings on `d=n_A·n_B`, constant-V straight-crease scope; refutes zero-dihedral / over-π / ext-wedge / malformed — 6 tests; searcher `wedge_cert` extracts crease normals, 90° cylinder fold certifies, flat joint refused — 3 tests. SIDE/COLLAR support content → C3) | ⬜ | — | ✅ (`wedge_clearing_sound`: each division-free clearing accepts **iff** its true sign-aware predicate holds, AND the `1+d>0` WEDGE guard is necessary — dropping it admits false certificates on the over-π branch; factored to i128 rationals) | ⬜ | — |
| trim/clip searcher (CLIP-DOM + TRIM-LOCAL) [M4] | closure → certify-core | ✅ (`closure::trim` **producer** for reused `certify1d::{trim_local,clip,clip_dom}`: builds `b_J`, `G_i=(C_i−x₀)·b_i` as three σ-rational coeffs `g0/g_mu/g_w`; `field_a/field_b`, `trim_local_cert`, `clip_w_cert`/`clip_mu_cert` cleared gauges, `sigma_deriv_corners` for CLIP-σ. 90° cylinder fold certifies TRIM-LOCAL+CLIP-W, wrong-side fiber refuted = SIDE, connected `clip_dom` census — 7 tests + doctest. SIDE=TRIM-LOCAL; COLLAR/TUBE vacuous κ_max=0. No new checker) | ⬜ | — | N/A (searcher; checkers CLIP-σ ★ [M2] proven) | N/A | — |
| LEDGE branch (CAP-IN-D24→LEDGE-DOM→CAP-OUT) [M4] | closure → arrange2d | ✅ (`closure::ledge`: bridge `ValidatedD24` `CanonicalEdge` Line-carriers → `arrange2d::Edge::Seg`, drive `ledge_dom_certified`; `ledge_cap_certified` single-operand cap = 1 face on the projected cylinder-flank quad, genuine two-operand π₀ merge across the crease, arc-carrier declined — 3 tests + doctest) | ⬜ | ✅ (CGAL `General_polygon_set_2` polygon-boolean on the SAME segment operands: face count **+ exact `a+b√d` boundary geometry**, convex + simple-concave quads — `ledge_cap_region_matches_cgal_polygon_boolean`) | N/A (wiring; soundness = the reused ★ cocycle [M3d] + CAP-OUT-LINK [M3e] checkers) | N/A | — |
| MITER branch (MITER-FIT ε_φ / EDGE-LEDGER / MITER-OUT) ★ [M4] | closure → certify-core | ✅ (`certify_core::miter`: `miter_fit` pairs the two flanks' projected cut edges via the monotone `φ_J`, minting `ε_φ` = **order sign** from one exact oriented-endpoint `cmp` (never the derivative sign, which the `σ_A³` stall zeroes); `clean_miter_cap` drives EDGE-LEDGER (PAIR-IDENTICAL + EDGE-OCCUPANCY) + MITER-OUT/EDGE-REG on the projected cylinder diamond; reversed pairing routed to ledge — tests + doctest) | ⬜ | — | ✅ (`eps_phi_is_endpoint_order`: verdict = exact endpoint order, total on distinct images — the anti-derivative-mint guarantee vs the interior stationary point) | ⬜ | — |
| CLOSURE-CAP disjunction + CLOSURE_VALID(j) [M4] | closure | ✅ (`closure::valid`: `closure_cap` = MITER ∨ LEDGE (miter first, ledge fallback, both faults on `NoBranch`); `closure_valid` ANDs regularity ∧ CLIP-DOM(A,B) ∧ TRIM-LOCAL(A,B) ∧ CLOSURE-CAP ∧ **SEW** — the full conjunction, no longer "minus SEW"; SEW inputs (`SewInput`: edge records + counts + per-vertex links) are searcher-supplied and audited by `certify_core::sew`; short-circuits to the first fault; the 90° cylinder fold certifies through **both** cap branches with SEW, a flat joint refused at REG-V, a pinch occupancy refused `SewEdges(Pinch)`, an `a→c→b→d` crossing refused `SewLink(LinkMismatch)` — 7 tests + doctest; generality corpus folds cylinder + 2 cone angles) | ⬜ | — | N/A (orchestration; soundness = the reused ★ branch checkers CAP-IN-D24 / regularity / MITER / CLIP-σ / SEW) | N/A | — |
| occupancy→row ★ [M4] | sew → certify-core | ✅ (`certify_core::sew::occupancy_row` feeds the four occupancy bits in cyclic quadrant order `[A_L, B_L, A_R, B_R]` to the reused, already-proven `classify_link` → `LinkClass{Exterior\|Interior\|Boundary\|Pinch}`; opposite-quadrant same-flank ⇒ Pinch (reject); `identity_mode` dispatches the identity obligation by boundary count (2⇒PAIR-IDENTICAL, 1⇒OUTPUT-SOURCE-IDENTICAL, 0⇒Provenance) — doctests on both) | ⬜ | — | ✅ (`occupancy_row_sound`: exhaustive over the 4+frame bits, `occupancy_row` = independent boundary-count reference for **all sixteen** patterns + frame-invariance under L↔R flip; the ★ rejects the grouped-mask order that would mis-classify the clean miter as a pinch) | ⬜ | — |
| SEW-EDGES (seam records + identity dispatch + counts) ★ [M4] | sew → certify-core | ✅ (`certify_core::sew::sew_edges`: over the seam's edge records, each record's occupancy through `occupancy_row` (pinch ⇒ `Pinch`, non-Boundary ⇒ `NonBoundaryRecord`), the declared `IdentityMode` cross-checked against the boundary-count dispatch (`ModeMismatch`) + its discharge (`IdentityFailed` / `ProvenanceMismatch`), then typed exact counts both directions with the reverse equality `{records} = {cap-to-flank} ⊔ {flank-to-flank}` (`CountMismatch`); empty/internal joint ⇒ zero records ∧ zero counts. Searcher `sew::records_from_miter_ledger` reads `miter::LedgerEdge.occupancy`; ARRANGEMENT-BITS recompute for the ledge branch — tests + corpus `clean_miter_seam_sews`, `opposite_quadrant_occupancy_is_a_pinch`) | ⬜ | — | ✅ (soundness = the `occupancy→row ★` classifier above, `occupancy_row_sound`; `sew_edges` composes it per record + the exact count equality) | ⬜ | — |
| SEW-LINK over V_∂ (Link_emitted≅geom per boundary vertex) ★ [M4] | sew → certify-core | ✅ (`certify_core::sew::sew_link`: per boundary vertex — ray/sector arity, `classify_link(sectors) == Boundary` (V_∂ only; Interior/Exterior/Pinch ⇒ `NotBoundary`), FACE-GERM species arity vs selected-sector count (`SpeciesArity`), licensed species only (Cap/Flank/Fan; **Apex deferred** ⇒ `UnlicensedSpecies`), then `Link_emitted ≅ Link_geometric` via the reused `link_iso_ok` (`LinkMismatch` — catches the count-passing `a→c→b→d` crossing). Searcher `sew::check_vertex_link` routes `arrange2d::boolean::vertex_link`'s (emitted, geometric, sector-mask) triple in — tests + corpus `union_boundary_links_sew`, `crossing_link_is_refused`) | ⬜ | — | ✅ (soundness = the reused `Link_emitted≅geom ★ [M3e]` — `link_iso_matches_cyclic_adjacency`; SEW-LINK adds only the V_∂-gate + species cover over it) | ⬜ | — |
| quotient emission ★ [M3d] | arrange2d → certify-core | ✅ | ✅ (Euler + rigid/rescale invariance) | 🚧 (CGAL ∪/∩ non-pinch; △-pinch + face-ID follow-up) | ✅ (`cocycle_implies_telescoping`, bounded DCEL ≤4 cells) | — (Kani sufficed) | — |
| CAP-OUT-LINK ★ [M3e] | arrange2d → certify-core | ✅ | ✅ (pinch rigid-invariance; degree-6 vertex) | ✅ (CGAL faces+holes **and exact a+b√d boundary vertices**, rational + irrational radii; △-pinch documented) | ✅ (`link_ok_iff_no_pinch`, bounded ≤6 sectors) | ✅ (`link_ok_spec`: `link_ok ↔ ≤1 run`, axiom-clean over the Aeneas lift, matching Kani; run-counter refinement `cyclic_true_runs_spec` done) — 2-manifold thm = research frontier | — |
| Link_emitted≅geom ★ [M3e] | arrange2d → certify-core | ✅ | ✅ (`links_consistent` on corpus) | — | ✅ (`link_iso_matches_cyclic_adjacency`, permutations N=4) | 🚧 (Aeneas-lifted, axiom-clean; iso-spec refinement = frontier) | — |
| completeness bijections [M3e] | arrange2d | ✅ (`certify_core::arrange::boundary_bijection_ok` — the **source-ID permutation** `{separating edges} ↔ {emitted boundary edges}`: each emitted edge carries its `SubEdge` id, checked a permutation of the separating set, so a drop-one/duplicate-another pair a count misses is caught; `ledge_dom_certified` gate. The component↔face / V_∂↔shell-vertex bijections remain deferred, see `docs/engineering-log.md`) | ✅ (segments/mixed/degree-6) | ✅ (CGAL faces+holes **+ exact a+b√d boundary vertices**, non-pinching) | — | — | — |
| device cone chart [M1] | geom → fixtures | ✅ | ✅ (n·n=1, n·n′=0, c·r=0, offsets-in-family; R₁ σ-law) | ✅ (golden n·ẑ ≡ 65/97 ≈ sin 42°; κ-cap = 65/194) | N/A | N/A | ✅ (`certified_cone`: REG-Q \|q\|², REG-Q \|n′\|², SLAB-S0, mesh κ-cap — all Verified) |
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
  deferred to the `certify_core::arrange` checkers (the ★ `[M3d]` cocycle check lands at slice 3d,
  Kani-proven; the ★ `[M3e]` CAP-OUT rows follow) — so these searcher rows are **not**
  soundness-critical and the gate does not demand a proof cell.
- **`quotient emission` `[M3d]`** (slice 3d): the `arrange2d` DCEL + eight-step boolean is an
  untrusted searcher, but its ℤ₂² **cocycle check** is the pure checker `certify_core::arrange::
  cocycle_ok`, and the searcher's labeling flows through it. **Kani** proves the checker sound —
  `cocycle_implies_telescoping` (certify-core `#[cfg(kani)] proof.rs`): if `cocycle_ok` accepts, every
  walk telescopes its flips, so every closed walk returns its bits (spec §6 step 4), over all
  arrangements up to 4 cells / 5 edges (**bounded DCEL bookkeeping**, vv-guide §5:73). The first Kani
  surface outside `lattice`. Lean was the fallback if Kani proved intractable; it did not.
