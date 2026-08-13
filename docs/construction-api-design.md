# Construction API — core + facade design (as decided)

*Design pass (task #218), 2026-08-12/13. Follows `construction-api-inventory.md`. Red-lining is
**complete**: D1–D5 DECIDED, D6 (naming) deferred-by-design with accumulated constraints. The coherence
pass (2026-08-13) rewrote the facade spec to the as-decided state and ground-truthed names against the
code (`CutSurface` exists in `develop::cut`; `RootPick` in `export::cut_oracle`; brep's `w` = the
physical thickness window, `flex_panel.rs:285`).*

## Decision status

Resolved 2026-08-13 over three red-line rounds; detailed discussion under *Decisions (record)* below.

| ID | Decision | Options (★ = current recommendation) | Status |
|----|----------|--------------------------------------|--------|
| D1 | Certification timing | declarative recipe, certify at `.develop()` | **DECIDED** (2026-08-13): declarative — builders total/infallible; optional `.check()` dry-run later |
| D2 | Apex-side model | dissolved — no chart-coordinate knobs in the facade | **DECIDED** (2026-08-13, as amended): witness doctrine (mechanism-agnostic resolver, conclusive-or-fault; picks = exact `RegionPick`, incl. rational rays) + product coordinates; core covariant in (σ,µ̂), reparametrization first-class later |
| D3 | Cut model | ~~role-named cuts~~ → material ops with solid cutters | **REVISED + DECIDED** (2026-08-14): `subtract`/`intersect` with **solid `Cutter`s** (HalfSpace/Cylinder/Extrude v1; Cone/Sphere/Quadric planned, CM class); roles *derived* in-domain (CutRole + RootPick internal); `Extrude` sketch = the flagship; see *Material ops* |
| D4 | Facade location | new crate day-one | **DECIDED** (2026-08-13, user override): dedicated crate from the start; publicize `export` items deliberately as the facade demands (docs-first) |
| D5 | Builder style | consuming `self -> Self` | **DECIDED** (2026-08-13): consuming; loops via rebinding |
| D6 | Naming | `Part` vs `Device`/`Blank`; verbs — decide late | **Crate name DECIDED** (2026-08-13): **`author`**. Rest OPEN — constraints: avoid `Panel` (PCB fab-array term); apex-side ≠ bond face-side names; pick verbs (`keep_near`/`keep_hit`) |

Riders (adopted 2026-08-13, no objection): the `develop::fold` piecewise/side extension ships
**immediately after facade v1** — v1 evaluators are `.develop()`/`.solid()`, `.fold()` lands with the
extension (v1.1); `FlatPattern`/`PartSolid` are **thin facade newtypes** over `FlatOutline`/`Brep`
(stable vocabulary; homes for `.svg()`/`.write_step()` and the D2 resolution report), `FoldedWire`
re-exported as-is.

## Confirmed this session

- **Split layering.** Certified core `develop::part::PiecewiseDevelopment` (exact, no float in the
  certificate) + a thin authoring facade `Part` (iterates).
- **Floats above the quarantine are fine — but exact stays exact.** Two clauses, both binding:
  1. *Floats are permitted above the pure tier* (`lattice` + `certify-core`). A certified result is
     indifferent to what FP ran during **solution search**, so the cut/trim authoring layer (float
     *proposal* via `cut_oracle`) moves **out of `#[cfg(feature="diagnostics")]` into the default build**;
     `diagnostics` reverts to meaning viewers/plots. Confirm `no_float` scopes to lattice+certify-core only.
  2. *What can be exact **must** be exact; approximation is **opt-in**, never the default.* Floats are for
     *search*, not for *storing a construction*. A value that is algebraically closed stays exact end to
     end; a genuinely non-elementary value carries a **certified ε** and that ε is the *explicit,
     opt-in* boundary — not a soft default that leaks into things that didn't need it.
- **The exactness ladder — where ε is sanctioned.** Three tiers, and the API keeps them visibly distinct:
  - **Exact, always** — the `Part` recipe data (chart quaternion `q`, support `RatFunc`, `TrimDisk`
    params, region `Band`s), the cut *locus* (surface∩cyl at a given σ is algebraic → AlgReal/exact), the
    boolean/DCEL arrangement, `ρ²`, the angle coefficient `c`. These are `Rat`/`RatFunc`/`AlgReal`, never
    f64. `PiecewiseDevelopment`'s certificate is in this tier.
  - **Certified-ε, irreducible** — **develop / fold only.** `ψ = c·arctan σ` is transcendental and
    `γ = ∫[a·e(ψ)+b·e⊥(ψ)]` is non-elementary; no exact form exists, so these return `Verdict<_,_,ε>`
    with the task-#216 quadrature ε. This is the *one* place approximation is forced by the mathematics.
  - **Certified-ε, opt-in (emission)** — the low-degree **rail fit** and STEP curve export. The locus is
    exact (tier 1); fitting it to a smooth degree-`d` rail is a *rendering* choice tuned by `RailFit`, so
    the ε here is explicitly requested (you ask for a rail), not inherent. `RailFit::exact_where_possible`
    should short-circuit to the exact locus when the intersection is already rational/low-degree
    (ground truth: partially real already — "a plane rail is exact and ignores `degree`", `trim.rs:191`).
  - (Impl consequence: keep `PiecewiseDevelopment`'s certificate exact regardless of the facade; don't let
    a `develop()` ε or a rail ε bleed into a `Part` whose cuts are all algebraically exact — those report
    `Verified` with residual 0.)
- **Controlled inexactness (forward-looking stance).** Beyond the *forced* ε's (irreducible develop/fold,
  opt-in emission), we may **later** choose to approximate an *exact-capable* op **deliberately** — e.g.
  trade an expensive exact boolean / `AlgReal` path for a faster certified enclosure — but only under a
  five-part contract, all mandatory:
  1. **Explicit** — visible in the type/API, never a hidden downgrade.
  2. **Opt-in** — exact stays the default; you *ask* for the fast-path.
  3. **Controllable** — the caller sets the tolerance / budget.
  4. **Certified** — the error is a bounded `Verdict` ε, not a hope.
  5. **Refineable** — tightening the budget monotonically shrinks ε, → the exact answer in the limit.

  This is the *general* form of which the emission tier is already a special case; `Verdict<_,_,ε>` +
  `DevConfig` budget is the vehicle (a future `.refine_to(ε)` is the ergonomic wrapper). No such
  discretionary fast-path exists today and none ships without all five — this is the only sanctioned way to
  buy performance without abandoning the exactness ladder.
- **Future scope — controlled Gaussian curvature & curvature-mediated bonds (recorded so today's API
  doesn't slam doors).** We eventually want (a) sheets with **controlled K≠0**, and (b) **BONDED joints
  mediated by controlled curvature**, so developables with *incompatible generators/rulings* can be
  attached (e.g. a slightly doubly-curved transition strip absorbing a ruling mismatch that no shared-
  generator crease could). Two facts shape the current design:
  - **Strain is a new budget kind, distinct from numeric ε.** By Theorema Egregium a K≠0 patch has *no*
    isometric flat pattern — its flattening carries an irreducible **strain floor** ~ |K|·diam²;
    "controlled" = a certified strain bound **δ ≤ the material budget** (copper/PI elongation for
    flex-PCB). It satisfies the five-part contract on every axis *except* refinement converges to the
    floor, not to zero. Never conflate δ with numeric ε — a future Verdict carries **both**, as separate
    certified quantities.
  - **Doors to keep open in this phase** (no strain machinery ships now):
    1. Document `Development` as "a certified enclosure of the develop map" — *exact isometry is an
       implementor property, not the trait contract* — so a strain-budgeted quasi-developable implementor
       can join later without touching the `unroll`/`fold`/`brep` consumers.
    2. `PartFault::NotDevelopable` is the δ=0 special case of "exceeds strain budget"; keep it a typed,
       extensible reason (it can grow a budget parameter).
    3. Keep §14 BONDED a **3D/rational certificate over rails + solids** (it already is — SEP/SLAB/SHEAR/
       CLEAR consume no flat chart), *not* re-typed to a pair of `ConeDevelopment`s. That 3D-ness is
       precisely what lets it later bond a developable to a curved strip. **To verify when we get there:**
       that nothing in the bonded pipeline implicitly assumes the two sheets share the frame `q` (the DD
       demo pair happens to). Note the lap bond already absorbs *positional* mismatch within SLAB/SHEAR;
       the new capability is curvature *in the sheets themselves*.
    4. The `Part` recipe's chart slot stays open to new chart kinds — `from_chart` + a typed fault, no
       public signature hard-coded to the arctan-cone family.
  - **Bond ontology sketch (how such bonds and their components are defined).** One **primitive bond** —
    the only kind the certifier ever sees — with components: **two faces** `(sheet, face-side ±w/2)`
    (face-side ≠ the apex-side `Side` — naming collision for D6); **one footprint** = a single 3D-authored
    region realized as certified rails on *each* face (one 3D region, two parametric shadows — the DD lap
    already works this way); a **medium** = adhesive spec `{thickness window [t_min,t_max] (=SLAB budget),
    shear budget δ_max (=SHEAR)}` as bond *data*, not demo constants; the **§14 certificate**
    SEP∧SLAB∧SHEAR∧CLEAR; plus the currently-implicit **correspondence predicate (CORR)** — the normal
    projection Π: A|fp → B|fp must be a bijection (trivial today under shared `q`; explicit once frames
    differ; = a reach/tubular-neighborhood condition `t·κ < 1`, kin of the Milestone-D metric cap).
    A **mediated bond** is *derived*: a **mediator strip** (controlled-K sheet, geometry *solved* by float
    search per the float policy, strain-certified δ ≤ material budget) + **two primitive bonds** — so
    incompatible-ruling attachment = mediator existence, never a new certificate; thick mismatch-absorbing
    adhesive = the degenerate mediator. An **Assembly** is a graph (sheets = nodes, bonds = edges,
    self-loops allowed — the rolled tube is one sheet self-bonded). Every bond induces the certified
    **flat↔flat transfer map** over its footprint (A-flat → fold → Π → develop → B-flat) — the ECAD-facing
    artifact (pad alignment, vias, mate keep-outs), carrying both ε and δ. Bonds are declared as recipe
    data + evaluate-time certification (the D1 model verbatim). Open: SHEAR scalar vs field; the strip
    solver; CORR formalization.
- **Future scope — the 3D authoring vocabulary (stance, 2026-08-13).** 2D is complete (exact
  arrangements, polys/arcs/splines); 3D is thin (one surface family, one cut primitive, no planes, no
  placements, no reference bodies). **Principle: don't mirror the 2D engine into 3D — pull 3D back into
  2D.** Everything authored in 3D is one of four kinds:
  1. **On a sheet** → authored in the domain with the full 2D vocabulary (the DD.1 "interior features in
     2D" doctrine generalized — curves-on-sheets never need a 3D representation).
  2. **A cut of a sheet** → an extrinsic surface. The load-bearing fact: our charts are ruled,
     `X(σ,µ̂)=p(σ)+µ̂·u(σ)` with `p,u` rational in σ, so **any quadric** `Q(X)=0` pulls back to
     `(uᵀAu)µ̂² + (…)µ̂ + (…) = 0` — a **deg-≤2 algebraic rail in µ̂ over ℚ(σ)**, exactly the CM
     (Biv/resultant-cofactor/AlgReal) certificate class. **Planes are linear in µ̂ → rational rails,
     essentially free.** The existing cylinder is one quadric. Composite cutters (prisms/slabs/pockets) =
     booleans of quadric cuts = pulled back per-piece, then composed **in the domain by the existing 2D
     arrangement engine** — the 2D engine stays the single workhorse.
  3. **A reference body** — obstacle/target geometry (what the flex conforms to); never developed,
     consumed only by predicates (CLEAR/keep-out) and as the target of solved constructions (mesh3d +
     CLEAR are the seeds).
  4. **Solved** — lofts (developable through given rails), mediator strips, wrap-around-target: float
     searches (per the float policy) producing charts that are then certified. Non-primitive 3D authoring
     = *solving for* the developable, never drawing it.
  Shared infrastructure: **exact rational rigid placements** (rational unit quaternions — dense in SO(3)
  — + rational translations) as one type used by cuts, assembly positioning, and reference bodies.
  **Non-goal: a general 3D B-rep boolean kernel** — emission stays OCCT's; 3D solid-vs-solid stays
  predicates, never constructive booleans. Current-phase hook: the D3 `CutSurface` slot (see amendment).
- **Future scope — covariant core & first-class reparametrization (user, 2026-08-13).** The core speaks
  (σ, µ̂) **covariantly**: chart parametrizations carry artifacts (singularities, the σ→±∞ horizon,
  poles), so **exact reparametrization** — the rational Möbius family `σ′=(aσ+b)/(cσ+d)`, precedent Stage
  2's `σ′=−1/σ` seam re-center — becomes a first-class operation later. It is the kin of, but short of,
  the parked atlas: *one chart re-coordinatized* vs *many charts glued*. Invariants: geometric outputs
  (3D points, flat patterns up to rigid motion, verdicts) are parametrization-independent; chart-indexed
  data (σ-values, bands, γ-grids, rails) transform mechanically because recipes record **chart + values
  together**. Candidate shape: a `Reparametrized<D: Development>` wrapper implementor — no consumer
  changes, by construction of the trait.
- **Build order.** `PiecewiseDevelopment` core first; this doc shapes its public API to fit the facade.

## The load-bearing refactor: a `Development` trait

Today `unroll_trim_loop(dev: &ConeDevelopment, …)`, `fold_outline(chart, …)`, and
`brep_trim_solid(chart, …)` are hard-typed to a single development. For the facade to sit over **both** a
single-region cone and a piecewise (γ-glued) development *without* a second copy of each function, the
core exposes a small trait and the pipeline generalizes to `&impl Development`:

```rust
// develop::part (or develop::cone)
pub trait Development<B: Backend> {
    fn point(&self, sigma: &Rat<B>, mu_hat: &Rat<B>, cfg: &DevConfig<B>) -> FlatBox<B>;
    fn point_on(&self, sigma: &RatIv<B>, mu_hat: &RatIv<B>, cfg: &DevConfig<B>) -> Option<FlatBox<B>>;
    fn angle_on(&self, sigma: &RatIv<B>, terms: usize) -> RatIv<B>;
    fn radius_on(&self, sigma: &RatIv<B>, eps: &Rat<B>) -> Option<RatIv<B>>;
    fn has_directrix(&self) -> bool;
}
```

`ConeDevelopment` already has all five methods (`cone.rs`), so `impl Development for ConeDevelopment` is
a delegation. `unroll_{freeboundary,trim_loop}` and `fold_*` change their `dev`/`chart` parameter to
`&impl Development` — a mechanical, behavior-preserving edit (regression-checked byte-for-byte on the
single-region path). This is the *anti-ossification* move: one pipeline, two implementors, no frozen
per-shape function.

## The core: `develop::part::PiecewiseDevelopment`

Owns a piecewise-support developable that shares one frame `q` across regions and glues them by the
cumulative running directrix (the γ-grid). Exact; no float in the certificate.

```rust
pub struct PiecewiseDevelopment<B: Backend = Bignum> {
    regions: Vec<(Interval<B>, ConeDevelopment<B>)>,  // σ-band → development, sorted & tiling
}

impl<B: Backend> PiecewiseDevelopment<B> {
    /// `None` unless the bands tile [σ_min,σ_max] gap/overlap-free AND all regions share the frame
    /// (same angle coeff `c` and `ρ²` — i.e. the same `q`; only the support `h` varies).
    pub fn new(regions: Vec<(Interval<B>, ConeDevelopment<B>)>) -> Option<Self>;

    /// Cumulative running γ at σ: Σ over regions fully before σ of `directrix_between(r.lo,r.hi)`,
    /// plus `directrix_between(r_k.lo, σ)` in the region r_k containing σ. (`directrix_between`,
    /// `cone.rs`, already integrates γ over a sub-range with the task-#216 O(h²) quadrature.)
    pub fn gamma_at(&self, sigma: &Rat<B>, cfg: &DevConfig<B>) -> Option<[RatIv<B>; 2]>;
}

impl<B: Backend> Development<B> for PiecewiseDevelopment<B> {
    // point(σ,µ̂) = r_k.point_from(base=Σ prior-region γ, r_k.lo, σ, µ̂)  — one connected frame.
    // angle_on/radius_on/point_on route to the region containing σ (support-independent ρ,ψ shared).
}
```

This is exactly the `SelfLapping` `gamma_grid`/`point_at` logic (`self_lapping_cone.rs:299–328`) lifted
verbatim, minus the demo constants. The 3D consumer for the same `regions` shape already exists
(`brep_trim_solid_regions`), so this closes the flat/3D asymmetry.

## Coordinate changes — two kinds (added 2026-08-14)

User red-line: no coordinate-change API existed, and BONDED gluing will need one. There are exactly two
kinds; they are orthogonal and must not be conflated:

1. **Extrinsic — rigid placement (3D), exact, ships with PR 2.**
   ```rust
   pub struct Placement<B> { q: [Rat<B>;4], t: [Rat<B>;3] }   // nonzero quaternion: R = rot(q)/|q|² is rational
   impl Placement { compose, inverse, apply_point, apply_chart, snap(pose) }
   ```
   `snap` interprets an approximate pose (axis-angle/Euler in degrees, FP) and snaps to a nearby rational
   quaternion, echoed back — rational rotations are dense in SO(3); the same doctrine as the azimuth
   snap. **The (q,h) representation is equivariant:** a rigid motion `g = (q_g, t)` maps a chart to
   `(q_g ⊗ q(σ), h(σ) + ⟨n(σ), t⟩)` — a quaternion product plus a `RatFunc` shift, both exact, *same σ* —
   so placement is closed in the representation and never forces a reparametrization. Home:
   `develop::place` (it acts on `Chart`), re-exported by `author`. Consumers: `Cutter::Extrude` frames
   (now); reference bodies and `Assembly::add(part, placement)` (bond time).
2. **Intrinsic — Möbius reparametrization (σ), future** — the covariance rider: `σ′ = (aσ+b)/(cσ+d)`
   relocating chart artifacts (horizons, singularities; S2 precedent `σ′ = −1/σ`), realized as
   `Reparametrized<D: Development>` when needed. Extrinsic changes *where the part sits*; intrinsic
   changes *how the chart indexes it*.

## The facade: `Part` — model and surface

### Model: a declarative recipe + certified evaluators (D1, decided)

`Part` **records authoring intent as exact data** (regions, cuts, holes, picks, config); certification
runs only in the evaluators `.develop()` / `.solid()` / `.fold()`, each returning a single
`Verdict<Result, PartFault, ε>`. Rationale: authoring can't half-fail mid-chain; one verdict at the
boundary (matches the engine's "everything returns a Verdict"); the recipe is inspectable and later
serializable (save/load a part). Builder methods are total/infallible; a `.check()` dry-run can be added
later without changing the model.

### Type, construction, config

```rust
// author::construct — geometry entry points are FREE FUNCTIONS, not Part methods (revised 2026-08-14):
// Part never enumerates surface kinds (anti-ossification), and future *solved* constructions
// (loft, wrap-around-target, mediator strips) land here as siblings.
pub fn cone(half_angle: Rat<B>) -> Part<B>;                          // primitive
pub fn from_chart(chart: &Chart<B>) -> Result<Part<B>, PartFault>;   // general (q,h) entry
// future: loft(rail_a, rail_b) -> Result<Part>, wrap(target: &RefBody) -> Result<Part>, …

pub struct Part<B: Backend = Bignum> { /* recipe: chart, regions, ops, holes, picks, config — exact data */ }

impl Part {
    // regions — product coordinates (D2): degrees in, snapped to exact rational σ, echoed in the report
    pub fn region_azimuth(self, deg: Range<f64>, support: SupportFn<B>) -> Self;
    pub fn region_sigma(self, band: Band<B>, support: SupportFn<B>) -> Self;   // power-user escape hatch

    // config — product units first (one knob per exactness-ladder tier); expert hatches kept
    pub fn thickness(self, t: Rat<B>) -> Self;             // sheet thickness = normal-offset window [0,t] (brep's `w`)
    pub fn clearance(self, c: Rat<B>) -> Self;             // DRC clearance
    pub fn flat_tolerance(self, eps: Rat<B>) -> Self;      // irreducible tier: develop/γ quadrature target → DevConfig
    pub fn step_tolerance(self, eps: Rat<B>) -> Self;      // emission tier: rail-fit ε target → RailFit refinement
                                                           //   (the handle is `subdiv`, trim.rs:191 — the G2 finding)
    pub fn budget(self, cfg: DevConfig<B>) -> Self;        // expert hatch
    pub fn fit(self, fit: RailFit) -> Self;                // expert hatch
    // [D2] picks are exact designations; only needed when resolution faults AmbiguousRegion
    pub fn keep(self, pick: RegionPick<B>) -> Self;        // .keep_near(point) / .keep_hit(ray) sugar
}
```

Defaults: `construct::cone(a)` sets `DevConfig::tight()`, `RailFit::default()`, `clearance = 1`, one region
spanning the whole domain; the apex side is **resolved, never configured** (D2). `Band` is a σ-interval
newtype. **Supports are authored over the region's unit coordinate `u ∈ [0,1]`**, mapped affinely onto
the snapped σ-band — `SupportFn::{constant(h), smoothstep(h0, h1), ramp(h0, h1), ratfunc(f)}` (the last
is the escape hatch) — so the facade user never writes an `h(σ)`. Note `u` is σ-affine, the only exact
option (azimuth-uniform progression is transcendental in σ); documented as such — the support *shape* is
approximate design intent anyway. `region_*` appends; `develop()`-time validation builds the
`PiecewiseDevelopment` (typed `PartFault::RegionGap/Overlap/FrameMismatch`).

### Material ops — cuts are booleans with solid cutters (D3 revised 2026-08-14)

User red-line: the role-sugar (`cut_outer/inner/notch`) was **flex_panel-shaped ossification** — CAD
authors *material operations*; the role of a cut is topology, not input. And the surface vocabulary
(plane + circle-base cylinder) was too thin. Revision, three moves:

```rust
    pub fn subtract(self, c: Cutter<B>) -> Self;    // remove material
    pub fn intersect(self, c: Cutter<B>) -> Self;   // restrict material ("keep inside")

pub enum Cutter<B> {                 // SOLID regions (they have an inside), never bare surfaces
    HalfSpace { n: [Rat<B>;3], d: Rat<B> },                  // n·X ≤ d           (v1)
    Cylinder  { axis_point, axis_dir, r2 },                  // arbitrary axis     (v1)
    Extrude   { sketch: Arrange2<B>, frame: Placement<B>, extent: Extent<B> },  // lines+arcs (v1)
    Cone      { apex, axis_dir, tan2 },                      // planned — quadric (CM class)
    Sphere    { center, r2 },                                // planned — quadric
    Quadric   { a: [[Rat<B>;3];3], b: [Rat<B>;3], c: Rat<B> },  // planned — the general form
}
```

1. **Cutters are solids, not surfaces.** A solid has an unambiguous inside, so the branch choice is
   *derived from containment* — `RootPick` (`export::cut_oracle`) goes fully internal, and the witness
   doctrine's "infer" case becomes conclusive except for genuinely disconnected results (a multi-component
   in-domain region still faults `AmbiguousRegion` → `.keep(…)`).
2. **Roles are derived, never authored.** Each op pulls its cutter's boundary back to rails (plane →
   rational µ̂(σ); quadric → deg-≤2 algebraic over ℚ(σ)); the ops compose **in the (σ,µ̂) domain via the
   existing 2D arrangement engine** (`BoolOp::Diff`, SA.1); face classification (point location +
   faces-with-holes — the M3/3e machinery) then *derives* what the role-sugar used to hard-code: an
   interior cut face is a hole, a rim-crossing one a notch, the outer boundary is whatever bounds the
   kept region. `CutRole` leaves the facade (an internal realization artifact); `hole_cylinder` dies too
   (subtract an interior cylinder — classification does the rest; the pedal-general drill A2 remains the
   realization, still a correctness fix). `hole_flat(poly)` stays — a genuinely different, flat-authored
   pipeline (fold-back features).
3. **The flagship cutter is the extruded sketch** (`Cutter::Extrude`) — conventional CAD's extrude-cut:
   a 2D arrangement in a `Placement`-framed plane (the **full existing 2D vocabulary** — polys, arcs;
   splines later), extruded along the frame normal, `Extent::{Full, Slab(t0,t1)}`. Pullback: the sheet's
   projection into the sketch plane is rational in (σ,µ̂) (µ̂-degree 1), so sketch **lines → rational
   rails** and sketch **arcs → deg-2 algebraic rails** — the same certificate classes as
   HalfSpace/Cylinder; the sketch's own booleans ride the domain arrangement. Sketch **splines** = future:
   higher-degree algebraic rails, or a certified biarc approximation (an opt-in emission-tier ε).

**Stock discipline:** the part starts as the chart band over the declared σ-regions with µ̂ unbounded;
the authored ops must bound it — `develop()` faults `UnboundedRegion` otherwise. At evaluate time the ops
resolve to the one general `Vec<BoundaryArc>` (per-region `certified_rail_piecewise`, A4) exactly as
before — **the revision changes authoring, not certification.** Ground truth still holds:
`develop::cut::CutSurface::{Plane, Cylinder}` and `TrimDisk` remain the realization layer beneath
`Cutter`; the planned Cone/Sphere/Quadric variants certify by the same quadratic-in-µ̂ resultant/AlgReal
route (CM did cone∩cone — the *hard* case — already).

### Evaluators — develop / fold / solid

```rust
    pub fn develop(&self) -> Verdict<FlatPattern<B>, PartFault, Rat<B>>;   // γ-grid + cuts + holes → outline
    pub fn solid(&self)   -> Verdict<PartSolid<B>, PartFault, Rat<B>>;     // uses .thickness(); STEP re-fit internal
    pub fn fold(&self, feature: &[[Rat<B>;2]], width: &Rat<B>)
        -> Verdict<FoldedWire<B>, PartFault, Rat<B>>;                      // v1.1 — needs the piecewise fold extension
}

impl FlatPattern { pub fn svg(&self, px: u32) -> String; pub fn outline(&self) -> &FlatOutline<B>;
                   pub fn holes(&self) -> &[Poly2<B>]; pub fn eps(&self) -> &Rat<B>;
                   pub fn report(&self) -> &ResolveReport; }  // the D2 echo: snapped σ per region, resolved picks
impl PartSolid   { pub fn write_step(&self, path: &str) -> StepReport;  // A3 emit+audit
                   pub fn brep(&self) -> &Brep<B>; }
```

`.develop()` delegates to the generalized `unroll_trim_loop(&piecewise_dev, arcs, …)` then
`assemble_flat`. `.solid()` delegates to `brep_trim_solid_regions` with the configured thickness window
(ground truth: brep's `w: &Interval` *is* the physical thickness — `flex_panel.rs:285` "`w = [0, 1/8]` is
the panel thickness" — a product quantity, hence `.thickness()` config; it is sheet data the bond
ontology also needs for its ±w faces) and the internal low-degree re-fit (seam #8). `.fold()` delegates
to `fold_outline` — the core keeps `mu_negative: bool` (`fold.rs:345`); the facade derives it from the D2
resolution, never from a caller bool (seam #3).

### Error taxonomy

```rust
pub enum PartFault {
    NotDevelopable(String),      // Chart isn't a recognized arctan-cone frame; carries the reason
    RegionGap(Band), RegionOverlap(Band), FrameMismatch,  // piecewise validation
    CutUnresolved { op: usize, eps: Rat },                // a rail didn't certify under clearance
    AmbiguousRegion { op: usize },                        // resolution couldn't decide soundly → add .keep(…)  (D2)
    UnboundedRegion,                                      // the ops left the part non-compact (missing intersect)
    Pole(Rat),                                            // enclosure denominator straddled zero
}
```

Replaces every bare `None`/`panic!("did not certify")` in the demos with a typed reason (seam #9).

## Call-site validation (the "usable" test)

**`flex_panel` today (~360 lines) →** note: no side knob, no `RootPick`, no roles, no raw `w` — four
cylinders differing only in placement, roles inferred by classification:
```rust
let part = construct::cone(Rat::new(65,97))
    .clearance(Rat::from_i128(1)).thickness(Rat::new(1,8))
    .intersect(Cutter::cylinder(axis0, r2_outer))    // bound the blank
    .subtract(Cutter::cylinder(axis1, r2_inner))     // ⇒ inner boundary (derived)
    .subtract(Cutter::cylinder(axis2, r2_notch))     // rim-crossing ⇒ notch (derived)
    .subtract(Cutter::cylinder(axis3, r2_hole));     // interior ⇒ hole (derived)
let flat = part.develop().expect_verified("panel");
flat.svg(720);                       // SVG
part.solid().expect_verified("solid").write_step("flex_panel.step");
```

**`self_lapping_cone` (918-line hand-rolled builder) →** collapses to azimuth-authored region
declarations + material ops; the γ-grid, region routing, and connected-frame accumulation move behind
`.region_azimuth(...)` + `.develop()`:
```rust
let part = construct::from_chart(&cone_wrap())?
    .region_azimuth(0.0..236.0,   SupportFn::constant(zero()))       // body, γ=0
    .region_azimuth(236.0..282.0, SupportFn::smoothstep(zero(), d()))// ramp, γ≠0
    .region_azimuth(282.0..296.0, SupportFn::constant(d()))          // plateau
    .intersect(Cutter::cylinder(axis, r2_outer))
    .subtract(Cutter::cylinder(axis_off, r2_inner))
    .subtract(drill);                                                // ⇒ hole (derived)
let flat = part.develop().expect_verified("self-lapping");
flat.report();   // echoes the snapped exact σ per region + the resolved picks
```

## Decisions (record)

- **[D1] DECIDED — declarative.** Builder methods are total/infallible (record exact data, never
  `Result`); certification runs only at the evaluators, one `Verdict` each. All validation (region tiling,
  rail resolution, side inference) reports as typed `PartFault` at evaluate time. If interactive fail-fast
  is ever wanted, an optional `.check()` dry-run — the model doesn't change.
- **[D2] DECIDED (2026-08-13, as amended) — dissolve the knob** (user red-line: "rail definition stuff
  feels like a leaky abstraction"). `Side`/µ̂-sign is chart-internal *branch selection*; it must not be a
  facade concept. Two parts:
  1. **The witness doctrine — for every discrete geometric choice** (apex side, quadric-root branch,
     intersection component, orientation): (a) **infer** when unambiguous — the common case: only one side
     yields a bounded region under the given cuts; (b) when the choice cannot be made **soundly** —
     genuine geometric ambiguity *or* the resolver's accuracy is insufficient to decide — **fail typed**
     (`PartFault::AmbiguousRegion`), demanding an explicit pick; (c) **record the resolved choice as exact
     discrete data** in the recipe — certification runs downstream of it. Amendments (user, 2026-08-13):
     the **resolution mechanism is an implementation detail** (float search is one option per the float
     policy; interval or exact machinery equally valid — an optimization choice), but its
     **conclusiveness is part of the contract** — an under-accurate resolver must fault `Ambiguous`, never
     guess (the Verdict discipline applied even to non-certified machinery). And **picks are exact
     designations, not only points**: a small `RegionPick` vocabulary — witness point `.keep_near(p)`,
     **rational ray** `.keep_hit(ray)` (scriptable GUI-picking; ray∩quadric over ℚ is exact first-hit),
     extensible — exact-capable end to end. (The old "implicit is circular" objection applied to deriving
     the side from certified rails; a resolution pre-pass has no such dependency.)
  2. **The product-coordinates principle.** The facade speaks **product coordinates** (3D points, rays,
     azimuth degrees, millimeters); the core speaks **chart coordinates** (σ, µ̂) — **covariantly, see the
     rider**; rails never appear in facade vocabulary (certificate currency only). Regions are authored in
     **azimuth** via the exact Stage-1 law `φ = 2·arctan σ`: the user's degrees are FP-interpreted and
     **snapped to a nearby exact rational σ** (tan-half-angle = the rational circle, so snapping is
     natural and tight), recorded exactly, echoed in the report — user intent is approximate, recipes are
     exact, certification is downstream. Config in product units too: `step_tolerance(mm)`-style, not
     `RailFit{degree, basis}` internals. Escape hatch: `Band::sigma(lo, hi)` stays for power users/tests.
     **Covariance rider (user amendment, 2026-08-13):** the core is **covariant** in (σ, µ̂) — chart
     coordinates carry artifacts (parametrization singularities, the σ→±∞ horizon, poles), so **exact
     reparametrization becomes first-class in the future**: the rational Möbius family
     `σ′ = (aσ+b)/(cσ+d)` preserves rational charts (precedent: Stage 2's seam re-center `σ′ = −1/σ`).
     Doors kept open now: recipes record **chart + σ-values together** so a reparametrization transforms
     them mechanically; a future `Reparametrized<D: Development>` wrapper implementor; and
     product-coordinate inputs (points, rays, azimuth) are parametrization-independent by nature — only
     the recorded snap is chart-relative, and it transforms by the Möbius map (the two D2 parts reinforce
     each other). Consequences: `band_side(Side)` is **deleted** from the facade (the core keeps the
     explicit sign — `unroll`/`fold` consume it); `region_azimuth(deg_lo..deg_hi, support)` is the primary
     region author; costs: snapped values differ slightly from nominal (echoed back), one resolution
     pre-pass per evaluate (cheap).
- **[D3] REVISED 2026-08-14 — material ops with solid cutters** (supersedes the role-sugar form, which
  was flex_panel-shaped ossification: roles are topology, not input). `subtract`/`intersect` with solid
  `Cutter`s; roles + `RootPick` derived in-domain (arrangement classification); `Extrude` sketch cutter =
  the flagship; Cone/Sphere/Quadric planned. The pullback math stands unchanged: every quadric → deg-≤2
  algebraic rail in µ̂ over ℚ(σ) (CM class; planes linear → rational). Full spec in the *Material ops*
  section.
- **[D4] DECIDED (user override) — a dedicated facade crate from day one: `author`** (name decided
  2026-08-13). Depends on `develop` + `export`; consequence accepted: `export` items the
  facade needs get **deliberately publicized as demanded** (docs-first, missing_docs=0 per the merge
  gate), rather than consumed as `pub(crate)`. Per no-interface-ossification, consumers update freely.
- **[D5] DECIDED — consuming `self -> Self`.** Fluent one-expression chains; programmatic construction
  (N holes from ECAD data) via rebinding `part = part.hole(h)`.

## Build steps (concrete)

**PR 1 — the core** (decision-independent, in `develop`):
1. `Development` trait + `impl for ConeDevelopment` (delegation).
2. Generalize `unroll_{freeboundary,trim_loop}` (and later `fold_*`) to `&impl Development` —
   byte-for-byte regression on the single-region path.
3. `develop::part::PiecewiseDevelopment` (`new` + `gamma_at` + `impl Development`), with tests that
   reproduce `self_lapping_cone`'s `gamma_grid`/`point_at` outputs.

**PR 2 — the `author` crate** (D4: day-one crate): the `construct` module + the `Part` recipe +
`Placement` (minimal: type, `apply_chart`, `snap` — in `develop::place`) + material ops with the v1
HalfSpace/Cylinder cutters + the **in-domain classification** (arrangement → derived roles) + the D2
resolution (infer / `AmbiguousRegion` / `keep`) + `.develop()`/`.solid()`, validated by rewriting
`flex_panel` on it; `export` items publicized as demanded (docs-first, missing_docs=0). Extraction
targets ride where first needed: A2 pedal-general hole (a correctness fix) + A3 `emit_certified_step` +
A4 `certified_rail_piecewise` land here.

**PR 3 — v1.1**: the piecewise/side fold extension in `develop::fold` + `.fold()`; rewrite
`self_lapping_cone` on the facade — **the 918-line collapse is the acceptance test** of the whole phase.

**PR 4 — `Cutter::Extrude`** (lines+arcs sketches in a placed frame): the flagship authoring primitive;
after the acceptance test so PR 2 stays lean. Cone/Sphere/Quadric cutter variants follow as demand
appears (same certificate class).
