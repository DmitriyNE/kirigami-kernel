# Flex Substrate Representation — Implementation Plan v1

Target: a certified-exact implementation of spec v0.24 (+ the pending-v0.25 profile notes). Companion to `flex-substrate-rep-spec-v0.24-full.md`; the spec wins on any conflict. This document is a plan, not a contract — it states module boundaries, dependency order, milestone slices, and the testing doctrine, with honest risk flags.

## 0. Ground rules inherited from the review

- **Certificate = data.** Every certificate is a stored object carrying its evidence (margins, Sturm sequences, isolating intervals, stamps), never a boolean that forgot its reasons. Three-valued verdicts everywhere: `Verified(evidence) | Refuted(witness) | Unresolved(margin)`.
- **Types encode the batch-19–21 findings.** Proof types are enums whose variants are the constructors; a demand stratum without a constructor is a compile error, not a review finding. Mode-indexed types (PAIR-IDENTICAL / OUTPUT-SOURCE-IDENTICAL), occupancy packets, and the V_cand/V_∂ split are the type layer's job.
- **Oracle ∧ audit.** Every independently recomputed reference (links, occupancies) is compared against stored records in the same function that computes it.
- **The counterexample corpus is the day-one test suite.** ~25 stored refutations with known verdicts (§5 below). A fixture per delta item; the verdict is the assertion.
- **Language: Rust.** Rationale: the type-system findings map directly (enums, exhaustive matches, newtype margins); exact arithmetic via `malachite` or `num-bigint`/`num-rational` (benchmark both at M0); no unsafe, no floats in any certified path (floats permitted only in diagnostics/plots behind a feature flag). CGAL is **not** a dependency — it is a differential-test *oracle* (§5).
- **Workspace layout:** resolved into a layered pure-core / shell structure — **see `environment-and-crate-layout.md §1` (authoritative)**. Pure tier (no_std, the extraction surface + TCB): `lattice`, `certify-core` (the latter absorbs the former `certify1d` as `certify_core::certify1d`, plus the extracted `arrange`/`sew`/`gate` checkers). Shell tier ("kernel-search"): `geom`, `arrange2d`, `closure`, `sew`, `gate`, `develop`, `export`, `fixtures`, `difftest`. Lean lives in `certify-check/` (a lake project, not a cargo crate). The spec lineage lives in-repo under `spec/`; the := census and the pattern-ledger lint list run in CI against both spec and code comments.

## 1. Module decomposition (dependency order)

**M0 — `lattice`: exact arithmetic.**
Quantum rationals (L0), √-extension values / degree-2 algebraic points (L2 as used by the arrangement), interval-plus-separation comparison, cleared-forms helpers, the squared-margin convention as a newtype (`MarginSq`), polynomial arithmetic over ℚ, Sturm sequences (isolation + sign-on-interval), bivariate resultants (for φ_J, EDGE-EMB). Small, boring, load-bearing. Everything above imports only this for numbers.

**M1 — `geom`: chart primitives.**
Quaternion splines q(σ), h-splines, C(σ, μ, w), n/r/pedal, tags, the hatted stall calculus (p̂, μ̂, r̂, n̂′, Ĵ; J_raw = p̂Ĵ as a tested identity), substitution/removability transport, b_J/b_i/G_i fields, N_i^cut. Fixtures: the device cone (β = 42°, ID 5 mm, 1.49 wrap) and the petal with its conical flank — the two normative instances, as data files.

**M2 — `certify1d`: the 1D certificate engine.**
REG-Q/REG-V/SLAB determinant forms, corner-evaluation utilities (min/max declared per the convexity rider), the CLIP ladder (CLIP-W → CLIP-μ → common-zero isolation → CLIP-a | CLIP-σ signed disjunction | reject), TRIM-LOCAL, the CLIP-DOM corner-sign census with Sturm-isolated events, EDGE-REG (three-way verdict incl. `Stall(t*) → Pending`), REPARAM as a pure function old-record → new-record with provenance.

**M3 — `arrange2d`: the §6 exact D24 arrangement + boolean kernel.** *The beast; highest risk, highest reuse.*
- 3a **Canonical decomposition** (pending-v0.25 profile): x-monotone split at exact extremal points; axis-aligned tag chart so the pole is subsumed; winding = provenance on the source curve, never edge multiplicity.
- 3b **Event spine**: stratified most-degenerate-first; PARALLEL/COINCIDENT as the displayed minor predicates; carrier ∩ carrier (degree-2 points); **interval membership both edges before classification** (winding calculus); transverse/tangent with guards.
- 3c **Stage-2 1D coincidence lattice** on shared carriers (disjoint / touch / partial + residuals / containment / equality).
- 3d **DCEL + the eight-step boolean**: half-edges, (0,0) seed, sidedness = orientation bits, bit propagation + **ℤ₂² cocycle check**, coincident-edge incidence vectors, pluggable selection (⊕/∧/∨), separating-edge law, **quotient emission** (faces = π₀).
- 3e **CAP-OUT**: correctness + completeness bijections (components/edges/vertices over V_∂), CAP-OUT-LINK over V_cand with V_∂ membership computation, **Link_emitted ≅ Link_geometric** (planar).

**M4 — `closure`: joint geometry.**
Fan/collar (WEDGE, EXT-WEDGE, D_collar, quotient wedge), Q-clip on b_J, G_i clips + CLIP-DOM domain species, CAP-IN-D24 census, MITER-FIT (ℓ_i, resultant pairing φ_J, per-cell branch identities, ε_φ as the order sign), the flank cap boundary construction.

**M5 — `sew`: the sewing layer.**
EDGE-OCCUPANCY (four bits + frame bit; both constructors), the identity dispatch table, MITER-EDGE-LEDGER + MITER-OUT (EDGE-REG/EMB/EDGE-EDGE, CYCLE, coverage, vertex quotient), the quadrant classifier with typed counts + reverse ⊔-equality, SEW-LINK (embedded spherical link, FACE-GERM branch index, invariant-jet ties, the record-vs-geometry isomorphism).

**M6 — `gate`: records + validity.**
The certificate store (append-only, provenance-linked, FRESH promotion), CLOSURE-CAP / CLOSURE_VALID / VALID_material / VALID_solid-closure evaluation as pure functions over the store, stamps, the unresolved-propagation rules.

**M7 — `develop`: the flat side.** D map + γ ODE per tag, content layer + flat booleans (shares M3's kernel), folds/reflection mates, seam, calibration. Cold-layer machinery; required for the material grade and fab exports, not for the closure vertical slice.

**M8 — `export`: STEP + mesh + marks.** Planar trims of ruled faces, rational patches with IDEALIZED flags, the mesh size-field cap, dimension/mark layers, GRID-closure rounding ledger.

## 2. Milestones (vertical slices, not layers)

**A — Kernel core (M0 + M3).** The boolean engine passing its share of the corpus on synthetic D24 inputs, plus differential agreement with the CGAL oracle on randomized inputs. Exit: ∪/∩/△ of arbitrary D24 region pairs, watertight-in-plane, all CAP-OUT clauses green. *Highest risk retired first.*

**B — One chart (M1 + M2).** The device cone chart: evaluated, REG/SLAB certified, CLIP ladder exercised on a synthetic trim, mesh with the κ-cap emitted. Exit: a certified single-chart record file.

**C — One joint end-to-end (M4 + M5 + a thin M6).** First the two-planar-flank joint (trapezoid case): clean miter and forced-ledge variants → sewn watertight shell, all link/coverage certificates green. Then the **petal's cone-flank joint** — the counterexample generator — through the same pipe. Exit: STEP-exportable shell for one joint, gate-passing.

**D — The device (M6 + M8 + atlas assembly).** Full cone + lap seam + petal atlas; VALID_solid-closure end-to-end; STEP loaded into OpenCascade with its checker as the external audit. Exit: the lens-assembly flex model as a certified solid.

**E — Material grade (M7).** Development, content, calibration, fab exports; VALID_material. This is where the eleven closure-era riders finally sweep the cold layers — expect findings; budget a review round.

Sequencing note: A and B are independent (parallelizable); C needs both; D needs C; E is orthogonal after B and can interleave.

## 3. Effort (honest, one primary implementer + review)

M0: days-to-a-week (mostly crate glue + Sturm/resultant, ~1–2 kLoC). M3: **3–4 weeks** — exact arrangements are notoriously fiddly, but the spec has pre-litigated the fiddly parts (the event strata, the coincidence lattice, the quotient); the residual risk is DCEL bookkeeping, which the cocycle/link/bijection checks are designed to catch loudly. M1+M2: ~2 weeks. M4+M5: **3–4 weeks** (the ledger/sew types are big but mechanical; MITER-FIT's resultant machinery is the thinking part). M6: ~1 week. D: 2–3 weeks incl. STEP wrangling. E: unbounded honestly — the cold layers haven't been audited; call it 3–4 weeks plus a review round. Total to milestone D: **~3 months** of focused work; calendar depends on your bandwidth. Claude Code carries the mechanical mass (type definitions, fixture transcription, DCEL plumbing, test scaffolds) with you reviewing certificates and me reviewing against the spec.

## 4. Testing doctrine

1. **Corpus fixtures** — one test per stored refutation, verdict asserted: σμ (CLIP-σ signed), antipodal arcs, coincident-vs-tangent circles, tangent-outside-arc, internal-tangency ∪/∩/△ triple, overlapping-disks ∪ (quotient), two abutting boxes (miter cap suppression), diagonal pinch (quadrant reject), crosswise a→c→b→d (link isomorphism), residual-arc 1-vs-3 quadrants, quadratic extents F_B = [0, 1+σ(1−σ)], nodal cubic (EDGE-EMB), σ_B = σ_A³ (ε_φ), cusp y² = x³ (EDGE-REG fail), rank-1 germ (u, v², uv) (FACE-GERM reject), tilted-circle ellipse (CAP-IN fail), |w|/cos α opposite-side cut, s_J = −1 orientation, nested disks (no collapse), diag(1,0,−1) (strict Sylvester), and the rest of the ledger's witnesses.
2. **Property tests** — random D24 inputs; invariants are cheap because the spec made them inventory comparisons: cocycle closure, Euler consistency, all completeness bijections, link isomorphisms, typed-count ⊔-equality, N_cut vs face orientation.
3. **Differential oracle** — CGAL `Arrangement_2` + circular kernel for M3 (agreement on vertex sets/face counts up to the quotient); OpenCascade's shape checker for milestone D shells. Oracles never inside the certified path.
4. **Metamorphic tests** — invariance under REPARAM, under frame-bit flips, under s_J flips, under lattice rescaling; gauge tests for the invariant-jet tie-breaker (same curve, wildly different parametrizations, same sort).
5. **Lint layer** — the pattern ledger as a review checklist; := census and tuple-predicate greps in CI over `spec/` and doc-comments; the truth-valued-only rule as a convention check on `gate`.

## 5. Immediate next actions

1. Repo skeleton: workspace, crate stubs, `spec/` with v0.24-full + deltas + `spec-pending-v025.md`, `fixtures/` with the two device instances and the corpus list as TODO-tests.
2. M0: pick the bigint backend by benchmark (Sturm on degree-12 over 256-bit rationals as the yardstick), land Sturm + resultants + comparison with separation.
3. M3a–3b behind it immediately — decomposition + event spine — since every arrangement test needs them.
4. Wire the CGAL difftest harness early (a tiny C++ shim, JSON in/out) so M3 grows against the oracle from the first week.
