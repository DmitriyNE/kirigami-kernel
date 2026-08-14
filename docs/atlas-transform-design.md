# The atlas and the transform — product flow, rationale, milestones

*Design pass, 2026-08-14. **Supersedes [`construction-api-design.md`](construction-api-design.md)**
(the `Part`/facade design, PRs 1–3 of which shipped); its still-live decisions are carried forward in
*Inherited decisions* below. Measured numbers cited here are recorded in
[`engineering-log.md`](engineering-log.md) under OPT.0 / OPT.1 / VV.1–VV.3 — this document does not
duplicate them, it reasons from them.*

The construction-API doc answered *how do you author one part*. That question is largely settled and
the answer shipped. This document answers the next one, which is bigger and reframes what the kernel
is optimizing for: **what does the full flex-PCB flow need, and what object sits at its centre?**

The short answer: a **frozen atlas** plus a **certified embedding map** over it. Everything
downstream — ECAD transfer, folding, meshing, strain, design optimization — is an evaluation of that
map rather than a fresh solve.

## 1. The product flow

Stated by the user, 2026-08-14; this is the spec the rest of the document serves.

1. **Define the outline — the atlas.** Chart boundaries, sewing. Boundaries are authored with ops in
   3-D *and* in 2-D developed coordinates, moving back and forth. The current use case: take a body
   like the self-lapping cone fixture, drill and cut pieces from it in 3-D using several sketches
   (2-D arrangements in different planes, extruded cuts, possibly with **draft angles**), then
   unwrap. Few iterations expected. Chart geometry may later change under optimization feedback, but
   **nothing drastic — no topology change**. After this the atlas is **frozen**.
2. **Author ECAD** — traces, drills, cutouts. These do **not** change atlas topology.
3. **Bridge and map.** Ingest IPC-2581 (or similar), extract geometry, simplify it with the 2-D
   arrangement engine, map into 3-D. Conversion modes select copper only, dielectrics only, or
   everything. *A working prototype already does this in Python over CGAL + OCCT.*
4. **Fold into the required shape.** This is a useful product on its own. Beyond it:
   - **(a) Transformation-informed FEM meshing.** A PCB is far easier to mesh flat, but the mesh is
     needed for the folded shape — so either mesh in 3-D informed by the transform, or mesh in the
     developed frame and map + refine.
   - **(b) Deformation and strain fields** to initialize a structural FEM, which then solves for the
     accurate shape accounting for stackup and mounted components.
   - **(c) Material-distribution optimization** (copper, stiffeners) authored in the developed frame
     and optimized for target deformations, antenna radiation patterns, and similar.

**Workload scale.** ~4 copper layers plus coverlays and dielectrics (all of which must be transformed
for simulation), 100–150 nets, and **tracks are geometrically fancy** — high vertex counts, not
simple rectangles.

## 2. What the flow implies

Four consequences, each of which changes a design choice.

**The frozen atlas inverts the cost model.** Step 1 runs once; steps 2–4 run constantly, and 4c runs
them in a loop. So **atlas construction cost is nearly irrelevant and evaluation cost is everything**.
Minutes to build an atlas is fine. Milliseconds per transformed point is not optional. Every
optimization to date has been measured against a whole-pipeline demo, which is the wrong yardstick.

**Meshing sets the bulk-transform requirement.** A FEM mesh is 10⁵–10⁶ vertices — an order of
magnitude past nets. Any design that solves per point independently is disqualified at 4a regardless
of its constant factor.

**Strain fields need derivatives, not just points.** A deformation gradient cannot be recovered from a
bisection-based point solve except by finite differencing, which is noisy and carries no honest error
bound. *But* — see §4 — it does not follow that the map must be a certified C² object. That is the
single most useful simplification in this document.

**Optimization loops need a reusable, re-certifiable atlas.** 4c perturbs geometry and rebuilds many
times without changing the atlas. That demands the atlas be an explicit value with a cheap
re-certification path, and it demands a discipline about when reuse is legitimate (§5).

## 3. Performance targets, derived

From the measured post-OPT.1 fold cost of **136 ms/point** (engineering log, OPT.1):

| workload | points | today | for 10 min | for 1 min |
|---|---|---|---|---|
| 150 nets × ~500 vertices | ~75 k | ~2.8 h | **~17×** | **~170×** |
| a modest FEM mesh | ~10⁶ | ~38 h | **~230×** | **~2300×** |

Scale linearly with real vertex counts. Two things follow. First, the required factor is **not**
reachable by constant-factor tuning of the current per-point solve. Second, the dominant stage for
this product is **fold** (2-D authored geometry → 3-D), not `develop` — `develop` runs once per layer
outline. Prior optimization work targeted the whole demo and therefore split its attention roughly
evenly across stages that matter very unevenly here.

## 4. The design

### 4.1 The search is not the certificate

The load-bearing observation. `fold`'s `invert_sigma` bisects because it must *find* σ. But what makes
a folded point certified is the **backward-error residual**: enclose the forward map at the candidate
and check it lands on the authored point within ε. That check does not care where the candidate came
from.

So the candidate may come from anything — a float Newton solve, a fitted inverse, a cached neighbour —
and the guarantee is unchanged. This is the repo's existing doctrine (*float search → certify exactly*,
already used for non-primitive 3-D placement) applied one level down. It is also what makes the whole
program incremental rather than a rewrite: the certification path is shared by every tier below.

### 4.2 Two tiers

**Tier 1 — swap the search, keep the certificate.** Candidate from a cheap float solve; existing
residual check unchanged.

> **Corrected by measurement, 2026-08-14 (MAP.1 shipped).** The projection here was ~50× on the
> reasoning that ~50 bisection steps collapse to one evaluation. Measured: **1.16×** (158.0 →
> 136.8 ms/pt on the acceptance outline, identical ε). The bisection is *not* the dominant cost of a
> fold point — OPT.1 had already removed the γ cost that made it appear so, and what remains is the
> region trials, the lift, and the round-trip re-development that **is** the certificate.
>
> **This bounds Tier 2 as well**: a fitted map replaces the same search, so its fold speedup is
> capped at roughly the same factor. The certified fold has a floor set by residual certification,
> and reaching §3's targets requires making the *enclosure evaluations* cheaper (the float-filter
> lever), not eliminating the search. Tier 2 keeps its other three justifications below — it is the
> ECAD artifact, it amortizes across the stackup, it is what an optimization loop re-certifies — but
> it is **not** the fold's performance answer. See the MAP.1 entry in the engineering log.

**Tier 2 — the certified embedding map.** Per region, a patchwise approximant of the flat ↔ domain map
with a rigorous sup-norm bound, built once at atlas-freeze time. Transforming a point becomes: locate
the patch, evaluate a low-degree polynomial, add the patch's certified error to ε. O(1) per point, no
search.

Tier 2 is more than an optimization:

- **It is a deliverable.** A certified flat↔flat transfer map is the ECAD artifact itself — the thing
  handed to a simulator or a fab, not merely an internal speedup.
- **It amortizes across the stackup.** With `w` carried as a parameter, coverlays and dielectrics
  reuse one construction instead of re-solving per layer.
- **It is certifiable with machinery already present.** The bound is a backward error over a patch —
  the `cut_fit` / ANCHOR discipline: enclose the forward map over the patch, bound the residual,
  subdivide where it fails.

### 4.3 The map only has to *locate* (σ, µ̂)

This is what keeps 4b tractable, and it is worth stating as an explicit design decision because the
obvious alternative — building a certified C² approximant — is much harder and unnecessary.

The chart carries **exact rational** pedal / ruling / normal fields. Once (σ, µ̂) is known, the 3-D
point, the tangent frame, and the curvatures all follow analytically from the chart. And because the
mid-surface map is a certified **isometry**, the composite deformation gradient is orthogonal there by
construction; the interesting strain is the bending term through the thickness, which is curvature ×
offset — and curvature is exactly what the chart provides.

So the fitted map needs **value accuracy only**, not certified derivative bounds. Differential
quantities are read off the chart at the located coordinates. 4b becomes bookkeeping over fields that
already exist exactly.

### 4.4 Atlas lifecycle

The atlas must be an explicit, held value rather than something implicit in a call stack. Four
operations:

- **build** — expensive, once: patch decomposition, branch designation, fitting. Cost is not a
  design constraint (§2).
- **freeze** — the structure becomes immutable; downstream artifacts may reference it.
- **transform** — bulk, cheap, the hot path. Batch-oriented (§4.6).
- **re-certify** — given perturbed geometry, re-derive the bounds over the *existing* structure. Pure
  evaluation, no search or fitting; this is what an optimization loop runs thousands of times.

### 4.5 Reuse must be verified, never assumed

The failure mode of a cached atlas is a silently invalid one — "the change was small, so it still
holds" is exactly how a confidently wrong certificate gets produced. The rule:

> Reuse the atlas's **structure** freely. Re-derive its **bounds** every time. Fail closed when a
> patch stops certifying — refine locally, or rebuild that patch.

Re-certification is cheap by construction, so this costs little and preserves the fail-closed doctrine
the kernel is built on. It also gives an optimization loop a natural early-out: a design whose residual
exceeds the DRC gate is rejected after evaluation, without ever fitting or solving.

### 4.6 Bulk transform API shape

Driven by 4a and by the likelihood of a process boundary (§6): **batch-oriented**, taking and
returning slices rather than single points, with no per-point FFI crossing and no per-point
allocation. Certification reports a bound per batch (and per patch), not a `Verdict` per point.

## 5. Milestones

Numbering continues the repo's convention; each lands under the usual gate (tests, doctests, fmt,
clippy `-D`, `xtask lint`) with a GO-gate doc where the scope warrants it.

| ID | Milestone | Depends on | Notes |
|----|-----------|-----------|-------|
| **MAP.1** | Search/certificate split — fast candidate + residual certification in `fold` | — | Small, local; validates residual-only certification. Make the candidate source an **abstraction**, not a hard-wired float solve, so Tier 2 drops in without retrofit. |
| **MAP.2** | The certified patch map (Tier 2) | MAP.1 | Patchwise approximant + sup-norm bound; subdivision where the residual fails; branch designation per patch. Deserves a GO-gate — it is a product artifact. |
| **MAP.3** | Atlas lifecycle — build / freeze / hold / re-certify | MAP.2 | The explicit atlas value + cheap re-certification + fail-closed invalidation (§4.5). |
| **BULK.1** | Batch transform API (+ boundary if the proto stays Python) | MAP.1 | Slice-in/slice-out; per-batch bounds. Shape it as if external (§6). |
| **AUTH.1** | `Cutter::Extrude` + **draft angles** | — | Step-1 blocker: "2-D arrangements in different planes + extruded cuts". Draft makes the cutter tapered/ruled, not a prism — more than the sketch-extrude previously scoped (D3). Independent of the MAP line. |
| **FEM.1** | Mesh transfer — mesh flat, map + refine | MAP.2, BULK.1 | Refinement driven by the patch distortion field the atlas already carries. |
| **FEM.2** | Deformation + strain fields | MAP.2 | Per §4.3, read from the chart at located coordinates. |
| **OPT.3** | Design-optimization loop support | MAP.3 | Parameterized recipes; early-out on DRC; structure/numerics split. |
| **OPT.2** | The remaining non-γ per-node cost (open, #233) | — | Independent of this line; lead is `develop::cut`'s absent outward rounding. Lower priority now that the map removes the per-point solve it would have accelerated. |

**Order.** MAP.1 → MAP.2 on the existing self-lapping fixture (close enough to the target shape to
validate against), with AUTH.1 when the real outline is needed rather than the fixture. FEM and OPT.3
are consumers and come after. MAP.1 is not a detour: it *is* Tier 2's kernel, differing only in where
the candidate originates.

## 6. Open decisions

- **Repo layout — absorb the prototype, or keep the kernel as a component?** The prototype's
  brep construction is already superseded by the kernel's; the genuinely new part is IPC-2581
  ingestion, which is a format adapter and belongs **outside the trust boundary** either way.
  Recommendation: keep the kernel narrow and certified, with an application layer owning ingestion,
  meshers and FEM drivers. A single repo remains viable *because* tier boundaries are already
  enforced mechanically (pure/shell tiers, `no_float`, panic-freedom discharge, vv-matrix) — an
  enforced "app tier" would extend that. Avoid git submodules; use a real dependency. **This decision
  does not block MAP.1–MAP.3**, which are identical under either layout.
- **The Python boundary.** If the prototype stays Python near-term: PyO3, or a CLI plus a
  serialization format for the atlas and point batches? This shapes BULK.1.
- **Acceptance runtime target.** What counts as acceptable for a full stackup — a minute, or ten?
  §3 gives the factor each implies.

## 7. Inherited decisions (carried forward from the construction API)

Still live and unchanged:

- **[D2] The witness doctrine + product coordinates.** Discrete geometric choices are *inferred* when
  unambiguous, *fault typed* when not (`PartFault::AmbiguousRegion`) — never guessed — and recorded as
  exact discrete data. The resolution mechanism is an implementation detail; its **conclusiveness is
  part of the contract**. The facade speaks product coordinates; the core speaks (σ, µ̂). **Covariance
  rider:** the core is covariant in (σ, µ̂), with exact Möbius reparametrization `σ′ = (aσ+b)/(cσ+d)`
  first-class in the future — recipes record chart + σ together so a reparametrization transforms them
  mechanically. *This matters more now, not less: an atlas is exactly the object a reparametrization
  must transform.*
- **[D3] Material ops with solid cutters.** `subtract`/`intersect` with solid `Cutter`s; roles and
  root picks derived in-domain. Every quadric pulls back to a degree-≤2 algebraic rail in µ̂ over ℚ(σ).
  `Extrude` is the flagship and is now on the critical path as **AUTH.1**.
- **[D1] Declarative recipes** (builders total; certification at the evaluators), **[D4] the `author`
  crate**, **[D5] consuming builders** — all shipped and unchanged.

Superseded: the construction-API doc's *Build steps* (PRs 1–3 shipped; its PR 4 is now **AUTH.1**) and
its framing of the pipeline as per-part evaluation, which §2–§4 replace.
