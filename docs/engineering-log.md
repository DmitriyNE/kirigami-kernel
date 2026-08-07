# Engineering log — todos, tech debt, findings, deferred items

A low-ceremony running log for things we hit *during* other work and don't want to stop
for: **todos**, **tech debt / sketchy things** (including soundness-scope caveats),
**findings** (surprises worth remembering), and **deferred** items (punted to a later
milestone, with a reason). See something mid-task → add a bullet under the right section,
keep going. The point is to not interrupt the main task, and to not lose the thread either.

This is the *general* log. Two siblings own narrower scopes — cross-referenced, never
duplicated: [`docs/proofs/ledger.md`](proofs/ledger.md) (formal proof obligations, Lean/Kani
status per checker) and [`../vv-matrix.md`](../vv-matrix.md) (the V&V coverage matrix,
CI-gated).

**How to add an entry:** a bold one-line title, a sentence or two of substance, then a
trailing italic line `date · status · ref`. Status ∈ `open · deferred(→Mx) · watching ·
resolved`. On resolve, move the bullet to **Resolved** at the bottom, keeping its date and a
one-line outcome. New sections (e.g. a **To do** that grows, or a per-milestone bucket) are
fine — this is a log, not a schema.

## To do

- **CAP-OUT completeness bijections — source-ID permutation DONE; the two *further* bijections remain.**
  *Done (debt-sprint item 7, `56accab`; vv-matrix 🚧→✅):* the scalar coverage count
  `separating_count == region_boundary_count` is replaced by a real **source-ID permutation**
  certificate — `emit_region` stamps each emitted boundary edge with its `SubEdge` arena id, and the
  pure `certify_core::arrange::boundary_bijection_ok` checks the emitted-id multiset is a permutation of
  the separating-edge id set (so a drop-one/duplicate-another pair a scalar count misses is caught).
  *Remaining:* the other two spec bijections — {selected components} ↔ {emitted faces} (component ids)
  and V_∂ ↔ {emitted shell vertices} — plus per-loop closure/orientation. These fold into **item 8**:
  they reuse its per-component gauge + per-pair emission plumbing. *2026-08-06 · deferred(→8) ·
  `vv-matrix.md` completeness-bijections row*

- **CAP-IN-D24 input license — minimal totality guard DONE; the full newtype census remains M4.** *Done
  (debt-sprint item 5, `5ffbe34`):* `validate_d24(&[Edge]) -> Result<(), CapInFault>` runs *before*
  `Dcel::build` in `ledge_dom_certified`, checking per edge `r² > 0` (circles), `a²+b² > 0` (lines),
  each endpoint on its carrier (residual = 0), and canonical `x_lo < x_hi` — so a hand-crafted malformed
  edge (r² ≤ 0 circle, endpoint off carrier, degenerate line, non-canonical piece) now returns
  `CapOutFault::InvalidInput(CapInFault)` instead of panicking. The certified entry is now total over
  arbitrary `&[Edge]`. *Remaining:* the full spec §8.5 input license — `CanonicalEdge`/`ValidatedD24`
  newtypes minted only by a CAP-IN-D24 checker, so validity is carried in the type rather than
  re-checked at the boundary — lands with `closure`/M4, where the census already lives
  (`closure/src/lib.rs`). *2026-08-06 · deferred(→M4) · spec §8.5 CAP-IN-D24*

- **CAP-OUT strict-manifold entry (`ShellReady`) — decide when SEW lands.** `ledge_dom_certified` is
  deliberately *relaxed*: a pinch (non-manifold vertex, e.g. a transverse `△`) is a valid, reported
  result (`CapOut.pinches`), not a refusal — the manifold requirement is owned by the downstream SEW-LINK
  gate, and there is no pre-SEW consumer today (confirmed in review batch 1). When SEW (M4/M5) is built,
  reconsider a typed strict entry `ledge_dom_manifold → ShellReady<B>` that additionally gates
  `pinches.is_empty()` and returns a type only a no-pinch region inhabits — so "forgot to check
  manifoldness" is a compile error and the proven `link_ok` is used in production. Deferred (not now)
  because the newtype's contract is SEW's to specify; building it blind risks guessing wrong.
  *2026-08-06 · deferred(→M4) · `certify_core::arrange::link_ok`*

- **Front-half geometry is trusted — add per-pair D24 intersection certificates.** The
  arrangement checkers (`certify_core::arrange`) read only the *combinatorial* certificate — indices,
  labels, cyclic orders — never coordinates, so the geometric front-half (`carrier`/`decompose`/
  `membership`/`classify`/`spine`) is *trusted* (differentially validated vs CGAL + property tests, not
  checker-certified; vv-guide §6 "trusted front-half"). A carrier-solver bug — a dropped intersection, a
  wrong point, a misclassified coincidence — can yield a self-consistent DCEL that passes every checker.
  Honest stamp today: "combinatorially self-consistent, geometry differentially validated." Fix: emit
  per-pair certificates (discriminant sign, exhaustive candidate count, carrier residuals, interval-
  membership decisions) for line/line, line/circle, circle/circle — cheap exact D24 algebra — and check
  them. README/AGENT reconciled to state this scope (batch 2). **This is debt-sprint item 8** (with the
  multi-component gauge anchor #9b and the folded `CoincEdge`/`CoincSet` deletion) — a genuine
  *geometric-checker* slice, not a quick fix: it needs independence from the solver (re-derive the
  discriminant/residuals over the output, don't re-solve), binding the per-pair evidence to the input edges
  (the #6-`CertifiedChart` transplant risk), and a re-verifiable per-component point-location for the
  gauge. Given its own **focused design pass** rather than rushed at the tail of the sprint; a natural fold
  into Milestone C, where `closure` builds the CAP-IN-D24 census + per-pair geometry and SEW-LINK needs the
  gauge-anchored labels anyway. *2026-08-06 · deferred(→ own pass / C) · spine.rs, carrier.rs, witness.rs*

- **`CertifiedChart` digest-binding — the *remaining* (persistence-only) half.** The in-memory
  claim/evidence binding is **done** (batch 2b): `CertifiedChart::certify` now re-derives the checked
  quantities (`|q|²`, `|n′|²`, det J at the `(μ,w)` box corners) from `chart + domain` via
  `regularity_targets`, recomputes the tag, verifies the evidence (Sturm chains + margins) against those
  derived targets, and stores the domain — so a certificate built for one chart cannot be attached to
  another (the chains fail to verify), and a margin is qualified by its domain. `CapOut` never had the
  transplant problem (it wraps the region `ledge_dom_certified` just checked, not independent args). What
  *remains* deferred is only cross-boundary integrity: binding a verdict to a canonical **digest** of its
  claim so it can't be transplanted across a serialize/deserialize boundary, and retaining the certificate
  for offline re-checking — meaningful only once a persistence path exists (there is none today; building
  it now = inventing a serialization format speculatively, the `ShellReady` YAGNI). `kappa_cap` also rides
  on `CertifiedChart` as searcher-derived, uncertified data (documented). *2026-08-06 · deferred ·
  `geom::record`*

- **CLIP ladder coverage — common-zero census DONE; μ-coverage + fiber-census remain.** *Census done
  (debt-sprint item 6, `60e890e`):* `ZeroCensus`/`census_ok` — `clip()` now certifies the per-zero path
  only if the supplied zeros are the complete isolated-root set of `b²+d²` (independently re-counted;
  disjoint, σ-ordered, one-per-interval), closing the omit-an-awkward-zero hole. *Remaining (deferred with
  rationale):* **(a)** μ-subspan coverage of the CLIP-W failing set `{R_W ≤ 0}` — sound-ly relating the
  failing region (whose boundaries are *irrational* R_W roots) to the searcher's *rational* μ-spans, with
  open/closed boundary handling under half-open Sturm counts, is genuinely hard, and there is **no CLIP
  searcher** yet to validate against (all CLIP certs are hand-built fixtures; the producer is M4/closure).
  Shipping an unvalidated coverage checker in the sprint meant to *fix* coverage was judged too risky.
  **(b)** the `trim_local`/`clip_dom` sign-event fiber census (needs the chart-domain sign-event
  polynomial). Both best done alongside C's searcher. *2026-08-06 · deferred(→C searcher) · spec §8.5 CLIP*

- **Multi-component cocycle gauge — the release-silent defaults are DONE; the gauge anchor remains (→8b).**
  *Done (debt-sprint item 2, `116ef78`):* `slab_locate` no longer silently defaults on the certified
  path — an incomplete slab decomposition or an unassigned cycle is now an explicit
  `CapOutFault::Incomplete` (the release-gone `debug_assert!` genericity check and the
  `unwrap_or((false,false))`/`(0,0)` defaults no longer sit on the certified route). *Remaining (the
  deeper half):* `cocycle_ok` pins the ℤ₂² gauge only in the seed's connected dual-component — for a
  disconnected dual graph (disjoint operands; holes, where one region is bounded by several edge-disjoint
  cycles) every other component can be uniformly XOR-shifted and still satisfy all edge equations, so its
  absolute labels come from *point-location* (trusted), uncertified — a point-location bug on a disjoint
  component would pass certification. Fix: per-component anchoring the checker re-verifies — **item 8b**,
  tied to the per-pair certificate work. *2026-08-06 · deferred(→8b) · boolean.rs, arrange.rs*

- **`link_iso` — permutation guard DONE; the unbounded (N>4) proof remains a frontier.** *Done
  (debt-sprint item 3, `4b94a53`):* `link_iso_ok` now validates its own precondition — a `has_duplicate`
  in-range/no-duplicates guard rejects the non-permutation inputs the Kani harness had only *assumed*;
  the Aeneas-lifted Lean model was regenerated and re-audited axiom-clean. *Remaining:*
  `link_iso_matches_cyclic_adjacency` still proves only length-4 permutations (vv-matrix labels the cell
  "N=4"); degree-6 vertices are property-tested, but the unbounded statement wants a Lean induction (the
  `link_ok`/pinch harness, by contrast, is already N=6). Research frontier. *2026-08-06 ·
  deferred(→frontier) · proof.rs, arrange.rs*

- **Coincidence lattice — `CoincSet` edge-list is dead; deletion folds into 8a.** Verified (debt sprint,
  item 4): `coincide` is *load-bearing* — its `touches` become `Coincident` incidences (`spine.rs:77`) that
  seed the overlap-boundary vertices `Dcel::build`'s step-3 merge depends on, so the **live merge is correct
  for *partial* overlap** (proven by the new `boolean_over_partially_overlapping_edges` fixture — two
  horizontally-offset rectangles, ∪/∩/△). Only the `CoincEdge`/`CoincSet` **edge-list** is dead (dropped as
  `_coinc`; `CoincOutcome` is `usize` counts, decoupled from `CoincEdge`). Its physical removal (`event.rs`
  `CoincEdge`/`Operand`, `spine.rs` `CoincSet` + `arrange_events` return, the randomized
  `coincident_edges_match_cgal` differential) **folds into item 8a's `PairWitness` rework** — one coherent
  witness change rather than double-churn. *2026-08-06 · deferred(→8a) · spine.rs, coincide.rs, difftest*

- **Differential-fuzz — harness + real fuzz run DONE (`differential-fuzz` branch); one wiring follow-up.**
  Op-chain differential (`crates/lattice/src/ratfuzz.rs`: `dashu` ≡ the *proven* `RefBackend` over
  size-bucketed operands + metamorphic mul identities) closes the two gaps the old single-op
  `rat::differential` had (no op-chains; i128-only ≤2-limb seeds ⇒ dashu never left schoolbook). **Done:**
  **(1)** seed buckets pinned to dashu-int 0.4.3's real mul thresholds — schoolbook ≤24 / Karatsuba 25–96 /
  Toom-3 97–4000 / NTT >4000 limbs (dispatch keys on the *smaller* operand, `mul/mod.rs`), straddled ±1.
  **(2)** seed corpus via `fuzz`'s `gen_corpus` bin (authoritative encoder `ratfuzz::corpus_seeds()`; 7 seeds
  across the thresholds) + a **real `cargo fuzz run`** — 2652–3118 coverage-guided runs, clean. **Key
  mechanism:** the fuzz build enables dashu's `tuning` feature (`fuzzing = ["dashu/tuning"]`) and the target
  lowers the thresholds via env vars (SIMPLE=2/KARATSUBA=16/NTT=160) so tiny operands route through
  Karatsuba/Toom-3/**NTT** at oracle-cheap sizes (no need for 4000-limb operands). **Finding (first run
  earned its keep):** thresholds MUST respect each algorithm's own `MIN_LEN` (Karatsuba 3, Toom-3 16) — my
  first values (KARATSUBA=6) routed 7–15-limb operands into Toom-3 and tripped *dashu's own*
  `assert!(b.len() >= MIN_LEN)`; not a dashu bug, a mis-config. **CI split (DONE):** per-PR = the
  *deterministic replay* (stable, no libFuzzer) — `replay_seed_corpus` unit test (in `nextest`) + the
  `fuzz regression replay` step (`cargo test -p lattice --features fuzzing --test fuzz_replay`, replays the
  committed crash corpus under the fuzzer's tuning); nightly = the *coverage-guided search*
  (`.github/workflows/fuzz-nightly.yml`, cron + `workflow_dispatch`, cargo-fuzz on the runner's rustup with
  a cached/persisted corpus, uploads crash artifacts). **STATUS:** the nightly cron is unvalidated on a
  real runner (nightly + `rust-src` + cargo-fuzz provisioning, same rustup-outside-nix pattern as dylint) —
  **watch the first scheduled run**; and provision the nightly fenix-natively if we want it inside nix.
  (A rational op-chain variant stays deferred — RefBackend's bit-serial `divrem`/`gcd` are too slow as a
  big-operand oracle; use metamorphic there.) *2026-08-06 · watching*

- **`RefBackend::int_from_le_bytes` must be proven if it ever leaves the test/fuzz harness.** It's a
  TEST/FUZZ-ONLY seed constructor (`#[cfg(any(test, feature = "fuzzing"))]`, banner on the fn), NOT a
  `Backend` trait method and NOT proven — its correctness is runtime-checked in the harness (seed
  byte-compared against dashu), never relied on for soundness. If it ever enters the `Backend` trait or
  any Aeneas-lifted / production path, it MUST first be proven `den(result) = value` in
  `certify-check/CertifyCheck/RefBackend.lean`, exactly like `from_i128` (`int_from_i128_eq`). The cfg
  gate keeps it physically out of the trait + the lift until then. *2026-08-06 · watching · `refbackend.rs`*

- **R.5 — finalize the algebra-trust rehaul (the `RefBackend = ℤ/ℚ` surface is DONE).** The whole reference
  `Backend` trait is now proven axiom-clean on `algebra-rehaul-r4` (`certify-check/CertifyCheck/RefBackend.lean`):
  RefNat = ℕ, RefInt = ℤ (ordered ring + gcd/lcm/divrem + i128 both directions), RefRat = ℚ (reduce + all
  arithmetic mul/div/add/sub + neg/numer/denom/is_zero/sign/cmp/from_ints/from_i128). Remaining is the V&V
  finalization: **(1)** promote the audit surface to a public `Backend`-instance corollary (the current
  `#print axioms` block lists the *private* op refinements; add a public theorem so `ci.yml`'s axiom-audit
  guards `RefBackend = ℤ/ℚ` at the trait level) + wire it into `.github/workflows/ci.yml`; **(2)** the dashu
  differential — make it a *proof-backed* oracle now that the reference is proven `= ℤ/ℚ` (`rat::differential`);
  **(3)** `vv-matrix.md` rows + `docs/algebra-trust.md` TCB update (dashu trust shrunk to the differential) +
  extraction-drift for the generated files; **(4)** merge-to-main review of `algebra-rehaul-r4`. Findings +
  the full method-by-method recipe in memory `algebra-rehaul.md`. *2026-08-05 · open*

- **Restore the `Backend` associated-type `Clone + Eq` bounds when Charon disambiguates
  trait parent-clauses.** The pinned Charon (`0.1.225`) lifts the `Backend` trait to a Lean
  `structure` whose parent-clause witnesses for *both* associated types (`type Int: Clone + Eq`
  and `type Rat: Clone + Eq`) are named identically (`corecloneCloneInst` / `corecmpEqInst`),
  so the structure has duplicate fields and does not typecheck — which blocks lifting any
  `Rat`-using checker (all `<B: Backend>`). Investigated exhaustively: no charon flag fixes it
  (`--remove-adt-clauses` targets ADTs not trait decls; `--exclude` leaves a dangling `sorry`
  and collapses the assoc-type lifting; `--hide-allocator` / `--opaque` variants keep the
  colliding structure). **Workaround (algebra-rehaul R.3):** dropped the `Clone + Eq` bounds
  from the trait's associated types (`Eq` was unused; `Clone` had exactly 4 call sites in
  `rat.rs`) and expressed clone as explicit `Backend::int_clone` / `rat_clone` methods (one impl,
  `Bignum`). This is contained and semantically inert, but it is a workaround: when Charon names
  those witnesses distinctly (check its releases past `0.1.225`), restore the associated-type
  bounds, delete `int_clone`/`rat_clone` + their impl, revert the 4 `rat.rs` sites to `.clone()`,
  and re-extract. Coordinated with a charon/aeneas pin bump (drags Lean/Mathlib). *2026-08-04 · open*

- **Make the dylint CI step fenix-native.** `cargo dylint`'s toolchain management is
  rustup-centric — it reads `lints/no_float/rust-toolchain` and runs that nightly *via rustup* —
  so the CI step (`.github/workflows/ci.yml`) runs on the **runner's rustup, outside nix** (like
  the Kani `cargo install` step) rather than a fenix-pinned toolchain. Functional and verified
  locally, but not consistent with the rest of the toolchain (fenix / `flake.nix`). Follow-up:
  supply `nightly-2026-05-28` + `rustc-dev`/`llvm-tools` via `fenix.toolchainOf` and run dylint
  inside `nix develop` — needs a rustup shim or dylint toolchain-env plumbing (the fiddly part
  I couldn't verify from the sandbox). Also: watch the first real CI run of the step.
  *2026-08-04 · open*

- **Pin `nixpkgs` off `nixos-unstable`.** `flake.nix` floats `nixos-unstable`, so a toolchain
  bump silently regresses CI with **no code change** — e.g. `gcc-15.3`/`glibc-2.42` emitting a
  `.debug_gdb_scripts` section that `rust-lld` rejected when linking the CGAL oracle shim (worked
  around with `.debug(false)` in `crates/difftest/build.rs`, commit `5276957`). Pin to a
  known-good rev (or a stable release) so CI is reproducible; the `.debug(false)` band-aid can
  then be reconsidered. Revisit floating-vs-pinned deliberately.
  *2026-08-04 · open*

## Tech debt / sketchy

- **The mesh κ-cap corner-min is representation-conditional.** It is sound only while `R₁`
  is affine/monotone in the chart parameters. Before promoting it to a *general* κ-cap
  checker (milestone C — the petal flank, where `R₁` is neither), Sturm-isolate the
  curvature extrema over the domain, or carry a certified convexity/monotonicity rider. This
  is the same class as the CLIP-σ / `corner_range` convexity rider. The cone golden is
  hand-verified, not unsound.
  *2026-08-04 · open · `fixtures::devices::certified_cone`, `certify1d::corner_range`*

- **The invariant lints scan code/doc-comments only, not spec text.** `tuple-predicate` and
  `:= census` (now in `cargo xtask lint`) should also scan `spec/`, but that needs an
  allow-list for the frozen spec's own meta-discussion of the rules (it explains *why*
  "proportional" is banned, using the word), and the spec is frozen — so we can't sprinkle
  inline allow markers. Deferred until that allow-list is designed.
  *2026-08-04 · open · `xtask/src/main.rs`*

- **`certify_core.lattice.backend.Backend` qualification in the certify-core externals.** The
  `refbackend` lift (algebra-rehaul R.4b) adds a *concrete* `lattice.backend.Backend` (the trait,
  pulled in by `impl Backend for RefBackend`) to the **Lattice** model. The certify-core model
  independently carries an *opaque* `Backend` (bound to ℚ), which — because Aeneas wraps a crate's
  model in `namespace <crate>` — is named `certify_core.lattice.backend.Backend`. The two coexist
  fine (different namespaces), but the hand-written `open certify_core` files (`CertifyCore/
  FunsExternal.lean`, `CertifyCheck/ClipSigma.lean`) referenced `Backend` *bare*, and bare now
  resolves to the Lattice model's concrete one (exact global match beats an `open`) instead of the
  intended opaque one. Worked around by fully-qualifying those references to
  `certify_core.lattice.backend.Backend`. This is explicit-and-correct but couples the hand-written
  externals to Aeneas's namespace-wrapping convention (stable at the pins; drift-checked). Cleaner
  long-term options if it ever bites: (a) extract `refbackend` into its own Lean lib so `Backend`
  never enters the shared `Lattice` model, or (b) move `impl Backend for RefBackend` to a sibling
  Rust module excluded from the `crate::refbackend` start-from (needs `pub(crate)` on the `RefInt`/
  `RefRat`/`RefNat` internals it touches).
  *2026-08-04 · open · `certify-check/CertifyCore/FunsExternal.lean`, `CertifyCheck/ClipSigma.lean`*

## Findings

- **Developable ≠ constant curvature.** A cone's nonzero principal radius is `R₁ = ρ·tan β`
  (ρ = slant distance from the apex) — *not* constant; only the cylinder has constant `κ₁`.
  This is why the mesh κ-cap is the domain minimum (the tightest radius, nearest the apex),
  not a value read off a fixed parameter station. A one-line property test caught the wrong
  "cone ⇒ symmetric ⇒ constant radius" assumption.
  *2026-08-04 · watching · `fixtures::devices::cone_principal_radius_shrinks_along_sigma`*

## Deferred (by milestone)

- **Petal conical-flank fixture + the `cx-cone-flank-trim-mu` corpus entry.** Spec §13
  geometry is not yet pinned; needed for closure/sew.
  *2026-08-04 · deferred(→M-C) · `fixtures/corpus.md`*

- **Algebra-trust rehaul.** Opaque `Int=ℤ` / `Rat=ℚ`, a reference bignum, its Lean
  equivalence proof, and a dashu differential stress-test.
  *2026-08-04 · deferred(post-B) · `docs/algebra-trust.md`*

- **SLAB-S1 / QPOS Bernstein positivity.** No Bernstein primitive yet.
  *2026-08-04 · deferred(→M4) · vv-guide §8 (B deferrals)*

- **The full EDGE-REG verdict logic + EDGE-EMB / EDGE-EDGE (embeddedness).** The Milestone-B
  version is only the `Pass | Fail | Stall` core plus `to_verdict`.
  *2026-08-04 · deferred(→M5/sew) · `certify1d::edge_reg`*

- **M0 `lattice` perf/robustness follow-ups.** Subresultant PRS (vs the naive resultant), a
  bivariate resultant, `AlgReal` refinement, proportional-lint softening.
  *2026-08-04 · deferred · M0 task-2 deferrals*

## Frontier (research-open)

- **Two open theorems, both tracked in the proof ledger.** Sturm's theorem is cited as an
  axiom (`sturm_root_count`, absent from Mathlib); CAP-OUT ⇒ 2-manifold-with-boundary is
  open. Runtime-checked hypotheses / bounded Kani cover soundness in the interim.
  *2026-08-04 · watching · `docs/proofs/ledger.md`*

## Resolved

- **Debt sprint — 7 of 8 review-batch items paid down (branch `debt-sprint`).** *Done 2026-08-06:*
  **(1)** structured `RegFault` splitting bad-paperwork from real degeneracy through the REG-Q family +
  `ChartFault` (`5834d0e`). **(2)** `slab_locate` no longer silently defaults an unassigned cycle /
  non-generic decomposition on the certified path — `CapOutFault::Incomplete` (`116ef78`). **(3)**
  `link_iso_ok` permutation guard (`has_duplicate`), with the Aeneas-lifted Lean model regenerated +
  `lake build` + axiom audit re-run clean (`4b94a53`); finding: Aeneas rejects `return` inside nested
  loops. **(4)** verified the *live* coincidence merge handles **partial** overlap (`coincide`'s touches
  seed the vertices; new fixture) — the dead `CoincEdge`/`CoincSet` deletion folds into item 8's witness
  rework (`abbdf7a`). **(5)** CAP-IN-D24 minimal totality guard `validate_d24` — `ledge_dom_certified` is
  now total over malformed input (`5ffbe34`). **(6, part 1)** CLIP-ladder **common-zero census**
  (`ZeroCensus`/`census_ok`) closing the omitted-zero hole (`60e890e`). **(7)** CAP-OUT **source-ID
  permutation bijection** replacing the coverage count (`boundary_bijection_ok`; `56accab`; vv-matrix
  🚧→✅). Each a full-workspace-green commit (nextest/doctests/fmt/clippy -D/xtask; #3 also lake+audit).
  **Deferred (judgment, with rationale):** item 6's **μ-coverage + fiber-census** — relating the CLIP-W
  failing region (irrational R_W roots) to rational μ-spans is hard to do *soundly* and there is no CLIP
  searcher to validate against; risky to ship an unvalidated coverage checker in the sprint meant to fix
  coverage. Best done with C's searcher (see the CLIP To-do entry). Item 8 (per-pair certs + gauge) is a
  genuine geometric-checker slice — given its own focused pass (see To-do). To-do reconciled
  (2026-08-07): the RegFault entry was removed (fully done); the CAP-OUT-bijection, CAP-IN
  totality-guard, slab_locate, and link_iso entries were rewritten to record what shipped vs. the
  genuine remainder (the two further bijections → 8, the gauge anchor → 8b, the unbounded link_iso
  proof → frontier).

- **`divrem` op refinement (algebra-rehaul R.4b.4).** *Done 2026-08-05:* `divrem_loop_spec` (the
  bit-serial restoring-division loop = Euclidean identity, `704196e`) + `divrem_eq` (the wrapper =
  `den self / den d`, `den self % den d`, `d26ba39`), both axiom-clean + CI-audited, full `lake
  build` green. New reusable lemmas: `nat_or_pow2_add`/`u64_or_pow2_add` (set-a-clear-bit = add),
  `den_head`. Strengthened `divrem_loop_spec`'s post with `Normalized result.2` (needed by the
  wrapper). Next: R.4b.5 `gcd`, R.4b.6 `RefInt`/`RefRat`→ℤ/ℚ.

- **Launder the bench build-artifact blobs out of git history.** *Done 2026-08-04:* rewrote
  `main` + `milestone-b` with `git filter-branch --index-filter` (filter-repo unavailable),
  stripping all 853 `benchmarks/two-tier-vs-dashu/target/` blobs — branch histories clean, the
  milestone merge + tree intact, `cargo build` green. Objects survive only via the
  `refs/original` backup + a bundle (won't be pushed; gc when convenient). The rewritten `main`
  still descends from `gho/main`, so the push was a clean fast-forward.

- **Push the laundered `main` to `gho`.** *Done 2026-08-04:* the remote fast-forwarded
  `c2a73c4 → 98005ea` (Milestone B + engineering-log + tidy Phases A/B), verified blob-free.

- **dylint `no_float` lint (float literals in certified paths).** *Done 2026-08-04:* a
  `rustc_private` `LateLintPass` (`lints/no_float`, pinned `nightly-2026-05-28`) flags float
  *literals* in `lattice`/`certify_core`/`arrange2d` — the type-aware complement to the
  `cargo xtask lint` token scan. Verified locally: compiles clean, the UI test fires on
  `1.5`/`2.0`/`3.0`, and the real certified crates are float-literal-free (it correctly ignores
  the `.1.0` tuple access at `boolean.rs:550`, where a text scan would false-flag `1.0`). CI
  wiring is written but pending a real-runner check (see To do).
