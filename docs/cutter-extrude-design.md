# AUTH.1 — the sketch-extrude cutter (frame × profile × apex × span)

The design for `Cutter::Extrude`, the step-1 blocker named in
[`atlas-transform-design.md`](atlas-transform-design.md) §5. Acceptance criteria live in
[`vv-guide.md`](vv-guide.md) ("AUTH.1 acceptance criteria"); rows in [`../vv-matrix.md`](../vv-matrix.md).

## 1. Where this sits in the product flow

Cuts authored here happen at **stage 2** of the flow, before any stackup exists:

> define atlas → **cut the substrate boundary** → develop → export to ECAD → define stackup →
> lay out the PCB → import → refold

So a cutter meets **neutral surfaces** — the locations where charts are embedded — not copper
layers and not the two faces of a layer. Cutting a real stackup is a later capability; nothing here
forecloses it, because the span rule (§5) already counts hits generically.

## 2. The model

A cutter is four orthogonal pieces:

| piece | what it is |
|---|---|
| **Frame** | a plane: origin + two spanning directions, all rational (§3) |
| **Profile** | an `arrange2d` region in frame coordinates, inside-designation included |
| **Apex** | one homogeneous point — finite cast point *or* direction (§4) |
| **Span** | which of the neutral surfaces the ray meets actually get cut (§5) |

The profile is **not** restricted to convex, and needs no decomposition: the arrangement engine
already carries faces-with-holes and the inside designation. Nothing in this milestone builds a
polygon-CSG layer.

### 2.1 Why the evaluation is exact rather than an approximation

Preimage commutes with every boolean operation, so for the sheet map `S(σ,µ,w)`

```text
footprint(A ∩ B) = { (σ,µ) : S(σ,µ) ∈ A ∩ B } = footprint(A) ∩ footprint(B)
```

and likewise for `∪` and complement. A cutter's footprint on the sheet may therefore be assembled
from its pieces **in the 2-D domain** with no loss — this is an identity, not a bound. It is the
licence for the whole approach and the reason no 3-D boolean kernel appears anywhere
(`docs/agent-glossary.md`: reference bodies are predicates only).

Two views follow, and the spec keeps them apart:

- **Predicate view** (containment, side choice): project the 3-D point onto the frame plane —
  through the apex, §4 — then test membership in the region. Both maps are rational, so this is
  exact and cheap.
- **Boundary view** (the cut curve on the sheet): pull each profile *edge*'s wall back to a rail,
  which is what `develop::cut` already does per surface.

### 2.2 The surface-class table

The wall class is decided by the profile edge class, and the apex does not change it:

| profile edge | apex at infinity (`w = 0`) | finite apex (`w ≠ 0`) |
|---|---|---|
| line segment | **Plane** | **Plane** (through the apex and the edge) |
| circular arc | **elliptic cylinder** | **elliptic cone** |

Everything stays degree ≤ 2, so every wall pulls back to the existing deg-≤2 rail machinery over
ℚ(σ). **One new surface covers both right-hand cells: `CutSurface::Quadric`** — the general
`XᵀMX + b·X + c = 0` on one nappe. A cast point costs no more than a drill does: it swaps a
translation for a projection, and both are rational.

> **Correction, made while building AUTH.1a.** An earlier draft of this table said *Cylinder* and
> *Cone*, meaning the two metric surfaces the kernel already had. Both cells are wrong at that
> reading, for two independent reasons, and either one alone forces the general form:
>
> - The cone over a circle from an apex **off that circle's own axis** is an *oblique* circular
>   cone, which is a general quadric cone (elliptic sections), not a right circular one. A single
>   cast point serves the whole profile, so it is off-axis for all but one of the profile's arcs.
> - Under an **affine** frame (§3) a profile "circle" is already an ellipse in 3-D, so even the
>   parallel case sweeps an elliptic cylinder.
>
> The general form is *fewer* new variants than the table originally implied, not more, and the
> existing `Plane`/`Cylinder` keep their exact closed-form distances untouched.

The price of generality is that a quadric has **no closed-form distance**, which the certificate has
to supply some other way — see §4.2.

### 2.3 One rule builds every wall

The two views share a single mechanism, and building AUTH.1b around it removed a whole class of
special cases. A point's frame coordinates are a **rational quotient**

```text
(a, b) = ( L₁·(X − o) / T(X),  L₂·(X − o) / T(X) )
```

with `L₁, L₂` rational 3-vectors and `T` a rational affine form — the projected point's homogeneous
weight. So **any 2-D carrier equation becomes a 3-D surface by substituting that quotient and
clearing the denominator**: a line clears to a plane, a circle clears (against `T²`) to a quadric.
One derivation covers both carrier classes and both apex kinds.

Three things fall out that would otherwise each need their own handling:

- A wall is read off its edge's **carrier**, never its endpoints, which is why a profile arc's
  algebraic endpoints (`Surd`, after a boolean) never enter — trimming to the authored piece is a
  `(σ, µ̂)`-domain boolean, not a surface property.
- A circle needs only **`r²`**, never the generally-irrational `r`, because `r²` survives the
  clearing linearly. That is exactly how `arrange2d` already stores circles.
- Each wall **inherits its own carrier's sign** — negative inside a disc, negative on the 2-D line's
  negative side. The *fill rule* stays with the region rather than being smeared across the walls,
  which is what lets a non-convex profile with holes work with no decomposition at all.

`T` vanishes exactly on the plane through the apex parallel to the frame — where a generatrix runs
parallel to the frame plane and no projection exists. That is the same plane the §4.1 nappe selector
uses, so one quantity carries both meanings.

## 3. The frame, and why it is affine rather than orthonormal

A frame is an origin `o` and two independent spanning vectors `u, v`, all rational. A profile point
`(a, b)` maps to `o + a·u + b·v`, an exact rational affine map; the plane normal is `u × v`, also
rational.

Requiring `u ⊥ v` with `|u| = |v| = 1` would be a trap: rational orthonormal frames exist only for
special normals, so a general picked frame could not be represented exactly. The affine frame has no
such restriction, and it costs the surface-class table nothing **now that the table is stated over
quadrics** — under a non-orthonormal frame a profile "circle" maps to an ellipse in 3-D, whose
cylinder or cone is still degree 2. (It is one of the two reasons the table's earlier metric reading
did not survive; see the correction in §2.2.)

The consequence is honest rather than hidden: a circle drawn in frame coordinates is a circle *in
those coordinates*, and is a true metric circle only when the frame is orthonormal. The frame
therefore **reports its metric distortion** (how far `u·v`, `|u|²−|v|²` are from zero) so a caller
that needs a true circle can see that it has one. Where a true-metric frame is wanted, note that
rational points on the unit sphere are dense — a picked normal can be snapped to a rational unit
vector as closely as required.

## 4. The apex is one homogeneous point

```text
Apex = [a : w]        w ≠ 0  →  finite cast point
                      w = 0  →  direction (today's parallel drill)
```

A parallel extrusion *is* a projection from a point at infinity, so these are one object, not two
variants. The generatrix through profile point `Q` is the line joining `[Q:1]` and `[a:w]`, and the
apex enters every formula below through the single expression `a − w·o` — at `w = 0` the direction,
at `w = 1` the offset from the frame origin. One formula, one code path, **one cut-fit certificate
derivation** instead of two.

This is idiomatic for this kernel rather than clever: `σ = tan(φ/2)` is already a projective
parameter, and Stage 2's seam result was that the seam sits at `σ = ±∞` and is removed by the exact
Möbius `σ' = −1/σ`. Treating "parallel" as "apex at infinity" is the same move, and it avoids
re-introducing the coordinate-singularity special-casing that re-centering exists to remove.

`w == 0` is an exact test on `Rat` — no float, no tolerance. Ergonomics are recovered with
`Apex::direction(d)` / `Apex::point(p)` over homogeneous storage. A useful side effect: pushing the
apex outward degrades the taper to parallel continuously, with no API discontinuity.

### 4.1 Two validity conditions, both fail-closed and exact

- **One nappe.** A finite apex generates a *double* cone. The cutter is the single nappe on the
  authored side; without this the cut reappears mirrored beyond the apex.
- **Apex clearance.** An apex lying between the frame plane and the surface being cut inverts
  "inside". That is a refusal (`Refuted`), never a repair.

Both are **one check** in the built cutter, because the apex sits exactly on the boundary plane of
the nappe selector: requiring the selector *strictly*, over the whole working ball, refuses the
mirror nappe and the apex neighbourhood together. A third, build-time condition joins them —
`ExtrudeFault::ApexInPlane`, the apex lying in the profile's own plane, where a finite apex sees the
profile edge-on and a direction extrudes parallel to it.

### 4.2 The cut-fit certificate for a quadric wall

`Plane` and `Cylinder` carry closed-form distances; a general quadric does not, so its certificate
uses a **first-order bound**: if `|∇F| ≥ g > 0` on the ball `B̄(X, R)` and `|F(X)| ≤ gR`, then
`{F = 0}` meets that ball within `|F(X)|/g` of `X` (gradient flow; `F` falls at unit rate while the
path advances at speed `≤ 1/g`).

Three properties make this fit the existing DRC rather than sit beside it:

- **The hypothesis is the gate.** The bound must fit inside its own ball, and the largest ball worth
  trying has `R = clearance/2` — which is exactly the DRC threshold. So the lemma holds on precisely
  the runs that end `Verified`.
- **It is self-validating.** It never has to be told that `M` really describes a cone: a quadric with
  no real points nearby cannot pass, because the hypotheses it would need are the ones that fail.
- **`R` is searched from small upward**, since `g` is a minimum over the ball and a smaller ball has
  a larger one. A rail actually on the surface succeeds at the smallest `R` on the first try.

Its limit is worth stating plainly: the ball must avoid the surface's **singular locus** (a cone's
apex, a cylinder's axis), so a cut whose error is an appreciable fraction of the feature's own radius
cannot be bounded. Measured on the device's `R = 1/5` drill: error `5·10⁻⁴` certifies at 1.4× the
exact distance; error `6·10⁻²` reads `Unresolved`. That is the right verdict — at that scale the
distance to the surface is no longer a first-order quantity.

## 5. The span counts neutral-surface hits

Along the reference ray, the neutral surfaces met are ordered by ray parameter, and the span selects
which are cut:

```text
Span = ToNext | NextN(k) | Through | Range(start..=end)
```

**Two ordinal modes exist; this milestone builds the first.**

- **Reference ray** (built): one ray — the pick ray, or a designated profile point — fixes the
  ordinal for the whole cut.
- **Per-generatrix** (deferred, §8): each generatrix terminates on its own hit count, so the cut
  depth varies across the profile.

The ordering must handle **the same chart hit twice by one ray**, which is not hypothetical: the
self-lapping cone's flap laps its body, so a ray through the lap meets two neutral surfaces. That
case is the acceptance test (§7) precisely because a layer-index model would get it wrong.

### 5.1 A neutral surface is a *region*, not a chart

Built as AUTH.1d, and the first thing it settled is what the unit of counting is. A part carries one
frame and several **regions** differing only in their support law `h(σ)` — and it is the support that
separates a lap from the sheet it laps. Measured on the device: the wrap chart taken **bare** sends
two different σ to the *same 3-D point* at the lap, because with `h ≡ 0` the flap and the body
coincide exactly. Give each region its own support and they separate by the ramp height. So a span
computed against a bare chart would be counting a double cover, not layers.

### 5.2 What makes an ordinal trustworthy

An ordinal needs more than a position does, and each of these is a refusal rather than a hope:

- **A certified count.** The crossings are the roots of the coplanarity residual, isolated by a
  **Sturm chain whose hypothesis is checked at runtime** — so none is missed. §9.1 flagged this as
  the gap AUTH.1c left open, and it is where it closes: a scan owns a density and can step over a
  double root or two roots in one cell, and an ordinal computed from a scan is a guess.
- **Tangency refused.** A repeated root means the ray grazes, where "how many surfaces" has no
  stable answer. Detected exactly as `gcd(g, g′)` having positive degree.
- **Indistinct crossings refused.** Two surfaces closer along the ray than the clearance cannot be
  ordered at that tolerance, and naming one "next" would be fiction. The verdict carries the gap, so
  a caller can see what clearance the geometry would need.

Two exact filters complete the count: a crossing of the ruling **line** outside the region's µ̂-trim
is not a crossing of the material, and one at **`t < 0`** is behind the caster — the surface may well
continue there (the device's far wall does) but a cut does not reach backwards.

The ordering is by **ray parameter**, never by σ. On the device those two disagree: the flap is
nearer the caster but has the *larger* σ, so an ordinal read off σ inverts the lap.

## 6. What has to change in existing code

Built as AUTH.1e, in two steps: a pure refactor with a no-goldens-move gate, then the capability.

**The load-bearing assumption was not in this list.** It was the `Shadow` type in `resolve.rs`: one
labelled µ̂-interval, because a quadric `MuCut` has exactly two roots. A general profile shadows a
ruling in *several* stretches, so `Shadow` became a union of `Patch`es (**AUTH.1e.1**) — with
`comp_intersect`/`comp_subtract` keeping their bodies verbatim and the union a `flat_map` and a
`fold` over them. The equivalence is structural: the old `Empty` arms return what those combinators
produce over **zero** patches, and the old single-interval arms are exactly one patch.

Then (**AUTH.1e.2**):

- `Cutter::surface() -> CutSurface` (`pub(crate)`, so no public break) became
  `walls() -> Result<Vec<CutSurface>>`. All five call sites index by wall; the metric cutters return
  exactly one, so each of them reads `walls()[0]` and nothing about them moved.
- `Label` gained `BranchSide::Wall(index, upper)`, keeping the tuple shape so the other 24 label
  sites compiled untouched.
- The two station sites keyed off the **cutter variant** (`matches!(cutter, Cylinder{..})` in
  `resolve.rs`, a per-variant `match` in `realize.rs`). Windowing is not a property of the variant —
  it is a property of the **wall**: one whose µ̂-pullback is a genuine quadratic (`a ≢ 0`) is real
  only between tangent rulings, an affine one everywhere. Both sites test that instead, which
  reproduces the old behaviour by construction (`cut.rs` sets `a: RatFunc::zero()` for a plane, and
  a cylinder's `a` vanishes identically only if the ruling is always parallel to the axis).

### 6.1 The reference ray is derived, and two obvious derivations are wrong

The span (§5) counts along a reference ray, and AUTH.1e.3 derives it rather than asking the author
for one — but the first two derivations tried were both wrong, in ways worth keeping.

**Not the frame origin.** §5 says "the pick ray, or a designated *profile* point", and the frame
origin need not lie in the profile at all. On a cone-charted part it is typically the apex, where the
ray runs along a ruling and the cast is refused as ungrounded — a correct refusal of a bad question.

**Not a circle's centre.** Searching the profile's own carrier data for an interior point, the
obvious candidate is a circle's centre — which sits *exactly* on the row `arrange2d`'s exact
ray-casting excludes, so the fill rule cannot answer there. The most obvious seed is the one
guaranteed to be undecidable, and an **odd** sample grid reproduces the same row at its midpoint.

What works is a search: an even-count grid over the profile's extent, first accepted point wins,
refusal if none is. The extent needs `r` from `r²`, which three Newton steps bound from above
rationally — so a profile's extent is computed without ever taking a root.

### 6.2 Edges are not carriers

A wall belongs to a **carrier**, and `arrange2d` hands out *decomposed* edges — a circle as its two
x-monotone arcs, a split line as several segments. Building a wall per edge therefore duplicates
surfaces, and while the µ̂-shadow absorbs that (coincident crossings leave zero-width stretches), the
σ-window stations run per wall: measured, an extruded disc derived **two interior holes where the
cylinder it equals derives one**. `Cast::carrier_walls` dedupes by carrier, with lines reduced by
their first nonzero coefficient so the same line at two scales collapses.

What caught it is worth recording: not the unit tests of the shadow — all green before and after —
but the end-to-end differential that authors the *same solid* two ways and compares the resolved
structures. The defect lived in the layer above the function under test.

## 7. Slices

| slice | content |
|---|---|
| **AUTH.1.0** | this document + `vv-guide` criteria + `vv-matrix` rows + tasks (the GO-gate) |
| **AUTH.1a** | `Apex` (homogeneous) + `CutSurface::Quadric` + its pullback in `cut_mu_form`; the §4.1 refusals; the §4.2 first-order distance bound — **done** (`develop::extrude`) |
| **AUTH.1b** | `Frame` (affine, with reported distortion) + the §2.3 carrier pullback + the projective inside predicate — **done** (`develop::extrude::Cast`) |
| **AUTH.1c** | ray-pick frames: search → **backward-error certificate** (§9) — **done** (`develop::pick`) |
| **AUTH.1d** | the span over neutral surfaces, reference-ray mode, with the lap test — **done** (`develop::pick::{Sheet,Span,ray_crossings}`) |
| **AUTH.1e** | `Cutter::Extrude` wired into `Part`; de-ossify `resolve.rs` / `realize.rs` (§6) — **done** (1e.1 shadow union, 1e.2 the cutter, 1e.3 the span) |
| **AUTH.1f** | acceptance demo + faithfulness tests + full gate + landing — **done** (`author/examples/sketch_cutter.rs`, `author/tests/sketch_cutter_part.rs`) |

**Named acceptance criterion (AUTH.1d).** On the self-lapping cone, a cut whose ray passes through
the lap satisfies: `ToNext` cuts the flap only; `NextN(2)` and `Through` cut flap **and** body. No
new fixture — the geometry is already certified, so the test measures span semantics rather than
re-testing the device.

### 7.1 What the demo had to check, and why ε could not

`ε` is the **max over pipeline stages**, and on this device the panel's boundary dominates it — so a
drafted hole and an undrafted one certify at *the same* `ε` (measured: 4.879e-1 both). A demo or test
asserting only `Verified` would therefore pass just as happily on a cutter that ignored its apex
entirely. Faithfulness has to be **geometric**:

| check | measured |
|---|---|
| the draft is the *right* draft | developed hole 0.4759 drafted vs 0.5969 parallel, **ratio 0.797** against the taper law `1 − z/z_apex` = 0.797 at `z ≈ 2.44`, `z_apex = 12` |
| the general cutter *is* the special one | a parallel-swept disc cuts the same hole as `Cutter::vertical_cylinder` to 1e-6, at the same `ε`, through `develop()` |

Both are tests, not just demo output. The measurement goes through
`export::svg::region_to_polys` — the quarantined exact→`f64` bridge — because a hand-rolled
conversion returns NaN on large rationals, and `min`/`max` then swallow it into a silent "could not
measure" rather than a failure.

## 8. Deferred, deliberately

Recorded so they read as scope decisions rather than oversights:

- **Per-edge draft slope.** A single apex forces one projective taper and cannot give edge A 5° and
  edge B 0°, which is real fab practice. Wanted later; not required now.
- **p-curve profile edges.** Lines and arcs keep every wall a plane-or-quadric. Admitting the PC
  p-curves would push walls past degree 2 and into new certificate territory.
- **Per-generatrix span** (§5).
- **The span counts *surface* crossings, not *material* crossings.** The crossing search uses each
  region's full µ̂ extent, so a ray that leaves the material and re-crosses the surface's untrimmed
  continuation still counts one. That matches §5's own wording — neutral surfaces, chart embeddings
  — and narrowing it to the trimmed material is circular: the material extent depends on the very
  ops the span restricts. Worth revisiting once an op ordering makes the bounding cuts separable.
- **Cutting a real stackup** (per-layer, §1), once a stackup exists in the flow.

## 9. Ray pick is a search, not a certificate

A ray meeting a rational developable solves a polynomial, so the hit point is in general
**algebraic**. Carrying it as such would push `AlgReal` arithmetic into every downstream cut.

Instead this follows the split MAP.1 established for `fold`: the hit-finding is a *search*, and the
frame it produces is certified by backward error rather than trusted. Everything downstream stays
exactly rational, and the searcher may be replaced freely without touching the certificate — the
same property that let MAP.1 swap its bisection.

**Built as AUTH.1c, with three refinements the sketch above did not anticipate.**

*The frame is exact; only σ is not.* A chart's `pedal`, `ruling` and `normal` are rational functions
of σ, so at a **rational** σ they evaluate to exact rational vectors. A frame built there has an
origin **exactly on the surface** and axes that are **exactly** the chart's own fields. Nothing is
"snapped". The whole backward error therefore collapses into one quantity, and the certificate reads:
*this frame is the exact pick of a ray parallel to the one you asked for, displaced by at most ε*.

*The residual is point-to-point, not point-to-line.* The obvious bound — distance from the frame
origin to the ray's line — is **blind to the sign of the ray parameter `t`**, because a negated `t`
lands on the same line. `t` is exactly what the span (§5) orders hits by, so the certificate measures
the distance to the ray's own point at `t` instead. That is a strictly stronger bound at no cost, and
it caught a sign error in the solve that the weaker one certified happily.

*No float is needed.* The ray meets the ruling at σ exactly where the two lines are coplanar, so the
hits are the roots of `g(σ) = det[base(σ) − origin, ruling(σ), dir]` — rational in σ, isolated to
rationals by the existing `scan_roots` bisection. This is not a change of doctrine but a
demonstration of it: the certificate never asked how σ was found. A float cast drops in for speed
with an identical guarantee.

The **in-plane** half of the statement turns out to be free or exactly measurable rather than
approximate: with the normal taken from the surface, the ruling already lies in the frame plane, so
the deviation is **exactly zero**; with the normal taken from the ray, it is nonzero by construction —
not an error but the geometry of that choice — and is reported as an exact `sin²`.

### 9.1 What the pick does not certify

The ε bound covers the frame's **geometry**, not the hit's **ordinal**. The root scan owns a density
and can step over a double root or two roots inside one cell, so "the third surface the ray meets" is
only as reliable as that scan, and no backward error detects a miscount. The span in §5 selects *by
ordinal*, so it needs a root **count** it can trust — a Sturm question, not a backward-error one, and
the first thing AUTH.1d has to settle.
