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
variants. The generatrix through profile point `Q` is the line joining `[Q:1]` and `[a:w]`; the wall
over a profile edge is the plane spanned by the edge's endpoints and the apex — a single
determinant, which at `w = 0` yields the plane containing direction `a`. One formula, one code path,
**one cut-fit certificate derivation** instead of two.

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

## 6. What has to change in existing code

- `Cutter::surface() -> CutSurface` (`crates/author/src/part.rs:119`) cannot survive — an extruded
  cutter has many walls. It becomes a walk yielding the wall surfaces plus the projective predicate.
- `crates/author/src/resolve.rs:521` special-cases `Subtract` + `Cutter::Cylinder` when choosing σ
  sample stations. An extruded cutter would silently receive no targeted stations and drop small
  features between cells. This generalizes to "the σ-windows where any wall of the cutter is
  active" — a de-ossification in the sense of the standing rule, not an addition beside it.
- `crates/author/src/realize.rs:223` dispatches per `Cutter` variant; same treatment.

## 7. Slices

| slice | content |
|---|---|
| **AUTH.1.0** | this document + `vv-guide` criteria + `vv-matrix` rows + tasks (the GO-gate) |
| **AUTH.1a** | `Apex` (homogeneous) + `CutSurface::Quadric` + its pullback in `cut_mu_form`; the §4.1 refusals; the §4.2 first-order distance bound — **done** (`develop::extrude`) |
| **AUTH.1b** | `Frame` (affine, with reported distortion) + profile-edge → wall mapping + the projective inside predicate |
| **AUTH.1c** | ray-pick frames: float search → rational snap → **backward-error certificate** (§9) |
| **AUTH.1d** | the span over neutral surfaces, reference-ray mode, with the lap test |
| **AUTH.1e** | `Cutter::Extrude` wired into `Part`; de-ossify `resolve.rs` / `realize.rs` (§6) |
| **AUTH.1f** | acceptance demo through develop → fold → STEP; full gate; vv-matrix rows to ✅ |

**Named acceptance criterion (AUTH.1d).** On the self-lapping cone, a cut whose ray passes through
the lap satisfies: `ToNext` cuts the flap only; `NextN(2)` and `Through` cut flap **and** body. No
new fixture — the geometry is already certified, so the test measures span semantics rather than
re-testing the device.

## 8. Deferred, deliberately

Recorded so they read as scope decisions rather than oversights:

- **Per-edge draft slope.** A single apex forces one projective taper and cannot give edge A 5° and
  edge B 0°, which is real fab practice. Wanted later; not required now.
- **p-curve profile edges.** Lines and arcs keep every wall a plane-or-quadric. Admitting the PC
  p-curves would push walls past degree 2 and into new certificate territory.
- **Per-generatrix span** (§5).
- **Cutting a real stackup** (per-layer, §1), once a stackup exists in the flow.

## 9. Ray pick is a search, not a certificate

A ray meeting a rational developable solves a polynomial, so the hit point is in general
**algebraic**. Carrying it as such would push `AlgReal` arithmetic into every downstream cut.

Instead this follows the split MAP.1 established for `fold`: the float ray-cast is a *search*; the
frame it produces is **snapped to exact rationals** and then certified by backward error — "this
rational frame lies within ε of a true surface hit, with its in-plane direction within δ of the
local ruling". Everything downstream stays exactly rational, and the searcher may be replaced freely
without touching the certificate, which is the same property that let MAP.1 swap the bisection.
