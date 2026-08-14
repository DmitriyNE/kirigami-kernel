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

- **A developed circular hole has slightly-flattened tangent points — an irreducible √-branch limit, not a bug.** A circle's boundary is double-valued in σ (near/far branches meeting at the two tangent rulings, where `μ̂` has a vertical tangent — a √-branch point). A polynomial rail cannot match the vertical tangent, so the near/far fits stop meeting a small gap short (~0.06 μ̂ on a ~0.4-tall hole, floor independent of margin/degree); the two branches are joined by a tangent micro-cap → the developed hole's two points are slightly flattened (an exact, watertight `Cap`). Fully-exact alternative (AlgReal-σ `BoundaryArc`, or a per-point developed polyline) deferred. *2026-08-11 · watching · branch `roadmap-flex-pcb`*

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
  *2026-08-04 · watching · `docs/proofs/ledger.md`*

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
