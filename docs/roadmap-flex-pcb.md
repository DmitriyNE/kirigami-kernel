# Flex-PCB roadmap — two acceptance targets, and whether they need hacks

**What this is.** The gap analysis + phased roadmap from today's kernel (post-DEV.2) to the product spine —
the bidirectional multilayer flex-PCB transform (`docs/implementation-plan-v1.md §6`; the driving
requirement). It organizes the remaining work around **two concrete, end-to-end *acceptance demos*** that
gate the milestones, and records the decision that we build these **fully general, without goal-specific
hacks** (§ "Generality" below). It is grounded in a read-only survey of `develop`, `export`, `arrange2d`,
`closure`, `sew`, `geom`, and the spec/paper.

This is a planning document — adopting it changes no code. It reuses the existing milestone taxonomy
(`docs/vv-guide.md`): DEV.3 already owns "the full 2π angular closure + multi-gore seam, the two product
pipelines end-to-end"; the lap seam is an original device target (`implementation-plan-v1.md:53`, "full cone
+ lap seam + petal atlas"); the interior-hole/arbitrary-trim B-rep extends the deferred exporter **V_∂
real-cut** slice; the bonded seam is **spec §14 (BONDED)**.

## The two acceptance targets

### Stage 1 — cone-sector geometry, back-and-forth
A ~300° (rational-approx) cone sector, cut by an **offset-plane curve** (exactly rational:
`μ(σ)=d/(n·ruling(σ))`) + a **fitted cone∩cylinder curve** → **unroll** to a certified flat outline → add a
**square interior hole** on the flat (exact 2D boolean) → **fold back** to 3D → emit **SVG + two STEPs**
(input cut cone; folded panel with the hole as a *real interior wire*). Per-panel, **single-layer**, open
gore. Certified, float-free, both directions. **Gaps G1–G7.**

### Stage 2 — cone + overlap seam
Take the Stage-1 rolled cone and **close it with a certified BONDED lap seam** — the gore's two radial edges
lap and bond across the full 2π closure. **Single-layer** (a single-layer bonded lap is certifiable before
the laminate). Adds, on top of Stage 1, the **transcendental 2π closure** and the **§14 BONDED joint
certificate** (SEP ≡ bond gap `g`, SLAB one-sided, two-to-one normal projection over the overlap; the
seam-ramp subdivision certificate of `docs/paper.md`). **Gaps S2 + S3.**

> **Scope assumption (open, flag to revisit):** Stage 2 is single-layer with a simple footprint. The
> **multilayer laminate**, **multi-panel atlas assembly**, and **complex authored ECAD boundary** are placed
> **beyond Stage 2**; any can be pulled forward.

### Beyond Stage 2 — the full flex-PCB
S5 multilayer stackup (4-layer laminate + bond gap, `z_N` strain budgets) · S4 atlas/Device multi-panel
assembly + S1 reflection-mate constructor · S6 complex authored ECAD boundary (board edge + cutouts,
free-form edges).

## Where we are (reusable substrate — DONE)

- **DEV.2** — per-panel `D`/`D⁻¹` (`develop::{cone,unroll,fold,anchor,interval}`), corroborated, float-free.
- **M4 CLOSURE + M5 SEW** — MONO edge-to-edge joints (`closure_valid = REG-V∧WEDGE∧…∧(MITER∨LEDGE)∧SEW`).
- **D4.1 assembly spine** — `certify_core::shell::closed_shell` (Kani-proven) + `valid_closed_solid`; the
  joint fold already supports **>1 joint** (`certify-core/src/gate.rs:143`).
- **Exact cone-band body** — `export::brep_build::brep_freeboundary` over `fixtures::devices::cone`
  (`brep_build.rs:516`, test `:966`), OCCT-corroborated — Stage 1's STEP substrate (~90%).
- **Exact 2D boolean** — `arrange2d::boolean::ledge_dom{,_certified}` (DCEL, Kani-proven); interior hole =
  `Xor` (`boolean.rs:1290`).
- **D24 boundary primitive** — `certify_core::cap_in::{Carrier,BoundaryComponent,cap_in_d24}`.
- **Offset-family identity** — `C(σ,μ,w;h)=X(σ,μ;h+w)` proven (`geom/src/chart.rs:267`) — the multilayer hook.

## Gap inventory (effort ∈ {small, moderate, milestone})

### Stage 1 — per-panel pipeline (critical path)

| # | Gap | Effort | Slot | Riskiest unknown |
|---|---|---|---|---|
| **G1** | Interval-trig **range reduction** (`cos_on`/`sin_on` outside [0,π]) → wide/two-sided gore | moderate | DEV.3-α | enclosure-width blow-up from subtracting the 2π-enclosure; mitigated by the bounded-`k` `[−π,3π/2]` reduction. `fold_point` is fixed *for free* (its `cross_at` calls `cos_on/sin_on`). |
| **G2** | Cut-curve **float oracle + exact rational fit** (cone∩cylinder μ̂(σ)). *Offset-plane cut is exactly rational — no fit.* | moderate | DEV.3-α (§14/CM long-term) | fit tight enough for `anchor_dev` at a real clearance; branch selection + σ-monotone arc split. |
| **G3** | **General trim-loop unroll** — ordered σ-monotone rational arcs → `FlatOutline` (reuse `rail_edge_eps`) | small–moderate | DEV.3-α | arc ordering/orientation. |
| **G4** | Certified **`fold_outline`** — whole flat loop (incl. hole) → 3D box-loop | moderate | DEV.3-α | per-vertex vs per-edge fidelity; recovering hole `(σ,μ)` for pcurves. |
| **G5** | **arrange2d hole glue** — `FlatOutline` + square → `Region{faces:[{outer,holes}]}`, loops carried for folding | small | small exporter slice | mapping `Region` loops back to σ. |
| **G6** | **Interior-hole + arbitrary-trim STEP B-rep** — Face holes in IR + buffers + OCCT shim; non-iso trim edges | **milestone** | new slot **D4.7 / E-EXPORT** (extends deferred V_∂ real-cut) | **pcurve-healing of the non-iso interior hole**. |
| **G7** | End-to-end **Stage-1 demo driver** + SVG-with-hole + STEP I/II | moderate | DEV.3-α acceptance example | pure glue. |

### Stage 2 — the overlap seam

| # | Gap | Effort | Slot | Riskiest unknown |
|---|---|---|---|---|
| **S2** | **Full 2π wrap / gore-seam closure** — the two radial edges live at σ→±∞ (transcendental); only the seam *position* exists (`cone.rs:377`). | milestone | **DEV.3-β** | transcendental closure; a rational chart is a gore <2π. Chart-graph **cycle** (closed wrap) is the deferred [D11] case. |
| **S3** | **BONDED lap-joint / seam overlap** — SEP (separation ≡ bond gap `g`) + SLAB + MATCH + two-to-one normal projection. Nothing exists (all joinery is MONO); reserved by design. | milestone | **spec §14 (BONDED)** | the **seam-ramp** certificate (constant-slope ansatz → scalar control, certified by interval subdivision over a ~60° Δφ box) — the single highest-risk unknown; rides on S2. |

### Beyond Stage 2 — the full flex-PCB

| # | Gap | Effort | Slot | Riskiest unknown |
|---|---|---|---|---|
| **S1** | **Reflection-mate constructor** — derive flank B = reflect(A) across the crease plane (`n_B=n_A−2(n_A·B/B·B)B`). Prose-only today; MITER certifies a pair, doesn't construct the mate. | small–moderate | D4.4 | identity check that constructed `n_B` is the reflection; pieces (bisector/wedge/`n·n=1`) exist. |
| **S4** | **Atlas / Device container** — assemble N charts + cross-joint seams (the >1-joint fold exists; container + cross-joint seams don't). | milestone | D4.4 | fold-vertices where >2 panels meet; chart-graph cycles deferred [D11]. |
| **S5** | **Multilayer stackup** — ordered `Vec` of `w`-bands + material tags over shared `q`; per-layer `z_N` + ±strain budgets; `Δ(σ)` midplane offset; doubled stack across a bonded lap. | moderate→milestone | new slot (laminate) | `z_N` calibration + strain budgets binding; multilayer + *curved* fold has no exact flat identification. |
| **S6** | **Authored substrate 2D boundary + cutouts** — real ECAD outline (multi-loop; free-form→conic/L3) replacing the rect footprint. D24 primitive exists; wiring as a *substrate* free boundary is the gap. | moderate | D4.3 (+ §14 curves) | multi-loop as a *certified* free boundary; free-form edges cross into the reserved conic-arrangement class. |

## Dependency DAG

```
DEV.2 per-panel D/D⁻¹  ✅
  ├─ STAGE 1   G1 ─→ G3 ─→ G5 ─→ G4 ─→ G6 ─→ G7        (G2 feeds G3)
  │            artifacts: A1 SVG → A2 SVG+hole → A3 folded mesh → A4 STEP I → A5 STEP II(=G6)
  │                │
  ├─ STAGE 2   S2 full-2π closure (DEV.3-β) ─→ S3 §14 BONDED lap     (on top of the Stage-1 pipeline)
  │
  └─ BEYOND    S6 authored boundary (D4.3) ; S1 reflection-mate + S4 Atlas (D4.4) ; S5 multilayer  ─→ ★
```

Stage 2's two gaps are the two **independent hard frontiers** — S2 (transcendental closure) and S3
(§14 BONDED) — and **S3 depends on S2** (the two radial edges must share a frame before they can lap).
Everything bonded rides on the **seam-ramp subdivision certificate**. Beyond-Stage-2 items are largely
independent of each other.

## Generality — do we need goal-specific hacks?

**No. It can be done the fully general way, and that is not luck — the founding doctrine makes it so.**
"Exactness is a *representation* property, not a *shape* property": transcendental/algebraic geometry is
handled by a **certified backward-error bound** (`anchor_dev`'s `sup|D(â)−g|≤ε`, DRC `ε<clearance/2`), not
by pretending it is rational. So "rational approximation of the cut curve" is **the designed general
treatment**, and it is **fail-closed** — a loose approximation can only yield `Unresolved`, never a wrong
`Verified`. Classification:

- **General, no hack:** G1 (bounded-`k` reduction is a *theorem about gores* — ψ<π per chart — not a
  demo shortcut), G3 (the arc-loop *is* the general outline model), G5 core (exact boolean on the certified
  polyline; ε honestly accounted), S1/S4/S5/S6 (all the real mechanisms).
- **General by doctrine + a sound bridge:** G2 — certified-ε is the product semantics; the only shortcut is
  letting a **float oracle *propose*** the rational coefficients, which the exact certificate re-verifies
  (fail-closed). The truly-general endpoint is exact algebraic intersection (§14/CM), a drop-in replacement
  that leaves the certificate untouched.
- **Conscious general-over-shortcut choices (recommended, to avoid rework):**
  - **G4** — build **per-edge** fold, not per-vertex (per-vertex leaves the edges between vertices
    uncertified; per-edge is the inverse of `anchor_dev`).
  - **G6** — do the hole via **explicit pcurves from the recovered (σ,μ)** (the RationalPatch's parameter
    space *is* (σ,μ)), not by re-ruling to dodge non-iso trims (a demo hack that won't generalize to
    arbitrary cuts — and the interior hole is irreducibly non-iso, forcing the general path anyway).
- **Bounded scope, extensible (not hacks):** interior-only square cut (boundary-crossing cuts add a `Sub`
  op + `Surd`-in-fold); per-gore range reduction.
- **General-or-nothing (cannot be faked):** S2 + S3. No sound single-chart shortcut exists for the
  transcendental closure; any shortcut is *uncertified visual*. This is why Stage 2 is a milestone.

**One genuinely new proof technique enters at Stage 2:** **certified interval subdivision** for the seam
ramp. Everything so far is closed-form (Sturm / resultant / interval-series); the seam is the one place the
paper hands off to subdivision. Sound and general, but a real methodological broadening — hence the spike/GO
gate. **Float stays quarantined throughout** (the G2 oracle lives in `export`, only proposes, never touches
a certificate; `certify_core`/`develop`/`lattice` stay pure — the no-float-certified invariant holds).

## Phased sequence (recommended)

- **Phase 0 (docs-only):** this roadmap + the `vv-guide` pointer + the engineering-log entry + memory. Done.
- **Phase 1 = STAGE 1 (G1→G7).** Splits DEV.3 into **α (per-panel pipeline, wide gore — reachable now)** and
  **β (transcendental closure)**. Ships the A1→A5 artifact ladder + the **G6 exporter milestone** (D4.7 /
  E-EXPORT). Highest value, lowest new-theory risk. **Recommended next.** Start with G1 (cheapest slice).
- **Phase 2 = STAGE 2 · closure (S2 = DEV.3-β).** The transcendental 2π wrap; spike-first / GO-gated (the
  chart-graph-cycle / [D11] risk).
- **Phase 3 = STAGE 2 · bonded seam (S3 = §14 BONDED).** SEP/SLAB/MATCH + the seam-ramp subdivision
  certificate; spike-first / GO-gated (highest-risk unknown in the roadmap).
- **Beyond (own GO gates, parallelizable):** D4.3 authored boundary (S6) · D4.4 Atlas + reflection-mate
  (S1+S4) · multilayer stackup (S5) → the full multilayer flex-PCB acceptance demo.

## Cheapest fallbacks for the riskiest gaps

- **G6 pcurve/non-iso trims:** (i) re-rule the retained region so *outer* trims are `ruled_from_rails`
  iso-boundaries (the hole is then the only non-iso loop); (ii) explicit pcurves for the hole
  (`BRep_Builder::UpdateEdge`) vs trusting `ShapeFix`; (iii) cheapest — ship STEP II as a **mesh** (A3),
  dropping the "real interior wire" property.
- **G2 fit:** exact offset-plane cut first, defer cone∩cylinder; long-term replace the fit with exact
  **CM/§14 conic** intersection.
- **G4 fidelity:** per-vertex first, per-edge is the DEV.3-grade upgrade (but see the generality note —
  per-edge is the non-shortcut choice).
- **G1:** bounded **`[−π, 3π/2]`** reduction covers the demo gore.
- **S2/S3:** Stage 2 stays **single-layer**; a single-layer bonded lap is certifiable before the laminate.

## Critical files (by gap)

- **develop (pure tier):** `interval.rs` (`cos_on:326`/`sin_on:338` — G1), `unroll.rs`
  (`unroll_freeboundary:117`, `rail_edge_eps:65` — G3), `fold.rs` (`fold_point:143`→`fold_outline` — G4),
  `anchor.rs` (`anchor_dev:102` — consumes G2 rails), `cone.rs:377` seam position (S2).
- **export:** new `cut_oracle.rs` (`diagnostics`, G2), new `flat_hole.rs` (G5), `brep.rs` (`Face:107` holes
  — G6a), `step.rs` (`BrepBuffers:192` multi-wire — G6b), `occt_shim.cc` (`MakeWire:252` + `MakeFace::Add`
  — G6c), `bezier.rs` (`ruled_from_rails:281` reused), new `examples/cone_demo.rs` (G7); `brep_build.rs:516`
  reused for STEP I.
- **closure/sew/certify_core/geom (Stage 2 + beyond):** new §14 BONDED checkers in `certify_core`
  (S3, SEP/SLAB per `docs/paper.md`); DEV.3-β closure over `develop::cone` (S2);
  `closure/{wedge,miter,ledge}.rs` + `certify_core::miter` (reflection-mate, S1); new `Atlas`/`Device`
  type + `certify_core::gate:143` fold (S4); `geom/src/chart.rs:267` + a new stackup type (S5);
  `certify_core::cap_in` + `free_boundary.rs` (S6).

## Doctrine (unchanged)

No float in any certificate (float only behind `export`'s `diagnostics`/`step`); `develop`, `certify_core`,
`lattice` stay pure/no_std/no-float TCB; `-D missing_docs`. Each slice commits green on a feature branch;
the gate is merge-to-main (per-action push/merge confirmation). Each Stage is the end-to-end **acceptance
test** for its phase, corroborated by a float oracle (`mesh3d::develop_cone`, the new `cut_oracle`), never
*certified* by it.
