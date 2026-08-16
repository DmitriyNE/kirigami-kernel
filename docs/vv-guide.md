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
  gated on curve-carrier / `AlgReal` support, or a symmetric-fold sub-fixture). **Disposition
  (investigated):** the honest-blocker branch is confirmed — `one_joint()` is *fixture-obstructed*
  from closing (the 2:1 ruling-speed overhang needs the irrational station `σ = √2 − 1`, and a single
  joint's substrate boundary is honestly open), a property of the geometry, not a code gap. Rather
  than a symmetric demo fixture that dodges it, the machinery the closure genuinely needs — the
  curved Π-cut miter — becomes its own milestone (**Curved MITER-FIT**, below), per the standing
  "build the incomplete machinery, don't manufacture demo geometry" directive.
- **D4.3 — substrate free boundary + anchors — met when:** the hardcoded **rectangular** slab footprint
  is replaced by an **authored substrate free boundary** — the σ-band-with-rational-μ-splines form
  (`spec §3.4:151`: `μ⁻(σ), μ⁺(σ)` over `[σ_lo, σ_hi]`) — carrying the **exact** part of the ANCHOR
  certificate (`spec:372`: positive width + boundary regularity + σ̂-monotonicity, all Sturm), with the
  sidewalls ruled **over the authored boundary** (`spec:194` exact-over-anchor), not just the support box.
  **Scope split with the user (see the dedicated section below):** Stage 1 is this exact-over-anchor
  closed solid; the *transcendental* part of ANCHOR (the backward-error bound `sup|D(â)−g| ≤ ε` via the
  development map `D = γ + μ̂·ρ·e(ψ)`, and the DRC) is a **separate milestone (DEV / M-E)**, deferred.
  Detailed criteria + slice arc: **"Milestone D (slice D4.3) acceptance criteria"** below.
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

### Curved MITER-FIT acceptance criteria (the transverse-rational `φ_J` correspondence — L3 activation)

*Authored before implementation.* D4.2's investigation established that `one_joint()` **cannot** close
into a two-flank solid — the 2:1 ruling-speed overhang leaves free tips (equalizing needs the irrational
station `σ = √2 − 1`) and a single joint's substrate boundary is honestly open — a **fixture
obstruction, not a code gap**. Rather than manufacture a symmetric demo fixture that dodges it, this
milestone builds the deferred certificate machinery the closure genuinely needs: **curved MITER-FIT**,
the *transverse-rational* regime where two flanks' cut rulings are **rationally** (not affinely)
parametrized and their coincidence in the bisector plane Π is certified through the correspondence
`R(σ_A, σ_B) = 0` (`spec §5.3`; the pass `certify-core/src/miter.rs:31-32` and
`docs/closure-scoping.md:52-54` defer). It is the **first downstream wiring** of `lattice`'s
built-but-unused `resultant` / `resultant_bivariate` (and, in later slices, `AlgReal` and conic
carriers). Today's degree-1 `miter_fit` handles only the transverse-*affine* case (straight `CutEnds`).

**Earned, not oracle (OCCT never enters).** The certificate is a **resultant-conditioned divisibility
identity** — exact, float-free. On the correspondence `{R = 0}` (paired rulings share their crease-line
point, so *position identity is free*) the remaining clean-miter condition is **carrier identity
`D_A ∥ D_B`**, together with the extents `E_{A,±} = E_{B,π(±)}`, each vanishing on `{R = 0}` — certified
by an **exact cofactor** `X == R·Q` (the searcher supplies `Q`; the checker multiplies and compares).
`X = R·Q ⇒ X ≡ 0 on {R = 0}` is an exact implication; the only trusted lemma is resultant⇔common-root
(cited/Lean, **out of Kani** per §5 — `verify_common_factor` is "exactly the spec's resultant-conditioned
A-identity"). Watertightness does **not** hinge on MITER-FIT (`spec §5.3`: `trimmed-A ⊂ {b_J ≥ 0}`,
`trimmed-B ⊂ {b_J ≤ 0}` meet only in Π; a non-coincident cut is a valid exposed **ledge** → LEDGE-DOM) —
so this is the *clean-miter* certificate, the disjoint alternative to the LEDGE branch. *Event agreement
is insufficient* for the extents: the spec's stored counterexample `F_B ∩ L_σ = [0, 1+σ(1−σ)]` against
`F_A ∩ L_σ = [0, 1]` shares endpoints yet differs, so the condition must be an on-variety identity.

- **CM.1 — the transverse-rational MITER-FIT certificate — met when:** an **additive**
  `certify_core::miter::miter_fit_transverse(...)` (beside the untouched degree-1 `miter_fit`) takes
  rational cut rulings `F_i(σ,μ) = P_i(σ) + μ·D_i(σ)`, forms the correspondence `R(σ_A,σ_B)` from
  `ℓ_A = ℓ_B`, and certifies **carrier identity** (`X_carrier = (D_A × D_B) == R·Q_carrier`, exact) and
  **extent identities** (`E_{A,±} = E_{B,π(±)}` on `{R = 0}`, same cofactor pattern), plus `ℓ_i`
  monotonicity (Sturm) and the `ε_φ` order sign; it accepts a reflection-symmetric cone pair and refutes
  a planar-vs-cylindrical pair (fails at curvature order), the spec's stored extent counterexample, and
  a wrong cofactor; a Kani harness proves the cofactor-identity check sound over bounded-degree
  polynomials; `resultant` / `resultant_bivariate` / `verify_common_factor` are wired downstream for the
  first time; `certify-core` stays `no_std`.
- **CM.2 — conic carriers — SKIPPED (unsound as framed; deferred to the conic-*arrangement* L3).**
  Investigation found the premise wrong: `Carrier` is consumed **only** by CAP-IN-D24 → the LEDGE
  arrangement, and the clean-miter path (CM.1) uses straight rulings `F_i = P_i + μ·D_i`, not conic
  carriers. A conic is **not** D24 content (D24 = lines + circular arcs, spec §6); CAP-IN-D24 refusing it
  is *correct* — "the cone is **correctly turned away**" (`cap_in.rs:19`; "falsely, not vacuously" = a
  genuine-false predicate, not an erroneous refusal). The `closure::ledge` bridge already declines even a
  **Circle** (`Carrier::Circle → UnsupportedCarrier`), and `arrange2d` has no conic-curve arrangement — so
  a `Conic` that *passed* CAP-IN-D24 would license non-D24 content into a line/circle-only engine
  (unsound). Genuine conic support is the deferred conic-**arrangement** L3 (spec §484), the LEDGE branch,
  orthogonal to this milestone's clean-miter thrust.
- **CM.3 — `AlgReal` wiring — met when:** `lattice::AlgReal` is used downstream for the first time —
  `AlgReal::sign_of` (the sign of a polynomial at an algebraic number) + `AlgReal::count_roots_upto` (its
  distinct-root count in `(lo, α]`) certify a σ-rational gauge at an **algebraic** σ-event, and
  `certify_core::miter::strictly_monotone_upto_alg` uses them to certify a cut-face's `ℓ_i` strictly
  monotone up to an algebraic cut-exit σ (the cone case — `(−3+√17)/4` and the like). The transverse
  identities are support-independent, so only the monotonicity domain reaches for `AlgReal`.
- **CM.4 — closure searcher + minimal cone-flank sub-fixture — met when:** an untrusted `closure`-side
  constructor assembles the transverse MITER-FIT inputs (`ℓ_i`, `D_i`, `R`, the cofactors) from a
  cone-flank joint, and a minimal cone-pair sub-fixture certifies end-to-end through the curved MITER-FIT.
- **CM.5 — Lean frontier (non-gating):** the resultant⇔common-root and "divisibility ⇒ vanishes on
  `{R = 0}`" lemmas in Lean (the trusted foundation CM.1 cites). Research; does not gate.

**Generality (hard gate) — met when:** CM.1's TCB edit is purely **additive** (a new
`miter_fit_transverse` + types + harness beside the untouched degree-1 `miter_fit`); no device constant
is added; flank type stays **data**; `arrange2d` / `sew` / `boolean.rs` / `closure/src/valid.rs` and any
export/OCCT are **not** touched in CM.1.

**Documentation (a merge gate) — met when:** the new public surface is documented usage-first under
`-D missing_docs`, each slice's status set as it lands.

**Status: CM.0 + CM.1 met** on `curved-miter-fit`. CM.0 authored this section + the `vv-matrix.md`
row + the engineering-log D4.2-obstruction finding. **CM.1 delivers the transverse-rational
certificate:** `lattice::Biv` (a bivariate polynomial over ℚ — the first consumer of the
`resultant_bivariate` `Vec<Poly>` convention) + `certify_core::miter::miter_fit_transverse`, which
forms the correspondence `R(σ_A,σ_B)` from `ℓ_A = ℓ_B` (the checker builds it, never trusting a
supplied `R`), certifies the **carrier** and **extent** identities by the exact bivariate cofactor
equality `X == R·Q`, checks `ℓ_i` strict monotonicity by Sturm, and mints `ε_φ` from the two slope
signs via the Kani-proven `eps_from_slopes` — all **additive** beside the untouched degree-1
`miter_fit`. A reflection-symmetric *genuinely-rational* pair (`ℓ_i = 2σ/(1+σ)`) certifies; the
curvature-order (planar-vs-cylindrical), extent-counterexample (`[0,1]` vs `[0,1+σ(1−σ)]`),
parallel-regime, and wrong-cofactor cases are refuted (6 unit + doctest, plus `Biv` 4 unit + doctest).
**Earned, no OCCT:** the certificate is an exact bivariate polynomial identity; its
resultant⇔common-root soundness is cited/Lean (CM.5, where `verify_common_factor_sound` is already
axiom-clean). **CM.2 SKIPPED** (unsound as framed — a conic is non-D24 content CAP-IN-D24 correctly
refuses, and the clean-miter path uses straight rulings, not conic carriers; genuine conic support is the
deferred conic-arrangement L3, orthogonal here). **CM.3 wires `AlgReal` downstream for the first time:**
`AlgReal::sign_of` / `AlgReal::count_roots_upto` (a polynomial's sign at, and root count up to, an
algebraic σ) + `certify_core::miter::strictly_monotone_upto_alg` (the transverse monotonicity certificate
over an algebraic cut-face σ-bound — the cone case). Full gate green. **CM.4 delivers the searcher + the
branch-aware refinement, validated on the adversarial two-cone miter:** `closure::miter::transverse_cut_family`
projects a flank's cut-ruling family into Π (`P = c−(g0/g_w)·n`, `D = r−(g_mu/g_w)·n`, both rational in σ); and
because a cone's `ℓ` is degree-2 the correspondence `R` factors, so `certify_core::miter` gains a
`TransverseBranch` (`R_φ` + cofactor) — the checker verifies `R_φ·C == R` (by multiplication), `R_φ`
single-valued (deg-1 in σ_B), `R_φ` vanishing at the `ε_φ`-paired support corners (rejecting the spurious
branch), then `X == R_φ·Q` — plus `lattice::Biv::div_exact` (the searcher's cofactor tool). The **adversarial**
fixture (two cones over a shared base conic from different apexes — unit-circle tangent families `t=σ` vs `t=2σ`)
has `R = 2(σ_A−2σ_B)(σ_A+2σ_B)`, the full `R ∤ X` (CM.1's full-`R` check refuses, `CarrierMismatch`), and the
branch `R_φ = σ_A−2σ_B` certifies (`Verified`). **Honest scope:** the adversarial cut families are built from the
conic tangent-line geometry directly; the searcher-from-`Chart` link for the adversarial *pair* is unbuilt (the
arbitrary-apex-cone chart inverse problem), though the single-cone searcher is validated separately. Full gate green
(nextest ws 381/381, export/step 37/37 + doctests, `-D missing_docs`, `xtask lint`, no_std thumbv7em).

**The transverse-MITER-FIT milestone is complete** on `curved-miter-fit` (CM.0–CM.4 met): the certificate
covers all three regimes — cylinder (degree-1 `miter_fit`), symmetric cone (full-`R` `miter_fit_transverse`),
and adversarial cone (the `TransverseBranch` refinement) — each validated. **CM.5** (the Lean lemmas behind the
divisibility certificate) is non-gating research, deferred. The adversarial *pair*'s `Chart`-inverse derivation
(two different-apex cones over a shared conic *as charts*) is a documented follow-up; the certificate is validated
on the genuine conic cut-family geometry, and the single-cone searcher on a real `Chart`.

---

### Milestone D (slice D4.3) acceptance criteria (authored free boundary + anchors → exact-over-anchor closed solid)

*Authored before implementation.* D4.1 delivered the first certified closed solid, but its footprint is
the **hardcoded support box** (`σ-support × μ-range × w-range`, `brep_slab_from_closure`) — a rectangle,
not a real material outline. D4.2 (a two-flank solid) was fixture-obstructed and detoured into the
now-merged Curved MITER-FIT. D4.3 resumes the device thread: replace the rectangular footprint with an
**authored substrate free boundary**, so a solid closes over a genuine material outline — the atlas's
missing input (`one_joint()` "had no contour to feed").

**Scope decision (locked with the user): Stage 1 = the exact-over-anchor solid; the transcendental tier
is a separate milestone.** The spec's ANCHOR certificate (`spec:372`, "ANCHOR after DEV" `spec:402`) has
an **exact** part — regularity `|â′|² ≥ m`, σ̂-monotonicity (both Sturm) — and a **transcendental** part:
the backward-error bound `sup|D(â) − g| ≤ ε` via the development map `D = γ + μ̂·ρ·e(ψ)` (Tier-C `{ψ,γ,D}`,
`ψ = ∫ψ′` → arctan/log, `γ = ∫e(ψ)` a nested transcendental, `ρ = |n′|` a radical). Certifying that bound
needs a whole new rigorous-transcendental-enclosure tier — the **DEV / M-E milestone**, not a slice.
Per `spec:194` **exact-over-anchor**: everything downstream of the boundary `â` is *exact and mutually
incident* (top/bottom/sidewall are exact images of the one `â`, zero registration) — exactly the
shared-exact-edge incidence a closed solid needs (`spec:192`, "solid boundaries consume incidence, not
distance"). So the closed **solid** is fully tractable in exact ℚ; only *fidelity to an authored flat
drawing* needs DEV, and that is deferred.

**The tractable form (avoids the missing composition primitive).** The spec's substrate free boundary
(`spec §3.4:151`) is a **σ-band with rational μ-boundary splines** `μ⁻(σ), μ⁺(σ)` over `[σ_lo, σ_hi]` —
*not* an arbitrary `(σ(t), μ(t))` outline. In that form every boundary rail is `c(σ) + μ±(σ)·r(σ) + w·n(σ)`,
all functions of the **same** σ, so lifting is `Vec3Rat::scale` by a `RatFunc` — the direct generalization
of `brep_slab`'s constant-μ `scale_rat`, **no polynomial-composition primitive** (the repo lacks one;
`Chart::surface(μ, w)` takes a *scalar* μ). General `(σ(t), μ(t))` outlines (needing composition + an
N-edge cap subdivision) are the follow-on, alongside DEV.

**Two doctrines govern (unchanged).** *Incidence, not proximity* (`spec:192`): faces meet along a shared
exact edge id, never a float tolerance. *Earned, not oracle* (`spec:332`): closedness is proven internally
by `closed_shell`; OCCT only corroborates.

- **D4.3.0 — docs/criteria (this section).** vv-guide D4.3 acceptance criteria (the exact-over-anchor
  scope, the free-boundary form, the DEV deferral); the `vv-matrix.md` free-boundary/anchor row; the
  engineering-log disposition (the composition gap + the DEV/M-E deferral). Green via `cargo xtask lint`.
- **D4.3a — the free-boundary / anchor checker (`certify-core`, TCB) — met when:** a new
  `certify_core::free_boundary` module certifies an authored free boundary is a valid solid footprint,
  **composing the reused `certify1d` positivity foundations** (the `slab_s0` / `trim_local` mold — a bundle
  of `RegCert`/`EdgeRegCert`, each with searcher-supplied Sturm chains the checker re-verifies): (1)
  **positive width** `μ⁺(σ) − μ⁻(σ) ≥ m > 0` on the span (a `reg_q` instance); (2) **boundary regularity**
  `|â′|² ≥ m` for each lifted μ-rail (an `edge_reg` instance on the rail's squared speed); (3)
  **σ̂-monotonicity** `σ̂′ ≥ m > 0` (a `reg_q` instance on the anchor's σ-projection derivative — the
  composition-free slice of the general-`(σ(t),μ(t))` obligation, trivially `σ̂ = σ` for the σ-graph but
  implemented + refuted on a fold-back anchor so the check is real). Verdict
  `Verified(ValidFreeBoundary)` / `Refuted(FreeBoundaryFault::{EmptySupport, CrossedBounds, NonRegular,
  NonMonotone})`; pure, `no_std`, panic-free. Unit tests: a valid μ-band accepts; a crossed band
  (`μ⁺ ≤ μ⁻`), a non-regular (cusp/stall) rail, a fold-back anchor, and an empty support each refute; a
  forged Sturm chain is rejected (inherited from `reg_q`). `certify-core` still builds `thumbv7em`
  `no_std`. This is the **exact** part of `spec:372` ANCHOR; the transcendental `sup|D(â)−g| ≤ ε` + DRC
  are **out of scope** (DEV / M-E).
- **D4.3b — the free-boundary closed solid (`export`) + a fixture — met when:** a new
  `export::brep_build::brep_freeboundary_from_closure` generalizes `brep_slab_from_closure` — the same
  8-vertex / 12-edge / 6-face topology (so `closed_shell` applies unchanged), with the constant μ replaced
  by authored splines `μ⁻(σ), μ⁺(σ)`: μ-rails `c.add(&r.scale(&μ±)).add(&n.scale_rat(&w)).reduce()`
  (`Vec3Rat::scale` by the `RatFunc` boundary — the one line changed vs the slab's `scale_rat`; the
  bases reduced once so all four rails keep a shared denominator, the `ruled_from_rails` precondition);
  2 `Plane` σ-caps + 2 `RationalPatch` w-sheets + 2 `RationalPatch` μ-walls, shared edges by identity via
  the existing `Builder`; a fixture (`one_joint()`'s flank A with a **genuinely-varying** authored μ-band,
  e.g. a tapered band) bridges `Brep::to_shell_certificate → closed_shell == Verified(ClosedShell{8,12,6})`
  **and** the `export::differential` OCCT oracle corroborates (`brepcheck_valid`, `free_edges == 0`,
  `nonmanifold_edges == 0`). The D4.3a checker verifies that fixture's free boundary (`Verified`).

**Generality (hard gate) — met when:** the TCB edit is purely **additive** (a new `certify-core`
`free_boundary` module + the composed `reg_q`/`edge_reg` reuse — no edit to `certify1d`, `arrange.rs`,
`sew.rs`, `shell.rs`, `closed_shell`, or `closure/src/valid.rs`); no device constant is added; flank type
stays **data**; the new solid geometry lives only in `export`, consuming charts read-only.

**Documentation (a merge gate) — met when:** the new public surface (`free_boundary` / `FreeBoundaryCert`
/ `ValidFreeBoundary` / `FreeBoundaryFault`, `brep_freeboundary_from_closure`) is documented usage-first
under `-D missing_docs`, each slice's status set as it lands.

**Status: D4.3.0 + D4.3a + D4.3b met** on `d4.3` — the exact-over-anchor closed solid is delivered.
D4.3.0 authored this section, the `vv-matrix.md` free-boundary/anchor row, and the engineering-log
disposition (the `Chart::surface`-scalar-μ composition gap → the σ-band form; the DEV / M-E
transcendental deferral). **D4.3a** delivers the checker: `certify_core::free_boundary` certifies the
exact-ANCHOR obligation set for an authored σ-band boundary — positive width (`reg_q`), boundary
regularity of each lifted μ-rail (`edge_reg`), σ̂-monotonicity (`reg_q`), plus the `EmptySupport` /
`SpanMismatch` guards — as `Verified(ValidFreeBoundary)` / `Refuted(FreeBoundaryFault)`, composing the
reused (already-proven) positivity foundations so a forged Sturm chain is rejected there (7 unit +
doctest; pure, `no_std`, panic-free). **D4.3b** delivers the solid: `export::brep_build::brep_freeboundary_from_closure`
generalizes the slab (constant μ → authored `RatFunc` splines via `Vec3Rat::scale`; the reduced μ-bases
share one denominator, so all four σ-rails do — and every side face is an exact `RationalPatch`, the
curved-in-σ boundary defeating the slab's straight `LinearExtrusion` w-sheets) plus the `free_boundary_cert`
geometry→certificate searcher. Over `one_joint()`'s flank A with a **genuinely-varying** tapered band
(`μ⁻(σ) = −1 + σ`, `μ⁺(σ) = 1 − σ`): the searcher's cert is `Verified` by the D4.3a checker, the emitted
solid bridges `to_shell_certificate → closed_shell == Verified(ClosedShell{8,12,6})` (and `valid_closed_solid`),
and the OCCT oracle *corroborates* (`brepcheck_valid`, `free_edges == 0`, `nonmanifold_edges == 0`). Earned,
not oracle — the first certified closed solid over a real material outline rather than a box. Full gate green
(real exit codes): fmt, nextest ws 389/389, export/step 39/39, workspace doctests, clippy `-D warnings`
(+ `-p export --features step`), `-D missing_docs` (`certify-core`/`export`), `xtask lint`, `thumbv7em`
`no_std`. **Deferred:** general `(σ(t), μ(t))` outlines (need a composition primitive) and the transcendental
ANCHOR tier (**DEV / M-E**).

---

### Milestone E (DEV) acceptance criteria (certified development — the flat↔3D layer; **spike-first, GO-gated**)

*Authored before implementation.* DEV is the certified **flat↔3D development** map — the layer that
unrolls a 3D developable to its flat pattern and folds a flat pattern back to 3D. Per
`docs/implementation-plan-v1.md §6` it is **half the product**, not a fidelity rider: product direction
① (develop 3D→flat — generate the flat PCB outline) *unrolls* with it, and ② (fold flat ECAD→3D) *folds*
with it. Today it exists only as a **float diagnostic** (`export::mesh3d::develop_cone`, the flat↔rolled
Three.js morph); the product needs it **certified** — an exact rational error enclosure with a fab-grade
backward-error bound. This is a genuinely new tier (rigorous transcendental enclosure), so — like the M0
extraction spike (§7) — **it opens with a scoped spike that GO/no-go's the whole tier before we commit.**

**Why "exactness" is a representation property, not a shape property (the honest frame).** The kernel is
*exact arithmetic over rational inputs, where transcendental inputs are rational approximations whose
error DEV certifies.* The "42° cone" is already the rational quaternion `q=(9,4,4σ,9σ)` (`n·ẑ≡65/97`, an
approximation); the μ-boundaries are hand-picked; everything downstream is exact. The transcendental
surface the product actually needs — the **angular closure / full 2π wrap** (a finite rational σ sweeps a
*bounded* azimuth `< 2π`, so a single rational chart is a **gore**, never a closed cone), the **seam**
(the overlap at that closure), general placements at arbitrary angles, and flat-authored ECAD outlines —
all live in the *approximated-input* regime. DEV is the machinery that turns those approximations into
**certified** ones (a bounded backward error), and owns the closure/seam a rational chart cannot name.

**The transcendental core, isolated (why the cone is the right spike target).** The development map is
`D = γ + μ̂·ρ·e(ψ)`, `e(ψ)=(cos ψ, sin ψ)` (spec §3.2 / `spec:372`). For a **cone** (`h≡0` ⇒ pedal
`c≡0`, apex→flat origin) it collapses to a **polar map** `D = μ̂·ρ·e(ψ)`:
- **radius** `ρ = |n′| = √(normal_deriv_sq)` — a **surd** (√ of a rational), *already* representable by
  `lattice::Surd` (`a+b√d`); no new arithmetic;
- **angle** `ψ(σ) = ∫₀^σ ψ′` where `ψ′ = chart.psi_prime = det(n,n′,n″)/|n′|²` is a **rational function**
  of σ — so `ψ` is an **arctan/log of rationals** (the integral of a rational is elementary). **This is
  the sole genuinely-new transcendental**, and it is the arctan-class angular coordinate — nothing worse.

So DEV is *not* "certify arbitrary transcendentals": it is "certify `∫(rational)` = an arctan/log, with a
rational error bound," radius already handled. `export::mesh3d::develop_cone` (radius = apex distance,
angle = accumulated `acos` of successive unit rulings) is the **float ground-truth** the certified
enclosure is checked against.

- **DEV.0 — this GO-gate (docs).** This section + the engineering-log DEV thread. No code. Green via
  `xtask lint`.
- **DEV.1 — the spike (GO / no-go) — ✅ MET, decision GO** (`docs/spike-development-report.md`; crate
  `develop` — `develop::interval` + `develop::cone`; corroboration in
  `export … certified_flat_point_corroborates_develop_cone`). The cone development reduces to a **single
  `arctan` of a rational**: `ψ(σ) = 2 sinβ · arctan σ` (verified as an exact polynomial identity), radius
  `ρ = √(rational)`. Method **(a)** (closed-form arctan + alternating-series rational bounds) selected. On
  the device cone: certified backward error `≈ 1e-11`, corroborated against `mesh3d::develop_cone` to
  `≈ 1.5e-8`. Digit-growth in naive series composition is the one engineering wall → fixed-precision
  outward rounding for DEV.2 (report §5). Met when:
  1. a **certified rational enclosure** `ψ(σ) ∈ [ψ_lo, ψ_hi]` of `ψ(σ)=∫₀^σ chart.psi_prime` with
     rational endpoints and a rational width bound `ε_ψ` (the spike **selects the enclosure method** among:
     (a) closed-form arctan/log with certified rational bounds on those functions — attractive since the
     integrand is rational; (b) verified interval integration; (c) Taylor models — and records the choice
     + why, mold of the §7 spike report);
  2. combined with the exact surd radius `ρ·μ̂`, a **certified flat point** for a cone-gore sample that
     encloses `mesh3d::develop_cone`'s float value (agreement within `ε`), across the gore — the
     oracle∧audit check (float diagnostic *corroborates* the certified enclosure, never defines it);
  3. the **backward-error scaffold**: `sup|D(â)−g| ≤ ε` stated as a checker over the enclosure, and the
     **DRC** `ε < clearance/2` (`spec:402`) as its gate — verdict-typed (`Verified(ε)` /
     `Unresolved(width)`), never a float compared with a float;
  4. the **seam / closure** identified as the acceptance case: the spike states precisely how the
     bounded-azimuth gore relates to the 2π closure and where the seam's certified angular position lives
     (even if closing the full cone is a post-GO deliverable), so the "cone with a seam" is scoped, not
     hand-waved.
  **GO** = a converging, verdict-typed enclosure with a fab-plausible `ε_ψ` on the device cone. **No-go**
  = the enclosure doesn't converge / the method is intractable → record the wall and the alternative
  (the honest §7-spike outcome), *before* the tier is built.
- **DEV.2 — the certified development tier for the *closed-form* developable class (post-GO, planned).**
  DEV.1's foundation is already general (the `develop::interval` enclosures, `ρ=√(‖n′‖²)` surd, and
  `ψ=∫ψ′` arctan/log-class for any chart); DEV.2 broadens from the device cone to **every developable
  whose development is elementary** — cones at any placement (`ψ=∫P/Q` = a sum of arctans/logs), and
  cylinders (`ψ′≡0` ⇒ `e(ψ)` constant ⇒ `γ` elementary). Slices (each commits green; additive; the pure
  `certify_core` TCB and the crease/atlas layer untouched):
  - **DEV.2a — fixed-precision outward rounding** (retire the DEV.1 digit-growth wall, report §5): a
    pure-tier `Rat::floor`/`ceil` (+ Kani panic-freedom) and a `develop::interval` `round_out(bits)`
    applied inside the series, so certified endpoints stay bounded-digit at any budget. Rigorous —
    outward rounding only grows an enclosure. **Met when** a high-term-budget development stays
    bounded-digit *and* still brackets the truth (corroboration digit-bound assertion). **MET** —
    endpoints ≤ 19 digits at 40 terms, backward error `≈ 6e-12`, corroboration `1.5e-8` (report §5).
  - **DEV.2b — the general closed-form angle**: `angle_enclosure` computing `ψ=∫P/Q` via complete-the-square
    (degree-2 core: positive-definite `Q` → `(a/2A)·log((σ−p₀)²+q₀²) + ((ap₀+b)/Aq₀)·arctan((σ−p₀)/q₀)`,
    the surd `q₀=√(−disc)/2A` via `sqrt`; higher-degree over `lattice::AlgReal` flagged as an extension),
    `Verdict`-shaped (higher-degree / real-root / γ≠0 charts → a clean `Unresolved(AngleDefer)` pointing at
    the extension / DEV.3, never a silent `None`). Adds the `interval::log` enclosure (`atanh` series +
    power-of-two reduction + geometric tail bound), `interval::arctan_on` (interval argument), and
    `RatIv::recip_pos`. **Met when** it matches DEV.1 on the device cones and certifies a general-placement
    cone, float-corroborated. **MET** — reproduces `ψ=c·arctan σ` on `cone()`/`cone_alt()` across the gore,
    certifies a reparametrized cone `q(σ−1)` (`Q=σ²−2σ+2` ⇒ `(130/97)(arctan(σ−1)+π/4)`) that the canonical
    recognizer declines, and the `log` branch on `σ/(1+σ²)=½ln(1+σ²)`; all corroborated to `≈ 1e-9`.
  - **DEV.2c — the ANCHOR backward-error certificate (the T-part)**: `sup_t|D(â(t))−g(t)| ≤ ε` + DRC
    `ε < clearance/2` (`spec:192`) as an evidence-carrying certificate (`free_boundary` mold:
    `*Cert`/`Valid*`/`*Fault`, `Verdict`-typed) in `develop` — it needs the transcendental enclosures, so
    it lives shell-side while `certify_core`'s A-part (`free_boundary`) stays pure; the two **compose**
    into the full ANCHOR (`T,1D + A,1D`, `spec:372`). Introduces the authored target `g` + rational anchor
    spline `â` (new vs the spike) and a rigorous `sup_t` via interval-`t` subdivision. **Met when** a
    closed-form anchor certifies to `Verified(ε)` under a fab clearance (too-tight → `Unresolved`), the
    per-span `ε` bounds the `develop_cone` deviation, and it composes with `free_boundary`. **MET** —
    `develop::anchor` = `AnchorDevCert`→`anchor_dev` (subdivides `[t_lo,t_hi]`, encloses `D(â([a,b]))` via
    `ConeDevelopment::point_on` + the target `g([a,b])` via `eval_ratfunc_on`, bounds `√(Δx²+Δy²)`, takes
    the max `ε`; `Verified`/`Unresolved(ε)`/`Refuted(DegenerateSpan|PoleInEval)`) composed by `anchor` with
    the pure `free_boundary` A-part into `Verified((A,T))`. The anchor is a **general rational-`t`** curve
    `â(t)=(σ(t),μ̂(t))` riding the band's affine μ⁻ rail (no composition primitive — the checker evaluates
    `σ(t)`, never symbolically composes). Device-cone fixture: `ε` shrinks with `subdiv`, a generous
    clearance `Verified`s and a tight one is `Unresolved`, `ε` upper-bounds the float chord-sagitta, and a
    σ-span-mismatched anchor → `SpanMismatch`.
  - **DEV.2d — certified unroll (direction ①)**: develop the free-boundary μ-band
    (`export::brep_build::brep_freeboundary`) to a certified flat outline; corroborate vs `develop_cone`.
    **MET** — `develop::unroll::unroll_freeboundary` develops the band boundary loop into a flat
    **polyline** (`FlatOutline`: ordered `FlatBox` vertices) and certifies each **rail edge** within `ε` of
    the true continuous developed rail via the DEV.2c `anchor_dev` lift bound (the σ-caps are rulings → exact
    straight radials); whole-outline `ε = max`, DRC-gated. `Verified(FlatOutline)`/`Unresolved(ε)` (refine
    `segments`)/`Refuted(UnrollFault::{DegenerateSpan,PoleInEval})`. Device-cone fixture: `ε` shrinks with
    `segments`, generous clearance `Verified`s / tight `Unresolved`s, vertices enclose the development, and
    the assembled outline corroborates `develop_cone` to `<1e-5` (`export::mesh3d::unroll_outline_corroborates_develop_cone`).
  - **DEV.2e — certified fold-inversion (direction ②, *per-panel*)**: `D⁻¹` flat→`(σ,μ)` enclosure, then
    exact chart eval `C(σ,μ,w)`→3D. **The single-panel isometry only** — multi-panel **creases /
    fold-mates are the atlas** (D4.4) + `closure`/`sew` (spec §5.3 MONO; the reflection mate is already in
    M-D), *not* `develop`. **MET** — `develop::fold::fold_point` inverts the polar map: **angle→σ** by
    monotone bisection on the signed area `cos ψ·y − sin ψ·x = r·sin(θ−ψ)` (a non-dyadic 3/7 split, so a
    rational root is never hit exactly and the σ-enclosure refines) — never computing the transcendental
    `θ`; **radius→μ̂** as `|μ̂| = r/ρ(σ)`; **lift** the exact surface `C = c + μ̂·r⃗ + w·n` over the
    `(σ,μ̂)` enclosures → a 3D box. Certificate = the **round-trip** backward error (re-developing `(σ,μ̂)`
    reproduces the input flat point within `ε`), DRC-gated. Device-cone fixture: folding the forward image
    of `(σ₀,μ₀)` recovers both enclosures + `|C| = r`, `ε` shrinks with bisection iters, tight clearance
    `Unresolved`s, an out-of-gore angle → `OutOfGore`.
- **DEV.3 — the non-elementary frontier (own milestone, spike-first).** `γ = ∫e(ψ)` for a **curved
  directrix** (tangent-developables / arbitrary ruled developables) is *not* elementary → **verified
  interval integration** (the DEV.1-selected method (b), with its own GO gate). Also the full 2π angular
  closure + multi-gore seam, the two product pipelines end-to-end (intersect→outline→unroll;
  ECAD→fold→solid), and Lean/Kani for the transcendental enclosure tier. Named here because the product's
  substrates span all developable classes; authored when DEV.2 lands.

**Doctrine.** No float in a certificate: the enclosure's endpoints and `ε` are **rationals** (interval
arithmetic over ℚ), the float `develop_cone` only corroborates. Exact-over-rational-inputs: DEV certifies
the *approximation* error of transcendental inputs (angles, wraps, placements), it does not pretend they
are algebraic. Oracle ∧ audit: the diagnostic development is the oracle, the enclosure is the audit.

**Generality (hard gate).** DEV.1 is **additive** — a new spike crate/module (or a `develop`-crate spike)
+ its enclosure primitive; it does not touch `certify-core`'s existing checkers, `arrange2d`, `closure`,
or the exact `export` path. The transcendental enclosure lives behind its own boundary; the pure exact
tier stays float-free and untouched.

**Documentation (a merge gate).** The spike ships a short **report** (like `docs/spike-*-report.md`) —
the method chosen, the certified `ε_ψ` on the device cone, the float-corroboration numbers, and the GO /
no-go call — plus usage-first docs on any new public surface under `-D missing_docs`.

**Status: DEV.0 + DEV.1 met (DEV.1 decision GO, `docs/spike-development-report.md`); DEV.2 COMPLETE
(DEV.2a + DEV.2b + DEV.2c + DEV.2d + DEV.2e all met).** The certified development is demonstrated on the device cone (`develop` crate):
`ψ = 2 sinβ · arctan σ` closed form, rigorous rational enclosure, backward error `≈ 1e-11`,
float-corroborated to `≈ 1.5e-8`. **DEV.2** broadens to the whole closed-form developable class (cones at
any placement + cylinders) across the slices above: DEV.2a retired the digit-growth wall (bounded-digit
outward rounding), DEV.2b generalized the angle (`angle_enclosure`: `∫P/Q` complete-the-square, a
`Verdict`-shaped arctan/log enclosure certifying cones at *any* placement, not just the canonical
`c/(1+σ²)` fast path), DEV.2c built the ANCHOR T-part (`anchor_dev`: the uniform lift bound
`sup_t|D(â)−g|≤ε` + DRC, composed with the pure `free_boundary` A-part into the full ANCHOR), and DEV.2d
certified the **unroll** (direction ①: `unroll_freeboundary` develops the free-boundary band to a flat
polyline `FlatOutline`, each rail edge within `ε` of the true development, corroborated vs `develop_cone`),
and DEV.2e certified the **fold-inversion** (direction ②, per-panel: `fold_point` inverts `D⁻¹` flat→3D
with a round-trip backward-error certificate). **Both product directions are now certified per-panel.**
**DEV.3** owns the γ≠0 curved-directrix frontier (interval integration) + the 2π
closure + the end-to-end pipelines. **Creases / fold-mates / multi-panel assembly are the atlas (D4.4) +
`closure`/`sew`**, not `develop`. The exact 3D substrate (charts, intersections, watertight solids, STEP)
that DEV sits on is delivered through M-D.

### Flex-PCB acceptance roadmap (two stages) — full detail in `docs/roadmap-flex-pcb.md`

The remaining path to the product spine (the bidirectional multilayer flex-PCB, `docs/implementation-plan-v1.md §6`)
is organized around **two concrete end-to-end acceptance demos** that gate the milestones. Both are built
**fully general, without goal-specific hacks** — the certified backward-error bound (`anchor_dev`) makes
rational approximation the *designed* treatment of transcendental/algebraic geometry, fail-closed (a loose
approximation → `Unresolved`, never a wrong `Verified`), not a shortcut.

- **Stage 1 — cone-sector geometry, back-and-forth.** A ~300° (rational-approx) cone sector, cut by an
  offset-plane curve (exactly rational, `μ=d/(n·ruling)`) + a fitted cone∩cylinder curve → unroll → a square
  interior hole on the flat (exact `arrange2d` boolean) → fold back → SVG + two STEPs (input cut cone; folded
  panel with the hole as a real interior wire). Per-panel, single-layer. **This is DEV.3-α (the per-panel
  pipeline, on a wide/two-sided gore) + one exporter milestone** — interior-hole / arbitrary-trim STEP
  B-rep, a new slice **D4.7 / E-EXPORT** extending the deferred V_∂ real-cut. Gap ladder **G1–G7** with an
  artifact ladder A1 (SVG) → A2 (SVG+hole) → A3 (folded mesh) → A4 (STEP I) → A5 (STEP II). Two conscious
  general-over-shortcut choices: certified `fold_outline` **per-edge** (not per-vertex), and the hole via
  **explicit (σ,μ) pcurves** (not re-ruling).
- **Stage 2 — cone + overlap seam.** Close the rolled cone with a certified **BONDED lap seam** (the
  original device's "lap seam", `implementation-plan-v1.md:53`). Single-layer. Two independent hard
  frontiers: **DEV.3-β** = the transcendental full-2π closure (a rational chart is a gore <2π; the seam
  lives at σ→±∞; chart-graph cycle = deferred [D11]) and **spec §14 (BONDED)** = the lap certificate (SEP ≡
  bond gap `g`, SLAB one-sided, two-to-one normal projection). S3 depends on S2. Everything bonded rides on
  the **seam-ramp subdivision certificate** (`docs/paper.md`) — the one place the method broadens from
  closed-form to certified interval subdivision, hence spike-first / GO-gated. Gaps **S2 + S3**.
- **Beyond Stage 2 (the full multilayer flex-PCB):** multilayer stackup (`w`-band stack + `z_N` strain
  budgets, new laminate slot), multi-panel atlas + reflection-mate constructor (D4.4), complex authored ECAD
  boundary with cutouts (D4.3 + §14 curves).

Sequence: Phase 1 = Stage 1 (start G1, the interval-trig range reduction) · Phase 2 = DEV.3-β closure ·
Phase 3 = §14 BONDED seam · then the beyond-Stage-2 extensions. **Status: Stage 1 COMPLETE + merged to
`main` (`480771a`) — the certified cut→unroll→hole→fold→STEP cone gore + the G-C curved-rail STEP builder
(the xy-trimmed panel → certified STEP I/II) + the ~296° widen; the σ↔azimuth law pinned exactly
`φ = 2·arctan σ`. Stage 2 authored (S2.0, below); S2 + S3 pending.**

### Stage 2 (BONDED lap seam) acceptance criteria (S2 full-2π closure + S3 §14 BONDED — spike-first, GO-gated)

*Authored before implementation (S2.0).* Stage 2 is the second end-to-end acceptance demo: take the
Stage-1 rolled cone gore and **close it with a certified BONDED lap seam** — the two radial gore edges
lap and bond across the full 2π wrap, single-layer (the device's original lap seam,
`implementation-plan-v1.md:53`). Two gaps, **S3 ⊳ S2**: **S2** the full-2π closure (slot DEV.3-β) and
**S3** the §14 BONDED lap certificate. Spike-first / GO-gated — the seam is the one place the method
broadens from closed-form to certified interval subdivision.

**Two framing facts settled at S2.0 (they scope the whole milestone).**

1. **The seam is single-chart *representable*; re-centering only *conditions* the certificate.**
   `σ ∈ ℝ ↔ φ₃D = 2·arctan σ ∈ (−π,π)` already covers the whole cone except the back ruling — the seam
   *is* that ruling, at `σ=±∞`. Re-centering adds no representational power; it moves the seam off the
   coordinate singularity so a *subdivision* certificate can **converge** there (at `σ=±∞` the enclosure
   widths `µ̂ ∝ 1+σ²` are unbounded / non-refinable). The re-center is a half-turn about the axis
   (`φ→φ+π`) = the exact rational Möbius `σ'=−1/σ`, giving `q'(σ')=(9σ',4σ',−4,−9)` — still a degree-1
   rational cone (the quaternion→rotation map is scale-invariant, `R(λq)=R(q)`). **Representation ≠
   certification-conditioning.**

2. **BONDED requires γ≠0 by design — the *mild* γ≠0.** The lap flap climbs Δ≈0.25 mm to seat on the
   other edge: the §8 degree-1 constant-slope ramp picks the family where **`n` rides the cone's Gauss
   circle (shared `q`) while the support `h` ramps** — a nonzero-support (γ≠0) developable. But `ψ` is
   `h`-independent (`geom::chart::psi_prime` reads only `n,n′,n″,|n′|²`, all from `q`), so `ψ = c·arctan σ`
   stays **closed-form**. This is **not** DEV.3's interval-integrated curved directrix (that trigger is a
   complex `q` making `ψ=∫ψ′` non-elementary — a genuinely deferred, different gap).

**Load-bearing consequence: the BONDED certificate is rational and lives in 3D.** `Chart::surface =
c + µr + wn` is rational in `(σ,µ,w)` for *every* developable (the pedal `c` already carries `h≠0`), so
the certifier is chart-agnostic and the transcendental `ψ` is confined to the *flat* development
(emission-only, refinable). The invariant decomposition (spec §7 face identity, §8 ramp, §11 table):
- **SEP** — separation ≡ bond gap `g` by the face identity `h_A + w_{A,face} + g = h_B + w_{B,face}`
  (§7); "compares two ring scalars" → **exact rational**.
- **SLAB** — ramp regularity `R₁ = s·tanβ + (h+h″) > 0`, one-sided (§8/§11 SLAB-S0) → **rational,
  single-span-Sturm**; reuse `certify1d::{reg_q, slab_s0}`.
- **SHEAR** (roadmap "MATCH") — Tier-1 identification `J = rigid ∘ ruling-shear`, `κ_g ≡ k`, `k`
  separated from 0, `Δ ≡ Δ₀`, `δ = −Δ₀/k` (§7); constancy = a rational identity, sign = one sample,
  separation = one ring compare → **exact rational**.
- **CLEAR** — the one genuinely new piece: pair/self clearance between the two lapping **rational**
  sheets over the thin Δφ≈60° ramp box, by **adaptive interval subdivision** of the 3D distance (via
  `interval::eval_ratfunc_on`); `Unresolved(ε)` fail-closed, mirror `develop::cut::cut_fit`. All prior
  subdivision is fixed-count / equal-width — this adaptive loop is the new proof technique, and it is
  **rational**, not transcendental (the roadmap's "single highest-risk unknown", de-risked here).
- **two-to-one normal projection** — the structural fact over the overlap (§3); the §7 correspondence
  lemma reduces SEP/CLEAR to a matched-ruling (σ↔σ) comparison.

**General/specific split (both general — the user's decision).** Build **both** the BONDED certifier
(chart-agnostic, above) **and** the `SeamFrame` reduction (a `Verdict`-shaped abstraction bringing a
seam into a finite regular parameter box + the exact transition). `SeamFrame` is pinned by two real
instances now — the cone body (`σ=±∞` → exact Möbius `σ'=−1/σ`) and the γ≠0 ramp tail (already at finite
σ → near-identity); the DEV.3 curved directrix is the deferred third instance the interface must not
preclude. **Specific = inputs only** (`q=(9,4,4σ,9σ)`, `c=130/97`, the ramp `h`, `σ'=−1/σ`); no checker
names "cone" or "−1/σ".

**Geometry & scope locks.** A lap is **doubled material** near the seam → the demo emits **two certified
solids (cone body + lap flap) + a certified BONDED interface** (§6.2, "two-piece assemblies bond through
laps"), *not* one self-touching OCCT solid. The **[D11] chart-graph cycle is out of scope** — a lap bond
is an assembly declaration with gap `g`, not a rigid metric cycle-closure. Single-layer; multilayer
stackup / multi-panel atlas / complex authored ECAD boundary are **beyond Stage 2**.

Slices (each commits green; additive; the pure `certify-core` TCB grows only by the new BONDED module):

- **S2.0 — this GO-gate (docs).** This section + the `vv-matrix.md` Stage-2 rows + the engineering-log
  S2 thread. No code. Green via `xtask lint`.
- **S2 (DEV.3-β) — full-2π closure / the shared seam frame.** **Met when:** (1) a general `SeamFrame`
  reduction (`Verdict`-shaped) brings a seam to a finite regular parameter box + an *exact* transition;
  (2) the re-centered cone view `cone_seam() = q'=(9σ',4σ',−4,−9)` is certified an exact reparametrization
  of `fixtures::cone()` (the transition `σ'=−1/σ` verified float-free); (3) the seam-adjacent cone body
  develops in the re-centered view at finite σ' (reusing `ConeDevelopment`/`unroll` unchanged — γ=0 there),
  the two radial edges expressed as rails in one shared finite frame; (4) float-corroborated vs
  `mesh3d::develop_cone` across the seam neighborhood (oracle ∧ audit). **GO** = the seam edges live at
  finite, well-conditioned σ' with a converging development. **No-go** = the re-centered development
  doesn't converge / the transition isn't exact → record the wall + alternative before building S3.
- **S3 (§14 BONDED) — the bonded seam certificate + demo.** **Met when:** (1) the γ≠0 support-ramp is a
  `Chart::new(cone_q, ramp_h)` fixture (representable today; the certificate reads `Chart::surface`
  directly — the flat-γ / pedal-rejection lift is emission-only, done *iff* the flat pattern is in demo
  scope); (2) a new `certify-core::bonded` module (own `Verdict`-shaped module, transcendental-free →
  the rational side of the future `develop` split) certifies **SEP** (≡ `g`, exact), **SLAB** (`R₁>0`,
  reuse `reg_q`/`slab_s0`), **SHEAR** (`δ=−Δ₀/k`, exact), and **CLEAR** (the new adaptive-subdivision
  clearance, mirror `cut_fit`), threaded by a `gate.rs`-style `valid_bonded_seam` via `conj`; a
  **refutation** case (too-small gap / interpenetrating ramp) is `Refuted`/`Unresolved`, never `Verified`;
  (3) the demo emits two certified solids (`brep_trim_solid` cone body + lap flap) + the certified BONDED
  interface, each corroborated by OCCT `audit_brep` (`brepcheck_valid`, `free_edges==0`). **GO** = the
  BONDED conjunction `Verified` on the device seam, OCCT-corroborated, refutation fires. **No-go** = the
  adaptive CLEAR subdivision doesn't converge over the Δφ≈60° box → record the wall.

**Doctrine.** No float in a certificate — SEP/SLAB/SHEAR exact rational, CLEAR a rational-interval `ε`;
the OCCT/mesh oracles corroborate, never certify. Fail-closed — a loose CLEAR → `Unresolved`, never a
wrong `Verified`. Oracle ∧ audit, never oracle-instead.

**Generality (hard gate).** The BONDED module is **additive** — a new `certify-core` module + the
`develop::seam_frame` reduction; it does not touch `arrange2d`, `closure`, the exact `export` path, or
the DEV transcendental core (except the optional, emission-only flat-γ layer in `develop::cone`). No new
`Verdict` variant (any extra sub-state is a domain enum lowered via the `EdgeReg::to_verdict` idiom). No
Kani/Lean TCB change unless a new *combinatorial* claim is introduced — flagged at that point if so.

**Documentation (a merge gate).** Usage-first docs on new public surface under `-D missing_docs`; a short
S2 spike report (`docs/spike-*.md` mold) recording the closure GO/no-go + the CLEAR-subdivision
convergence numbers on the device seam.

**Status: S2.0 + S2 met** (decision GO, `docs/spike-seam-closure-report.md`) — on `stage-2-seam`. S2.0
authored this section + the `vv-matrix.md` rows + engineering-log. **S2** brought the seam to a finite,
regular parameter, exactly and certified: `fixtures::devices::cone_seam` (the device cone re-centered by
the axis half-turn `σ=−1/σ'`, seam at `σ'=0`), the general `develop::seam_frame` reduction
(`seam_frame_exact` — the re-centering discharged as an exact rational identity, refutations fire), the
existing `ConeDevelopment` develops the seam ruling to the exact `(144/97,0)` unchanged, and the float
`develop_cone` corroborates the re-centered development to `≈1.5e-8` (backward error `≈6e-12`). The
seam-ramp frontier is **rational, not transcendental** — scoped to S3's CLEAR. **S3 (§14 BONDED) met
— Stage 2 COMPLETE.** The `develop::bonded` certifier: **SEP** (plateau separation ≡ gap `g`, exact §7
identity) · **SLAB-S0** (offset slab regular, `det J ≥ m > 0`, reuses `certify_core::reg_q` Sturm
positivity) · **SHEAR** (Tier-1 `J = rigid ∘ shear`, `κ_g = −65/72`, `δ = 18/65 ≈ 0.28 mm` — the paper's
number) · **CLEAR** (the novel **adaptive interval subdivision** of the true 3D lap-rail distance, sound
despite the §7 tangential shift — Verified in 18 nodes; user-flagged as brute-force tech-debt for a future
structural rewrite) · **`valid_bonded_seam`** conjoins all four. The **demo** emits **two certified
solids** (cone body γ=0 + γ≠0 lap flap, each `closed_shell_holed`-Verified + OCCT `brepcheck_valid` /
`free_edges==0`) **+ a certified bond** (a lap is doubled material, §6.2, not one self-touching solid).
The BONDED certificate is **3D-rational** — no flat-`γ` needed; the transcendental `ψ` stays in emission.

---

### Driving Demo (DD) acceptance criteria (the self-lapping cone-with-ramp — full 3D↔2D round-trip; **round-trip-first, GO-gated**)

*Authored before implementation (DD.0).* The Driving Demo is the **culminating acceptance demo** the
whole flex-PCB spine (`docs/roadmap-flex-pcb.md`, the [[driving-requirement]] bidirectional transform)
builds toward: a **cone whose scalar support `h` ramps `0 → D ≈ 0.27 mm` over the last ~60°** of its 2π
wrap, so an offset sector **laps over the base** at the seam, put through the **full bidirectional
round-trip** — author the construction + **cut boundaries in 3D** → **unwrap/develop** to the flat
pattern → author **interior ECAD features in 2D** → **fold/wrap back** to 3D. Validated numerically in the
user's prototype `~/temp/dev-quat-rep-test` (same `(q,h)` chart representation as the kernel). It
**composes** what is already built — Stage 1 (`cut → unroll → 2D-hole → STEP` per-panel pipeline), Stage 2
(`develop::bonded` §14 BONDED + `develop::seam_frame`), Milestone E (certified develop ① / fold ②) — and
lands the **one genuine new frontier the geometry forces: the γ≠0 flat-directrix integrator** (DEV.3
"method (b)"). It realizes DEV.3's "two product pipelines end-to-end". It **defers** multilayer stackup,
multi-panel atlas / reflection-mate (D4.4), and complex authored ECAD boundary with cutouts (D4.3 / §14
curves) — all *Beyond Stage 2*.

**Four framing decisions settled at DD.0 (they scope the whole milestone).**

1. **Representation = body gore + ramp flap + certified bond.** The base-cone gore (`h≡0`, ~300°) and the
   ramp flap (`h:0→D`, ~60°) together cover the full 2π, the flap lapping the body's head; each is a valid
   OCCT solid, joined by the Stage-2 `valid_bonded_seam`. This is the §6.2 "two-piece lap" doctrine and the
   Stage-2 geometry lock — a lap is **doubled material**, so a *single self-touching 2π solid* is
   OCCT-rejected and out of scope (no single-chart 2π cover, no `[D11]` chart-graph cycle). The
   self-lapping mental model and the body+flap assembly are the **same geometry**: one ramping-`h` chart
   whose 2π wrap self-laps, realized as the two-view assembly.
2. **Boundaries are cut in 3D; only interior ECAD features are authored in 2D and folded back.** The
   outer/inner/notch boundaries are cut in 3D exactly as today's Stage-1 demo does (`export::trim` /
   `develop::unroll::unroll_trim_loop`) — **not** re-authored on the flat. The 2D→3D fold leg carries
   **interior** cuts (an ECAD feature authored in the flat pattern, folded back onto the surface).
3. **Round-trip first, then the seam** — de-risk the fold-back leg (DD.1) on the plain cone gore before the
   transcendental γ frontier (DD.2) and the seam device (DD.4).
4. **Build the γ integrator** — the ramp flap's flat pattern is *certified* (a verified interval
   enclosure), not emission-only. This is the DEV.3 method-(b) commitment.

**The gaps (from the gap analysis) → the slices.**

1. **The fold-back leg is unwired.** `develop::fold::fold_outline` is built + unit-tested, but the Stage-1
   driver lifts cuts *forward* (`brep_trim_solid`); it never authors a cut in flat coordinates and folds it
   back to 3D. → **DD.1**.
2. **The ramp flap's certified flat pattern needs `γ(σ) = ∫₀^σ e(ψ)·(pedal speed) dσ = ∫(rational ×
   cos(c·arctan σ))`** — non-elementary for `c = 130/97` (`ψ` *is* closed-form; `γ` is not). `develop::cone`
   hard-codes the pure-radial polar map `D = µ̂·ρ·e(ψ)` (γ≡0) and rejects `h≠0` (the `cone_angle_coeff`
   pedal gate); `develop::interval` has **no quadrature**. → **DD.2** (the DEV.3 method-(b) frontier,
   spike-first).
3. **The γ≠0 fold** — `fold::invert_sigma` recovers σ from the pure-radial signed area `= µ̂·ρ·e(ψ)`; with
   γ≠0 the flat point is `γ(σ) + µ̂·ρ·e(ψ)`, a coupled 2-D inversion (subtract `γ(σ*)`, σ unknown). →
   **DD.3**.
4. **Full-2π / self-lap = the body+flap+bond assembly.** `brep_trim_solid` spans a finite σ-band; the 2π
   cover is the two-solid assembly (body chart + re-centered flap chart) + the certified bond. Extend the
   Stage-2 two-solid stub from the tiny σ'-box to the real device. → **DD.4**.

Slices (each commits green; additive; **round-trip-first**, so the transcendental frontier is isolated to
DD.2/DD.3 and the plain-cone fold leg is de-risked first):

- **DD.0 — this GO-gate (docs).** This section + the `vv-matrix.md` DD rows + the engineering-log DD
  thread; lock the scope + the four decisions + the round-trip structure. No code. Green via `xtask lint`.
- **DD.1 — the round-trip on the cone gore (γ=0, no seam).** **Met when:** (1) a driver keeps the **3D
  boundary cuts** (reuse `export::trim::{outer_loop,hole_loop}` + `develop::unroll::unroll_trim_loop`) and
  develops the gore to a flat pattern; (2) an **interior ECAD feature authored in the flat pattern is
  folded back** via `develop::fold::fold_outline` → a certified `FoldedWire` lifted onto the cone gore (the
  leg the demo skips today) — closing the "ECAD → fold → solid" pipeline; (3) a round-trip integration test
  recovers a flat-authored feature to `< ε` per-stage `Verified`, float-corroborated against `mesh3d`
  (oracle ∧ audit); (4) the driver writes SVG + STEP. **No new frontier** (γ=0 throughout).
- **DD.2 — the γ≠0 flat-directrix integrator (DEV.3 method b) — SPIKE-FIRST.** **Met when:** (1) a
  **verified interval integrator** for `γ(σ) = ∫₀^σ e(ψ)·(pedal speed)` lands in `develop::interval`
  (candidate: Taylor models / verified Riemann sums over the existing `cos_on`/`sin_on` + `eval_ratfunc_on`
  enclosures; the spike selects + prices the method, `docs/spike-development-report.md` mold); (2)
  `ConeDevelopment` is **generalized in place** — an optional directrix `γ`, `point`/`point_on` add the
  `+γ(σ)` term, a `γ≡0` fast path so the cone reproduces today's output **byte-for-byte**, and the
  `cone_angle_coeff` pedal-nonzero rejection is lifted for the ramp; `unroll`/`anchor` ride unchanged (the
  single chokepoint is `dev.point`). **GO** = a converging, `Verdict`-typed γ-enclosure with fab-plausible
  ε on the ramp flap, float-corroborated by a directrix oracle. **No-go** = the enclosure doesn't converge
  over the ramp box → record the wall + alternative.
- **DD.3 — the γ≠0 fold.** **Met when:** `fold::fold_point`/`invert_sigma` subtract `γ(σ*)` and solve the
  coupled 2-D inversion (ψ-monotonicity from `split_domain` stays valid; the 3-D lift already reads
  `chart.pedal()`), and a flat-authored feature folds onto the ramp flap with a certified round-trip ε that
  shrinks with iters (too-tight → `Unresolved`, never a wrong `Verified`).
- **DD.4 — the seam device (the acceptance demo).** **Met when:** the Stage-2 two-solid stub is extended to
  the **real device** — **body gore** (γ=0, ~300°, 3D-cut boundaries, developed + interior features folded)
  + **ramp flap** (γ≠0, ~60°, developed via DD.2 + folded via DD.3) + the **certified bond** (Stage-2
  `valid_bonded_seam`), body ∪ flap covering 2π; two STEP solids + the certified bond + the full
  round-trip artifacts, each corroborated by OCCT `audit_brep` (`brepcheck_valid`, `free_edges==0`).

**Doctrine.** No float in a certificate — the γ integrator is a **rational-interval enclosure** (shell-tier
interval arithmetic, like `develop::cut`/`anchor`/`bonded`; no Kani/Lean TCB growth), the OCCT/mesh oracles
corroborate, never certify. Fail-closed — a loose γ or fold → `Unresolved`, never a wrong `Verified`.
Oracle ∧ audit, never oracle-instead.

**Generality (hard gate).** Fully general, no goal-specific hacks (the roadmap's standing decision). The γ
integrator is a general directrix enclosure (`γ` is any developable's flat directrix, the cone-with-ramp is
one instance); `ConeDevelopment` gains an *optional* γ with a byte-identical γ≡0 fast path (**no interface
ossification** — one general engine, the γ=0 cone is the thin special case, consumers updated freely). No
checker names "ramp" or "cone-seam". No new `Verdict` variant (any extra sub-state is a domain enum lowered
via the `EdgeReg::to_verdict` idiom). No Kani/Lean TCB change unless a new *combinatorial* claim appears —
flagged at that point if so.

**Documentation (a merge gate).** Usage-first, history-free docs on new public surface under
`-D missing_docs`, worked doctests on entry points; a DD.2 spike report (`docs/spike-*.md` mold) recording
the γ method + the certified `ε_γ` on the ramp flap + the float directrix-oracle corroboration + the
GO/no-go.

**Status: DD.0 + DD.1 met** — on `driving-demo` (branched off `stage-2-seam`). DD.0 authored this section
+ the `vv-matrix.md` DD rows + the engineering-log DD thread. **DD.1** wired the fold-back leg the Stage-1
demo skips: the `roundtrip_panel` driver cuts the gore boundary in 3D (eccentric annulus), develops it
(direction ①), authors an interior ECAD feature **on the flat pattern**, and **folds it back** onto the
cone (`develop::fold::fold_outline`, direction ②) to a certified 3-D wire — round-trip backward error
≈2.7e-12. The `roundtrip_fold` integration test closes the loop (develop ∘ fold recovers the 3-D geometry
to <1e-6), float-corroborated by `develop_cone` (<1e-5, oracle ∧ audit), fail-closed on an out-of-gore
feature; the driver writes SVG + a certified STEP annulus solid (OCCT `write_brep` "ok", 0 free edges). No
new frontier (γ=0 throughout). **DD.2** landed the one genuine frontier — the **γ≠0 flat-directrix
integrator** (DEV.3 method b): `develop::interval::integrate_on` (a verified interval Riemann sum) +
`ConeDevelopment` generalized in place with an optional directrix `γ(σ) = ∫₀^σ [a·e(ψ) + b·e⊥(ψ)]`
(`a = (c′·r)/ρ`, `b = −(c′·n′)/ρ`, spec §Tier C), a byte-identical `γ ≡ 0` fast path, and the pedal
gate lifted (`arctan_coeff`). GO (`docs/spike-directrix-report.md`): the development is a **machine-exact
local isometry** (`|D_σ|²−|X_σ|² = 7.1e-15`, confirming the Tier-C frame/sign) with a converging,
fab-plausible `ε_γ` (1.45e-3 → 9.0e-5 over 64 → 1024 panels). **DD.3** extended the fold (direction ②)
to `γ ≠ 0`: `develop::fold::fold_point` now builds `new_developable`, and `invert_sigma` inverts the
**directrix residual** `(x,y) − γ(σ)` (signed area `cos ψ·(y−γ_y) − sin ψ·(x−γ_x)`) with a **flip** for
the γ≠0/µ̂<0 residual-at-(ψ+π) case, the radius reading `r = |(x,y) − γ(σ)|`. Folding a flat point on the
seam-ramp flap (`cone_seam_ramp`, µ̂ < 0) recovers its `(σ′, µ̂)` and round-trips to **ε ≈ 3.85e-3** (at 64
γ-panels, clears the DRC; a tight clearance → `Unresolved`, fail-closed); the γ ≡ 0 path stays byte-identical
(every DEV.2e fold test passes). **DD.4 — the acceptance demo — MET, the Driving Demo milestone COMPLETE.** The `bonded_seam_device` driver
+ composed test realize the full bidirectional round-trip on the self-lapping cone-with-ramp as **body gore
(γ = 0) + ramp flap (γ ≠ 0) + a certified bond** (§6.2, a lap is doubled material): each sheet **develops** to
a certified flat pattern (flap ε ≈ 7.05e-3 γ≠0, body ε ≈ 7.6e-13 γ=0) and a flat point **folds back** onto it
(flap ε ≈ 8.1e-2, body ε ≈ 2.7e-12), the seam **bond** is certified by `valid_bonded_seam` (SEP ∧ SLAB ∧
SHEAR δ=18/65≈0.28 mm ∧ CLEAR), and the two sheets emit as **two certified STEP solids** (`brep_trim_solid`,
each OCCT `audit valid ∧ free_edges==0 ∧ nonmanifold==0`). Both product directions, on the real γ≠0
self-lapping geometry, bonded — the flex-PCB spine's acceptance.

### AUTH.1 acceptance criteria (the sketch-extrude cutter — frame × profile × apex × span)

Full design in [`cutter-extrude-design.md`](cutter-extrude-design.md). The step-1 blocker: authoring
a real substrate boundary needs cuts defined by a **2-D arrangement placed in a plane and extruded**,
with taper and with control over how deep the cut reaches. Cuts here happen **before any stackup
exists**, so a span counts **neutral surfaces** (chart embeddings), not copper layers and not faces.

**AUTH.1a — the apex and the wall.** One homogeneous `Apex = [a : w]` covers both extrusion modes
(`w = 0` is today's parallel drill, `w ≠ 0` a finite cast point), so the generatrix and the wall are
one formula and the cut-fit certificate is derived once. `w == 0` is an exact `Rat` test. A wall is a
Plane for a segment edge and `CutSurface::Quadric` — the general `XᵀMX + b·X + c = 0` on one nappe —
for an arc edge, so everything stays degree ≤ 2 over ℚ(σ). The general form is not optional: a cone
over a circle from an apex off *that circle's* axis is oblique, hence elliptic, and an affine frame
makes a profile circle an ellipse to begin with. Both §4.1 validity conditions **refuse** rather than
repair, and they are one runtime check, because the apex sits on the nappe selector's own boundary:
the cutter is the single nappe on the authored side, and a band that reaches the apex — where
"inside" inverts — is `Refuted`.

A quadric has **no closed-form distance**, so the certificate uses the first-order gradient-flow
bound, whose hypotheses (`|∇F| ≥ g > 0` on the working ball, `|F|/g ≤ R`) are discharged per box at
runtime. Two things must be shown, not assumed: that the general wall really *is* the special
surface it generalizes (an extruded metric circle must pull back to the metric cylinder's own
µ̂-form, up to scale), and that the bound tracks a distance that is independently known — against the
exact cylinder distance, and against exactly-known geometry. Its limit is stated rather than hidden:
the working ball must avoid the surface's singular locus, so an error comparable to the feature's own
radius reads `Unresolved`.

**AUTH.1b — frame and profile, on one rule.** The frame is affine — origin plus two independent
rational spanning vectors — because rational *orthonormal* frames exist only for special normals and
a picked frame could not otherwise be represented exactly. The frame reports its metric distortion,
so a caller needing a true metric circle can tell whether it has one. The profile is an `arrange2d`
region with its existing inside designation: non-convex profiles and holes need **no** decomposition
and no polygon-CSG layer. The inside predicate is projection-through-the-apex followed by
point-in-region, both rational hence exact.

The design's §2.3 rule is what has to be demonstrated, not just implemented: a point's frame
coordinates are a rational quotient, so every wall is a 2-D carrier equation with that quotient
substituted and the denominator cleared. Three consequences are each a test rather than a claim —
a wall is read off its edge's **carrier**, so a profile arc's algebraic endpoints never enter; a
circle needs only **`r²`**, never the irrational `r`, so an arc can come straight from `arrange2d`;
and each wall **mirrors its own carrier's sign**, leaving the fill rule with the region. The last of
these is what makes the non-convex-with-holes claim testable: the boundary view and the predicate
view must agree at every interior and exterior sample of a convex profile, and the predicate view
alone must decide a non-convex one. Undecidable queries — no projection, or a row exact ray-casting
excludes — are `None`, never a guess.

**AUTH.1c — ray-pick frames are certified by backward error.** A ray meeting a rational developable
solves a polynomial, so the hit is algebraic; carrying it as such would push `AlgReal` into every
downstream cut. The hit-finding is therefore a **search**, and the frame it produces carries a
certificate rather than trust — the same split MAP.1 installed in `fold`, and testable the same way:
**the certificate must hold under a deliberately degraded searcher**, which means not a fixed
threshold but the sharp statement that the DRC tracks the reported ε at *every* damage level (a
perturbation small enough to stay inside the clearance should still certify, and does).

Two things the criterion has to be stated carefully about. The frame is **exact** — chart fields at a
rational σ are exact rational vectors, so the origin is *on* the surface and the axes *are* the local
ruling and normal; only σ is approximate, which is why one residual suffices. And that residual must
be **point-to-point, not point-to-line**: the distance to the ray's line is blind to the sign of the
ray parameter `t`, and `t` is what the span orders hits by. A test that only checks the line distance
certifies a sign-flipped solve.

The pick's ε covers **geometry, not ordinal**. A root scan can step over a double root or two roots
in one cell, and no backward error detects a miscount — so an ordinal claim needs a root **count**,
which is AUTH.1d's problem, not this one's.

**AUTH.1d — the span, and the criterion that distinguishes it from a layer index.** `ToNext |
NextN(k) | Through | Range(start..=end)` over the neutral surfaces the reference ray meets, ordered
by ray parameter. **The named test:** on the self-lapping cone, a ray through the lap meets the same
chart twice, so `ToNext` cuts the flap only while `NextN(2)` and `Through` cut flap **and** body. No
new fixture — the geometry is already certified, so the test measures span semantics rather than
re-testing the device. A ray that misses, or grazes within tolerance, is `Unresolved`/`Refuted`.

The unit of counting is a **region**, not a chart, and the test has to show why: taken bare, the wrap
chart sends two σ to the *same* 3-D point at the lap, and only the per-region support separates them.
The named test must therefore pin three things a weaker one would miss — that both crossings come
from **one** chart distinguished only by support law; that the ordering is by **ray parameter**, which
on this device is the *reverse* of the σ order, so an ordinal read off σ inverts the lap; and that the
far wall the same *line* meets at `t < 0` is not counted.

An ordinal needs a **certified count**, which is what §9.1 left open: roots isolated by a Sturm chain
whose hypothesis is checked at runtime, tangency refused exactly (`gcd(g, g′)` of positive degree),
and crossings closer than the clearance refused as unorderable — with the gap reported so a caller
can see what clearance the geometry would need. The scan-based searcher remains a valid second
opinion on the *values* and is tested against the Sturm one differentially; it is only the
*completeness* claim it cannot make.

**AUTH.1e — no special case survives the generalization.** `Cutter::surface() -> CutSurface` cannot
represent a multi-wall cutter and is replaced; the `Subtract`+`Cutter::Cylinder` station-sampling
special case in `resolve.rs` becomes "the σ-windows where any wall is active" — otherwise an extruded
cutter gets no targeted stations and drops small features between cells. Existing `HalfSpace` and
`Cylinder` cutters keep working, with their pinned ε and chord goldens unchanged.

**Split the refactor from the capability, and gate them differently.** The µ̂-`Shadow` has to become a
union before an extruded cutter can have one, and that step should land alone with the strongest gate
available: the whole suite green with every pinned ε, chord golden **and work budget** unchanged. The
work-budget tests assert counted work, so their passing is evidence nothing *moved*, not merely that
nothing broke. Only then is the capability safe to layer on.

**The generalization must key off the right property.** Both station sites matched on the *cutter
variant*; windowing is a property of the **wall** (`a ≢ 0` ⟹ real only between tangent rulings). Test
that the new criterion reproduces the old behaviour by construction rather than by observation.

**Unit tests of the new function are not enough, and here is the evidence.** The shadow's own tests
passed before and after a defect that made an extruded disc derive **two** interior holes where the
cylinder it equals derives one — a duplicated wall, because `arrange2d` edges are decomposed pieces
and several share one carrier. What found it was an **end-to-end differential**: author the same
solid two ways, resolve both, compare roles/holes/kept-side. A slice claiming "wired into `Part`"
needs a test that goes through `Part`.

The membership sampling deserves its own case. Between consecutive wall crossings membership is
constant, which is what makes one midpoint sample exact — but only while the sample stays inside its
stretch, so the genericity nudge must be scaled to the **stretch**, not to the profile. The test is a
lobe two orders of magnitude thinner than its neighbour, measured at its true width.

**AUTH.1f — the demo cuts something real.** A drafted cut authored on a rational frame, taken
through develop → STEP: per-stage `Verified`, the emitted geometry checked **faithful to the
authored profile** (not merely certificate-green), and the full gate. vv-matrix rows go ✅ and
`[AUTH.1]` joins the landed set in `cargo xtask lint`.

**Why "faithful" cannot be an ε assertion here, stated as a criterion.** `ε` is the max over
pipeline stages, and on this device the panel boundary dominates it — so a drafted hole and an
undrafted one certify at *the same* `ε`. Any test asserting only `Verified`, or only an ε bound,
passes on a cutter that ignores its apex entirely. The faithfulness check must therefore measure
**geometry**: the developed hole's size against the taper law the cast point implies
(`1 − z/z_apex`), and a parallel sweep against the metric cylinder it reproduces. Measure through the
quarantined exact→`f64` bridge, never a hand-rolled conversion — that returns NaN on large rationals
and `min`/`max` swallow it, turning a real measurement into a silent "could not measure".

**The demo is a gate on the composition, and it earns that place.** Two defects reached it through
six commits of unit and integration tests, both invisible to every layer test because they lived in
the *composition*: a duplicated wall (edges are not carriers) and a polygonal slot deriving
`Inactive` — a green certificate on a cut that did nothing, because affine-walled cutters received no
targeted stations. A milestone whose slices are each tested in isolation still needs the end-to-end
run before it can be called done.

**AUTH.1e.4 — the multi-wall hole loop.** A profile with several carriers realizes end-to-end, and a
footprint the band representation cannot express is refused **by name**. The criteria:

*A two-sided differential, not a golden.* A square prism's hole must contain the hole of the
cylinder inscribed in it and sit inside the one circumscribing it — `disc(h) ⊂ square(h) ⊂ disc(h√2)`
as solids, hence for the developed holes, hence for their widths. Both bounds come from the metric
cylinder path, which shares no line of code with the wall-crossing band builder. Asserted twice: on
the emitted `(σ, µ̂)` loop in `develop` (measured 0.2364 / 0.2342 / 0.2323 against inner 0.1615 /
0.2303 / 0.1639 and outer 0.2841 / 0.3257 / 0.2800 — a 1.7% squeeze at the middle ruling), and
end-to-end on the developed pattern in `author`.

*The loop must be seen to use several walls.* AUTH.1e.2's failure mode was following one wall
silently, so a size check alone is not enough: every emitted vertex is tested against all four
planes and must sit on one of them, and the vertices between them must use at least three. A loop
that had quietly stayed on `walls[0]` passes the ε gate and fails this.

*The scope refusal is a test, not a comment.* A ring profile must come back
`PartFault::ProfileNotSimple`, and the point of asserting it is that it used to fail closed by
*accident* — on a window search declining a shape it could not read. An accidental refusal is one
refactor away from becoming an accidental acceptance.

*The end-to-end run is where the derived quantities get read.* AUTH.1e.4 found that
`Extrusion::extent` — AUTH.1f's own code, feeding both the hole's σ-window and the span's reference
ray — bracketed segment endpoints with an upper bound on *both* sides, producing a box of zero
height that contained none of the profile. Two slices of tests passed over it because none read the
box quantitatively: the disc path never builds one from a `Surd`, and the polygonal-slot test only
asked that the role was not `Inactive`. The criterion: a derived quantity is unverified until some
test asserts its **value**, not merely that the pipeline it feeds returns a verdict.

*Soundness may not rest on a search.* The corner bisection is a tightness device; what keeps the
loop honest is that each piece is also compared, at its own σ-midpoint, against the boundary the
exact fill rule reports there, with the deviation folded into ε. State it as a criterion because the
failure it guards is invisible to the per-piece certificate: a chord that stays on wall A across a
missed corner certifies perfectly while the true boundary dips onto wall B beneath it.

*Nor may a **window derivation** rest on a scan, and the difference is absolute versus relative.* The
resolver read a cutter's σ-windows from a fixed 256-subdivision sign scan of the whole band, so a
window narrower than one cell had both its roots in that cell and the op resolved `Inactive` (#268) —
the same fail-open verdict as above, reached arithmetically rather than structurally: the two discs
that separate the cases have the **same radius**. Where the window is not known in advance, isolate:
`tangent_events` brackets every root exactly, and a window read as the *gap between two brackets* is
proved root-free, so one midpoint evaluation decides it. A scan whose interval is derived from the
window it seeks (`surface_tangents`, padded by a sixteenth of the window's own width) is a different
thing and stays. Two criteria come out of this. **Pin such a fix with a differential, not a verdict**
— development is an isometry, so a disc of a given radius cuts the same *area* wherever it sits, and
the narrow-window drill must develop to the wide one's hole; asserting only `Hole` would pass on any
hole at all. And **the deferral's cost is itself a measurement**: this one was deferred for a blast
radius across every pinned ε, golden and work counter, and re-measuring found all of them
bit-identical, because the cases a sampling assumption got right are the ones a sound method
reproduces. Record the comparison rather than the expectation.

### AUTH.2 acceptance criteria (the general non-convex footprint — the tracer)

Lifts 1e.4's band restriction for footprints that are **non-convex but connected** (L-slot, T-slot,
keyhole, dogbone). Design in `docs/cutter-extrude-design.md` §11. Scoped by measurement rather than
by assumption, which is the first criterion:

*Scout before sizing, and let a control tell you when you are measuring the wrong thing.* The
milestone was filed as "holes must become regions end to end, through the flat boolean and into the
B-rep builder". Authoring a non-convex `(σ, µ̂)` loop directly (`Part::hole_domain`) showed the flat
path and the within-slice solid path are **already general**, and that the only downstream gap is a
loop crossing a σ-station. Both measurements are pinned as tests in `author/tests/fold_part.rs` — one
that must stay green through the milestone, one that is *expected to flip* when 2e lands. Method
note worth keeping: the first three scout runs refused with `TopologyMismatch`, which looks exactly
like the thing being tested, and it took a convex control at the same coordinates to show it was
placement (the slot sat on the panel's own drill, then on its inner boundary).

*The event set is exact, and its arithmetic is checked against an independent computation — at every
degree, including the degenerate ones.* The tracer's σ-partition comes from three polynomial
families: `disc_µ̂(f_i)`, `Res_µ̂(f_i,f_j)`, and `a_i(σ)` (§11.2). The resultant is checked two ways,
because a wrong one produces a *plausible* partition rather than an obvious failure: against a full
4×4 Sylvester determinant where both forms are genuinely quadratic, and — at every degree — against
the property that actually matters, vanishing exactly when the two walls cross the ruling at a common
µ̂.

The second check is not ceremony. The published quadratic-by-quadratic closed form is the Sylvester
determinant of the two forms *padded to degree 2*, and with **both** walls affine it is identically
zero, for meeting and non-meeting walls alike. Every wall of a polygonal profile is affine, so
using it throughout would have silently erased every corner of the L-slot the milestone is for —
while still passing a differential built only from quadratics. The criterion generalizes: **when a
formula has degenerate cases, the test set must contain the case the feature is for**, not just the
generic one.

*The exact events buy tightness, not soundness — demonstrated, not asserted.* The design doc's
§10.3 discipline carries over unchanged: every piece is compared at its own σ-midpoint against the
boundary the fill rule reports there. The criterion is a test that **perturbs the event set** and
shows ε degrading into `Unresolved` while the geometry stays honest. Without it, "the search is not
load-bearing" is a claim about code that nothing checks.

*A traced loop must be seen to be non-convex.* The 1e.4 lesson, one level up: a size or ε check
passes on a hole that was silently convexified. The emitted loop must turn **both ways** (a reflex
corner in the developed pattern), and in the solid it must contribute one wall per edge — the same
check that catches a loop quietly collapsed to its bounding band. Already asserted, with both
assertions mutation-verified, on the pinned pre-state. The traced L adds the sharper form: at a
ruling through the notch its loop is met **four** times, which is precisely what a near/far rail
pair has no way to carry.

*A differential's tolerance may not be derived from the quantity under test.* The tracer is checked
against the band builder on the band's own fixture, and the first version of that check compared the
two loops to within `band.eps + traced.eps` — which grows exactly when the tracer gets worse. It
passed a mutation that made the tracer's ε **8× worse**, silently. The tolerance is now a fixed
multiple of the *band's* bound alone, and the mutation fails it. State it as a criterion because the
vacuous version looks more rigorous than the sound one: it cites both certificates instead of one.

*A fixture must produce the phenomenon, not merely resemble it.* The L-slot fixture went through
two false negatives that read as tracer failures: an L whose arms lie along this cone's radial
rulings has a perfectly ordinary **band** footprint, and even once rotated, the reflex corner
visible in its flat pattern proves nothing (a band can be a non-convex region). The property under
test is a ruling meeting the cutter twice, and the fixture has to be checked against *that* — here
by the ruling at the crossed station being cut **four** times in the emitted solid — two intervals,
which no band makes. A test whose fixture does not exhibit the phenomenon passes for the wrong
reason and hides the capability it claims to prove.

*Lifting a restriction means auditing what the old special case **supplied**, not only what it
required.* The trim builder skips the wall on a radial at an interior σ-station, because two slices
share that cross-ring — true for as long as every hole was a `HoleRail`, whose branches are
continuous in σ. A polygon hole with a `σ = const` edge on a station breaks it: the two lids keep
different material there, and skipping the step left four free edges under a `Verified` verdict. The
criterion has two halves. First, the *shell* property is asserted directly on the emitted B-rep
(`free_edges == 0`, `nonmanifold_edges == 0`, genus) rather than inferred from a verdict — the
verdict is about the certificates upstream and says nothing about how the shell was sewn. Second,
the fixture must put the hole's step **on** a station, because that is where an authored corner
tends to fall and where the two paths disagree. The mirror-image error is worth stating too: a
refusal added for a combination one believes unrepresentable encodes a belief about the data, and
"a rail branch is not a polygon operand" was false for every hole the kernel emits — the band is a
polyline, so both channels share a slice by conversion rather than by refusal.

*AUTH.2f — the acceptance demo measures the phenomenon, not its consequences.* The milestone's
claim is that a footprint no band can express goes through the whole pipeline, and the criteria are
what make that claim checkable on the artifact rather than on the verdicts.

The **ruling-crossing signature is readable off the flat pattern**, and that is the criterion worth
stating generally: development is an isometry sending each ruling to a ray from the flat apex, so
"a ruling meets the cutter twice" is "a ray meets the developed hole in two intervals". Four
crossings. Every band gives two, however non-convex its planar shape — so the metric probes are
measured alongside and must give two, which is what turns the four into a signature rather than a
number. Without it the demo asserts only what a within-slice band-shaped hole also satisfies.

*A two-sided differential needs a third clause once the footprint is non-convex.* 1e.4's
`disc ⊂ square ⊂ disc√2` transfers — the slot contains a disc inscribed in one arm and lies within
one circumscribing it — but **both containments are satisfied by a slot convexified to its bounding
band**, which is precisely the failure mode. The third probe sits in the notch the slot does not
cover and must be **disjoint** from it. State the containment test too: non-crossing plus one
interior point, never a vertex sample, because a vertex test passes on a ring that pokes out between
two of its own vertices.

*When the claim is about which branch ran, the artifact is the wrong place to look.* A hole that
crossed a σ-station and one that sat inside a slice certify alike and build alike; nothing in the
emitted solid distinguishes them. The criterion is therefore a **counter**, not an assertion:
`develop::counters::poly_slice_clips` witnesses the general channel running on more than one slice,
with the un-slotted control asserted at zero so the counter is known to be measuring the slot. A
milestone slice that cannot be seen in the output has to be made countable or it is on the demo's
critical path only by assertion.

*The round-trip is checked against the input's own source, not against itself.* Direction ② folds
the flat pattern's emitted vertices back and measures the distance to the **authored profile's
boundary** — a quantity neither leg computes. A round-trip compared against its own input is
satisfied by both legs sharing a mistake.

*The exact tier and the export tier have different minimum feature sizes, and the seam is where the
demo failed.* Every certificate was `Verified` and the STEP write refused the shell: the tracer
samples one grid step (`2⁻³⁰`) inside each cell end to keep pinches tight, and those vertices become
edges whose 3-D span is an order **below** OCCT's `10⁻⁷` vertex tolerance, so the edge's own curve
reads as closed while its two vertices are distinct. Measured on the L-slot: 220 shell vertices at
145 distinct positions, 76 sub-tolerance Bézier edges. The criterion has two halves. First, an
export profile needs a **declared minimum step** enforced where geometry is handed to it
(`hole_poly`, the station partition), not a hope that the exact tier never emits anything smaller.
Second — and this is why it belongs in the acceptance criteria rather than the design notes — *the
verdict does not cover the exporter*: `Verified` is about the rails, and an artifact that no
downstream consumer can read is a failed demo whatever the certificate says. A demo that does not
run the exporter cannot discover this.

There is a third half, learned from the four edges that survived the first fix: **a minimum step
enforced within one structure does not reconcile it with another**. `hole_poly` compares a loop's
vertices with each other and `thin_stations` compares stations with each other, but the loop and the
partition are derived independently — the tracer from the cut's own event set, the stations from the
surface's positive-weight bisection — and their *disagreement* was under nobody's tolerance. Where
two such structures meet, the criterion is that one of them yields, and which one is not arbitrary:
the station carries validity the exported patches depend on and is shared by every other hole, while
the polygon is already declared to be the loop only to within the step, so the **vertex** moves. The
check that makes this testable is an equality, not a bound — a feature authored a grid step off a
station must build what the same feature authored **on** it builds, vertex for vertex — and the
number to report is the **shortest emitted edge**, which is what the consumer actually decides on
and what no verdict states (`acceptance::measure::shortest_edge`, against `CAD_VERTEX_TOL`).

*The ring stays refused, and for its own reason.* A footprint with its own hole is not a
representational shortfall — an annular through-cut leaves a disc of material floating, which is two
parts. It must come back as a typed refusal on the *nested loop*, tested by name (§11.8).

---

## 9. Sequencing

M0 grows Kani harnesses with the code (fast-path lattice verified before anything consumes it) and runs the §7 spike. `certify-core` splits out at M2 as the Lean target from birth. Stratum-weighted generators land with M3a (arrangement). The V&V matrix and `docs/proofs/ledger.md` start as stubs in the repo skeleton.
