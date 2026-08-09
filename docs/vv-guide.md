# V&V Guide (unified) — verification & validation for the Kirigami kernel

Authoritative. Supersedes and merges `vv-plan-v1.md` and `vv-addendum-1-lean-extraction.md` (both retained only as history). Companion to `implementation-plan-v1.md`. **Verification** = code ⊨ spec v0.24; **validation** = spec ⊨ physical intent.

---

## 0. Organizing principle — certifying algorithms, verified checkers

Spec v0.24 makes every result a `(claim, certificate)` pair. The formal-methods budget therefore goes to **checkers** — `check(claim, cert) -> Verdict` — not to constructors. Searchers (arrangement, resultant pairing, sewing) may be arbitrarily clever and are only *tested + differentially checked*; soundness rests on checkers that are small, pure, loop-light, and proven. LEDA's certifying-algorithm discipline; the de Bruijn criterion for a CAD kernel.

Binding API rule: **every constructor returns evidence sufficient for an independent checker; a result whose checker cannot run is `Unresolved`, never `Verified`.**

**Runtime-checked hypotheses** — the biggest proof-effort reducer. Where a checker's soundness rests on a deep theorem with decidable hypotheses, check the hypotheses exactly at runtime and cite the theorem (to a Mathlib lemma name where one exists). Sturm: verify the chain identities `p_{i+1} = −(p_{i−1} mod p_i)` by exact division on the given sequence → the variation theorem becomes a citation, and the provable surface shrinks to sign-counting. Same for resultant-vanishing ⇔ common root (verify the Sylvester-matrix identity on the instance), Sylvester's criterion (verify the minors are the stated minors), Descartes bounds. Each such theorem gets a `docs/proofs/ledger.md` entry: statement, citation, hypotheses-checked-at-runtime vs structural.

---

## 1. Architecture the verification forces: pure-core / imperative-shell

Both the checker discipline and the Rust→Lean extraction path (see §4) demand the same partition. It is hard architecture, not preference.

- **`certify-core`** — the checkers and their algebra: polynomial ops, Sturm sign-counting, resultant/Sylvester identity checks, corner min/max evaluators (declared min-or-max per the convexity rider), interval membership on half-angle tags, bijection and ⊔-equality checkers, the occupancy→sewing-row classifier, verdict algebra. **Pure, total, panic-free, recursion-explicit, `no_std`.** This crate is the deductive-verification surface. Prefer persistent structures; where performance forces mutation, keep it Aeneas-shaped (local `&mut`, no aliasing).
- **`kernel-search`** — arrangement, DCEL plumbing, MITER-FIT search, sewing construction: arbitrarily imperative, cache-friendly, **only tested + differentially checked**. Emits `(claim, certificate)`; never trusted; every output flows through `certify-core`.

Soundness argument: `certify-core` proven ∧ every `kernel-search` result checked by it ⇒ the kernel's cleverness is outside the TCB. The certifying-algorithm boundary doubles as the extraction-tractability boundary and the TCB boundary.

---

## 2. Layer T — types (free, already committed by the spec's design)

Three-valued `Verdict { Verified(Evidence) | Refuted(Witness) | Unresolved(Margin) }`. Proof types are enums whose variants are the constructors — an uninhabited demand stratum is a compile error, not a review finding (this mechanically catches the batch-19–21 defect class: EDGE-OCCUPANCY's four bits, the PAIR-IDENTICAL / OUTPUT-SOURCE-IDENTICAL split, V_cand/V_∂). `MarginSq` newtype for the squared-margin convention; lattice newtypes; frame-covariance as phantom types where cheap. Exhaustive matches enforced.

---

## 3. Layer P — randomized property + fuzz testing (`proptest`, `cargo-fuzz`)

- **Stratum-weighted generators**, not uniform: the bugs live on degenerate strata. Deliberately emit coincident carriers, internal/external tangencies, shared endpoints, equal radii, antipodal arcs, pole-wrapped arcs, zero-dihedral joints, cone-vs-planar flank pairs; a stratum-weight knob runs CI degenerate-heavy.
- **Custom shrinkers** toward degeneracy and smaller lattice coordinates (default shrinking yields useless generic-position minima).
- **Invariant properties** (cheap — the spec made them inventory comparisons): cocycle closure, Euler consistency, all completeness bijections, typed-count ⊔-equality, `Link_emitted ≅ Link_geometric`, N_cut/orientation coherence. Run inside fuzz targets as crash conditions.
- **Metamorphic properties**: invariance under REPARAM, frame-bit flip, s_J flip, lattice rescaling, and rational rigid motions (rational-quaternion rotation + lattice translation stays in-lattice — a free test symmetry group).
- **Differential oracles** (in `difftest/`, never in a certified path): CGAL `Arrangement_2` circular kernel for the arrangement (agreement up to the quotient); a second bignum backend for the lattice; OpenCascade's shape checker for shells.

---

## 4. Layer D — Lean 4 deductive verification + Rust→Lean extraction

**Proof assistant: Lean 4 + Mathlib** — the right home for algebraically-loaded certificates (polynomials, resultants, Sturm-adjacent real-root theory, linear algebra for Sylvester).

**Extraction direction is Rust → Lean, always.** Lean→Rust codegen is not production-ready (Lean emits C); the "prove in Lean, extract the crate" workflow does not exist. We write Rust, lift it into Lean, and prove the lifted model against a hand-written Lean spec.

- **hax**: Rust → Lean transpilation of a pure, panic-free subset. For the functional parts of `certify-core` (classifiers, verdict algebra, sign-counting).
- **Aeneas** (via Charon/MIR): compiles imperative Rust (mutation through `&mut`) to a **pure functional image** in Lean via backward-functions. For `certify-core` parts that need local mutation (in-place polynomial arithmetic).
- Pick **per function** at the spike (§7); they coexist in one Lean-targeted repo. If one covers `certify-core` alone, prefer it for uniformity.

**The alignment gap — the one real risk, managed:**
1. *Lifting fidelity* (hax/Aeneas faithful to Rust semantics) — trusted, in the TCB.
2. *Arithmetic model match* — Lean proofs reason over `Int`/`Rat`. **Prove the checkers over `Int`/`Rat` matching the BigInt slow path** (whose semantics match Lean directly and which is the semantic reference anyway); **Kani proves the fixed-limb fast path ≡ slow path** on its domain (Layer K). Lean and Kani meet at the lattice boundary.
3. *Spec match* — each checker's Lean spec is hand-written and reviewed; it *is* the formalization of the certificate definition and will surface spec ambiguities (formalization always does), which is a feature.

**Escalation for deep lemmas** where runtime-checked hypotheses don't reduce enough: quotient-emission correctness (π₀ faces ⇒ valid cycles), CAP-OUT-LINK ⇒ 2-manifold-with-boundary. Research-flavored and open-ended, **optional for milestone D** — the runtime-checked-hypothesis route covers soundness in the interim.

**Creusot/Verus/hax-to-F\*** noted as fallbacks only; no tool religion; `certify-core` is small and pure so it can migrate. Creusot is *not* run in parallel with Lean (redundant).

Binding rule: **soundness-critical ∧ small ∧ pure → prove (Kani or Lean); large ∧ search-like → certify + checked.** The falsely-certifying defect class (CLIP-σ corner reduction, the Sylvester semidefinite slip) is exactly what proven checkers eliminate.

---

## 5. Layer K — Kani (bounded model checking), honestly scoped

Kani = CBMC: strong on bounded integers, bit-level logic, panic/overflow/UB reachability; **weak-to-hopeless on unbounded BigInt heap loops**. Feasible, high-value targets:

- **Fast≡slow lattice equivalence** (the Lean bridge): the L0 fixed-limb fast path (i128 / `[u64; N]`, explicit overflow → promotion) proven equal to the BigInt slow path on its domain, and promotion-trigger correctness. This is *the* handoff that keeps Lean in `Int`-world.
- **Finite combinatorial functions, exhaustively**: the occupancy→row classifier (≤ 6 bits — proven total and correct against the quadrant-interval definition), the CLIP-ladder dispatch, `gate` verdict-propagation (pure enum algebra), sign-variation counting given a sign sequence (redundant with Lean — cheap second witness).
- **Bounded DCEL bookkeeping**: edge split, twin pairing, coincident-edge merge, bit propagation maintain invariants for all configurations up to N ≈ 6–10 half-edges (unwind-bounded). Precisely the bookkeeping-defect class the review kept finding.
- **Panic-freedom / no-overflow** across the `lattice` fast path and `gate`.

Out of Kani scope (tested + checked): full arrangements over ℚ, Sturm over arbitrary degree, resultants.

---

## 6. The V&V matrix — living CI-gated artifact (`vv-matrix.md`)

Rows = every certificate + kernel operation; columns = {unit, property, differential, Kani, Lean, validation}; cells = status. **CI fails a milestone gate if a soundness-critical row has an empty {Kani ∨ Lean ∨ runtime-checked-hypothesis} cell.** The pattern ledger (spec deltas) is the code-review checklist; the `:=` census and tuple-predicate greps run in CI over `spec/` and doc-comments; the truth-valued-only rule is a convention check on `gate`.

**TCB, stated**: rustc; the bignum crate core ops (mitigated: second backend + Kani on limbs); the hax/Aeneas lifter + Charon/MIR frontend; Lean 4 kernel + cited Mathlib lemmas; Kani/CBMC; the hand-written Lean specs of the checkers (reviewed); spec v0.24. Everything else is checked by something in this list. Every addition buys a defect class the review proved we have.

---

## 7. The extraction de-risking spike (M0, do before committing the approach)

Highest-*variance* unknown; price it before building on it.

1. Implement one real `certify-core` function — the **sign-variation counter** (small, pure, soundness-critical, representative).
2. Lift it with **both** hax and Aeneas to Lean 4.
3. Write its Lean spec (variation count = the mathematical definition) and **prove the lift meets it**.
4. Separately, prove the **Sturm hypothesis-checker** (chain identities ⇒ Sturm chain) against a Mathlib-cited statement.
5. Record: which tool lifted more cleanly, proof effort, Mathlib coverage gaps, semantic-fidelity surprises.

Exit: go/no-go with real numbers + a template every later `certify-core` function follows.
**Pre-decided fallback if ugly**: Lean for stand-alone theorem development (certificate math proven about Lean models hand-transcribed from the checkers) + heavy Kani/property/differential coverage of the Rust, accepting a hand-audited transcription gap. Weaker than end-to-end, still far past industry norm, and exactly what the certifying-algorithm structure tolerates.

---

## 8. Validation ladder (spec ⊨ reality)

1. **Analytic golden instances**: cone/cylinder/plane configs with pencil-and-paper values for every certified quantity (κ fields, development lengths, cap areas, ledge extents) — exact expected-output files; device parameters (β = 42°, ID 5 mm, 1.49 wrap, 1.6 mm seam) the primary fixture.
2. **Independent-kernel acceptance**: STEP shells into OpenCascade *and* one commercial kernel (NX/Fusion import) — pass their checkers, survive a boolean; disagreements are findings against either side, triaged.
3. **Fab-artifact regression**: the tool's flat patterns diffed against the existing hand-proven cone flat pattern and petal Gerber/DXF — the line's current artifacts are ground truth bought the expensive way.
4. **Hardware loop**: the Phase 3 Rev A/B prototypes are the acceptance vehicles — flat pattern → formed cone on the vacuum mandrel → dimensional inspection vs the model's certified dimensions. The tool ships nothing to the line before reproducing a part the line already trusts.
5. **Float-viewer disagreement logging**: any place a float diagnostic disagrees with an exact predicate is logged (usually a viewer bug; occasionally a conditioning insight).

Milestone acceptance criteria are written **before** each milestone's implementation, appended here.

### M0 acceptance criteria (repo skeleton + `lattice` + the extraction spike)

*Authored 2026-07-30, before implementation, per the rule above.*

**Skeleton (task 1) — met when:**
- `cargo check`, `cargo clippy --all-targets -- -D warnings`, and `cargo fmt --all --check` are green on both `x86_64-linux` and `aarch64-darwin`.
- The pure tier (`lattice`, `certify-core`) is `#![no_std]` + `#![forbid(unsafe_code)]`; the only crate not forbidding unsafe is `difftest` (FFI, non-certified oracle).
- `nix develop` provides the pinned toolchain (`rustc` == the `rust-toolchain.toml` pin) plus the difftest/Lean tooling; `flake.lock` is committed.
- CI runs the `cargo xtask lint` checks + fmt + clippy + tests inside the devShell; invariant 1 (no floats — literals **and** `f32`/`f64` types) is enforced by the separate `no_float` dylint lint over the certified lib paths (rustup-nightly leg, not xtask).

**`lattice` (task 2) — met when:**
- The bignum backend is chosen by the documented benchmark (Sturm on a degree-12 polynomial over 256-bit rationals) **and** is `no_std + alloc`; it sits behind `backend::Backend`, with no raw bignum ops outside `lattice`.
- Exact `cmp`/`sign`/`gcd`, interval-plus-separation comparison, polynomial arithmetic, Sturm (isolation + sign-on-interval), and bivariate resultants are implemented and unit-tested.
- Kani harnesses are green for: L0 fast-path ≡ BigInt slow-path on the fast path's domain, promotion-trigger correctness, and panic-/overflow-freedom on the fast path.
- The `lattice cmp/sign`, `Sturm isolate`, and `resultant` rows in `vv-matrix.md` have no empty `{Kani ∨ Lean ∨ runtime-checked-hypothesis}` cell.

**Extraction spike (task 3) — met when (this is the §7 go/no-go):**
- The sign-variation counter is lifted to Lean by **both** hax and Aeneas and proven against its hand-written Lean spec.
- The Sturm hypothesis-checker (chain identities ⇒ Sturm chain) is proven against a Mathlib-cited statement; `docs/proofs/ledger.md` carries the entry (citation, hypotheses-checked-at-runtime vs structural).
- Recorded: which tool lifted more cleanly, proof effort, Mathlib coverage gaps, semantic-fidelity surprises, the per-checker template, and the go/no-go decision (with the §7 fallback taken if no-go).
- The exact Kani / hax / Charon+Aeneas / Lean / Mathlib versions are locked into `flake.lock` + the toolchain files, resolving the `[spike]` items in `environment-and-crate-layout.md §2/§3`.

### M3a acceptance criteria (`arrange2d` canonical decomposition + event spine)

*Authored 2026-07-31, before implementation, per the rule above.* M3a is the front half of the §6 exact arrangement — canonical decomposition + the event spine (steps 1–4). `arrange2d` is an untrusted **searcher** (soundness lives in the `certify_core::arrange` checkers at M3e); M3a is validated by differential + property + corpus, emitting a replayable `(events, witness)`.

**Foundations (Phase 0) — met when:**
- `lattice::Surd` has exact field arithmetic (`add`/`sub`/`neg`/`scale`/`mul`) — same-radical closed in `Surd`, cross-radical escalating to `AlgReal` (via resultant elimination) — unit-tested + differentially cross-checked (closed-form vs interval/resultant); `lattice` stays `#![no_std]` (the `thumbv7em` gate green).
- `geom::content` defines the D24 2D content primitives (`Line` a·x+b·y+c; `Circle` center+r²; `Point2` with `Surd` coords + lexicographic `Ord`; x-monotone `Edge` pieces; `Winding` = provenance), reachable by the shell crates without depending on `arrange2d`.
- The CGAL `Arrangement_2` (circular-kernel) differential oracle builds under `nix develop` (feature-gated `cgal`; cxx/JSON FFI; the only `unsafe`) and a link smoke test returns the **exact rational vertex** of two crossing lines.

**Decomposition + event spine (Phases 1–4) — met when:**
- Canonical x-monotone decomposition (pending-v0.25 profile) is exact: a full circle splits into simple x-monotone pieces at its exact x-extremal points, the axis-aligned chart subsumes the pole, no `Edge` spans more than one simple arc, and winding is provenance (never DCEL multiplicity).
- The event spine implements spec §6 steps 1–4 most-degenerate-first: CARRIER-COINCIDENT tested first; carrier∩carrier solved exactly as degree-≤2 `Surd` points; interval membership on **both** edges before any classification (non-members discarded, no record); retained points classified transverse/tangent under the `d²>0 ∨ ¬COINCIDENT` guard, with sidedness bits. Line predicates use the exact PARALLEL (direction cross) and COINCIDENT (three-minor) forms. Zero-length is an identity (ℓ=0 ⇒ vertex id; `0<ℓ<q_sep` stays a real edge); below-lattice-decidability ⇒ `Unresolved(margin)`.
- **No floats in any certified predicate path** (the `no_float` dylint lint over `lattice`, `certify-core`, `arrange2d` — float literals **and** `f32`/`f64` types; test code is exempt).
- The M3a-core corpus fixtures reproduce their verdicts as `cx_*` unit tests: `cx_antipodal_arcs`, `cx_coincident_vs_tangent_circles`, `cx_tangent_outside_arc`, `cx_parallel_distinct_lines`, `cx_full_circle_edge`.

**V&V activation (Phase 5) — met when:**
- The event set agrees with the CGAL `Arrangement_2` oracle **up to the quotient** on corpus + generated inputs (exact `a+b√d` comparison, no tolerance).
- Stratum-weighted generators (over-sampling degenerate strata, built exactly in-lattice) + degeneracy-directed shrinkers are live with a CI degenerate-heavy knob; the event set satisfies the metamorphic invariants (rational rigid motion, lattice rescaling).
- CI activates the stratum-weighted proptest/fuzz suites and the milestone-gate `vv-matrix.md` check (scoped to M3a's rows); the M3a `arrange2d` rows carry unit / property / differential (Kani / Lean = N/A for the ℚ-arrangement searcher slice).

**Status: M3a met.** Phases 0–5 landed: the `geom::content` primitives + `lattice::Surd`
arithmetic; the `arrange2d` decomposition → predicates/carrier → membership → classification +
event-spine pipeline (`arrange_events`); the CGAL `Arrangement_2` differential (up to the
quotient, exact `a+b√d`) + the in-crate `resultant_bivariate` count oracle + the rigid-motion /
lattice-rescaling metamorphic invariants; the stratum generators with the `ARRANGE_STRATUM_WEIGHT`
knob; and the CI activation (CGAL suite + milestone gate) with the `arrange2d` rows filled in
`vv-matrix.md`.

**Out of M3a (deferred, not gated here):** stage-2 1D coincidence lattice (3c); DCEL + 8-step boolean + π₀ quotient emission (3d); CAP-OUT / CAP-OUT-LINK / completeness bijections (3e — the ★ soundness-critical rows, Lean = "(research)").

### 3c acceptance criteria (stage-2 1D coincidence lattice + §8.3 azimuth calculus)

*Authored before implementation, per the rule above.* Slice 3c fills the M3a event-spine seam: when two edges share a CARRIER (coincident lines / arcs on one circle), the exact **1D domain arrangement on that carrier** decides the outcome lattice (spec §6, `…-v0.24-full.md:289`) and **emits arrangement primitives** for 3d's DCEL. Like M3a, 3c is an untrusted **searcher** (Kani / Lean = N/A; soundness is the `certify_core::arrange` checkers at M3e); it is validated by corpus + property + differential. 3c processes only **distinct-source** coincident pairs (two pieces of one decomposed curve are structure, not coincidence).

**§8.3 azimuth calculus (foundation) — met when:**
- `arrange2d::azimuth` computes the exact half-angle stereographic tag `t = (p_y−cy)/((p_x−cx)+√r²) = tan(θ/2)` of a point on a circle, stored unevaluated as `(num, den)` `Surd`s (one radical `d = r²`); the pole `den = 0` is exactly the x-min extremal `L` (θ = π) — discharging the "winding integer computed in slice 3c" note. `tag_cmp` is the exact CCW angular order (phase + cross-multiplied `Surd` comparison, no angle materialized); the ±2π signed-crossing winding integer is exact (Sturm-isolated poles) and is structurally 0 for an x-monotone piece.
- Verified: tags monotone around the circle; `tag_cmp` invariant under rational rigid motion; winding of hand-built wrapping vs non-wrapping arcs; the pole is `L`.

**The 1D coincidence lattice (core) — met when:**
- On a shared carrier, the unified 1D overlap decides the normative outcome lattice exactly: `disjoint` ⇒ nothing; `touch-at-point` ⇒ a vertex; `partial overlap` ⇒ one merged edge (both operands) **plus the residual sub-edges** (single operand); `containment` and `equality` as the degenerate cases of the same form. Segments use the along-line parameter; arcs use the §8.3 tag interval (winding-aware, unifying both halves). Zero-length is identity (ℓ=0 ⇒ vertex id); below-lattice-decidability ⇒ `Unresolved(margin)` (unreachable at degree ≤2).
- The searcher entry emits the coincidence primitives (`arrange_events → (EventSet, CoincSet, Witness)`) with a replayable coincidence certificate; the spine skips same-source pairs.
- Corpus: all five outcomes for a shared line and a shared circle; a true antipodal-arcs pair (distinct-source, disjoint) ⇒ nothing; two coincident circles (distinct source) ⇒ equality; the M3a `cx_antipodal_arcs` (same source) stays 0.
- Property: reassembly (`merged ∪ residuals` reconstruct the union of the two edges' point sets), residuals disjoint from the merged span, outcome invariant under rational rigid motion + lattice rescaling.

**V&V activation — met when:**
- The coincidence output agrees with the CGAL `Arrangement_2` **overlap-edge** oracle (`Arr_curve_data_traits_2` provenance + overlap merge): our merged edge ≡ CGAL's overlap edge (≥2 originating ids), our residuals ≡ CGAL's single-id edges, exact `a+b√d` endpoints.
- CI runs the coincident configs in the `--features cgal` differential; `vv-matrix.md` gains an `arrange2d [M3c]` searcher row (unit/property/differential; Kani/Lean N/A). Corrects the Phase-5c mis-tag: CLIP-σ → `[M2]`, strict Sylvester → `[M4]` (M2/M4 CLIP-DOM checkers, not the 3c arrangement lattice).

**Status: 3c met.** Phases 3c.0–3c.2 landed: the §8.3 azimuth calculus (`arrange2d::azimuth` —
half-angle tag, exact CCW `tag_cmp`, winding); the 1D coincidence lattice (`arrange2d::coincide` —
the five-outcome unified overlap for shared lines/circles) emitting `CoincEdge`s (merged + residual)
through `arrange_events → (EventSet, CoincSet, Witness)`, wired into the spine's CARRIER-COINCIDENT
branch (distinct-source only); and the CGAL `Arr_curve_data_traits_2` overlap-edge differential +
the `vv-matrix.md` `[M3c]` row + gate. Next: 3d (DCEL + 8-step boolean + π₀ quotient emission).

### 3d acceptance criteria (DCEL + the eight-step boolean + π₀ quotient emission — LEDGE-DOM)

*Authored before implementation, per the rule above.* Slice 3d mints the region semantics the event
calculus lacked: the spec §6 "Cell construction", steps (1)–(8), certificate-named **LEDGE-DOM**
(`spec/…-v0.24-full.md:289`; §8.5 row `:383`). It consumes the 3c searcher output
(`arrange_events → (EventSet, CoincSet, Witness)`) and emits the boolean's region faces.

**Unlike 3a/3b/3c, 3d is NOT a pure searcher slice.** The DCEL construction, bit propagation,
selection, and separating-edge/π₀ emission are the untrusted **searcher** (in `arrange2d`); but the
**ℤ₂² cocycle check is a checker** that lives in the pure tier (`certify_core::arrange`), and the
`vv-matrix.md` `quotient emission ★ [M3d]` row is **soundness-critical**. That ★ cell is discharged
by a **real proof — Kani (bounded model checking) first; if intractable, a deductive Lean proof via
the Charon+Aeneas extraction pipeline** (no runtime-checked-hypothesis shortcut, no defer). "Bounded
DCEL bookkeeping … bit propagation … N ≈ 6–10 half-edges" is explicitly in Kani scope (§5). The
region certificate (CAP-OUT / CAP-OUT-LINK / `Link_emitted ≅ Link_geometric`) stays 3e; 3d ships only
its two self-diagnostics (the ℤ₂² cocycle check and the substrate-link twin-pairing/no-dangling check).

**The general-tangent azimuth comparator (foundation) — met when:**
- `arrange2d` computes the exact **outgoing-tangent azimuth** of a half-edge leaving a vertex — over
  arbitrary `Surd` direction vectors from mixed edges (lines + different circles) — as a total cyclic
  order (half-plane split + cross-product sign, no angle materialized), with the segment tangent
  `±(b,−a)` and the arc tangent `±(−n_y, n_x)` (sign resolved from `half` + which x-extreme the vertex
  is), and a **curvature tie-break** for coincident tangents (the `TouchKind::Tangent` case — line
  tangent to circle, mutually-tangent circles). Distinct from `azimuth::tag_cmp` (positions on one
  fixed circle).
- Verified: total cyclic order (antisymmetry/transitivity), rigid-invariance of the order, and the
  curvature tie-break (a line tangent to a circle from inside vs outside; two mutually-tangent circles).

**The DCEL + eight-step boolean (core) — met when:**
- The half-edge DCEL is built from **(input edges split at the interior `EventSet` vertices) ∪ the
  `CoincSet` sub-edges** (3d owns the ordinary-edge splitting; the spine emits only coincidence
  sub-edges pre-split), with twin pairing, the azimuth-sorted vertex rotation, and traced faces; the
  **substrate-link self-diagnostic** (twin pairing complete, no dangling half-edge, alternating labels)
  passes.
- The eight steps hold exactly (spec `:289`): seed the unbounded cell `(A,B)=(0,0)`; operand sidedness
  = the **stored face-orientation bit, re-read not computed** (`SegPiece.orient` / `ArcPiece.winding.
  orient`); propagate `(A,B)` (∂F_A flips A, ∂F_B flips B, coincident flips both); the **ℤ₂² cocycle
  check** (every closed walk returns its bits); coincident edges carry both operands' signed incidence;
  **pluggable selection** △ = A⊕B, ∧, ∨; **emit only separating edges** (three-way law); and
  **faces = π₀** of selected-cell adjacency along selected|selected edges — **one face per connected
  component, not per cell**.
- Corpus: two overlapping disks — ∪ ⇒ one face, ∩ ⇒ one face, △ ⇒ two faces pinched at the internal
  tangency (the "three selected cells, mutual arcs suppressed" case); a clean miter ⇒ empty △.

**The ★ soundness proof (`certify_core::arrange`) — met when:**
- The ℤ₂² cocycle-closure check is a pure, `no_std`, panic-free checker over a **flattened index-array
  certificate** (Kani-harnessable + Charon-extractable); the `arrange2d` searcher calls it as its
  self-diagnostic.
- Its soundness is discharged by **Kani** (a bounded-DCEL bit-propagation/cocycle harness, N ≈ 6–10
  half-edges, unwind-bounded — the first Kani surface outside `lattice`) **or**, if Kani is
  intractable, a **deductive Lean proof** over the Aeneas-lifted model (the `GcdReduce.lean`
  "CBMC-intractable → proven here" template; certify-core stood up as a second extraction surface),
  axiom-clean under the `#print axioms` gate. The `vv-matrix.md` `[M3d]` Kani-or-Lean cell is filled
  and `M3d` joins the milestone-gate `landed` set.

**V&V activation — met when:**
- The emitted region agrees with **two CGAL oracles**: Option A `Boolean_set_operations_2` +
  `Gps_circle_segment_traits_2` (per-component `General_polygon_with_holes_2` ↔ faces = π₀, exact
  `a+b√d` boundary edges) as the primary independent boolean, and Option B `Arrangement_2` face
  iteration as a DCEL-structure cross-check, on △/∧/∨ over corpus + generated inputs.
- The emitted faces satisfy the metamorphic invariants (rational rigid motion, lattice rescaling) and
  an **Euler-characteristic** consistency property; CI runs the region/boolean configs in the
  `--features cgal` differential; `vv-matrix.md` gains the `[M3d]` row (unit / property (Euler) /
  differential (CGAL) / Kani-or-Lean).

**Status: 3d met (connected regime), with a scoped follow-up.** Landed: the general-tangent azimuth
comparator (`arrange2d::tangent`); the half-edge DCEL (`arrange2d::dcel` — split at arrangement
vertices, coincident-merge, azimuth rotation, face-cycle tracing, substrate-link self-diagnostic); the
eight-step ℤ₂² boolean (`arrange2d::boolean` — seed, propagate, △/∧/∨ selection, separating-edge
emission, π₀ quotient); the **★ soundness** as a Kani proof — the pure `certify_core::arrange::
cocycle_ok` checker (first Kani surface outside `lattice`), with `cocycle_implies_telescoping`
verifying accept ⇒ every closed walk returns its bits (Euler V−E+F asserted in the DCEL corpus;
rigid + lattice-rescale invariance of the △/∩/∪ face counts); and the CGAL `Boolean_set_operations_2`
region differential (∪/∩ face counts agree on the non-pinching overlapping-disk cases). Also fixed the
milestone gate, which had been passing **vacuously** (whitespace field split — now `FS="|"`).

**Explicit deferrals (a focused follow-up, natural home 3e CAP-OUT):**
- **General face identification** — the boolean is exact on **transverse-crossing** overlaps and
  **identical/coincident** operands; three degenerate classes need per-component point-location +
  cycle→face nesting: **disconnected** (disjoint) and **nested non-touching** (annulus/hole) are
  **self-detected** (the proven cocycle checker returns `false` ⇒ `Unresolved`, never silently wrong);
  **tangency** (operands touching at a point) *does* close the cocycle, so it is **not** self-detected
  and its emitted face count can be **frame-dependent** (the tangent point may coincide with the
  axis-aligned decomposition's x-extremum after a rotation) — a real caveat until face-ID lands. This
  is exactly the domain of 3e's CAP-OUT components↔faces bijection; tests are scoped to the transverse
  regime.
- **△ pinch semantics** (a *validated finding*, not a defect): at a pinch point (△ of overlapping
  disks), our π₀ separates the lunes into two faces (spec §6: "π₀ keeps them separate, CAP-OUT-LINK
  rejects the vertex"), while CGAL's set-boolean joins them into one — so face counts differ by exactly
  the pinch. The CGAL Option B (`Arrangement_2` face iteration) cross-check and the exact per-edge
  `a+b√d` boundary-set differential both need the same point-location machinery as the face-ID
  follow-up, so they land together there.

### 3e acceptance criteria (CAP-OUT — the region certificate + accurate degenerate handling)

*Authored before implementation, per the rule above.* Slice 3e mints the **region certificate** the
3d self-diagnostics deliberately did not: spec §8.5:383 **CAP-OUT** (`LEDGE-BRANCH := CAP-IN-D24 ∧
LEDGE-DOM ∧ CAP-OUT ∧ SEW`, §8.6:422), a per-run output postcondition, **correctness ∧ completeness**.
It also closes the three degenerate classes 3d left open (disjoint, nested-annulus, tangency), which all
converge on **one** new machinery: exact point-location (horizontal ray-cast winding) + per-component
seeding + CAP-OUT-LINK + Face-with-holes nesting. Milestone A's exit (`implementation-plan-v1.md:47`,
"all CAP-OUT clauses green") is gated on this slice.

**Like 3d, 3e is NOT a pure searcher slice.** The point-location, region rebuild, and CAP-OUT assembly
are the untrusted **searcher** (in `arrange2d`); but the two new **checkers** — the V_∂ sector
cyclic-interval test and the `Link_emitted ≅ Link_geometric` identity-fixing cyclic isomorphism — live in
the pure tier (`certify_core::arrange`), and the `vv-matrix.md` `[M3e]` **CAP-OUT-LINK** and **Link≅geom**
rows are ★ soundness-critical. Those ★ cells are discharged by **Kani** (bounded sector/link bookkeeping,
N ≈ 6–10 sectors — §5:73 in scope; the load-bearing discharge that **gates the merge**), **plus** a real
**Lean-via-Aeneas attempt at the deep theorem** (CAP-OUT-LINK at every vertex ⇒ 2-manifold-with-boundary;
π₀ faces ⇒ valid closed cycles) — `certify-core` stood up as a **second extraction surface**, targeting
axiom-clean, else 1-cited-axiom (SturmChecker precedent) or a documented scoped fragment. The deep theorem
is open-ended research (§5:59, optional-for-D); it **upgrades the Lean cell but does not gate the merge**.
No runtime-checked-hypothesis shortcut.

**Exact point-location (foundation) — met when:**
- `arrange2d::locate` computes the exact horizontal-ray ∩ edge crossings — arc crossing
  `x = cx ± √(r2 − (y0−cy)²)` as a `Surd` in the **new radical** `d = r2−(y0−cy)²` (filtered by `Half`
  and the arc's `[x_lo, x_hi]`; an upper/lower half is met twice), segment crossing rational — and a
  ray-cast **crossing-parity** point-in-region test (transverse crossings only; grazing extrema and
  vertex-height rays excluded by genericity of `y0`), plus a **strict** between and a `Surd`-vs-`Rat`
  comparator (`Surd::cmp` is cross-radical-safe).
- Verified: known rational interior/exterior points of a disk / annulus / lens; rigid-motion and
  lattice-rescale invariance of the point-in-region verdict; parity independent of the `y0` choice
  (away from vertices).

**The region rebuild (Face-with-holes + per-component seeding) — met when:**
- `Face` carries an **outer** cycle (CCW) + counter-oriented **holes** (CW); the emitted region nests
  each hole cycle into its containing face by point-location.
- Cell labeling **seeds per connected component** of the cycle-adjacency graph (one geometric face — the
  unbounded one especially — is bounded by several traced cycles; a single BFS seed cannot reach them),
  computing each seed's `(A,B)` label by ray-cast winding. Disconnected + nested now label consistently ⇒
  the proven cocycle **closes** correctly instead of failing to `Unresolved`.
- The `ledge_dom` silent-wrong path is closed (correct on disconnected/nested, not merely `ledge_dom_checked`).
- Corpus: two disjoint disks ∪ ⇒ two faces; a nested annulus ⇒ one face + one hole; ∩ of nested disks ⇒
  the inner disk.

**CAP-OUT-LINK + V_∂ membership (the tangency fix, ★) — met when:**
- The pure `certify_core::arrange` checker computes V_∂ membership from a vertex's cyclic sector-selected
  mask: **one proper cyclic interval ⇒ v ∈ V_∂**; **full circle or none ⇒ v ∉ V_∂** (interior/exterior);
  **≥2 disjoint intervals (a pinch) ⇒ reject**. Pure, `no_std`, panic-free; **Kani-proved** over all
  bounded masks (the second Kani surface in certify-core).
- Because the sector order is `dir_cmp` (geometric, frame-invariant), tangency's emitted structure stops
  depending on the decomposition's x-extrema: the pinch vertex is classified deterministically (spec:
  "π₀ keeps them separate, CAP-OUT-LINK rejects the vertex"). Tangency **leaves the transverse-only test
  scope** — the 3d.4b rigid-invariance proptest is re-enabled for it and passes.

**Link_emitted ≅ Link_geometric + completeness bijections (the CAP-OUT verdict, ★) — met when:**
- The pure `certify_core::arrange` `link_iso_ok` checker verifies the stored face-cycle order equals the
  geometric sort as an **identity-fixing oriented cyclic isomorphism** (not a multiset match — the spec's
  `a→c→b→d` passes every count yet crosses); **Kani-proved** (bounded).
- The CAP-OUT verdict asserts the three completeness bijections {components}↔{faces}, {separating
  edges}↔{boundary edges}, V_∂↔{emitted shell vertices}, plus: selected cycles close, outer/hole cycles
  counter-oriented, the separation predicate, no duplicates.

**V&V activation — met when:**
- The emitted region (outer + holes) agrees with **both CGAL oracles**: Option A `Boolean_set_operations_2`
  per-component `General_polygon_with_holes_2`, and Option B `Arrangement_2` face iteration
  (`is_unbounded`/`outer_ccb`/`holes_begin`) — on △/∧/∨ over the corpus + generated inputs across the
  **full** regime (transverse, coincident, disjoint, annulus, tangency), exact `a+b√d` per-edge boundary.
- Euler-characteristic + rigid/rescale invariance hold over the full regime; CI runs the region configs in
  the `--features cgal` differential; the three `vv-matrix.md` `[M3e]` rows are filled (the two ★ rows
  Kani-proved, the Lean cell upgraded by the deep-theorem attempt), `M3e` joins the milestone-gate
  `landed` set, and the theorems join the `#print axioms` audit.

**Status: 3e met (soundness + full-regime frame invariance), with a scoped V&V follow-up.** Landed:
exact horizontal-ray point-location (`arrange2d::locate`); the horizontal-slab per-cell labeling that made
**disjoint + nested exact** (cocycle closes where 3d self-detected `Unresolved`) and fixed a latent
`unbounded_cycle` bug 3d had masked; the **Face-with-holes** boundary-loop emission (an annulus `△` is one
face with one hole) which also removed 3d's **tangency frame-dependence** (face counts are now invariant
under rigid motion + rescaling across the **full** regime — transverse, disjoint, nested, tangency,
identical — the proptests dropped `crosses_twice`); **CAP-OUT-LINK** (`classify_link`/`v_boundary`/`link_ok`)
and **Link_emitted ≅ Link_geometric** (`link_iso_ok`) as pure `certify_core::arrange` checkers, each
**Kani-proven** (`link_ok_iff_no_pinch`, `link_iso_matches_cyclic_adjacency` — the 2nd and 3rd Kani surfaces
outside `lattice`, the ★ discharge that gates the merge); the searcher wiring (`link_classes`/`has_pinch`,
`links_consistent`) + the {separating edges}↔{boundary edges} bijection; and the CGAL differential extended
to `General_polygon_with_holes_2` (faces **and** holes) over the non-pinching regime.

**The Lean-via-Aeneas attempt (3e.5) landed a real fragment:** `certify-core` is now a **second Charon+Aeneas
extraction surface** (`certify-check/CertifyCore/`, registered in the lakefile + extraction-drift), and —
unlike `lattice::small` — it needs **no hand-written externals** (its lifted TCB is exactly Charon+Aeneas+Lean).
The link checkers (`v_boundary`/`link_ok`/`link_iso_ok` + closure) lift, typecheck, and are **axiom-clean and
sorry-free** (`#print axioms … [propext, Classical.choice, Quot.sound]`). `CertifyCheck.CapOut` proves the
**dispatch-soundness** layer deductively over the lifted model: `link_ok` returns `false` iff the vertex is a
`Pinch` (a `V_∂` exclusion — the strict-manifold predicate, not a region rejection), `v_boundary ↔ Boundary`,
and `V_∂ ⊆ accepted` — all axiom-clean.

**The run-counter refinement is now proven in Lean** (`CertifyCheck.CapOutRefine`, axiom-clean): the
Aeneas-lifted `cyclic_true_runs` provably computes the mathematical cyclic-run count (`cyclic_true_runs_spec`,
via `loop.spec_decr_nat` + `step` over the modular-indexing loop, mirroring `sign_variations_spec`), so
**`link_ok ↔ ≤1 run`** is a deductive theorem (`link_ok_spec`) — the unbounded Lean analogue of the bounded
Kani `link_ok_iff_no_pinch`, over the extracted model. So the CAP-OUT-LINK ★ carries *both* a Kani proof
and a Lean refinement.

**Remaining frontier (not gating — research escalation, §5:59):** only **the topological 2-manifold theorem**
(CAP-OUT-LINK at every vertex ⇒ 2-manifold-with-boundary; π₀ faces ⇒ valid cycles). The checker-soundness
proofs (Kani + Lean) are the load-bearing discharge.
- **CGAL Option B** (`Arrangement_2` face iteration) — a redundant DCEL-structure cross-check of the
  Option-A oracle. The △-of-overlapping-disks pinch stays the documented spec-aligned divergence (our π₀
  2 lunes vs CGAL 1 joined region).

**Post-merge hardening (arrange2d verification-gap audit, closed).** A three-front audit of `arrange2d`
surfaced that the proven checkers were largely off the critical path, and the differential/input coverage
was thin. Closed:
- **Certified entry** `boolean::ledge_dom_certified → Verdict<CapOut, CapOutFault, ()>`: computes the
  labeling once and runs *every* proven checker (substrate-link, `cocycle_ok`, per-vertex `link_iso_ok`,
  separating↔boundary bijection) over the **emitted** region, returning a real `Refuted(fault)` on defect
  (a corrupted-labeling test confirms the gate fires) plus the CAP-OUT-LINK `V_∂`/pinch classification.
- **Exact `a+b√d` boundary differential** (`cgal_boolean_boundary`): the emitted region's boundary vertex
  set matches CGAL exactly (radical-safe, rational + irrational radii) — not just counts.
- **Input diversity**: boolean tests over polygon (segment) operands, mixed line+circle, and a degree-6
  vertex (three circles concurrent) — all certify; the disks-only corpus hid no bug.
- **Slab genericity self-check** (`generic_height` + `debug_assert!` in `slab_locate`): a missing
  critical-y (incomplete `critical_ys`) is now a detected fault, not a silent-wrong label.

The **trusted front-half** (`carrier`/`decompose`/`membership`/`classify`/`spine` — no checker reads
coordinates) remains the honest TCB boundary, validated by the CGAL exact-geometry differential.

---

### Milestone B acceptance criteria (device-cone chart + the 1D certificate engine — M1 `geom` + M2 `certify1d`)

*Authored before implementation, per the rule above.* Milestone B builds the first chart layer (M1
`geom`) and the 1D certificate engine (M2 `certify_core::certify1d`) — independent of Milestone A, and the
gate that unblocks C (closure + sew). Exit (`implementation-plan-v1.md:49`): **a certified single-chart
record for the device cone** — evaluated, REG/SLAB certified, the CLIP ladder exercised on a synthetic
trim, mesh κ-cap emitted.

**B has no real searcher.** The M2 checkers verify certificates *about* M1 `geom` chart fields, and the
searcher that produces those (the `closure` crate) is M4. So B is exercised by the device-cone golden +
the M2 corpus fixtures + hand-built flat certificates. Scope boundary, pinned here: `geom` owns the
**total-exact field computations** (untrusted searcher); `certify_core::certify1d` owns the **certified
predicates**. The one ★ soundness-critical row is **CLIP-σ signed** (`[M2]`), discharged by **Kani**
(bounded corner-range signed-disjunction; gates the merge) **plus** a non-gating **Lean-via-Aeneas**
stretch on the certify-core extraction surface. No runtime-checked-hypothesis shortcut.

**The rational-function substrate — met when:**
- `lattice::ratfunc` provides `RatFunc = (num, den)` over `Poly` and a common-denominator `Vec3Rat`
  (`{num:[Poly;3], den}`) with dot / cross / derivative (quotient rule) and a gcd-`reduce`. Stays strictly
  rational: the quaternion sandwich auto-normalizes `n` (numerator norm = `|q|²`), so no unsquared norm
  appears in B; the σ-parametric √ (`ρ = |n̂′|`) is deferred to M3 and is **not** `Surd` (constant radicand).
- Verified: ring axioms; derivative = quotient rule; `reduce` canonicalizes; differential vs a
  `num-rational` oracle at sample σ; stays `no_std` (the `thumbv7em` gate).

**M1 `geom` chart fields (total exact) — met when:**
- `geom::chart` computes `q` (polynomial quaternion spline), `h` (positive-weight rational spline), and
  `n = q·e₃·q̄/|q|²`, `n′`, `r = n×n′`, `|n′|²`, pedal `c = h·n + (h′/|n′|²)·n′`, `X = c+μr`, `C = X+w·n`,
  `det J = (c′+μr′)·n′ + w|n′|²`, `ψ′` (spec §3.2) — all exact over ℚ(σ).
- `geom::tags` computes `CONE(A,a)` (linear solve `h≡n·A`), CYLINDER / CIRC-CYL / CONSLOPE / PLANE
  (spec §3.6), origin-explicit witnesses.
- The hatted stall calculus (`geom::stall`, spec §3.2.2): `p̂=εp`, `μ̂=p̂μ`, `r̂`, `n̂′=εn′/p`, `Ĵ`, the
  stall-limit condition — and the tested identity **`J_raw = p̂·Ĵ`** (one positive factor; guards the `/p`
  fossil and its second site `ĝ_x′ = −Ĵ`). REPARAM (`geom::reparam`, spec §7) is a pure old→new record
  transform, **not** a checker.
- Verified: the unconditional identities (`n` numerator = `|q|²`, developability, offsets-in-family,
  `c·r=0`) as property tests; differential vs num-rational; `J_raw = p̂·Ĵ` on synthetic stall spans (the
  cone has none). These M1 fields are total-exact, checked by structure — **not ★**.

**The device-cone golden — met when:**
- The cone device (β=42°, ID 5 mm, `h≡0`, `CONE(0)`, deg-1 G1 spans, w∈[−120,+120]µm; spec §13) is data
  under `crates/fixtures/`, evaluated through the chart layer, and its golden numbers hold: κ₁≈0.297 mm⁻¹,
  developed sector 2π sin β≈240.9°, ≈1.49 wraps, SLAB slack R₁+w⁻≈3.24 mm.
- The **certified single-chart cone record** assembles the REG-Q / SLAB-S0 / CLIP verdicts + mesh κ-cap
  (`min(s_max, 1/κ₁)`) — the B exit artifact. The `device cone chart [M1]` vv-matrix row is the golden
  (non-★) validation lane.

**M2 `certify_core::certify1d` checkers (pure, `no_std`, total, panic-free) — met when:**
- The corner min/max evaluator carries an explicit **min-or-max tag validated against convexity** (spec
  §8.2 rider) — the utility that makes a corner range sound.
- REG-Q and SLAB-S0 (spec §8.5): consume `Poly` + a **supplied `SturmChain` re-verified via `verify_chain`
  before counting** + `MarginSq` + `Interval`; positive-denominator discipline; SLAB-S0 collapses `+w` at
  `w⁻` and adds the stall-limit ring checks. `MarginSq` is correct here (√-carrying-cleared squares).
- The CLIP ladder (spec §8.5): CLIP-W → CLIP-μ → common-zero isolation (Sturm on `b²+d²`) → per zero
  {CLIP-a | CLIP-σ | reject}, terminating in {certified, rejected}.
- **CLIP-σ signed ★**: `clip_sigma` ranges the **signed** affine `∂_σG` over the four corners (min *and*
  max), `Verified(sign)` iff single-signed and separated by `m_σ`, straddle ⇒ `Unresolved` (subdivide).
  The threshold `m_σ` is a plain **signed `Rat`, never `MarginSq`** — squaring reintroduces the
  interior-minimizing `|·|` unsoundness this row exists to kill (the stored `G=σμ` counterexample,
  `cx-sigma-mu-crossing`).
- TRIM-LOCAL (`G_i>0` at the four outer-fiber corners; catches re-entry) + the CLIP-DOM corner-sign census
  (four G-sign event classes Sturm-isolated + connectivity + consumer re-pointing).
- EDGE-REG returns `EdgeReg{ Pass | Fail | Stall }` — the `Stall→Pending` fourth state is a
  **domain-specific enum, not a `Verdict` variant**; `to_verdict` lowers `Stall → Refuted(Stalled)`
  (gate-failing as stored), never `Unresolved`.
- Verified: the M2 corpus (`cx-sigma-mu-crossing` → `Unresolved`; `cx-clip-common-zero` → `Verified` via
  CLIP-a; `cx-stall-reparam` → `Pending`, then `Verified` after REPARAM); property tests against the cone
  fields.

**★ CLIP-σ discharge — met when:**
- A bounded **Kani** harness (`clip_sigma` over `[Rat;4]`, allocation-free) proves the corner-range
  signed-disjunction correct and rejects the `σμ` falsely-certifying class; registered by name in
  `ci.yml`. This is the load-bearing ★ discharge that **gates the merge**.
- A **Lean-via-Aeneas** stretch lifts `clip_sigma` onto the certify-core extraction surface with a
  `CertifyCheck/ClipSigma.lean` spec under the `#print axioms` audit, landing its honest result
  (axiom-clean / 1-cited-axiom / documented fragment). **Upgrades the Lean cell but does not gate.**

**Documentation (a merge gate, not a retrofit) — met when:** every new public item across
`lattice::ratfunc`, the `geom` chart modules, and `certify_core::certify1d` is documented usage-first and
history-free (no slice/phase tags), with worked runnable doctests on the entry points; `-W missing_docs`
= 0 on the new surface; `cargo doc` clean under `-D` broken/private intra-doc links.

**Deferred to later milestones** (documented, not silently dropped): the petal cone-flank fixture
(geometry not yet pinned by spec §13 — needed for C); SLAB-S1 / QPOS Bernstein (no Bernstein primitive;
M4-adjacent); the full EDGE-REG verdict logic (lives in `sew`/M5); deep substitution/removability
transport; the σ-parametric function-field surd (M3 / Tier-C).

**Status: B met.** Phases B.0–B.7 landed on `milestone-b`: `lattice::ratfunc`; the `geom` chart layer
(`chart`/`tags`/`stall`/`reparam`/`record`); the `certify_core::certify1d` engine (REG-Q, SLAB-S0,
the CLIP ladder with the **★ CLIP-σ signed** checker, TRIM-LOCAL, CLIP-DOM census, EDGE-REG/Pending);
the certified device-cone `ChartRecord` (`fixtures::devices::certified_cone` — REG-Q `|q|²`/`|n′|²`,
SLAB-S0, mesh κ-cap 65/194, all Verified); and the M2 corpus. The ★ CLIP-σ discharge is the Kani proof
`clip_sigma_signed_disjunction_sound` (gating), with `ClipSigma.lean` an axiom-clean ℤ second witness
(hand-mirror; the Rat-carrying Aeneas lift is the post-B algebra rehaul). Deferred items above are
carried forward, not dropped.

---

### Milestone C (part 1) acceptance criteria (the CLOSURE treatment for one joint — M4 `closure`)

*Authored before implementation, per the rule above.* Milestone C is "one joint end-to-end"
(M4 `closure` + M5 `sew` + a thin M6); this block covers the front half, **M4 `closure`** — the
searcher for the `CLOSURE_VALID(j)` treatment obligation (spec §8.5/§8.6, and `docs/closure-scoping.md`
for the per-conjunct disposition). C needs both A and B, which are met, so it is the roadmap's next.
Exit: **a straight-root cylinder-flank joint produces a gate-passing certified cap through *both* the
clean-miter (MITER) and forced-ledge (LEDGE) branches** — `CLOSURE_VALID(j)` minus SEW (SEW is M5). A
genuine plane is not a `Chart` today (`n′ ≡ 0`; see `docs/closure-scoping.md §8`), so the cylinder —
representable, line cut-edges, moving normal — is the first slice; the §13 planar-hub petal is deferred.

**Searcher/checker split, pinned here** (same doctrine as `arrange2d` vs `certify_core::arrange`):
`closure` is the untrusted **searcher** — it builds the joint fields (`b_J`, `G_i`, `V`) and flank
edges and runs the CLIP/MITER searches; the pure-tier `certify_core` **checkers** own every certified
predicate (the extraction/TCB surface). `CLOSURE-CAP(j) := MITER-BRANCH ∨ LEDGE-BRANCH` is a genuine
disjunction of constructions. The one ★ soundness-critical decision is **MITER-FIT monotonicity /
`ε_φ`** (the `σ_B=σ_A³` fossil), with **CAP-IN-D24** the second ★, both discharged by Kani.

**The joint searcher (`closure`, untrusted, any two charts) — met when:**
- `closure`'s entry takes **two arbitrary `geom::Chart`s** (strip spans — cone / cylinder / …) + a
  straight crease + orientation + retained-μ ranges — flank *type* is data, never a control-flow branch —
  and builds the joint-level fields: `b_J = s_J(n_A − n_B)` (`b_A=b_J`, `b_B=−b_J`), the retained-side
  `G_i = (C_i − x₀)·b_i` (kept side `G_i ≥ 0`; the raw `H_i` is diagnostic-only, never in a predicate),
  `V`/`s_bev` (the fan generator + bevel slope), and the flank edges as `geom::content::Edge`s.
- The searcher emits `(claim, certificate)` bundles consumed by the `certify_core` checkers; no
  soundness decision is taken in `closure`. Verified: the fields are exact over ℚ(σ) (differential vs
  the `geom` field accessors), and `H_i` appears in no predicate (the `no_repr_leak`-style discipline).

**CAP-IN-D24 input license (C1) — met when:**
- The `CanonicalEdge` / `ValidatedD24` newtypes are **minted only** by the CAP-IN-D24 checker, over the
  existing `validate_d24` totality seed: per source boundary component, carrier by named identity test
  (planar ⇒ line; cylinder-type ⇒ ruling lines; **cone/oblique/generalized ⇒ conic ⇒ FAIL**), exact
  finite interval, endpoint ownership, verified flank correspondence.
- Consulted **only on the LEDGE branch** — a clean miter never invokes it; for conic caps it is FALSE,
  not vacuous. Verified: cylinder-type (ruling-line) flanks mint `ValidatedD24`; a cone (conic) flank is
  refused with the named fault; endpoint-off-carrier / degenerate / non-canonical inputs are refused
  (extends the `validate_d24` corpus).

**Per-flank regularity bundle (C2) — met when:**
- New `certify_core::wedge` checkers over the joint's two **unit crease normals**, in the
  `MarginSq`/`Verdict` idiom. On the straight-crease (**constant-V**) scope the fan is carried by
  `V = (n_A × n_B)/(1 + n_A·n_B)`, so `|V|² = (1 − d)/(1 + d)` with `d = n_A·n_B`, and every gauge is a
  **division-free ring comparison** clearing the WEDGE denominator `1 + d > 0` (no Sturm, no span): 
  **WEDGE** (`1 + d > 0` — fan sector sub-π on [0,1]), **REG-V** (`|V|² ≥ m > 0`, cleared to
  `(1 − d) − m(1 + d) ≥ 0`; `V=0` deletes the record), **EXT-WEDGE** (`s_bev(1+s_bev)|V|² < 1`, cleared
  to `(1 + d) − s_bev(1+s_bev)(1 − d) > 0` — the [0,1] WEDGE bound does *not* certify the extension).
- **SIDE(b_J)** and **COLLAR** are decomposed by scope. Their **crease-local witness** is delivered by
  the C2 bundle: SIDE's oriented bisector is nonzero (`|b_J|² = 2(1 − d) > 0`) and the bevel split
  `Q(s)=1−2s−|V|²s²` is complementary (`Q(0)>0>Q(1)` free) once REG-V ∧ WEDGE hold; COLLAR's
  quotient-wedge embeds once WEDGE ∧ EXT-WEDGE hold. Their **independently-refutable, support-scoped**
  content — SIDE's "wrong-side" test (retained side `G_i ≥ 0` over the actual flank support) and
  COLLAR's cross-t **TUBE** padding by `D_collar` — needs the `G_i`/tube fields, so it lands with its
  sibling checkers in **C3** (TRIM-LOCAL) and TUBE-LOCAL, noted there. `TUBE-SELF` is the vacuous
  straight-crease case (`κ_max = 0`, §13).
- Verified: each atom refutes on a perturbed field — below-margin `|V|²` (zero-dihedral), over-π fan
  sector (antipodal normals), `s_bev` past the EXT-WEDGE bound — plus malformed-input rejection
  (non-unit normal, non-positive margin, negative bevel), matching the `MarginSq` refutation pattern.

**The trim/clip searcher (C3) — met when:**
- New `closure::trim` **searcher** (no new checker — it is the missing *producer* for the reused
  `certify1d` checkers). It builds `b_J = s_J·(n_A − n_B)` and the retained-side field
  `G_i = (C_i − x₀)·b_i` as its three σ-rational coefficients in the affine `(μ, w)` expansion —
  `g0 = (pedal − x₀)·b`, `g_mu = ruling·b = ∂_μG`, `g_w = normal·b = ∂_wG` (`b_A = b_J`, `b_B = −b_J`)
  — then hands the checkers their certificates: **TRIM-LOCAL** (`trim_local`: `G_i > 0` at the four
  corners of each outer support fiber + a single-fiber interior confinement `reg_q`), the **CLIP-DOM
  ladder** (`clip`: CLIP-W = `g_w² ≥ m` and CLIP-μ = `g_mu² ≥ m` cleared `reg_q` gauges → per-zero
  {CLIP-a | signed CLIP-σ from `∂_σG` corners | reject}), and the **CLIP-DOM census** (`clip_dom` /
  `classify_fiber` over `G_i` box corners). Closes the engineering-log "CLIP μ-coverage + fiber-census
  — producer is M4/closure".
- **SIDE(b_J)'s support-level wrong-side test IS TRIM-LOCAL's corner positivity** (a `G_i ≤ 0` outer
  fiber refutes with `RegFault::OuterFiber`) — no separate checker. **COLLAR's cross-`t` TUBE padding
  `D²_collar` is vacuous** on the straight-crease scope (`κ_max = 0`, §13), so TUBE-LOCAL / TUBE-SELF
  discharge totally — the C2 deferrals land here with no new runtime obligation.
- Verified: a 90° cylinder self-fold (`g_mu ≡ 0` — crease-parallel rulings, so `G_i` is w-only)
  certifies through TRIM-LOCAL and CLIP-W; extending the support past the `g_w` root puts an outer
  fiber on the deleted side ⇒ `trim_local` refutes (the SIDE catch); `clip_dom` reports a connected
  retained support. The signed CLIP-σ leaf keeps the `cx-sigma-mu-crossing` slip `Unresolved`
  (no four-corner `|·|`).

**The LEDGE branch end-to-end (C4) — met when:**
- On the forced-ledge cylinder-flank variant: CAP-IN-D24 (C1) → **reuse** `arrange2d::boolean::
  ledge_dom_certified` (the §6 steps (1)–(8): arrangement → seed (0,0) → operand sidedness → ℤ₂²
  cocycle → coincident incidence → boolean select → separating edges → π₀-quotient emit) → **reuse**
  CAP-OUT (`certify_core::arrange`, CAP-OUT-LINK) ⇒ a certified planar cap region. Mostly wiring; the
  `ShellReady` strict-manifold entry is decided here.
- Verified: a CGAL exact-geometry differential on the planar cap region (the boolean-region lane), and
  the emitted faces match the expected connected components.

**The MITER branch, line-edge / degree-1 (C5) — met when:**
- On the clean-miter cylinder-flank variant: **MITER-FIT** degree-1 corollary (`ℓ_i` affine + monotone via
  Sturm; `φ_J` explicit degree-1 rational map, no resultant machinery), **MITER-EDGE-LEDGER**
  (materialize the passed identities as PAIR-IDENTICAL + EDGE-OCCUPANCY), **MITER-OUT** (EDGE-REG via
  **reuse** `edge_reg`; EDGE-EMB / EDGE-EDGE / CYCLE / EDGE-COVERAGE / VERTEX-ISOLATION).
- **`ε_φ` is the order sign of the monotone correspondence** (a theorem on the regular locus, minted by
  *one* exact oriented-endpoint comparison), **never** the derivative-sign definition. Verified: the
  `σ_B = σ_A³` fossil (strictly monotone, positive endpoint order, zero derivative at 0) does not
  falsely certify; a reversed-order pairing is refused.

**CLOSURE-CAP disjunction + gate wiring (C6) — met when:**
- `CLOSURE-CAP(j) = MITER-BRANCH ∨ LEDGE-BRANCH` and the closure-level `CLOSURE_VALID(j)` conjunction
  **minus SEW** are wired in `certify_core`, with the `:=` census (`xtask lint`) satisfied and SEW
  cited per §8.5 (never redefined). `vv-matrix.md` gains the `[M4]` rows.

**Generality (a hard gate, not a claim) — met when:**
- **No device-specific constant** (no `65/97`, no cone-angle literal) appears in `closure` or
  `certify_core` — checked by grep-lint / review; device data stays in `fixtures`/`export`/tests.
- The M4 exit **corpus spans ≥ 2 representable developable classes and ≥ 2 cone angles**:
  `fixtures::devices` grows a promoted `cylinder()` (out of the `tags.rs` test) and a **second-angle
  cone (≠ 65/97)**. The certified both-branches pipe is demonstrated on the cylinder-flank joint; the
  cone demonstrates CAP-IN-D24 *correctly refusing* a conic cap (the class distinction is real, not
  cone-hardcoded); the second angle proves nothing is 65/97-locked — so generality is *exercised*, not
  merely asserted (spec §13 co-normativity). A genuine `plane()` needs the deferred planar-span type.

**★ soundness discharge — met when:**
- A bounded **Kani** harness proves **MITER-FIT monotonicity / `ε_φ`** correct and rejects the
  `σ_B=σ_A³` fossil class (the load-bearing ★, pattern of `clip_sigma_signed_disjunction_sound`),
  registered by name in `ci.yml`; and a **CAP-IN-D24** harness proves the carrier-identity census
  refuses conic caps. Lean lift optional per the checker doctrine (upgrades the cell, does not gate).

**Documentation (a merge gate, not a retrofit) — met when:** every new public item across `closure`
and the new `certify_core` checkers is documented usage-first and history-free (no slice/phase tags),
with worked runnable doctests on the entry points; `-W missing_docs` = 0 on the new surface; `cargo
doc` clean under `-D` broken/private intra-doc links.

**Deferred to later milestones** (documented, not silently dropped): the **planar-span
representation** (`n′ ≡ 0` — a `PlanarChart` / relaxed `Chart` with its own pedal/ruling calculus; a
`geom`/M1-adjacent feature) and hence the *genuine* §13 planar-hub petal disk and the **petal
cone-flank** second pass (the fold/stall/directrix adversary — also blocked on the §13 petal
geometry); **SEW** (SEW-EDGES ∧ SEW-LINK — M5; M4 emits its EDGE-OCCUPANCY input); the **thin M6**
gate / STEP export; non-straight (curved) crease scope for COLLAR.

**Status: M4 met.** C0–C6 landed on `milestone-c` and merged to `main` (merge `efb174c`): the C0
scoping report (`docs/closure-scoping.md`; cylinder-first, not plane-first — §8) + these criteria +
generality fixtures (promoted `cylinder()` + a second-angle cone); CAP-IN-D24 license census
(C1); the REG-V/WEDGE/EXT-WEDGE regularity bundle (C2); the trim/clip searcher driving the reused
`certify1d` CLIP-DOM/TRIM-LOCAL ladder (C3); the LEDGE branch via `ledge_dom_certified` + CAP-OUT
(C4); the degree-1 MITER branch with `ε_φ` by one exact endpoint compare (C5); the `CLOSURE-CAP =
MITER ∨ LEDGE` disjunction and `CLOSURE_VALID(j)` **minus SEW** capstone (C6). The ★ Kani harnesses
`eps_phi_is_endpoint_order`, `cap_in_cycle_census_sound`, `wedge_clearing_sound` run in `ci.yml`.
SEW (the remaining conjunct on both branches) is Milestone C part 2 — the M5 criteria below.

---

### Milestone C (part 2) acceptance criteria (the SEW obligation — M5 `sew`)

*Authored before implementation, per the rule above.* SEW is the shared final conjunct of **both**
CLOSURE-CAP branches — `MITER-BRANCH := … ∧ SEW`, `LEDGE-BRANCH := … ∧ SEW` (spec §8.5 lines 421–422) —
so closing it turns M4's `CLOSURE_VALID(j)` **minus SEW** into the **full** obligation: a *watertight
sewn shell*. `SEW := SEW-EDGES ∧ SEW-LINK` is **defined once, in §8.5 line 385**, and is cited here,
never redefined (a same-commit twin definition fails the `:=` census). Scope is the same
straight-crease cylinder-flank slice M4 established (`docs/closure-scoping.md`): line cut-edges, both
branches, no `apex` FACE-GERM species (fold-vertices are VERTEX's / banded), no curved-crease jet ties.

**Searcher/checker split, pinned here** (same doctrine as `arrange2d` vs `certify_core::arrange`): the
untrusted **searcher/constructor** lives in the `sew` crate — it *produces* the EDGE-OCCUPANCY packets
and the link records from the M4 branch outputs; the pure-tier `certify_core::sew` **checkers** own
every certified predicate (the extraction/TCB surface), consuming packets and records agnostic to
origin. **M4 mints SEW's inputs, M5 only reads them**: `ε_φ` (`miter::OrderSign`, minted by
`eps_from_cmp`) and the four-bit `Occupancy` + frame bit (`miter::Occupancy`) — no re-mint. The one ★
soundness-critical decision is the **occupancy→row quadrant classifier**, discharged by Kani.

**SEW-EDGES — the edge layer — met when:**
- The **input signature is EDGE-OCCUPANCY = (A_L, A_R, B_L, B_R) + frame bit** per edge — the four
  adjacent-cell occupancies verbatim (two interior-side signs cannot encode one-vs-three quadrants);
  left = the cross-product side of `(t_e, n_Π × t_e)`, the packet **frame-covariant**. Two
  **constructors**: **ARRANGEMENT-BITS** (a projection of the §6 cell labels — four lookups, recomputed
  in `sew` from the public `arrange2d::boolean::CellLabeling`/`separating_ids` on the LEDGE branch) and
  **MITER-REGION-IDENTITY** (scoped to the boundary-boundary stratum `A_L ≠ A_R ∧ B_L ≠ B_R`; sides
  from the stored boundary orientations, same-side agreement derived ∘ `ε_φ` and **checked**; reads
  `miter::LedgerEdge.occupancy` on the MITER branch).
- **Identity obligations dispatched by occupancy** (`Occupancy::is_boundary_boundary` is the key): two
  boundaries ⇒ **PAIR-IDENTICAL** (point-set identity + `ε_φ`); one boundary ⇒ **OUTPUT-SOURCE-IDENTICAL**
  (same carrier ∧ interval **containment** — the arrangement legitimately splits sources — ∧ `ε` vs the
  source half-edge sense, a re-verification of the stored back-reference); zero boundaries ⇒ provenance
  + the zero-output assertions, **no edge-pair identity**.
- **The quadrant test on the packet** — one cyclic interval / all four / none; **opposite quadrants ⇒
  pinch, reject** — plus **typed exact counts both directions** and the reverse equality `{records} =
  {cap-to-flank} ⊔ {flank-to-flank}`; empty and internal ⇒ zero incidence ∧ zero records, asserted.
- Verified: a boundary-boundary miter edge dispatches to PAIR-IDENTICAL and passes; an
  arrangement-split source dispatches to OUTPUT-SOURCE-IDENTICAL with interval containment; an
  **opposite-quadrant** occupancy is refused as a pinch; the reverse-equality count mismatch refutes.

**SEW-LINK — the vertex layer, over V_∂ only — met when:**
- `V_∂` (not `V_cand`) is the domain — asking a suppressed-interior candidate's nonexistent faces for a
  cycle is what the edge layer forbade; on the miter branch `V_cand = V_∂`. For each `v ∈ V_∂` the
  **embedded spherical link** is built: rays licensed by **EDGE-REG**, **sectors by FACE-GERM(species)**
  — **cap** ⇒ Π + the CAP-OUT-LINK sector (cited via `classify_link`); **flank** ⇒ SLAB ∧ CLIP-DOM
  corner ∧ `N_i^cut` (the chart immersion *is* the germ certificate); **fan** ⇒ WEDGE ∧ REG-V; **no
  branch ⇒ reject**.
- **Conclusion: `Link_emitted(v) ≅ Link_geometric(v)`** — the stored half-edge walk equals the
  geometric azimuth sort as an **identity-fixing, oriented cyclic isomorphism**, checked by **reuse** of
  `certify_core::arrange::link_iso_ok` (oracle ∧ audit, never oracle-instead-of-audit — computing the
  reference and never comparing the records lets `a→c→b→d` pass every per-edge count while crossing).
- Verified: a cylinder-flank cap vertex's emitted link matches its geometric sort; an `a→c→b→d`
  crossing link — count-passing — is **refused** by the record comparison.

**★ soundness discharge — met when:** a bounded **Kani** harness `occupancy_row_sound` proves the
SEW-EDGES quadrant→row classifier correct — exhaustive over the four (+frame) occupancy bits,
cross-checked against `classify_link` on the four-quadrant cyclic mask (the independent, already-proven
reference; pattern of `link_ok_iff_no_pinch`) — registered by name in `ci.yml`. It fills the
`occupancy→row ★ [M4]` row of `vv-matrix.md`. Lean lift optional per the checker doctrine.

**CLOSURE_VALID(j) closure + generality — met when:** `SEW := SEW-EDGES ∧ SEW-LINK` is composed into
**both** CLOSURE-CAP branches in `closure::valid`, so `closure_valid` returns the **full**
`CLOSURE_VALID(j)` (the "minus SEW" wording is dropped from code and docs; `ClosureFault` gains the SEW
arm; SEW stays *cited* per §8.5, the `:=` census green). **No device-specific constant** appears in
`sew` or `certify_core::sew` (the flank *type* is data, never a branch). The exit corpus in `fixtures`:
the cylinder-flank joint's clean-miter **and** forced-ledge variants each produce a **SEW-passing sewn
shell**; a pinch (opposite-quadrant) and an `a→c→b→d` crossing are each refuted by name.

**Documentation (a merge gate, not a retrofit) — met when:** every new public item across `sew` and
`certify_core::sew` is documented usage-first and history-free (no slice/phase tags), with worked
runnable doctests on the entry points; `-W missing_docs` = 0 on the new surface; `cargo doc` clean.

**Deferred to later milestones** (documented, not silently dropped): the SEW-LINK **`apex`
tangent-cone species** (no fold-vertex on a straight crease) and the **coincident-ray tie-break
machinery** (invariant jet / normal-jet ladder / osculation-reject / 3D II ties) — the transverse
straight-crease slice has no tangent incident rays; both are the curved/petal second pass, banded here
(mirroring M3e's 2-manifold deferral). Also carried forward from M4: the **planar-span representation**
and the genuine §13 planar-hub / petal cone-flank pass; the **thin M6** gate / STEP export.

**Status: M5 met.** M5.0–M5.4 landed on `milestone-c`: the pure-tier `certify_core::sew` checkers
(`occupancy_row` + `identity_mode` dispatch — the `occupancy_row_sound` ★; `sew_edges` seam records +
both-direction counts; `sew_link` over V_∂ concluding `Link_emitted ≅ Link_geometric` via the reused
`link_iso_ok`), the `sew` searcher (`records_from_miter_ledger`, `check_vertex_link` over
`arrange2d::boolean::vertex_link`), and `closure::valid::closure_valid` now returning the **full**
`CLOSURE_VALID(j)` conjunction — `SEW := SEW-EDGES ∧ SEW-LINK` wired into both CLOSURE-CAP branches. The
cylinder-flank joint's clean-miter and forced-ledge variants each produce a SEW-passing sewn shell
(`fixtures::corpus`); an opposite-quadrant occupancy is refuted `SewEdges(Pinch)` and an `a→c→b→d`
crossing link is refused `SewLink(LinkMismatch)`. Deferred to the curved/petal pass: the `apex`
tangent-cone species and the coincident-ray tie-break jet machinery (no fold-vertex / tangent incident
rays on a straight crease). No new ★: SEW-EDGES rides on `occupancy_row_sound`, SEW-LINK on the
`[M3e]` `link_iso_ok` harness.

---

### Milestone C (part 3) acceptance criteria (the thin M6 — the gate + the STEP export)

*Authored before implementation, per the rule above.* Milestone C's stated exit is a
"**STEP-exportable shell for one joint, gate-passing**" (`docs/implementation-plan-v1.md:51`). With M4
(`closure`) and M5 (`sew`) landed, `closure_valid` already returns the full `CLOSURE_VALID(j)`; the
**thin M6** closes C by (a) evaluating the gate formula `VALID_solid-closure` over one joint and (b)
writing that joint's shell to a real `.step` file. Two spec formulas anchor it (spec §8.6, lines
438–439):

```
VALID_material      := VALID_complement ∧ ⋀_j ( treatment(j) ∈ {SMOOTH, DEFERRED} ∧ CLOSURE_VALID(j) )
VALID_solid-closure := VALID_complement ∧ ⋀_j CLOSURE_VALID(j)
```

Thin M6 delivers **`VALID_solid-closure` for one joint** and its STEP shell. It is explicitly *thin*:
the full cone+petal **atlas** and `VALID_material`, and the **external-kernel audit** (load the `.step`
back into OpenCASCADE/NX/Fusion and run *its* checker), are **Milestone D**; **FRESH** promotion is
**material-grade (Milestone E)**. Gate formulas are truth-valued only — no "band or fail" disjunct (spec
§8.2/§8.6). The pure verdict algebra lives in `certify_core::gate` (the TCB/extraction surface, `no_std`);
the stateful certificate store is the `gate` shell crate; the STEP writer is an `export` deliverable
(floats permitted there, behind the `step` feature, via the quarantined `approx` bridge).

**M6.1 — the pure gate algebra (`certify_core::gate`; the ★) — met when:** the **first reusable
verdict-propagation combinator** over `Verdict<E,W,M>` (workspace-wide, conjunction is 121 hand-rolled
3-arm matches — this is the algebra they reduce to): a short-circuiting **conjunction fold** with
all-`Verified` ⇒ `Verified`; **any** `Refuted` ⇒ the **first** `Refuted` (order-preserving); else (some
`Unresolved`, none `Refuted`) ⇒ `Unresolved`. `VALID_solid-closure` is expressed as this fold over the
per-joint `CLOSURE_VALID(j)` verdicts, with the `VALID_complement` conjunct **vacuously `Verified`** on
the one-joint straight-crease slice (no complement clips). **★ Kani `gate_conj_sound`:** exhaustive over
the three-valued lattice for a bounded N — the fold is `Verified` **iff** every input is `Verified`;
`Refuted` (returning the **first** refuter) **iff** any input is `Refuted`; else `Unresolved`.
Registered by name in `ci.yml`; fills a new gate-algebra ★ row in `vv-matrix.md`. Lean lift optional per
the checker doctrine.

**M6.2 — the certificate store (`gate` shell) — met when:** an **append-only, provenance-linked**
record store — each entry a certificate id + a `Verdict` + provenance links to its source evaluations +
a **stamp** (the certified enclosure) — over which `VALID_solid-closure` is evaluated for one joint via
the M6.1 algebra. The **provenance chain rule** (spec:203) is enforced: a stamp is bounded below by its
sources' certified enclosures, so the store ingests only certified `Verdict` / `MarginSq` data, **never
a naked float**. **FRESH** is a documented deferred **stub** (the three-way containment re-test is
material-grade, M-E; the store's provenance-chain enforcement is its precondition, but the re-test is
not built).

**M6.3 — the STEP-exportable shell + the OCCT `.step` writer — met when:** a neutral **shell record**
(exact) is assembled from `closure::valid::ClosureValid { wedge, cap }` — the cap face (a Ledge
`CapWitness::Ledge(CapOut)` region's `Line`/`Arc` loops lifted into the 3D cap plane via the `cap_in`
`PiFrame`) plus the two flank faces (ruled from the joint charts over their `MuRange`), sewn along the
`v_boundary()` (V_∂). The `export` `step`-feature shim builds a `TopoDS_Shell` from the record (exact →
`f64` through `approx`), writes it with `STEPControl_Writer`, and a shim-side **re-read + `BRepCheck`**
confirms the file loads (**write-then-reload**, *not* the external-kernel audit — that is M-D). The
one-joint **end-to-end**: the cylinder-flank joint → `closure_valid` `Verified` → gate
`VALID_solid-closure` **pass** → a `.step` written **and** reloaded, asserted as a `fixtures` corpus
fixture (both cap branches where the geometry supports it). Fills the `STEP shell [export]`
`vv-matrix.md` row.

**Generality (hard gate) — met when:** no device-specific constant (no `65/97`, no cone-angle literal)
appears in `certify_core::gate`, `gate`, or the shell-record assembly — the gate is a fold over
verdicts, agnostic to the treatment that produced them; the shell record is built from the generic
`ClosureValid` / chart data, the flank *type* carried by the chart, never a Rust branch.

**Documentation (a merge gate, not a retrofit) — met when:** every new public item across
`certify_core::gate`, `gate`, and `export::step` (+ the shell-record assembly) is documented usage-first
and history-free (no slice/phase tags), with worked runnable doctests on the entry points;
`-W missing_docs` = 0 on the new surface; `cargo doc` clean. The `step` feature is **off by default**, so
default builds / CI clippy need no system OCCT (mirrors difftest's `cgal`); the OCCT-backed round-trip
is exercised under `nix develop --features step`.

**Deferred to later milestones** (documented, not silently dropped): **FRESH** promotion (→ M-E,
material grade); the full cone + lap-seam + **petal atlas** and `VALID_material` (→ M-D); the
**external-kernel audit** — `.step` → OCC/NX/Fusion, run *its* checker (→ M-D); the curved-crease
COLLAR; a hand-rolled AP242 emitter (future work — OCCT is the writer for now).

**Status: thin M6 met — Milestone C complete.** M6.0: criteria authored + the OCCT STEP shim de-risked
GO (the `export` `step` feature writes + reloads a box under `nix develop` — `docs/engineering-log.md`).
M6.1 met (the pure `certify_core::gate` algebra + `gate_conj_sound` ★). M6.2 met (the `gate::store`
append-only provenance-linked certificate store, chain rule enforced, `VALID_solid-closure` evaluated for
one joint). M6.3 met: `export::shell::shell_from_closure` assembles the exact float-free `ShellRecord`
(two flank strips at the certified offset `w = t.w.lo` + the Ledge cap fanned through the joint `PiFrame`);
the `step`-feature shim writes it with `STEPControl_Writer` and re-reads it through `BRepCheck_Analyzer`;
the one-joint end-to-end (the `one_joint_*_writes_a_reloadable_step_shell` corpus) runs the cylinder fold →
`closure_valid` `Verified` → gate `VALID_solid-closure` **pass** → a `.step` that reloads
valid, on the public `fixtures::closure_joint` corpus.

*Two honest scope notes.* (1) The faces are joined by OCCT's coincident-edge sewing
(`BRepBuilderAPI_Sewing`), not the explicit `v_boundary()` (V_∂)-guided seam the criterion names — adequate
for the write-then-reload check; the V_∂-guided sew (and thereby a claim of manifold watertightness rather
than per-face `BRepCheck` validity) is a **M-D** follow-up. (2) The cap is exercised through the **Ledge**
branch only; a Miter cap contributes no separate planar face, so the two-branch end-to-end reduces to the
one branch whose geometry adds a face.

### Milestone D (slice 1) acceptance criteria (a physically-meaningful one-joint fixture)

*Authored before implementation, per the rule above.* The roadmap's Milestone D
(`docs/implementation-plan-v1.md:53`) is the **whole device** — "full cone + lap seam + petal atlas;
`VALID_solid-closure` end-to-end; STEP loaded into OpenCascade with its checker as the external audit;
exit: the lens-assembly flex model as a certified solid." That is a culmination, not one vertical slice, so
Milestone D is built as a **sequence of slices**. Three threads decompose the deferred work:

1. **Physical fixture** — replace the certification-artifact fixture with a joint whose STEP renders as a
   recognizable fold: a true `h ≠ 0` cylinder (parallel rulings, not a cone), two distinct flanks sharing
   one crease (edges coincident, no gap), a metric-faithful cap. **This is slice 1.**
2. **Audit + V_∂-guided seam** — drive the sew from the certified `v_boundary()` / `pinches()` (Kani-proven
   in `certify_core::arrange`, but not yet consumed by `export`), and wire OpenCASCADE's `BRepCheck` as a
   **differential oracle** compared against the internal SEW-LINK verdict. A later slice — near-vacuous
   until slice 1 gives two flanks that actually meet.
3. **Atlas breadth** — the petal cone-flank joint (roadmap `C:51`'s deferred second pass, blocked on the
   spec §13 petal geometry) and multi-joint assembly toward the lens model. Later slices.

*Two readings locked here.* (a) **`VALID_material` → Milestone E, not D.** It adds the conjunct
`treatment(j) ∈ {SMOOTH, DEFERRED}` (spec §8.6:438), which requires physical transition bands (the M7
`develop` cold-layer crate) and FRESH promotion — both already E. D stays scoped to `VALID_solid-closure`.
(b) **The "external-kernel audit" is an *oracle*, not the certificate.** The spec is explicit — "no kernel
CSG" (P5:16, §11:470, STEP:464); region/shell manifoldness is certified *internally* by CAP-OUT-LINK /
SEW-LINK (§8.5); and "independent recomputation certifies nothing until compared against the stored answer
— oracle ∧ audit, never oracle-instead-of-audit" (§8.2:332). OpenCASCADE's checker is therefore a
differential oracle (as CGAL is for M3, `implementation-plan-v1.md:75`), compared against our verdict —
never the source of truth. This governs thread 2, not slice 1.

**Slice 1 — the physical joint fixture — met when:** the one-joint STEP shell is a recognizable physical
fold that still certifies. Concretely:

- **A true cylinder — met when:** `fixtures::closure_joint`'s flank charts carry `h ≠ 0`, so pedal `c ≠ 0`
  and the rulings stay parallel (no apex). `geom::chart::Chart` already supports `h ≠ 0` (its `support`
  field is a general `RatFunc`); with `h = const`, `c = h·n` traces a circular directrix — a genuine
  cylinder. Asserted on exact coordinates: the reconstructed flank strips have a **nonzero, non-collapsing
  pedal** across the retained σ-support.
- **A shared crease, no gap — met when:** the two flanks are **distinct charts** whose crease-station
  rulings *coincide* in world space at the joint's dihedral (the straight-crease fold construction — a
  rigid rotation about the crease line; the MONO reflection `n_B = n_A − 2(n_A·B/B·B)B` degenerates cleanly
  at `B ≡ 0`, spec §5.3:248), with retained σ-supports that **abut** the crease. Asserted: the flank-A and
  flank-B crease edges are the **same** exact segment (a real seam), not two disjoint bands.
- **Both cap branches, metric-faithful — met when:** the fixture certifies through **both** a clean-**Miter**
  variant (`cap_miter: Some`, flanks meet directly, no cap face — exercising the C5 MITER machinery the M6
  fixture left unused, filling roadmap `C:51`'s "clean miter *and* forced-ledge variants") **and** a
  forced-**Ledge** variant whose cap lifts **isometrically**. The metric fix: `export::shell::lift`'s frame
  is already orthogonal (`n·n′ = 0 ⇒ r₀·n₀ = 0`); normalizing `u = r₀/√s` with `s = normal_deriv_sq().eval(σ*)`
  (one rational at the single crease station) lifts a rational cap point to a `Surd(a, b, s)` with a
  **common `d = s`** — expressible in the existing `a+b√d` type, no new algebra. Asserted: a unit cap square
  lifts to a **unit** (not `|r₀|`-stretched) world square, by an exact edge-length² equality on the surds.
- **Still certified, still exported — met when:** each variant runs `closure_valid → Verified` (the MITER
  variant via the miter branch), the gate evaluates `VALID_solid-closure` → `Verified`, and the shell writes
  a `.step` that reloads through `BRepCheck` (feature `step`, under `nix develop`). No checker code changes —
  the straight-crease machinery applies unchanged (an `h ≠ 0` cylinder still has `B ≡ 0`); only the
  `treatment` margins (REG-V, `w`, σ_a/σ_b, trim/clip) are **re-tuned** to the new geometry, and the SEW
  packet is rebuilt to describe the real flank-to-flank seam.

**Generality (hard gate) — met when:** the device constants (radius, dihedral, σ-boxes) live **only** in
`fixtures` (a non-certified crate); no certified crate gains a constant, and the flank *type* stays data on
the chart, never a Rust branch.

**Documentation (a merge gate) — met when:** the three warts documented in the `export::shell` /
`fixtures::closure_joint` module docs (cone-taper, disjoint-support gap, stretched cap) are **discharged**
and their doc notes updated to say so; new/changed public surface is documented usage-first with
`-W missing_docs = 0`.

**Deferred to later M-D slices / M-E** (documented, not dropped): the `v_boundary()`-guided seam +
OpenCASCADE `BRepCheck` differential oracle (M-D thread 2 — now unblocked by the real crease); the full
cone + lap-seam + **petal atlas** and multi-joint assembly (M-D later); the petal cone-flank joint (blocked
on spec §13); `VALID_material`, FRESH, the `develop` crate (→ M-E); the curved-crease COLLAR / §14
transition patch; a hand-rolled AP242 emitter.

**Status: slice 1 met.** D.0: this decomposition + criteria authored; dispositions in
`docs/engineering-log.md`. D.1 (`1410d2e`): the physical **90° cylinder self-fold** in
`fixtures::closure_joint` — flank A a true unit cylinder about x̂ (`q = 1+σi`, `h ≡ 1`: nonzero pedal
`c = h·n`, rulings ∥ x̂), flank B that cylinder rigidly translated by `t = (0,1,1)` ⊥ the rulings (a
*distinct* chart, same `q`, support `h_B(σ) = 2(1−σ)/(1+σ²)`); crease stations σ_a = 0 (n = ẑ), σ_b = 1
(n = −ŷ) → a 90° dihedral whose two crease neutral edges both lie on the shared ruling line
`L = {(x,0,1)}`. It certifies through **both** cap branches on the same joint — **MITER**
(`miter_cap` + `treatment_miter`, `cap_miter: Some`, no cap face) and **LEDGE** (`ledge_d24` + `treatment`).
D.2 (`3697a9f`): `export::shell::lift` lifts the Ledge cap through the **orthonormal** crease frame
`{r₀/√s, n₀}`, `s = |r₀|² = normal_deriv_sq(σ*)` — each world coordinate a `Surd(a, b, s)` with common
`d = s`, **isometric** (unit cap square → unit world square, asserted by an exact edge-length² equality on
the surds). D.3: the one-joint **end-to-end** corpus — `one_joint_ledge_writes_a_reloadable_step_shell` and
`one_joint_miter_writes_a_reloadable_step_shell` each run `closure_valid → Verified` → gate
`VALID_solid-closure` **pass** → a `.step` reloaded through `BRepCheck`. The three warts (cone-taper /
disjoint-support gap / stretched cap) are discharged, their `export::shell` and `fixtures::closure_joint`
module-doc notes updated to say so.

*Two honest scope notes.* (1) A shared-crease dihedral is **geometrically impossible** with constant-`h`
charts — matching two flanks' crease baselines `c + w·n` at a nonzero dihedral forces `n_A = n_B` (no fold)
or a line-collapse. The fold is therefore shared at the **neutral surface `w = 0`** (where a true cylinder,
unlike the cone, is non-degenerate), not at the exported offset band `w ∈ [1,2]`; the shell samples
`w = t.w.lo = 1`, so the two flank strips render as offset cylinder faces meeting along `L` at the `w = 0`
boundary of the certified band rather than as coincident edges at the sampled offset. The planned "rigid
rotation about the crease line, same exact segment" is thus replaced by this offset-axis construction, which
also carries a cosmetic **2:1 ruling-speed overhang** (`|r| = 2` at σ = 0 vs `1` at σ = 1; equalising needs
the irrational station σ = √2 − 1) — so the two crease edges share the *line* `L`, not the same extent. The
`the_fold_is_a_physical_shared_crease_right_angle` test asserts exactly this (both edges on `L`, `n_A·n_B = 0`,
parallel rulings off a nonzero pedal). *Slice 2 turns this prose note into a CI-enforced fact:* the OCC
differential oracle (below) asserts `free_edges > 0` ∧ `closed == false` on the exported band while the
internal certificate stays manifold — the overhang, made a differential expectation rather than a footnote.
(2) The Ledge cap's 2D outline is still the CAP-IN-D24 **licensing square** (now lifted isometrically), not a
real projected flank cut; the `v_boundary()`-guided cap + the external-kernel differential oracle is the next
M-D slice, now unblocked by the real crease.

### Milestone D (slice 2) acceptance criteria (the OpenCASCADE differential oracle)

*Authored before implementation, per the rule above.* Slice 2 picks up **thread 2's oracle half**: wire
OpenCASCADE's `BRepCheck` as a **differential oracle** compared against the internal verdict, and make
`export` **consume** the certified `v_boundary()` / `pinches()` (read-only) rather than ignore them. The
geometry-changing watertight **V_∂-guided seam** — an indexed-shell FFI channel so OCCT welds by identity
not float tolerance, and a `SewInput` derived from the emitted geometry — is deferred to **slice 3**: a
geometrically-coincident seam does not exist in the slice-1 fixture at the sampled band (the 2:1
ruling-speed overhang above means the two crease edges share the *line* `L` but not the same extent), so
building one now would fabricate topology the "oracle ∧ audit" doctrine forbids.

- **The oracle, compared not trusted — met when:** a strings-only `occt_shell_audit` (in the existing
  `export` `step`-feature `cxx` shim, alongside `occt_write_shell`) sews the *same* triangle soup and
  reports **extended** `BRepCheck` facts beyond today's bare `IsValid()` — free-edge count (edges incident
  to exactly one face, via `TopExp::MapShapesAndAncestors`), non-manifold-edge count (≥3 incident faces),
  shell closedness (`BRep_Tool::IsClosed`), and the analyzer's own validity — as a typed `ShellAudit`. A
  test-only `export::differential` harness (`#[cfg(all(test, feature = "step"))]`, mirroring
  `difftest::differential` for CGAL) then **compares** it against the internal verdict: the agreement
  conjuncts (`closure_valid → Verified`; internal `pinches().len() == 0` ⟺ OCC `nonmanifold_edges == 0`;
  OCC `IsValid()`) are asserted equal, and the **documented divergence** — OCC `free_edges > 0` ∧
  `closed == false` while the internal certificate says manifold — is asserted as *expected*, mirroring
  `difftest`'s `boolean_xor_pinch_documented`. The divergence's two causes are recorded in the assertion's
  doc comment: (i) the 2:1 overhang leaves the sampled-offset flank edges collinear-but-not-coextensive,
  and (ii) the fixture's `SewInput` is hand-authored and *decoupled* from the emitted triangles. The
  external kernel is thus **oracle ∧ audit, never oracle-instead-of-audit** (§8.2:332; spec "no kernel
  CSG" P5:16/§11:470/STEP:464; `implementation-plan-v1.md:75`) — it *surfaces* the slice-1 scope note as a
  CI-enforced fact, never overturns the certificate.
- **Certificate consumption — met when:** `export` reads the certified `v_boundary()` / `pinches()` off
  `valid.cap` (the `CapOut` carried on `CapWitness::Ledge`) — in the comparison layer (the internal-verdict
  summary), and in the **emitted path** by gating the Ledge `cap_tris` fan in `shell_from_closure` on
  `valid.cap.pinches().is_empty()` (a real certificate precondition before a cap face is emitted). No
  fabricated seam: consumption is read-only via existing accessors; the geometry-derived `SewInput` and
  indexed shell are slice 3.
- **CI coverage — met when:** the `export` `step`-gated suite runs **in CI** (a dedicated
  `nix develop --features step` leg — `cargo nextest run -p export --features step` + the `step` doctests),
  mirroring the CGAL oracle leg. This closes a gap the default `--workspace` legs leave open (the `step`
  module compiles out without the feature), and retroactively covers slice 1's `one_joint_*` STEP tests.

**Generality (hard gate) — met when:** the oracle, the audit wrapper, and all certificate consumption live
**only** in `export` (the non-certified float/FFI tier); no certified crate (`certify-core`, `arrange2d`,
`closure`, `sew`) changes, and no device constant is added — the comparison reads existing accessors.

**Documentation (a merge gate) — met when:** the new public surface (`ShellAudit`, `audit_shell`, the
bridge fn) is documented usage-first with `-W missing_docs = 0`, and the slice-1 §8 scope note (1) is
updated to point at this slice's oracle as its resolution.

**Deferred to slice 3 / later** (documented, not dropped): the watertight **V_∂-guided seam** — sampling
the `w = 0` crease loop (or an explicit shared crease edge), an **indexed-shell FFI channel** (shared
vertices + edge identity so OCCT welds by identity not float tolerance), and a `SewInput` **derived from
the emitted geometry** via the untrusted constructors `records_from_miter_ledger` / `arrangement_bits` /
`check_vertex_link` (already present in the `sew` crate); the STEP-reloaded (round-trip) audit variant; the
petal atlas / multi-joint assembly (M-D later); the petal cone-flank joint (blocked spec §13);
`VALID_material` / FRESH / `develop` (→ M-E).

**Status: slice 2 met.** D2.0 (`c5800e8`): this section + dispositions in `docs/engineering-log.md`. D2.1
(`1eb404a`): `occt_shell_audit` in the `cxx` shim (sewing loop shared with `occt_write_shell`) reporting the
extended facts as a typed `ShellAudit`, plus the `audit_shell<B>` wrapper — GO, the `TopExp` / `BRep_Tool` /
`TopTools` headers link with the existing `TKBRep` toolkit (no toolkit added); the CI `--features step` leg
landed here. D2.2 (`9d59418`): the `export::differential` harness — for **both** LEDGE and MITER, the
agreement conjuncts (`Verified`; internal `pinches().len() == 0` ∧ OCC `nonmanifold_edges == 0`; OCC
`IsValid()`) hold and the documented overhang divergence (`free_edges > 0` ∧ `closed == false`, measured
`free = 38`/`36`) is asserted as expected — plus the emitted-path gate on `pinches().is_empty()` in
`shell_from_closure`. D2.3: this status + the `vv-matrix` row + `-W missing_docs = 0` on the new surface. The
watertight `V_∂`-guided seam remains slice 3 (below).

---

### Milestone D (slice 3) acceptance criteria (exact ruled-surface STEP emission — certified-seam, honest-open)

*Authored before implementation, per the rule above.* Slice 3 delivers the **M8 STEP body as exact
rational surfaces**, not triangles. `spec §10:464` mandates it — "face surfaces exact; … sidewalls
exactly ruled …; closure patches … as rational patches with IDEALIZED flags; … no kernel CSG" — and
`§11:470` makes discrete meshes a **non-peer** export. Today's only export path is the triangle soup
(`shell.rs` samples `chart.surface` on a σ-grid at fixed `w`, fan-triangulates the D24 licensing
square; `occt_shim.cc` sews planar triangle-polygons by float tolerance). That soup is a **stopgap**,
and it is what manufactured slice 1's "2:1 overhang": sampling the *untrimmed* `μ∈[−1,1]` rectangle
never applies the certified plane trim, so the band is unavoidably open. **This supersedes the
slice-2 deferral's "indexed-shell / vertex-welding" framing** (`:984`) — the watertight seam done
right is two exact ruled faces sharing an exact **edge**, watertight by construction, not vertices
merged by tolerance.

The exact object already exists internally: `Chart::surface(μ,w) = c(σ)+μ·r(σ)+w·n(σ)` is a
`Vec3Rat` — an exact rational ruled surface. The gap is the emission path. The spec's own construction
dissolves the overhang: emit each flank as an **exact ruled face trimmed by the exact bisector plane
Π** — "STEP receives the flank face trimmed by the exact plane — a planar trim of a ruled face,
kernel-native, no boolean" (`§5.3`). Where the two trims coincide (**MITER-FIT**) they share the cut
edge; where they don't, the difference is a first-class **exposed planar ledge** (`face_A △ face_B`,
"a valid boundary step, not a hole" — **LEDGE-DOM**). *The seam SEW certifies is the cut in the cap
plane Π — not the fold crease, which is internal and coincides only at `w=0`; MITER-FIT is the
certificate that A's and B's Π-cut lines coincide.*

**Scope: certified-seam, honest-open.** The certificate is **joint-local** — SEW-EDGES/SEW-LINK
certify only the cap-to-flank and flank-to-flank seam edges and the `V_∂` links; CAP-OUT-LINK
certifies the cap region. Nothing certifies the substrate's outer boundary: `spec P1:12` makes a
single joint a slice of an atlas ("joint closures are export-layer solidization, not device
material"), and the closing sidewalls are "ruled **over anchors**" (`:192`/`:464`) — anchored to the
flat pattern's outer contour, machinery that does not exist and that `one_joint()` has no contour to
feed. So slice 3 emits **only the certificate-backed exact faces**, shares Π-seam edges **by
identity**, and leaves the substrate boundary **honestly open** (free edges there, annotated). No
fabricated closure face — a forced `closed == true` would be oracle-instead-of-audit.

**Representation: Strategy B** — emit the exact rational-Bézier **boundary curves** and let OCCT build
the ruled/linear-extrusion surface between them; the watertight object is the shared 1D edge we
control directly. Exactness through the FFI is a wash against emitting a full rational *patch* (both
cast to `f64` at the OCCT boundary), so the full-patch path (Strategy A) is the later generalization
for varying-ruling (cone) flanks. Order: **MITER first** (shared Π-cut edge, empty ledge — the
smallest watertight-by-construction unit), then **LEDGE** (cap as the exact `v_boundary` arrangement
face).

- **Exact-curve primitive + B-rep IR — met when:** an always-compiled, float-free
  `export::bezier` converts a `Vec3Rat`-in-σ into exact rational-Bézier poles + weights
  (monomial→Bernstein, exact `Rat`/`Surd`), reproducing known control nets exactly; and an
  `export::brep` exact IR (a shared vertex table, a shared **edge** table with identity = index —
  `Line` | `RationalBezier` — and faces `Plane` | `LinearExtrusion` carrying a `(edge_id, reversed)`
  wire) round-trips a hand-built two-face shell. `Surd`/`Rat` only, like `shell.rs`.
- **MITER exact ruled flanks, shared Π-cut edge — met when:** `brep_from_closure` (`CapWitness::Miter`)
  builds each flank as an exact ruled face trimmed to Π from the certified cut data (`ruling_cut_ends` /
  `segment_cut_ends` / `CutEnds`, `PiFrame`, `trim::bisector`, `chart.surface`), with the **single
  shared** Π-cut edge referenced by both flanks; a new `cxx` surface channel (`occt_write_brep` /
  `occt_brep_audit`) builds `Geom_BSplineCurve` (rational) / `gp_Lin` edges from a shared vertex+edge
  table and `Geom_SurfaceOfLinearExtrusion` faces via `BRepBuilderAPI_MakeEdge/MakeWire/MakeFace`,
  assembled with `BRep_Builder` over the **shared edges** (**no `Sewing`**), reusing the existing
  `STEPControl_Writer` + `MapShapesAndAncestors` audit; the single exact→`f64` cast lives in
  `step::brep_to_buffers` (mirroring `record_to_floats`). The oracle then reports the Π-cut seam as a
  **2-incidence** edge — `nonmanifold_edges == 0`, `brepcheck_valid`, and `free_edges` **strictly
  lower** than the triangle path — with free edges remaining **only** on the annotated-open substrate
  boundary.
- **LEDGE exact body = the certified flanks; exact cap deferred — met when:** `brep_from_closure`
  (`CapWitness::Ledge`) emits the **same two flank sheets** as the MITER arm and **no exact cap face**.
  *Reframe (locked with the user):* the only LEDGE cap outline available to `export` is the CAP-IN-D24
  **licensing square** — a placeholder, not the real `V_∂`-projected cut — and its crease edge overlaps
  the certified A+B seam `M`, so no certificate backs a flank↔cap seam. Emitting a cap face anyway is a
  three-way dead end proven against OCCT for this fixture: sharing the crease **edge** makes `M`
  3-incident → **non-manifold**; sharing only the crease **vertex** is a cone-point junction →
  `BRepCheck`-invalid; sharing **nothing** in one shell is a disconnected shell → `BRepCheck`-invalid
  (`BRepCheck_NotConnected`). So a single 3-face shell for this fixture *cannot* be `brepcheck_valid` —
  a topological fact, not a bug. Rather than fabricate a seam (oracle-instead-of-audit), the exact body
  emits only certificate-backed geometry (the flanks) and **defers the exact cap to the `V_∂` real-cut
  slice**; the cap survives only in the `§11` mesh (triangle) path. The oracle then reports the LEDGE
  body identically to MITER: **two** faces, one 2-incidence crease edge, `nonmanifold_edges == 0`,
  `brepcheck_valid`, and the file reloads. (See `docs/engineering-log.md` Findings for the three-way box.)
- **Differential flip + STEP-body routing + mesh retention — met when:** `export::differential` audits
  **both** representations of each certified witness and compares each against the internal verdict: the
  **exact §10 body** (`brep_from_closure` → `audit_brep`) shows the certified crease seam `M` as a single
  **2-incidence** edge (watertight-by-identity) with `nonmanifold_edges == 0` and `brepcheck_valid`,
  while the **§11 mesh** (`shell_from_closure` → `audit_shell`) keeps the documented overhang divergence
  (`free_edges > 0`, `closed == false`) — and the exact body's `free_edges` is **strictly below** the
  mesh's (the crease is one shared edge, not two open boundaries). The watertight claim is **narrowly the
  crease seam**: under Option B there is no exact cap seam (deferred), so both witnesses assert the same
  crease-`M` seam and the exact body stays *honestly open* elsewhere (uncertified substrate boundary +
  overhang tips remain free — not a closed solid). The end-to-end corpus routes the **STEP body through
  `brep`** (`write_brep`, the §10 solid body) while **keeping** `write_shell` as the §11 mesh diagnostic;
  both reload clean through `BRepCheck` for each witness.

**Generality (hard gate) — met when:** all new machinery (`bezier`, `brep`, the surface FFI,
`brep_from_closure`) lives **only** in `export`; no certified crate (`certify-core`, `arrange2d`,
`closure`, `sew`, `geom`) changes, no device constant is added, and **flank type stays data** —
cylinder-vs-cone follows from `chart` fields (constant vs varying ruling direction), never a Rust
`match` on type. Consumption is read-only via existing accessors.

**Documentation (a merge gate) — met when:** the new public surface (`bezier`, `brep`, the bridge fns,
`brep_from_closure`, `write_brep` / `audit_brep`) is documented usage-first with `-W missing_docs = 0`,
and this section's status is set to "met".

**Deferred (documented, not dropped):** the **certified closed solid** — anchored outer contour →
ruled sidewalls carrying their own CAP-OUT/SEW-LINK coverage → whole-solid watertightness certified —
is effectively its own milestone (**atlas assembly**) and is the correct path to a genuinely closed
solid; the "exact closed slab by-construction" (emitting support-box sidewall/end faces to force
`closed == true`, real geometry but closedness uncertified away from the joint) is **explicitly
declined** this slice. Also: Strategy-A full rational *patch* emission (cone / varying-ruling flanks);
the geometry-derived `SewInput`; the STEP-reloaded round-trip audit variant; petal atlas / multi-joint;
the petal cone-flank joint (blocked spec §13); `VALID_material` / FRESH / `develop` (→ M-E).

**Status: met** on `milestone-d` (slice 3 complete). D3.1 (exact-curve primitive + B-rep IR), D3.2
(MITER exact ruled flanks sharing the crease seam + the surface FFI), D3.3 (LEDGE exact body = the
certified flanks, exact cap deferred to the `V_∂` real-cut slice), and D3.4 (differential harness
auditing both paths — watertight crease seam on the exact body, retained overhang on the mesh — with
the STEP body routed through `brep` and the mesh kept as the §11 diagnostic) are implemented and
gate-green. **Scope honestly recorded:** the exact body is *certified-seam, honest-open* — only the
fold crease `M` is watertight-by-identity; the substrate outer boundary stays open (atlas-level,
uncertified) and the exact LEDGE cap is deferred to the `V_∂` real-cut slice. A genuinely closed solid
is atlas assembly (Deferred, above).

### Milestone D (slice 4) acceptance criteria (atlas assembly → the certified closed solid)

*Authored before implementation, per the rule above.* Slice 4 builds the **atlas-assembly** layer
slice 3 deferred: it turns whole-solid closedness from an **oracle verdict into an earned
certificate**. Today closedness is decided only by OpenCASCADE `BRepCheck` — an *oracle, not the
certificate* (`spec:332` "oracle ∧ audit, never oracle-instead"; §8.2 above). The spec has **no
predicate certifying whole-solid closedness**: `VALID_solid-closure` (`spec:439`) is only
`VALID_complement ∧ ⋀_j CLOSURE_VALID(j)`, a conjunction of *joint-local* facts. The docs pre-name
the missing layer ("ruled sidewalls carrying their own CAP-OUT/SEW-LINK coverage → whole-solid
watertightness certified", above) but flag it unbuilt. Slice 4's spine is a **new proven checker in
`certify-core`** — the assembly-scale analogue of the frontier theorem in `CapOut.lean:25-30`
("CAP-OUT-LINK at every vertex ⇒ 2-manifold-with-boundary").

**Two doctrines govern (unchanged, load-bearing).** *Incidence, not proximity* (`spec:192`, "solid
boundaries consume incidence… proximity is never attachment"): faces meet along a **shared exact edge
id**, never a float tolerance (the `brep` IR's watertight-by-identity property). *Earned, not oracle*
(`spec:332`): a forced `closed == true` is illegitimate; closedness is proven **internally** by the
checker, and OCCT only **corroborates** (differential oracle).

**Single-flank first (the geometry forces it).** M-D D.1 *proves* the two flanks' crease coincides
**only at the neutral surface `w = 0`** (a shared-crease dihedral is impossible with constant-h charts
off `w = 0`; the "2:1 overhang" is the residue). So a **two-flank** watertight slab is genuinely
obstructed — the outer (`w = t`) crease rulings diverge, and gluing the two boxes yields a non-manifold
edge, not a closed solid — while a **single-flank** closed slab (a bent box: top `w = 0` + bottom
`w = t` + four ruled sidewalls) **is** an exact, genuine closed 2-manifold. The honest *first* certified
closed solid is therefore the single-flank slab; the two-flank union confronts the `w=0`-only
obstruction as its own phase (D4.2). The slab's footprint is the **support box** (σ-support × μ-range ×
`w ∈ [0,t]`), a legitimate free-boundary contour (`spec:151`, "free boundary covers material with
rational conservative margin") — so the first closed solid needs **no** authored-flat-content / anchor
/ multi-joint machinery (those enter at D4.3/D4.4). The "exact closed slab by-construction" M-D
*declined* (closedness uncertified away from the joint) becomes legitimate here precisely because D4.1
now supplies the missing certificate.

The certificate itself: a shell is a combinatorial 2-complex `(V, E, F)` (edges = endpoint-vertex-id
pairs, faces = closed wires of half-edges `(edge_id, reversed)`). It is a **closed oriented
2-manifold** iff (1) all ids in range; (2) every wire closes end→start (the certify-core analogue of
`Brep::wire_is_closed`); (3) **∂² = 0** — every edge used exactly twice, once forward once reversed
(no free ⇒ closed; no ≥3 ⇒ manifold edges; opposite orientation ⇒ orientable); and (4) every
**vertex link is a single cycle** — the incident darts, walked by the rotation-system permutation
(face-corner successor ∘ edge involution), form one orbit (the 3-D analogue of `classify_link`'s
single-run test; a vertex-pinched pseudomanifold passes 1–3, fails 4). Checks 1–4 are pure, total,
`no_std`, panic-free, index-arrays-only — the `arrange.rs` mold.

- **D4.1 — closed-shell certificate + the certified single-flank closed slab — met when:** a new
  `certify_core::shell::closed_shell(...) → Verdict<ClosedShell, ClosedShellFault, ()>` implements
  checks 1–4 over flat index arrays (no coordinates), accepts a hand-built cube/tetrahedron, and
  refutes an open box, a 3-incidence (non-manifold) edge, a flipped-orientation edge, and a **vertex
  pinch** (two tetrahedra glued at one vertex — the case that passes ∂²=0 yet fails the vertex link,
  the test that earns check 4); a `closed_shell_sound` Kani harness proves acceptance **iff** an
  independent reference closed-2-manifold predicate holds over **bounded** shells (mold of
  `link_ok_iff_no_pinch` / `occupancy_row_sound`; the unbounded proof is D4.6, a tracked Lean
  frontier — *not* claimed here, exactly as `link_iso_ok` ships Kani-N=4); an **additive**
  `certify_core::gate::valid_closed_solid` conjoins the existing `valid_solid_closure` verdict with
  `closed_shell` over the assembled shell (`valid_solid_closure` itself untouched); `export` emits one
  flank as a closed slab (`brep_slab_from_closure`: top + bottom ruled sheets + four `LinearExtrusion`
  sidewalls over the support box, sharing edges by identity → 8 verts / 12 edges / 6 faces) with a
  `Brep::to_shell_certificate` bridge to the index arrays; and the end-to-end test asserts the slab
  bridges to `Verified(ClosedShell)` **and** the OCCT oracle *corroborates* (`closed == true`,
  `brepcheck_valid`, `free_edges == 0`, `nonmanifold == 0`). The existing honest-open flank-sheet body
  + §11 mesh tests are unchanged (a distinct representation).
- **D4.2 — two-flank joint closed solid — met when:** the two flank slabs are unioned watertight and
  the union certifies as `ClosedShell`, *or* the `w=0`-only crease obstruction is recorded as an
  honest documented blocker (the through-thickness miter Π-cut is the curved `w_trim(σ)` cut, likely
  gated on curve-carrier / `AlgReal` support, or a symmetric-fold sub-fixture).
- **D4.3 — substrate contour + anchors — met when:** a standalone closed-contour type (a closed D24
  loop of `Line`/`Arc` edges, independent of a joint's A/B/Crease scheme) and **anchor** function
  objects (`spec §4.6`: rational spline `â(t)` lifting an authored flat curve to chart coords) carry
  the ANCHOR certificate (`spec:372`), and sidewalls are ruled **over the authored contour**
  (`spec:194` exact-over-anchor), not just the support box.
- **D4.4 — multi-joint / atlas container — met when:** an `Atlas`/`Device` type feeds the
  `valid_solid_closure` fold over **>1** joint (the fold already supports it, `gate.rs:142`), with
  cross-joint seams, assembling a multi-joint certified closed solid.
- **D4.5 — sew sidewall coverage — met when:** `certify_core::sew` `EdgeProvenance` and
  `FaceGermSpecies` gain an **additive** sidewall/wall species + provenance + count (re-proven), so
  each sidewall seam carries its own SEW-EDGES/SEW-LINK coverage — deepening the per-seam certificate
  beyond D4.1's combinatorial whole-shell one.
- **D4.6 — Lean closed-2-manifold theorem (frontier, non-gating):** the unbounded "∂²=0 ∧
  vertex-link single-cycle ⇒ closed 2-manifold" proof (the `CapOut.lean:25-30` assembly analogue).
  Research, like the existing deep-theorem attempts; does not gate the milestone.

**Generality (hard gate) — met when:** the TCB edit is purely **additive** (a new `certify-core`
`shell` module + a new Kani harness + an additive `valid_closed_solid`); the existing proven checkers
`arrange.rs` / `sew.rs` / `boolean.rs` and `closure/src/valid.rs` are **not** modified in D4.1 (D4.5
touches `sew.rs` additively, later); no device constant is added; flank type stays **data**; and the
slab geometry lives only in `export`, consuming charts read-only.

**Documentation (a merge gate) — met when:** the new public surface (`closed_shell` / `ClosedShell` /
`ClosedShellFault`, `valid_closed_solid` / `ClosedSolid`, `brep_slab_from_closure`,
`Brep::to_shell_certificate`) is documented usage-first with `-D missing_docs`, and each phase's
status is set as it lands.

**Status: D4.0 + D4.1 met** on `milestone-d-atlas`. D4.0 authored this section + the `vv-matrix.md`
closed-shell row + the engineering-log disposition. **D4.1 delivers the first certified closed
solid:** `certify_core::shell::closed_shell` (the closed-oriented-2-manifold checker: shape → wires →
∂²=0 oriented edge census → vertex-link single-cycle) is Kani-proven (`closed_shell_sound` over the
tetrahedron's 2¹² orientations, `closed_shell_never_accepts_a_vertex_pinch`); the additive
`valid_closed_solid` gate conjoins it with the joint's `CLOSURE_VALID`; `export::brep_slab_from_closure`
emits the single flank as an exact closed slab (2 `Plane` σ-caps, 2 `LinearExtrusion` w-sheets, 2
`RationalPatch` μ-walls — the walls ruled along the rotating normal, emitted as rational
`Geom_BezierSurface` patches via the new `patches` FFI buffer), and `Brep::to_shell_certificate`
bridges its combinatorics to the checker. The e2e
`export::differential::the_flank_slab_is_a_certified_closed_solid` shows the slab is
`Verified(ClosedSolid)` internally **and** the OCCT oracle corroborates (`brepcheck_valid`,
`free_edges == 0`, `nonmanifold_edges == 0`). Two engineering notes recorded (see engineering-log): the
`Vec3Rat` degree must be **reduced** before the Bézier cast (`chart.surface`'s denominator-multiplying
adds inflate a μ-wall to degree ~18 → ±∞ poles → OCCT crash; reduced ~4), and the patch is a
`Geom_BezierSurface` (single-span, no knots) because `Geom_BSplineSurface` segfaulted. **Scope honestly
recorded:** this is the *single-flank* closed solid; the two-flank watertight union (the `w=0`-only
crease obstruction) is D4.2. D4.2–D4.6 **todo**, executed per-phase with a pause after each.

---

## 9. Sequencing

M0 grows Kani harnesses with the code (fast-path lattice verified before anything consumes it) and runs the §7 spike. `certify-core` splits out at M2 as the Lean target from birth. Stratum-weighted generators land with M3a (arrangement). The V&V matrix and `docs/proofs/ledger.md` start as stubs in the repo skeleton.
