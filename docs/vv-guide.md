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

## 9. Sequencing

M0 grows Kani harnesses with the code (fast-path lattice verified before anything consumes it) and runs the §7 spike. `certify-core` splits out at M2 as the Lean target from birth. Stratum-weighted generators land with M3a (arrangement). The V&V matrix and `docs/proofs/ledger.md` start as stubs in the repo skeleton.
