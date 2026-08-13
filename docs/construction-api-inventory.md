# Construction-API inventory & design sketch

*Scouting pass (task #217), 2026-08-12. Three read-only surveys — demo→engine candidates, the
current public authoring surface, and a focused V&V gap scan — synthesized here. This is the basis for
the `Part`/builder API design (#218); the API sketch below is a **proposal pending sign-off**, not settled.*

## Headline diagnosis

**`crates/export/examples/self_lapping_cone.rs` (918 lines) is a hand-written `Part` builder.** Its
`SelfLapping`/`WrapDemo`/`Outline`/`DrillHole` types encapsulate exactly the region-gluing, cut, develop,
fold, and emit logic a `Part` abstraction should own. The general engine underneath — `Chart` +
`ConeDevelopment` + the `(σ,µ̂)` `BoundaryArc` loop + `unroll`/`fold` — is clean and genuinely general.
**The gap is a thin sugar layer, not new geometry.** There is no `Part`/`Panel`/`Builder` type anywhere
in `crates/*/src`; every end-to-end artifact is a bespoke binary that re-threads 6–12 imports and
re-derives the same boilerplate. The strongest extraction signal is that the **3D/STEP half already
exists in the engine** (`export::brep_build::brep_trim_solid_regions` consumes the piecewise
`charts + inner/outer rails` shape), while the **flat half lives only in the demo** — so extraction
closes an asymmetry rather than inventing an abstraction.

## Extraction map (demo → engine home)

Ranked by leverage. "Home" is the proposed destination; "why trapped" notes the missing composition.

| # | What (file:line) | → Engine home / API | Why it's trapped |
|---|---|---|---|
| **A1** | Piecewise-support development orchestrator `SelfLapping` (`self_lapping_cone.rs:218–449`): `gamma_grid`, `point_at`, `region` routing | `develop::part::PiecewiseDevelopment` — owns region partitioning + the cumulative-γ running frame; `point`/`gamma_at`/`outline` delegate to the public `directrix_between`/`point_from` | The **per-region atoms are public and general** (`cone.rs:directrix_between`, `point_from`); only the multi-region driver that routes σ and accumulates the running γ base is demo-only. |
| **A2** | Surface∩cylinder hole from the **true** 3D surface `drill_hole` (`self_lapping_cone.rs:483–532`) | Generalize `export::trim::hole_loop` → one pedal-general `surface_cylinder_hole(chart, cx,cy,r2, band, segments)` | `hole_loop` uses an apex-ray `tangent_poly` that **assumes a cone through the origin** — silently wrong on an offset tail / under the wrap. The demo re-implements it correctly (comment at `:487`). A correctness upgrade, not just a move. |
| **A3** | Certified-STEP emit + OCCT audit report (copy-pasted **4×** + 2× in `trim.rs` tests) | `export::step::emit_certified_step(path, solid) -> StepReport` | Identical `to_shell_certificate → closed_shell_holed → write_brep → audit_brep → print` sequence everywhere. The single biggest duplication in the tree. |
| **A4** | Per-region certified cut rails `cyl_rails`/`trim_rails` (`self_lapping_cone.rs:140–207`) | `export::trim::certified_rail_piecewise(bands, disk, fit, clearance, cfg)` | Single-region `certified_rail` + `annulus_loop` are engine; the piecewise-over-bands composition (flat-side analogue of `brep_trim_solid_regions`) is not. Pairs with A1. |
| **A5** | Physical-xy disk-arrangement certification `certify_arrangement` (`flex_panel.rs:225–266`, verbatim in a `trim.rs` test) | `export::trim::certify_disk_arrangement(disks, op) -> Verdict<Region, …>` | Reusable authoring precondition; its exact logic already appears in an engine test but has no home. |

**Borderline (extract after the core, or fix in-engine):**
- `seam_drill` (one 3D feature → N flat images) → `part.drill(cyl) -> Vec<HolePoly>` after A1/A2.
- `fold_flat`/`hex_poly` (`:583–603`) — a **hand-rolled** closed-form flat→(σ,µ̂) inverse that *bypasses*
  the certified `develop::fold`. The real gap is that `fold_point`/`fold_outline` don't yet accept a
  piecewise/wrap developable → **tech-debt against `develop::fold`**, not an extraction.
- `ramp_support` cubic smoothstep → `develop::support::smoothstep(σ_a,σ_b,D)` (low urgency).
- `RailFit{degree:4,subdiv:256,bits:44}` (5 sites) → named `RailFit::occt_low()`.

**Genuine demo glue (leave, or one shared util):** `verdict_tag`/`bail`/`why` printers (5 files);
`qf`/`e3`/`to_f64` float-conv (6 files → should call the existing `export::approx::rat_to_f64`);
`plane_rail` (dup of `develop::cut::plane_cut_rail`); projection/formatting; arg-parsing.

## Ergonomic seams the `Part` API fixes (ranked)

1. **No authoring type** — every demo hand-rolls one. *(the whole point)*
2. **Config is raw and re-threaded at every call** — `DevConfig`, `RailFit`, `clearance/segments/panels/iters` are positional args stage after stage; `outer_loop`/`hole_loop`/`fold_outline` each carry `#[allow(clippy::too_many_arguments)]`. Set one budget once.
3. **The apex-side sign is a leaky `bool`** — `fold_*` take `mu_negative: bool`; `ConeDevelopment` exposes `point` (|µ̂|) vs `point_signed` (signed) that *must not be mixed on a connected boundary* (doc warns). The region's side is knowable; the builder should own it.
4. **Piecewise-support / γ-grid gluing has no public helper** — the core product capability is stitched by hand (A1).
5. **Two cut-hole worlds, one silently wrong off the apex cone** — `hole_loop` (apex-ray) vs the demo's true-surface `drill_hole` (A2). Exactly the frozen special-case the *no-interface-ossification* principle warns against.
6. **The cut/trim authoring verbs are behind `#[cfg(feature="diagnostics")]`** — a default build cannot cut a boundary. The feature name (viewers/plots) misdescribes what it gates (the float-*proposal* authoring layer; the certificate stays float-free regardless).
7. **Verdict handling is copy-pasted** — no `Verdict` combinator (`.expect_verified("stage")`), so every stage boundary is a hand-written 3-arm match.
8. **STEP needs a second, differently-shaped rail representation + a re-fit** — `trim_rail_chains`/`hole_rail` + a low-degree `RailFit{degree:4}` re-fit (OCCT f64 edge tolerance rejects the default degree-6 rails). Same panel fit twice; inner/outer naming even swaps between the SVG and STEP worlds.
9. **`Chart` is authored as raw quaternion polynomials** — no `Chart::cone(half_angle)`/`Chart::cylinder(axis)`; the only cones are six fixtures. `ConeDevelopment::new` returns bare `None` with no reason.
10. **Fixed-topology boundary constructors ossify each case** — `outer_loop` = exactly "D1−D2−D3-notch"; adding a D5 means a new bespoke function, though the general substrate (`Vec<BoundaryArc>` + `unroll_trim_loop`) already exists.
11. **`FlatBox`→polygon reduction is manual/duplicated and drops the ε box** — two `cut_hole` (Xor-one) vs `assemble_flat` (Diff-many) entry points.

## Proposed `Part` builder (draft — pending sign-off)

Intent: **one authoring context over the one general engine.** Threads the budget once, owns the
apex-side sign per region, encapsulates the γ-grid gluing, and *delegates every geometric decision to the
existing certified functions* — invents no geometry, freezes no topology. Cuts accumulate into the
**general** `Vec<BoundaryArc>` + hole list, so N cuts need no new function.

```rust
pub struct Part<B: Backend = Bignum> {
    dev: ConeDevelopment<B>,          // built once (γ=0 or γ≠0)
    budget: DevConfig<B>,             // set once
    fit: RailFit,                     // set once
    clearance: Rat<B>,
    side: Side,                       // owns µ̂-sign; replaces the mu_negative bool
    regions: Vec<Region<B>>,          // piecewise support, γ-grid encapsulated
    boundary: Vec<BoundaryArc<B>>,    // the ONE general loop
    holes: Vec<HoleSpec<B>>,          // xy-cylinder OR flat-polygon, unified
}

impl Part {
    pub fn cone(half_angle: Rat<B>) -> Self;                       // no raw quaternions
    pub fn from_chart(chart: &Chart<B>) -> Result<Self, PartFault>;// wraps None → reason
    pub fn support_ramp(self, band: Band, h: RatFunc<B>) -> Self;  // piecewise support
    pub fn budget(self, cfg) -> Self;  pub fn clearance(self, c) -> Self;
    pub fn band_side(self, s: Side) -> Self;                       // owns the µ̂ sign

    pub fn cut_outer(self, disk: TrimDisk<B>) -> Self;             // → certified_rail, append Rail arc
    pub fn cut_inner(self, disk) -> Self;  pub fn notch(self, disk) -> Self;
    pub fn hole_cylinder(self, disk) -> Self;                      // true surface∩cyl (fixes #5)
    pub fn hole_flat(self, poly: &[[Rat<B>;2]]) -> Self;

    pub fn develop(&self) -> Verdict<FlatPattern<B>, PartFault, Rat<B>>;  // γ-grid internal (#4)
    pub fn fold(&self, flat, w) -> Verdict<FoldedWire<B>, …>;             // sign from self.side (#3)
    pub fn solid(&self, w: &Interval<B>) -> Verdict<Brep<B>, …>;          // STEP re-fit internal (#8)
}
impl FlatPattern { pub fn svg(&self, px) -> String;  pub fn eps(&self) -> &Rat<B>; }
```

Faithfulness: the builder holds exactly one `ConeDevelopment` and emits into exactly one
`Vec<BoundaryArc>` + one hole list — the same representations `unroll_trim_loop`/`assemble_flat` already
consume; `.develop()/.fold()/.solid()` delegate verbatim to `unroll_trim_loop`/`fold_outline`/`brep_trim_solid`.
The only *new* behavior it owns is (a) the region γ-gluing lifted out of the demo and (b) the STEP
low-degree re-fit lifted out of the demo — both currently duplicated, neither new geometry.

**Two prerequisites the sketch implies:** the cut/trim authoring layer moves out from behind
`diagnostics` (seam #6), and `hole_cylinder` reads the true surface (seam #5 / A2).

## Coupled V&V gaps (focused-pass candidates, #220)

The certificate stack is **not float-backed by design** — no checker rests solely on a float oracle, so
these are bounded. Tied to this API work:
- **A2 fixes a real correctness gap** (`hole_loop` apex-only) — the strongest "close it while extracting".
- **`RailFit` Vandermonde ill-conditioning near σ-origin** (`trim.rs:520`) — degree ≥4 monomial fits
  explode under interval-Horner (the notch is forced to degree 3; the D2 ramp fit ε≈0.15 is the same
  class). A Chebyshev/Bernstein-basis fit would remove the cliff and improve every cut rail — high-value,
  directly under the cut API.
- **Not blocking the API but adjacent:** general `(σ(t),µ(t))` outlines need a `lattice` polynomial-
  composition primitive (real ECAD contours); free-form directrix ANCHOR tier; `PlanarChart` (`n′≡0`).

Separate from this work (deferred to the proper V&V sweep): CLEAR brute-force, mesh κ-cap
representation-conditional, FRESH stub, Sturm axiom, CAP-OUT⇒manifold open theorem.

## Open design decisions (for sign-off)

1. **Where does `Part` live?** New `author` crate (depends on develop+export; the clean top-level
   surface — one import for users) vs. inside `export` (already depends on develop; fewer crates) vs.
   split (`develop::part::PiecewiseDevelopment` core + a thin top-level facade).
2. **Feature strategy for the cut/trim authoring layer** — move the float-*proposal* layer into the
   default build (certificate stays float-free) vs. a new `authoring` feature distinct from `diagnostics`
   (viewers) vs. leave gated and require the feature.
3. **First build step** — safe high-duplication consolidations first (A3 `emit_certified_step`,
   `RailFit::occt_low`, float-conv → `export::approx`, generalize `hole_loop`→surface∩cyl) then the
   `PiecewiseDevelopment` core then the `Part` facade; vs. core-first.

## Proposed sequencing

0. *(done)* scouting inventory (this doc).
1. **Consolidations** (A3, A5, `RailFit::occt_low`, float-conv, generalize `hole_loop`) — safe, independent, declutters the demos and closes the A2 correctness gap.
2. **`PiecewiseDevelopment`** (A1) + `certified_rail_piecewise` (A4) — the flat-side piecewise core.
3. **`Part` facade** — the sugar; migrate `flex_panel` then `self_lapping_cone` onto it (each migration re-verifies + gains usage-first docs/doctests).
4. **Focused V&V gaps** (#220) — Chebyshev/Bernstein rail basis + any coverage the new API surface needs.
5. *(later)* proper V&V sweep; *(later still)* multilayer. Atlas parked.
