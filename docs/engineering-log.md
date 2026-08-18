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

- **Certification runtime is product-blocking, and it is no longer a constant-factor problem
  (#279).** Raised by the user while an AUTH.3c probe ran: *"These running times are not acceptable
  for the real-world use."* The numbers that make it a scope item rather than a grumble: after
  OPT.0–OPT.3 — which removed the 128-bit software division, **60%** of runtime — a **small**
  fixture still costs tens of seconds per pipeline run. Small input, that much work, obvious
  constant factors already gone: the pattern points at coefficient growth rather than a hot loop.

  **What the honest numbers are, and are not.** The figures to hand are *post*-opt (this session's
  `test`-profile run) but they are **test wall-clock**, not per-call: `the_same_footprint…` is `93s`
  for **3 develops + 1 fold**, `only_the_declared_band…` is `258s` for **2 develops + 1 fold**, and
  both ran under nextest across 298 tests on 10 cores, so contention inflates them. Per `develop()`
  that is tens of seconds, not minutes. The first thing #279's triage owes is a **clean single-run
  measurement** — one call, no contention, stated profile. Recorded because the user caught me
  quoting a test timing as a call timing: *a suite number and a call number differ by both the call
  count and the parallel load, and neither correction is small.*

  ⚠️ **Correction to this entry as first written.** It also cited a `solid()` probe that had not
  returned after 14 minutes. That number is **not** evidence of coefficient growth and must not be
  cited as such — `sample`-ing the process at 30 min put **100%** of stacks in `sigma_splits::go`,
  i.e. it was the unbounded-subdivision hang (#280), a different defect. The runtime concern stands
  on the 93s/258s figures alone. Recorded because the mistake is the instructive part: *a slow
  number and a hung number are indistinguishable without observability, and I generalized from one
  to a cost model before sampling the process.* Sampling took ten seconds and settled it.

  **⚠️ The triage ran early (2026-08-17) and refuted this entry's own hypothesis.** Per-stage,
  release, single run, no contention (`author/examples/auth3c_probe.rs`):

  | fixture | develop | outline pts | solid | fold |
  |---|---|---|---|---|
  | square, polygon corner | `1.20s` | 196 | `0.06s` | `1.22s` (6.2 ms/pt) |
  | cylinder, sole-bound | `3.21s` | 192 | `0.08s` | `1.15s` (6.0 ms/pt) |
  | **cylinder, whole-side** | **`175.68s`** | **6386** | `2.30s` | — |
  | lateral trim (control) | `0.48s` | 98 | `0.04s` | Verified, 6 faces |

  **The driver is emitted vertex count, not arithmetic.** Per-point cost is 6–27 ms and roughly
  flat; the *count* is 33× on one shape (#281). Two explanations were tested and both died: the
  **build profile** costs ~8% (same binary with `debug-assertions` + `overflow-checks` on: 1.28s vs
  1.20s, 3.47s vs 3.21s — so keeping them on in `[profile.test]` is nearly free, and the earlier
  "20–35× profile gap" reasoning was wrong), and **coefficient growth** would show as *rising*
  per-point cost, which it does not. The one slow fixture explains test 2's `258s` almost exactly.

  So the coefficient-histogram plan below is **not** where to start; it stays as a later check if a
  per-point trend ever appears. The lever that measurement actually points at is *how many vertices
  a boundary shape emits*, which is #281. Kept in full because the reasoning was plausible and
  wrong, and the correction cost one afternoon of measurement rather than a rewrite of the
  arithmetic tier: **a cost model asserted before a per-stage measurement is a guess with a table
  around it.**

  The original suspects, now demoted to "check only if per-point cost rises": `reduce()`'s
  polynomial gcd over degree-24 denominators, Sturm chain construction, resultant/discriminant
  formation in `develop::cut`, the tracer's per-segment rail fits.

  Three candidate answers, to be chosen on that measurement and not before: bounded-precision
  dyadic arithmetic with outward rounding pushed further up the pipeline (the enclosure tier
  already does this with `RatIv` + `ROUND_BITS`); modular / evaluation-interpolation for the
  resultant and gcd work; or an explicitly **uncertified float preview tier** with certification run
  once at the end — which is the split an interactive ECAD user actually needs, since authoring and
  the final certificate have different budgets. #257 folds in as a subset. The standing constraints
  do not move: the float quarantine (lattice + certify-core only, exact-stays-exact, approx opt-in
  under the five-part contract) and `no_repr_leak`. A preview tier has to be a *differently typed
  path*, never a quiet precision drop inside a certified one.

  **Deprioritized 2026-08-17, by the user, on those numbers:** *"4-9 ms per point is kinda fine for
  a beginning, at least we can parallelize it in future. I don't expect interactivity right now.
  Plus we have MAP planned, which will hopefully solve parts of the problem."* Per-vertex work is
  embarrassingly parallel, and MAP.2 (#235) would cut the fold leg **structurally** rather than by a
  constant factor — so #279 stays open but is not the next milestone. Recorded so it is not
  re-escalated off a single slow fixture; the correct response to one of those turned out to be
  #281, a bug fix.
  *2026-08-16 · open, deprioritized 2026-08-17 · #279*

- **AUTH.1 deferrals — scope decisions, taken with the user, not oversights.** Recorded so the
  narrower first slice reads as a choice: **(a) per-edge draft slope** — a single cast point forces
  one projective taper and cannot give edge A 5° and edge B 0°, which is real fab practice; wanted
  later. **(b) p-curve profile edges** — lines and arcs keep every wall a plane-or-quadric (degree
  ≤2 over ℚ(σ)); admitting the PC p-curves pushes walls past degree 2 into new certificate
  territory. **(c) per-generatrix span** — each generatrix terminating on its own hit count, so cut
  depth varies across the profile; the reference-ray ordinal ships first. **(d) cutting a real
  stackup** per-layer — cuts currently happen at flow stage 2, *before* a stackup exists, so a span
  counts neutral surfaces; nothing in the span rule forecloses layers later.
  *2026-08-15 · deferred(→post-AUTH.1) · `docs/cutter-extrude-design.md` §8, #237*

- **Two CI gaps the OPT.3 pre-push gate exposed — neither is a code bug, both hide real failures.**
  **(1) The self-hosted Linux runner is dead in the water** (#241): `nix-installer-action` hangs and the
  job dies at the 6 h cap before running a single gate step, so half the matrix — and the *only* leg
  covering `x86_64-linux` — reports nothing. Fix the runner or pin the installer action. **(2) Nothing
  locally type-checks feature-gated code** (#242): `--features step` / `--features cgal` / `--features
  fuzzing` targets are compiled out of the default `--workspace` legs, which is how a PC.5/PC.6 call-site
  break survived two weeks (see Findings). Cheapest fix is a `cargo check --workspace --all-targets
  --all-features` in the fast loop; the fuller one is a `cargo xtask gate` that replays the CI step list
  so "run the gate" is one command instead of a hand-assembled script.
  *2026-08-15 · open · `.github/workflows/ci.yml`, #241, #242*

- **The driving requirement (product north star): the bidirectional multilayer flex-PCB transform.** The kernel
  exists to (**① develop** 3D→flat — generate the flat PCB outline by intersecting the generating shape with 3D
  geometry, then unroll) and (**② fold** flat→3D — fold flat ECAD data into folded 3D geometry). Framing + the
  exact-vs-transcendental mapping now live in `docs/implementation-plan-v1.md §6`. Two first-class threads fall out,
  and neither is a tail deferral: (a) **certified development** (the flat↔3D isometry, the `develop` crate / M-E) —
  the keystone *both* directions pivot on, today only a float diagnostic (`export::mesh3d`); (b) the **exact-
  intersection → outline** path (direction ①) — `resultant`/`AlgReal`/`arrange2d`/CLIP produce the PCB outline as a
  3D-intersection result, feeding the free-boundary emit machinery (`export::brep_build::brep_freeboundary`) already
  built in M-D. Multilayer = the `w` thickness dimension (native to the chart; layers at distinct `w`-offsets).
  *2026-08-10 · open (product framing recorded; DEV reprioritized in Deferred) · `docs/implementation-plan-v1.md §6`*

- **Flex-PCB roadmap — two acceptance targets, and the "no goal-specific hacks" decision.** With the user,
  the path from post-DEV.2 to the product spine was restructured around **two concrete end-to-end acceptance
  demos** that gate the milestones (full detail `docs/roadmap-flex-pcb.md`; `vv-guide.md` Milestone E → "Flex-PCB
  acceptance roadmap"). **Stage 1 — cone-sector back-and-forth:** a ~300° (rational-approx) cone sector, cut
  by an offset-plane curve (exactly rational) + a fitted cone∩cylinder curve → unroll → a square interior
  hole (exact `arrange2d` boolean) → fold back → SVG + two STEPs (input cut cone; folded panel with the hole
  as a *real interior wire*). = **DEV.3-α** (per-panel pipeline on a wide/two-sided gore) + one exporter
  milestone (**D4.7 / E-EXPORT** interior-hole/arbitrary-trim B-rep, extending the deferred V_∂ real-cut).
  Gap ladder **G1–G7**, artifact ladder A1 SVG→A2 SVG+hole→A3 folded mesh→A4 STEP I→A5 STEP II. **Stage 2 —
  cone + overlap seam:** close the rolled cone with a certified **BONDED lap seam** (the original device's lap
  seam, `implementation-plan-v1.md:53`) = **DEV.3-β** (transcendental full-2π closure; seam at σ→±∞;
  chart-graph cycle = [D11]) + **spec §14 BONDED** (SEP≡bond-gap `g`, SLAB, two-to-one projection; the
  seam-ramp subdivision certificate of `docs/paper.md`). S3⊳S2. Beyond Stage 2: multilayer stackup, atlas
  (D4.4) + reflection-mate, complex ECAD boundary (D4.3). **The generality analysis (user asked):** it can be
  done **fully general, no goal-specific hacks** — "exactness is a representation property, not a shape
  property," so the certified backward-error bound (`anchor_dev`, DRC `ε<clearance/2`) makes rational
  approximation the *designed*, **fail-closed** treatment (loose → `Unresolved`, never a wrong `Verified`),
  not a shortcut. Two conscious general-over-shortcut choices in Stage 1: certified `fold_outline`
  **per-edge** (not per-vertex — else inter-vertex edges are uncertified) and the hole via **explicit (σ,μ)
  pcurves** (not re-ruling to dodge non-iso trims). Bounded-but-extensible scope (not hacks): interior-only
  square cut; per-gore range reduction. **General-or-nothing:** S2+S3 have no sound single-chart shortcut —
  any "shortcut" is uncertified visual, which is why Stage 2 is a milestone. **One genuinely new proof
  technique enters at Stage 2:** certified **interval subdivision** for the seam ramp (all prior work is
  closed-form Sturm/resultant/interval-series) — spike/GO-gated. Float stays quarantined (the G2 cut oracle
  lives in `export`, only proposes, never touches a certificate). Sequence: Phase 1 = Stage 1 (start G1,
  interval-trig range reduction) → Phase 2 DEV.3-β closure → Phase 3 §14 BONDED → beyond. **G1 met** —
  generic **mod-2π range reduction** in `develop::interval`: `cos_on`/`sin_on` now certify **any** real
  angle (the two-sided/shifted cone gore, `ψ = c·arctan σ` crossing 0), not a fixed window. Decision with
  the user (over an arbitrary `[−π,π]`/`[−π,3π/2]` window — which would bake in "open gore, centred at 0,
  sub-π", a goal-specific hack): fully general via reduction, using the `exp(iθ)` symmetries. Point
  evaluators reduce into `[−π,π]` with a certified integer `k` (`k=0` fast path for `|ψ|<π` — byte-identical
  to the old `[0,π]` result, so no regression); interval evaluators clamp to the exact `±1` at an enclosed
  extremum (over-approximated ⇒ always sound since cos,sin ∈ [−1,1]) else the monotone endpoint hull, and
  `[−1,1]` once θ spans a full period. `fold_point`'s bisection is fixed *for free* within a gore (the
  `span < π` precondition now documented in `invert_sigma`; a two-sided domain reaching span π splits at
  σ=0 — future G4). Multi-period soundness sweep (≈[−4π,4π]) + straddle/shifted/large-argument/regression
  tests. Full gate green (nextest ws 441, export 21, develop doctests, clippy `-D warnings`, fmt,
  `xtask lint`, no_std thumbv7em `lattice`+`certify-core`). **G2 met** — the **cut-curve fit certificate**
  (turn a cutting *surface* into a certified rational rail `μ̂(σ)`). **G2a** `develop::cut` (pure, no float):
  a **new sibling checker** `cut_fit` (not an `anchor_dev` reuse — the residual `F(C(σ,μ̂(σ)))` is purely
  rational in σ, no `cos/sin`), certifying `sup_σ dist(C(σ,μ̂(σ)), {F=0}) ≤ ε` by direct **geometric
  distance** (plane `|n·C−d|/|n|`; cylinder `|√perp2−R|`, `perp2=|C−p|²−((C−p)·â)²/(â·â)`), DRC
  `ε<clearance/2`; `CutSurface{Plane,Cylinder}` = the 3-D lift of `cap_in::Carrier`; `plane_cut_rail` = the
  **exact** offset-plane rail `(d−n·pedal)/(n·ruling)` (verified ε≈0). Founding split: the rail is on the
  cone *by construction*, so only surface-membership is certified. **G2b** `export::cut_oracle` (behind
  `diagnostics`): the **float oracle** `fit_cut_rail` proposes a rational fit for the surd cone∩cylinder cut
  (Chebyshev-node quadratic-branch solve → Vandermonde interpolation → coeffs snapped by the new
  `approx::f64_to_rat`, the reverse of the exact→f64 bridge); float **proposes**, `cut_fit` **decides**
  (fail-closed). **FINDING:** the certified `ε` is an interval upper bound, so its refinement handle is
  **`subdiv`** (as in `anchor_dev`/`unroll`), *not* fit degree — interval-Horner dependency overestimation
  of a high-degree σ-polynomial rail *grows* with degree, so a higher-degree fit tightens the *true* error
  but can loosen the *certified* bound; the demo picks a moderate degree + adequate `subdiv`. Full gate green
  (nextest ws 446, export/diagnostics 47 incl. corroboration, develop+export doctests, clippy `-D warnings`
  (default + `-p export --features diagnostics`), fmt, `xtask lint`, no_std thumbv7em). *2026-08-10 · roadmap
  authored (docs-only) + **G1 · G2 met**; branch `roadmap-flex-pcb` · `docs/roadmap-flex-pcb.md`,
  `crates/develop/src/{interval,cut}.rs`, `crates/export/src/{approx,cut_oracle}.rs`*

- **G3 met — general trim-loop unroll (`develop::unroll::unroll_trim_loop`).** Generalized the flat-pattern
  unroll from the two-rail **band** to an **arbitrary ordered loop** of `BoundaryArc`s — σ-monotone **rail**
  arcs `μ̂(σ)` (incl. the G2 cut rails) joined by ruling **cap**s. Each rail arc develops to a chord polyline
  certified against the true developed rail by the same DEV.2c `rail_edge_eps` lift bound; caps develop to
  *exact* straight radial edges (no fidelity cost); the loop must chain **end→start in `(σ, μ̂)`** (checked
  exactly, float-free — development is injective on the gore, so `(σ,μ̂)`-equality ⟺ coincident flat points),
  refused as `ArcDiscontinuity` otherwise; `ε = max` over rail edges, DRC `ε<clearance/2`, fail-closed
  (loose→`Unresolved`, structural→`Refuted{DegenerateSpan,PoleInEval,ArcDiscontinuity,EmptyLoop}`). **DESIGN
  (user steer, see [[no-interface-ossification]]):** rejected the "keep the special-case `unroll_freeboundary`
  frozen because an example/test pins its interface" instinct — that ossifies an unsettled kernel. Instead
  made `unroll_trim_loop` the **one canonical engine** and reimplemented `unroll_freeboundary` as a **thin
  delegating constructor** (band = the 4-arc loop `[Rail μ⁻, Cap, Rail μ⁺, Cap]`), deleting the duplicated
  develop/ε/assembly body — single source of truth. The `cone_flat` example + `mesh3d` corroboration stay
  green by *identical output* (a consequence, not a constraint). **Precondition (documented, not enforced):**
  no arc crosses the apex `μ̂=0` (the development uses `|μ̂|·ρ`); a real cut region never does. New tests:
  band≡explicit-loop, triangle (2 rails + cap, ε↓ with segments), a `plane_cut_rail` loop (G2→G3
  composition, Verified/Unresolved), a two-sided gore (σ across 0, exercises the G1 range reduction),
  open-loop/pole/empty/degenerate refutations, corner-enclosure corroboration. Full gate green (develop 63 +
  6 doctests, export/diagnostics 47 incl. corroboration, clippy `-D warnings` default + `-p export --features
  diagnostics`, fmt, missing_docs=0, `xtask lint`, no_std thumbv7em). *2026-08-10 · **G3 met**; branch
  `roadmap-flex-pcb`, `crates/develop/src/unroll.rs`*

- **G5 met — arrange2d hole glue (`develop::flat::cut_hole`).** Cut an authored interior hole out of a G3
  `FlatOutline` via the exact 2-D boolean kernel — the **first** `develop` code to actually wire in `arrange2d`
  (the dep was declared + charter-named but unused). Adapter `outline_to_edges` reduces each `FlatBox` vertex
  to its rational `center()` and lifts to `Point2::from_rat` (exact over ℚ — no float; outline stays
  ε-faithful, the hole is placed exactly on that rational polygon). `cut_hole(outline, hole)` builds outline =
  operand A (src 0) + hole = operand B (src 1) and runs `ledge_dom_certified(…, BoolOp::Xor)`. **KEY: no
  `BoolOp::Difference` exists** (only `Xor/And/Or`); for a *strictly-interior* hole `A △ B = A ∖ B`, so `Xor`
  is the in-tree convention (`fixtures::gallery::square_with_hole`). "Strictly interior" is **not assumed** —
  the checker certifies the postcondition **one face ∧ one hole ∧ no pinch**, else `Refuted(HoleNotInterior)`
  (a hole outside/crossing/tangent fails it), fail-closed alongside `DegenerateOutline` and
  `Boolean(CapOutFault)`. Result `HoledFlat{region, eps, clearance}` carries the outline's ε so the composed
  guarantee (G3 dev-fidelity ∘ G5 exact-boolean) travels to G6/G7. Tests: synthetic square−square clean cut
  (1 face/1 hole/4 edges, ε carried), hole-outside refused, degenerate refused, and the **G3→G5 bridge** (a
  *real* `unroll_freeboundary` band develops to a valid simple arrange2d operand — one hole-free face under a
  single-operand `Or`). Full gate green (develop 67 + 7 doctests, clippy `-D warnings` default + `-p export
  --features diagnostics`, fmt, missing_docs=0, `xtask lint`, no_std thumbv7em). *2026-08-10 · **G5 met**;
  branch `roadmap-flex-pcb`, `crates/develop/src/flat.rs`. The A2 SVG-with-hole render
  (`export::svg::region_to_polys`, mirroring `annulus_xor_has_ring_and_hole`) is deferred to the G7 demo
  driver.*

- **G4 met — certified `fold_outline` (`develop::fold`, with the two-sided σ=0 split).** Lifted DEV.2e's
  single-point `fold_point` (flat→3-D inversion, direction ②) to a whole **loop**: fold every flat vertex into
  a certified 3-D wire `FoldedWire{points, eps, clearance}`. The genuinely new piece is the **σ=0 split** that
  `invert_sigma`'s own doc deferred to "future G4": the signed-area bisection is faithful only while
  `|θ−ψ(σ)| < π`; a one-sided σ-domain always satisfies this (span `≤ c·π/2 < π`) but the Stage-1 **wide
  two-sided gore** (≈240°, ψ-span up to `c·π > π`) does not. `split_domain` restricts each vertex's bisection
  to the half matching `sign(θ)=sign(y)` (exact: for a gore point `|θ| < c·π/2 < π`, so
  `sign(y)=sign(sinθ)=sign(θ)=sign(σ)`) — each half one-sided, span `< π`, correct. Reuses `fold_point`
  unchanged (additive, no DEV.2e edits); per-vertex permissive clearance to read raw ε, one wire-level DRC (the
  `unroll::rail_edge_eps` pattern). Fail-closed: any vertex out-of-gore/pole/non-cone → `Refuted(FoldFault)`;
  empty loop → new `Refuted(EmptyLoop)`; loose → `Unresolved`. **Tests: the back-and-forth**
  `roundtrip_unroll_then_fold` (unroll a band → fold it back → recovers the original 3-D `chart.surface` to
  `<1e-3`, i.e. develop∘fold ≈ identity) + `two_sided_fold_splits_at_zero` (a wide gore over [−3,3], ψ-span >
  π, folds correctly — would be silently wrong without the split) + ε-shrinks-with-iters, out-of-gore, empty.
  Full gate green (develop 72 + 8 doctests, clippy `-D warnings` default + `-p export --features diagnostics`,
  fmt, missing_docs=0, `xtask lint`, no_std thumbv7em). *2026-08-10 · **G4 met**; branch `roadmap-flex-pcb`,
  `crates/develop/src/fold.rs`. The folded outer + hole `FoldedWire`s feed G6 (interior-hole STEP B-rep).*

- **G6a met — exact B-rep faces with interior hole wires (`export::brep` + `brep_build`, pure IR).** First
  half of the interior-hole STEP milestone (split with the user: **G6a** pure IR now, **G6b** the OCCT bridge
  next). The exact B-rep `Face` had a single outer `wire` and no holes concept (unlike the 2-D
  `arrange2d::Face{outer,holes}`); G6a lifts holes into the 3-D IR: `Face` gains `holes: Vec<Vec<HalfEdge>>`,
  `add_face` delegates to a new `add_face_with_holes` (no caller churn — the 22 `add_face`/`add_plane` sites
  and the sole `Face{}` literal are source-compatible), `edge_incidence`/`indices_in_range` fold in the hole
  loops, the wire-closure logic is extracted to `loop_is_closed` with new `hole_is_closed`/`all_loops_closed`,
  and `to_shell_certificate` is documented **outer-wire only** (a holed face is an honestly *open* sheet — all
  boundary edges free — outside the `closed_shell` TCB's scope; hole-free breps certify byte-identically, so no
  `certify_core` change). Builder `brep_build::brep_holed_panel(surface, outer, holes)` assembles one face from
  polyline (`EdgeGeom::Line`) loops via a private `polyline_loop` — the straight-chord wires a folded
  `FoldedWire` already is (G7 collapses its `[RatIv;3]` boxes to `[Rat;3]` midpoints; builder stays
  surface-agnostic, cone panel = `RationalPatch`). Fail-closed stays pure combinatorics + watertight-by-identity
  (hole shares no edge/vertex with the outer). Tests: holed plane face closes both loops / 8 free edges / 0
  nonmanifold / disjoint edge ids; broken-hole detected; `add_face` keeps holes empty; cert excludes holes.
  Full gate green (export 25 + 8 doctests, clippy `-D warnings`, fmt, missing_docs=0, `xtask lint`, no_std
  thumbv7em). **`step`/OCCT untouched (G6b).** *2026-08-10 · **G6a met**; branch `roadmap-flex-pcb`,
  `crates/export/src/{brep,brep_build}.rs`. G6b widens the FFI face record to N wires + `occt_shim.cc`
  `mf.Add(reversed holeWire)` before `ShapeFix_Face` + an OCCT `BRepCheck` differential test under `nix develop`.*

- **G6b met — interior-hole STEP B-rep: the OCCT bridge (N-loop faces, `export::step` + `occt_shim`).** Second
  half of the interior-hole milestone: widen the STEP bridge end-to-end so a G6a holed `Face` emits a
  `TopoDS_Face` with an outer wire **plus N inner (hole) wires** that round-trips through `BRepCheck_Analyzer`.
  **Buffer layout (CSR-of-CSR):** `BrepBuffers` gains a `loops` pool (2 f64/loop = `wire_off, wire_len` into
  `wires`); the 7-f64 face record's last two fields move from `(wire_off, wire_len)` down one indirection to
  `(loop_off, n_loops)` into `loops` (loop 0 = outer wire, rest = holes). A hole-free face emits exactly one
  loop ⇒ **byte-identical geometry** to the pre-hole encoding, so the existing 5 differential + step tests
  re-run through the new path untouched. **C++ shim:** a `build_loop` lambda assembles each loop's wire
  (shared edges by identity, as before); the surf-kind branches add holes via `mf.Add(holeWire)` before
  `IsDone`, and `ShapeFix_Face` (`FixOrientationMode=1` + `FixOrientation`) reverses inner wires to proper
  holes — the extrusion/patch branches already ran that ShapeFix (holes fold in free), the plane branch now
  runs it **only when holed** (hole-free plane path kept exactly, zero regression). **KEY RESULT (user chose
  "also attempt curved"): OCCT accepts BOTH the planar AND the curved holed panel.** The planar gate (6×6
  square, 2×2 hole, both authored CCW so `ShapeFix` genuinely reverses the hole) audits as one face, 8 edges,
  8 free (open sheet), 0 nonmanifold, `brepcheck_valid`. And the **curved cone `RationalPatch` panel with an
  off-surface-chord interior hole** — one open `brep_freeboundary` side face (on-surface rail+ruling outer
  isolates the risk) whose hole corners lie on the device cone but whose edges are straight `Line` chords
  cutting across it — **also passes `BRepCheck`**: `ShapeFix` projects the chord edges' pcurves onto the cone
  within tolerance. So STEP-II (the curved holed panel) round-trips at this hole scale with **no new
  pcurve-edge IR needed**. *Caveat carried to G7:* `ShapeFix` may inflate an edge tolerance to absorb the
  chord→surface sag (an oracle-side approximation, never the certificate); for a larger/again-curved hole
  whose sag exceeds tolerance, on-surface (σ,μ)-pcurve hole edges would become necessary — the fold-back hole
  in G7 is small, so this stays a flagged contingency, not a blocker. Gate green under `nix develop`: export
  **49 tests + 15 doctests** (`--features step`), clippy `-D warnings` (default **and** `--features step`),
  fmt, `xtask lint`, no_std thumbv7em; the default build is unaffected (all G6b code is `step`-gated).
  *2026-08-10 · **G6b met**; branch `roadmap-flex-pcb`, `crates/export/src/{step.rs,occt_shim.cc,occt_shim.h,
  differential.rs}`. Next = **G7** (demo driver: fold `FoldedWire` → cone `RationalPatch` holed panel via
  `brep_holed_panel` → STEP II + A2 SVG-with-hole).*

- **G7 pipeline driver + a STEP-export stress probe that reshaped the export (→ G9 σ-subdivision).** The
  `flex_panel` example (`crates/export/examples/flex_panel.rs`, `--features diagnostics[,step]`) drives the whole
  Stage-1 chain over a *wide two-sided* device-cone gore, printing a certified per-stage verdict: unroll → cut a
  square hole (`develop::flat::cut_hole`) → A2 SVG-with-hole (`export::svg::region_to_polys`/`polys_svg`, even-odd
  fill) → fold outer+hole back to 3-D → STEP. **The certified pipeline holds at the full ~300° two-sided gore**
  (σ∈[−15/4,15/4]): unroll/hole/fold all Verified, fold ε≈3.6e-12. **FINDING (the probe's payoff):** a *two-sided*
  cone gore could not be written to STEP — a single rational Bézier needs **positive weights** (the Bernstein
  coefficients of the denominator), and the cone's `1+σ²` denominator over a symmetric span `[−s,s]` has middle
  weight `1−s²`, exactly 0 at s=1, negative beyond. Located precisely: exports at σ≤9/10, breaks at σ=1; it's the
  span **crossing σ=0**, not width (one-sided gores of any width are fine). **Finding #0** (analytical): a single
  offset-plane *cut* also can't span a wide gore — `μ̂=d/(n·ruling)` hits a ruling-parallel pole past ~180° — so
  the wide demo uses a μ-band. **Finding #2**: the shim's `w==0.0` weight guard missed *negative* weights (|σ|>1
  crashed OCCT); hardened to `w<=0.0`.
- **G8 attempt (abandoned): a σ=0-split rational B-spline.** Represented the σ=0-crossing rail/patch as a 2-span
  positive-weight B-spline (split at σ=0, merge two one-sided Béziers). It exported the *open* holed panel (STEP
  II) at the wide gore, but **SIGSEGV'd OCCT inside a *closed* shell** (STEP I) — uncatchable in this OCC build
  (`OSD::SetSignal`+`OCC_CATCH_SIGNALS` "no catch was found"). **User rejected the direction: σ=0 is a
  *parametrization artifact*** (just where `ψ=c·arctan σ` centers; the cone has no feature there), so keying a
  split on it is fragile and could re-manifest under a different chart. Reverted in full (kept only the `w<=0.0`
  guard).
- **G9 (the robust replacement) — intrinsic σ-subdivision, single-span Bézier only.** The parametrization-
  *independent* fix: subdivide σ until every piece has positive weights — an exact, self-correcting criterion
  that never names σ=0. `brep_build::sigma_splits(den, a, b)` adaptively bisects any sub-interval failing
  `positive_weights` (all Bernstein coefficients of `den` > 0, checked at `deg(den)` — elevation preserves
  positivity). Small enough slices are always positive-weight (incl. the one straddling σ=0), and single-span
  Bézier faces are the OCCT-accepted path in closed shells (the one-sided cone-frustum solid already proves it) —
  so subdivision kills *both* the weight and the closed-shell-crash problems, with no σ=0 anywhere and each piece
  still **exact**. `brep_freeboundary` now auto-subdivides into a **fused N-σ-slice watertight solid**
  (`4(N+1)` verts, `8N+4` edges, `4N+2` faces; interior cross-rings are shared *edges* only, no interior faces);
  **N=1 (one-sided) is byte-identical to the old 8/12/6 box**, so the certified fixtures/tests are untouched.
  `closed_shell` (the TCB) certifies the subdivided solid unchanged. **STEP I (the input cone) now exports
  cleanly as a proper two-sided solid** (`write_brep` → `ok`, no abort; wide σ=±15/4 and σ=2 both green). Dead
  `cone_panel_surface` removed (superseded by the subdivision; a footgun over wide spans). Tests:
  `sigma_splits_subdivides_until_positive_weights`, `the_two_sided_cone_gore_subdivides_and_certifies`
  (closed_shell), `the_two_sided_cone_gore_is_a_robust_subdivided_solid` (OCCT `brepcheck_valid`, `free_edges==0`).
  Gate green: export lib 27 + step 51+16 doctests (`--features step`), clippy default+step, fmt, xtask lint,
  no_std. *2026-08-11 · **G7 + G9 commit 1**; branch `roadmap-flex-pcb`. Next = **G9 commit 2** — STEP II as a
  real solid slab with a through-hole (grid-minus-cell, disk-faced, closed_shell-certified genus-1).*

- **STEP II geometry — the general arrangement-per-slice construction (holes cross σ-stations freely).**
  The single-slice `add_hole` had a geometry bug the user caught: a hole had to sit *strictly inside one
  positive-weight σ-slice*, so the natural demo hole — centred on the symmetric gore's **σ=0**, which the
  positive-weight partition *forces* to be a station — could not be placed there and got shoved off-centre
  into a corner, huge and distorted (topology certified fine, geometry wrong). Two non-fixes were rejected
  (both the same mistake): grid-minus-cell (hole represented implicitly) and station-bracketing (a bandaid —
  with many holes *any* σ=const line crosses some hole, so no dodging placement exists). **The fix accepts
  that σ-stations cross holes.** The solid is now the prism over the exact 2-D region `P ∖ H` (P = panel
  `(σ,μ)` rectangle, H = holes) extruded through the thickness. Stations come from positive-weights **alone**
  (`sigma_splits`, hole-independent); per σ-slice the two developable **lids** are `strip ∖ (holes ∩ slice)`
  computed by the **same** exact `arrange2d` boolean the flat side uses (`develop::flat::cut_hole`,
  `A △ B = A ∖ B`). The arrangement decides the per-slice cell shape with **no special case**: a hole inside a
  slice → **annular** (an inner loop); a hole crossing a station → **notch** (opens onto the split station
  edge, no inner loop); a hole spanning a slice → **two μ-bands** (two faces). Each hole's tube is **split at
  every station it crosses** so each wall is single-span. **Watertight for free** via a new `Builder` edge
  dedup (`line_edge`/`rail_edge`, keyed by undirected endpoints + geometry-kind): adjacent slices share the
  split station edges by identity, and each lid shares its hole-rim edges with the tube walls — the edge-level
  analogue of the existing vertex-coordinate dedup, no global adjacency graph. Lift `(σ,μ)`→3-D by edge
  orientation: a **horizontal** edge (μ=const) → a σ-rail Bézier, a **vertical** edge (σ=const) → a straight
  radial line, a vertex → `surf(μ,w).eval(σ)`. Winding: top lid = arrangement CCW as-is, bottom lid = its
  reverse, each tube wall the reverse of both lids' use of the shared edge (once-forward-once-reversed) — the
  consistent CCW orientation makes every shared station/rim edge oppositely-directed automatically. **Scope:**
  any number of `(σ,μ)`-**rectangle** holes at any positions crossing any stations, on a **rectangular** panel
  (constant μ-band). Deferred (orthogonal): non-rectangle polygon holes (the arrangement already handles them
  once authoring emits them) and a **curved** free-boundary ∂P (a curved `(σ,μ)` boundary is not a polygon
  operand — so the **hole-free** path keeps the curved-μ N-slice slab, extracted verbatim as
  `brep_freeboundary_slab`; `brep_freeboundary` and its curved-boundary tests are untouched). `add_hole` + the
  strictly-inside-one-slice refusal are gone; refusal now only for a genuinely non-interior authored hole (or
  a non-rectangular panel / arrangement pinch). **Certified:** genus by Euler `g = (2 − (V−E+2F−L))/2`
  (representation-invariant — a notch reads genus 1 with *no* inner loop), `closed_shell_holed` (the TCB,
  unchanged) + OCCT `brepcheck_valid`/`free_edges==0`/`nonmanifold==0`. Tests (export lib):
  `a_through_hole_crossing_a_sigma_station_is_a_certified_genus_1_solid` (**the reported bug**, hole on σ=0),
  `a_through_hole_spanning_a_slice_splits_into_mu_bands_and_certifies`,
  `two_holes_one_crossing_one_interior_compose_to_genus_2`, the interior-hole test reused (now via the
  arrangement, same 16/24/10 counts), `a_hole_touching_the_panel_boundary_is_refused`; the OCCT differential
  `the_two_sided_cone_gore_with_a_station_crossing_hole_is_a_robust_genus_1_solid` drills the **σ=0-crossing**
  hole and OCCT accepts it watertight — the ground-truth that the geometry is now faithful. **Demo coherence:**
  `flex_panel` authors **one** `(σ,μ)` rectangle centred on σ=0 and derives *both* the flat cut (its
  development, a curved quad) and the STEP-II drill from it, so the SVG and STEP II land the hole in the same
  place. Full gate green: export lib 33 + doctests 8, clippy, fmt, xtask lint; certify-core 115+17 unchanged
  (the multi-loop TCB + 4 Kani harnesses are untouched — this is a `brep_build` construction rebuild, not a
  TCB change); export/step differential 10 (OCCT). *2026-08-11 · **STEP II geometry rebuilt (general)**;
  branch `roadmap-flex-pcb`. Next = Stage-2 seam (DEV.3-β · §14 BONDED).*

- **STEP II done — certified genus-`g` solids via multi-loop faces (the generic through-hole, *not* grid-minus-cell).**
  The grid-minus-cell fallback was rejected (user, same objection as σ=0): it is a *specific* construction that
  dodges a real limitation instead of removing it. The generic move is the opposite — make the **TCB certify
  faces with holes**. Key realization from reading `closed_shell` end-to-end: it is **not fundamentally
  disk-only**. Checks 3 (`∂²=0` edge census) and 4 (vertex-link single cycle) read only per-*dart* data and are
  topology-agnostic; the one-loop-per-face restriction lived entirely in check 2's input shape (one CSR wire
  per face) and in `next_in_face`. So the change is a focused **two-level CSR** (faces → loops → darts):
  **(A · TCB)** `certify_core::shell::closed_shell_holed(…, loop_start, face_start)` runs check 2 / the check-4
  rotation **per loop** (`next_in_loop`); census unchanged. `closed_shell` becomes a thin wrapper with the
  identity face→loop nesting, so **every prior caller/test/Kani harness is untouched verbatim**. `ClosedShell`
  gains `loops` (`loops − faces` = hole count). Soundness (the argument, since it is a TCB edit): declaring two
  loops one *annular* face rather than two disks is exactly "replace two disks by a tube" = drill one handle —
  preserves closed-orientable-manifoldness, only raises genus, and the checks never depended on the loop→face
  grouping (they read local dart data). Per-face *realizability* is delegated to the OCCT oracle
  (`brepcheck_valid`) — **the same delegation disk faces already rely on**, not a new trust axis. Two new Kani
  harnesses, both SUCCESSFUL: `closed_shell_holed_verdict_is_grouping_invariant` (the accept/reject verdict is
  invariant under regrouping loops into faces — transfers the disk-case soundness to the holed path) and
  `closed_shell_holed_hides_no_pinch_in_a_multi_loop_face` (a pinch packed into one multi-loop face is still
  rejected). **(B · emitter)** `Brep::to_shell_certificate` stops excluding holes — emits each face's outer wire
  + hole wires as loops (a hole-free `Brep` yields the identity nesting, certified as before). **(C ·
  construction)** `brep_freeboundary_holed(chart, σ, w, μ⁻, μ⁺, holes)` cuts a `HoleRect` authored in the
  sheet's `(σ,μ)` domain — the intrinsic coords, so the *same* hole describes the flat and folded cuts. The
  pierced `w=const` sheets become **annular** faces (an inner loop each); a **tube** (two ruled `μ=const` walls,
  two planar `σ=const` walls) closes it through the thickness; each hole raises the genus by one. The hole must
  sit strictly inside one positive-weight σ-slice — exposed via `sigma_stations` so a caller can place it — and
  the builder **refuses (returns `None`)** a hole straddling a slice boundary rather than silently mis-building
  (the general arrangement partition for wide/straddling holes is the documented, deferred scaling path).
  `brep_freeboundary` is now a thin `holes=&[]` delegate (no-interface-ossification: one engine + sugar).
  **(D · STEP II)** the demo's STEP II is a **real genus-1 through-hole solid** (`brep_freeboundary_holed` →
  OCCT `MakeFace` inner wire, the G6b path): `flex_panel_II.step` writes `ok` (14 faces, 0 free edges).
  `closed_shell_holed` certifies it internally (`loops = faces + 2`) **and** OCCT corroborates
  (`brepcheck_valid`, `free_edges==0`, `nonmanifold==0`). Tests: `a_square_slab_with_a_through_hole_is_a_closed_torus`
  + census/open-loop/wrapper refutations (certify-core), `a_through_hole_slab_is_a_certified_genus_1_solid` +
  `a_hole_that_does_not_fit_one_slice_is_refused` (export lib),
  `the_two_sided_cone_gore_with_a_through_hole_is_a_robust_genus_1_solid` (OCCT differential). Full gate green:
  certify-core 115+17, export 30+8 default / 56+15 step, demo STEP I+II `ok`, 4 Kani harnesses, clippy
  default+step, fmt, xtask lint, certify-core no_std. *2026-08-11 · **STEP II / genus-`g`**; branch
  `roadmap-flex-pcb`. Next = general arrangement-driven partition for holes wider than a slice / straddling —
  **DONE** (see the "STEP II geometry — general arrangement-per-slice" entry above).*

- **TECH-DEBT (user-flagged, 2026-08-10): `develop` is becoming a catch-all — future crate split.** As the
  flex-PCB slices land, `develop` now holds the transcendental enclosures (`interval`), the cone development
  (`cone`), and a growing family of **geometry certificates** (`anchor`, `unroll`, `fold`, and now `cut`).
  Several of these are certificate checkers of the same shape as `certify_core::{cap_in,miter}` (subdivide →
  interval-enclose → `ε=max` → DRC → `Verdict`) and arguably belong beside them, not in the flat-side crate.
  The natural seam is the **transcendental-vs-rational** boundary: `cut_fit` for instance is *purely rational*
  in σ and touches no `cos/sin/arctan`, so it could live in a `develop_geom` (or move to `certify_core`)
  while the transcendental core (`interval`, `cone`) stays a `develop_core`. **Deferred, not blocking** —
  revisit as a dedicated refactor once the Stage-1 pipeline (G1–G7) is complete and the module boundaries
  have settled; keep new checkers cohesive (own module, `Verdict`-shaped) so the eventual move is mechanical.

- **DEV / M-E = certified development (the flat↔3D layer); the chosen next big bet, opened as a GO-gated spike.** Product-decision (with the user): after M-D's exact 3D closed solids, the next thread is **DEV**, not the D4.4 atlas — because DEV is the product bottleneck (both directions pivot on it) and the highest-risk unknown (retire-highest-risk-first). Reasoning captured in the exchange: "exactness is a representation property, not a shape property" — the closed cone / **seam** / full 2π wrap is *transcendental* (a rational chart sweeps a bounded azimuth `<2π`, so one chart = a gore), so the seam and general shapes are DEV + rational-input approximation, not algebraic intersection. GO-gate criteria authored in `docs/vv-guide.md` (Milestone E (DEV)). **The spike (DEV.1)** = a certified rational enclosure of the cone's development angle `ψ(σ)=∫ψ′` (`ψ′=chart.psi_prime`, rational ⇒ arctan/log; radius `ρ=|n′|` is a surd, already in `lattice::Surd`), checked against the float ground-truth `export::mesh3d::develop_cone`, verdict-typed, with the backward-error `sup|D(â)−g|≤ε` + DRC `ε<clearance/2` scaffold and the seam as the acceptance case; it **selects the enclosure method** (closed-form arctan/log + certified rational bounds ∣ interval integration ∣ Taylor models) and GO/no-go's the tier. Additive (its own spike boundary; the pure exact tier untouched). *2026-08-10 · **DEV.0 + DEV.1 met — decision GO** (`docs/spike-development-report.md`); the cone development reduces to a single `arctan` of a rational (`ψ = 2 sinβ · arctan σ`, verified as an exact polynomial identity), method (a) closed-form arctan + rational alternating-series bounds selected, certified backward error `≈1e-11` corroborated to `≈1.5e-8` — see the DEV.1 finding below · `docs/vv-guide.md` Milestone E (DEV), `docs/implementation-plan-v1.md §6`*

- **DEV.1 spike GO — the cone development is a single `arctan`, and the one wall is digit-growth.** The spike (`crate develop`: `develop::interval` rational enclosures of `arctan`/`π`/`cos`/`sin`/`√`; `develop::cone` composing them into a certified `FlatBox`) priced the certified flat↔3D development on the device cone and **GOes**. Findings: (1) **the transcendental core is minimal** — `ψ′ = det(n,n′,n″)/|n′|²` reduces to `c/(1+σ²)`, so `ψ(σ) = c·arctan σ` with `c = 2 sinβ` rational (the textbook `ψ = sinβ·φ₃D`); `cone_angle_coeff` **verifies** `ψ′·(1+σ²) ≡ c` as an exact polynomial identity, and the radius `ρ = |n′|` is a surd (perfect-square-rational for the device fixtures). So DEV is "certify `∫(rational)` = an arctan/log," not "certify arbitrary transcendentals." (2) **Method (a)** (closed-form arctan + alternating-series rational brackets, argument-reduced to `|t|≤½` for geometric convergence) beats interval integration (`O(1/N)`, kept as the DEV.2 fallback for the non-elementary `γ=∫e(ψ)`) and Taylor models. (3) **The wall: naive exact-rational composition of the `cos`/`sin` series over a many-digit `arctan` argument blows the endpoint digit count to hundreds–thousands** (values `O(1)`, representation huge). This bit the corroboration harness — a `numer/denom→f64` cast overflowed both to `∞`, `∞/∞=NaN`, and `f64::max` silently dropped every `NaN`, so a *broken* test read green (checking only the trivial `σ=0` row). Caught by challenging the too-perfect `max_diag==max_analytic`; fixed with a leading-digits `big_rat_to_f64`, and it re-surfaced the real numbers (backward error `1e-11`, corroboration `1.5e-8`). **Remedy: fixed-precision interval arithmetic with directed (outward) rounding** — a DEV.2 build item (wants a small additive `floor`/`ceil` on `lattice::Rat`), *not* a viability risk. *2026-08-10 · GO · `docs/spike-development-report.md`, `crates/develop/**`, `export::mesh3d::certified_flat_point_corroborates_develop_cone`*

- **DEV.2 planned — the certified development tier for the *closed-form* developable class; two scope decisions with the user.** After the DEV.1 GO, the next milestone builds out the tier. Two framing decisions locked with the user: **(1) generality = the closed-form class, now.** DEV.1's foundation is already general (the `interval` enclosures, `ρ=√(‖n′‖²)` surd, `ψ=∫ψ′` arctan/log-class for any chart); DEV.2 broadens from the device cone to every developable whose development is *elementary* — cones at any placement (`ψ=∫P/Q` = a sum of arctans/logs, needs a new `log` enclosure + partial fractions; higher-degree `Q` over `AlgReal` flagged) and cylinders (`ψ′≡0` ⇒ `e(ψ)` const ⇒ `γ` elementary). The genuinely non-elementary case — `γ=∫[rational]·e(arctan)` with a **curved directrix** (tangent-developables / arbitrary ruled) — is deferred to **DEV.3** (verified interval integration, the DEV.1-selected method (b), its own GO). **(2) creases = atlas, not `develop`.** `develop` certifies the flat↔3D isometry of a *single* chart; the multi-panel **creases / fold-mates** (spec §5.3 MONO; the reflection mate `n_B=n_A−2(n_A·B/B·B)·B`, already built for one joint in the M-D closure/sew layer) are the **atlas** (D4.4) + `closure`/`sew`. So direction ② splits: `develop` supplies the per-panel `D⁻¹`+chart-eval map, the atlas assembles across creases. Slice arc: DEV.2.0 (docs) · DEV.2a fixed-precision outward rounding (the digit-growth remedy — `Rat::floor`/`ceil` pure + Kani, `RatIv::round_out`) · DEV.2b general closed-form angle · DEV.2c ANCHOR backward-error certificate (T-part, `develop`, composes with the pure `certify_core` A-part) · DEV.2d unroll ① · DEV.2e fold-inversion ②. *2026-08-10 · **DEV.2.0 + DEV.2a + DEV.2b met**. DEV.2a (`3e18c61`) retired the digit-growth wall: `lattice::Rat::floor`/`ceil` (pure, Kani panic-freedom `floor_ceil_fast_path_panic_free_full_domain`) + `develop::interval::round_out`/`ROUND_BITS=60` carried through every series accumulator → device-cone endpoints ≤ 19 digits at 40 terms, backward error `≈6e-12`, corroboration `1.5e-8` (the `big_rat_to_f64` workaround gone). DEV.2b generalized the angle: `develop::cone::angle_enclosure` integrates `ψ=∫P/Q` by completing the square on a degree-2 positive-definite `Q` → `(a/2A)·log((σ−p₀)²+q₀²) + ((ap₀+b)/Aq₀)·arctan((σ−p₀)/q₀)` (surd `q₀` via `sqrt` + `RatIv::recip_pos`), enclosed by the new `interval::log` (`atanh` series + power-of-two reduction + geometric tail bound) and `interval::arctan_on` (interval argument). `Verdict`-shaped with `AngleDefer` (higher-degree → `DenominatorDegree`, real-roots → `RealRoots`, unsigned radius → `RadiusNotSigned`) so a non-closed-form chart is a clean `Unresolved` pointing at the `AlgReal` extension / DEV.3, never a silent `None`. Reproduces DEV.1's `c·arctan σ` on `cone()`/`cone_alt()` across the gore, certifies a reparametrized cone `q(σ−1)` (`Q=σ²−2σ+2` ⇒ `(130/97)(arctan(σ−1)+π/4)`) the canonical recognizer declines, and validates the log branch on `σ/(1+σ²)=½ln(1+σ²)`; all float-corroborated to `≈1e-9`. Full gate green (fmt, clippy `-D warnings`, nextest ws 413 + export/step+diagnostics 57, doctests, `-D missing_docs` develop, `xtask lint`, no_std thumbv7em). **DEV.2c met** (`develop::anchor`, `16bfe70`→next commit): the ANCHOR **T-part** `sup_t|D(â(t))−g(t)|≤ε` + DRC `ε<clearance/2`. `anchor_dev` subdivides the `t`-span, encloses the developed anchor `D(â([a,b]))` (new `ConeDevelopment::point_on`/`angle_on`/`radius_on` over σ-intervals) and the authored target `g([a,b])` (new `interval::eval_poly_on`/`eval_ratfunc_on`/`sqrt_on`/`abs_on`), bounds `√(Δx²+Δy²)`, takes the max `ε`; `Verified(ValidAnchorDev{eps})` / `Unresolved(ε)` (refine `subdiv`) / `Refuted(AnchorDevFault::{DegenerateSpan,PoleInEval})`. `anchor` composes it with the **pure** `certify_core::free_boundary` A-part into the full ANCHOR (`T,1D + A,1D`), auditing that the anchor's σ-range = the band's σ-span (`AnchorFault::SpanMismatch`). Decisions with the user: **general rational-`t`** anchor `â(t)=(σ(t),μ̂(t))` — realizable without the missing composition primitive (the checker *evaluates* `σ(t)` over `t`-intervals, never symbolically forms `ρ∘σ`) — **riding a free-boundary μ-rail** (affine ⇒ `μ̂(t)=μ⁻(σ(t))` is a `scale`+`add`, no composition). Device-cone fixture: `ε` shrinks with `subdiv`, generous clearance `Verified`s / tight `Unresolved`s, `ε` upper-bounds the float chord-sagitta, span-mismatch refused. Full gate green (nextest ws 421 + export 57). **DEV.2d met** (`develop::unroll`, `7c252c5`→next commit): certified **unroll** (direction ①). `unroll_freeboundary` develops the free-boundary band boundary loop into a flat **polyline** `FlatOutline`{vertices:Vec<FlatBox>, eps, clearance}: develops each rail station to a `FlatBox`, and certifies each **rail edge** within `ε` of the true continuous developed rail via the DEV.2c `anchor_dev` lift bound (chord target, per-edge; the two σ-caps are rulings → exact straight radials, no ε). Whole-outline `ε = max` edge bound, DRC-gated: `Verified(FlatOutline)`/`Unresolved(ε)` (refine `segments`)/`Refuted(UnrollFault::{DegenerateSpan,PoleInEval})`. Genuinely consumes DEV.2c. Device-cone fixture (μ⁻=−1, μ⁺(σ)=−1+σ): `ε` shrinks with `segments`, generous clearance `Verified`s / tight `Unresolved`s, vertices enclose the development; corroborated vertex-by-vertex vs the float `develop_cone` to `<1e-5` (`export::mesh3d::unroll_outline_corroborates_develop_cone`). Also hand-wrote `Clone` for `ConeDevelopment`/`DevConfig` (the `RatIv` B-not-Clone pattern) since unroll clones them into per-edge certs; corrected the stale `mesh3d` module doc ("certified development cannot live in the rational kernel" → it now does, as rational-interval enclosures). Full gate green (nextest ws 425 + export/step+diagnostics 58). **DEV.2e met — DEV.2 COMPLETE** (`develop::fold`, `ed19eb5`→next commit): certified **fold-inversion** (direction ②, per-panel). `fold_point(chart,x,y,w,domain,iters,mu_negative,cfg,clearance) -> Verdict<Fold3D{sigma,mu,point:[RatIv;3],eps,clearance}, FoldFault{NotACone,DegenerateDomain,OutOfGore,PoleInEval}, Rat>`. **angle→σ**: `θ=atan2(y,x)=ψ(σ)` inverted by monotone bisection on the signed area `cos ψ·y − sin ψ·x = r·sin(θ−ψ)` — never computing the transcendental `θ`; **a non-dyadic 3/7 split** (KEY: bisecting at the exact midpoint stalls on dyadic device roots σ=1/2,3/4 — cross straddles 0 at step 1 → returns the full-width [lo,hi] that never refines; 3/7 never hits a rational root exactly so the straddle-stop triggers only at the precision floor). **radius→μ̂** = `r/ρ(σ)` (sign = authored panel side `mu_negative`). **lift** exact `C=c+μ̂·r⃗+w·n` (chart pedal/ruling/normal `.comp(i)` interval-eval'd over σ) → 3D box. Certificate = **round-trip** backward error (`axis_residual` of the re-developed `D(σ,μ̂)` box to `(x,y)`), DRC-gated. Device-cone fixture: folding the forward image of `(σ₀,μ₀)` recovers both enclosures + `|C|=r`, `ε` shrinks with iters, tight clearance `Unresolved`s, out-of-gore angle → `OutOfGore`. Full gate green (nextest ws 430 + export 58). **DEV.2 (the certified closed-form development tier) is DONE — both product directions certified per-panel.** Next: DEV.3 (γ≠0 curved-directrix frontier, own spike/GO) OR the atlas (D4.4, multi-panel crease assembly) — a milestone-level decision. Branch `dev-go-gate` still UNMERGED · `docs/vv-guide.md` Milestone E DEV.2, plan `plan-first-and-then-twinkly-minsky.md`*

- **Curved MITER-FIT = the transverse-rational `φ_J` correspondence (L3 activation); the machinery D4.2 needs, pursued as its own milestone.** D4.2 (a two-flank closed solid on `one_joint()`) is **fixture-obstructed, not blocked by missing code** (see Findings) — so per the standing "build the incomplete machinery, don't manufacture demo geometry" directive, this milestone builds the deferred **curved MITER-FIT**: the *transverse* regime where two flanks' cut rulings are **rationally** (not affinely) parametrized and their coincidence in the bisector plane Π is certified through the correspondence `R(σ_A,σ_B)=0` (spec §5.3; `certify-core/src/miter.rs:31-32` + `docs/closure-scoping.md:52-54` defer it). First downstream wiring of `lattice`'s built-but-unused `resultant`/`resultant_bivariate` (and, later, `AlgReal` + conic carriers). **Earned, not oracle (OCCT never enters):** the certificate is a resultant-conditioned **divisibility identity** — on `{R=0}` (paired rulings share their crease-line point, so position identity is free) certify carrier identity `D_A ∥ D_B` + extents `E_{A,±}=E_{B,π(±)}` by an **exact cofactor** `X == R·Q` (`X=R·Q ⇒ X≡0 on {R=0}`, an exact implication); the only trusted lemma is resultant⇔common-root (Lean, out of Kani per vv-guide §5 — `verify_common_factor` is "exactly the spec's resultant-conditioned A-identity"). Watertightness does not hinge on it (a non-coincident cut is a valid exposed LEDGE, spec §5.3). Slices: CM.0 (criteria) → CM.1 (`miter_fit_transverse` checker + Kani, **additive** to `certify-core` beside the degree-1 `miter_fit`) → CM.2 (conic carriers) → CM.3 (`AlgReal` wiring) → CM.4 (closure searcher + minimal cone-flank sub-fixture) → CM.5 (Lean frontier, non-gating). Criteria in `docs/vv-guide.md` (Curved MITER-FIT). *2026-08-09 · CM.0 + CM.1 met — CM.1 landed `lattice::Biv` (bivariate polynomial over ℚ, the first consumer of the `resultant_bivariate` convention) + `certify_core::miter::miter_fit_transverse` (forms `R(σ_A,σ_B)` from `ℓ_A = ℓ_B`; carrier + extent identities by the exact cofactor `X == R·Q`; `ℓ_i` monotonicity by Sturm; `ε_φ` from slope signs via the Kani-proven `eps_from_slopes`), additive beside the degree-1 `miter_fit`; genuinely-rational symmetric pair certifies, curvature-order / extent-counterexample / parallel-regime / wrong-cofactor refused. Full gate green (nextest ws 375, export/step 37, doctests, `-D missing_docs`, `xtask lint`, no_std thumbv7em, Kani `eps_from_slopes_is_slope_agreement`). **CM.2 (conic carriers) SKIPPED** — unsound as framed (see Findings): a conic is non-D24 content CAP-IN-D24 correctly refuses, and the clean-miter path uses straight rulings, not conic carriers; deferred to the conic-arrangement L3. **CM.3 met** — first downstream use of `lattice::AlgReal`: `AlgReal::sign_of` + `AlgReal::count_roots_upto` (polynomial sign / root-count at & up-to an algebraic σ) + `certify_core::miter::strictly_monotone_upto_alg` (transverse monotonicity certificate over an algebraic cut-face σ-bound — the cone's cut-exit σ). Full gate green (nextest ws 378, export/step 37, doctests, `-D missing_docs`, `xtask lint`, no_std thumbv7em). CM.4 (cone searcher + minimal fixture) next · branch `curved-miter-fit`*

- **M-D slice 4 = atlas assembly → the certified closed solid; the spine is a new *proven* `certify-core` checker.** Slice 3 left every solid *certified-seam, honest-open*: closedness of the whole solid is decided only by OpenCASCADE `BRepCheck` — an **oracle, not the certificate** (spec §8.2:332). The spec has **no predicate certifying whole-solid closedness** (`VALID_solid-closure` §8.6:439 is only `VALID_complement ∧ ⋀_j CLOSURE_VALID(j)`, joint-local); the docs pre-name the missing layer ("ruled sidewalls carrying their own CAP-OUT/SEW-LINK coverage → whole-solid watertightness certified") but flag it unbuilt → **atlas assembly**. Slice 4 builds it, spine-first: a `certify_core::shell::closed_shell` **closed-2-manifold** checker (checks 1 range, 2 wires-closed, 3 **∂²=0** oriented edge census, 4 **vertex-link single-cycle** via a rotation-system permutation) — the assembly-scale analogue of the `CapOut.lean:25-30` frontier theorem — Kani-proven bounded, composed into an **additive** `valid_closed_solid` gate, and *corroborated* (never overturned) by the OCCT oracle. Two doctrines bake in: **incidence not proximity** (spec:192, faces share an exact edge *id*, never a tolerance) and **earned not oracle** (a forced `closed=true` is oracle-instead-of-audit). **Single-flank first (geometry forces it):** M-D D.1 *proves* the two flanks' crease coincides only at the neutral sheet `w=0`, so a two-flank watertight slab is obstructed (the `w=t` outer creases diverge → gluing yields a non-manifold edge), while a single-flank bent box (top `w=0` + bottom `w=t` + four ruled sidewalls over the **support box** — a legitimate free-boundary contour, spec:151) *is* an exact closed 2-manifold; the two-flank union is its own phase (D4.2). The "exact closed slab by-construction" slice 3 **declined** becomes legitimate here precisely because D4.1 now supplies the missing certificate — no anchors / authored contour / multi-joint machinery needed for the first closed solid (those are D4.3/D4.4). The TCB edit is **purely additive** (new `shell` module + Kani harness + `valid_closed_solid`; `arrange.rs`/`sew.rs`/`boolean.rs`/`valid.rs` untouched in D4.1). Phases: D4.0 (criteria) → D4.1 (checker + single-flank closed slab, the "both" slice) → D4.2 (two-flank union / the `w=0` obstruction) → D4.3 (contour + anchors, spec §4.6) → D4.4 (multi-joint / atlas container) → D4.5 (sew sidewall coverage, additive) → D4.6 (Lean 2-manifold theorem, frontier, non-gating). Criteria in `docs/vv-guide.md §8` (Milestone D slice 4). *2026-08-09 · D4.0 met (`2ffceda`) + **D4.1 met** — the first certified closed solid: `certify_core::shell::closed_shell` + 2 Kani harnesses (`5763183`), `valid_closed_solid` gate (`77d39a7`), `export::brep_slab_from_closure` + `Brep::to_shell_certificate` (`0b40734`), rational-patch surface FFI + `Vec3Rat::reduce` + e2e OCCT corroboration (`aaea9c0`). Two findings below (degree-inflation reduce; `Geom_BezierSurface` vs segfault). D4.2 (two-flank union) next · branch `milestone-d-atlas`*

- **Milestone D scoped as a sequence of slices; slice 1 = the physical joint fixture.** The roadmap's D
  (`implementation-plan-v1.md:53`) is the whole device (cone + lap-seam + petal atlas → lens-assembly
  solid) — a culmination, not one vertical slice. Decomposed into three threads (physical fixture / audit +
  `V_∂`-guided seam + OCC oracle / atlas breadth); criteria in `docs/vv-guide.md §8` (Milestone D). **Slice 1**
  discharges the three M6 fixture warts — `h ≡ 0` cone → true `h ≠ 0` cylinder, disjoint-support gap → two
  distinct flanks sharing one crease, stretched cap → metric-faithful `Surd(a,b,s)` lift — with the joint
  still certifying through both the MITER and LEDGE branches. Two readings locked: **`VALID_material` → M-E**
  (needs SMOOTH/DEFERRED bands + FRESH, both E — consistent with the FRESH deferral below); **the
  external-kernel audit is an *oracle*, not the certificate** (spec "no kernel CSG"; region/shell
  manifoldness is CAP-OUT-LINK / SEW-LINK; "oracle ∧ audit, never oracle-instead" §8.2:332) — that governs
  thread 2, not slice 1. *2026-08-09 · open · `docs/vv-guide.md §8` (Milestone D), branch `milestone-d`*

- **M-D slice 2 = the OpenCASCADE differential oracle (thread-2 half b); the watertight V_∂ seam is slice 3.**
  Wire OCCT `BRepCheck` as an oracle **compared** against the internal verdict (a strings-only
  `occt_shell_audit` reporting free-edge / non-manifold-edge / closedness facts beyond bare `IsValid()`, a
  test-only `export::differential` harness mirroring `difftest`), and have `export` **consume** the certified
  `v_boundary()`/`pinches()` read-only (comparison layer + a `cap_tris` gate on `pinches().is_empty()`).
  Scope split forced by geometry: slice 1's 2:1 ruling-speed overhang means a geometrically-coincident V_∂
  seam does not exist at the sampled band, so the oracle's headline output is *surfacing* that overhang as a
  documented, CI-enforced divergence (OCC free-edges/non-watertight vs internal manifold) — never overturning
  the certificate. The geometry-changing seam (indexed-shell FFI + geometry-derived `SewInput`) is **slice 3**.
  Criteria in `docs/vv-guide.md §8` (Milestone D slice 2). *2026-08-09 · DONE — slice 2 met (`c5800e8` criteria, `1eb404a` shim+audit+CI leg, `9d59418` differential harness + `pinches()` gate; agreement + documented overhang divergence asserted for both cap branches) · `docs/vv-guide.md §8`, branch `milestone-d`*

- **M-D slice 3 = exact ruled-surface STEP emission (certified-seam, honest-open); the slice-2 "indexed-shell / V_∂-welding" framing is superseded.** The STEP *body* the spec mandates is **exact rational surfaces**, not triangles (§10:464 "face surfaces exact; sidewalls exactly ruled …; no kernel CSG"; §11:470 makes discrete meshes an explicit **non-peer** export). The current triangle soup (`shell.rs` σ-grid samples of `chart.surface` + D24-square cap, `occt_shim.cc` float-tolerance sewing) is therefore a stopgap — and it *manufactured* the 2:1 overhang: sampling the untrimmed `μ∈[−1,1]` rectangle never applies the certified plane trim, so the band is unavoidably open. The exact object already exists (`Chart::surface(μ,w) = c(σ)+μ·r(σ)+w·n(σ)` is a `Vec3Rat`); the gap is the emission path. Fix per §5.3: emit each flank as an exact ruled face **trimmed by the exact bisector plane Π**, with the shared Π-cut edge referenced **by identity** (watertight-by-construction) — MITER where the trims coincide (empty ledge), an exposed planar LEDGE (`face_A △ face_B`, a boundary step not a hole) where they don't. **Scope decision (with the user): certified-seam / honest-open** — the certificate is joint-local (SEW/CAP-OUT cover only the seam edges + `V_∂` links, nothing certifies the substrate outer boundary; by P1:12 a joint is a slice of an atlas, closing sidewalls are "ruled over anchors" §:192/:464 = unbuilt machinery `one_joint()` has no contour to feed), so emit only certificate-backed exact faces and leave the substrate boundary honestly open (annotated), never a fabricated `closed=true` (that would be oracle-instead-of-audit). Representation = **Strategy B** (emit exact rational-Bézier boundary curves, let OCCT build the ruled/linear-extrusion surface; the watertight object is the shared 1D edge). Order MITER→LEDGE. **Explicitly declined this slice:** the "exact closed slab by-construction" (support-box sidewalls to force `closed=true`, closedness uncertified away from the joint) and the certified closed solid (anchored contour → ruled sidewalls with their own SEW/CAP-OUT coverage), the latter deferred to **atlas assembly**. Phases D3.0 (criteria) → D3.1 (`bezier.rs` monomial→Bernstein + `brep.rs` exact IR) → D3.2 (MITER ruled flanks + surface FFI, GO/NO-GO on `Geom_*`/`MakeEdge` linkage) → D3.3 (LEDGE cap from `region().faces[].outer`) → D3.4 (differential flip + mesh retention). Criteria in `docs/vv-guide.md §8` (Milestone D slice 3). *2026-08-09 · DONE — slice 3 met. D3.0 criteria (`vv-guide §8`, `vv-matrix`); D3.1 exact primitives (`bezier.rs` + `brep.rs`, `9b3a320`); D3.2a surface FFI (`3db3746`); D3.2b fixture flip so the `w=0` neutral sheet is retained (`f9eebee`) + the mesh-cap fan fix (`8ee76a4`); D3.2 MITER two ruled sheets sharing the crease middle `M` by identity (`7fc1fb5`); D3.3 LEDGE exact body = the same two flanks, exact cap deferred (Option B — see the three-way `BRepCheck` box in Findings, `34843cf`); D3.4 differential harness audits **both** paths (watertight crease seam on the exact body, retained overhang on the mesh) + STEP body routed through `brep` + doc gate. Scope certified-seam / honest-open; the exact LEDGE cap + a genuinely-closed solid remain deferred (`V_∂` real-cut slice / atlas assembly). Gate green each phase (real exit codes, `--features step` leg included). · branch `milestone-d`*

- **Finding + pivot: D3.2's "share the Π-cut edge by identity" has no honest realization on `one_joint()` within the certified box — the phase was started before the machinery to place a shared 3D seam existed.** Grounded by exact computation over the fixture (throwaway `scratch_explore`, since deleted). The crease line is `L = {(x, 0, 1)} ⊂ Π`, anchor `x₀ = (0,0,1)`, bisector `b_J = (0,1,1)`. **At the fold crease `w = 0`:** flank A's crease ruling covers `x ∈ [−2, 2]` on `L`, flank B's covers `x ∈ [−1, 1]`; they overlap only on `x ∈ [−1, 1]` and A overhangs 2:1 — but this is *at `w = 0`*, which is **outside** the certified `w`-box `[1, 2]`. **Within the certified box** (`shell.rs` samples at `w = w_lo = 1`): the two flanks have **diverged entirely** — A rides out to `z = 2`, B down to `y = −1` — and share **nothing**. `G_A(σ=0, w=0) = 0` exactly (flanks touch Π only along the crease, at `w = 0`); `G_A(w=1) > 0` throughout (Full-retained). So the flanks are Full-retained and **disjoint** everywhere the certificate actually covers; the miter diamond and the SEW seam are **2D cap-frame licensing artifacts** (coincidence in the projected `(μ, w)` frame), not shared 3D edges. Emitting a single shared `TopoDS_Edge` between them would be **oracle-instead-of-audit** — fabricating watertightness the certificate does not back. **Root cause:** the plan assumed the certified region touches the crease where the flanks meet; it does not (`w`-box floor is `1`, crease is at `0`), and no existing machinery derives a certificate-backed 3D seam curve away from `w = 0`. **Decision (with the user):** land the surface-FFI infrastructure now (`occt_write_brep`/`occt_brep_audit` + `brep_to_buffers`/`write_brep`/`audit_brep`, proving `Geom_*`/`MakeEdge`/`BRep_Builder` link and that a hand-built two-face brep audits as 2-incidence / `nonmanifold==0` / valid), then **re-think the D3.2–D3.4 implementation plan** before wiring `brep_from_closure(MITER)`. The FFI is unblocked and independently valuable; the closure-wiring is what lacked machinery. *2026-08-09 · superseded by the D3.2b resolution below — FFI landed (`3db3746`); re-think done · branch `milestone-d`*

- **D3.2b resolution — the certified box is a *retention window*, not the face extent; the shared edge is the crease `L` (MITER-certified), and the emission is re-framed around it (Path A, with the user).** Re-derived the fixture's `w`-box mechanically (throwaway `scratch_wbox`, since deleted; drove the real `closure::trim` `field_a`/`GField::eval` API, numbers below match the algebra exactly). For flank A (`h≡1` ⇒ `pedal_A = n_A`, `x₀ = n_A(0) = (0,0,1)`, `b_J = (0,1,1)`): `G_A(σ,μ,w) = g0(σ) + w·g_w(σ)` with `g_w = (1−2σ−σ²)/(1+σ²)` and `g0 = g_w − 1 = −2σ(1+σ)/(1+σ²)`. **Why `w`-box `[1,2]` not `[0,2]`:** TRIM-LOCAL needs `G_A > 0` (with margin) over the whole box, but over the support `σ∈[0,1/8]` the neutral sheet `w=0` is on the *deleted* side for every `σ>0` (`g0(1/8) = −18/65 ≈ −0.277`); `w=0` and `w=¼` go negative, only `w≥1` clears with margin — the floor is forced by geometry + margin, not an arbitrary constant, and `[0,2]` is refuted at the `σ=1/8,w=0` corner (`RegFault::OuterFiber`). **Is `w=0` meant to be certified? No, and it structurally can't be:** the crease `(σ=0,w=0)` sits *exactly on* Π (`G_A=0`); the retained region `G>0` is an open half-space and the crease is a boundary point, so any product-of-intervals box strictly inside `G>0` is necessarily bounded away from `w=0`. **Consequence:** inside `[1,2]` `G_A>0` strictly ⇒ CLIP-DOM Full-retained, *no cut in the box*; the Π-cut curve `{G_A=0}` runs from the crease `(0,0)` up to `≈(1/8, 0.38)`, entirely below the `w=1` floor. The one real shared 3D edge is the crease line `L={(x,0,1)}` at `w=0`, and that is exactly what MITER-FIT/SEW certify as PAIR-IDENTICAL. So SEW/MITER (seam at `L`, `w=0`) and TRIM/CLIP (retained slab, `w∈[1,2]`) certify **disjoint regions with an uncertified `w∈[0,1)` gap**; the earlier NO-GO was correct that no Π-cut is shared *in the slab*, but wrong to conclude nothing is shared — the crease `L` is. **Decision (Path A, with the user):** re-frame D3.2 emission so the shared edge is the MITER-certified crease `L` and the `[1,2]` box is read as the *retention witness*, not the face boundary — each flank ruled face carries `L` (or its common sub-segment, split at the 2:1 overhang into a shared middle + free tips) as a boundary edge referenced by identity. No fixture surgery (the flip-support alternative — support `σ∈[−1/8,0]` makes `G_A≥0` at `w=0`, confirmed 0.215/0.117/0.060/0, so the box *could* reach `w=0` — was declined: it re-keys the MITER/SEW/LEDGE packets and changes which flap is retained, for no gain over consuming the seam MITER already certifies). D3.2–D3.4 re-planned around `L`. *2026-08-09 · resolved — Path A chosen; D3.2 emission re-plan next · branch `milestone-d`*

- **D3.2b landed (fixture flip) + a stale-comment finding.** Deeper `surface(μ,w)` probing (throwaway `scratch_surf`, since deleted) settled two things. (1) **The `w=0` neutral sheet is a genuine 2D face** for the `h=1` cylinder (as σ sweeps, the ruling — an x-segment — moves through `(y,z)`: `(∓1.97,+0.25,.97) → (∓2,0,1)=L → (∓1.97,−0.25,.97)`), *not* degenerate — so `shell.rs`'s rationale "the ruled patch degenerates to a line at `w=0`" is **stale** (true for Milestone C's `h≡0` cone whose pedal collapses to the axis, false for the current cylinder), and that stale note is exactly what pushed the mesh emission to `w=1` where the flanks are disjoint. Fix `shell.rs` when the emission moves to `w=0` in D3.2. (2) **Pure Path A (keep the fixture, emit `w=0`) is not honest:** on the *current* support `σ∈[0,1/8]` the `w=0` sheet is on the *deleted* side of Π (`G_A<0`), so it would poke through into flank B's half-space. Path A's honest form therefore **converges with the flip:** support `σ_a∈[−1/8,0]`, `σ_b∈[1,9/8]` puts the `w=0` sheet on the retained side (`G_A≥0`, touching Π only at the crease), meeting along `L` (A's `x∈[−2,2]` ⊇ B's `x∈[−1,1]`, the 2:1 overhang → shared middle + free tips). **Change (with the user):** flipped `boxes()`'s σ-supports in `fixtures::closure_joint`; re-verified `closure_valid` still `Verified` via **both** the MITER and LEDGE caps (`confine=(0,1)`, `w=[1,2]` unchanged — `g_w>0`, `g_mu≡0` keep `(μ=0,w=1)` the box-minimizing corner, and MITER/SEW/LEDGE are Π-frame 2D, independent of the σ-support), full workspace `nextest` 350/350, fmt/clippy/xtask/doctests/`missing_docs=0` all green. The `w`-box `[1,2]` is retained as the retention *window*; the emission (D3.2) will carry `L` as the shared edge. *2026-08-09 · fixture flip landed; D3.2 emission next · branch `milestone-d`*

- **Finding: the `export` `step` feature is not exercised in CI at all.** `.github/workflows/ci.yml` runs
  `cargo nextest run --workspace` and the doctests with **no** `--features step`, and `export::step` is
  `#[cfg(feature = "step")]` — so the slice-1 STEP end-to-end suite (`one_joint_{ledge,miter}_writes_a_reloadable_step_shell`)
  is green only when run locally under `nix develop --features step`, never in CI. M-D slice 2 adds the
  missing dedicated `nix develop --features step` leg (mirroring the CGAL oracle leg at `ci.yml:65-66`), which
  retroactively covers slice 1. *2026-08-09 · DONE (`1eb404a`: the `--features step` CI leg runs `clippy` + `nextest -p export` + `step` doctests inside `nix develop`, covering the slice-1 `one_joint_*` tests and slice-2's `export::differential`) · `.github/workflows/ci.yml`*

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
  - **DONE (C1, milestone-c):** `certify_core::cap_in` mints `ValidatedD24` (opaque, private-field
    `CanonicalEdge` cycle) only via `cap_in_d24`, which runs the full census — carrier identity by
    exact `on_carrier` rational-function residual (a conic satisfies no line/circle identity →
    `OffCarrier`, *falsely* not vacuously), finite interval, rational endpoints, closed cycle, and
    A/B flank correspondence — returning a two-valued `Verdict`. The `closure::cap_in` searcher
    projects a flank chart into the cap plane (`PiFrame`, `project`, `ruling_edge`, `sigma_edge`,
    `line_through`): a cylinder ruling → line passes; a cone σ-cut → conic is refused. Consumed on
    the LEDGE branch only. The `arrange2d::validate_d24` boundary guard stays as the totality
    net; the type-level license supersedes it as the *input* gate. *2026-08-08 · done(C1) · spec §8.5*
  - **DONE (C2, milestone-c) — regularity bundle + a SIDE/COLLAR scope split.** `certify_core::wedge`
    checks REG-V ∧ WEDGE ∧ EXT-WEDGE at the crease. On the straight-crease **constant-V** scope
    `|V|² = (1 − d)/(1 + d)` with `d = n_A·n_B`, so all three are **division-free `Rat` ring
    comparisons** clearing `1 + d > 0` — no Sturm/span (simpler than `reg_q`, same `MarginSq`/`Verdict`
    idiom). The searcher `closure::wedge::wedge_cert` evaluates the two flank charts' unit normals at
    the crease stations; the checker re-derives `d` and verifies the normals are unit before clearing.
    **Scope decision (feeds C3):** SIDE(b_J) and COLLAR are bundle members whose crease-local witness is
    *implied* by REG-V ∧ WEDGE (`|b_J|² = 2(1 − d) > 0`; the `Q(s)` split is complementary for free) and
    WEDGE ∧ EXT-WEDGE (quotient-wedge embeds) respectively — so C2 delivers three *independent*
    crease-local atoms, not five. SIDE's independently-refutable "wrong-side" content (retained side
    `G_i ≥ 0` over the actual support) is **TRIM-LOCAL** and COLLAR's cross-t **TUBE** padding by
    `D²_collar = 4w²s_bev²|V|²/(1+s_bev²|V|²)` is **TUBE-LOCAL** — both need the `G_i`/tube fields, so
    they land in C3 with their siblings, not fabricated as thin crease-local predicates here. `s_bev`
    and the REG-V margin are authored treatment data threaded through the searcher call (not on
    `Joint`), to be folded into the `{s_J, b_J, φ_J}` closure bundle at C6. *2026-08-08 · done(C2) ·
    spec §8.5 :266/:382*

- **CAP-OUT strict-manifold entry (`ShellReady`) — decide when SEW lands.** `ledge_dom_certified` is
  deliberately *relaxed*: a pinch (non-manifold vertex, e.g. a transverse `△`) is a valid, reported
  result (`CapOut.pinches`), not a refusal — the manifold requirement is owned by the downstream SEW-LINK
  gate, and there is no pre-SEW consumer today (confirmed in review batch 1). When SEW (M4/M5) is built,
  reconsider a typed strict entry `ledge_dom_manifold → ShellReady<B>` that additionally gates
  `pinches.is_empty()` and returns a type only a no-pinch region inhabits — so "forgot to check
  manifoldness" is a compile error and the proven `link_ok` is used in production. Deferred (not now)
  because the newtype's contract is SEW's to specify; building it blind risks guessing wrong.
  *2026-08-06 · deferred(→M4) · `certify_core::arrange::link_ok`* **· C4 review (2026-08-08):**
  re-confirmed the deferral — `closure::ledge::ledge_cap_certified` (the C4 LEDGE driver) returns the
  **relaxed** `Verdict<CapOut>` verbatim, reporting `pinches()` rather than gating on them, and there is
  still no pre-SEW consumer (the cylinder-flank cap is convex ⇒ `pinches().is_empty()` holds, asserted
  in the unit test, but the driver does not *require* it). The `ShellReady` newtype stays SEW's to mint;
  C4 introduces no new checker (pure wiring over the proven `ledge_dom_certified` + CAP-OUT-LINK).
  *2026-08-08 · still deferred(→M5/SEW) · `closure::ledge`*

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
  - **Producer landed (C3, milestone-c).** `closure::trim` is the missing CLIP producer: it builds
    `b_J` and the retained-side field `G_i = (C_i − x₀)·b_i` as three σ-rational coefficients
    (`g0`, `g_mu = ∂_μG`, `g_w = ∂_wG`) and drives all three reused checkers from a real joint —
    `clip_w_cert`/`clip_mu_cert` (the cleared `g_w²`/`g_mu²` `reg_q` gauges), `trim_local_cert` (outer
    corners + one confinement fiber), `sigma_deriv_corners` (the signed CLIP-σ leaf), and
    `field.corners` → `clip_dom` (the fiber census). The 90° cylinder self-fold certifies TRIM-LOCAL +
    CLIP-W end-to-end, so the checkers are no longer only hand-built fixtures. **Still deferred as
    *searcher-completeness* refinements** (the checkers are sound regardless; this is about the searcher
    *automatically* supplying complete inputs): **(a)** deriving the CLIP-μ failing sub-spans from the
    *irrational* `R_W` roots (the caller currently supplies sub-spans), and **(b)** Sturm-isolating the
    fiber sign-event σ's rather than sampling representative stations. *2026-08-08 · producer done,
    coverage-completeness deferred · spec §8.5 CLIP*

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

- **The σ-station partition is derived from one representative rail and never re-checked against the
  patches it is supposed to serve.** `sigma_stations` builds its anchor from the slice's **lower**
  rail only — documented as sound because "all four σ-rails share one denominator (the `µ⁻` base
  fixes it)" — but `trim_surf` calls `reduce()`, whose gcd division depends on the numerator, so two
  rails over the same chart can reduce to *different* denominators. It is already loose for the
  general polygon channel (AUTH.2e), where a slice's lid rails are per-edge affine functions the
  partition never saw. Nothing has produced a bad weight, and `sigma_splits` is deliberately
  conservative, so this is a latent fail-open rather than a bug: **the emitted patch's weights are
  not verified, only a proxy's.** The cheap close is a check in the builder's second pass —
  `positive_weights(anchor.den(), sk, sk1)` per emitted rail, refusing rather than emitting — which
  would also turn the outer-wire polygon channel (§12.4) from "consistent with the existing
  looseness" into something actually gated. Noticed while sign-normalizing the weights for AUTH.3c;
  deliberately not folded into that slice. *2026-08-17 · open · `export::brep_build`*

- **A local `xtask gate --full` failed three OCCT/CGAL legs once and passed them on a clean re-run —
  cause unidentified, so it is recorded rather than dismissed.** The run reported `FAIL` for *OCCT
  STEP export*, *OCCT STEP doctests* and *CGAL differential*, while its own compile legs
  (`clippy --features step`, `--features cgal`) passed in the same invocation. Running the failed
  legs' exact commands by hand immediately after — same shell, same `nix develop` — passed
  (91/91 on `export --features step`), and a full re-run was green on all 16 legs. The one
  suspicious antecedent is that a **non-nix** `cargo clippy -p export --features step` had been run
  just before, which fails at the OCCT build script; a poisoned `target/` fingerprint is the
  hypothesis, not a finding. Worth naming because the failure mode is the dangerous direction — a
  gate that reports FAIL when the code is fine trains people to re-run until green, which is how a
  real failure gets waved through. If it recurs, capture the leg's stderr (the gate summary alone
  does not carry it) before re-running. *2026-08-16 · watching · `xtask/src/main.rs` gate legs*

- **γ≠0 chord-certified unroll re-runs the verified quadrature per rail edge — needs the perf pass before multilayer.** The piecewise unroll's anchor frames call `gamma_at(edge.lo)` and the checker's `point_from_on` integrates `∫γ′` from the region window start per edge/subdivision point, each at the region's full `panels` count regardless of span width. `PiecewiseDevelopment` now memoizes the cumulative-γ prefixes (budget-keyed `RefCell` — the dominant cross-region re-integration is gone), but the per-edge own-window integrals remain O(edges × panels × interval-transcendental evals) — a γ-heavy part at fab segment counts takes minutes in debug builds. The **piecewise fold** (PR 3) joins the same family: each `invert_sigma_from` bisection step re-integrates `directrix_between(lo, mid)` at full panel count, so a γ-region fold costs O(iters × panels) — same fix family applies. Candidate fixes, in order of principle: **(a)** scale the quadrature panel count with the integration span (the midpoint-slope rule's error is O((w/panels)²·w), so short spans need few panels — soundness is panel-count-independent); **(b)** an incremental frame walk in the unroll (edges march monotonically; each frame extends the previous by one short increment, the demo's old `gamma_grid` shape); **(c)** release-profile evaluation for authoring workflows. *2026-08-14 · open · `develop::part` (`gamma_at`/`anchor_pieces`), `develop::cone::directrix_between`, `develop::fold` (pw) · surfaced by the `author` facade's piecewise tests (budgets right-sized there in the meantime).*

- **The self-lapping demo keeps a C¹ cubic ramp; the C² quintic is blocked by the trim geometry, not the γ-quadrature.** As of task #216 the `integrate_on_slope` quadrature develops the quintic smootherstep tightly (the original blocker is gone), but restoring it (now `SupportFn::InU` in `crates/author/examples/self_lapping_cone.rs` — the old hand-wired demo is deleted) fails at the **trim rail**: the quintic reshapes the ramp surface so the fixed outer/inner cylinder cuts (tuned for the cubic) no longer produce a smooth low-degree rail over the ramp region (D1-outer ε≈14.7; raising the fit degree makes it *and* the unchanged body region worse — Runge, i.e. a geometry/branch wall). **To restore C²:** re-tune the trim-cylinder placement/radii (and/or sub-band the ramp) for the steeper quintic surface, then regenerate the SVG/STEP artifacts. Purely a demo-geometry task — the kernel quadrature is done. *2026-08-12 · open · the self-lapping recipe (now `author/examples/self_lapping_cone.rs`)*

- **CLEAR (`develop::bonded::clear`) is a brute-force adaptive-AABB min-distance search — sound, but the user flagged it as unsatisfying (2026-08-11); resolve in future.** It de-risks the seam-ramp subdivision *technique* (S3.2, `4ff425a`) and is correct/fail-closed, but it is inelegant on several axes: **(1) rails-only** — it certifies the sheets' `(µ,w)`-rails, not the full-band *surface* (a world-AABB over a whole ruling is dominated by its length, so µ must also be subdivided — an O(more) mechanical extension, unbuilt); **(2) AABB-loose** — world-axis boxes of a rotating ruled arc are slack, forcing extra subdivision; **(3) linearly-convergent** — interval subdivision tightens linearly, so certifying a keep-out *near* the true min is deep/slow (fine when the clearance is comfortable, which is the physical case, but brittle otherwise); **(4) ignores the shared-frame structure** — the two lapping sheets differ by *exactly* the pedal-offset field `c_B(σ') − c_A(σ')` (µ,w-**independent**, a σ'-only rational vector), so the true min-distance is really a **1-parameter** problem (support gap + a correspondence residual bounded by the ruling regularity), not a blind 2-parameter box search. **A better future certificate** should exploit that structure — reduce to the support scalar plus an exact/Sturm residual bound (the §7 correspondence-lemma "stationary support ⇒ same-ruling" made quantitative), i.e. closer to the rejected "normal-gap + residual" option but made rigorous — giving an *exact/structural* CLEAR (no linear-convergence brute force) that also covers full surfaces. Keep the current `clear` as the honest baseline + the technique-of-record until then. Owner: S3 follow-up / a `develop`-split-time revisit.

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

- **#292 spike, GO: certify the wall in the chart and recognition stops mattering (2026-08-18).**
  `develop::cut::ruling_cut_fit` measures the rail's distance to its wall the way the chart already
  knows how, and it answers every question the spike was set:

  > `dist(C(σ), wall) ≤ |s(σ)| / |∂s/∂µ̂| · |ruling(σ)|`, `s = a µ̂_fit² + b µ̂_fit + c` the rail's own
  > residual in the µ̂-pullback [`cut_mu_form`] — **an exact rational function of σ**.

  No ball, no gradient bound, no classification. The nappe becomes *which root*, decided at a point
  rather than over an inflated box.

  **Tightness, per rail, on the two-ramp device with a plain bore** (both arms certify, so they can
  be compared; `subdiv 160`):

  | rail | 3-D arm (closed form) | chart arm |
  |---|---|---|
  | outer, base cone | 1.4295e-1 | **6.6359e-14** |
  | outer, ramp ① | 1.6854e-2 | **5.1627e-4** |
  | outer, ramp ② | 2.0055e-2 | **1.7776e-3** |
  | outer, flat ccw | 2.2124e-2 | **6.6767e-14** |
  | bore, base cone | 6.2563e-8 | **5.8617e-14** |
  | bore, ramp ① | 6.4560e-3 | **9.6798e-4** |
  | bore, ramp ② | 1.2990e-2 | **7.5357e-3** |
  | bore, flat ccw | 7.9878e-3 | **3.6707e-14** |

  Tighter on **every** rail, by 10² to 10¹². That is not a surprise once stated: the 3-D arm
  re-derives from an inflated ball a quantity the chart holds exactly, and the `σ ↔ µ̂` cancellation
  it loses is the whole difference. Convergence on the ramp rails is clean `O(h)` — `5.1627e-4 →
  2.6597e-4 → 1.4182e-4` and `1.7776e-3 → 8.9200e-4 → 4.5317e-4` over `subdiv 160 → 320 → 640`.

  **And the wall that started this.** The drawing's Ø 8 rim — the imported circle `2.3e-14 mm`
  off-axis, which `RevCone` declines and the 3-D arm cannot certify at *any* subdivision — certifies
  at **ε 1.0384e-13**. All eight trim rails certify. Recognition is now what it should always have
  been: an optimization that can be dropped without losing a capability.

  **Two things the spike also found, and both are worth carrying.**

  - *A sign error that looked like success.* The first cut returned `ε 0.0000e0` on every rail. The
    mean-value term kept the slope's sign, so the "bound" was negative and `max` discarded it. A
    certificate arm that reports **zero** is the failure mode to fear most, and the only reason it
    was caught is that zero was implausible enough to check the terms. *Assert `ε ≥ 0` in the test,
    and be suspicious of a bound that improves by orders of magnitude in one edit.*
  - *The dependency trap it replaced.* Written as `|µ̂_fit − µ̂*|` with the two enclosed
    independently, the bound reads `5.5e-1` where the rail is exact to `6.3e-8`: each of the two
    swings by `~0.5` across a σ-piece and the difference inherits both swings. Forming `s` as a
    rational function *first* and dividing by a slope **lower** bound is what keeps the
    cancellation — a loose root enclosure cannot spoil a lower bound.

  **Costs and open ends.** The chart arm ran the plain bore in `8.3 s` against `5.7 s` (+45%) —
  `s` is a RatFunc product, so this lands on #279's coefficient growth. It degrades near a tangent
  ruling where `∂s/∂µ̂ → 0`; those windows are already isolated exactly by `tangent_events` and owned
  by the p-curve arm, but that hand-off is *not yet measured*. And with all rails certifying, the
  drawing's next blocker is a different subsystem: `fit_cut_rail` — the **float oracle** — declines
  on span `[−1.0397, −0.0602]`, which is neither certificate nor resolver.

  *2026-08-18 · open(#292) · `crates/develop/src/cut.rs`*

- **Recognition must never be a capability gate — and the general arm is not yet good enough to make
  that true (2026-08-18, → #292).** The user rejected backward-error recognition as the fix for the
  entry below, in two sentences worth keeping verbatim in spirit: *"I really don't understand how it
  can be controlled and at which level the near-miss should convert to intent. Technically if we
  have an exact intent, we can build exact geometry inside the kernel without resorting to import."*
  And: *"even if we'll fix it by snapping to an exact coax cone, the next authored curve can be
  intentionally not a coax cone and this will fail again."* Both are right, and together they name
  the actual architectural defect: **whether a cut can be performed at all must not depend on a
  classification.** A tolerance that decides *meaning* has no principled setting, and a fallback
  that only works for the shapes the classifier recognizes is not a fallback.

  **First half, fixed and measured.** The refusal was not even about the eccentricity. The general
  quadric arm's nappe condition — "the ball in which the first-order lemma places a zero must lie on
  the authored nappe's side" — was checked once at a **fixed** `clearance/2 = 3.5 mm`, deliberately,
  to bundle a DRC cushion into a soundness gate. The device's imported bore clears its drafted
  cutter's apex plane by `r·tan β = 3.61 mm`, so the cushion had ~0.1 mm of headroom and the ℓ¹
  inflation ate it. Instrumented, the selector **holds at `r/2`, `r/4`, `r/8`, `r/16` and fails only
  at the constant**. Checking it at the ball the bound actually closes on is strictly sufficient for
  the lemma's own conclusion, and both §4.1 conditions still bite — a trace that genuinely reaches
  the mirror nappe or the apex fails at every radius, `NappeCrossed` as before. Result on the
  two-ramp device with the drawing: `Refuted(CutUnresolved)` → **`Unresolved`**, a refinable
  looseness rather than a fault. 783/783 green.

  **Second half, open, and the more interesting one.** Refinable is not the same as convergent.
  Sweeping the rail fit: `subdiv 160 → ε 1.1220e1`, `320 → 3.7633e0`, `1280 → 7.0000e0`,
  `5120 → 7.0000e0`. Non-monotone, then pinned at the clearance. So the general quadric arm is not
  merely *slower* than the closed forms — on this wall it does not converge at all, and 32× the
  subdivision buys nothing. That is why recognition became load-bearing in the first place: it was
  never an optimization, it was the only arm that worked.

  **The proposal that removes recognition from the capability question altogether.** The general arm
  measures distance in 3-D: enclose the traced point in a box, inflate to a ball, bound `|F|/|∇F|`.
  It pays for that with the lost `σ ↔ µ̂` cancellation, which is exactly what saturates. But the
  chart already has the quantity we want, exactly and rationally: the resolver computes every wall's
  **µ̂-pullback** `a(σ)µ̂² + b(σ)µ̂ + c(σ)` (`cut_mu_form`) to decide the shadow at all. Distance to a
  surface is an *upper* bound problem, so any point on the surface will do — take the point on the
  **same ruling** at the wall's own root `µ̂*`, and

      dist(X(σ), wall) ≤ |µ̂_fit(σ) − µ̂*(σ)| · |r(σ)|

  is sound, needs one `sqrt` enclosure for the root and one for the ruling speed, and is tight
  wherever the ruling meets the wall transversally. No ball, no gradient, no recognition — and the
  nappe question becomes "which root", which `BranchSide`/`RootPick` already carry exactly. It
  degrades near a tangent ruling (the two roots collide), and those windows are already isolated
  exactly by `tangent_events`, which is where the p-curve arm takes over. If this holds up,
  `RevCone`/`RevCylinder` become a pure speed/tightness optimization — which is all a recognizer
  should ever be — and an *intentionally* non-coaxial cut certifies through the same door.

  *2026-08-18 · open(#292) · `crates/develop/src/cut.rs`*

- **A circle 2.3e-14 mm off-axis is not a cone of revolution, and that is enough to refuse the cut
  (2026-08-18, → #292).** The user checked the entry below against *their* test device — the two-ramp
  recipe in `crates/author/examples/lapped_cone.rs` — where the drawing's tab lands on conical sheet
  in **both** passes, and reported that it is refused anyway. It is, and for an unrelated reason:
  `Refuted(CutUnresolved { op: 1 })`, from `rail (1, Wall(2, false)) region 0 span [−1.125, 0.4625]
  → NappeCrossed`. Wall 2 is the Ø 8 rim — the plainest wall in the file. **No section splits
  anywhere in that run**, so this is a second, independent blocker, and the entry below overreached
  in saying the drawing certifies on a cone: absence of a split was measured, "it certifies" was
  not.

  **The cause is a knife-edge exact predicate meeting a real file.** `RevCone::recognize` is a chain
  of exact rational equalities. The drawing's rim circle is `cx = −1.7e-15`, `cy = +2.26e-14`,
  `r² = 15.999999256…` (`r = 3.9999999070`). The cutter's apex is on the axis, so a circle whose
  centre misses the axis by `2.3e-14 mm` sweeps an **oblique** cone; recognition declines, correctly,
  and the wall falls to the general-quadric arm — whose nappe test inflates the traced box by
  `clearance/2 = 3.5 mm` and needs the whole ball on one side of the selector. It is not, so
  `NappeCrossed`: precisely the fault #288/#289 added `RevCone` to remove. Measured side by side on
  the same device — plain bore **8/8 walls recognized, Verified in 5.7 s**; the drawing **4
  recognized, the rim declined, refused in 24.7 s**.

  **The lesson, and it generalizes past this file.** Exact recognition is the right doctrine for
  *authored* geometry and the wrong one for *imported* geometry: no drawing will ever land on the
  knife edge, and every near-miss silently costs the closed form — a seven-orders-of-magnitude ε
  cliff (#289) or, as here, an outright refusal. Recognition has to be a *verified proposal with a
  backward error*, which is what its own docs already claim it is — and the cheap place to do that
  is the **builder**, not the quadric: `Cast::circle_wall` knows the circle and the apex, so "is
  this a cone of revolution" is "is the apex on the circle's axis", and the near-miss is a length
  rather than an optimization problem. #292 carries it.

  *2026-08-18 · open(#292) · `crates/develop/src/cut.rs`, `crates/develop/src/extrude.rs`*

- **The drawing's tab is refused by the ramp, not by its own flanks — and the same fault name covers
  a second open bug (2026-08-18).** #291 was filed as "the resolver cannot place an imported
  outline's tab", with `choose_comps` named as the suspect and an exactly-radial flank named as the
  cause. Instrumented, both are wrong, and the way they are wrong is the lesson: **a fault name that
  points at the wrong subsystem costs a whole session.** `AmbiguousRegion { op: 1 }` says "the
  resolver could not decide"; what actually happens is the resolver deciding correctly and hitting
  the limit of what its *model* can hold.

  **The mechanism.** A region is modelled as **one µ̂-interval per σ** — a lower rail and an upper
  rail, both graphs over σ — plus interior holes carved by a single subtract op. A **tab** left in
  the bore is material reaching inward. A ruling that runs *across* the tab instead of along it
  enters the tab, leaves it, crosses a sliver of cut, and re-enters the sheet: the kept material is
  **two µ̂-intervals at one σ**. `sample_comps` merges them and records the gap as a hole of op 1;
  the role derivation then finds op 1 both holing and bounding and refuses. The refusal is *right* —
  the gap opens into the exterior at the low-σ end of its band, so shipping it as a hole would emit
  a closed island where the part has an open bay — but its name was not.

  **The controlled measurement, inside one run.** The drawing's tab passes the 410.7° chart twice at
  the same plan azimuth, and the two passes land on different sheet:

  | pass | region | `h` | ruling's plan miss | section |
  |---|---|---|---|---|
  | σ ∈ −1.079…−0.927 | 0 | `0` | **exactly 0.000000000** | one interval ✓ |
  | σ ∈ +0.888…+1.049 | 1 (the ramp) | `0 → 1/4` | up to **0.481 mm** | **split** at σ = 0.897, 0.906, 0.915 ✗ |

  Same cut, same walls traversed in the same order (`Wall 2→3→5→6→7→1→2`, the r = 4 rim, the root
  fillets, the flanks, the tip). Only the sheet differs, and the number that separates them is
  geometric and blunt: on a cone (`h′ = 0`) a ruling's plan projection passes exactly through the
  axis, so a **radially** flanked tab is entered once and left once; across the ramp it misses the
  axis by 0.481 mm, against a tab **0.35 mm** half-wide at its root. *On the ramp the ruling is
  further off-axis than the tab is wide.* Corroborated by moving the ramp off the tab (`ramp` σ
  ∈ (0.1, 0.5)): every gap disappears and the refusal moves on to an unrelated rail fit.

  The split gap runs from the tab's **root fillet** (`Wall 3`) to its **flank** (`Wall 4`), born at
  the flank∩tip-fillet corner and pinched shut at the flank∩root-fillet corner — the tab's own two
  corners, 3.8° of azimuth apart, which is why exactly three of the 48-per-region samples see it.

  **The straight-sided tab is the other half of the same statement.** Vertical flanks are not radial,
  so a radial ruling crosses them sideways too — that fixture splits the section on the *base cone*,
  and at **every** width tried (half-width 0.347 → 2.400 mm), because the two-interval band lives
  between the tab's corner azimuth and its flank azimuth and every width has one. The earlier
  reading ("not sampling density, therefore `choose_comps`") had the first half right.

  **One model limit, two open bugs.** #287 — a too-steep seam ramp whose edge of regression sweeps
  `≈0.9·max|h|/Δσ²` into the kept material — reaches the *same site* by separating the material into
  two stretches at one σ, and the entry below already noted its fault "names an op-role conflict and
  points at the inner trim; the op named is innocent". Both now raise
  `PartFault::SectionNotSimple { op }`, whose docs carry the mechanism and both routes to it.

  **What this hands on.** Not a patch: the region's boundary has to be *traced as a loop* over the
  event partition — which AUTH.2c already does for a cutter's footprint — instead of fitted as one
  lower rail and one upper rail.

  ⚠️ **Correction (2026-08-18, same day).** This entry closed by saying "the drawing cuts a cone but
  not a ramp". Only half of that was measured: the *absence of a split* on conical sheet was, "it
  cuts" was not. On the two-ramp device, where both tab passes are conical, the drawing is still
  refused — for the unrelated reason in the entry above (#292). Two independent blockers, and both
  must land.

  *2026-08-18 · open(#287, #290, #291) · `crates/author/src/{resolve,part}.rs`,
  `crates/author/tests/rim_notch.rs`, `docs/cutter-extrude-design.md` §12.5*

- **The first real cut file: what it cost, and the three walls it hit (2026-08-17).** The user
  supplied `crates/acceptance/data/inner-cut.dxf` — the device's Ø 8 bore with a 10° tab reaching in
  to Ø 4, root fillets R 0.25, tip fillets R 0.15 — to replace the plain disc. Four separate things
  came out of trying to cut it, and only two of them are now fixed.

  **The import: the deferred junction lift was exactly what a real file needed.** `element.rs` had
  written down, and not built, the arc-to-arc junction rule: where two `ARC` entities meet, neither
  endpoint may move without leaving its own circle. A `LWPOLYLINE` of bulges never hits it — and the
  first real drawing is eight chained `ARC`/`LINE` entities with **four** such junctions. Built as
  specified (`ExactArc::regauged`): hold the centre, take the shared vertex as the new start, and
  carry the far end round by the arc's **own** sweep, applied as the rotation its two endpoints
  already encode — `cos = (u·w)/r²`, `sin = (u×w)/r²`, both rational, and `cos² + sin² = 1`
  *exactly* because `is_consistent` has already put both on one circle. So the arc is re-gauged,
  never bent, and its radius is what moved. Read: one loop, **δ = 2.6e-14**, closure gap **2.3e-10**.
  Two details worth keeping: the file declares `$MEASUREMENT 1` (metric) but no `$INSUNITS`, and
  those are different claims — the reader refuses rather than guess a 10× part; and the drawing's
  Ø 8 is stated as `r = 3.999999907`, which the importer carries rather than tidies.

  Also learned by a failing test I had written to assert the opposite: **a lens of two concentric
  arcs now closes**, because re-gauging the follower puts it on the leader's own circle and its sweep
  carries it exactly back to the anchor. Only arcs about *different* centres still refuse.

  **`RevCylinder`: the same conditioning hole as last session's `RevCone`, one apex kind over.** A
  profile swept from a direction rather than a point clears to a `Quadric` that `RevCone` cannot
  recognize (a cylinder's eigenvalues are `λ, λ, 0`, so it passes the double-root step and declines
  at `cos²α = 1`, not where that function's doc claimed), so the two apex kinds were costing seven
  orders of magnitude differently for no geometric reason. Measured on the device's
  own bore: **ε 5.5585e0 → 1.5266e0**, now digit-identical to the closed-form cylinder *and* to the
  drafted cone. Recognition is the same verified-proposal shape: rank one, `|a|² = λ·N_jj`,
  `b + 2Sp = 0`, `R² > 0`, every step an exact ℚ equality. One asymmetry with `RevCone` is
  load-bearing: a **live nappe selector is declined outright**, because the distance to a whole
  cylinder is a *lower* bound for the distance to half of one, and a certificate may not be
  optimistic.

  **What still refuses, and it is not what I first thought (→ #291).** A rim notch is not expensive:
  a wedge whose flanks clear the axis certifies at **ε 1.5266e0**, the plain bore's own figure. Three
  distinct refusals sit between that and this drawing — an exactly-radial flank gives `Pole` (the
  wall contains a whole ruling, so the µ̂-pullback degenerates, and the drawing's flanks are radial
  to **8.8e-16 mm**); a near-radial one gives `AmbiguousRegion` down to about 1/32 mm of miss; and an
  all-straight notch gives `AmbiguousRegion` at **every** width from half-width 0.347 to 2.400 mm,
  which rules out sampling density as the cause of that one.

  **And the negative result worth more than the fix I attempted.** `resolve.rs` substitutes a mixed
  profile's bounding-circle proxy for the per-wall tangent windows, and the comment beside it says a
  superset is the right error. Making it a union *did* localize the imported tab — its fault moved
  `AmbiguousRegion` → `DisconnectedRegion` — and **regressed three green acceptance tests**. So
  extra stations are not free: `choose_comps` refuses at any sample it cannot attribute, and a
  station near a wall's tangent ruling is exactly such a sample. Reverted; the comment's claim is
  wrong and #291 carries it.

  ⚠️ **Correction to this entry as first written (2026-08-18).** "Three distinct refusals" reads as
  three causes; the drawing has **one**, and `choose_comps` is not it — instrumented, the drawing
  never reaches `choose_comps` with an unattributable sample. See the next entry: the refusal is the
  role derivation, and the "near-radial flank" and "all-straight notch" figures are two symptoms of
  the same split section rather than a degenerate-wall ladder.

  *2026-08-17 · open(#290, #291) · `crates/interchange/src/{arc,element}.rs`,
  `crates/develop/src/cut.rs`, `crates/acceptance/{data,src,tests}`*

- **A cone of revolution has a closed-form distance, and using it made the drafted trim free
  (2026-08-17).** The pinned device now cuts **normal to the sheet** at both radii — the physical
  edge — instead of with a vertical cylinder, and the trim style costs nothing: same ε, same
  `subdiv`, and the two agree to the digit at the same radius.

  **What was actually wrong.** `CutSurface::Quadric`'s certificate bounds the distance to `{F = 0}`
  by inflating a **box** around the traced point until a first-order bound closes inside it. That is
  the right instrument for a general quadric and the wrong one for a cone: it throws away the σ↔µ̂
  correlation the two special surfaces keep by cancelling symbolically, so it needed 64× the split
  on a gore and more than the device could afford on an annulus; and because the ball has to clear
  the nappe selector by the full working radius, a cut passing within `clearance/2` of the apex
  plane was **refused** (`NappeCrossed`) rather than loosened. Neither is reachable with a cylinder,
  which is why every cut before this one was fine.

  **The fix is a recognition step, not a new surface kind.** `develop::cut::RevCone::recognize`
  extracts `(apex, axis, cos²α)` from `(M, b, c)` by exact linear algebra over ℚ — the double root
  of the characteristic cubic in closed form (a double root of a rational cubic *is* rational), then
  `S − rI` must be exactly rank one, then the apex must solve `Sp = −b/2` with `c = pᵀSp`. Every
  step is a checked equality, so an elliptic cone, a cylinder and a hyperboloid each fail a
  different one and fall back to the general arm. What it buys: in the meridian half-plane the
  nappe is a ray, and the distance to a ray is the perpendicular drop `|s·cos α − t̂·sin α|` where
  the projection lands on it, `|X − apex|` where it does not — both with rational radicands, so the
  irrational half-angle is never represented. Measured on the gore's inner bound: `Unresolved` at
  ε 3.3e1 → **Verified at ε 7.6e-8**, at `subdiv = 160`, and refining 8× no longer moves it — what
  is left is the degree-4 rail fit, which is the cylinder arm's behaviour exactly.

  Generalizable: **a certificate's conditioning is a property of the representation you reach for,
  not of the geometry.** The box arm was not a weaker claim about the same object; it was a claim
  made without the structure that was sitting in the coefficients. Worth asking, at any arm that
  needs an enclosure the others do not, whether the special case is recognizable — the recognition
  is cheap, once, and the verification is what makes it sound.

  Also: the second "wall" recorded below — `subdiv = 1280` refusing where 160 was `Unresolved` —
  was never a monotonicity defect. Verdicts are fail-fast, so the coarse run stopped at an earlier
  stage's `Unresolved` and never reached the cut that refuses. A worse verdict under refinement is
  worth suspecting, but the first thing to check is whether the two runs got to the same place.

- **The outer diameter was Ø 21.5, not Ø 43 — and the correction re-audited the fixture again
  (2026-08-17).** Halving the annulus dropped every feature placed in millimetres off the part
  (`Inactive` roles, genus 0). Re-placed by **scaling the direction vector, not re-authoring the
  point**: azimuth is what fixes which σ, which region and which ramp height a feature lands at, so
  a pure radial scale moves the drill and the slot while leaving every σ-pinned measurement — the
  `γ` probes, the ramp-band window, the region assignments — exactly where they were. The one
  exception is the drafted-sweep apex, which scales in `z` too, because the cast is a similarity.

  It also found the restated-constant tax for the **fifth** time: the seam drill's centre was
  written in `seam_drill_axis()` *and* inline in `self_lapping_cone_from`, so the round-trip test
  folded holes back onto a cylinder the part was no longer cut with. Now one reads the other.

  Two pins moved for reasons worth keeping. `refold` was measuring `|ρ² − r²|` — a **squared**
  residual, ≈`2r` times the length anyone reading `1/20` would assume, and one that scales with the
  square of everything; it is a radius now. And the ramp width at which the even profile beats the
  cubic moved `Δσ 11/32 → 3/16`, because that threshold is where the fold line's swing reaches
  material — a property of the *annulus*, not of the profiles.

- **The device is the product's size now, and the trim that made it necessary is the one thing
  still deferred (2026-08-17).** *(Superseded above: the trim landed, and the diameter was
  corrected to Ø 21.5.)* The annulus is Ø 8 → Ø 43 mm, the target dimensions. What landed:
  `LappedCone` carries **radii, not squared radii** (a normal cut needs `r` itself — its disc plane
  sits where the neutral surface *has* that radius — and `√(r²)` is not rational in general), plus
  a `TrimStyle` choosing between a vertical cylinder and a cut normal to the sheet.

  **What certifies, and what does not.** Cylindrical at the new proportions: develop ε 2.627 against
  a 3.5 gate, solid 1.337e-1, 18 faces, 0 free edges, 5.4 s. `NormalCut` certifies on a gore
  (`normal_trim.rs`, ε 2.277e-1, identical to a cylinder at the same radius) but **not yet at this
  device's proportions**: an annulus of aspect 5.4 wants a `subdiv` that runs past ten minutes, and
  at `subdiv = 1280` the inner cone refuses outright rather than loosening — which is a second
  wall, not the same one, since refining should never turn `Unresolved` into `Refuted`. So the
  pinned device stays `Cylindrical`, said out loud in the recipe rather than left to be discovered.

  **Re-pinning, with the causes kept apart.** `develop 3/4 → 13/4` is *exactly* the radius ratio:
  outer 5.115 → 21.5 is 4.2×, and ε 7.264e-1 → 3.053e0 is 4.2×. `solid 1/6 → 1/3` is 2.1× — half
  of that, because a solid's ε is set by the σ-slice chords and the azimuthal sampling did not
  change. `fold` and `refold` did not move at all and keep their old budgets: neither tracks the
  radius. A traced-cut bound went 1e-2 → 1/40 because the slot itself grew 2.8×.

  **The restated-constant tax, a fourth time — and this time it was paid off.** Four more absolutes
  had quietly stopped meaning anything: a "contour containing the whole panel" was a ±8 square that
  the Ø 43 panel now *bites*; the drafted-sweep apex was left at the old scale so the sweep missed
  the sheets; and two DRC gates were half of a clearance that had since changed. Each is now
  **derived** — the squares as multiples of the device's own `outer_r`, the gates from a new
  `Part::drc_clearance()`. That accessor exists precisely so a test can say "under this part's own
  gate" instead of restating the number.

  Generalizable: **a re-proportioning is the cheapest audit of a fixture there is, and it keeps
  finding the same class of defect.** Every constant that needed a hand edit was one that should
  have been read from the recipe. The count across this session is now four independent instances;
  the fix each time is to express the relationship, not the value.

- **Normal-cut trims work: two bugs, and the second one is a convention mismatch that only a
  downward-opening wall could expose (2026-08-17).** The construction was right from the start; the
  kernel had two independent defects between it and a certificate, and a cylinder hit neither.

  **① The oracle declined the whole class.** `fit_cut_rail` returned `None` for every
  `CutSurface::Quadric`, reasoning that a general quadric wall turns around in σ and a graph rail
  cannot follow one that does. It is the *search*, not the certificate — `cut_fit` re-checks the
  proposal against the real surface — so declining was conservatism that cost the capability.
  Fixed by sampling `cut_mu_form` at the same Chebyshev nodes the cylinder arm uses.

  **② The pick and the label meant different things.** `BranchSide::Wall(_, upper)` says which end
  of the cutter's **shadow** an end is; `RootPick` names a **root** of the µ̂-quadratic. Those
  coincide only when the quadratic opens *upward*, so that "inside the cutter" is the interval
  **between** its roots — which every cylinder satisfies (`a = |u|² − (u·â)²/|â|² ≥ 0` by
  Cauchy–Schwarz, always). A cone wall met twice on one side has `a < 0`: inside is the
  *complement*, so a shadow piece's **lower** end is the quadratic's **upper** root. Exactly
  inverted, and unobservable until a downward-opening wall existed. `mu_form_opens_up` now reads
  the sign of `a` and reconciles them.

  **What it looked like from outside, and why that was misleading.** The symptom was
  `CutFitFault::NappeCrossed` — and the certificate was *right*: with the wrong root the oracle
  traced the far branch at `µ̂ ≈ −12.3`, whose 3-D points sit at `z ∈ [−100, −30]` against a cut
  circle at `z = −2.77`. The fitted rail really was off on the mirror nappe. Two hypotheses died
  on the way: root-pick (tested by flipping — *vacuously*, because at that point the oracle still
  returned `None` before the pick was read) and apex proximity (the nappe numbers, `n_z = 325/144`
  and `d = −235225/20736`, matched a hand computation and left 5.09 of margin against a required
  2.26). What settled it was bucketing the actual box the check sees.

  **The result corroborates itself.** A vertical cylinder and a normal-cut cone at the same radius
  meet the sheet in the *same circle* — both are surfaces of revolution about the chart's axis —
  and they now certify to **ε 2.277e-1 both**, agreeing to the digit through two unrelated
  representations: a `Cylinder` with a symbolic residual and a `Quadric` bounded by a first-order
  ball. The quadric route pays 64× the `subdiv` (10240 against 160) because its arm encloses the
  traced point in a box instead of cancelling the surface equation against the chart fields — a
  conditioning cost its own doc comment predicted, and it says `Unresolved` rather than certifying
  loosely. Both pinned in `crates/author/tests/normal_trim.rs`.

  Generalizable: **two vocabularies for the same geometric end will agree on every case you have,
  until sign flips.** "Which end of the shadow" and "which root of the quadratic" had been
  interchangeable across every fixture in the repo, because every cutter so far was met *between*
  its roots. Nothing marked the assumption because nothing had violated it.

- **A normal-cut annulus bound is exactly constructible and the kernel cannot yet resolve it
  (2026-08-17) — superseded by the entry above; the diagnosis there is the correct one.** Today's annulus is bounded by vertical cylinders, which meet the 42° cone at a
  bevel; a real trim is cut perpendicular to the sheet. The construction the user specified turns
  out to need **no new cutter kind and no approximation**: put a disc of radius `r` in the plane
  where the base cone's *neutral* surface has that radius (`z = −(72/65)r` on `72ρ + 65z = 0`),
  and put the sweep apex on the axis so the generatrix through the rim runs along the cone's own
  normal `(72, 65)/97` — i.e. `z_apex = −(97²/(65·72))·r`. Both cutters then come out with
  generatrix ratio `Δρ/Δz = 72/65` **exactly**, half-angle `arctan(72/65) = 90° − β`,
  complementary to the base cone as a normal cut must be. `Cutter::extrude` of a
  `Profile::circle` from an `Apex::point` *is* that cone, and every number is rational.

  **It does not resolve** — `Unresolved(ε = clearance)`. **My first diagnosis of *where* was wrong
  and the correction is the substance of this entry.** I attributed it to `assemble_flat` (the 2-D
  boolean) because a grep found exactly one site returning `Unresolved(clearance)`; a probe placed
  at that site never fires. It is `RErr::Loose` from `export::trim::certified_rail_surface`,
  reporting the clearance as its own bound — the rail **fit**, one stage earlier.

  Finding that needed a *controlled* A/B, which the first pass was not: it had changed the radii,
  the pick, `inner_r2` and the clearance all at once. One blank (flat gore, `h ≡ 0`, outer cylinder
  `r = 5`, named witness, clearance 3, segments 64), inner bound the **same circle** `ρ = 5/2`,
  only the cutter differing:

  | inner bound | verdict |
  |---|---|
  | cylinder | Verified, ε 2.277e-1, 0 holes, roles `[LowerBound, UpperBound]` |
  | cone | Unresolved, ε 3.000e0 |

  **The resolver is completely correct for both.** Probed: identical roles, one run over the whole
  σ range, and *numerically identical* µ̂ intervals sample for sample — `(−6.2779, −3.1389)`,
  `(−3.2647, −1.6323)`, `(−2.5557, −1.2778)`, `(−4.1509, −2.0755)`. It must be so: the base cone
  and the cutter cone are coaxial surfaces of revolution, so both meet the sheet in the circle
  `ρ = 5/2`, which on a cone chart is `µ̂ = const`. The only difference anywhere is the end label —
  `(1, Lower)` against `(1, Wall(0, false))`.

  So: `walls()` is fine (a disc dedupes to one carrier quadric), the role derivation is fine, the
  shadow is fine. What fails is fitting a rail to that wall — **on a rail whose true value is
  constant**, the easiest fit there is. A fitter that cannot manage a constant is not hitting a
  conditioning wall; something upstream of it is wrong for this wall. The root pick is ruled out
  (flipping it changes nothing). The standing hypothesis is that a **coaxial** cone-cone pullback
  degenerates a coefficient the general path assumes nonzero — every extruded fixture so far
  (`lap_slot`, `ell_slot`, the sketch panels) has its apex off the chart's axis, so a cutter
  sharing the chart's axis is simply an untested configuration. Task #288.

  Two things worth keeping from the correction. **"Exactly one site returns this value" is not a
  diagnosis** — it is a hypothesis, and the cheap confirmation is a print at that site, which took
  one run. And **an A/B that changes four things measures none of them**: the first pass looked
  like evidence of a broad gap ("extruded cutters cannot bound") and the controlled one showed the
  bound machinery working perfectly right up to the last stage.

  Worth noting for the roadmap conversation this came out of: the *authoring* side of the product
  question was already answered — the kernel's existing vocabulary expresses a manufacturing-real
  normal cut exactly, in ℚ, with the orthogonality holding by construction rather than by fit.
  What is missing is one resolver path, not a representation.

- **The ψ-correction to the ramp split: measured, and rejected on the metric that matters
  (2026-08-17).** `u` is affine in σ, but curvature turns with `ψ ∝ arctan σ`, whose speed
  `dψ/dσ ∝ 1/(1+σ²)` varies **1.508×** across the acceptance ramp — so `EvenCurvature`'s
  σ-midpoint split is skewed in the parameter that governs `R₁`. Since at the ramp's two ends
  `h_σ = 0`, the weight on `h_σσ` there is exactly `(1+σ²)²`, and balancing the two ends wants

  > `w₂/w₁ = ((1 + hi²)/(1 + lo²))²`

  — **exactly rational**, no approximation of `arctan` anywhere, even though the quantity being
  evened is an angle. For the acceptance ramp that is `(98/65)²`, putting the split at `9713/13829`
  instead of `11/14`.

  **It works on the proxy and loses on the product metric.** Peak `|µ̂_fold|` at the design width
  `Δσ = 3/7`: cubic 2.474, midpoint split 1.641, weighted split **1.445** — a further 1.14× (my
  endpoint-balancing algebra had predicted 1.39×; it over-predicts because the peak is not purely
  at the ends, the mid-ramp `2σ h_σ` term matters). But swept against *ramp width*, which is the
  thing the knob exists to buy:

  | `Δσ` | cubic | midpoint split | weighted split |
  |---|---|---|---|
  | 3/8 | ε 7.48e-1 | ε 7.48e-1 | ε 7.48e-1 |
  | 11/32 | Unresolved 9.62e-1 | **ε 7.60e-1** | **Refused** |
  | 5/16 | Refused | Refused | Refused |

  The weighted split trades one end of the ramp against the other, and which end binds depends on
  the width: narrowing the first half to balance the far end makes the *near* end the constraint.
  It reduces the peak at the design width and **costs** ramp-angle range at the frontier. Reverted;
  `EvenCurvature` keeps the σ-midpoint split.

  Generalizable, and the reason this was worth measuring rather than reasoning: **a proxy that
  ranked two options correctly once can rank them backwards elsewhere.** Peak `|µ̂_fold|` at a
  fixed width was the right proxy for cubic-vs-even (predicted 1.50, measured 1.507) and the wrong
  one for midpoint-vs-weighted, because the two profiles differ in *where* the peak sits, not just
  how big it is. The product question was never "what is the peak" but "how narrow a ramp still
  certifies" — and that one had to be swept.

- **The seam ramp's cubic spends its bend at the joins, and an even one buys 1.5× of ramp angle
  (2026-08-17).** `SupportFn::Smoothstep` is `3u² − 2u³`, so `h″ = (6 − 12u)·Δ/L²` — *linear*,
  peaking at `±6Δ/L²` at **both ends** and passing through zero mid-ramp. All the bending is
  crammed into the two joins with the constant neighbours and the middle of the ramp does no work.
  That is the uneven distribution, exactly.

  **The two symptoms are one number.** `R₁ + w = det J / |n′|²` (`develop::bonded`), so material
  at `µ̂` has `R₁ ∝ (µ̂ − µ̂_fold)` and bending strain goes as `w/(µ̂ − µ̂_fold)` — where `µ̂_fold`
  is the `det J = 0` rail, the same fold line whose excursion caps the ramp angle. Peak strain and
  the angle limit are not two constraints to trade off; they are one quantity seen twice.

  **What is achievable, and it is a closed-form optimum.** Subject to `h′ = 0` at both ends and a
  rise `Δ` over width `L`, the profile minimising peak `|h″|` is the bang-bang pair of parabolas,
  `h″ = ±4Δ/L²` — constant in magnitude, which is what "even" means. `4` against `6`: 1.5×.
  Measured peak `|µ̂_fold|` on the acceptance ramp, **2.474 → 1.641 = 1.507×**, against the 1.500
  the peaks predict, at an *identical* certified ε. Swept for where it bites: both profiles hold
  at `Δσ = 3/8`; at `Δσ = 11/32` the cubic's ε has run past the part's own DRC gate (9.6e-1
  against 5/6) while the even profile certifies at 7.60e-1; by `5/16` neither holds.

  Landed as `RampProfile::{Smoothstep, EvenCurvature}` — `Smoothstep` the default, since every
  pinned measurement was taken on it. No engine change was needed: `SupportFn::InU` already takes
  an arbitrary rational function of `u`, and the recipe already emits multiple bands, so the
  optimum is two polynomial half-bands over ℚ and stays exact.

  Two things deliberately *not* taken. `EvenCurvature` is C¹ but its `h″` **steps** at the ends
  and midpoint — no crease, but a curvature step is its own stress concentrator, and a trapezoidal
  `h″` would round it off for some of the 1.5×. And there is a second ~1.3× in the
  *parametrization*: `u` is affine in σ, but curvature lives in the turning angle
  `ψ = (260/97)·arctan σ`, whose `dψ/dσ` varies 1.35× across this ramp, so even an even-in-`u`
  profile is skewed in the parameter that governs `R₁`. Correcting that needs a rational
  approximant (arctan is not rational), which is sound here precisely because `h` is *authored*
  rather than approximated — the certificate bounds whatever profile it is given.

  Generalizable: **"C¹ and gap-free" is a smoothness claim, not a distribution claim.** The cubic
  was chosen for the joins it makes and was never asked what it does *between* them; the answer is
  "nothing in the middle, everything at the edges". Worth asking of any interpolant whose second
  derivative is what the physics reads.

- **The acceptance device's dimensions were arbitrary, and scaling it to the real ones is what
  found out which of its pins were physics (2026-08-17).** `thickness: 1/20` traced back to
  `839ff53` (2026-08-11), where `brep_trim_solid` first needed a `w` window and got a round
  rational; the 917-line demo carried it, the facade rewrite carried it, `LappedCone` inherited it
  as a *parameter*. No comment, doc or commit message ever justified it. The half-angle was real
  all along (`sin β = 65/97` via the Pythagorean `(72, 65, 97)`); the lengths were not.

  **The derivation chain, so the numbers are auditable rather than asserted.** `t = 6/25` mm
  (240 µm) from `paper.md` §338's `w ∈ ±120 µm` — which independently confirms `neutral = 1/2`.
  `Δ = 1/4` mm is *pinned, not rounded*: the certified SHEAR is `δ = Δ·cot β = Δ·(72/65) = 18/65`,
  so `Δ` can only be `1/4`. Then `g = Δ − t = 1/100` mm — a 10 µm ACF bondline, consistent with
  `SEP ≡ ACF gap` — and `c = t/2 + g/2 = 1/8` keeps the one-ramp device with a ramp height of
  exactly `Δ`. Every length scales `5/3` to put the off-axis inner bound's closest approach at the
  stated inner Ø 5 mm.

  **The device certifies on the real numbers**: 771/771, `develop` ε 7.264e-1, `fold` 3.345e-1,
  `refold` 3.745e-2, `solid` 1.278e-1, genus 2, watertight, and the full demo emits 5 derived holes
  with a 2.7e-3 slot residual.

  **What the scaling exposed, which is the actual value of the exercise.** Every constant that had
  to be edited by hand was a *restated* one — a number that should have been read from the recipe
  and was copied instead:

  * `self_lapping_part.rs` measured the refold residual against a hard-coded `(-0.5, 2.7, 1/40)`
    copy of the drill axis — the exact thing `seam_drill_axis()`'s doc comment says it exists to
    prevent ("so a round-trip check tests the *same* cylinder the part was cut with instead of
    restating its numbers"). It reported 4.62 against a 1e-2 budget; read from the recipe it is
    3.7e-2. A 460× phantom failure that would have been "read" as the scale change.
  * The drafted-slot apex `(27/40, 27/10, 12)` did not scale, so the sweep missed the sheets and
    the taper test failed by finding *no* slot holes rather than by measuring a wrong taper.
  * The flat-authored hexagon's centre and radius, the slot's `far.offset` window (pinned to the
    old `Δ = 1/10`), and the DRC gate `1/2` (half a `clearance` that itself did not scale) were all
    absolutes standing in for relationships. Each is now derived — the offset window as a fraction
    of `Δ`, the budgets as the spec's own `t + g`.

  Re-pinned VV.2 with the two effects separated: `develop 0.45 → 3/4` and `solid 0.1 → 1/6` are the
  pure `5/3`; `fold 1/3 → 7/20` and `refold 25/900 → 1/20` carry a surcharge because the ramp now
  climbs `Δ = 1/4` over `Δσ = 3/7` where it climbed `1/10` over `1/2` — 2.5× the step in 6/7 of the
  azimuth. That surcharge is the ramp, and it is the number to watch if the ramp is tightened.

  Generalizable: **an arbitrary fixture dimension is not neutral — it hides which pins are physics
  and which are bookkeeping.** Nothing was *wrong* while every length was 1; the restated constants
  and the derived ones were indistinguishable because they never had to disagree. Changing the
  scale made them disagree, and the ones that broke were exactly the ones that should never have
  been written down. A cheap audit for any fixture: multiply it by a constant and see what fails.

  Open, not resolved here: `docs/agent-glossary.md` says the device wraps "~1.49 turns", but the
  demo measures a 275.2° developed sector against 240.9° per turn — 1.14 turns. `paper.md` §338
  warns that an earlier ledger conflated the developed-sector and spatial-azimuth frames, so this
  is probably the same conflation surviving in the glossary. Not touched without a decision.

- **A seam ramp has a fold line, it sweeps across the ruling, and a too-abrupt ramp drags it
  through the part (2026-08-17).** Steepening the lapped cone's CCW ramp (`ramp_start` 1/2 → 11/16,
  so `Δσ` 0.25 → 0.0625) or raising its seam offset (`c` 0 → 1/10) each independently turns
  `develop`/`solid` into `Refuted(AmbiguousRegion { op: 1 })`, op 1 being the inner-radius
  `subtract`. Raising `segments` 16 → 64 does not help. **This is a real geometric limit, not a
  bookkeeping artifact** — the finding was initially mis-read as the latter and the correction is
  the point of the entry.

  **What the refusal actually detects.** A region's support ramp bends the sheet in σ, and the
  ruled surface that realizes the bend has an edge of regression — the fold line where the rulings
  converge, `det J = 0`. At `h ≡ 0` it is the cone's apex ray, `µ̂ = 0`. Under a ramp it slides
  monotonically along the ruling, and the excursion scales with `max|h| / Δσ²`. Traced across the
  CCW ramp band:

  | recipe | `max|h|/Δσ²` | fold line sweeps | vs. kept material |
  |---|---|---|---|
  | `c = 0`, `Δσ = 0.25` (baseline) | 0.8 | `+0.39 → −0.72` | stays in the `|µ̂| ≲ 1.26` hole ✓ |
  | `c = 1/10`, `Δσ = 0.25` | 2.4 | `+1.18 → −2.17` | **inside** `(−2.32, −1.44)` for `σ ≥ 0.7214` ✗ |
  | `c = 0`, `Δσ = 0.0625` | 12.8 | `+10.2 → −∞` | **inside** `(−2.29, −1.44)` at `σ ≈ 0.7233` ✗ |

  Directly measured, not inferred: at `σ = 0.7233` and `0.7246` the steep recipe's fold line sits
  at `µ̂ = −1.49` and `−1.98`, inside the kept lower nappe. The sheet would have to crease across
  itself. The measured law is `|µ̂_fold|max ≈ 0.9 · max|h|/Δσ²` (baseline 0.72 vs 0.72 predicted;
  `c = 1/10` 2.17 vs 2.16) and the part is buildable iff that excursion clears the inner hole,
  `r_in / ρ_r`, with `ρ_r ≈ 1.3815` the ruling's radial speed on this cone.

  **The resolver detects it correctly and reports it wrongly.** `sample_comps` merges two
  µ̂-intervals separated by one subtract op into a face-with-hole unless the gap contains the
  `det J = 0` rail. On this part topology that test *is* "is the fold line clear of the material",
  checked at every sample — sound, and it fires before the fold reaches the sheet (the rail must
  leave the hole before it can enter the material). But the *fault* it raises is
  `AmbiguousRegion { op }`, which names an op-role conflict and points at the inner trim. The
  cause is the ramp. The op named is innocent.

  **The envelope, measured on the 42° device (`r ∈ [2, 3.069]`, `t = g = 1/20`).** Passing
  `max|h|/Δσ²`: 0.15, 0.25, 0.27, 0.60, 0.80, 1.20, 1.42, 1.44. Refusing: 2.40, 2.52, 3.20, 12.8,
  38. Frontier in `(1.44, 2.40)`, consistent with `0.9·x < 1.26`. **The ramp's slope, not its
  height, is the constraint** — any offset is reachable given enough azimuth. Confirmed by
  prediction: `c = 1/5` (five times the acceptance device's offset) refuses with a narrow CW ramp
  and verifies once that ramp alone is widened `Δσ` 0.25 → 0.5; `c = 1/10` with
  `ccw.ramp_start = 2/5` verifies through `solid` (22 faces, 0 free edges, ε 4.5e-2). A *failed*
  prediction was as informative: widening the inner radius 2 → 2.5 to let the fold line pass
  through the hole does **not** rescue `c = 1/10, Δσ = 0.25`, because the excursion runs to −2.17
  and would need `r_in > 3.0` — past the outer radius. At that ramp no annulus exists.

  Generalizable, and the correction is the lesson: **"the checker's reasoning is a proxy" does not
  imply "the refusal is spurious."** The merge test is stated in terms of a parametrization rail
  and reads like bookkeeping, so the first diagnosis was "sound but over-conservative, let it
  through". Tracing the rail against the material showed the opposite — relaxing it would have
  shipped a self-folding sheet. What is actually wrong is the *diagnostic*: a fault named for the
  bookkeeping instead of for the geometry sends the user to tune the wrong parameter. Worth
  checking the reverse direction too: a validation that measures the fold-line excursion up front
  would refuse by name, with numbers, before any certification runs.

- **The developed surface was the stack's bottom face, and that is a flat-pattern error, not a
  cosmetic one (2026-08-17).** The solid's thickness window was hard-coded `[0, t]`, so the chart —
  the surface `develop` unrolls **isometrically** — sat on a *face* of the material rather than
  through it. A bent laminate's flat pattern is only true on its **bending-neutral axis**; taken on a
  face it is wrong by roughly `(t/2)·κ`. `Part::neutral(f)` now places the window at
  `[−f·t, (1−f)·t]` and **defaults to `1/2`**, with `f` outside `[0, 1]` refused as
  `NeutralOutsideStack` (the developed surface would leave the material).

  **How it surfaced is worth recording: from a viewer, not from a test.** The user opened
  `cutter_dump.step` and observed that the cutter body terminated *on a surface* rather than inside
  the sheet. Measured on the L-slot device, `w = 0` is at `z = 2.2038` and `w = t` at `z = 2.2876`
  with `n·ẑ = +0.67` — the stack was entirely above the chart, so the body stopped at the underside
  instead of passing through. The same fact, seen two ways.

  **The trade it makes, stated rather than buried.** The footprint is computed exactly where the cut
  meets the developed surface, and the solid's walls are then ruled along `n` from there. Under
  `[0, t]` that made the cut exact on the `w = 0` face and off by a full `t` at the other; centred it
  is off by `t/2` on **both**. The worst-case envelope halves and "exact on one face" is lost. Two
  AUTH.3 faithfulness tests asserted the lost property and had to be re-based — but to something
  *stronger*, and derived rather than magic: a lid vertex sits off the authored vertical cylinder by
  exactly `(t/2)·|n_xy| = (t/2)·(72/97)`, predicted from the cone invariant `n·ẑ = 65/97` and
  measured at `4.639e-2` against a prediction of `4.639e-2`. The old window could not have passed
  that bound.

  Generalizable: **a hard-coded window is a modelling decision in disguise.** `[0, t]` reads as an
  implementation detail and is in fact the claim "the pattern is the bottom face's pattern" — which
  nothing in the codebase ever stated, and which the word "neutral surface" (used throughout for the
  chart) actively contradicted.

- **LAP: the lapped cone becomes a parameter set, and three of its checks turn out to be exact
  (2026-08-17).** The self-lapping device was a hand-written recipe; it is now one point in
  `acceptance::LappedCone` — apex direction, stack thickness, seam gap, which end laps on top, the
  seam offset, three azimuths per side, trim radii. `self_lapping_cone` is re-expressed through it,
  and the 277-test author/acceptance/develop/fixtures run — VV.1 work budgets, VV.2 ε bounds, VV.3
  chord goldens included — passes unchanged, which is what makes the re-expression a *refactor*
  rather than a new device wearing the old pins.

  **The datum, which is where the design nearly went wrong.** The seam offset is stated
  mid-surface to mid-surface, not to the chart. The solid's thickness window is `[0, t]`, so the
  chart surface `w = 0` is a *face* of the sheet; measuring the seam centreline against it puts a
  spurious `t/2` in the placement law and the "one ramp vanishes" condition stops closing. The
  correct law is `h_upper = c + t/2 + g/2`, `h_lower = c − t/2 − g/2`, whose vanishing condition is
  `c = ±(t/2 + g/2)` — which is what the user stated in the first place and what I got wrong when I
  first wrote it out. Generalizable: **when a kernel's natural datum is a face and the product's is a
  mid-plane, the parameter belongs in product language and the conversion belongs in one place.**
  (The same slip produced a "buried mid-thickness" claim in the IO.3 docs, corrected with this.)

  **Three checks that look like they need `arctan` and do not.** On the wrapping chart
  `φ = 4·arctan σ`, so two azimuths differ by exactly `2π` iff `1 + σ₁σ₀ = 0`. Therefore: the `2π`
  shift **is** the Möbius `σ ↦ −1/σ` — the same involution Stage 2 re-centred the seam with, arrived
  at from the opposite direction; a lap exists iff `1 + σ_ccw·σ_cw < 0`, a sign over ℚ; and the two
  overlap windows are exactly `[−1/σ_cw, σ_ccw]` and `[σ_cw, −1/σ_ccw]`. No transcendental, no
  tolerance, in any precondition.

  **What a rational direction buys, per parameter.** For the **apex** it is exact whenever the
  direction is Pythagorean, and the reason is structural: `wrap_cone(a, b)` has
  `sin β = (a² − b²)/(a² + b²)`, the Pythagorean *generator*, so `b/a = tan(45° − β/2)` and two
  half-angle steps rationalize any Pythagorean `(cos β, sin β)`. The 42° device is literally
  `(65, 72, 97)`. For the **azimuths** it buys nothing: `σ = tan(φ/4)` is a *quarter*-angle, needing
  the direction and its half-direction both Pythagorean, so those snap and echo, and a `σ` escape
  hatch is what makes an existing σ-authored device reproducible at δ = 0.

  **The generator's scale is gauge.** `wrap_cone(234, 104)` and `wrap_cone(9, 4)` store different
  `q` and have *identical* `normal`, `ruling` and `pedal` — the Hopf map is invariant under
  `q ↦ λq`. Measured rather than assumed, and it is what lets an apex direction be converted with no
  gcd bookkeeping.

  **The minimum gap is BONDED's to report, not the fixture's to compute.** Where a ramp descends
  inside the lap the gap is not the authored one, and the honest number is `bonded::clear`'s
  certified lower bound on the true 3-D distance — sound despite the tangential shift, which is the
  whole reason CLEAR exists. It needed one widening: a lap pairs the head's σ-window with the tail's,
  two disjoint intervals, where `clear` seeded both boxes the same. Its search was always pairwise
  over `I_A × I_B`, so `clear_boxes` is the general entry and `clear` the equal-box sugar. Two limits
  stated rather than implied: it certifies **rails** (fixed `µ, w`), so sampling the band edges is a
  check on the sheets and not a proof about the band; and it **proves `≥ keep_out` rather than
  measuring**, so reading the gap off it means bracketing. On the device: `1/100` certifies, the
  authored `1/20` does not — the ramp's intrusion, visible in the number.

- **IO.3b: two routes to the same curve, and the sharpest number is not the certified one
  (2026-08-17).** `author::dump::cutter_bodies` completes the dump: each hole op's certified `(σ, µ̂)`
  footprint lifted to the sheet, cast **back** down its own generatrices to the sketch plane, ruled
  between, triangulated.

  The reason to emit both caps is that the near cap and the sketch face (IO.3a) are the *same closed
  curve reached by two computations that share no code* — one from the authored profile edges through
  `Frame::point`, one from the traced footprint through the chart and back through `Cast::coords`.
  Measured on the L-slot device, every near-cap vertex lies **1.3e-9** from the authored outline. That
  is the `hole_poly` 2⁻³⁰ snap grid, and it is five orders **tighter** than the cut's own certified
  ε ≈ 7e-4 — because the tracer walks the *exact* wall equations, so casting back lands on the profile
  itself; ε bounds the cut against its ideal, which is a different quantity. Generalizable: when two
  independent routes to one object agree far better than either's certificate requires, the agreement
  is evidence about the *construction*, and it is worth asserting at its real size rather than at the
  certificate's.

  **Two scouting claims the build refuted, both recorded in IO.3a above.** (1) `hole_poly` was written
  off as unusable because it returns `None` for any loop not all-traced-`Curve` — true of the type,
  false of this input, since `certify_holes` produces all-`Curve` loops on both branches. It is not
  merely usable but *the right* converter: sharing it shows the polygon the part was actually cut
  with, and inherits the sub-`MIN_STEP` merge the body needs for the same reason the solid does — the
  tracer parks a vertex pair ~10⁻⁹ apart at each cell boundary, and a triangle on one is unbuildable
  by any `f64` consumer. A diagnostic that samples its own way answers a slightly different question
  than the build, which is worse than no diagnostic. (2) The AUTH.2 fixtures were priced at ~30 s of
  certification each; measured, the L-slot panel costs **1.8 s** and the self-lapping seam drill
  **6.8 s**, so the tests run the real devices rather than reductions of them. An estimate carried
  forward from scouting is a guess until something measures it.

  **The body closes, and that is a check on the tracer.** Caps share one exact-rational ear-clipped
  triangulation (a fan would lay triangles outside a non-convex footprint — the content of AUTH.2) and
  the walls share the caps' boundary edges, all by edge *identity*: `free = 0`, non-manifold `= 0`,
  `V − E + F = 2`, closed-shell certificate **Verified**, and OCCT's independent audit agrees
  (`closed`, `BRepCheck valid`). A footprint that self-crossed or dropped a vertex could not produce
  that at any ε. It is emphatically **not** a warrant for the geometry — the guard stays structural:
  raw `write_brep`, never `emit_certified_step`, and one open sketch face reopens the compound.

  *A defect in the already-shipped half, found only by writing the thing out.* Every sketch ring
  closed on a **duplicate of its first vertex** — the chaining walk ends where it started — so each
  polygonal profile carried a zero-length edge in its face wire. Four IO.3a tests passed over it
  (vertex counts were asserted as inequalities and `free_edges == verts`, both of which a duplicate
  satisfies). It became visible the first time the dump reached OCCT, which is the same shape as
  #267: the exact-versus-`f64` seam is where a picture stops being able to hide.

  *The lap, as predicted and sharper.* The self-lapping seam drill's two footprints land on **two
  different regions** (body and tail plateau) — which is why the region travels with the loop instead
  of being searched for afterwards. A metric cutter, having no sketch plane, gets its far cap and
  nothing else: an open patch that says so, rather than a silently omitted hole or a wall ruled to an
  invented plane.

- **IO.3a: what a picture can check that a certificate cannot (2026-08-17).** `author::dump` emits
  each extruded cutter's authored sketch as a planar face at its true 3-D position. The reason it
  exists is narrow and worth stating: a cutter's frame is a **search result** — the ray pick snaps a
  picked plane to rationals and certifies the *snap* (AUTH.1c) — and no certificate can say whether
  the **pick** landed where the author meant. That is a question about intent.

  The claim splits in two, and conflating them would make the artifact useless. (1) The face lies in
  its frame's plane as an **exact rational identity**, `N·(X − o) = 0`, which holds for *any*
  rational in-plane coordinate — so chording the arcs and snapping the `Surd` extrema costs the
  outline's shape a little and costs the plane nothing. (2) *Where* that plane sits is not invariant:
  shift the frame one unit along its normal and every vertex moves one unit while (1) keeps holding
  for both. **So (1) alone can never catch a mis-pick**, which is exactly why the picture is the
  instrument and the residual is not.

  Both are made non-vacuous from the other side — the same vertices measured against a *different*
  plane must give a nonzero residual — and the dump is asserted to be an **open shell the
  closed-shell certificate refuses**, so a diagnostic structurally cannot pass for a part.

  *Placed in `author`, not `interchange`.* The dump is a `Part → Brep` map, the same shape as
  `Part::solid_brep`; `interchange` stays about files. The milestone tag is not the module boundary.

  *Two facts found while scouting the unbuilt half (the cutter body), recorded so the next attempt
  starts from them — **and the first and the cost estimate below were both wrong; see IO.3b
  above**:* `export::trim::hole_poly` returns `None` unless **every** arc of the loop is a
  traced `Curve`, so a general body must sample `BoundaryArc` itself; and `structure.holes` carries
  `(op, **region**, window)`, which hands over the chart index directly — no σ-band search. The
  acceptance panel is the wrong fixture for it, because its own cutter is the *boundary* (an
  `intersect`) rather than a hole, so its `structure.holes` is empty; the AUTH.2 traced-slot devices
  are the fixtures, at ~30 s of certification apiece. Route table in
  `docs/interchange-design.md` §7b.

- **IO.2: three defects, and only one of them came from a test (2026-08-17).** The writers landed
  with a round-trip suite, and what it caught is more interesting than what it asserted.

  *From the round trip.* (a) An R12 `POLYLINE` carries a **dummy** `10/20/30` of its own — the
  format requires it, and the real vertices live one entity deeper in the `VERTEX` records. The
  reader was collecting every `10/20` in the span, so it picked up a phantom vertex at the origin
  and split every written loop in two. Nothing but the writer's own output exercises the R12
  spelling, so no hand-written fixture could have found it. (b) The reader's y-flip used
  `vb.y + vb.h`; reflecting a `viewBox` **onto itself** is `2·vb.y + vb.h`. The two are identical
  whenever `vb.y = 0`, which every hand-written fixture happened to have — the writer's own frame,
  centred on its geometry at `vb.y = −5`, was the first non-zero one and came back five millimetres
  off.

  *From reading the demo's output, not from a test.* The `viewBox` was printed at six places while
  coordinates went to nine, because a frame at twelve places is unreadable. But the reader
  reconstructs the flip axis **from the printed frame**, so the writer's exact flip constant and
  the reader's rounded one differed by the frame's rounding and shifted the whole drawing by a
  micron in `y`. Fixed by rounding the frame *first* and deriving the flip from the rounded values;
  pinned by a fixture with deliberately untidy bounds, since every earlier fixture had integer
  extents and printed exactly. **Generalizable: when a reader reconstructs a constant from data the
  writer also rounds, the writer must use the rounded value, not the exact one.**

  *A claim of my own the implementation refuted.* The IO.0 gate's table had inbound's "which datum
  moves" applying outbound as well. It does not: outbound, **both** formats derive centre and radius
  from one written scalar, and the asymmetry is *which* scalar — DXF writes `tan(Δθ/4)`, SVG writes
  `√r²`. That makes them exact on different arcs, which is a sharper and more useful fact: a quarter
  turn of radius 5 is free in SVG and costs DXF a rounding; a semicircle of radius `√2` is free in
  DXF and costs SVG one. Two-sided, so neither format dominates.

  Worth keeping: for an arc with rational centre and endpoints, `cos Δθ` and `sin Δθ` are **exact
  rationals** (`u·v/r²` and `u×v/r²`). The *turn* of an exact arc is exact — only its quarter-tangent
  is not — so `large-arc` and the bulge's major/minor branch are decided by an exact sign test
  rather than by a comparison against a tolerance.

- **IO.0: most of an import is exact, and "files are floats" is wrong twice over (2026-08-17).**
  The reflex design for a CAD reader is "snap to a tolerance". Two facts kill it. First, a decimal
  literal **is** a rational — `12.345` is `12345/1000` — so the only loss in reading a coordinate is
  the `f64` the parser transports it through, and Rust's shortest-round-trip `Display` gives the
  literal back exactly for anything under 17 significant digits. Every `LINE`, `CIRCLE`,
  `LWPOLYLINE` vertex, SVG `L`/`M`/`H`/`V`, unit conversion (`1mm = 480/127 px` exactly; DXF names
  its unit) and `matrix`/`translate`/`scale` transform imports at **δ = 0**.

  Second, where δ ≠ 0 does arise it is a *consistency* failure, not a rounding: a DXF `ARC` states
  centre, radius **and** two angles — four exact rationals describing an irrational point — so one
  datum must move, and *which* one differs per source form. The surprise is the ranking. A DXF
  `LWPOLYLINE` **bulge** is exact and free: with `d = P₁ − P₀`, `n = perp(d)` and `b = tan(Δθ/4)`
  from the file, the centre `c = mid + ((1−b²)/(4b))·n` is rational (the normalization of `n` cancels
  against the half-chord, since `cot(Δθ/2) = (1−b²)/(2b)`), and `r² := |P₀−c|²` seats *both* vertices
  on the circle — `P₁` because `c` is on their perpendicular bisector by construction. SVG `A` is
  the same trick read backwards: hold the two rational endpoints, choose the centre on their rational
  bisector, and δ becomes a **radius** deviation rather than an endpoint one. Only DXF `ARC` is
  genuinely lossy, and its δ is certified by the shipped `develop::interval` `arctan`/`pi`/`sin_on`
  enclosures over a rational tangent-half-angle rotation `M(t) = [[1−t², −2t], [2t, 1−t²]]/(1+t²)`,
  which is exactly on the circle for every rational `t` because `(1−t²)² + (2t)² = (1+t²)²`.

  *Actionable, and the reason this is a Finding rather than a design note:* **export your outline as
  `LWPOLYLINE` with bulges and the import is exact; export the same outline as `ARC` entities and it
  costs a certified δ.** That is a sentence for the user-facing docs.

  *Refinement stops at a floor, and the floor is the enclosure.* Measured on the DXF-`ARC` path:
  `δ` = 1.3e-1 / 7.3e-3 / 3.7e-5 / 2.0e-10 at 4/8/16/32 bisections, then flat at ~1.6e-16·r. Past
  ~54 steps the search is finer than the `cos`/`sin`/`π` enclosures it is *measured against*, so
  what is left is their accumulated `ROUND_BITS` rounding — and more series terms very slightly
  **widen** it, because each extra term is another rounding. Pinned as a floor with a control at 96
  iterations, not behind a loose "smaller than last time".

  *Two implementation traps, both worth carrying forward.* (a) **Uncapped Newton for `√k` is not
  slow, it is unusable** — it roughly squares the denominator per step, so 40 steps build a
  ~2⁴⁰-bit number and the test simply hangs. (`author::part::rational_sqrt_above` takes three steps
  for exactly this reason.) Bounding it with outward rounding onto a `2⁻ᵇⁱᵗˢ` grid costs the exact
  answer when there is one, which is repaired by asking for the **simplest rational in the final
  bracket** (Stern–Brocot) and testing whether it squares to `k`. (b) An angle that is a whole
  multiple of 90° had *already* been solved by the quarter-turn reduction, and the code was handing
  the zero residual to the enclosure machinery and reporting the enclosure floor as if it were
  error. Fixing it made an SVG rounded rectangle — the flex outline shape — import at `δ = 0`, and
  broke three tests that had been using 0°/90°/270° arcs to exercise the *snapped* path. Same
  pattern as the p-curve milestone: **defects caused by the geometry getting better.**

  *`δ = 0` is a statement about the translator, not about the file.* An SVG `<rect rx>` states a
  **shape** (its corner endpoints are the axis-aligned tangent points, so the arcs are exactly
  tangent); a DXF bulge states a **curve**, and `tan(Δθ/4)` for a quarter turn is `√2 − 1`, which no
  file can write. So a real rounded rectangle exported as bulges imports *exactly* — as the curve
  the file actually contains, whose `r²` sits ~10⁻¹¹ off the quarter-circle its author meant. Saying
  "exact" without that distinction would be the most misleading true sentence in the milestone.

  Same construction, one level up: a rational-quadratic circular arc needs weight `cos(Δθ/2)`, which
  is rational exactly when `1 + tan²(Δθ/2)` is a rational square — i.e. at **Pythagorean** angles
  (`t = 3/4 → w = 4/5`). Those are dense, so exact conic edges in the B-rep are reachable by
  subdividing at Pythagorean rotations rather than chording. Recorded as the escalation path for
  IO.3, not taken there.

- **#275: the cross-op σ-end, and why it took a *placement* rather than a mechanism (2026-08-17).**
  §12.2 derives the σ-ends from the union of **every op's** walls, because the two walls closing the
  kept µ̂-interval need not belong to the same cutter. Every fixture closed on the contour's own
  tangents, so that half of the union had nothing exercising it — the gap that held AUTH.3's
  `rc-hyp` at 🚧 and, with it, the milestone out of `LANDED`.

  The construction is arithmetic, not code: a contour whose own tangent points fall *inside* the
  annulus carve has no material at its own tangent rulings, so the extent **cannot** close there.
  Tangent radius is `√(D²−r²)`; the carve's boundary at that azimuth is `[cos δ + √(cos²δ+7)]/2`.
  Centre `(0, 8/5)`, `r = 3/5` gives `1.483` against `1.865` — inside. It certifies, and the
  signatures are unambiguous: roles `[Inactive, LowerBound, UpperBound]` (the **subtract** bounds one
  side, the **intersect** the other) and the folded boundary at `51`/`51` on the two authored
  cylinders with **4 on both at once** — those four *are* the σ-ends. The control, tangent outside
  the carve, gives `192`/`0` and **0** corners.

  **The near-miss placements are the finding.** Nudge the contour out until its tangent is only just
  inside the carve — `(0, 19/10)`, `r = 1/2`, `1.833` against `1.891` — and the end derives correctly
  but no graph rail certifies up to it: `RailSpanShort`. The crossing has to clear the contour's own
  √-branch. Logged rather than pinned as a test, because it is a limit worth *lifting* (the turn-arc
  splice only fires on a `pinch: true` end, and a cross-op end is `pinch: false`) rather than a scope
  exclusion — pinning it would ossify the refusal. *Three placements refused before one worked, and
  the difference was a number I could compute in advance; computing it first is what turned a search
  into a construction.*
  **And landing the milestone broke the gate's own mutation test**, in the instructive way: its
  "a dotted tag that has *not* landed is out of scope" case was written as `[AUTH.3]`, so the day
  AUTH.3 landed the test's premise became false. The case is about *dotted ∧ unlanded*, not about
  which milestone, so it now uses a tag that is deliberately not real. **A test that names a live
  instance to demonstrate a property will fail for the wrong reason exactly when the property is
  most interesting.**
  *2026-08-17 · resolved · #275, branch `auth-3c`*

- **AUTH.3d: the acceptance closed, and the milestone's own gate said it is not finished
  (2026-08-17).** `acceptance::contour_panel` is the first device in the repo whose boundary is an
  **authored outline** rather than a declared band, and the round-trip closes on it: 3-D contour →
  certified flat pattern, a feature authored in the **developed** panel's ECAD coordinates → folded
  back → drilled, watertight genus-1 solid → STEP (`cert=Verified occt=ok`, 210 faces, 0 free,
  19134 entities under OCCT). Two things about *how* it is stated are the reusable part:

  - **"The outline bounds it alone" is a derived fact, not a pruned recipe.** The panel's own `z ≤ 3`
    bound and annulus carve stay in the ops list and both come back `Inactive`. Deleting them would
    have made the same test pass while proving nothing about the resolver.
  - **The artifact was checked, not just the verdict.** The drilled SVG carries **2** subpaths where
    the plain one carries 1 — the authored feature is actually in the emitted geometry. Cheap, and
    it is the check [[verify-demo-faithfulness]] exists for.

  **And the gate refused the victory lap.** Marking AUTH.3 `LANDED` in xtask was the obvious next
  move; running it first showed the vv-matrix gate **FAIL** — the ★ row carries `rc-hyp 🚧`, and it
  is 🚧 because the cross-op σ-end has no fixture (#275). So the milestone is *feature*-complete and
  not *evidence*-complete, and the gate is what distinguishes those. **Try the declaration before
  writing it down**; a gate you have not run is a gate you are asserting.
  *2026-08-17 · resolved · AUTH.3d, branch `auth-3c`*

- **A radiused outline is not "the quadric case with more walls" — it is the case that showed the
  contour path was asking the wrong question (AUTH.3d.1).** The `sole_pinched_contour` fork asked
  for **one wall**; a rounded rectangle has eight. Generalizing it looked like relaxing a bound, and
  the measurement said otherwise: a corner arc is a *short* quadric wall whose **entire**
  disc-positive window lies within `~10⁻⁴` of a tangent ruling, so `certified_rail_surface` clamps
  the fit into the √-branch and the oracle declines from the first sample — `Unresolved`, not the
  `RailSpanShort` the earlier shapes gave. There is no span the clamp can choose, because the whole
  window *is* the branch.

  The fix was to ask which **cutter** bounds the part rather than which **wall**, and then read the
  boundary from the cutter's own fill rule — `shadow_hole_loops`, the multi-wall tracer AUTH.2 built
  for non-convex *holes*, called on an outline. The one-wall case joins it instead of being
  special-cased around. Deliberately **not** taken by an all-affine contour: its rails are exact and
  its corners certify at `ε = 0`, so tracing would swap exact rails for chords. *A traced loop is
  earned by a quadric wall, not by having many walls.*

  Two costs, both measured, both worth stating because neither is obvious:
  - **The tracer spends `segments` over the whole loop, so a small radius starves its own arcs.** At
    `r = w/5` nothing certifies below 384 segments; at `r = 2w/5` it certifies at 48 and converges
    `5.4e-2 → 4.0e-2 → 1.7e-2` over `48 → 96 → 192`, with `206 / 350 / 734`-face watertight solids.
    Too *large* fails the other way — at `r = 3w/5` the corners eat the sides and the footprint stops
    being one µ̂-interval per ruling (`AmbiguousRegion`).
  - **The outer wire may not inherit the hole budget, and it silently had been.** `outline_solid`
    took `certify_holes`' `clamp(8, 16)` — right for a hole, where a coarse loop is a fidelity trade,
    and wrong for the boundary, where it is a *refusal*: the radiused outline certifies flat at 48
    segments and was `Unresolved` in the solid at 16. A defect introduced by me in AUTH.3c part 2,
    found only because a harder fixture asked. **Reusing a budget is reusing a judgement about what
    the thing is for.**
  *2026-08-17 · resolved · AUTH.3d.1, branch `auth-3c`*

- **The outer wire cost one boolean operator, and the mixed boundary one currency (AUTH.3c).** §12.4 had framed the remaining pinch
  shapes as needing "the polygon channel extended one level out", with a named fallback if that
  turned out expensive. It did not. A slice's footprint was `strip ∖ holes`; for a part bounded by a
  traced loop it is `strip ∩ ({outline} ∖ holes)` — and even-odd parity already reads a loop
  strictly inside another as a hole in it, so the outline and its holes are *one* operand and the
  only change is `BoolOp::Diff → BoolOp::And`.

  What made it cheap is that the band did **not** have to be replaced. It is demoted to the two jobs
  that still need a rail — the σ-station partition and the ruled patch each footprint is trimmed out
  of — and both only require it to *contain* the wire. So where there is no boundary rail to derive
  one from, the evaluator synthesizes one. The pad is relative (a sixteenth of the wire's own µ̂-span
  each side) rather than fixed, so a small contour's band cannot wander onto the chart's singular
  rail, where the parametrization breaks down rather than the part.

  Measured: the quadric contour that bounds its part alone builds a watertight genus-0 solid, 66
  faces, with every vertex within a thickness of the authored cylinder and the `w = 0` lid on it.
  The two worries going in were both wrong. The terminal slices meet the strip's σ-edge
  **tangentially** — the wire reaches each σ-end at a single point, the case an interior hole is
  explicitly forbidden from creating — and the arrangement handled it without a pinch. And "the
  pinch makes the solid degenerate" was a phrase, not a fact: the footprint is a closed oval, and an
  oval swept through a thickness is an ordinary prism. Only the *representation* ever failed.

  One rule runs opposite to a hole's and is load-bearing: a hole must be strictly interior in σ, an
  outer wire must reach **both** σ-ends. A wire falling short would leave the terminal slices bounded
  by the synthesized band — a longer part than asked for, every certificate green. Pinned as its own
  refusal test, because the two share a code path and the shared path is where that inverts.

  **The mixed shape then cost a currency, not a construction.** Where the boundary is a rail out and
  an arc back, chording the rail would have traded a certified fit for a chord sagitta nobody
  bounded. So a wire vertex became either an explicit `(σ,µ̂)` or *a σ at which the wire runs along a
  named rail* (`WirePoint`) — the mechanism being the one `railed_corners` already had, a footprint
  vertex on a proxy horizontal reading back as the true curved rail. Two adjustments fell out. The
  strip's rect now opens a unit beyond the proxies when a wire is present, so a wire running along a
  rail is an interior edge rather than one coincident with the strip's own. And `railed_corners`'
  refusal of a non-radial rail-to-free edge became a chord fitted through the rail's **true** value:
  a proxy horizontal is a height with no metric meaning. That arm read as a guard against a hole
  touching the boundary, but the vertex-level `inside_band` test is the actual guard — the arm was
  doing it only incidentally, which is worth noticing before deleting anything that looks defensive.

  143 faces, genus 0, watertight. **And the faithfulness assertion earned its keep twice.** It first
  failed at `0 vertices on the plane` — which turned out not to be a missing rail but a tolerance
  borrowed from the flat tests (`5e-3`) sitting *below* the part's own certified `ε = 8.9e-3`, since
  the solid is emitted at the deliberately coarser STEP fit. Restated against `solid.eps()` it
  passes, and it is now the honest test rather than the lucky one: **ask the part what it promised,
  do not borrow a constant from a different profile.** The measured split — 6 vertices on the plane
  against 149 on the cylinder — is itself the claim that the rail was named rather than chorded: a
  Bézier costs corners only at its σ-stations, a chord costs one each.
  *2026-08-17 · resolved · AUTH.3c, branch `auth-3c`*

- **A part refused because its denominator had the wrong sign — and the sign meant nothing
  (AUTH.3c).** The σ-stock's solid path was expected to be a one-line fix: `brep_trim_solid_regions`
  sweeps the *authored* region bands, so clip them to the derived `structure.domain` first. That
  clip is correct and necessary, and it did **not** make the polygonal contour build. It moved the
  refusal from `piece_at` to `sigma_splits`, which reported that the anchor `c + µ̂·r + w·n` had a
  denominator **negative at every one of nine samples across the extent** — uniformly negative, not
  crossing.

  `(N, D)` and `(−N, −D)` are the same rational curve. Which one arrives is a convention: an
  extruded profile's wall facing the other way flips the sign of its µ̂-pullback's denominator, and
  `reduce()` — a polynomial gcd division — may flip it again on the way to the anchor. So
  `positive_weights`, which demanded strictly positive Bernstein coefficients, was refusing parts
  every emitted patch is perfectly well-conditioned over. What is genuinely unbuildable is a
  **crossing**: a weight through zero is a pole inside the span, and that is what subdividing is
  for. The gate is now sign-*definiteness*, `sigma_splits` carries the run's sign down its
  recursion, and `RatBezier::from_vec3rat` / `RatBezierSurface::ruled_from_rails` pick the positive
  representative (`bezier::positive_representative`) where the Bernstein form is actually made —
  which is the only place that can, since `reduce()` sits between the rail and the patch.

  Result: the σ-terminating square contour builds a **watertight genus-0 solid, 14 faces**, against
  the lateral-trim control's unmoved **10**. The normalization is exact, so no emitted point and no
  certified bound moves — verified the strong way, `693/693` workspace tests green including every
  golden and acceptance fixture, because a change touching every rational patch in the crate is not
  something a targeted test can clear.

  **Two things worth carrying forward.** First, the diagnosis only existed because the probe was a
  *comparison* — four fixtures with a working control — so "the control builds and this does not"
  was a fact from the first run rather than a hypothesis. Second, the false lead cost two attempts:
  I twice sign-normalized the **rail** (`poly_rail`) and twice the probe was unmoved, because
  `reduce()` re-introduces the sign downstream of it. *Normalize where the representation is
  consumed, not where it is produced* — anything in between may re-derive it.
  *2026-08-17 · resolved · AUTH.3c, branch `auth-3c`*

- **The vv-matrix gate has been vacuous for every `[AUTH.x]` row, and `LANDED`'s `"AUTH.1"` /
  `"AUTH.2"` entries had never been read.** `milestone_tag` required a tag to start with `M` and hold
  alphanumerics only — a tightening that reads as harmless and is not: `AUTH.2` fails *both* halves,
  so the parser returned `None` and the row was skipped before the landed check ever ran. Six ★ rows
  were gated by nothing. Found while adding the AUTH.3 row, by asking whether the row I was writing
  would actually be enforced: strip `rc-hyp ✅` from the AUTH.2 ★ row and the gate still reported
  **OK**. Fixed to accept an uppercase-initial tag with alphanumerics and dots, which is what the
  repo's own tag vocabulary has been since AUTH.1; the mutation now fails and both cases are unit
  tests. Worth recording as a pattern rather than a typo: **this is the second time this gate has
  passed vacuously** (before slice 3d it split on whitespace and matched a mid-cell word instead of
  the ★ Item), and both times the shape was the same — the gate ran, printed OK, and was checking a
  set that had silently become empty. A gate whose *scope* is computed needs a test that the scope is
  non-empty, not only a test that the check fires on a hand-built row.
  *2026-08-16 · resolved · AUTH.3.0, branch `auth-3`*

- **A 33× cost regression that every faithfulness assertion passed (#281).** The per-stage triage
  that #280's sampling unblocked found one fixture wildly out of line — the whole-side contour
  emitting **6386** outline points where the *same contour traced as a sole boundary* emits **192**,
  at `175s` to develop against `3s`. Cause, in #278's own `push_turn`: each **already-traced**
  p-curve piece was wrapped as `BoundaryArc::Curve { segments: part.segments }`. But `segments` on a
  `Curve` means *how finely to re-sample this one piece*, while the tracer's `segments` sets *how
  many pieces the loop has* — so the two multiply. `133 pieces × 48 = 6384`, against the 6386 seen.
  `export::trim` has mapped a traced piece to `segments: 1` since PC.4; the fix is to agree with it.

  Measured: the whole-side test **`258.770s → 5.545s` (47×)**, and the four-crate suite
  **`260.4s → 124.9s`** overall, 300/300 green with every certified ε unchanged — one chord per
  traced piece is the resolution the tracer already chose, not a coarsening.

  **The reusable part is why the tests missed it.** #278's assertions check the boundary is
  *correct* — every folded vertex on the authored cylinder or plane to `< 5e-3`, folded x-span
  `> 1.9r` proving the arc wraps rather than stopping at a tangent — and a 6386-point outline
  satisfies all of them, more comfortably than a 133-point one does. **Faithfulness assertions are
  monotone in refinement: they cannot fail from emitting too much, so a cost regression is exactly
  the defect class they are blind to.** Now pinned as its own budget (`n_out < 8 × segments`), on
  the shape rather than a golden number: a boundary made of a rail plus an arc over the traced loop
  is `O(segments)` and never `O(segments²)`. Same spirit as VV.3's golden flat-pattern metric.

  Also worth noting against [[verify-demo-faithfulness]]: this is that lesson's mirror image. There
  the risk was green certificates over unfaithful geometry; here it was faithful geometry that no
  assertion could price. Both come of checking only the property the milestone is *about*.
  *2026-08-17 · resolved · #281, branch `auth-3`*

- **A 30-minute "slow" run was a hang, and ten seconds of `sample` said so (#280).** Probing
  AUTH.3c with `solid_brep`'s guard bypassed, the first fixture had not returned after 30 minutes,
  and there was no way to tell 3 more minutes from 3000. macOS `sample <pid> 10` put **100%** of
  stacks in `export::brep_build::sigma_splits::go`, recursing dozens of levels — an unbounded
  subdivision, not arithmetic cost. **Two defects, both in code shipped since G9.2:**

  - **The hang.** `sigma_splits` bisects until every sub-interval's rational Bézier has all-positive
    weights, and its own doc carries the termination argument: *"a strictly-positive polynomial's
    Bernstein coefficients converge to its (positive) values under subdivision"*. That argument
    needs `den > 0` on `[a,b]`. Where `den` is non-positive over a *region*, nothing converges and
    the whole region expands to `MAX_DEPTH` — and the cap bounds **depth, not node count**, so the
    work is `2³²` sub-intervals. The precondition was stated correctly and never checked.
  - **The fail-open underneath it.** On depth exhaustion `go` did `out.push(b); return` — emitting
    the very piece the function exists to exclude. A caller got an invalid Bézier weight (a control
    point at or through infinity) with every certificate still green.

  Fix: the end weights of a piece **are** `den` at its ends, so one evaluation per split point
  decides it — `den(x) ≤ 0` refuses immediately instead of bisecting toward it — plus a `MAX_NODES`
  backstop, and `Option` returns so exhaustion refuses rather than emits. The pathological case went
  from **30+ min to 16 ms**, and all 73 export/acceptance tests stayed green, so it is not
  over-refusing.

  Three things worth keeping. **A documented precondition with no check is a hang waiting for a
  caller** — this one survived because `solid_brep`'s AUTH.3c guard happened to keep every shipped
  fixture inside the valid range, so the bug was reachable only by the slice that had to remove the
  guard. **A depth cap is not a work cap**; `2³²` leaves is indistinguishable from a hang, and the
  cap read as a safety net while providing none. And **a slow number and a hung number are identical
  without observability** — I had already generalized this 30 minutes into a cost model for #279
  before sampling, which was wrong; the runtime concern stands on other numbers.
  *2026-08-17 · resolved · #280, branch `auth-3`*

- **Two guesses that read as facts, and both failed as `None` rather than as bad geometry.** The
  last boundary shape AUTH.3b owed (#278) is a contour bounding one **whole side**: its chain there
  is a single segment, so the per-end splice — "remove that end's outermost segment from both chains"
  — has nothing to remove twice, and the framing had to go. The general statement is that **an arc is
  the run of the contour's loop between the two σ where a non-contour rail takes over**, wrapping
  zero, one or two tangents. Making `tangent_turn_arc` general surfaced two assumptions that had been
  correct only for the one-turn case:

  - *σ does not say which branch a junction is on.* The first version picked the starting piece by σ,
    but a loop that turns visits each σ twice; the arc came back as a graph. Fixed by taking
    `from_upper`/`to_upper` from the caller — the chains already know which side they are — rather
    than inferring them.
  - *the walk covered the cycle once.* A two-turn arc leaves the upper branch, turns, crosses the
    lower branch, turns again and rejoins the upper — so its end lies **after** a full lap and was
    unreachable. Fixed by walking `2·n` pieces and stopping on turn count, not index count.

  Neither produced a wrong boundary; both produced `None`, which routes to `CutUnresolved` — safe,
  and uninformative. **A total function over a partial traversal fails silently by construction: the
  case it cannot reach is indistinguishable from the case that does not exist.** Worth pairing with
  the `params_at_sigma` entry below, which is the same milestone's opposite failure — an *inexact*
  value that looked right (closing "to 1e-12") where this is a *correct* value that never arrived.
  *2026-08-16 · AUTH.3b‴, branch `auth-3`*

- **A searched parameter is not a join: `params_at_sigma` bisects, and an exactly-checked chain says
  so.** Splicing a p-curve turn arc into a graph chain, the arc has to start exactly where the rail
  it follows ends. `PCurve::params_at_sigma` looked like the tool for it — "the parameters where the
  curve crosses σ = s" — and it is, for locating; but it is `scan_roots`, a bisection, so the
  parameter is *near* the crossing and the trimmed arc starts *near* the junction. `unroll_trim_loop`
  compares consecutive arcs over ℚ (`sm_eq`) and rejected it, correctly, as `ArcDiscontinuity`. The
  fix is that the object is a **chord**: `σ(t) = a + (b − a)·t`, so the junction parameter is one
  division in ℚ and the endpoint is exactly the junction. A piece whose σ is not affine is now
  refused rather than approximated, because an inexact join is a boundary that does not close.

  Two things worth carrying. **Where a structure is checked exactly, every value entering it must be
  constructed exactly, not found.** And the diagnosis was slow for an avoidable reason: the chain
  printed as closing "to 1e-12" in floats and the fault name (`LoopBroken`) had already discarded the
  index the unroll reported. Ten seconds of surfacing `ArcDiscontinuity { index }` beat twenty
  minutes of reasoning about which junction *ought* to be wrong — *when a refusal is re-typed on the
  way out, the diagnostic is what gets dropped.*
  *2026-08-16 · AUTH.3b″, branch `auth-3`*

- **Separating the two pinch classes turned the "hard" one into no new construction at all.** The
  quadric end was filed as the expensive half of AUTH.3b — a √-branch no graph fit can reach, needing
  §12.4's p-curve. Once the classes were told apart (entry below), the question sharpened from *"how
  do we fit a rail through a tangent ruling"* to *"what is the boundary there"*, and the answer was
  already built: when the contour bounds the part **alone**, the outer boundary **is** that wall's
  traced footprint loop, which `surface_hole_loop` has produced since PC.3 and `unroll_trim_loop` has
  accepted since PC.4. Thirty lines of detection and dispatch, no new geometry. Measured on a radius-
  `1/5` disc: 192 outline points, `ε = 2.26e-3`, every vertex folded back on the authored cylinder.
  **The pattern: a construction built for one role (an interior hole) is often the whole answer in
  another (an outer boundary), and what hides that is describing the problem by its difficulty
  ("fit a rail through a tangent") instead of by its object ("the boundary is this loop").** The
  residual case is genuinely different work and is now scoped as such: a quadric contour *sharing*
  the boundary with other ops needs the p-curve arc **spliced** into a graph chain at the junctions.
  *2026-08-16 · AUTH.3b′, branch `auth-3`*

- **The two "pinch" ends are not the same shape, and the affine one is the exact one.** §12.4 filed
  a single termination class — "the two bounding rails meet, so a graph fit runs into `∂s/∂µ̂ → 0`;
  a p-curve, then" — covering both a quadric's tangent ruling and a polygon's corner. Measured at
  AUTH.3b they behave oppositely. A **quadric** contour ends at a √-branch and its rail is a *fit*
  whose certified span stops short of the end, so it refuses. A **polygon** contour ends at a corner
  where two walls cross transversally, every wall is affine, `plane_cut_rail` is **exact** — no fit,
  no window, no clamp — and the whole boundary certifies at **ε = 0** right through the corner. So
  "keep what is inside this contour" shipped for polygonal contours a slice before quadric ones,
  which is the reverse of the usual order: the metric cutters are normally the easy case, and on
  this axis the affine wall is the exact one. The general lesson is about how a design note groups
  cases: *"the rails meet"* is a statement about the picture, and the thing that decides the work is
  whether the rail is a **fit** or a **formula**. Two ends that look alike in the domain can sit on
  opposite sides of the only distinction that matters.
  *2026-08-16 · AUTH.3b, branch `auth-3`*

- **A rail was being evaluated outside the span it was certified over, and only a derived σ-end could
  reach it.** `certified_rail_surface` clamps its fit span to the wall's disc-positive window, inset
  a hair, precisely because the near-tangent region blows the bound. Nothing checked that the *chain
  segments* stayed inside the clamped span — and nothing needed to, while the outer boundary always
  ran between authored band edges and only interior holes ever approached a tangent (which is why
  p-curves were built for holes at PC.3 and for nothing else). Give the boundary a derived end that
  lands on a tangent and the fitted graph is read past its certificate, into a √-branch of unbounded
  slope, with the reported ε describing a stretch of rail the geometry does not use. `RailPiece` now
  carries its certified span and `PartFault::RailSpanShort` refuses the mismatch. **The shape worth
  recognising: a bound that was safe because of a property of the *inputs* rather than of the
  code, in a codebase where the inputs just got more general.**
  *2026-08-16 · AUTH.3b, branch `auth-3`*

- **A `Pole` that was not a pole: two functions computing the same quantity from different
  sources.** With the derived extent wired into `certify_boundary`, the polygonal contour refused
  `Pole` — and the boundary certified perfectly (ε = 0, four wall rails, segments covering the
  domain, corners located). `flat_pattern` recomputes `domain` from the region **bands** rather than
  taking the one the structure derived, so it evaluated the closing caps at σ = ±1 while every rail
  piece was fitted over the contour's own ±0.064 footprint — asking a rail for its value a
  quarter-turn from where it exists. Two derivations of "the domain", one updated. Exactly the
  #267 shape (two independently derived structures reconciled nowhere), and worth logging separately
  because the *symptom* pointed at the arithmetic — a fault named `Pole`, on a fixture that really
  does have two rails with genuine poles at σ = 0, neither of which is used where it poles. The
  plausible cause was there to be found and was not the cause.
  *2026-08-16 · AUTH.3b, branch `auth-3`*

- **Which of two sub-cell events ends the material — the GO-gate got it backwards, and the mechanism
  it specified is what caught that.** AUTH.3.0 claimed the quadric fixture's material closes at
  `Meet(1, 2)` (σ = ±0.238427501), *inside* the contour's own `Tangent(2)` (σ = ±0.240408206), and
  billed that as proof the naive "a contour's σ-extent is its own tangent rulings" was unsafe. **The
  two are the other way round.** Measured across the stretch once AUTH.3a's gap evaluation existed:

  | σ | kept µ̂-interval | bounded by |
  |---|---|---|
  | −0.238400 | `[1.95723, 2.21527]` | carve below, contour above |
  | −0.238500 | `[1.95953, 2.21192]` | **both the contour's own walls** |
  | −0.240000 | `[2.02568, 2.14259]` | contour, narrowing |
  | −0.240400 | `[2.07542, 2.09200]` | contour, nearly shut |
  | −0.240500 | — | gone |

  `Meet(1, 2)` is a **handover** — where the lower bound stops being the annulus carve and becomes
  the contour's own lower root — and the material lives another `2·10⁻³` past it before pinching at
  the contour's tangent. The gate's reasoning had inferred the mechanism from a sampled scan: it read
  the labels at the last live sample (`[carve, contour]`) and assumed they persisted to the end. They
  do not. **The lesson is about evidence rather than geometry: a design note that reasons from a
  sampled scan about which of two sub-cell events matters is a hypothesis, and the only evidence is
  the evaluation the design itself specifies.** The derivation disagreed with its author the first
  time it ran, which is the cheapest place this could have surfaced.

  Two things survive. The union over every op's walls is still required — the two rails that close
  the interval genuinely need not belong to one cutter, since a contour's band can slide entirely
  inside a subtract's carve — but that case now has **no fixture**, and it is filed rather than
  claimed. And the replacement assertion is stronger than the one it replaces: the handover stretch
  is asserted directly (`both bounds are the contour's own walls, and the interval narrows`), because
  that is precisely what a nearest-event derivation denies. The near-miss recorded at the gate —
  a locator that returned the *first* event in the cell and so reported an asymmetry a
  mirror-symmetric fixture cannot have — has the same root, and was the warning shot.
  *2026-08-16 · corrected at AUTH.3a, branch `auth-3`*

- **The third σ-end class exists, is implemented, and is unreachable — and the reason lives in
  another tier.** §12.2 names three ways material can end: two rails converging (`Meet`/`Tangent`)
  and a wall degenerate in µ̂ flipping its coverage at a root of `c`
  (`develop::cut::coverage_events`, added here). The third needs `n ⊥ ruling(σ)` for every σ, so
  every ruling must point one way — a cylinder chart. The shipped cylinder has `h ≡ 0`, which puts
  its whole `w = 0` surface in one plane and leaves `c` constant (measured `c ≡ −1`: no root, no
  flip). Give it a moving support and `c` does vary (measured `2.2, 0, −1, −2, −4.2`, root at
  `σ = −1`) — but that chart comes back `NotDevelopable`, refused a step earlier. So the family is
  folded in, correct, cheap, and presently dead code. Worth recording because the instinct is to call
  that over-engineering: it is unreachable *from the resolver's side*, and what would make it
  reachable is a development-tier capability, so the branch starts firing the day a supported
  cylinder develops rather than the day someone edits `resolve.rs`.
  *2026-08-16 · AUTH.3a, branch `auth-3`*

- **A measurement can stop being valid when the fixture moves, and stay green: `max_ray_crossings` on
  a chart whose support curves.** AUTH.2's headline property — a ruling meets the cutter twice — is
  read off the flat pattern as *four crossings by a ray from the flat apex*, and that reading is
  exact only because every AUTH.2 fixture lives on a chart with `γ ≡ 0`, where the ruling images
  really are a pencil through the origin. Carrying the same L onto the self-lapping device (#269)
  breaks the premise silently: the smoothstep ramp's images are each offset by the running directrix
  `γ(σ)`, measured `|γ| = 0.159` at `σ = 7/8` against exactly `0` on the body. The origin-ray count
  happens to still return 4 there — the offset is ~2.3° of angle at flat radius 4.1 — so a test
  asserting it would have passed for a reason that no longer holds, which is the worst failure mode a
  V&V instrument has. Fixed by measuring against the family the development actually produces:
  `PiecewiseDevelopment::point` → `Part::flat_rulings` (the glued development at `µ̂ = 0` and `µ̂ = 1`,
  two points fixing the image line) → `acceptance::measure::max_ruling_crossings`, with the
  non-concurrency itself pinned so the instrument is known to be needed. **The general shape: a
  measurement's validity rests on a property of the fixture, and moving the fixture is exactly when
  nobody re-derives it.** *2026-08-16 · resolved · #269, branch `stress-fixture`*

- **A classifier that reads "which cutter made this hole" off the hole's *size* is a differential
  waiting to reclassify itself.** The self-lapping stress fixture has two cutters over the lap and
  four derived holes, and the tests need to know which is which. Area separates them cleanly — the
  seam drill's enclose `0.116` and the traced slot's `0.070` — until the drafted variant, where the
  slot's holes grow to `0.110` and the threshold quietly swaps them. That is not a tuning problem:
  the taper test's whole subject is that a drafted cut is *bigger*, so the classifier was keyed on
  the exact quantity under test. The fix is to ask the cutter — fold one vertex back and check
  whether it lands on `acceptance::seam_drill_axis`'s cylinder, which is the same object the recipe
  cut with. Caught by the test failing rather than by review, which is the argument for having built
  the drafted variant at all. *2026-08-16 · resolved · #269*

- **A quality golden can score a *legitimate* fixture inside its own defect band, and widening the
  gate is the wrong answer.** VV.3's chord metric (longest emitted edge as a fraction of the ring's
  size) was built to catch a graph-model bridge across the tangent rulings, historically 30–48%. The
  traced lap slot scores **28.6%** at the device's `segments(16)` — the L's own straight sides are
  legitimately a large fraction of its box, and the tracer's vertices come from the σ-event partition
  rather than from a uniform chording, so they cluster (near-duplicate pairs `~5·10⁻⁹` apart) and
  leave long edges between. Raising the gate to 35% would have made the metric decoration on this
  device. The discriminating property is that **a bridge is structural and a chord is not**: measured
  28.6% → 18.0% → 9.0% at `segments` 16 → 32 → 64, with the metric drill hole (9.4% → 4.7% → 2.4%,
  exactly `1/n`) as the control that the comparison is measuring the loop and not the knob. Where a
  fixture legitimately scores badly, assert the property that separates it from the failure.
  *2026-08-16 · resolved · #269*

- **Every certificate `Verified` and the STEP write refused the shell: the exact tier's minimum
  feature is smaller than the exporter's.** The AUTH.2f acceptance demo's traced-slot solid audited
  clean — watertight, manifold, genus as expected — and OCCT's `BRepBuilderAPI_MakeEdge` then
  rejected it with `DifferentPointsOnClosedCurve`. The mechanism is a seam, not a bug in either
  side. The tracer samples one grid step (`2⁻³⁰ ≈ 9.3·10⁻¹⁰`) inside each cell end, which is what
  keeps a pinch tight (§11.4), so a traced loop carries a pair of vertices `≈10⁻⁹` apart at every
  cell boundary; each becomes a wall whose curved rails span `≈10⁻⁸` in 3-D, an order **below**
  OCCT's `10⁻⁷` vertex tolerance. The curve's own two ends therefore read as coincident — a closed
  curve — while the two vertices handed with it are distinct, and OCCT declines. Measured on the
  L-slot: 220 shell vertices at only **145 distinct positions**, 76 sub-tolerance Bézier edges;
  the disc and metric-drill panels, whose loops have no cell structure, had none. Fixed on both
  sides of the same principle — `hole_poly` merges emitted vertices closer than a declared
  `MIN_STEP` (`2⁻²⁰`, three orders above the snap grid and three below the device's certified cut
  bound), and the solid builder's station list is thinned the same way, since a slice `10⁻⁹` wide
  has the same problem in its lids. The keyhole then exports clean (156/156 distinct vertices, 0
  sub-tolerance edges) and the L-slot's residue fell 76 → 4, **still enough for OCCT to refuse**.
  The four survivors sit at `σ = 0`, where this L's authored corner lands exactly on the panel's own
  station, and they are not polygon vertices or stations — coarsening `MIN_STEP` by 16× does not
  move them — so they come from a third source, found and closed the next day (next entry). Two
  general points are worth keeping. **The verdict does not cover the exporter**: `Verified` is a statement
  about the rails, and says nothing about whether a floating-point consumer can represent what was
  built — a demo that does not actually run the exporter cannot discover that. And **an export
  profile needs a declared minimum feature size, enforced where geometry crosses into it**, rather
  than an assumption that the exact tier never emits anything smaller.
  *2026-08-16 · `crates/export/src/trim.rs::hole_poly`, `crates/export/src/brep_build.rs::thin_stations`*

- **The last four were the loop and the partition disagreeing by less than either could carry —
  neither wrong, and the pair unbuildable.** The residue above was neither a rail-piece join (the
  builder already stitches those to agree *exactly*, `stitched_poly_chain`) nor a station pair (they
  are thinned). Instrumenting every emitted rail edge with its σ-interval named it in one run: the
  four edges run from `σ = 0` to `σ = ∓2⁻³⁰`, spans `5.8·10⁻⁹` and `9.7·10⁻⁹`. `σ = 0` is the gore's
  own midpoint station and the L's authored corner lands there, so the tracer — sampling one grid
  step inside each cell end (§11.4) — puts the loop's vertex `2⁻³⁰` from it; the slice boolean then
  clips the loop *at* the station and the lid runs from that clip to the vertex beside it.
  `hole_poly`'s merge could not see it, because it compares a loop's vertices with **each other**,
  and this pair is a vertex against a partition point derived independently of the loop. Fixed by
  reconciling them where both are known — the builder snaps a polygon-hole vertex within
  `min_export_step` of a station onto it (`snap_poly_to_stations`) — and the **vertex** moves, not
  the station: the station is shared by every rail and every other hole and carries the exported
  patches' positive-weight validity, while `hole_poly` already declares this polygon to be the loop
  only to within that same step. The L-slot now writes its `.step` (`occt=ok`, 80 faces, shortest
  emitted edge `1.6·10⁻³`), and the regression is a fixture rather than a device: a stepped hole
  authored `2⁻³⁰` off the station must build **what the same hole authored on it builds, vertex for
  vertex** — with the snap disabled that assertion fails, and OCCT returns the original
  `MakeEdge(bezier) failed`. Two things worth keeping. The measurement that ends a hunt like this is
  the **emission site with its domain coordinates**, not the artifact: three plausible sources were
  eliminated by argument and the fourth arrived with its σ printed. And the general shape of the
  defect — *two independently derived structures reconciled nowhere, disagreeing by less than the
  consumer's resolution* — is where to look first the next time everything certifies and the
  exporter still says no. Deliberately **not** extended to the band channel's own piece boundaries:
  no such disagreement has been measured there, and `HoleRail` is itself up for retirement (#266).
  *2026-08-16 · `crates/export/src/brep_build.rs::snap_poly_to_stations`, `crates/export/src/trim.rs::export_apart`*

- **A σ-window derived from the quadric walls does not cover a profile that also has affine ones —
  and the tracer refuses the cut rather than mis-building it.** Station targeting needs the σ-range
  where a cutter is active. For a quadric wall that is its tangent-ruling window; a profile of
  straight edges has none, so AUTH.1e.2 gave an **all-affine** profile a bounding-circle proxy. The
  criterion was one case too narrow: a *mixed* profile — AUTH.2f's keyhole, a circular head with a
  straight stem — took its window from the head's circle alone, and the stem runs past it. The
  tracer then found its footprint occupying the scan's own first or last ruling and refused with
  `ShadowUnbounded`, surfacing as `PartFault::CutUnresolved`, insensitive to `clearance` and
  `segments` because it was structural rather than loose. Fixed by asking for the proxy whenever
  **any** wall is affine; a profile whose walls are all quadric still needs none, since each wall's
  window covers its own arc. The same edit removed a second latent error: the "is this bracket a
  real window" test read the *wall-indexed* pullback, which is the proxy's own only in the all-affine
  case, so a mixed profile filtered the proxy's brackets by the circle's reality. Both are the same
  mistake — a rule stated for the case that motivated it rather than for the property it needs. The
  general form is worth keeping: **a superset of stations costs only samples where the cut is
  absent; a missing one loses the cut**, so when the covering argument is in doubt, widen.
  *2026-08-16 · `crates/author/src/resolve.rs`, AUTH.2f*

- **A small metric disc can resolve `Inactive` — a green certificate on a cut that does nothing —
  when its σ-window is narrower than one cell of the resolver's root scan.** Found while building
  AUTH.2f's metric probes: a disc of radius `1/16` at `(1/16, 37/16)` on the device gore resolves
  `OpRole::Inactive` and develops to a hole-free panel, while the *same radius* at `(1/16, 9/4)`
  cuts a hole. The two are geometrically indistinguishable in kind, and the discriminator is
  arithmetic: `surface_disc_roots` seeds its sign-change scan with a fixed **256** subdivisions of
  the whole σ-band `[−7/2, 7/2]`, a cell width of `7/256 ≈ 0.02734`, and the two windows are
  `0.02703` and `0.02779` wide. The narrower one puts both tangent roots inside one cell, the scan
  sees no sign change, and the op is reported as never touching the material. This is the
  fail-**open** direction and nothing downstream can object: a hole that was never derived leaves no
  trace in the flat pattern, the solid, or ε. Worth stating as a pattern — the resolver's station
  targeting is still a *scan* while AUTH.2a built the **exact** event set (disc + resultant,
  Sturm-isolated) for the tracer, so the fix is to point the same machinery at this one. Not fixed
  in AUTH.2f: it is a pre-existing AUTH.1 gap with a blast radius across every pinned ε, chord golden
  and work budget, and it deserved its own slice (next entry). The AUTH.2f probes were placed clear
  of the threshold instead, which is itself the reason to record the number: the next fixture placed
  by eye will land on it.
  *2026-08-16 · `crates/export/src/trim.rs::surface_disc_roots`, `crates/author/src/resolve.rs`*

- **The feared blast radius was nil, and that is the finding.** Pointing AUTH.2a's exact machinery at
  the resolver's window derivation — `develop::cut::tangent_events`, the `Tangent` family on one form,
  Sturm-isolated — turns "did the scan happen to straddle both roots" into "isolate every root, then
  read a window as the **gap between two brackets**". The gap is the part of the window the brackets
  prove root-free, so the discriminant has one sign across it and a single midpoint evaluation decides
  it; that is a property a sign scan cannot offer at any subdivision. The reproduction now resolves
  `Hole`, and its pin is a differential rather than a verdict: development is an isometry, so the same
  radius cuts the same **area** wherever it sits, and the narrow-window drill must develop to the wide
  one's hole (0.018307 both). What was expected to be expensive was re-measuring the pins. Every
  printed number in the author suite — VV.1 counters (γ cells 2256, γ′ 2640, cut evals 4096), VV.2 ε
  (develop 4.1481e-1 · fold 1.3879e-1 · refold 5.9982e-3 · solid 5.7663e-2 · flex 2.7573e-1 · L-slot
  4.8792e-1 / cut 2.8439e-4 · keyhole cut 1.4320e-2), VV.3 goldens (3.0 / 7.7 / 9.1 / 9.4 / 10.1 %),
  the probe areas and the fold residual — is **bit-identical** before and after, because for every
  window the scan *did* find, the exact brackets land within `2⁻⁴⁰` of its bisected roots and
  everything emitted snaps to `2⁻³⁰`. Worth keeping as a pattern: *a sampling assumption's blast
  radius is feared for the cases it got wrong, but it is measured on the cases it got right — and
  those are exactly the ones a sound method reproduces*. Runtime is unchanged too (one Sturm chain
  over a reduced discriminant per wall per region, against 256 polynomial evaluations). The related
  scan in `surface_tangents` is deliberately left: its `span` is the resolved window padded by a
  sixteenth of its own width, so it is a *relative* scan and cannot exhibit this defect — the doc now
  says so, and `surface_disc_roots` is private, so the absolute-band form is no longer reachable.
  *2026-08-16 · `crates/develop/src/cut.rs::tangent_events`, `crates/author/src/{resolve,realize}.rs`*

- **A merge is not distinguished by its stretch count, and the first version of the keyhole test
  proved nothing.** AUTH.2f needed a fixture exercising a genuine **merge** — two ruling stretches
  rejoining — as opposed to the L's births and deaths. The first test asserted exactly that: sweep,
  find the drop from two stretches to one, and check that what closed was the **gap** between them
  rather than either one's width. It passed. Pointed at the L instead of the keyhole, it also
  passed, because an L's two arms *do* rejoin at the reflex corner — the premise that the L only
  births and dies was simply wrong, and the test was measuring a property both shapes have. What is
  actually the keyhole's own is **which walls face across the closing gap**: the head's circle and a
  straight stem side, so the saddle is the mixed quadric-against-affine case of the pairwise
  resultant, which no polygon can reach and which §11.2 asks for by name. Re-asserted by reading the
  wall each end names and checking their pullbacks differ in degree; the L now fails it. The general
  lesson is the vv-guide's, one turn further in: it is not enough to name the phenomenon a fixture is
  for — the *assertion* has to be one the other fixtures fail. *2026-08-16 · `crates/develop/src/cut.rs`*

- **The fixture that produces the phenomenon had to be searched for, twice, and the search is
  recorded because it is not free.** §11.6 already noted that an L along the rulings has a band
  footprint. The keyhole added the same lesson for a curved profile: what a ruling has to cross to
  see two stretches is the small **notch beside the stem**, so a stem `3/5` as wide as the head
  nearly closes it. Measured over an 800-ruling sweep, a wide stem gave 9 two-stretch rulings, a
  narrow one (`7/25`) 14, and the rotation mattered as much — the unrotated keyhole gave 8, four
  orientations gave 0. Both constants in `keyhole_profile` are therefore load-bearing, and the doc
  comment says so, because the next person to "simplify" the fixture to round numbers will silently
  delete the property it exists to exercise. *2026-08-16 · `crates/develop/src/cut.rs`, `crates/acceptance/src/lib.rs`*

- **A σ-station crossing leaves no trace in the artifact, so the acceptance demo counts it.** A hole
  that crossed a station and one that sat inside a single slice certify alike and build alike; the
  emitted solid has nothing that distinguishes them, since the extra wall a crossing adds is
  indistinguishable from an ordinary one. AUTH.2f could therefore assert only *consequences* a
  within-slice hole shares — watertight, manifold, one added handle — and would have had AUTH.2e/2
  on its critical path by assertion rather than by evidence. `develop::counters::poly_slice_clips`
  closes it: bumped once per slice the builder's general polygon channel trims, so with the slot as
  the part's only polygon hole a count above 1 *is* the crossing. Measured 2 for both AUTH.2f
  fixtures and 0 for the un-slotted control (the control's zero matters — otherwise the counter
  might be measuring the panel rather than the slot). Worth generalizing: when a milestone's claim is
  about **which branch ran**, the artifact is the wrong place to look for it, and a counter is not a
  performance tool but a witness. *2026-08-16 · `crates/develop/src/counters.rs`, `crates/export/src/brep_build.rs`*

- **"A radial at an interior station is a shared cross-ring" was a property of `HoleRail`, not of
  the builder — and reading it as the builder's made a `Verified` solid with four free edges.** The
  trim builder emits a wall per footprint edge except a radial at an interior σ-station, which two
  adjacent slices are assumed to share. That assumption held for as long as every interior hole was
  a `HoleRail`: its near/far branches are *continuous in σ*, so both slices cut the station at the
  same two µ̂ and the two lids really do meet along one edge. AUTH.2e's polygon channel breaks it —
  a hole with a `σ = const` edge sitting **on** a station keeps material on one side and not the
  other, so the two lids differ there and the step between them is a wall. Skipped, it left four
  free edges under a `Verified` verdict: an open shell reported as a solid, and the pipeline had
  nothing to object with, since `Part::solid()`'s verdict is about the *certificates* upstream and
  not about the shell it hands back. Found by measuring rather than reasoning — the fixture already
  in the suite (`fold_part`'s authored L, whose step lands exactly on σ = 0, which is where an
  authored corner tends to fall) was about to be flipped from `SolidRefused` to green on the
  strength of the verdict alone. The rule now asks the neighbouring slice for its segments on that
  station and emits the wall unless one matches exactly; a partial overlap is refused rather than
  sewn. Exact matching is sound *because* each slice runs its boolean against the whole loop rather
  than a pre-clipped one, so both sides see the same crossings on the shared line.
  *Generalizable:* when lifting a restriction, the invariants the old special case *supplied* are as
  load-bearing as the ones it required — and they are invisible, because nothing states them.
  *2026-08-16 · resolved · `export::brep_build::{cross_ring,radial_segments}`, design §11.7*

- **A fail-closed refusal is only as honest as the premise it rests on — and mine was false for
  every hole the kernel emits.** Bringing the general polygon channel up to per-slice clipping, I
  refused any slice reached by *both* a polygon hole and a `HoleRail`, reasoning that a rail's
  branches are curved and a boolean has no operand for a curve. The gate rejected it in the first
  run: the doctest panel is precisely that case (an authored slot beside a derived drill), and the
  leg the milestone was explicitly not allowed to regress is the one that broke. The premise was
  wrong — `hole_rail` builds **linear** rails between consecutive loop vertices, so a band *is* a
  polygon, and converting it (`rail_hole_poly`) lets both kinds join one boolean. Two things worth
  keeping: a refusal added because "I cannot represent that" deserves the same scrutiny as a claim,
  since it encodes a belief about the data; and it was cheap to be wrong here only because the
  regression suite already owned the case — the design doc's §11.1 measurement said the
  within-slice mixture worked, and the test said so too.
  *2026-08-16 · resolved · `export::brep_build::rail_hole_poly`, design §11.7*

- **A non-convex profile does not give a non-convex footprint, and a reflex corner in the flat
  pattern does not prove one either.** AUTH.2d's fixture is an L-slot cutter, and the first two
  attempts were *false negatives that looked like tracer bugs*. **(1)** This cone's rulings project
  to radial rays, so an L whose arms lie along the radius is met by every ray exactly once: the
  notch never appears in `(σ, µ̂)` at all and the footprint is an ordinary band. An L is only
  non-band when the notch opens **across** the rulings, which took an exact `(3,4,5)` rotation of
  the profile's axes to arrange — and then a placement that keeps every vertex in the material (the
  first landed on the panel's inner carve and came back `AmbiguousRegion`, a resolver fault two
  stages upstream of the thing being tested). **(2)** Worse, the obvious check is not a check: the
  developed hole had a genuine reflex corner and *still* came from a band, because a band
  `[lo(σ), hi(σ)]` can be a perfectly non-convex planar region. The signature that actually
  distinguishes them is a ruling meeting the cutter twice — pinned here as `solid()` refusing with
  `LoopBroken`, since the near/far rail adapter is exactly what a non-band loop breaks. What AUTH.2
  lifts is a restriction on **footprints**; "non-convex profile" is neither necessary nor sufficient
  and the fixtures have to demonstrate the real thing.
  *2026-08-16 · resolved · `author/tests/sketch_cutter_part.rs`, design §11.6*

- **`hole_rail` accepted a loop it cannot represent and built a certified solid around the wrong
  hole.** The near/far rail adapter splits an interior loop at its two σ-extremes, which assumes the
  loop turns around in σ exactly twice — true of every band, and false of a traced non-convex loop.
  Handed one, `chain` swapped the ends of each backward step and sorted, producing **overlapping**
  σ-bands that the slice builder read as a hole of a different shape: the L-slot part came back
  `Verified` with a solid whose hole was not the loop that had been certified. Nothing in the
  pipeline objected, because every certificate along the way is about the *loop*, and the loop was
  fine. Newly reachable the moment AUTH.2c's tracer replaced the band builder, and found only
  because a test asserted the refusal that ought to happen — the pin written to make AUTH.2e's
  landing visible caught a live defect instead. Fixed by counting σ-direction reversals and refusing
  anything but two. *Own-goal worth recording:* the first version of that count walked `0..n` and
  never compared the last step back to the first, so a band counted **one** reversal and every hole
  was refused — a cycle is a cycle, including its wrap.
  *2026-08-16 · resolved · `export::trim::hole_rail`*

- **A differential whose tolerance is built from the quantity under test cannot fail — and it looks
  more rigorous than the sound version.** AUTH.2c's headline check is that the new general tracer
  reproduces AUTH.1e.4's band builder on the band's own square-prism fixture. Written the obvious
  way, it compared the two emitted boundaries to within `band.eps + traced.eps` — "each is a
  certified distance to the same walls, so they may differ by at most the sum", which is *true* and
  useless: the tolerance grows exactly when the tracer degrades. Mutation-testing the sampling (drop
  the grid-adjacent nodes at each cell end) made the tracer's ε **8× worse**, `1.79e-2` against the
  band's `2.24e-3`, and the check passed without a murmur. Rewritten to a fixed multiple of the
  **band's** bound alone — an external reference, unaffected by the code under test — the same
  mutation fails it immediately. Generalizes past this one test: a two-sided bound is only a test if
  at least one side is independent of what it is testing. *Related process note:* restoring a
  mutated file from a copy can leave it with an **older mtime than the build artifact**, so cargo
  silently reuses the mutated binary — a "the fix did not take" result that is really a stale build.
  `touch` after restoring. *2026-08-16 · resolved · `develop::cut` tests, vv-guide AUTH.2 criteria*

- **"Edges are not carriers" has a converse, and it only bites on non-convex profiles: a carrier
  crossing the cutter's own interior is not a boundary.** AUTH.1e.2 found that `arrange2d` splits a
  circle into two arcs sharing one carrier, so a per-*edge* wall list duplicates a surface
  (`Cast::carrier_walls` was the fix). AUTH.2b hit the dual: a carrier is the whole infinite **line**,
  not the profile edge lying on it, so a non-convex profile has carriers that run through its own
  interior — an L's `y = 1` bounds one arm and is interior to the other. Those crossings arrive in
  `ruling_patches`' sorted list like any other, and taken as-is they break one inside stretch into
  two abutting ones. Measured on an L-profile cutter: some rulings reported **three** stretches,
  which a straight line meeting two convex arms cannot do, and one L orientation reported two
  stretches at 27 of 401 sampled rulings where the truth was one. Fixed by merging stretches that
  share an endpoint — exact rather than a tolerance, since the union of two intervals sharing an
  endpoint *is* the interval, and the shared value is one `Rat` by construction. **Convex profiles
  cannot exhibit it** (their carriers are supporting lines, so every extra crossing falls outside the
  inside stretch), which is why AUTH.1e.4 shipped without it and why no existing test moved. Worth
  keeping next to its sibling: both defects come from conflating a *carrier* with the *edge* on it,
  in opposite directions, and both were invisible until a shape that distinguishes them showed up.
  *2026-08-16 · resolved · `develop::cut::ruling_patches`, #260*

- **The textbook resultant is identically zero on exactly the case AUTH.2 exists for, and a
  differential built from generic inputs would not have noticed.** AUTH.2a's event set needs
  `Res_µ̂(f_i, f_j)` — zero where two walls cross a ruling at the same µ̂ — and the quadratic-by-
  quadratic closed form `(a₁c₂−a₂c₁)² − (a₁b₂−a₂b₁)(b₁c₂−b₂c₁)` went into the design doc backed by an
  exact check against a 4×4 Sylvester determinant over 2000 random rational pairs, plus the
  shared-root and *linear-vs-quadratic* degenerate cases. All green, and all beside the point: that
  closed form is the Sylvester determinant of the two forms **padded to degree 2**, and padding a
  genuinely affine form adds a shared root at infinity. With one wall affine the padding is harmless
  (the determinant picks up a nonzero factor). With **both** affine it collapses to `0` — for walls
  that meet and walls that never meet alike. Every wall of a polygonal profile is affine, so the
  L-slot, the T-slot and the keyhole — the shapes the whole milestone is for — would have had every
  corner erased, presenting as a tracer that quietly found no events rather than as an arithmetic
  error. Fixed by dispatching on `a ≡ 0` as a rational function (2×2 / 2×1 / 1×1 forms, design doc
  §11.2); an isolated σ where a genuine conic's `a(σ)` vanishes needs no case, since the 2×2 form
  factors there into the 2×1 condition times `a_j ≠ 0`. Three tests catch the naive version, one of
  them end-to-end on the square prism, all mutation-verified against it. *The lesson is about test
  selection, not resultants:* the differential was real, independent, and exhaustive over the
  **generic** stratum, and the defect lived in the degenerate one — which is where the feature lives.
  When a formula has degenerate cases, the test set has to contain the case the feature is for.
  *2026-08-16 · resolved · `develop::cut::{MuCut::resultant, structure_events}`, #259*

- **AUTH.2's scout: the band lives in one file, and everything downstream already takes a general
  loop.** #256 sized the work as "holes must become regions end to end, through the flat boolean and
  into the B-rep builder", and named `HoleRail`'s band (`brep_build.rs:222`) as the load-bearing
  blocker to scout before planning. Measured instead, by drilling a deliberately **non-convex**
  L-shaped `(σ, µ̂)` loop through the doctest panel with `Part::hole_domain` (the authored-polygon
  channel, which is the same currency a traced footprint would produce): **(1)** the flat leg is
  free — develop → exact `arrange2d` boolean → topology gate, all `Verified`, with a convex rectangle
  as the control; **(2)** the solid leg is *also* free while the loop stays inside one σ-slice —
  `brep_trim_solid_regions`' `poly_holes` channel takes an arbitrary `(σ,µ̂)` loop as a lid inner
  wire and sweeps a wall per edge, and the shell certifies; **(3)** the one real solid-path gap is a
  loop **crossing a σ-station**, refused by name at `brep_build.rs:1739` (`SolidRefused`), confirmed
  by placing the same slot across `σ=0` with the drill removed. So `HoleRail`'s band is not what
  stands between us and non-convex profiles — it is the channel for *station-crossing* holes, which
  is an orthogonal axis. Two corollaries: the **resolver was already general** (AUTH.1e.1's
  `Shadow(Vec<Patch>)` carries several µ̂-stretches per ruling, so structure/stations/spans need no
  work), and the refusal is confined to **two lines** in `develop::cut` (`ruling_patch`'s
  several-stretches check, `cut.rs:663`, and the window-gap check, `cut.rs:812`). AUTH.2 is therefore
  a **tracer** milestone, not a plumbing one. *Method note:* the first three runs all refused with
  `TopologyMismatch` and it took a convex control to show that was placement, not convexity — the
  slot was sitting on the panel's existing drill, and then on the inner boundary. A refusal that
  looks like the thing you are testing is worth one control run before it becomes a finding.
  *2026-08-16 · resolved · #256, `crates/develop/src/cut.rs`, `crates/export/src/brep_build.rs`*

- **A drafted round hole is not a round cone, and AUTH.1a's surface-class table said it was.** The
  GO-gate's table (`docs/cutter-extrude-design.md` §2.2) mapped *arc + finite apex* to a `Cone` and
  *arc + direction* to the existing `Cylinder`, meaning the two metric surfaces the kernel already
  had. Both cells are wrong, for two reasons that are independent — either one alone forces the
  general form. **(1)** The cone over a circle from an apex **off that circle's own axis** is an
  *oblique* circular cone, which as a quadric is an **elliptic** cone, not a right circular one. A
  cutter has one cast point serving the whole profile, so it is off-axis for all but one of the
  profile's arcs — the on-axis case is the exception, not the rule. **(2)** Under the *affine* frame
  §3 argues for, a profile "circle" is already an ellipse in 3-D, so even the parallel case sweeps an
  elliptic cylinder. Resolved with **one** new variant, `CutSurface::Quadric` (general `XᵀMX+b·X+c`
  on one `Nappe`), which is *fewer* new variants than the table implied while being strictly more
  general; `Plane`/`Cylinder` keep their exact closed-form distances untouched. The knock-on is the
  real cost: a general quadric **has no closed-form distance**, so the certificate needed new
  machinery — the first-order gradient-flow bound (see the next entry). Worth noting how the error
  survived review: the table was checked for *degree* (everything stays ≤ 2, which is true and is
  what the pullback needs) and that was silently read as also fixing the *metric class*, which it
  does not.

- **A semi-hermetic build only looks hermetic until it meets a bare machine.** The first
  `x86_64-linux` CI signal in eight days came back red — not on any AUTH.1 code (90/90 `export`
  tests passed there, including the 122s `full_panel_assembles`) but on the **doctests**:
  `libTKDESTEP.so.7.9: cannot open shared object file`. The devShell lists `opencascade-occt`, which
  puts OCCT on the *compile* path, and `build.rs` derives `-L <occt>/lib` from it — but nothing ever
  declared it a **runtime** dependency. Ordinary test binaries survive because nix's ld-wrapper bakes
  an rpath at the final link; rustdoc's doctest binaries are linked without the build script's link
  args, carry no rpath, and the loader has nowhere to look. It had worked everywhere OCCT happened to
  be reachable by other means; a runner with nothing installed globally has nothing to fall back on.
  Fixed by declaring the runtime path once in the devShell (`LD_LIBRARY_PATH` +
  `DYLD_FALLBACK_LIBRARY_PATH`). The general shape is worth remembering: **a dependency that is only
  ever declared at build time is untested as a runtime dependency**, and the machine that finds out
  is the most minimal one you own. It also took eight days to learn, because a CI leg that never
  finishes is indistinguishable from one that passes — the hang (fault a) hid the misconfiguration
  (fault c) completely.

- **A green certificate on a cut that did nothing — and the doc had predicted it.** AUTH.1e.2's
  station criterion is "a wall whose µ̂-pullback is a genuine quadratic (`a ≢ 0`) has tangent windows
  and needs targeted stations; an affine one does not." That reproduces `Cylinder` vs `HalfSpace`
  exactly — and gives a **polygon zero stations**, because every wall of a polygon is affine. A
  square slot subtending ≈0.045 in σ against ≈0.146 sample cells fell between them and the resolver
  derived `OpRole::Inactive`: the part certified, the authored cut was simply absent. That is
  verbatim `docs/cutter-extrude-design.md` §6's prediction — "an extruded cutter would silently
  receive no targeted stations and drop small features between cells" — reintroduced *while*
  carefully preserving the behaviour §6 was warning about. Fixed with the profile's bounding circle
  as a probe: its wall is a quadric, so it has one tangent window, and that window contains the whole
  profile's σ-support. A **superset is the right error** here — extra stations sample where the cut
  is absent and cost nothing, a missing one loses the cut silently.

- **ε cannot see a draft angle, so the acceptance check had to be geometric.** The drafted and
  parallel variants of the same hole certify at *the same* ε (4.879e-1 both), because ε is the max
  over pipeline stages and the panel's boundary dominates. Every certificate in the pipeline was
  green on a cut whose *shape* was the entire point. The check that distinguishes them measures the
  developed hole — 0.4759 vs 0.5969, ratio 0.797 against the taper law's 0.797 — and is a test, not
  demo output. Worth generalising: when a feature's headline property is a *shape*, a residual bound
  that is dominated by something else is not evidence about it, however green.

- **Four wrong diagnoses before one controlled comparison.** Chasing why the demo failed, I: read
  ε ≈ 0.94 as the `Unresolved(clearance)` sentinel (it is exactly 1.0 — I quoted the number without
  checking what it implied); blamed `fit_cut_rail` declining `Quadric` (real code fact, wrong
  culprit — extruded cuts derive `Hole` and take the p-curve route, which works); predicted the ring
  would silently emit a disc (it fails closed); and estimated the fix as PC.3-scale (the half that
  mattered was 30 lines). The actual cause was my demo recipe dropping the rim notch — nothing to do
  with AUTH.1. What ended it was authoring the *same solid* two ways and comparing, which is the
  first thing I should have done. **Write a new recipe by changing one line of a working one.**

- **Edges are not carriers, and the difference cost a duplicated hole.** AUTH.1b built
  `Cast::walls` per *edge* and documented that as deliberate: "carriers are not deduplicated, which
  is what the caller wants when it is walking edges." AUTH.1e.2's caller wants the opposite and the
  doc note did not save me. `arrange2d` hands out **decomposed** pieces — a circle arrives as its
  two x-monotone arcs — and both arcs sweep the *same* quadric, so an extruded disc produced two
  identical walls. The µ̂-shadow survived that (coincident crossings leave zero-width stretches,
  which the scan skips), but the σ-window station loop runs per wall, so the window was recorded
  twice and the resolver derived **two interior holes where the cylinder it equals derives one**.
  Fixed with `Cast::carrier_walls`, which dedupes by carrier — lines normalized against their first
  nonzero coefficient so the same line at a different scale collapses too.
  Two things worth keeping. First, **what caught it**: not the three unit tests of `extruded_shadow`
  (all green, all passing before and after), but the end-to-end differential that authored the *same
  solid* two ways and compared the resolved structures. The bug lived entirely in the layer above
  the function under test. Second, the smell in advance: I had written "which is what the caller
  wants" about a caller that did not exist yet. A justification written for a hypothetical consumer
  is worth re-checking the moment a real one appears.

- **A lap is made of support, not of charts — so a span counts regions.** AUTH.1d started from the
  design's phrase "neutral surfaces (chart embeddings)" and the obvious reading, one chart = one
  surface, is wrong on the very device the acceptance test uses. Cast down the seam-drill axis at
  the *bare* wrap chart and it reports **three** crossings — but two of them are at the **same 3-D
  point**, because with `h ≡ 0` the flap and the body coincide exactly; the chart is a double cover
  there and the two σ are the same material. Give each region its own support law and they separate
  by the ramp height (measured: `2.892` vs `3.042` along the ray, a gap of `0.149`). So the unit of
  counting is a **region**, and a span computed against a bare chart counts a double cover rather
  than layers. Two smaller things fell out of the same probe: the ordering must be by **ray
  parameter**, which on this device is the *reverse* of the σ order — an ordinal read off σ inverts
  the lap — and a `Ray` is a ray, not a line, so the far wall the same line meets at `t = −3.04` is
  not a crossing. Both are now filters, and both are in the named test.

- **A backward-error bound can be blind to the quantity that matters downstream.** AUTH.1c's
  obvious residual is the distance from the picked frame's origin to the cast ray's *line*, and it
  is a perfectly good bound — of the wrong thing. The ray parameter `t` is what the span (§5) orders
  hits by, and a `t` of the **wrong sign** puts the point on the same line, so the line bound
  certifies it happily. There was in fact a sign error in the 2×2 Cramer solve (`t` negated), and it
  survived the first green test run: ε was ~10⁻¹⁵ and every certificate passed. What exposed it was
  not the certificate but an ordering test that turned out to be **vacuous** — the probe ray crossed
  the cone once, so the `windows(2)` loop asserted nothing. Fixing the fixture to a ray that crosses
  twice made `t` observable, and the exact corroboration `t₀ + t₁ = 10` (the crossings are symmetric
  about the axis the ray is aimed at) pinned it. The residual is now point-to-point, which is
  strictly stronger at no cost and certifies `t` along with the position. Two habits earned their
  keep: **be suspicious of tests that pass first try**, and check that a bound constrains the
  quantity a consumer will actually read, not merely a quantity that sounds like it.

- **Building walls from carriers instead of endpoints deleted three problems at once.** AUTH.1a
  built a wall from the *endpoints* of a profile edge (`segment_wall`: one determinant through two
  3-D points and the apex), following the GO-gate's §4 wording. AUTH.1b needed the same thing for a
  profile coming out of `arrange2d`, where that shape does not fit: an edge's endpoints are `Surd`
  (degree-2 algebraic after any boolean) and an arc's circle is stored by **`r²`**, never `r`.
  Rather than plumb algebraic coordinates through, the wall is now read off the edge's **carrier**
  and built by the one rule in §2.3 — frame coordinates are a rational quotient, so any 2-D carrier
  equation becomes a 3-D surface by substituting it and clearing the denominator. Three things fell
  out of that single change: the `Surd` endpoints never enter (a wall is unbounded; trimming is a
  `(σ, µ̂)` boolean); `r²` survives the clearing **linearly**, so no root is ever taken; and each
  wall inherits its own carrier's sign, which leaves the fill rule with the region and is exactly
  why a non-convex profile with holes needs no decomposition. `segment_wall` was then a second
  derivation of what `line_wall` computes, with no consumer left — deleted per the standing
  no-ossification rule, its property re-pinned against the engine that survived. The general lesson
  is the one the milestone keeps producing: the *carrier* is the durable object, the endpoints are
  trim data, and code that reaches for endpoints is usually about to need a coordinate type it
  cannot afford.

- **The first-order distance bound wants headroom, and says so.** `Plane` and `Cylinder` have exact
  distances; a quadric gets `dist ≤ |F|/g` from the gradient-flow lemma, valid when `|∇F| ≥ g > 0` on
  a ball `B̄(X, R)` and `|F|/g ≤ R`. Three things fell out while building it. **The hypothesis is
  free**: the largest useful `R` is `clearance/2`, which is exactly the DRC gate, so the lemma holds
  on precisely the runs that end `Verified`. **`R` must be searched small-first**: `g` is a minimum
  over the ball, so a smaller ball gives a tighter ε — always using the ceiling would inflate ε by
  the ratio of the clearance to the true error, i.e. by the whole quantity being measured. And the
  bound **cannot** work when the ball reaches the surface's singular locus (a cone's apex, a
  cylinder's axis): measured on the device's `R = 1/5` drill, an error of `5·10⁻⁴` certifies at 1.4×
  the exact distance, while an error of `6·10⁻²` — a chord across a third of the hole — is
  `Unresolved`. That is the honest verdict rather than a defect: at that scale "distance to the
  surface" is not a first-order quantity. The test that first hit this was measuring the wrong thing
  (a deliberately-coarse chord rail) and its failure was the finding.

- **The axiom gate was rejecting the one axiom the docs say it should accept — and its parser could
  have missed a real one.** `main` has been CI-red since 2026-08-09 on the Lean step, with the build
  itself clean (8823 jobs, every footprint `[propext, Classical.choice, Quot.sound]`) and the failure
  coming entirely from the audit: `AXIOM AUDIT FAILED: non-allowlisted axiom(s): sturm_root_count`.
  That axiom is the *deliberate citation* on `verify_chain_sound` — the 📌 row of
  `docs/proofs/ledger.md`, whose header states 📌 rows are in the gate. When the audit was tightened
  from "grep for `sorryAx`" to "allowlist `[propext, Classical.choice, Quot.sound]`" the documented
  citation was not carried across, so the gate contradicted the ledger. **The interesting part is the
  second defect**, found while fixing the first: the old parser did `grep "depends on axioms:"` and
  then flattened every footprint into one anonymous stream. That has two consequences — it *cannot*
  express a per-theorem rule (so the obvious fix, adding `sturm_root_count` to the global allowlist,
  would have let that axiom appear under **any** proof unnoticed, which is precisely the leak the gate
  exists to catch), and because `#print axioms` **wraps long footprints across lines** while only the
  first line matches the grep, any axiom pushed onto a continuation line was invisible — a `sorryAx`
  could have slipped through on a sufficiently long footprint. Replaced by
  [`scripts/check-axioms.sh`](../scripts/check-axioms.sh): joins wrapped records, checks each
  theorem's footprint against *its own* declared citation, and is **two-sided** — a citation that
  stops appearing also fails, so discharging Sturm later forces the ledger row from 📌 to ✅ instead
  of silently going stale. Guard verified by negative test, not just by passing: dropping the
  citation, inventing one, and renaming a theorem each exit 1 with the right message.
  *2026-08-15 · resolved · `scripts/check-axioms.sh`, `.github/workflows/ci.yml`, `docs/proofs/{ledger,README}.md`*

- **A Kani harness that has never once run in CI, and costs 45+ min when it does.** DEV.2a
  (`3e18c61`, 2026-08-10) added `floor_ceil_fast_path_panic_free_full_domain` together with its
  `--harness` entry, and the commit message states "Kani harness runs in CI." **It never has.** The
  Kani step is 10th of 13, and no run since has reached it: the `main` runs that *did* pass Kani
  predate the harness (their logs tally `3 successfully verified harnesses` for `lattice`, and the
  echoed command lists only three), `dev-go-gate` died at `fmt`, `pcurve` died at the OCCT step, the
  rest are `fuzz-nightly` (a different workflow) or queued behind the dead Linux runner. So the
  green-looking 21-second Kani step everyone remembers is a step that was never asked to do this work.
  **The cost is real:** locally the harness runs 45+ min of single-threaded CBMC (kani 0.67.0,
  aarch64-darwin, 211 k vars / 1.12 M clauses) while the other twelve harnesses together take ~21 s.
  **Why:** the harness asserts the Euclid identity `num == f * den + rem` over the *full* `i128`
  domain, so on top of two symbolic 128-bit divisions CBMC must bit-blast a symbolic **128×128
  multiply**. That is a correctness claim, not a panic-freedom one — and `proof.rs`'s own header
  assigns exactly that split elsewhere ("BMC is the wrong tool for iterative number theory"; gcd /
  reduce *correctness* is Lean's, Kani keeps the gcd-free bridge + panic-freedom), with the defining
  brackets already covered natively by `floor_ceil_fast_path_grid` + the slow-tier differential. So
  the expensive assertion is both the odd one out doctrinally and the whole cost. **Measured, not
  guessed:** a probe harness identical but for that one line verified in **18.8 s** against the
  original's 45 min+. **Resolved** by dropping the identity and keeping every panic-freedom
  assertion — the whole four-harness `lattice` leg now verifies in 170 s including the build.
  Note what the assertion actually claimed: the harness *mirrors* the fast path rather than calling
  `Rat::floor`, so `num == f * den + rem` is `div_euclid`/`rem_euclid`'s own **libcore** contract
  restated symbolically, not a fact about `lattice`. **That mirroring is itself a smell** worth
  carrying: a harness that re-implements the code under test passes even when the two diverge, and
  this one never exercised `Rat::floor`'s Fast/Slow dispatch at all.
  *2026-08-15 · resolved · #244, `crates/lattice/src/proof.rs`*

- **A feature-gated test is only as good as the leg that compiles it — and a CI matrix is only as good
  as the legs you actually read.** Running the full gate locally before pushing the OPT.3 arc turned up
  a hard compile failure in `cargo clippy -p export --features step --all-targets`:
  `trim::tests::full_panel_solid_exports` still called the pre-PC.6 **10-arg** `hole_loop` (with the
  deleted `fit`/`margin` ladder args) and still built `HoleRail` with the pre-PC.5 **scalar** `near`/`far`,
  which PC.5 had widened to `Vec<(Interval, RatFunc)>`. Nothing in the OPT.0–OPT.3 arc touched `trim.rs`;
  the break was ~2 weeks stale. **Mechanism:** the test is `#[cfg(feature = "step")]`, and the default
  `--workspace` clippy/nextest legs pass no `--features`, so it is *compiled out* of every fast local
  check. Exactly one of the thirteen CI steps builds it, and it sits eighth — so the whole cheap prefix
  stays green and the local loop never types the code at all. The sibling *flat* test
  `full_panel_assembles`, two screens up in the same module, had been migrated by PC.6 correctly; the
  gated one was simply invisible. **The remote agrees and adds a second failure:** run `31800967684`
  (the PC.5 commit) failed the macOS leg at precisely that step, exit 101, and every push since
  (PC.6 → OPT.0–3 → VV.1 → MAP.1) is still `queued`/`in_progress` behind a backlog — so no green run
  exists for any of it. Meanwhile `build (self-hosted, Linux, x64)` hung *inside*
  `DeterminateSystems/nix-installer-action@main` and was killed by the 6 h job cap without reaching step
  one, so `x86_64-linux` — a first-class target per `environment-and-crate-layout.md §4` — has had **zero**
  signal for days. Both failure modes are silent in the way that matters: one hides behind a feature
  flag, the other behind a queue. *2026-08-15 · the break resolved, the two gaps open · `crates/export/src/trim.rs`, #241, #242*

- **The OPT.3 re-proof hits a TCB question, not a proof-difficulty question: `trailing_zeros` is an
  intrinsic Aeneas cannot model.** Regenerating the model (`nix run .#extract`, clean) lifts the new
  gcd tidily — `gcd_u128_loop0_loop0` (the `u64` inner Euclid, structurally the old proof at a second
  width), `gcd_u128_loop0` (outer, with the narrowing branch), and a wrapper. But `lake build` fails
  on the *model*, not the theorem: **`Unknown identifier core.num.U128.trailing_zeros`**, emitted
  into `extract/lattice.FunsExternal_Template.lean` as a bare `axiom … : Std.U128 → Result Std.U32`.
  `Lattice/FunsExternal.lean` states the house rule plainly: such holes are filled with a *faithful
  `def`* rather than left as axioms, "which would pollute every downstream proof's `#print axioms`
  footprint and defeat the axiom-clean guarantee", and those defs "are the ENTIRE hand-written TCB
  surface of the `lattice` model" (guarded by `scripts/check-externals.sh`). So the shipped gcd needs
  **one new entry on that audited surface** — small and auditable ("count of trailing zero bits, 128
  for 0"), arguably simpler than the `unsigned_abs` already there, but growth nonetheless.
  **The alternative costs performance instead of TCB:** strip the twos with a plain loop, which lifts
  natively and is provable with the same `loop.spec_decr_nat` machinery. Measured: **80.1 ns/call
  (3.24×)** against the intrinsic version's **44.2 ns (5.87×)** — end-to-end roughly **1.7× vs 2.0×**,
  i.e. ~14% of the overall win traded for zero TCB growth. Note the intrinsic route is also *less*
  proof work (one def + one lemma, versus three loop specs), so this is not a
  effort-versus-purity trade — it is purely performance versus audited surface.
  *Either way the remaining theorem work is shared*: the `u64` loop spec (the existing proof at a
  second width), the strip-twos identity `gcd(2^i·m, 2^j·n) = 2^min(i,j)·gcd(m,n)`, and the
  shift-no-overflow bound `gcd(m,n)·2^shift ≤ min(a,b)`. *2026-08-14 · open · OPT.3-proof (#240)*

- **Stein / binary GCD benchmarked head-to-head and rejected on evidence — it is not faster here, on
  either operand mix.** The earlier rejection was an argument about proof cost; since OPT.3 already
  owes a re-proof, the question was legitimately reopened and settled by measurement instead.
  *On the harvested mix:* pure Stein **243.4 ns/call — 1.08×**, essentially no better than the plain
  Euclidean loop it would replace. The reason is the same fact that made strip-twos win: 84.7% of
  calls have a power-of-two operand, whose odd part is 1, and Stein walks ~bit-length shift/subtract
  iterations to discover `gcd(m, 1) = 1` where one comparison settles it. Add that trivial exit and
  Stein lands at **44.6 ns — identical to the shipped strip-twos (1.00×)**, because the exit is
  doing all the work and the algorithm underneath is irrelevant.
  *On general operands only* (the power-of-two share removed, to test whether the conclusion is an
  artifact of this device's dyadic grids): current 399.7 ns, **shipped 283.2 ns (1.41×)**, Stein
  336.5 ns (1.19×) — **Stein is 0.84× the shipped speed, i.e. 16% slower**. The `u64` narrowing puts
  the general case on a *hardware* divide, which beats an O(bit-length) shift/subtract loop; Stein's
  advantage only materializes where no divide is fast at any width.
  **Conclusion: no case for Stein at any point in the mix, and it carries the larger proof burden
  (new measure + invariant) — so the strip-twos shape stands.** Worth keeping because the intuition
  is genuinely misleading: "the divide is slow, so use the division-free algorithm" is exactly the
  wrong inference when the real win is an early exit and a narrower divide.
  *2026-08-14 · resolved · OPT.3 (#239)*

- **OPT.3's proof debt is discharged — `gcd_u128` re-proven, same axiom footprint.**
  `CertifyCheck.gcd_u128_spec` is green at **`[propext, Classical.choice, Quot.sound]`**, identical
  to what the original Euclidean proof carried, and `Lattice/FunsExternal.lean` gained a *faithful
  `def`* for `trailing_zeros` rather than an axiom — so nothing leaked into any downstream
  `#print axioms`. `check-externals.sh` green at 13 modelled items, full `lake build` green.
  **The pricing done up front held exactly.** The plan was: reuse the loop argument, add the
  strip-twos identity, mirror the loop at `u64` width. That is what shipped —
  `gcd_u128_loop0_loop0_spec` is the original `loop.spec_decr_nat` argument transplanted verbatim
  to `u64`, and `gcd_two_pow_mul` (`gcd(2^i·m, 2^j·n) = 2^min(i,j)·gcd(m,n)`, via `Nat.gcd_mul_left`
  plus coprime cancellation on the odd parts) is the only genuinely new mathematics. This is the
  concrete reason a binary/Stein gcd was the wrong choice even before the benchmark said it was
  slower: it would have discarded that reusable loop argument.
  **Two things that cost time and are worth knowing.** *(1)* Aeneas's `<<<` **wraps** mod `2^128`
  and carries a `shift < 128` side condition — it does not fail on value overflow — so the
  no-wrap fact has to be threaded as a hypothesis (`hfit`) through the loop invariant rather than
  discharged locally. *(2)* `simp`/`scalar_tac` hit `maxRecDepth` on the 39-digit `u128` literal in
  the zero branches, the same hazard `FunsExternal`'s `irreducible_def i128FitBound` is sealed
  against; targeted `rw` with `Nat.gcd_zero_left/right` avoids it.
  *2026-08-15 · resolved · OPT.3-proof (#240)*

- **OPT.3 shipped — 1.7× on `develop`, 2.9× on `fold`, and the hot spot is gone.** `gcd_u128` is now
  strip-twos + `u64`-narrowed Euclidean (see step 0/1 below for the harvest, the benchmark and why
  this shape rather than a binary gcd). **Measured end-to-end** on `scale_probe` at the demo's
  fidelity: `develop` **89.87 → 52.27 s (1.72×)**, `fold` **136.8 → 47.6 ms/pt (2.87×)**; the author
  suites fell 78.4 → 62.1 s, 16.8 → 12.1 s, 38.9 → 25.8 s. **Re-profiled**: `u128_div_rem` drops from
  **53% of samples to 13.8%**, and the profile is now *flat* — allocation (`malloc`/`free`/`Repr`
  drop+clone, ~20%) is comparable to division, and the `dashu` bignum path is relatively more
  prominent than `small`. The single dominant hotspot no longer exists, which means the next
  optimization needs its own profile rather than an extrapolation from this one.
  **Arithmetically invisible, as designed and as checked**: every pinned ε bit-identical (`develop`
  4.1481e-1, `fold` 1.3879e-1, `refold` 5.9982e-3, `solid` 5.7663e-2, flex 2.7573e-1/1.3663e-1),
  every chord golden unchanged (3.0/9.4/10.1%), every VV.1 work counter unchanged (2256 γ cells,
  4096 cut evals). That was the whole point of picking a value-preserving optimization: correctness
  evidence is exact equality, not a tolerance.
  **It shipped with a proof debt, now discharged** — see the OPT.3-proof entry above. Rust-side
  evidence remains a differential test against the Euclidean reference over ~80k pairs (powers of
  two, `u64`-boundary straddles, `2^127`) plus the bit-identical pins.
  *2026-08-14 · open (proof debt) · OPT.3 (#239)*

- **OPT.3 step 0/1 — the gcd operand mix is 85% powers of two, and ~6× is available for one
  standard identity.** *Harvested* (temporary counters in `gcd_u128`, one `scale_probe` run):
  **168 246 619 gcd calls** in 36 s — **84.7% have a power-of-two operand**, 76.5% have both
  operands under `2^64`, 17.6% are trivial, mean **11.98** Euclidean iterations, max width 127 bits.
  That is ~2 × 10⁹ `u128 %` operations, which is the 60% the profile attributed. The operand mix is
  not an accident: the kernel snaps coordinates to `2^-30` and `2^-50` dyadic grids everywhere, so
  denominators are powers of two by construction.
  *Benchmarked* (`benchmarks/gcd-hot-path`, 2M pairs matching the harvested mix, every candidate
  checked against the shipping implementation on 200k pairs): current **265.6 ns/call**; power-of-two
  fast path **63.7 ns (4.17×)**; + `u64`-narrowed Euclidean **43.8 ns (6.06×)**; strip-twos +
  `u64` **44.8 ns (5.93×)**. Since gcd-driven division is ~60% of runtime, ~6× on it predicts
  **≈2× end-to-end across every crate**.
  **Preferred shape: strip the common power of two, then Euclidean on the odd parts** —
  `gcd(2^i·m, 2^j·n) = 2^min(i,j)·gcd(m, n)`. It matches the branchy version's speed (within noise)
  but the power-of-two case stops being a special case: a power of two has odd part 1, so the
  Euclidean call returns immediately. One standard identity to state instead of a pile of branches.
  **Proof cost (step 1), priced rather than guessed:** `docs/proofs/ledger.md` puts `small::gcd_u128`
  under `GcdReduce.lean` and `SmallRat::reduce` under `Reduce.lean`, both ✅ axiom-clean. The gcd
  proof is **45 lines** and is structurally tied to the Euclidean loop — `loop.spec_decr_nat` with
  invariant `Nat.gcd st.1 st.2 = Nat.gcd a b` (discharged by `Nat.gcd_rec`) and measure `st.2.val`
  (discharged by `Nat.mod_lt`). Consequences: a **binary/Stein gcd would need an entirely new proof**
  (different measure, 2-factor bookkeeping) — which is why it is *not* the recommendation despite
  being the obvious textbook answer; whereas strip-twos keeps that loop verbatim and adds the one
  identity above plus the zero cases, and the `u64` narrowing is the same proof at a second width
  plus a cast. Tractable, bounded, and it does not reopen the previously-rejected binary-GCD
  question.
  **Correctness check available for free:** the change computes *identical rationals*, so every
  pinned ε must return bit-identical (VV.2) and every work counter unchanged (VV.1). Any movement at
  all is a bug, not a tradeoff. *2026-08-14 · open · OPT.3 (#239)*

- **60% of the kernel's runtime is 128-bit software division inside the i128 rational tier — and no
  amount of algorithmic work upstream would have found it.** Profiled (`sample`, macOS) on the new
  fold-heavy `scale_probe`, 43 757 top-of-stack samples: `compiler_builtins::…::u128_div_rem`
  **23 320 (53%)**, plus `__umodti3` 2 065, `__udivti3` 508, `__divti3` 250 — together **~60%**.
  The callers are not the bignum path: `lattice::small::div` 5 518, `::mul` 4 025, `::add` 3 378,
  `::sub` 207, `SmallRat::reduce` 100, against ~600 for all of `dashu` combined. The reason is
  visible in `small.rs`: every `add`/`sub` computes `i128_gcd(x.den, y.den)`, divides both
  denominators by it, and then calls `SmallRat::reduce`, which computes a **second** gcd — and
  `i128_gcd` is Euclidean, so each gcd is a chain of `u128 %`, which on ARM64 is a *software*
  routine (no hardware 128-bit divide).
  **Why this matters more than anything else on the list:** it is not specific to fold. Every
  certificate, every enclosure, every boolean, in every crate, pays this tax on every rational
  operation. It also explains why OPT.1 and MAP.1 returned so much less than their operation-count
  reductions suggested — they removed *operations*, but each surviving operation still pays a
  software-division tax that dominates it. And uniquely among the levers considered, **a faster gcd
  is semantically identical**: it computes the same rational, so ε does not move, no certificate
  changes, and no enclosure structure is touched. Compare the float filter, which changes what the
  enclosures *are*.
  **Candidate levers, cheapest first:** *(a)* a **u64 fast path** — the common operands are dyadic
  (`2^-30`, `2^-50` grids) and small integers, and ARM64 *does* have hardware 64-bit divide, so
  gcd-and-divide on values that fit in `u64` skips the software routine entirely; *(b)* **fewer
  reductions** — `add` currently reduces twice, once via the lcm trick and once in `reduce`;
  *(c)* **binary (Stein) gcd**, shifts and subtractions only, no division at all.
  **The catch, stated plainly:** this hot spot is in `lattice` — the **pure tier / TCB**, where Lean
  owns gcd/reduce correctness. Changing it is not free the way a shell-tier change is, and a
  binary-GCD was previously *rejected* — but that rejection was about using it as a bandage for a
  **verification** gap, not a response to a profile. The performance case is new evidence, and the
  decision should be re-taken on its own terms rather than assumed either way.
  *2026-08-14 · open · profile (follows MAP.1 #234); supersedes the float-filter-first
  recommendation in `docs/atlas-transform-design.md` §"levers"*

- **A "fast path" that mostly did not fire, and the counter that caught it (MAP.1).** The
  search/certificate split landed with every certificate green and every pinned ε *identical* —
  `develop` 4.1481e-1, `fold` 1.3879e-1, `refold` 5.9982e-3, `solid` 5.7663e-2, chord goldens
  unchanged. That proves nothing. A seeded bracket and a bisection compute the **same** certified
  answer; the bisection is merely slower. So identical ε is exactly what a silently-never-firing
  fast path also produces. The `bracket_seeded` / `bracket_bisected` counters said **seeded 2,
  bisected 6** — the path was taken a quarter of the time, and without the counter it would have
  shipped looking like a success.
  **Two bugs, both mine, both arithmetic rather than design.** *(1) The widening budget could not
  reach the root.* The window starts at `2^-36`·domain and quadruples at most 14 times, reaching
  only `2^-8`·domain — so any seed further off than 0.4% of the domain was unreachable no matter how
  correct it was. *(2) The seed was coarser than the window searching for it.* `from_f64` snapped to
  a `2^-30` grid (~1e-9), a hundred times the initial half-width of ~1.5e-11·domain, so the snap's
  own error placed the root outside the first window by construction. Fixed by a `2^-50` grid and 26
  attempts → **seeded 6, bisected 2**. A third case — a vertex sitting on a region seam, where
  `cross` straddles zero at the clamped endpoint and no *definite* sign exists — is legitimately
  relaxable: at a domain endpoint the caller's gore precondition already established the same
  one-sided fact, and the bisection starts from exactly that bracket under exactly that precondition.
  Accepting it there gives **seeded 7, bisected 1**, and is no weaker, because in both cases the
  certificate is the downstream round-trip residual and not the bracket.
  **Then the fixed version was a regression, which only the demo showed.** With 7/8 seeded, the
  acceptance demo came back **slower and less accurate**: fold 17.5 s → 26.1 s, fold ε 3.562e-2 →
  1.521e-1, refold 5.803e-4 → 1.158e-3. Cause: the seeded bracket was being *returned as the
  answer*. A widened window is a valid bracket but a **wide** one, and ε is set by the bracket's
  width — so every widening both cost an evaluation pair and multiplied ε by four. The unit-level
  hit-rate counter could not see this; only the end-to-end ε and timing could.
  **The fix is structural, and is the shape this should have had from the start:** the seed only
  ever *narrows the starting bracket*; the bisection still runs, to a **width target** derived from
  `iters` (the width it would have reached anyway). A good seed then removes almost every bisection
  step, and a bad or widened seed costs a little time and **never** accuracy. Result: pins equal or
  better — `develop`/`fold`/`solid` ε identical, and **refold improved to 4.3633e-3** from the
  5.9982e-3 baseline, because a seeded bracket starts narrower and the same budget converges further.
  **And then the premise itself turned out to be wrong.** Measured properly — 40 acceptance-outline
  vertices folded in **one** `fold` call, so region construction is paid once as the demo pays it —
  the split is worth **1.16×**: 158.0 ms/pt with the seed off, **136.8 ms/pt** at three widening
  attempts (69% hit rate, identical ε). Not the ~50× the design document projected from "≈50
  bisection steps become one evaluation". **The bisection is not the dominant cost of a fold point.**
  OPT.1 had already removed the γ cost that made it look dominant; what remains per point is the
  per-region trials, `directrix_on_iv`, `radius_on`, `lift_box`, and the round-trip `point_on` —
  and that last one *is* the certificate, so it cannot be optimized away at all.
  **Roadmap consequence, and it is the important part:** MAP.2's fitted map replaces the same
  search, so its speedup on the fold is bounded by the same ~1.2×. The certified fold has a **floor
  set by the residual certification**, not by the search. Getting the order of magnitude the product
  needs therefore requires making the *enclosure evaluations themselves* cheaper — the float-filter
  lever — not eliminating the search. MAP.2 remains worth building for the other three reasons (it
  is the ECAD artifact, it amortizes across the stackup, it is what an optimization loop re-certifies
  cheaply), but **not** as the fold's performance answer. `docs/atlas-transform-design.md` §4.2
  overstates that and should be corrected when MAP.2 is specified.
  **Three general lessons.** *(1)* An optimization that preserves outputs exactly is **unfalsifiable
  by its outputs** — when the fast and slow paths agree by construction, the only honest evidence
  the fast path exists is an instrument counting which one ran. *(2)* A fast path must be built so
  that failing is *only* slower, never worse: returning the search's own bracket made accuracy
  depend on how well the search happened to do, while routing it through the same convergence
  criterion makes the optimization unable to damage the certificate even when it misses. *(3)* The
  instrument must isolate what it claims to measure — the first probe folded point-by-point, so 95%
  of its 740 ms/pt was `build_regions` and the inversion signal was invisible.
  *2026-08-14 · MAP.1 (#234)*

- **The perf gate counts operations, not seconds (VV.1) — and it is proven to fire.** There was no
  performance regression detection for the geometry pipeline at all; the only benchmarks in the tree
  measure algebra backends, which is why a 10× slowdown survived a whole milestone and was found by
  accident. A committed wall-clock baseline would have been a flaky gate — it moves with machine
  speed and load, so it is either too loose to catch a real regression or it cries wolf — and the
  regression in question was a *complexity* change, `N × panels` where `N + cells` was available.
  So `develop::counters` counts **γ cells integrated** and **cut-certificate sub-interval
  evaluations**, thread-local (parallel tests cannot perturb each other) and always compiled in (a
  `Cell<u64>` bump is free beside exact-rational interval arithmetic). Measured on the acceptance
  device at `segments(16)`/`support_panels(8)`: **2 256 γ cells · 4 096 cut evaluations**, budgeted
  at 3 200 / 5 800. **Verified live by sabotage:** forcing every cache lookup to miss — exactly what
  deleting the memoization does — makes the sweep cost **512 = 32 × 16 cells**, the naive `N ×
  panels` shape on the nose, with no reuse on repeat, and drives `develop` to 4 160 γ cells, failing
  the budget with the message that names the cause. The second test asserts the property directly
  rather than inferring it from a total: *asking for γ again costs nothing*. Wall-clock is
  deliberately not asserted anywhere; the demo's `[time]` lines carry it for humans.
  *2026-08-14 · resolved · VV.1 (#229)*

- **The γ prefix table buys 2.7× overall and 14.8× on the fold — and shows the remaining cost is
  *not* γ (OPT.1).** `γ` is an integral, so it is additive; memoizing prefix sums over a grid
  anchored at the integration origin turns `N` queries × `panels` subintervals into `cells + N`.
  Measured on the acceptance demo at `segments=24`: **627 s → 235 s** (10:34 → 3:55 wall), with
  `develop` 163.1 → 89.9 s (1.8×), **`fold` 259.7 → 17.5 s (14.8×)**, `solid` 203.5 → 126.2 s (1.6×).
  The fold wins hugely because it ran at `GAMMA_PANELS = 64` *per bisection step*; develop and solid
  ran at 20 and spend much of their time elsewhere. **Certificates are preserved**: `develop`
  ε 2.687e-1 and refold 5.803e-4 bit-identical, `fold` ε 3.561e-2 → 3.562e-2, STEP still
  `cert=Verified occt=ok 148 faces 0 free`; on the test-tier parts `develop`/`solid` ε identical,
  `fold` 1.3878e-1 → 1.3879e-1, `refold` 5.9975e-3 → 5.9982e-3 (~0.01%). The ε pins from VV.2 are
  what make that a *measurement* rather than a hope.
  **The honest part: this does not restore the pre-p-curve ~1 min.** OPT.0 established that γ was the
  dominant *per-point* cost; it did not establish what fraction of each stage was γ, and now we know —
  γ was ~73 s of develop and ~77 s of solid, but nearly all of fold. The remaining 216 s is the
  p-curve node-count increase multiplying *non-γ* per-node work (rail fitting, the interval arithmetic
  in the cut certificates, the arrangement). That is a separate optimization, and the `develop::cut`
  unbounded-rounding item below is one concrete lead into it.
  *Design notes worth keeping:* the grid step is set by the **first** query as `(σ₁ − lo)/panels`, so
  the error at σ₁ equals the direct rule's exactly, farther queries get more cells (tighter) and
  nearer ones fewer (looser, but bounded by σ₁'s error) — since ε is a max over queried points, the
  worst case is preserved, which is what the measurements above confirm. Four origins are cached
  because `directrix_at` (origin 0) and `directrix_between` (region `lo`) interleave and a
  single-entry cache would thrash into being *slower* than no cache.
  **The bug worth remembering:** the first version always used the *last* prefix entry, so a query
  landing *below* a table built by an earlier farther query integrated backwards from a grid point
  past σ — `integrate_on_slope` correctly returned `None` and six tests failed on `unwrap`. The index
  must be searched for (largest grid point ≤ σ), not assumed to be the end.
  *2026-08-14 · resolved · OPT.1 (#232)*

- **`flex_part` — a part with a *derived* hole — had no `solid()` test at all (VV.3).** Building the
  golden metric turned up the coverage gap directly: `solid()` on the Stage-1 flex panel was only
  ever exercised by an *example*, never by a test, even though it carries a derived interior hole
  (D4). That is precisely the shape of the PC.4 regression, where `solid()` broke for every part
  with a derived hole and all 205 tests still passed — the flat path was covered and the solid path
  was not. Now closed, and asserted with `free_edges == 0` ∧ `nonmanifold_edges == 0` rather than a
  face count, because those are the two conditions the PC.5 defects actually violated (a zero-length
  edge at a collapsed tangent cap; a corner inheriting the wrong rail, giving an edge incidence 4).
  A face count would have passed through both. **The chord golden itself:** longest emitted edge as
  a fraction of the hole's own diameter, measured on the polylines `region_to_polys` hands the SVG —
  self-lapping holes **9.4%** and **10.1%**, flex D4 **3.0%**, against the **30–48%** the graph model
  produced on this very drill. Gated at 15%, deliberately a *structural* threshold (does a chord
  bridge the tangent rulings?) rather than an ε-style ratchet, since the metric scales as ~1/n and
  would otherwise be brittle to any resolution change. The gate is proved live by
  `the_chord_golden_rejects_a_bridged_hole`, which reconstructs the defect shape from a circle with
  a run of samples removed and checks the metric both scores it in the observed band and rejects it.
  *2026-08-14 · resolved · VV.3 (#231)*

- **The acceptance device certifies at 83% of its DRC ceiling — `develop` has no room to absorb a
  looser bound (VV.2).** Pinning the ε budget (`the_certified_bounds_stay_within_budget`) measured
  the self-lapping device at `segments(16)`/`support_panels(8)`: **develop 4.1481e-1 · fold
  1.3878e-1 · refold 5.9975e-3 · solid 5.7663e-2**, and the γ≡0 flex panel at **develop 2.7573e-1**.
  The DRC gate is `clearance/2 = 1/2`, so the self-lapping `develop` sits at **83%** of the value
  that would stop it certifying at all (the flex panel is at 55%). A 21% degradation turns the
  acceptance demo red — which is exactly what `segments(12)` already does (`Unresolved` at 5.737e-1,
  see the OPT.0 entry). **Consequence:** OPT.1's prefix-table change to γ *repartitions* `[0, σ]` and
  will move ε; if it moves `develop` the wrong way by even a fifth, the device stops certifying. So
  the budget is not bureaucracy here, it is the only thing standing between an optimization and a
  silently un-shippable part. Both parts are pinned so a moved bound can be *localized*: the flex
  panel has no flat directrix, so if it moves too, the cause is not the γ quadrature.
  *2026-08-14 · resolved · VV.2 (#230)*

- **The post-p-curve 10× slowdown is the γ quadrature re-run per point, not the p-curve subdivision
  (OPT.0 triage).** The demo went ~1 min → 10.5 min across the p-curve milestone, and the standing
  suspicion was the certificate's first-order bound forcing node count up. **Measured, and that is
  not where the time is.** One real drill-hole `quadric_cut_loop` at `n=12` costs **9.5 s**, and the
  demo runs ~4 of them (2 holes × flat `n=12`, solid `n=16`) — ~45 s of a 627 s run, ~7%. The stage
  split at `segments=24`: **`develop` 163.1 s (26%) · `fold` 259.7 s (41%) · `solid` 203.5 s (33%) ·
  `write_step` 0.8 s · svg 0.0 s** — i.e. spread across all three geometry stages, *not* concentrated
  in the flat side as previously assumed (flat = develop+fold = 67%, solid = 33%). The real driver: a single
  `ConeDevelopment::point` costs **1.34 ms** at `γ ≡ 0` but **111 ms** at `γ ≠ 0` with the demo's
  `support_panels(20)` — **83×** — and scales *linearly* in `panels` (4.0× measured for 4× panels).
  `directrix_at`/`directrix_between` call `integrate_on_slope` over all `panels` subintervals **from
  scratch on every query**, twice (x and y), with no prefix table or memoization — and `fold.rs:29`
  already says so in its own doc comment ("each `invert_sigma` bisection step re-integrates `γ(σ)`
  from 0"), at `GAMMA_PANELS = 64`, so the fold pays it once *per bisection step*. Clean in-run
  confirmation: fold rings 1 and 2 are the two drill holes with **identical 24-point counts** but cost
  **15.3 s vs 64.7 s** — a 4.2× spread whose only variable is whether the hole sits in the γ≡0 body or
  the γ≠0 lap. So the cost is
  `(#γ≠0 point evaluations) × 2 × panels × (velocity+accel enclosure)`, and what the p-curve
  milestone changed was the *first* factor: a hole went from ~4 boundary arcs to ~4n ≈ 48. **The 10×
  is a node-count increase multiplying an already-quadratic-in-disguise per-node cost.** Consequence
  for planning: the fix is **bounded** — γ is an integral, so it is additive; accumulating a prefix
  table over a shared panel grid turns `N×P` into `N+P` (interval addition of adjacent panel
  enclosures is the *same* quadrature, so the certificate is untouched). It is **not** the
  certificate redesign the first-order bound would have implied. **But it will move ε**: a prefix
  table answers a query as `prefix[k] + partial panel`, a *different* partition of `[0, σ]` than the
  `panels`-uniform one, so the enclosure shifts (tighter or looser, must be measured). That is the
  concrete reason to land VV.2 (ε pinning, #230) **before** OPT.1 rather than after — without it the
  change is invisible, since a 10×-worse ε still certifies `Verified` under the clearance.
  *Second measurement, worth keeping:*
  at `segments=12` `develop` returns **Unresolved at ε 5.737e-1** against **2.687e-1** at 24 — ratio
  2.13 for 2× segments, i.e. first-order convergence confirmed at the top level, and the acceptance
  demo sits close to its DRC margin. *2026-08-14 · open · OPT.0 (#228), fix tracked as OPT.1 (#232)*

- **`develop::cut`'s p-curve hot path never applies the outward rounding `interval.rs` exists to
  provide.** `cut.rs` contains **zero** `.rounded()` calls, against 33 in `cone.rs` and 16 in
  `interval.rs`; `eval_poly_on` is a bare interval Horner and `chart_point_on`/`surface_distance_on`
  chain exact-rational interval ops with no budget. With degree-24 field denominators over 2⁻³⁰-grid
  endpoints, an evaluation reaches ~720-bit denominators before the three fields even combine.
  `ROUND_BITS = 60` (DEV.2a) is documented as the mechanism that bounds exactly this growth. Not
  currently the dominant cost (the cut loops are ~7% of the demo), so it is a *secondary* OPT.1
  candidate rather than the headline — but it is a real unbounded-growth path in a hot certified
  routine, and rounding outward is sound by construction (a wider enclosure is still an enclosure).
  *2026-08-14 · open · OPT.0 (#228)*

- **Interior holes are shaped by a representation choice, not a fit-quality limit — the p-curve
  milestone (PC.0 GO-gate).** The device's drill holes export as two cubic rails sewn by two
  straight chords. Measured on the emitted flat pattern, each hole's two longest edges are ~0.14
  against a 0.46 hole diameter — **31% flats**, 2.3× longer than any other edge — and in the STEP
  solid those chords are literal straight lines between two Bézier rails. Root cause: the trim
  layer represents a cut as a **graph** `µ̂ = f(σ)`. A closed cut turns around in σ at the two
  tangent rulings (where the cutter grazes the sheet and `dµ̂/dσ` blows up), which no graph can
  represent, so the loop is split into near/far graphs that must stop short of the turning points,
  and the gap is bridged straight. The gap closes only as **√inset**, so it is stubborn.
  **Spike (three strategies, on the real drill; window width 0.053, hole height 0.291):** *(S1)*
  the current fitted rail — the best rung over the whole margin×subdiv ladder is a **30% cap at
  ε 0.257** (margin 1/200, subdiv ×16); the rung that certifies in the demo gives **48%**. There
  is no rung with a small cap: shrinking the inset to fix the shape makes the fit diverge, which
  is exactly why the ladder escalated the inset *up* and capped degree at 3 — it traded the hole's
  shape to buy certification. *(S2)* graded pieces marching toward the tangent, reusing the
  existing fit — **ε 1.6e2 … 1.9e9**, catastrophic: a cubic in σ over a 1e-5-wide window at
  σ ≈ −0.94 is Vandermonde-hopeless, so the monomial basis blocks the piecewise route until #220
  lands. *(S3)* the **exact algebraic branch** — the cut is `a(σ)µ̂² + b(σ)µ̂ + c(σ) = 0`, so the
  boundary is `µ̂ = m(σ) ± √H(σ)` with `m = −b/2a` and `H = (b²−4ac)/4a²` *exact rational
  functions*; with no fit in the way the cap follows √inset with no floor (4.5% at 1/2000, below
  f64 resolution at 1/200000). **Decision: represent domain cut curves as p-curves `(σ(t), µ̂(t))`,
  not graphs.** This is not hole-specific — `Cutter::Extrude` (PR 4) puts turning points on the
  *outer* boundary too, so general intersections need the machinery regardless. Cheaper than it
  looks: the deepest certified layer is **already parametric** — `AnchorDevCert` has always carried
  `sigma`/`mu` as functions of a parameter `t`, and the unroll merely instantiates it with the
  identity reparametrization. The graph assumption is bolted on above it in four places
  (`BoundaryArc::Rail`, `HoleRail`'s near/far band, `cut_fit`, and the solid builder's
  single-slice restriction on polygon holes). History note: the deleted demo's `drill_hole` was
  the exact-branch construction — no margin, no fit, tangent vertices exactly at `disc = 0`, no
  caps, refold defect 1.4e-6 against today's 1.8e-3 — but float-sampled and uncertified, which is
  why the facade replaced it. The milestone's point is to have both. *2026-08-14 · PC.0 GO ·
  branch `pcurve` · tasks #221–#227*

- **Making the hole faithful broke the B-rep: a collapsed tangent cap is a zero-length edge
  (PC.5).** The graph model bridged each tangent ruling with a straight chord ~30–48% of the hole
  across; the p-curve loop meets its tangent at a *single point*, both branches evaluating to the
  midline there. The solid builder still emitted that σ-cap as an edge, so it asked OCCT to build
  a **zero-length line** — `MakeEdge(line) failed`, and the STEP certificate came back `REFUTED`
  (the gate working exactly as intended: the shell was refused, not shipped). The lesson is worth
  keeping: *the more faithful the hole gets, the more certainly this fires* — a defect that hides
  behind a coarse approximation and appears when the approximation improves. Fixed by collapsing
  corner pairs that map to the same `(σ, µ̂)` point before lifting, which is also the honest
  topology — the loop really does have one vertex there, not two. **Also measured (PC.5):** hole
  chain-piece boundaries become σ-stations, so hole segment count now drives the solid's face
  count directly (48 segments → ~770 faces on the doctest panel against 28 before; the solid takes
  a loop clamped to 16 → ~256). Build time rose sharply with it, and the cause was **not** the face
  count: the slice loop re-`reduce()`d the chart's surface fields *per slice*, and `reduce()` is a
  polynomial gcd over degree-24 denominators, so the cost scaled with hole fidelity through the
  station count. Hoisting the reduction to once per region (regions are few; slices are now many)
  cut the doctest panel's solid from ~21s to 8.2s. The general lesson for this builder: anything
  expensive that depends only on the *region* must not sit inside the *slice* loop, because the
  slice count is now driven by authored fidelity rather than by the chart.
  **The sliver-slice defect and its fix.** Making every chain-piece boundary a σ-station worked but
  was the wrong coupling: √-graded nodes sit ~1e-4 apart in σ, so the whole panel inherited sliver
  slices, OCCT rejected the reloaded shape's `BRepCheck`, and the build took 10.5 min at 516 faces.
  Resolving a hole and partitioning a panel are different concerns. The partition went back to what
  it was, and the footprint now emits a corner at each chain-piece boundary along a hole's rail
  runs (each carrying the piece covering the span ahead, which is the rail `lift_trim_edge` already
  uses) — so a hole's fidelity buys hole *edges* and nothing else. Result: **`occt=ok`**, faces
  516 → 148, and the per-slice hole projection deleted (net −36 lines).
  **A collapsed corner must inherit the OUTGOING rail, not the incoming one.** With `occt=ok` the
  shell was still refused by our own certificate: 0 free edges but **4 non-manifold edges**, 2 per
  hole, each with incidence *4* while listed by only *3* faces — one face's wire traversing an edge
  **twice**, a spike. Cause: a corner's rail is the rail of the edge *leaving* it, and the dedup
  kept the first of each coincident pair. At a hole's tangent the far run ends and the near run
  begins at the same point, so the survivor carried the **far** rail while the next edge was meant
  to follow the **near** branch back — giving that edge far-rail geometry over the same σ-span as
  the real far edge. Identical geometry, so the builder's edge dedup merged the two into one and
  the wire walked it twice. Keeping the outgoing rail on collapse fixes it: non-manifold edges
  **4 → 0**, the shell is a closed 2-manifold. The general rule: when merging coincident corners,
  position comes from either but the rail must come from the *later* one.
  **Device green (2026-08-14):** `cert=Verified  occt=ok  (148 faces, 0 free)`. Note the shape of
  this milestone's bug tail — every one of the three solid-path defects was *caused by the geometry
  getting better*, and each had been invisible while a hole was two graphs bridged by a 30–48%
  chord: a zero-length edge where the cap used to be, sliver slices from tying hole resolution to
  panel partitioning, and a wrong-branch rail at the collapsed corner. Coarse approximations hide
  degeneracies; improving them is what exposes the assumptions built on top.
  **Remaining cost:** the device's demo takes ~10.5 min wall clock (against ~1 min before the
  milestone) — the flat pattern's fine hole loops, not the solid, now dominate. Untriaged; the
  γ-quadrature per-edge item is the standing suspect. *2026-08-14 · PC.5 · branch `pcurve`*

- **Interior cuts are p-curve loops on the flat path — measured (PC.4).** `surface_hole_loop` no
  longer fits two graphs and bridges them; it returns the closed p-curve loop of
  `quadric_cut_loop`, and `unroll` grew a `BoundaryArc::Curve` whose chords are certified by the
  *same* lift bound (which was always parametric — only the identity reparametrization was
  hard-coded). On the acceptance demo, measured on the emitted flat pattern: each drill hole went
  from 27 vertices with two anomalous 0.14 edges — **31% of the hole**, 2.3× any other edge — to
  49 vertices whose longest edge is 0.029, i.e. **7%, and no longer an outlier at all** but simply
  the uniform chord spacing of a smooth loop. Develop ε fell 3.688e-1 → **2.687e-1** (the hole
  ladder's 0.33 contribution is gone; the boundary rails now dominate) and the refold defect
  1.779e-3 → **5.803e-4**. On the offset-support ramp fixture the hole certifies at ε 9.1e-3 with
  a tangent gap of 2.7e-5. **Known limitation, pinned by a test rather than left to be
  discovered:** the solid builder consumes interior holes as either a near/far `HoleRail` band —
  which cannot express a curve that turns around in σ — or an exact `(σ, µ̂)` polygon, which it
  requires to sit inside one σ-slice. Derived holes are drilled as polygons now, so a hole
  straddling a station is refused with a typed fault until per-slice clipping lands (PC.5); the
  flat pattern, which is the manufacturing artifact, is unaffected and already carries the good
  geometry. Worth recording how that was caught: the whole 205-test suite passed after the switch
  because **no test exercised `solid()` with a derived hole expecting success** — the gap was
  found by probing the path deliberately, not by the gate. *2026-08-14 · PC.4 · branch `pcurve`*

- **The p-curve cut certificate must enclose in the domain, not compose into the parameter — and
  its bound is first-order (PC.2/PC.3).** The tidy way to state "this curve traces the surface" is
  to compose the chart fields into the curve's parameter and enclose the resulting residual, as
  the graph checker does in σ. On the device's wrapping chart that is **numerically ruinous**:
  substituting an affine `σ(t)` into a degree-24 field denominator produces monomial coefficients
  around 10²⁰⁰ whose true value is ~10², and interval evaluation of that cancellation straddles
  zero — so a *pole* is reported at every single node of a perfectly regular hole. The fix is to
  enclose `(σ, µ̂)` over the parameter sub-interval and then evaluate the chart's own fields at the
  enclosed σ, keeping every polynomial in its own well-scaled variable. The price is the lost
  µ̂↔σ correlation across a piece, which makes the bound **first-order** in the subdivision
  (measured on an exact plane rail: ε 3.7e-1 → 4.0e-2 → 4.9e-3 for 8× steps) where the symbolic
  graph residual is exact. Consequences, all deliberate: graph rails keep using `cut_fit`
  (unchanged, still ε ≈ 0); the p-curve path is for curves the graph cannot express at all; and
  composition stays, tested and exact, for the *export* lift, where a p-curve's 3-D image being
  rational in `t` is what gives an exact Bézier. **Tightening is a known follow-up** — a
  mean-value/centred form would restore second order. To be precise about what is and is not
  avoidable (user, 2026-08-14): *subdivision itself is not avoidable* — certifying a curve of any
  real complexity means resolving it, and no formulation escapes that. What the formulation does
  decide is the **order**: at first order, halving ε costs twice the work forever; at second, four
  times the resolution per halving. That is the difference between a boundary that certifies at a
  few hundred sub-intervals and one that needs tens of thousands, and it is the reason this is
  tracked rather than shrugged off. It becomes load-bearing the moment p-curves carry *outer*
  boundaries (`Cutter::Extrude`), where a single curve spans the whole part instead of a hole's
  short pieces. A second lesson from the same stage: curve
  vertices derived from surds and 60-step bisected roots carry thousand-digit numerators, and the
  residual polynomials built from them stop being evaluable at all; every emitted coordinate is
  snapped to a 2⁻³⁰ grid and the accumulated ε rounded *up* onto it (sound — a larger upper bound
  is still an upper bound), which is what keeps the certificate both small and honest.
  *2026-08-14 · resolved (PC.2/PC.3) · branch `pcurve`*

- **A root landing exactly on a scan node was invisible to the rational root scanner.**
  `scan_roots` only registered a *sign change* between adjacent nodes, and required both signs
  non-zero — so a root sitting precisely on a node was skipped twice (each flanking cell has a
  zero endpoint, so neither reads as a change). Symmetric geometry produces this routinely: a
  curve turning at `t = 0`, a hole centred on a ruling, any dyadic scan over a symmetric span.
  Found by the p-curve core's own turning-point test (the unit circle's turn at `t = 0` reported
  zero turns). Fixed by taking an exact zero at a node as a root as it stands. The primitive moved
  down to `develop::pcurve` (the curve core needs it to locate turning points and station
  crossings) and `export::trim` now re-exports it instead of keeping a second copy — the copy was
  how the two drifted. Full workspace re-run after the fix: no behaviour change anywhere else.
  *2026-08-14 · resolved (PC.1) · branch `pcurve`*

- **Construction-API PR 3 — the piecewise/side fold, and what the self-lapping rewrite flushed
  out.** `develop::fold::{fold_point_pw, fold_outline_pw}` invert the *signed* connected
  development `D = Γ(σ) + µ̂·ρ·e(ψ)` per region in its running frame, and `author` grew
  `Part::fold` (µ̂-side **derived** from the resolution — seam #3 closed) + `Part::hole_flat`
  (ECAD 2-D cutouts: cut as-is in the flat boolean, folded back and drilled at solid time). The
  918-line hand-wired self-lapping demo is now a ~75-line recipe (`author/examples/
  self_lapping_cone.rs`) whose structure is derived — including the seam drill: **one cutter, two
  holes** (one per disc-positive window, head + lapping tail flap). Three findings the rewrite
  forced, each a general-engine fix: **(1) a fixed 3-D witness cannot resolve a wrapping window
  alone** — past ~half a turn the cone's *mirror nappe* comes closer to the witness point than the
  kept sheet that has rotated away, so the per-sample nearest pick flips mid-domain (observed as
  branch-flipped runs at `4·arctan σ ≈ ±150°` and a 24-face "outline"). The resolver now **seeds**
  the choice at the sample with the widest distance margin (where the witness actually lies) and
  **propagates by continuity** — σ-adjacent kept µ̂-intervals of one connected face must overlap —
  with the witness re-deciding only where overlap is inconclusive; hole-gaps are now counted only
  inside the *chosen* component (the drill also gaps the unchosen sheet). **(2) the σ=0 split is
  not enough for a wrapping fold** — on a `c ≥ 2` chart even a one-sided domain sweeps past π,
  where the signed-area bisection loses sign-faithfulness; `faithful_pieces` splits each region
  band until every piece's *certified* ψ-span clears a rational lower bound of π, and every
  region/piece is tried with the smallest round-trip ε winning (sound regardless of how a
  candidate was found — the round-trip *is* the certificate). **(3) hole loops need an
  escalation ladder, not one fit profile** — the wrap drill's window is a narrow off-origin span
  where degree ≥ 4 monomial fits are Vandermonde-catastrophic (ε 10⁴–10⁷) and a 1/200 tangent
  inset leaves the `∂s/∂µ̂ → 0` region inside the fit span (ε floor ≈ 1.2 at any subdiv);
  `certify_holes` now caps hole fits at degree 3 (the G2 narrow-span rule) and escalates inset
  (1/200 → 1/20) then subdivision (×1/×4/×16 from the user's knob) until a rung certifies
  (wrap head window: Verified ε ≈ 0.33) — fail-closed with the tightest ε reached. The #220
  Chebyshev/Bernstein-basis item would remove the degree cap wholesale. Also learned cheaply: flat
  holes are pairwise-disjoint operands of the exact Diff (an overlapping `hole_flat` XORs an
  island face and the coherence gate refuses it), and a drill's *flat* position is set by the σ
  whose ruling pierces it (the device drill develops around ψ ≈ 0), not by its 3-D azimuth.
  **Numbers are not comparable to the old demo's:** its headline flat ε ≈ 7.9e-3 / refold 1.4e-6
  were *pointwise/f64 emission* quality (the PR 1 "two rigor levels" finding); the facade's
  develop ε ≈ 0.37 / fold ε ≈ 0.2 / refold ≈ 1.4e-2 at the default budget are **certified
  chord/round-trip bounds** (dominated by the hole-ladder ε ≈ 0.33 and the 20-panel γ
  quadrature) — refine with `segments`/`support_panels`/`budget`, not by reading them as a
  regression.

- **Construction-API PR 3 pre-merge review sweep — four fail-open paths closed.** A two-reviewer
  adversarial pass over the PR 3 diff before merge confirmed four gaps, all fixed fail-closed on
  the branch: **(1) the pw fold's chart pairing was only length-checked** — the round-trip
  certificate lives entirely on the flat side, so a same-length mis-ordered chart slice lifted a
  `Verified` fold onto the wrong surface; `fold_point_pw`/`fold_outline_pw` now require each
  chart to *re-derive* its region's exact flat data (`ConeDevelopment::derives_from`: `c`, `ρ²`,
  directrix dot products — value equality, refused as `ChartMismatch`; necessary-not-sufficient:
  a pedal component along the surface normal is flat-invisible by construction). **(2) the
  development is genuinely 2-to-1 past a 360° flat sector** — on a flat span > 2π (c > 2 with a
  wide band; the current device's 275° sector is safe) a lap-wedge point has two *exact*
  σ-preimages, both round-tripping, and the min-ε pick between them was arbitrary noise
  comparison; the fold now collects every DRC-passing candidate and refuses σ-disjoint pairs as
  `AmbiguousPreimage` (new `FoldFault`/`PartFault` variants; per-vertex in the wire path, which
  also killed the permissive-clearance hack — the inner fold returns raw ε now). **(3) `solid()`
  drilled authored polygon holes with no coherence gate** — the builder's checks are slice-local
  (σ-extent only), so an overlapping/out-of-band `hole_flat`/`hole_domain` polygon that
  `develop()` refuses as `TopologyMismatch` sewed a self-intersecting `Verified` shell; `solid()`
  now runs the same exact flat boolean before building whenever authored polygons are present
  (cost: one extra flat evaluation on hole-bearing parts — acceptable until a shared-evaluation
  cache), plus a defense-in-depth `len < 3` polygon guard in `solid_brep` (an empty `hole_domain`
  used to *panic* in the builder's vertex indexing). **(4) the resolver's 0-overlap junction
  fallback re-trusted the raw witness metric** — exactly the mirror-nappe hazard the seeded
  propagation exists to avoid (buildable via a support discontinuity or a cutter outrunning the
  sample grid); a no-overlap junction now faults `AmbiguousRegion` instead of witness-re-deciding
  (≥ 2 overlapping candidates keep the witness tiebreak — those all continue the material).
  **Logged, deliberately not fixed here:** the hole ladder's inset escalation trades tangent
  micro-cap size (`HoleLoop::max_microcap`, µ̂ units) that is *not* folded into ε — the flat-length
  bound needs a certified ρ conversion and belongs with the #220 fit-basis rework (folding it in
  naïvely could flip the demo's rung); the `snap30` of folded hole vertices leaves the certified
  enclosure by ≤ ~2⁻³⁰ per coordinate (× the chart metric) without an ε term — same #220 bucket,
  practically ~1e-9 against clearance 1; and a caller-facing σ-window for `fold` (restrict to a
  lap deliberately) is PR 4 API material. *2026-08-14 · branch `construction-api`.*

- **Construction-API PR 2 — the `author` facade, and what deriving roles flushed out.** The new
  `author` crate evaluates declarative `Part` recipes (regions + solid-cutter material ops) by an
  in-domain resolution sweep (float mechanism, conclusive-or-fault) + certified realization, gated by
  a **topology-coherence check**: the exact flat boolean must reproduce the resolved structure, so a
  mis-resolution is refused, never shipped. Three findings from making roles *derived* instead of
  authored: **(1) the antipodal sheet is real** — on the full 296° gore the four demo solids
  genuinely keep material on *both* sheets of the cone (the antipodal ray crosses the disks too);
  the old pipeline never saw it because `RootPick::Upper` hard-picked the branch per call. The facade
  surfaces it as `AmbiguousRegion`, resolved by an exact `keep_near` witness (the D2 doctrine working
  as designed), with the chart's **singular rail** (`det J = 0` at `w=0`, `µ̂ₛ = −(c′·n′)/(r′·n′)`,
  exact rational) guarding hole-merges across sheets. **(2) Cutter windows, not tangent pairs** — a
  wide gore meets a solid cylinder along *several* disc-positive σ-windows (one per ruling sheet), so
  the 2-root `surface_tangents` shape is a special case; rail-fit spans clamp to the window
  *containing* their usage (fitting past a window's √-branch ends makes the oracle rightly decline),
  and hole extents are per-window — which is also the shape the self-lapping seam drill (one cutter,
  two pierces) needs in PR 3. **(3) γ≠0 unroll cost is per-edge** — chord-certifying a piecewise
  outline runs the verified γ quadrature per rail edge (`gamma_at` + the checker's from-`lo`
  integrals); memoizing `PiecewiseDevelopment`'s cumulative-γ prefixes (a `RefCell` cache keyed by
  the budget) removed the dominant re-integration, but the per-edge own-window integrals remain
  O(edges × panels) — logged under Tech debt for the perf pass (adaptive panel counts by span width,
  or an incremental frame walk). Deliberately deferred from PR 2: the `flat_tolerance`/
  `step_tolerance` product knobs (no principled mapping to `DevConfig`/`RailFit` yet — the exact
  `budget`/`fit` hatches shipped instead), the `keep_hit` ray pick, and `hole_flat` (needs the PR 3
  fold extension). The old 415-line `flex_panel` demo is now ~60 recipe lines in
  `author/examples/flex_panel.rs` (same geometry; Verified at ε ≈ 0.488 vs the old demo's documented
  ≈ 0.49), corroborated against the legacy `outer_loop` pipeline by exact-polygon shoelace area in
  the integration suite.

- **Construction-API PR 1 — the piecewise chord-certificate gap (found + closed) and `develop::part`.**
  **The finding:** the codebase had **two rigor levels masquerading as one** — `flex_panel`'s
  `unroll_trim_loop` certifies every rail edge's *chord-lift* via `anchor_dev` (the DEV.2c checker),
  but the self-lapping demo's outline ε is **pointwise only** (`FlatBox::backward_error` per sample,
  maxed) + the float isometry corroboration, because `AnchorDevCert` develops via `point_on`, which
  integrates γ **from 0** — and a piecewise region's support blows up outside its own σ-window, so
  γ≠0/piecewise rails simply could not be chord-certified. The demo dodged the gap rather than closing
  it. **Closed (user call: extend the anchor now, not pointwise parity):**
  `ConeDevelopment::point_from_on` (the interval sibling of `point_from` — from-`lo` γ on a running
  `base`, signed µ̂) + `AnchorDevCert.frame: Option<AnchorFrame{base, lo}>` (`None` = the original
  path, byte-identical; tests: a zero frame reproduces the frameless γ-anchor ε **exactly**, and an
  integer base-shift with an equally shifted target leaves ε unchanged — translation equivariance,
  the gluing's soundness core). Shell-tier, no Kani/Lean TCB. **And the extraction:** `develop::part`
  — the `Development` trait (one pipeline, many implementors; the µ̂-sign convention is an
  *implementor property*: the cone keeps its `|µ̂|` fast path, the gluing is always signed — the two
  must never mix on one connected boundary) + `PiecewiseDevelopment` (the demo's
  `gamma_grid`/`point_at`/`gamma_at` lifted: tiling + shared-frame validation, cumulative γ,
  region-routed `point`/`point_on`, and `anchor_pieces` — a span split at region joins, each piece
  carrying its running frame, so piecewise rails get the **same chord certificate** as single-region
  ones). Empirical bonus: `cone()` and `cone_seam_ramp()` share a frame (validated by `new`), the
  wrap chart correctly refuses. **Commit 3** generalized `unroll_{freeboundary,trim_loop}` to
  `&impl Development` (`rail_edge_eps` anchors piece-by-piece; single-region = the original cert
  byte-identical) and landed the **first chord-certified piecewise flat outline** (a rail edge
  straddling the region join gets its ε from two framed anchor pieces). **Bonus robustness catch**
  (bisected by a probe test): `anchor_dev`'s σ(t) image is *outward-rounded*, so at a **non-dyadic**
  window start (7/24) the enclosure's `lo()` dipped below the region's `lo` →
  `directrix_between(lo, σ<lo)` → `None` → a spurious `PoleInEval`; dyadic starts (1/4, 3/8) masked
  it, and the same latent bug existed in the old `directrix_on` path near σ=0. Fixed by clamping the
  γ base point to the window start (sound — true rail points never lie below `lo`; the enclosure
  covers `σ ∩ [lo, ∞)`). Full gate: develop 103 lib + 16 doctests, export 38+2+8, clippy 0, fmt,
  `xtask lint`. *2026-08-14 · **PR 1 COMPLETE (commits 1–3)** · branch `construction-api`*

- **The γ-integrator (task #216) — a verified midpoint-slope quadrature (O(h²)), replacing the O(1/N) directrix Riemann sum; and the quintic-ramp demo wall it revealed.** DD.2's `develop::interval::integrate_on` was a first-order interval Riemann sum (γ-enclosure width `∝ 1/panels`); its looseness had forced two compromises (see the self-lapping / DD.2 entries): a **cubic** (C¹) ramp instead of a **quintic smootherstep** (C²), and generally loose γ. **Built** `develop::interval::integrate_on_slope`: a **verified midpoint rule with a first-derivative (slope) remainder** — per panel `[a,b]` (`h=b−a`, midpoint `m`), `∫f = f(m)·h + R` with `R ∈ [−(h²/8)·w, (h²/8)·w]`, `w = width(f′(panel))` (rigorous from the MVT: `f(s)−f(m) ∈ f′([a,b])·(s−m)`, and `∫(s−m)ds = 0` cancels the linear term). Two wins: (1) **O(h²)** convergence — exactly 4× tighter per panel-doubling; (2) the main term reads `f` at a **thin midpoint**, so the interval-*dependency* blowup that makes big-coefficient integrands loose never enters (a spike test integrates `200s⁴−400s³+210s²−10s` ≥20× tighter than Riemann; `∫cos` ≥50× tighter at 64 panels). **`γ″` is supplied, not auto-diffed:** `γ″` shares `γ′`'s rotating-frame form `A·e(ψ)+B·e⊥(ψ)` with `A=a′−bψ′`, `B=aψ′+b′`, `ψ′=c/(1+σ²)` exact, and `a′,b′` from the exact `RatFunc::derivative()` of `cr,cn,ρ²` — so `ConeDevelopment::directrix_accel` reuses the existing enclosure primitives, **no AD layer, no new Kani/Lean TCB** (shell-tier interval arithmetic, like `cut_fit`/`anchor`). **Soundness matters here** — a `γ″` that under-bounds the true derivative would make the enclosure *unsound*, not merely loose — so it is guarded two ways: a float-oracle test (central finite-difference of the analytic `γ′` lands inside the `directrix_accel` box) and the preserved §Tier C isometry check. **Measured** on the ramp flap (`cone_seam_ramp`, mid-ramp): γ-width **9.3e-5 at 16 panels → 5.8e-6 at 64 → 9.1e-8 at 512**, perfect 4× steps — i.e. **16 panels now beat the old rule's 1024** (1.45e-3→9.0e-5 linear), ~64× less work. **The quintic-ramp wall (honest negative result):** restoring the C² quintic smootherstep in `self_lapping_cone.rs` — which the quadrature now develops fine — **fails at the trim rail, not the integrator**: region 1's D1-outer cylinder-cut fit went to ε≈14.7, and *raising* the fit degree 4→8 made it (**and the unchanged `h≡0` body region**, 6e-10→1.7e-2) worse — the **Runge signature of a geometry/branch wall**, not conditioning. The quintic reshapes the ramp surface enough that the fixed outer/inner trim cylinders (tuned for the cubic) no longer cut a smooth low-degree rail; restoring C² needs re-tuning the trim-cylinder placement — a **demo-geometry follow-up**, tracked, out of the integrator's scope. The demo keeps the cubic and stays certified end-to-end. Full gate green (fmt, clippy default+diagnostics+step, develop lib 92 + doctests 14, export lib 38, demo default 3 + `--features diagnostics,step` 4, `xtask lint`). *2026-08-12 · **task #216 met — O(h²) verified quadrature; quintic-demo restore is a deferred trim-geometry item** · branch `roadmap-flex-pcb`*

- **The self-lapping cone as ONE connected chart (no atlas) — and five cone-shortcuts it exposed.** The prior DD.4 shipped the self-lapping device as **two solids + a bond** (the "lap = doubled material" doctrine); the user rejected that three times — the real intent is **one connected body + one development chart** (cut it from a sheet, fold it into the shape). Built on branch `self-lapping-wrap` (off pushed `39a2874`): `crates/export/examples/self_lapping_cone.rs`, driven by a new device fixture `fixtures::devices::cone_wrap` and three new general `develop::cone` primitives. **The keystone insight (the user's):** the seam-at-σ=∞ stall that forced an atlas is an artifact of the *degree-1* arctan cone `q=(9,4,4σ,9σ)` (one 2π turn over σ∈ℝ, seam stranded at σ=±∞ where `|n′|→0`). A **degree-2** quaternion **in the same 2-plane** — `cone_wrap: q=(9−9σ², 4−4σ², 8σ, 18σ) = (1−σ²)q_a+2σq_b` — Hopf-maps to the *same* circle (`n·ẑ≡65/97`) traversed **twice** (`φ=4·arctan σ`), so one turn-plus-lap fits the finite window `σ∈[−5/4,5/4]`, seam at the regular `σ=±1`. **ψ stays closed-form** (`ψ′·(1+σ²)=260/97` exactly → `ψ=(260/97)·arctan σ`, = 2× the degree-1 coeff), so `cone_angle_coeff` accepts it verbatim — no ∫ψ′ quadrature needed (the plan overestimated this). **The build:** piecewise support on the one frame (body `h≡0` true cone | cubic §8 smoothstep `0→D` | plateau `h≡D`), developed via a **cumulative γ-grid** (`ρ,ψ` support-independent, `γ` the running integral) → ONE connected ~275° annular-sector SVG (flat ε≈7.9e-3) that **folds back isometrically** (flat-chord vs 3-D-chord defect **4.1e-5**). Boundaries by real intersection; interior holes = two round seam-drills (one vertical cylinder through the lap, piercing head+tail — refold defect **1.4e-6**, so the two flat holes land on the *same* drill cylinder) + a 2-D hexagon folded back. Emitted as **one watertight STEP solid** (new `export::brep_build::brep_trim_solid_regions` — piecewise-chart shell, region joins share cross-rings so no internal cap; **faces=116, free=0, nonmanifold=0, closed=true, valid=true**). **Five cone-shortcuts ripped out (each a hidden apex/cone assumption that broke the offset-tail or the wrap):** (1) the σ=∞ stall (degree-1 chart) → wrapping `q`; (2) `point`'s `|µ̂|` gore path flips a µ̂<0 gore across the apex vs the signed `γ+µ̂ρe(ψ)` — mixing them along a connected boundary breaks it → new `ConeDevelopment::{point_signed, point_from, directrix_between}` (the canonical signed development + sub-range γ for piecewise gluing); (3) `concentric_disk`'s `{z=d}` **plane** cut is a circle only *on a cone* — it spirals the offset tail's outer edge → a real concentric **cylinder** cut; (4) `hole_loop`'s `tangent_poly` is the **apex-ray** tangency — wrong on the offset tail, 4-rooted under the wrap → drill the hole directly from the real surface point `c+µ̂r` (quadratic in µ̂, σ-extent from `disc=0`); (5) `HoleRail` is a near/far µ̂-**band** — pill-shaped round holes, no polygons → `brep_trim_solid_regions` now cuts general `(σ,µ̂)` **polygon holes** (each edge the line through its corners; the lift machinery already supported it, the 4-corner band was the restriction). **Honesty items (docs):** the D2 eccentric-inner **ramp fit ε≈0.15** is the loosest (degree-4 surd fit on the narrow ramp band); the ramp is C¹ **cubic** smoothstep (a mild curvature crease at the joins, not a gap) rather than C² quintic smootherstep, because the quintic's large global-σ coefficients made the interval γ-quadrature O(1/N)-loose (ε=6.6 at 32 panels vs cubic's 0.14). Full gate green (fmt, clippy default+diagnostics+step, nextest fixtures/develop/export + example default & `--features diagnostics`, doctests, `xtask lint`); the real (offset-annulus + cylinder-cuts + holes) geometry needs `--features diagnostics`, with a `{z=const}`-plane concentric fallback so the default build/tests stay green. *2026-08-12 · **resolved — one connected self-lapping chart + holed STEP, supersedes the two-solid DD.4** · branch `self-lapping-wrap`*

- **Driving Demo · DD.4 — the acceptance demo: the certified BONDED seam device end-to-end — the flex-PCB spine's north star, MET.** The self-lapping cone-with-ramp is realized as **body gore (γ=0) + ramp flap (γ≠0) + a certified bond** (§6.2 — a lap is doubled material, not one self-touching solid), and the whole bidirectional round-trip is certified on it. **The driver** (`crates/export/examples/bonded_seam_device.rs`) + **the composed test** (`crates/develop/tests/bonded_seam_device.rs`): each sheet **develops** to a certified flat pattern (**flap ε≈7.05e-3, γ≠0** via the DD.2 directrix integrator; **body ε≈7.6e-13, γ=0**) and a flat point **folds back** onto it (**flap ε≈8.1e-2** via DD.3's signed-µ̂ residual; **body ε≈2.7e-12**, matching DD.1), recovering (σ′≈1/4, µ̂≈−3/2) on both; the seam **bond** is certified by the Stage-2 §14 `valid_bonded_seam` (SEP ∧ SLAB ∧ SHEAR δ=18/65≈0.28mm ∧ CLEAR); and the two sheets emit as **two certified STEP solids** (`brep_trim_solid`, each `closed_shell_holed`-Verified + OCCT `audit_brep` **valid=true, free_edges=0, nonmanifold=0** — watertight + manifold), plus the two flat-pattern SVGs. **This composes the whole spine:** Stage 1 (per-panel pipeline) + Stage 2 (`develop::bonded` + `seam_frame`) + Milestone E (certified develop/fold) + DD.1 (fold-back leg) + DD.2 (the γ≠0 directrix integrator) + DD.3 (the γ≠0 fold) — **both product directions, on the real γ≠0 self-lapping geometry, bonded.** **Two decisions carried through:** (1) the flap develop is **point-sampled** at the band corners (each a certified `dev.point`) rather than boundary-`unroll`ed — `unroll` composes the same `point_on` and rides unchanged, but for a γ≠0 chart its anchor subdivision re-integrates γ per sub-interval, so it is impractically slow (a precomputed-γ-grid, or memoizing the running γ, would fix it — logged with the DD.3 perf note); (2) the two solids + the bond are the Stage-2 two-solid doctrine at the acceptance scale (no single self-touching 2π solid, no [D11]). **THE DRIVING DEMO MILESTONE (DD.0–DD.4) IS COMPLETE.** *2026-08-11 · **DD.4 met — Driving Demo COMPLETE** · branch `driving-demo`*

- **Driving Demo · DD.3 — the γ≠0 fold (folding onto the seam-ramp flap), with the signed-µ̂ residual flip.** DEV.2e's `fold_point` inverts the *pure-radial* development `D = µ̂·ρ·e(ψ)` (γ=0): recover σ by bisecting the signed area `cosψ·y − sinψ·x`, then `|µ̂| = |(x,y)|/ρ`. DD.3 extends it to the γ≠0 flap `D = γ(σ) + µ̂·ρ·e(ψ)`: `fold_point` now builds `ConeDevelopment::new_developable` (γ=0 charts get `directrix = None` → **byte-identical** to DEV.2e, every existing fold test passes), `cross_at` inverts the **directrix residual** `(x,y) − γ(σ)` (signed area `cosψ·(y−γ_y(σ)) − sinψ·(x−γ_x(σ))`, γ from the DD.2 `directrix_at`), and the radius reads `r = |(x,y) − γ(σ)|` via `sqrt_on`. **The one subtlety** (and the thing that would silently give the wrong σ): the γ≠0 development uses **signed** µ̂, so the device band's `µ̂ < 0` puts the residual `µ̂·ρ·e(ψ)` at angle **ψ+π**, which *flips* the signed-area bracketing `invert_sigma` bisects on (`+→−` becomes `−→+`). Fixed with a `flip = has_directrix ∧ mu_negative` flag that negates the signed area — the apex cone (which uses `|µ̂|`, residual always at ψ) never flips, so its bisection stays byte-identical. **Result:** folding a flat point on `cone_seam_ramp` (µ̂ = −3/2, σ′ = 1/4) recovers its `(σ′, µ̂)` and round-trips to **ε ≈ 3.85e-3** at 64 γ-panels (γ-floored — the DD.2 quadrature ε dominates; refine `GAMMA_PANELS` to tighten), clearing the DRC; a tight clearance → `Unresolved` (fail-closed). `point_on` already carries γ (DD.2), so the round-trip re-development is correct with no change. **Perf note (logged):** each `invert_sigma` bisection step re-integrates γ from 0, so a γ≠0 fold is O(iters × panels) — fine at the spike scale, a candidate for a precomputed-γ-grid optimization if the seam device needs many folds. *2026-08-11 · **DD.3 met** · branch `driving-demo`*

- **Driving Demo · DD.2 — the γ≠0 flat-directrix integrator (DEV.3 method b) — GO, machine-exact isometry.** The one genuine frontier the Driving Demo forces: the ramp flap (`cone_seam_ramp`, shared cone-seam `q` + ramping support `h = 1/4 − σ′/2`) is a **γ ≠ 0** developable whose flat pattern gains a **directrix** `γ(σ) = ∫₀^σ [a·e(ψ) + b·e⊥(ψ)]`, `a = (c′·r)/ρ`, `b = −(c′·n′)/ρ` (spec §Tier C — the development maps the positively oriented tangent pair `(r/ρ, −n′/ρ)` to the flat frame `(e, e⊥)`). The integrand is `rational × {cos,sin}(c·arctan σ)` — **non-elementary**, so it needs **validated quadrature**. **Built:** `develop::interval::integrate_on` (a verified interval Riemann sum — panel-range × width contains the integral, width `∝ 1/panels`; doctest + a transcendental-integrand unit test `∫₀¹ cos = sin 1`); `develop::cone` **generalized in place** — an optional `Directrix{c′·r, c′·n′}`, `point`/`point_on` add `+γ(σ)` with **signed** µ̂ (the directrix breaks the apex symmetry), a **byte-identical `γ ≡ 0` fast path** (`new_developable` on the apex cone reproduces `new` exactly — unit-tested), and `arctan_coeff` (the pedal-free half of `cone_angle_coeff`) lifting the pedal gate that turned the ramp away; `unroll`/`anchor` ride unchanged (the chokepoint is `dev.point`). **The load-bearing corroboration is the local isometry** `|D_σ|² = |X_σ|²`, computed from the directrix *velocity* `γ′` (no quadrature error) against the 3-D surface's own first fundamental form — it lands at **7.1e-15** (float epsilon), so the §Tier C frame/sign is exactly right (a wrong sign gives the non-isometric defect `4bℓψ′ ~ O(0.1)`, which this catches; the paper explicitly flags an earlier draft that got it wrong). The certified `ε_γ` converges ~linearly and is fab-plausible: **1.45e-3 (64 panels) → 9.0e-5 (1024 panels)** on the mid-ramp, far under the demo DRC. Report `docs/spike-directrix-report.md`. **Deferred (logged, not built):** the quadrature is linearly-convergent (a higher-order/adaptive rule would tighten it — same class as the CLEAR tech-debt); `point_on` re-integrates from 0 per interval-σ call (correct, unoptimized); two-sided σ<0 is out of scope (the flap is one-sided `σ′∈[0,1/2]`). *2026-08-11 · **DD.2 met — GO** · branch `driving-demo`*

- **Driving Demo · DD.1 — the fold-back leg wired end-to-end: the γ=0 cone-gore round-trip (author on the flat pattern → fold onto the cone).** The Stage-1 `flex_panel` demo only lifts cuts *forward* (author in `(σ,μ)` → develop / `brep_trim_solid`); it never authors on the flat pattern and folds *back*. DD.1 wires that missing leg. **The driver** (`crates/export/examples/roundtrip_panel.rs`): **boundaries cut in 3D** (`certified_rail` on a concentric plane cut `D1` + an eccentric cone∩cylinder cut `D2` → `annulus_loop`) → **develop** to the flat pattern (`unroll_loop`, direction ①) → **author an interior ECAD feature on the flat pattern** (a rectangle in developed `(x,y)`, placed around the developed gore centre) → **fold it back** onto the cone (`develop::fold::fold_outline`, direction ②) → a certified 3-D wire `C(σ,μ̂,w)`; SVG (`assemble_flat` panel − feature, even-odd) + a certified STEP annulus solid (`brep_trim_solid` at low-degree rails, OCCT `write_brep` "ok", 10 faces / 0 free). The fold-back **round-trip backward error is ≈2.7e-12** on the device gore (σ∈[−1,1], ~180°) — the fold recovers the surface to ~1e-12. **The certified evidence** (`crates/export/tests/roundtrip_fold.rs`): a flat-authored rectangle (its 3-D preimage a chart-space rectangle developed forward so the test self-locates) **folds back and recovers the original 3-D geometry to <1e-6** — develop∘fold ≈ identity — measured against the independent float 3-D positions; the independent float `develop_cone` oracle reproduces the flat feature to <1e-5 (oracle ∧ audit, the develop leg the fold inverts); a feature whose angle exceeds the gore is `Refuted(OutOfGore)` (fail-closed, never a wrong `Verified`). **Scope (correctly bounded, no new frontier):** DD.1 is γ=0 throughout and reuses DEV.2d `unroll` + DEV.2e `fold` + `export::trim` — no new machinery. **Cutting the folded feature THROUGH the curved B-rep** is deliberately deferred to DD.4: there is no general `(σ,μ)`-polygon hole in a curved solid (`brep_trim_solid` holes are `HoleRail` σ-bands; a freely-folded feature needs its recovered `(σ,μ̂)` fitted to μ-rails via the existing cut oracle), and the plan places that device-assembly step in DD.4 (body gore + ramp flap + folded interior features). Here the folded feature is the certified 3-D wire on the solid. *2026-08-11 · **DD.1 met** · branch `driving-demo`*

- **Driving Demo (DD) — the S2.0-style GO-gate: the self-lapping cone-with-ramp, full 3D↔2D round-trip (DD.0).** The post-Stage-2 culminating acceptance demo (the whole flex-PCB spine's north star, `memory/driving-demo.md`): a cone whose scalar support `h` **ramps `0→D≈0.27mm` over the last ~60°** so an offset sector **laps** the base at the seam, put through the **full round-trip** — author + **cut boundaries in 3D** → **develop** → author **interior ECAD in 2D** → **fold back** to 3D. It **composes** what exists (Stage 1 per-panel pipeline, Stage 2 `develop::bonded`+`seam_frame`, Milestone E develop ①/fold ②) and lands the one new frontier the geometry forces — **the γ≠0 flat-directrix integrator** (DEV.3 method b). Criteria in `docs/vv-guide.md` ("Driving Demo (DD) acceptance criteria") + `vv-matrix.md` DD rows + tasks DD.0–DD.4; branch `driving-demo` (off `stage-2-seam`). **Four decisions locked with the user (they scope the milestone):** (1) **Representation = body gore + ramp flap + certified bond** — the base-cone gore (`h≡0`, ~300°) + the ramp flap (`h:0→D`, ~60°) cover 2π, joined by Stage-2 `valid_bonded_seam`; the "self-lapping single chart" mental model and the two-view body+flap assembly are the *same* geometry (a lap is doubled material, §6.2 — no single self-touching 2π solid, no [D11]). (2) **Boundaries cut in 3D; only interior ECAD authored in 2D + folded back** — the outer/inner/notch stay `export::trim`/`unroll_trim_loop` exactly as Stage 1; the 2D→3D fold leg carries interior cuts. (3) **Round-trip first, then the seam** — de-risk the fold-back leg on the plain cone gore (DD.1) before the transcendental γ (DD.2/DD.3) and the seam device (DD.4). (4) **Build the γ integrator** — the flap's flat pattern is *certified* (verified interval enclosure), not emission-only. **The four gaps → slices:** DD.1 the fold-back leg is unwired (`fold_outline` built+tested but the driver only lifts cuts *forward*); DD.2 the flap's flat pattern needs `γ(σ)=∫₀^σ e(ψ)·pedal-speed = ∫(rational×cos(c·arctan σ))`, non-elementary for `c=130/97` — `develop::cone` hard-codes the pure-radial `D=µ̂·ρ·e(ψ)` (γ≡0) + rejects `h≠0` (the `cone_angle_coeff` pedal gate), `develop::interval` has no quadrature; DD.3 the γ≠0 fold makes `invert_sigma` a coupled 2-D solve (subtract `γ(σ*)`); DD.4 the full-2π self-lap = the body+flap+bond assembly (extend the Stage-2 two-solid stub to the real device). **Doctrine:** no float in a certificate (the γ integrator is shell-tier rational-interval arithmetic, like `cut_fit`/`anchor`/`clear` — no Kani/Lean TCB growth); fail-closed; oracle ∧ audit. **Generality (hard gate):** `ConeDevelopment` gains an *optional* γ with a byte-identical γ≡0 fast path (one general engine, the γ=0 cone is the thin special case — no interface ossification); no checker names "ramp"/"cone-seam". *2026-08-11 · **DD.0 met** (docs/criteria); DD.1–DD.4 pending · branch `driving-demo`*

- **Stage 2 · S3 (§14 BONDED) COMPLETE — the certified BONDED lap seam (SEP/SLAB/SHEAR/CLEAR + gate + two-solid demo).** The `develop::bonded` certifier discharges the four §14 invariants on the device seam: **SEP** (face separation ≡ bond gap `g` on the plateau — §7 `c·n≡h`, an exact scalar identity; the ramp's varying support → `NotConstant`, i.e. SEP is a plateau property, the ramp is CLEAR's); **SHEAR** (the Tier-1 identification `J=rigid∘shear` collapses: `κ_g=−65/72`(−tanβ) const, `Δ₀=1/4` ⇒ `δ=−Δ₀/k=18/65≈0.28mm` — reproduces the paper's ghost-footprint number exactly); **SLAB-S0** (offset slab regular, `det J≥m>0 ⟺ R₁+w>0`, **reusing** `certify_core::certify1d::reg_q`'s Sturm positivity — the searcher builds the σ'-num/den + chains, the checker re-verifies); **CLEAR** (S3.2 — the novel adaptive interval subdivision). `valid_bonded_seam` conjoins the four (strong-Kleene: first Refuted wins, CLEAR Unresolved propagates fail-closed). **The two-solid demo** (step-gated, `export::brep_build`): the seam emits as **two independent closed solids** — the cone body (γ=0) and the **γ≠0 lap flap** (`brep_trim_solid` pulls `c=chart.pedal()`, so it builds the ramped surface with no change) — each `closed_shell_holed`-Verified **and** OCCT-corroborated (`brepcheck_valid`, `free_edges==0`, `nonmanifold==0`), the bond `valid_bonded_seam`-Verified, two STEP solids `write_brep`→"ok". **Two findings:** (1) the γ≠0 flap solid passes OCCT cleanly — no pcurve / `IntersectingWires` surprise at this band scale (unlike G-C's flush-σ-cap); (2) a lap is **doubled material** (§6.2), so the right emission is two solids + a certified bond interface, NOT one self-touching OCCT solid. **De-risking headline:** the roadmap's scariest item — the "seam-ramp subdivision" — turned out **rational, not transcendental**, and three of the four invariants are exact/Sturm (reusing existing molds); only CLEAR is new (and flagged as brute-force tech-debt for a structural rewrite). Whole develop crate green (83 lib + 9 doc); demo green under `--features step`. **Stage 2 COMPLETE** (S2 closure + S3 BONDED) — the second flex-PCB acceptance demo. *2026-08-11 · **S3 met — Stage 2 COMPLETE**; branch `stage-2-seam` (commits `37443d5`→`24e6d8a`)*

- **Stage 2 · S2 (full-2π closure) — the seam brought to a finite, regular parameter, exactly + certified (GO).** The seam ruling (`φ₃D=±π`, `σ=±∞`) is single-chart *representable* but not *certifiable* there (subdivision needs finite widths; `µ̂∝1+σ²` is unbounded). Fix = a re-centered rational chart: the axis half-turn `φ→φ+π` is the exact Möbius `σ=−1/σ'`, and substituting into `q=(9,4,4σ,9σ)` + clearing (`R(λq)=R(q)`) gives `q'=(9σ',4σ',−4,−9)` — still a degree-1 rational cone, seam at `σ'=0`. Packaged as the chart-agnostic `develop::seam_frame` (`SeamFrame{view,transition,seam_param}` + `seam_frame_exact` discharging the re-centering as an **exact rational identity** `view.normal ≡ base.normal ∘ transition`, pedal too — `RatFunc` Horner composition over the public ops, lattice/TCB untouched, fail-closed). Three verified facts: (1) `n_seam(−1/σ)≡n_cone(σ)` exactly + `seam_frame_exact` Verified (refutations fire on a wrong transition / a different cone); (2) the existing `ConeDevelopment` recognizes `cone_seam` (`c=130/97`) and develops the seam ruling — unreachable at `σ=±∞` — to the exact `(144/97,0)`, unchanged; (3) oracle∧audit — the float `develop_cone` corroborates the re-centered development to `max_diag≈1.5e-8` (analytic `≈1.8e-12`, backward error `≈6e-12`), `ρ_seam(σ')=144/(97(1+σ'²))` same form as the canonical chart. **Key de-risking:** the "seam-ramp subdivision" the roadmap feared is **rational, not transcendental** (3D distance between two rational sheets), scoped to S3's CLEAR; the flat-γ is never needed for the certificate. Report `docs/spike-seam-closure-report.md`. *2026-08-11 · **S2 met — decision GO**; S3 next · branch `stage-2-seam`*

- **Stage 2 (BONDED lap seam) — the S2.0 GO-gate + two design corrections that reshaped the milestone.** Stage 2 closes the rolled cone with a certified **BONDED lap seam** (S2 full-2π closure + S3 §14 BONDED, S3⊳S2). Criteria authored in `docs/vv-guide.md` ("Stage 2 (BONDED lap seam) acceptance criteria") + `vv-matrix.md` rows + tasks S2.0/S2/S3; branch `stage-2-seam`. Two user-steered corrections to the initial "cone + bolt-on seam" framing, worth carrying: **(1) the seam is single-chart *representable* — re-centering only *conditions* the certificate.** `σ∈ℝ ↔ φ₃D=2·arctan σ∈(−π,π)` already covers the whole cone except the back ruling; the seam *is* that ruling, at `σ=±∞`. Re-centering (`σ'=−1/σ`, the exact rational half-turn `φ→φ+π`, giving `q'=(9σ',4σ',−4,−9)`, still a degree-1 rational cone since `R(λq)=R(q)`) adds **zero representational power** — it only moves the seam off the coordinate singularity so a *subdivision* certificate can **converge** there (at `σ=±∞` the enclosure widths `µ̂∝1+σ²` are unbounded/non-refinable). Representation ≠ certification-conditioning — the two had been conflated. **(2) BONDED requires γ≠0 by design — but the *mild* γ≠0.** The lap flap climbs Δ≈0.25 mm to seat: `docs/paper.md §8`'s degree-1 ramp picks the family where **`n` rides the cone's Gauss circle (shared `q`) while the support `h` ramps** — a nonzero-support (γ≠0) developable. But `ψ` is `h`-independent (`geom::chart::psi_prime` reads only `n,n′,n″,|n′|²`, all from `q`; verified), so `ψ=c·arctan σ` stays **closed-form** — this is NOT DEV.3's interval-integrated curved directrix (that trigger is a complex `q`; a genuinely deferred, different gap). **The load-bearing de-risking consequence: the BONDED certificate is rational and lives in 3D.** `Chart::surface = c+µr+wn` is rational in `(σ,µ,w)` for *every* developable (`c` already carries `h≠0`, cf. the `closure_joint.rs` nonzero-`h` fixtures), so the certifier is chart-agnostic and the transcendental `ψ` is confined to the *flat* development (emission-only). Decomposition (§7/§8/§11): **SEP** (≡ bond gap `g`, §7 face identity, "two ring scalars") and **SHEAR** (`δ=−Δ₀/k`, §7) are **exact rational**; **SLAB** (`R₁=s·tanβ+(h+h″)>0`, §8/§11) is **single-span-Sturm** (reuse `reg_q`/`slab_s0`); only **CLEAR** (the pair/self clearance over the Δφ≈60° ramp box) is the new piece — an **adaptive interval subdivision of the 3D distance between two RATIONAL sheets** (`eval_ratfunc_on`, mirror `cut_fit`, `Unresolved(ε)` fail-closed), **rational not transcendental** (the roadmap's "single highest-risk unknown", de-risked). **Decisions locked at S2.0:** build BOTH the certifier AND the `SeamFrame` reduction general (pinned by two instances — cone body Möbius + γ≠0 ramp tail); the demo emits **two certified solids + a certified BONDED interface** (a lap is doubled material, §6.2 — not one self-touching OCCT solid); **[D11] chart-graph cycle out of scope** (a lap bond is an assembly declaration with gap `g`, not a rigid metric cycle-closure). New checkers → own transcendental-free `certify-core` module (rational side of the future `develop` split). *2026-08-11 · **S2.0 met** (docs/criteria); S2 + S3 pending · branch `stage-2-seam`*

- **STEP builder (gap G-C): the xy-trimmed panel as a certified curved-rail B-rep solid (`brep_trim_solid`).** The trim's `(σ,μ̂)` loops lift **directly** (no fold, no arrangement — development is a certified isometry, so STEP I and STEP II are the same locus) into a watertight closed solid on the cone: a **generalized band** `μ ∈ [μ̂_inner(σ), μ̂_outer(σ)]` extruded through `w`, σ-sliced at `sigma_splits ∪ piece-boundaries`, each slice four ruled sides (two lids + inner/outer walls) + planar σ-caps, `Rail→RationalBezier`, `Cap→Line`, watertight by the `Builder`'s vertex+edge dedup. **Stage A (genus-0, `839ff53`):** eccentric annulus + D3 rim **notch** (piecewise outer boundary, the kink corner `μ̂_D1=μ̂_D3` dedups). Four reusable fixes fell out: `ruled_common` (cross-multiply two μ̂-rails to a shared denominator — `ruled_from_rails` needs *identical* denominators, the straight ruling's equal end-weights); `poly_rail`+stitch-ordering (polynomialize each piece *first*, then constant-shift the corner — a shift of a polynomial stays low-degree, so D1 doesn't blow up to degree 16); `snap_boundaries` (dyadic-snap the ~60-bit bisection crossings — huge-rational Bézier control points break OCCT's f64 conversion); low-degree fit (degree 4). **Stage B (genus-1):** an interior hole drills a through-tunnel — both lids become annular (`add_face_with_holes`) and a four-wall tube (near/far ruled walls + two planar σ-cap walls) closes the bore, +1 genus. **The non-obvious constraint (an OCCT-oracle catch the combinatorial TCB can't see):** a hole must sit **strictly interior** to a single σ-slice. My first cut *added the hole's tangent σ as stations* — which made the slice exactly as wide as the hole, so the hole's σ-cap edges lay flush on the slice's own σ-caps; the combinatorial `closed_shell_holed` certified it (∂²=0, vertex-links, genus all hold) and OCCT agreed it was topologically closed (`free=0, nonmanifold=0`) — yet `BRepCheck` rejected it with `IntersectingWires` (status 22) on the annular lids, because the hole loop *touches* the outer loop in the face's `(u,v)`. Fix: **don't** add hole σ as stations (aligning a slice boundary to the hole is exactly the flush-degeneracy), assign each hole to the slice that *strictly* contains it (`sk < s1 ≤ s2 < sk1`), and refuse (`None`) a hole that touches or straddles a station (the cross-station **notch** is Stage C). Both stages certified (`closed_shell_holed` Verified + Euler genus) **and** OCCT-corroborated (`write_brep` "ok", `free_edges==0`). **Stage C (fully general — a hole may cross any station):** rather than special-case the notch, the per-slice construction was **unified** into a curved-rail analogue of `brep_freeboundary_holed`'s proven *top-forward/bottom-reversed lid + wall-per-rim-edge* convention. A footprint corner is `(σ, μ̂-rail)`; a boundary edge with varying σ is a rail `RationalBezier`, a `σ=const` edge a radial `Line` (`slice_footprint`/`lift_trim_edge`/`emit_trim_wall`). `slice_footprint` returns the slice's `(σ,μ̂)` region — the band strip minus the holes touching it — as `(outer, [holes])` faces: a hole strictly inside is an annular inner loop; a hole reaching a station opens onto the cross-ring as a curved **notch** (its σ-cap becomes the split cross-ring radial, shared once-each-way with the neighbour slice's lid); a hole spanning a full slice **splits** its lid into a `[μ⁻,near]` bottom band and a `[far,μ⁺]` top band. Both lids lift from the same footprint (whi forward, wlo reversed → face outward), and every footprint edge is swept as a wall **except** a radial at an interior station (the shared cross-ring) — so band walls, hole tubes (split per slice), σ-end caps, and notch openings all fall out uniformly, watertight by dedup. This *replaced* the Stage-B annular-tube special case (one engine, no interface ossification). Holes need only be strictly interior in σ + pairwise σ-disjoint; the demo's D4 at σ=0 (a forced station, since `1+σ²`'s middle Bernstein weight is 0 over `[−1,1]`) is now a first-class notch. All three regimes (interior genus-1, station-crossing notch, multi-station span) certified + OCCT `write_brep` "ok". **Demo wiring:** `flex_panel` now emits the *actual* trimmed panel — STEP I the annulus+notch blank, STEP II + the D4 through-hole (which lands at σ=0, a forced station, so it exports as a real cross-ring notch) + the authored quad, both from the same `(σ,μ̂)` loops the SVG uses (`export::trim::hole_rail` adapts a `HoleLoop`→`HoleRail`, dyadic-snapping the tangent σ). One finding: the STEP path re-fits rails at **degree 4** — the SVG's default degree-6 cut rails carry too many Bézier control points, so OCCT's `MakeEdge` f64 endpoints drift off the shared vertices (the exact-rational IR is fine; only the f64 export cares). STEP I 18 faces / STEP II 28 faces, both `closed_shell_holed`-Verified + `write_brep` "ok", 0 free edges (`full_panel_solid_exports` pins it). **Why the gore was ~180°, and widening it to ~296° (user asked "we planned 300°").** The σ↔azimuth law is *exactly* `φ = 2·arctan σ`: the development coefficient is `c = 130/97` and the development factor is `sin β = n·ẑ = 65/97`, and `c = 2·sin β` **exactly** (`130 = 2·65`), so `φ = ψ/sin β = c·arctan σ / sin β = 2·arctan σ`. Hence `σ∈[−1,1]` is *exactly* 180°, and the legacy band's `σ∈[−15/4,15/4]` was 300.3°. The `[−1,1]` cap was a **conservative default, not a limit** — a σ-sweep shows the trim's `outer_loop` stays Verified (`ε≈0.20`) and the full **STEP II exports "ok" past 304°** (the STEP path lifts the rails directly — no unroll). The real cap is the **SVG unroll**: the D1 plane cut's rail grows `µ̂ ∝ (1+σ²)` (2.72·(1+σ²) exactly), so the interval-development DRC `ε` climbs — `0.16`(180°)→`0.29`(254°)→`0.42`(286°)→`0.49`(296°)→ crosses `clearance/2 = 0.5` and goes `Unresolved` at 300°. Widened the demo to `σ∈[−7/2,7/2] = 296°` where the whole pipeline (SVG unroll `ε≈0.487` + both STEP solids) still certifies at the honest `clearance=1`; a larger clearance / tighter dev config pushes further. *2026-08-11 · Stage A + B + C + demo (296°) done · branch `roadmap-flex-pcb`*

- **Stage-1 flat, xy-trimming rebuild: the cone trimmed by an arrangement of vertical cylinders → certified flat pattern (`export::trim`).** The demo now authors trimming as *disks in the physical xy-plane* — `(D1−D2)−D3−D4`: **D1** concentric outer (exact `{z=d}` plane rail), **D2** eccentric inner containing the apex (cone∩cylinder → eccentric annulus), **D3** a boundary **notch** straddling the rim, **D4** an interior circular **hole** — pulls each back to a certified ruling-rail `μ̂(σ)` (G2: `fit_cut_rail` proposes, `cut_fit` decides), unrolls the boundary loops, and stitches the flat panel with the exact `arrange2d` boolean. The **key design realization**: `arrange2d`'s content class is lines + circles, and the *only* plane where `(D1−D2)−D3−D4` lives in it is **physical-xy** (in `(σ,μ)` the disks are unsupported Bézier rails, in the developed/flat plane they are transcendental curves); the cone→xy projection is a homeomorphism on a <360° slice, so the physical-xy arrangement combinatorics are the panel's. Five findings fell out, each logged below; all fail-closed, all certified. **Scope: flat-first** — fold + STEP export of the trimmed geometry needs a curved-rail B-rep builder (gap **G-C**, deferred to Stage B); the demo keeps the legacy band+rectangle STEP under `--features step`. *2026-08-11 · SA.1–SA.6 done · branch `roadmap-flex-pcb`*

- **`arrange2d` had no set-difference — added `BoolOp::Diff` (A∧¬B).** Only `Xor/And/Or` existed; difference was faked as `Xor`, valid only for a strictly-interior subtrahend. `(D1−D2)−D3−D4` has boundary-crossing subtrahends (D2 touches the apex/wedge, D3 straddles D1), so a true difference is required. It is **one selector arm** (`select`); the labeling / ℤ₂² cocycle / CAP-OUT machinery reads the selection, not the op, so the certified entry certifies `Diff` across the whole corpus. Caveat (documented on the variant): overlapping `B` loops must be `Or`-composed first — disjoint loops already read as their union. *2026-08-11 · resolved (`614da8f`) · branch `roadmap-flex-pcb`*

- **A *cut* (circular) boundary is a varying-μ̂ rail, so a large radius / wide gore blows the interval development up — the constant-μ band is exempt.** A circle on the cone develops to a rail `μ̂(σ)` that is *not* constant (only the free-boundary band is), and `μ̂ ∝ 1/|ruling_⊥(σ)|` grows like `(1+σ²)` toward the gore edges (µ̂ ~ 300 at the ~300° edge while `ρ ~ 0`); the developed point `|μ̂|·ρ` is bounded but interval arithmetic can't cancel the huge×tiny product, so the certified unroll ε stays loose (~1/segments) and slow. **Two levers, both scale-invariant for the shape:** band-scale units (µ̂ small, like the µ=−2 band) and a moderate gore (`σ∈[−1,1]`, ~180°). The demo uses both. A reparametrized development (tracking `|μ̂|·ρ` directly, a TCB change) would lift this; deferred. *2026-08-11 · watching · branch `roadmap-flex-pcb`*

- **Algebraic-σ arrangement vertices → the micro-cap treatment.** Arrangement-derived boundary transitions (D3∩D1 crossings; the D4 tangent rulings where near meets far) land at **algebraic (non-rational) σ**, but `unroll::BoundaryArc` requires **rational** `sigma_start/end` and `unroll_trim_loop` chains loops **exactly** (`sm_eq`). Fail-closed treatment: snap σ to a rational (from a certified bisection of the relevant rational residual — the D1-rail∩D3-cylinder `h(σ)`, or `tangent_poly` `g(σ)`), and bridge the tiny `μ̂` mismatch between the two adjacent rails with a **micro-cap** (an exact radial `Cap`). The loop then chains exactly while the geometric error is a certified-small residual. **SVG-polish refinement:** for a *transverse* crossing (D3∩D1) the micro-cap is driven to ~0 by refining the crossing σ to where the **fitted** D3 meets D1 (`μ̂_D3fit − μ̂_D1 = 0`, a bisection) instead of the exact geometric crossing — the two rails then coincide there and the D1↔D3 corner is a **clean join**, no visible step. Below the development's rounding precision the two developed points collapse to the *same* rational (a zero-length edge `arrange2d` rejects as `DegenerateLine`), so `flat_to_poly` also **dedups exactly-coincident consecutive vertices** (float-free). D4 tangent micro-caps stay (the √-branch residual, see next). *2026-08-11 · resolved (`5973302`, `131ca8c`; SVG polish follow-up) · branch `roadmap-flex-pcb`*

- **A developed circular hole has slightly-flattened tangent points — an irreducible √-branch limit, not a bug.** A circle's boundary is double-valued in σ (near/far branches meeting at the two tangent rulings, where `μ̂` has a vertical tangent — a √-branch point). A polynomial rail cannot match the vertical tangent, so the near/far fits stop meeting a small gap short (~0.06 μ̂ on a ~0.4-tall hole, floor independent of margin/degree); the two branches are joined by a tangent micro-cap → the developed hole's two points are slightly flattened (an exact, watertight `Cap`). Fully-exact alternative (AlgReal-σ `BoundaryArc`, or a per-point developed polyline) deferred. *2026-08-11 · watching · branch `roadmap-flex-pcb`* — **AMENDED 2026-08-14: "irreducible" was true only of the *graph* representation `µ̂ = f(σ)`, and the flattening is far larger than this entry's ~0.06 estimate suggests.** The limit is not the √-branch as such but the decision to represent a closed cut as two graphs; measured on the device drill, the *best* the graph model achieves over every margin/degree/subdiv rung is a chord ~30% of the hole's height (the shipped rung gives 48%). See the p-curve entry at the top of this section — the deferred "per-point developed polyline" was in fact built and shipped in the old demo, then deleted with it.

- **The cut oracle's monomial-basis Vandermonde fit is ill-conditioned over a narrow off-origin σ-range.** Fitting the D3 notch's near branch over `σ≈[0.3,0.5]` (narrow, far from the σ-origin) with `fit_cut_rail`'s monomial Vandermonde yields huge coefficients at degree ≥ 4, whose interval evaluation explodes (`cut_fit` ε ~ 100s–1000s, *growing* with degree). A **low degree** (3; the dip is gentle) certifies tightly there. The hole fit is unaffected because its range straddles the origin. A Chebyshev-basis oracle would remove the per-rail degree cap; deferred (oracle-only, float-side, fail-closed regardless). *2026-08-11 · watching · branch `roadmap-flex-pcb`*

- **DEV.2a committed fmt-unclean — a false-green the DEV.2b gate caught.** At DEV.2b start, `cargo fmt --all --check` (run to a real exit code across the whole workspace) failed on four hunks that had already landed in the DEV.2a commit (`3e18c61`): long single-line `if`/`assert!`/struct-literal expressions in `crates/lattice/src/rat.rs` (the `floor`/`ceil` fast path) and `crates/export/src/mesh3d.rs` (the corroboration test) that rustfmt wraps. Pure whitespace, no semantic change — but it means the DEV.2a "full gate green (fmt …)" claim did **not** actually run `fmt --all --check` to a checked exit (the exact [[verify-green-rigorously]] failure mode: a `| tail`-masked or scoped fmt invocation reads green while the real check fails). Fixed in a dedicated `fmt:` commit (`5a652b8`) reformatting only those two files, so the DEV.2b commit's diff stays clean and the workspace fmt gate is honestly green. Lesson reinforced: the fmt gate is `--all --check` with `EXIT=$?`, never a filtered or per-crate run. *2026-08-10 · resolved (`5a652b8`) · branch `dev-go-gate`*

- **D4.3 scope: the σ-band free-boundary form is forced by a missing polynomial-composition primitive — and it is the tractable, spec-sanctioned one.** The spec's substrate free boundary (`spec §3.4:151`) is a **σ-band with rational μ-boundary splines** `μ⁻(σ), μ⁺(σ)` over `[σ_lo, σ_hi]`, *not* an arbitrary planar outline `(σ(t), μ(t))`. This is not a shortcut — it is exactly what lets D4.3 close a solid with the existing machinery: every boundary rail is `c(σ) + μ±(σ)·r(σ) + w·n(σ)`, all functions of the **same** σ, so lifting is `Vec3Rat::scale` by a `RatFunc` (the direct generalization of `brep_slab`'s constant-μ `scale_rat`). A general `(σ(t), μ(t))` outline would need `c ∘ σ(t)` — **polynomial/RatFunc composition**, which the repo lacks (`geom::Chart::surface(μ, w)` takes a *scalar* μ; there is no `Poly::compose` / `RatFunc::compose`). So the general contour is deferred to a follow-on (composition primitive + an N-edge top/bottom cap subdivision), and D4.3 Stage 1 builds the σ-band. The **σ̂-monotonicity** arm of the ANCHOR checker is the one piece of the general-anchor obligation that is composition-*free* (it is purely about the σ-projection `σ̂(t)`), so it is implemented + refutation-tested now (a fold-back `σ̂` refuses) even though the σ-graph makes it trivial — real, forward-compatible verified code, not demo. *2026-08-09 · recorded (D4.3.0) · branch `d4.3`*

- **CM.4 finding (corrects a feared CM.1 gap): the full-`R` cofactor check certifies a reflection-mate cone miter — no branch-refinement needed for it.** A cone (rotating rulings) has a genuinely-rational, **non-affine** crease-line coordinate `ℓ(σ)` (measured `num` deg 1 / `den` deg 2 for `q=(1,σ,1,0)`, h=0, Π={x=1}), so the correspondence `R = ℓ_A(σ_A)=ℓ_B(σ_B)` **factors**: `(2,−1)` is an off-diagonal solution, so `R ≠ const·(σ_A−σ_B)`. The feared gap: CM.1's cofactor check `X == R·Q` uses the full `R`, so it needs the carrier `X = D_A×D_B` to vanish on the spurious branch too, which for *two different* cones it wouldn't. **But for a reflection-mate (shared-apex) miter it DOES** — empirically `X(2,−1)=0`: two rulings of *one* cone meeting `L` at a shared point pass through {apex, point} and are therefore the **same line** ⇒ parallel ⇒ `X=0`. So the full `R` divides `X` (shared-apex, structural), and `miter_fit_transverse` handles the cone's carrier identity as-is. **Consequence:** the branch-refinement I feared (searcher-supplied `R_φ`) is *not* required for the achievable cone fixture; it is needed only for exotic two-different-cut-family (different-apex) miters with a nontrivial `φ_J` — deferred/documented. Verified by `closure::miter::a_cone_transverse_cut_family_certifies_through_the_full_r` + the new `transverse_cut_family` searcher. **RESOLVED (CM.4a): the adversarial case IS reachable and now certified.** The adversarial config is **two cones over a shared base conic from different apexes** — realized as the unit circle's tangent-line families at `t = σ` vs `t = 2σ`: `ℓ_A = (1+σ²)/(1−σ²)`, `ℓ_B = (1+4σ²)/(1−4σ²)`, `D_A = (2σ, σ²−1)`, `D_B = (4σ, 4σ²−1)`. `ℓ` is degree-2 ⇒ `R = 2(σ_A−2σ_B)(σ_A+2σ_B)` factors; `X_carrier` vanishes on the real branch `σ_A = 2σ_B` (same tangent) but **not** on the spurious `σ_A = −2σ_B` (the *other* tangent through the shared L-point) — so the full `R ∤ X` (verified: `x_carrier.div_exact(R_full) == None`) and CM.1's full-`R` check refuses, while `R_φ = σ_A − 2σ_B` divides `X` and certifies. **Built:** `certify_core::miter::TransverseBranch` + the branch-aware `miter_fit_transverse` (verifies `R_φ·C == R` by multiplication, `R_φ` single-valued/degree-1-in-σ_B, `R_φ` vanishes at the `ε_φ`-paired support corners — which rejects the spurious branch — then discharges `X == R_φ·Q`), and `lattice::Biv::div_exact` (the searcher's cofactor tool). Test `a_two_cone_adversarial_miter_certifies_via_the_branch`: full-`R` → `CarrierMismatch`, branch → `Verified`. **Honest scope:** the adversarial cut families are built from the conic tangent-line geometry directly (genuine geometry); the searcher-from-`Chart` link for the *adversarial pair* is not established (the arbitrary-apex-cone chart inverse problem is unsolved), though the single-cone `transverse_cut_family` searcher is validated separately. *2026-08-09 · CM.4a done · branch `curved-miter-fit`*

- **CM.2 finding: "conic carriers so a cone's cut image passes CAP-IN-D24" is unsound as framed — skipped.** Traced every `Carrier` consumer: it is used **only** by CAP-IN-D24 → the LEDGE arrangement, and the clean-miter path (CM.1 `miter_fit_transverse`) never touches `Carrier` — spec §5.3 makes the transverse cut faces **ruled by straight lines in Π** (`F_i = P_i + μ·D_i`), parametrized rationally in σ, which is exactly the `ℓ_i`/`D_i` data CM.1 already handles. A conic is **not D24 content** (D24 = lines + circular arcs, spec §6); CAP-IN-D24 refusing it is **correct** — `cap_in.rs:19` "the cone is correctly turned away" ("*falsely, not vacuously*" = a genuine-false predicate, i.e. the conic really lies on no line/circle, not an erroneous refusal). Two confirmations it would be unsound to "make it pass": (1) the `closure::ledge` bridge already declines even a **Circle** (`Carrier::Circle → LedgeError::UnsupportedCarrier`, `ledge.rs:70-72`) — so CAP-IN-D24 licensing a carrier never means the line/circle-only arrangement can build it; (2) `arrange2d` handles line/circle *intersection points* (degree-2, one radical), not conic **curves** — no conic arrangement exists. So a `Conic` that passed CAP-IN-D24 would license non-D24 content into an engine that cannot arrange it. Genuine conic support is the deferred conic-**arrangement** L3 (spec §484), the LEDGE branch — orthogonal to the clean-miter thrust. **Decision (with the user):** skip CM.2, keep the plan's `Conic`-carrier idea filed under the conic-arrangement L3, and proceed to CM.3 (`AlgReal` wiring). *2026-08-09 · skipped; conic support → conic-arrangement L3 · branch `curved-miter-fit`*

- **D4.2 finding: `one_joint()` cannot close into a two-flank solid — a *fixture* obstruction, not a code gap.** Two exhaustive surveys (the trim / Π-cut machinery; the V_∂ / CAP-OUT cap machinery) established two independent obstructions: (1) the **2:1 ruling-speed overhang** — flank A's crease spans `x∈[−2,2]`, B's `x∈[−1,1]` (`|r|=2` at σ=0 vs `1` at σ=1) — leaves free tips, and equalizing them needs the **irrational** station `σ=√2−1`, unavailable to a rational crease; (2) a single joint's **substrate boundary is honestly open** ("no contour to feed"; closing sidewalls need an anchored contour — atlas-scale). Even a real `V_∂`-projected cut cap would fix only the *cap↔flank* seam, leaving those *flank↔flank* overhang tips + the open substrate free. Geometry verified symbolically: the crossing locus `w*(σ) = −g0/g_w = 2σ(1+σ)/(1−2σ−σ²)` is **rational** (not algebraic — no `AlgReal` needed to *emit* `flank∩Π`), and the neutral sheet `w=0` is **regular** (`ψ'=0` for the cylinder ⇒ `det J = |n'|²(1+w) > 0`); but the flanks meet only along the crease **line** `L` (`w=0`, on Π), which sits on the certificate's **trim boundary** (`G=0`, not the strict-retained `G>0`) — so there is no 2D shared interface to make a closed manifold. **Decision (with the user):** do not dodge with a symmetric demo fixture; build the machinery the closure genuinely needs — the curved Π-cut miter — as the **Curved MITER-FIT** milestone (To do above; `docs/vv-guide.md`). *2026-08-09 · recorded; pivot to Curved MITER-FIT · branch `curved-miter-fit`*

- **D4.1: a curved flank slab's `Vec3Rat` surface MUST be reduced before the Bézier/`f64` cast, or
  OCCT segfaults.** `geom::Chart::surface(μ,w) = c.add(&r.scale_rat(μ)).add(&n.scale_rat(w))` uses
  `Vec3Rat::add`, which *multiplies* denominators (not lcm) — so `c + μr + wn` piles up a common factor
  and the μ-wall rational reaches **degree ~18**. Converting that to a rational Bézier produces enormous
  Bernstein coefficients that overflow to `±∞` on the exact→`f64` cast; `new Geom_BezierSurface` (or
  `Geom_BSplineSurface`) with `±∞` poles then **crashes with SIGSEGV** (below the C++ try/catch, so no
  error string). Fix: added `lattice::Vec3Rat::reduce()` (common-gcd cancellation, value-preserving,
  keeps the shared-denominator form) and build the slab's σ-rails as reduced `base_j + w_k·n` — degree
  drops to ~4, poles finite, OCCT happy. Reducing `base` and `n` first also keeps a μ-wall's two
  `w`-rails sharing one denominator (the shared-weights condition the ruled patch needs to be exact).
  Diagnosed by stderr-marker bisection (lldb has no `debugserver` in the nix shell). *2026-08-09 ·
  resolved (`aaea9c0`) · `crates/lattice/src/ratfunc.rs`, `crates/export/src/brep_build.rs`*

- **D4.1: emit a rational patch as `Geom_BezierSurface`, not `Geom_BSplineSurface`.** Even at the
  reduced degree 4 with finite, sane poles, `new Geom_BSplineSurface(poles, weights, uknots, vknots,
  umults, vmults, udeg, vdeg)` **segfaulted** in the constructor (the knot/multiplicity relation checked
  out — `Σmults = nPoles + degree + 1` — but something in the clamped-single-span setup faulted). A
  single-span rational Bézier patch is exactly `Geom_BezierSurface(poles, weights)` — no knots to
  author — and it builds, faces, and passes `BRepCheck` cleanly. The rational-Bézier *edge* path still
  uses `Geom_BSplineCurve` (works); only the surface switched. *2026-08-09 · resolved (`aaea9c0`) ·
  `crates/export/src/occt_shim.cc`*

- **The three-way `BRepCheck` box: for the one-joint fixture an exact LEDGE cap face cannot join a valid
  shell — so we don't emit one (M-D D3.3, Option B).** The exact §10 LEDGE body ships only the two
  certified flank sheets (identical to MITER) and emits **no exact cap face**; the cap survives only in
  the §11 mesh path. Why: the sole cap outline `export` has is the CAP-IN-D24 **licensing square** — a
  placeholder, not the real `V_∂`-projected cut — and for this fixture its crease edge coincides with
  the certified A+B miter seam `M` (the crease line `L`). A cap face can therefore meet the flanks only
  along `L`, and every way of expressing that in OCCT was empirically pinned to a dead end with minimal
  diagnostic breps (OCCT 7.9.3, `BRepCheck_Analyzer`): **(1)** share the crease **edge** → `M` becomes
  3-incident → **non-manifold**; **(2)** share only the crease **vertex** → a cone-point junction →
  `BRepCheck`-invalid; **(3)** share **nothing** inside one shell → disconnected shell →
  `BRepCheck`-invalid (`BRepCheck_NotConnected`). So a single 3-face shell for this fixture *cannot* be
  `brepcheck_valid` — a topological fact, not a bug. The certificate (`CLOSURE_VALID`:
  SEW-EDGES/SEW-LINK/CAP-OUT-LINK) proves seam-local, honest-open facts, **not** a valid
  closed/connected solid; the friction surfaces in `export` because that is where the abstract
  certificate becomes concrete OCCT-checked coordinates, and there is genuinely no certificate-backed
  flank↔cap seam to emit while the cap is the placeholder square. *Decision (with the user):* export
  whatever geometry is certificate-backed rather than fabricate a seam to get a good-looking STEP
  ("oracle ∧ audit, never oracle-instead"); the exact cap is **deferred to the `V_∂` real-cut slice**,
  which projects the true cut and gives a seam identity can share. The prior-session plan to emit a
  "vertex-shared exact planar cap" is retracted (outcome 2 above). *2026-08-09 · decided ·
  `crates/export/src/brep_build.rs`, `docs/vv-guide.md §8`*

- **Mesh LEDGE cap fanned unordered `face.outer` edges → degenerate cap, hidden by a false-green.**
  `shell::cap_tris` fanned `edge_start` of each `CapOut::region().faces[].outer` edge, assuming the
  boundary arrived loop-ordered head-to-tail. The arrangement (`arrange2d`) stores those edges
  **unordered** and with mixed orientation: the D24 cap square arrives as four segments whose starts
  are (2,0), (0,2), (0,0), (0,0). Fanning the starts alone dropped the (2,2) corner and doubled (0,0),
  so the cap covered only its lower-left half-triangle plus one zero-area triangle — which OCCT's
  BRepCheck rejects (`brepcheck_valid=false`). Fixed by `shell::ordered_ring`, which walks the edges
  through their shared endpoints (matching either orientation) into the true corner loop before
  `cap_tris` fans it. The bug was **latent, not a D3.2b regression** (the same degenerate triangle
  reproduces on the pre-flip fixture). *Process finding:* it went uncaught because the `--features
  step` leg — `occt_audits_the_one_joint_shell`, `one_joint_ledge_writes_a_reloadable_step_shell`,
  `differential::ledge_oracle`, all of which assert `brepcheck_valid` — was **never run with a real
  exit code** when M-D.2 and D3.2a were called green (a "step 28/28"-style claim that had not actually
  executed). Carried forward as a standing-gate requirement for all remaining D3.x phases: the
  `--features step` nextest leg must be in every green check with `${PIPESTATUS[0]}` verified.
  *2026-08-09 · resolved · `crates/export/src/shell.rs` (`ordered_ring`; commit `8ee76a4`)*

- **OCCT STEP-export shim de-risk (thin-M6 GO/NO-GO) — GO.** The `export` crate's off-by-default
  `step` feature builds a `cxx` shim to OpenCASCADE's `STEPControl_Writer`; the M6.0 smoke writes a
  unit box, reads it back, and `BRepCheck`s the reload — green under `nix develop` (OCCT 7.9.3, 350
  STEP entities). Two darwin gotchas worth remembering: (1) OCCT nests its headers in
  `<occt>/include/opencascade/` and they `#include` each other **unqualified**, so that dir must be on
  the include path *directly* (the nix cc-wrapper only injects `<occt>/include`); `build.rs` derives it
  from the `-isystem` token in `NIX_CFLAGS_COMPILE` (or `OCCT_INCLUDE_DIR`). (2) The STEP reader/writer
  moved to `libTKDESTEP` in OCCT 7.8+ (was `libTKSTEP`). The **stdlib-ABI question** resolved the
  opposite way from a naïve read: OCCT is prebuilt *libc++*, but the shim + the `cxx` runtime compile
  with the nix-default g++/**libstdc++** (forcing `clang++` fails — the shim's libc++ `rust::String`
  ctors don't match the libstdc++ cxx runtime). It links anyway because the shim only crosses the OCCT
  boundary via `const char*`/`double`/OCCT types — no `std::` object crosses — so libc++ and libstdc++
  coexist at load time. *Caveat carried to M6.3:* an OCCT exception (`Standard_Failure`, a libc++
  `std::exception`) caught by the libstdc++ `catch` in the shim is technically cross-ABI; fine for
  valid geometry, but the M6.3 writer should prefer status codes / validity pre-checks over relying on
  the catch for malformed input. *2026-08-08 · resolved · `crates/export/{build.rs,src/occt_shim.cc,src/step.rs}`*

- **CGAL boolean oracle extended to segment (polygon) operands — the C4 LEDGE cap lane.** The
  `cgal_boolean_*` shim only parsed disk operands (`C cx cy r2 operand`), so the LEDGE cap — a
  straight-edge *polygon* (cylinder rulings + crease) — had no CGAL region differential. Added an
  `L x1 y1 x2 y2 operand` boundary-edge line: per operand the edges accumulate into a CCW list and
  build a `Gps_circle_segment_traits_2::Polygon_2` via **direct** `X_monotone_curve_2(source, target)`
  construction (the traits' *linear*-segment ctor takes `Kernel::Point_2`, **not** the traits
  `Point_2` — that was the compile fight). Direct construction is required over `make_x_monotone`
  per-edge: the latter sorts each segment left-to-right and would flip right-to-left boundary edges,
  breaking the loop. Two live gotchas: CGAL **hard-aborts** (`SIGABRT`, uncatchable) on a non-simple
  polygon (a self-intersecting "bowtie" quad triggered it — the differential inputs must be verified
  simple + CCW), and axis-aligned (vertical) edges hit CGAL's vertical-segment special case, so the
  `ledge_cap_region_matches_cgal_polygon_boolean` cases use generic convex + simple-concave quads with
  no axis-aligned edge. Face count **and** exact `a+b√d` boundary geometry now match CGAL for the C4
  cap. *2026-08-08 · resolved · `difftest::cgal_shim` `boolean_components`*

- **Developable ≠ constant curvature.** A cone's nonzero principal radius is `R₁ = ρ·tan β`
  (ρ = slant distance from the apex) — *not* constant; only the cylinder has constant `κ₁`.
  This is why the mesh κ-cap is the domain minimum (the tightest radius, nearest the apex),
  not a value read off a fixed parameter station. A one-line property test caught the wrong
  "cone ⇒ symmetric ⇒ constant radius" assumption.
  *2026-08-04 · watching · `fixtures::devices::cone_principal_radius_shrinks_along_sigma`*

- **An exact cone development is transcendental — so the flat↔rolled morph is diagnostics-only.**
  The device cone's azimuth is `φ(σ) = 2·arctan σ − 90°` and its half-angle β is constant with
  `sin β` irrational (`n·ẑ ≡ 65/97`), so the isometric unrolling (flat angle `θ = sin β · Δφ`,
  flat radius = apex distance) lands outside ℚ. A *certified* development therefore cannot live
  in the rational kernel; it is future `develop`-layer work (M7). The viewer's morph is an honest
  `f64` unrolling in `export::mesh3d::develop_cone` (apex at the origin since `c ≡ 0`; the flat
  angle accumulates the true 3D angle between successive rulings = the directrix arc length on the
  unit sphere, which reduces to `sin β · Δφ` here). Empirically the certified strip develops to a
  **60.3° annular sector** (`= sin β · 90°`), and radius is preserved to machine epsilon. The
  development is float — it never touches a predicate, so it stays inside spec invariant 1.
  *2026-08-08 · watching · `export::mesh3d::flat_development_is_isometric_along_rulings`*

- **A genuine plane is not a `Chart` — the closure vertical slice is cylinder-first, not plane-first.**
  C0 recon (M4/closure) found the approved plan's "two-planar-flank" / `plane()` assumption false at the
  representation level: the spec (§, line 81) distinguishes a **`strip`** span (`|n′| > 0`) from a
  **`planar`** span (`n′ ≡ 0`, a coefficient identity), and `geom::chart::Chart` implements only the
  *strip* case — `Chart::new` debug-asserts `|n′|² ≢ 0` (`chart.rs:106`) and its whole field calculus
  divides by `|n′|²`. A plane (constant normal) has `n′ ≡ 0`, so it cannot be a `Chart`, and `geom` has
  no planar-span type. **Resolution:** the **cylinder** is the representable developable whose ruling
  cut-edges are straight *lines* (so CAP-IN-D24 passes and both closure branches run) *and* it carries a
  moving normal for the regularity bundle — so the M4 slice is built on it, with the **cone** as the
  contrasting conic class. The genuine planar-hub §13 petal disk waits on the planar-span type.
  *2026-08-08 · finding · `docs/closure-scoping.md §8`, `docs/vv-guide.md §8` (M4)*

- **EDGE-OCCUPANCY is asymmetric across the two closure branches — the LEDGE side recomputes it, no
  new `arrange2d` private surface.** M5.0 recon: the MITER branch carries the four-bit occupancy + frame
  bit *first-class* (`certify_core::miter::LedgerEdge.occupancy`, minted at M4), so its constructor
  (MITER-REGION-IDENTITY) just reads it. The LEDGE branch does **not**: the emitted `arrange2d::boolean::
  CapOut`/`Region` carry only `source: CurveId` + `orient` per edge; the transverse occupancy lives
  transiently in `boolean.rs` (`sector_mask`/`edge_flips` are private). **Resolution for ARRANGEMENT-BITS
  (adopted):** recompute the four bits in the `sew` searcher from the **public** surface — `label_cells`
  → `CellLabeling { labels, adj, seed }` (`boolean.rs:242`) + `separating_ids` (`:997`) — the spec's
  "projection of the §6 cell labels, four lookups". No `arrange2d` visibility change is needed for the
  cylinder slice; a minimal public `CapOut` accessor is added only if the recompute later proves to need
  the DCEL directly. The occupancy packet stays a `sew`-searcher product; `certify_core::sew` consumes it
  origin-agnostic. *2026-08-08 · finding · `crates/sew/src/*`, `docs/vv-guide.md §8` (M5)*

- **`DevConfig::terms` is NOT the transcendental bottleneck — and four attempts to tame the bignum
  traffic all failed.** Recorded as a NEGATIVE result so it is not re-derived. Re-profiling in an
  OPTIMIZED build (after the profile fix) inverted the picture: no `arctan`/`cos_on`/`sin_on` frame
  appears at all; ~a third of samples are ALLOCATION (malloc/free/memmove/memset/`Repr::clone`/
  `Repr::drop`) and the rest is dashu BIG-integer work (`lehmer_guess`, `gcd_large_dword`,
  `mul_large`, `UBig::div`). `lattice::small` — which dominated the UNOPTIMIZED profile — is now
  minor. Two profiles of the same code disagreed about the hot spot because one of them was
  measuring the wrong build.
  Tried, all reverted: (1) `ROUND_BITS` 60→30 — **30% faster and rejected**, because VV.1's
  `the_fold_takes_the_seeded_bracket` failed "seeded 0, bisected 8": at 2^-30 the enclosure is too
  wide to verify and MAP.1's fast path silently degrades to bisection, same certified answer, quietly
  weaker. That test's doc says it is "the only check that would notice"; it was. (2) rounding the
  product before adding in `point_from_on`, 6 sites — no gain. (3) rounding between Horner's multiply
  and add in `eval_poly_on` — no gain. (4) large polynomial coefficients — disproved, 18/9/4 digits.
  The arithmetic made (2)/(3) look compelling: `SmallRat` is `i128/i128`, so at 60-bit operands ONE
  op fits and the second unrounded op overflows to bignum. Shortening the chains should have helped
  and measurably did not — so either promotion is not the cost, or the allocation traffic comes from
  somewhere else. **That is the question any future attempt must answer before optimizing**, and it
  is why the follow-ups are filed (#257) rather than attempted.
  The one clean lesson: a speedup that a gate rejects is not a speedup. (1) would have shipped a 30%
  win that quietly disabled a certified fast path.
  *2026-08-16 · finding · task #233 → #257*

- **The test suite had no `[profile]` section at all — every test ran at `opt-level = 0`.** The
  single largest factor in the suite's runtime, and it was configuration, not code: **185.0s → 25.6s
  (7.2×)**; the heaviest test 157.6s → 22.0s. Counters byte-identical (2 256 / 2 640 / 4 096), so
  only code generation changed.
  **HOW IT SURFACED — the profile named it, twice removed.** A flat `sample` profile showed
  `copy_nonoverlapping::precondition_check` 537, `is_aligned_to` 361, `from_raw_parts::
  precondition_check` 283, `ub_checks::maybe_is_nonoverlapping` 250 — ~6% of samples in **std's debug
  preconditions**, which exist only in an unoptimized build. Those lines are not a cost to optimize;
  they are a *fingerprint* of the whole build being unoptimized. Reading a profile for what its
  presence implies, not just for its hot rows.
  KEPT ON DELIBERATELY: `debug-assertions` and `overflow-checks`. A two-tier lattice that silently
  wrapped would be a correctness bug, and these are the tests that would catch it; `opt-level` is
  orthogonal to both.
  **SCOPE — this is a developer-time win, NOT a product win.** Demos and production already build in
  release. OPT.2.1/2.2 were genuine engine wins that help release too; this one only shortens the
  measure-fix-measure loop. Worth separating, or a 25× headline gets attributed to the engine.
  **LTO DOES NOT PAY HERE** (checked, since the crate chain lattice→geom→develop→export→author looks
  like a cross-crate-inlining candidate): fat LTO + cgu=1 ran 22.9s but cost ~112s to build vs ~74s —
  ~11% run for ~50% build, a loss for iteration AND for CI, which builds cold anyway. Thin LTO was
  dominated on both axes (27.1s run, ~127s build). **The ceiling is low for a structural reason: the
  hot arithmetic is generic over `Backend`, so `Rat`'s methods are monomorphized into each consuming
  crate — cross-crate inlining already happens without LTO.** O3 gives a real but marginal ~3%
  (7.61s vs 7.83s on the heavy test, repeatable) for ~8% more build; kept O2, and note the ambient
  run-to-run variance (~5%) is larger than that gap.
  *2026-08-16 · finding · `Cargo.toml`, task #233*

- **A vector-valued integrand integrated component-by-component evaluates itself once per
  component.** OPT.2.2. `directrix_accumulated`'s cell integrated `γ` by calling
  `integrate_on_slope` twice — once with `|p| directrix_velocity(..).map(|f| f[0])`, once with
  `f[1]`. But `directrix_velocity` (and `directrix_accel`) return **both** components in one call, so
  each was computed twice and half of each result thrown away. **Every γ integrand evaluation
  happened twice.**
  **MEASURED:** `gamma_velocity` 4 896 → **2 640**; `self_lapping`'s develop **85.1s → 58.9s (−31%)**;
  `flex_panel` (γ≡0) unchanged at 25.8s, which is the check — a change that sped up the γ≡0 fixture
  would mean something other than the γ integrand had moved. `gamma_cells` and `cut_evals` both
  unchanged. The residual decomposes exactly: solving `2a + b = 4896`, `a + b = 2640` gives
  a = 2 256 (one per γ cell, precisely the cell count) and b = 384 (96 lift-bound edges × subdiv 4,
  the direct `directrix_between_on` tail term) — nothing unexplained.
  FIX: `integrate_on_slope_n`, the same slope rule generalised over `N` components, evaluating the
  integrand once per point. The scalar form is now the `N = 1` wrapper, so there is ONE
  implementation of the rule and no second copy to drift.
  **THE PROCESS POINT, which is why this was found at all.** The plan was "make each evaluation
  faster" (the transcendental series at `terms: 14`). Settling *is the count honest?* first was what
  exposed the doubling — a per-eval speedup would have multiplied against a doubled base and left
  the waste in place, permanently, looking like a win. Three hypotheses died on the way: anchor-piece
  over-splitting (exactly 1 piece/edge), `develop_arc` (224 of 4 896), and the code path itself
  (predicted 384 against 3 392 measured — the 8.8× gap that had to come from somewhere).
  **AND IT WAS ONLY VISIBLE BECAUSE THE COUNTER HAD JUST BEEN ADDED** one commit earlier: wall clock
  cannot see it, and `gamma_cells` counts *cells*, which never changed. The instrumentation gap and
  the waste were the same finding twice.
  RIDER: the first budget for the new counter was 1.4× of the *pre-fix* 4 896 = 6 900, which could
  not have caught a revert (4 896 < 6 900). Re-baselined to 3 700. A budget that cannot detect the
  regression it was written for is decoration.
  *2026-08-15 · finding · `crates/develop/src/{interval,cone}.rs`, task #233 (OPT.2.2)*

- **Rational ADDITION multiplies denominators — five unrounded ops turned 18 digits into 120, on
  every evaluation.** OPT.2.1. `develop::cut` carried ZERO `.rounded()` calls against `cone.rs`'s 37
  and `interval.rs`'s 16; it never adopted the DEV.2a outward-rounding discipline. `eval_ratfunc_on`
  rounds, so chart fields arrived ~18 digits — but `chart_point_on`'s `p + µ̂·r + w·n` is five more
  ops, and adding coprime-denominator rationals multiplies the denominators, so the point came out
  ~120 digits. `metric_distance_on` then squared and summed those and took an exact rational √,
  whose enclosure must *narrow* as its input box narrows — so finer subdivision bought hundreds of
  digits instead of a tighter answer, reaching **499 digits at subdiv ≥ 64**.
  **MEASURED:** cut-certificate path 8.5–9× faster (77.2→8.6s, 163.3→19.3s), whole develop 2.9–3.9×
  (99.1→25.1s, 248.0→84.3s), with **`cut_evals` IDENTICAL** (6144, 4096) — pure cost-per-operation.
  Cost: `2^-60 ≈ 8.7e-19` per op against ε ≈ 0.15; VV.2's pinned ε and VV.1's counters both pass
  unchanged.
  **THE METHODOLOGICAL POINT, which is the durable part.** `develop::counters` counts γ cells and cut
  evals — and total time was ~LINEAR in subdiv, so a count-based reading said "constant cost per
  operation, nothing here". The constant was not constant: it doubled at subdiv 64 when operands
  crossed into bignum. Counts are the right *gate* (machine-independent, cannot flake — VV.1's whole
  rationale) and are **insufficient for diagnosis**. What found this was measuring operand SIZE
  (`numer_denom_decimal().len()`) at each link of the chain: t-endpoints 2, σ 11, µ̂ 13, chart_point
  **120**, distance **499** — the inflation is localised to the two sites with no rounding.
  Pinned by `the_certificate_chain_keeps_its_operands_bounded`, mutation-verified: deleting one
  `.rounded()` returns chart_point to 119 digits and fails.
  **Also corrected en route:** OPT.0's "cut certificates are ~7% of runtime" was STALE — it measured
  the *demo* before the p-curve milestone multiplied the hole path's node count. On the test payloads
  `certify_holes` was 66–78% of a develop. And γ is no longer hot: `gamma_cells` is 0 in both
  boundary and holes on both fixtures; OPT.1 did its job. After this fix the profile INVERTS again —
  unroll + flat boolean + topology is now 63–72%.
  *2026-08-15 · finding · `crates/develop/src/cut.rs`, task #233 (OPT.2.1)*

- **A constraint I asserted twice did not exist — the helper had ossified into a believed property.**
  Designing the `Profile` builder I claimed circles need a *rational radius*, because an arc's
  extreme points must be named exactly, and offered the user a refuse-or-bracket decision for a
  squared-radius constructor. Wrong on both counts. `Surd::new(a, b, d)` is `a + b√d` for any
  rational `d ≥ 0`, so an extreme point is exactly `Surd::new(cx, ±1, r2)` — and `arrange2d`'s own
  `decompose::extrema` **already computes precisely that**. The belief came from the test helpers,
  which happen to take a rational `r` and pass `Surd::from_rat(cx ± r)`; I generalised the helper's
  shape into a property of the arrangement. Verified the correction against `r² = 1/40` (the device
  drill's own, irrational) end to end. Consequences: `circle_r2` is the primary constructor and
  `circle(r)` the sugar, and the builder is ~80 lines of `Curve` + `decompose` rather than a fourth
  hand-rolled decomposition. **The pattern is [[no-interface-ossification]] in its cheapest form** —
  an example hardening into a constraint — and the tell was that I could state the limitation but
  not point at the line enforcing it. *2026-08-15 · finding · `crates/arrange2d/src/profile.rs`*

- **Scripted inserts anchored on "nearest preceding `///`" cut into the neighbouring doc comment.**
  Twice now. In `23276f9` an insert placed a helper *inside* the doc block of
  `a_polygonal_slot_is_not_dropped_between_sample_cells`, truncating it mid-sentence ("The result
  was") — and it shipped, because a mangled doc comment compiles. Found only when a later edit to
  the same region failed to parse. Anchor scripted edits on **unique whole-line matches** with an
  asserted occurrence count, never on a scan backwards for a comment marker.
  *2026-08-15 · finding · `crates/author/src/resolve.rs`*

- **A gate that compiles gated code *out* is not a gate over it — `cargo xtask gate`.** The default
  `--workspace` clippy/nextest legs pass no `--features`, so `export`'s `step` items, `difftest`'s
  `cgal` items and `lattice`'s `fuzzing` items are absent from the everyday loop entirely. PC.5 and
  PC.6 each broke `full_panel_solid_exports` and it went unnoticed locally for the whole OPT/VV/MAP
  arc. The new `cargo xtask gate` mirrors the CI step list and runs `clippy --all-targets` on **each
  feature combination** (which compiles the gated *tests*, the part that was missing); `--full` adds
  the test legs. Verified by mutation: a deliberate break inside the step-gated test leaves the plain
  `clippy` leg **green** and fails only `clippy --features step (export)` — the exact PC.5/PC.6
  signature. Two design points worth keeping: steps the gate does not run (Kani, dylint, the Lean
  audit) are **named in the summary with their commands**, because a gate that quietly covers less
  than it appears to is worse than one that covers less and says so; and it found a real hit on its
  first run — `ratfuzz.rs`'s `unreachable!()` violates the pure tier's panic-freedom deny, which no
  CI step evaluated because CI runs `cargo test` on `--features fuzzing`, never `clippy`. Fixed by
  making `3 => …` the catch-all (`opcode % 4` is `0..=3`), removing the panic path rather than
  discharging it. **What it does NOT close:** platform-specific *runtime* linkage, e.g. #241(c)'s
  Linux doctest failure — that is invisible on macOS at any feature setting.
  *2026-08-15 · finding · `xtask/src/main.rs`, `AGENT.md` (#242)*

- **A quantity no test reads quantitatively is unverified, however many tests run through it.**
  `Extrusion::extent` (AUTH.1f) bracketed segment endpoints with `arrange2d::locate::rational_above`
  — a strict upper bound found by *doubling from zero* — and used it for **both** sides of the box.
  A square at `(0, 11/5) ± 1/5` came out `[0, 1] × [3, 3]`: zero height, containing none of the
  profile. Everything built on it (`bounding_wall`'s σ-window, `reference_point`'s span ray) was
  therefore wrong for every polygonal profile. It survived two slices because nothing *read* the box:
  the arc path derives its extent from an exact centre and never calls `rational_above`; the
  polygonal-slot test only asserts the role is not `Inactive`; and `reference_point` is
  short-circuited for a `Through` span. AUTH.1e.4 was the first consumer that needed the number to be
  right, and it surfaced as a `ShadowUnbounded` refusal on a cut that should have realized.
  **Fix:** `[rational_below, rational_above]` bisected to `2⁻⁴⁸` — the raw doubling bracket is sound
  but answers at integer scale, and a bounding circle an order of magnitude too big is its own
  problem. Regression test asserts the extent brackets the corners *tightly*, and was checked against
  a re-introduction of the bug.
  *2026-08-15 · finding · `crates/author/src/part.rs`, `docs/cutter-extrude-design.md` §6.3 (AUTH.1e.4)*

- **A cut piece can be certified against the right *surface* and still be the wrong *boundary*.**
  AUTH.1e.4's multi-wall loop reads each ruling's boundary from every wall's crossings, classified by
  the profile's own fill rule, so the governing wall changes at every profile corner. The obvious
  design — bisect each governing-wall change, insert a node, certify each piece against the wall its
  endpoints name — is sound only where the corner search succeeds. Where it misses one (two corners
  inside a node interval), the emitted chord **stays on wall A the whole way and certifies perfectly**
  while the true boundary dips onto wall B beneath it: a hole quietly too large, with a green
  certificate. `pcurve_cut_fit` cannot see it, because distance-to-a-surface is not the claim being
  made. **Resolution:** every piece is additionally compared at its own σ-midpoint against the
  boundary the exact fill rule reports there, and the deviation folded into ε — so soundness rests on
  the fill rule (exact) and the corner search only buys tightness. Worth generalizing: whenever a
  certificate bounds distance to *one* member of a family, ask what pins the choice of member.
  *2026-08-15 · finding · `crates/develop/src/cut.rs`, `docs/cutter-extrude-design.md` §10.3 (AUTH.1e.4)*

- **The negative control fired through a different door than expected, which is itself the finding.**
  To check the square-prism band test was not vacuous, the profile was mutated 1.5× larger. It failed
  — but on `"the square's loop must certify"`, not on the inscribed/circumscribed bounds: an enlarged
  profile overruns the bounding-circle window it was handed, and the loop refuses `ShadowUnbounded`
  rather than emitting a band that reaches the window's edge. The fail-closed path works, and the
  band-size assertions were left unproven by that mutation. What settled them instead was printing the
  measurements: band 0.2364/0.2342/0.2323 against inner 0.1615/0.2303/0.1639 — a 1.7% squeeze at the
  middle ruling, plainly neither disc. A mutation that trips a *different* guard has not exercised the
  assertion you were testing. *2026-08-15 · finding · `crates/develop/src/cut.rs` tests (AUTH.1e.4)*

## Deferred (by milestone)

- **DEV / Tier-C — the transcendental ANCHOR backward-error bound (its own milestone, M-E).** D4.3 Stage 1
  certifies the **exact** part of ANCHOR (`spec:372`: positive width, boundary regularity, σ̂-monotonicity —
  all Sturm) and closes the exact-over-anchor solid. The **transcendental** part — the backward-error bound
  `sup|D(â) − g| ≤ ε` (fidelity of the developed boundary `D` to an authored flat drawing `g`) via the
  development map `D = γ + μ̂·ρ·e(ψ)`, where `ψ = ∫ψ′` (→ arctan/log), `γ = ∫e(ψ)` (a nested transcendental),
  `ρ = |n′|` (a radical) — plus the DRC `ε < clearance/2` (`spec:402`) need a rigorous
  transcendental-enclosure tier (interval/Taylor-model `ψ`/`γ`/`D` bounds). That is a whole new tier, gated on
  its own GO, **not** a D4.3 slice. **Priority note (driving requirement, see `docs/implementation-plan-v1.md §6`):
  this is HALF THE PRODUCT, not a fidelity nicety — the certified flat↔3D development is the keystone *both*
  product directions pivot on (① develop 3D→flat generates the flat PCB outline; ② fold flat ECAD→3D). Weight it
  as a primary thread (co-equal with the D4.4 atlas), not a tail deferral.** *2026-08-09 · deferred(→DEV / M-E),
  reprioritized 2026-08-10 · `spec:372`,`spec:402`, `develop` crate, `certify_core::free_boundary`*

- **General `(σ(t), μ(t))` authored outlines + the polynomial-composition primitive.** D4.3 Stage 1 builds the
  **σ-band** free boundary (`μ⁻(σ), μ⁺(σ)`, all functions of the same σ). An arbitrary planar outline needs
  `c ∘ σ(t)` — a `Poly`/`RatFunc` **composition** primitive the repo lacks — plus an N-edge top/bottom cap
  subdivision (the σ-band's 2 `Plane` σ-caps become an N-gon fan). Build the composition primitive when the
  general contour is genuinely needed (atlas-scale authored petals), not before. See the D4.3 finding above.
  *2026-08-09 · deferred(→D4.3 general-contour follow-on) · `lattice::{Poly,RatFunc}`, `export::brep_build`*

- **STEP export of a *certified-closed* body as a `CLOSED_SHELL` / `MANIFOLD_SOLID_BREP`.** D4.1's
  `write_brep` emits the closed slab as a surface model with an `OPEN_SHELL` (the C++ builder never
  stamps the `TopoDS_Shell` `Closed` flag), even though `closed_shell` certifies it closed and OCCT
  agrees `free_edges == 0`. Since the certificate is the authority, a caller holding
  `Verified(ClosedShell)` may legitimately stamp `shell.Closed(true)` + wrap in a solid so the STEP
  declares it closed — gated on that certificate so honest-open MITER/LEDGE bodies stay `OPEN_SHELL`.
  Do this the next time STEP emission is touched, not as its own slice.
  *2026-08-09 · deferred (next STEP-emission pass) · `crates/export/src/{occt_shim.cc,step.rs}`*

- **The planar-span representation (`n′ ≡ 0`) — a `PlanarChart` / relaxed `Chart` with its own
  pedal/ruling calculus.** A `geom`/M1-adjacent feature; unblocks the *genuine* §13 planar-hub petal disk
  (the closure slice uses the cylinder as the representable line-carrier stand-in meanwhile). See the
  finding above.
  *2026-08-08 · deferred(→M-C petal pass / M1-adjacent) · `crates/geom/src/chart.rs`*

- **Petal conical-flank fixture + the `cx-cone-flank-trim-mu` corpus entry.** Spec §13
  geometry is not yet pinned; needed for closure/sew.
  *2026-08-04 · deferred(→M-C) · `fixtures/corpus.md`*

- **Algebra-trust rehaul.** Opaque `Int=ℤ` / `Rat=ℚ`, a reference bignum, its Lean
  equivalence proof, and a dashu differential stress-test.
  *2026-08-04 · deferred(post-B) · `docs/algebra-trust.md`*

- **SLAB-S1 / QPOS Bernstein positivity.** No Bernstein primitive yet.
  *2026-08-04 · deferred(→M4) · vv-guide §8 (B deferrals)*

- **FRESH promotion (three-way containment re-test) — deferred out of thin M6.** FRESH keys on the
  fab-gating stamp fields (`materialStripWidth`): a regenerated enclosure ⊆ stamp ⇒ green, disjoint ⇒
  stale hard-fail, partial ⇒ undecided (spec:203). That is a `VALID_material` / material-grade concern,
  not `VALID_solid-closure` — the thin-M6 gate proves solid-closure only. The M6.2 certificate store
  ships a documented FRESH *stub* (the provenance chain rule it enforces — a stamp bounded below by its
  sources' certified enclosures, never a naked float — is the FRESH precondition, but the re-test
  itself is not built). *2026-08-08 · deferred(→M-E material grade) · spec:203, `certify_core::gate` / `gate`*

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
  *2026-08-15:* revisited with the user while fixing the axiom gate — decision is to **keep
  Sturm cited** and formalize it opportunistically, not now. The citation is no longer a red
  CI signal: `scripts/check-axioms.sh` accepts it *on `verify_chain_sound` only*, and fails
  the moment it appears anywhere else or stops appearing at all. So the assumption is pinned
  and visible rather than either hidden or noisy.
  *2026-08-04 · watching · `docs/proofs/ledger.md`, `scripts/check-axioms.sh`*

## Resolved

- **Invariant 1 consolidated onto one type-aware dylint lint — the `xtask` token scan is gone.** *Done
  2026-08-08 (branch `no-float-dylint-only`):* invariant 1 (no floats in certified paths) was guarded by
  **two** overlapping lints — the `cargo xtask lint` `f32`/`f64` **token** scan and the `no_float` dylint
  **literal** check. The token scan was comment-blind (it false-positived on Phase-1 doc prose — `` `f64`
  cast ``, which prompted this) and couldn't tell lib code from test code, so it over-policed
  `#[cfg(test)]`/`tests/` and needed a `testgen.rs` carve-out that only existed to escape it. Fix: extend
  the dylint lint with `check_ty` (matches `Res::PrimTy(PrimTy::Float(_))`) so it now catches float
  **types** — fn sigs, fields, casts, generic args (`Vec<f64>`), and type-relative paths (`f64::EPSILON`,
  caught as a `Ty` node — no extra `check_expr` arm needed) — **and** the existing literal check; then
  delete `fn no_float`, its report call, its unit test, and the now-dead `contains_word`/`is_word_byte`
  helpers from `xtask`. The dylint lint is now the sole invariant-1 gate. UI fixture `ui/lattice.rs`
  exercises all nine cases (3 literals + fn param/return + field + generic + cast + assoc). *Decision —
  floats are allowed in tests:* the ban's real scope is the certified **predicate path** (AGENT.md inv 1;
  vv-guide §6), not test code. A test float can't reach a predicate (`cfg(test)` is compiled out), and
  floats are *useful* in tests — independent `f64` **oracles** (compute expected, assert the exact result
  matches — the highest-value float use in an exact kernel), input **generators**, readable expectations.
  Scoping to lib targets (`-p`, no `--all-targets`) is not a gap but the intended policy. *Accepted edge:*
  dylint doesn't lint **doctests**, so a float in a `///` example is unguarded — style, not soundness (a
  doctest float can't reach a certified predicate either). *Trade-off:* dylint is now the only gate, on the
  rustup-nightly CI leg; a broken dylint toolchain surfaces as **red CI** (loud, blocks merge — not a
  silent gap), and local float-checking now means running `cargo dylint`, not `cargo xtask lint` (noted in
  AGENT.md). *2026-08-08 · resolved · `lints/no_float/`, `xtask/src/main.rs`, `ci.yml`, AGENT.md inv 1,
  vv-guide §6*

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
