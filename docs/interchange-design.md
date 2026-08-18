# IO — the interchange boundary (DXF/SVG in and out, and the diagnostic dump)

The design for the translators that let a real device outline enter the kernel and a real flat
pattern leave it. Acceptance criteria live in [`vv-guide.md`](vv-guide.md) ("IO acceptance
criteria"); rows in [`../vv-matrix.md`](../vv-matrix.md).

This is the milestone that makes the system *usable in production* — until a fab-house outline can
be read and a flat pattern written, every device is a literal in `crates/acceptance`. It ships
before MAP because MAP's value is measured on real geometry.

## 1. Where this sits in the product flow

> define atlas → **cut the substrate boundary** → develop → **export to ECAD** → define stackup →
> lay out the PCB → **import** → refold

Three of those seven arrows are file I/O, and the kernel currently has none of them: the boundary is
authored as Rust literals and the flat pattern leaves as a diagnostic SVG whose only job was to be
looked at. IO.1 supplies the first arrow, IO.2 the second and third, IO.3 the picture that says
whether the first one landed where the author meant.

## 2. What is exact, and where the loss actually is

The reflex answer — "files are floats, so import is a snap with a tolerance" — is wrong twice over,
and getting it right is most of this design.

### 2.1 A decimal literal is a rational

`12.345` in a DXF group-value line or an SVG `d=` attribute is exactly `12345/1000`. There is no
approximation in reading it; the approximation is entirely an artifact of routing it through
`f64`. So the number bridge is **decimal text → `Rat`**, and the float is transport, not a
representation:

```text
file text ──parser──▶ f64 ──shortest round-trip decimal──▶ Rat
```

Rust's `f64` Display emits the shortest decimal that round-trips, so a literal of ≤17 significant
digits is recovered **exactly**. Beyond 17 digits the recovered decimal differs from the literal by
at most one ulp, which is reported rather than assumed away. No CAD writer emits more than 17.

> This is the whole reason a "tolerance" is not the top-level concept. Most of a real outline —
> every `LINE`, every `LWPOLYLINE` vertex, every `CIRCLE`, every SVG `L`/`M`/`H`/`V`, every unit
> conversion, every `matrix()`/`translate()`/`scale()` transform — imports with **δ = 0**.

### 2.2 The loss is a *consistency* condition, not a rounding

Where δ ≠ 0 arises is where the file states a curve **over-determined** in a way rational numbers
cannot satisfy simultaneously. `Profile::arc` needs its endpoints to satisfy
`(x − cx)² + (y − cy)² = r²` **exactly** (it is total, and emits an off-circle arc as drawn — so an
importer that hands it an inconsistent arc feeds the arrangement bad data rather than earning a
refusal). A DXF `ARC` gives centre, radius *and* two angles: four exact rationals describing a point
that is irrational. One of them must move. §4 decides which, and by how much.

### 2.3 Three numbers, never conflated

Every read returns three quantities that a single "tolerance" would blur:

| | what it measures | whose fault |
|---|---|---|
| **δ** (backward error) | how far the geometry we built is from the geometry the file states | ours |
| **transport** | an ulp bound on decimal-text → rational, where a parser got there first | the parser's |
| **closure gap** | how far the file's own adjacent entities are from meeting | the file's |

A file whose outline has a 3 µm gap between a LINE end and the next ARC start is a *data* problem;
reporting it inside δ would let a sloppy file masquerade as a lossy importer, and would hide a real
importer regression behind whichever file happened to be worst. They are separate fields.

**Transport is exactly zero for DXF and only bounded for SVG**, and the difference is the reason
§8 reads DXF by hand: the DXF reader sees the file's own group-value text, so `rat_from_decimal`
applies to the literal. The SVG path grammar arrives through `svgtypes` as `f64`, recovered via
shortest-round-trip `Display` — the literal itself for anything under 17 significant digits, and
bounded by an ulp beyond that. We cannot see the text from that side, so we do not claim to have
recovered it.

### 2.4 `δ = 0` is a statement about the translator, not a promise about the file

Both an SVG `<rect rx>` and a DXF bulge polyline import at `δ = 0`, and they mean different things
by it. The `<rect>` states a **shape**: its corner endpoints are the axis-aligned tangent points, so
the arcs come out exactly tangent to the sides. A bulge states a **curve**: `tan(Δθ/4)` for a
quarter turn is `√2 − 1`, which no file can write down, so a real file carries a decimal near it and
the import reproduces *that* curve exactly — faithful to the file, and not quite the tangent
quarter-circle its author had in mind. Measured: `|r² − (1/4)²| ≈ 10⁻¹¹` for a ten-decimal bulge.

Saying "exact" without that distinction would be the most misleading true sentence in the milestone.

### 2.4 The five-part contract

`docs/construction-api-design.md` §"Controlled inexactness" sanctions approximation only under five
mandatory clauses. The importer satisfies all five, which is why it is allowed to exist at all:

1. **Explicit** — the return type is `(geometry, ImportReport)`; δ is not optional to look at.
2. **Opt-in** — exact is the default and the common case (§2.1); only a construction that *cannot*
   be exact consumes tolerance, and it is refused rather than silently approximated when the
   tolerance is not met.
3. **Controllable** — the caller sets the tolerance.
4. **Certified** — δ is an outward-rounded bound from `develop::interval` (`arctan`, `pi`, `cos_on`,
   `sin_on`), the same enclosure machinery the development runs on. Not a float estimate.
5. **Refineable** — tightening the tolerance deepens the rational search (§4.1) and δ → 0, since the
   rationals are dense in the circle's parametrization.

## 3. The four directions, and why they are not symmetric

| direction | from | to | consumed by | exactness |
|---|---|---|---|---|
| **sketch-in** | DXF/SVG | `Vec<Edge>` in frame coordinates | `Cutter::extrude` | arcs stay arcs; δ per §4 |
| **flat-in** | DXF/SVG | `Vec<[Rat; 2]>` polygons | `Part::hole_flat` | arcs **chorded** — the consumer takes a polygon |
| **sketch-out** | `Vec<Edge>` | DXF/SVG | fab, review | arcs → decimal angles: **lossy outbound** |
| **flat-out** | `FlatPattern` | DXF/SVG | fab | already chords; carries the pattern's own ε |

Two asymmetries are worth stating because they are counter-intuitive and they set what the tests
must measure.

**Inbound and outbound lose on opposite sides.** A DXF `LWPOLYLINE` bulge is a rational number, and
§4.2 turns it into an exactly-consistent arc with **δ = 0**. Going the other way, an exact arc
between two rational endpoints has bulge `tan(Δθ/4)` — irrational. So the format that imports
perfectly exports approximately. The round-trip test therefore composes two different errors and
must report them separately.

**Outbound, both formats derive everything from one written scalar — but not the same one**, and
that makes them exact on different arcs. DXF writes `tan(Δθ/4)`; SVG writes `√r²`. A quarter turn of
radius 5 costs SVG nothing and DXF a rounding; a semicircle of radius `√2` costs DXF nothing and SVG
a rounding. Two-sided, so neither format dominates and a caller can pick by the arc. (Corrected from
an earlier reading of this design that had *inbound*'s "which datum moves" table applying outbound
too — outbound, the centre and radius are derived from the written scalar in both formats.)

One exactness that does survive outbound: for an arc with rational centre and endpoints, `cos Δθ`
and `sin Δθ` are **exact rationals** (`u·v/r²` and `u×v/r²`). The turn is exact — only its
quarter-tangent is not — so `large-arc` and the bulge's major/minor branch are decided by an exact
sign test rather than by a comparison against a tolerance.

**The same parse feeds two consumers with different rules.** sketch-in and flat-in read the same
files with the same parser; they differ only in that `Part::hole_flat` takes a polygon, so flat-in
must chord any arc and report the chord deviation as a *second*, separately-named δ. One parser, two
policies — not two parsers.

## 4. The arc rule

The one place the design earns its keep. Three source forms, three different answers, and the
answers are not interchangeable.

### 4.1 DXF `ARC` — hold the circle, move the endpoints

Centre and radius are exact from the file; the endpoints `c + r·(cos θ, sin θ)` are not. The circle
of rational radius `r` about a rational centre carries the rational point `c + (r, 0)`, and from any
rational point on it the rest are dense via the tangent-half-angle rotation

```text
P(t) = c + M(t)·(r, 0)        M(t) = 1/(1+t²) · [[1 − t², −2t], [2t, 1 − t²]]
```

which is rational for every rational `t`, and exactly on the circle by construction (`M(t)` is
orthogonal with determinant 1 — `(1−t²)² + (2t)² = (1+t²)²`). So:

1. **search** — a float `t ≈ tan(θ/2)` (the file's `θ` is exact rational degrees);
2. **snap** — a rational `t` from the Stern–Brocot refinement of that float;
3. **certify** — `δ = 2r·|sin((2·arctan t − θπ/180)/2)|`, bounded above by the existing certified
   `arctan`/`pi`/`sin_on` enclosures, outward-rounded.

Refusal above tolerance; refinement deepens step 2. This is AUTH.1c's "search → rational snap →
backward-error certificate" applied to a second thing, not a new doctrine.

### 4.2 DXF `LWPOLYLINE` bulge — free, exact, δ = 0

The bulge `b = tan(Δθ/4)` is a rational from the file and the two vertices are rationals. With
`d = P₁ − P₀` and `n = perp(d)` (unnormalized, so rational), the centre is

```text
c = (P₀ + P₁)/2 + λ·n        λ = (1 − b²) / (4b)
```

— rational, because `cot(Δθ/2) = (1 − b²)/(2b)` and the normalization of `n` cancels against the
half-chord length. Then `r² := |P₀ − c|²` puts **both** endpoints exactly on the circle: `P₀` by
definition, `P₁` because `c` sits on their perpendicular bisector by construction.

*Actionable for the workflow:* an outline exported as `LWPOLYLINE` with bulges imports exactly; the
same outline exported as `ARC` entities costs a certified δ. That is a sentence for the user-facing
docs, not just this one.

### 4.3 SVG `A` — hold the endpoints, move the centre

SVG states the two endpoints (exact rationals) and a radius; the centre is derived. The roles flip:
the centre must lie on the endpoints' perpendicular bisector, which is a **rational line**, so
choosing any rational point on it makes `r² := |P₀ − c|²` satisfy `|P₁ − c|² = r²` identically. Both
endpoints land exactly on the circle and δ is a **radius** deviation, `|r² − r_file²|` scaled to a
length — never an endpoint deviation. That is also the honest thing to report, since SVG's own spec
already permits scaling the radius up when the stated one cannot span the endpoints.

### 4.4 The junction rule — arcs pin, lines follow

Adjacent entities in a real file meet only to file precision, and `Profile` needs them to share a
vertex *exactly*. A line through two rational points is exact wherever those points are; an arc has
a consistency condition it can violate. So at every junction the **arc's** endpoint is authoritative
and the neighbouring segment's endpoint is moved to it. The move is recorded in the closure gap
(§2.3), not in δ — we did not degrade the arc, we absorbed a gap the file already had.

Two arcs meeting at a junction is the one case where neither side is free. Rule: the first arc in
traversal order pins the vertex, the second is re-derived through §4.1/§4.3 with the pinned point as
its start, and if that pushes its own δ over tolerance it is a **refusal** naming both entities.

## 5. Units are exact, and they are not optional

The kernel is unitless in code and **millimetres** in meaning (the §14 BONDED shear budget is
`δ = 18/65 ≈ 0.28 mm`). Every CSS absolute unit is a rational multiple of px — `1in = 96px`,
`1mm = 480/127 px`, `1pt = 4/3 px`, `1pc = 16px` — and DXF `$INSUNITS` names its unit outright. So
unit conversion is **exact**, and there is no excuse for guessing:

- the reader takes a target unit (default mm) and converts by an exact rational factor;
- a file that declares no unit is **refused** unless the caller passes one explicitly (a silently
  assumed unit is a 25.4× part, and it will look plausible);
- the unit found, the unit produced, and the factor all appear in the report.

SVG's `viewBox` plus `width`/`height` is a rational scale; SVG's y-axis points **down**, so the
reader applies `y ↦ −y` (exact, orientation-reversing, irrelevant to the even-odd fill rule) and
says so in the report.

## 6. Refusals, by name

A translator that repairs its input silently is worse than one that refuses, because the repair is
invisible in the part that comes out. Each of these is a typed variant naming the offending entity:

| refusal | why |
|---|---|
| `EllipticalArc` | SVG `A` with `rx ≠ ry`, or an `<ellipse>`. `Profile::arc` is circular. |
| `BezierSegment` | SVG `C`/`S`/`Q`/`T`. `arrange2d::Edge` carries lines and circular arcs only. Opt-in chording (§6.1) lifts it with a reported δ. |
| `NonSimilarityTransform` | a transform whose linear part is not `[[a,−b],[b,a]]` up to reflection **and** the geometry contains an arc — it would map the circle to an ellipse. All-straight geometry accepts any exact `matrix()`. |
| `IrrationalTransform` | `rotate(θ)`/`skewX`/`skewY` at an angle whose sine and cosine are not rational, unless a tolerance is supplied (then it is a §4.1-style snap with certified δ). |
| `UnsupportedEntity` | a DXF entity outside {`LINE`, `ARC`, `CIRCLE`, `LWPOLYLINE`, `POLYLINE`} — SPLINE, ELLIPSE, INSERT, blocks, 3D entities. |
| `UnknownUnit` | §5. |
| `OpenLoop` | the entities do not chain into closed loops, with the worst gap reported so the user can see whether it is a data problem or a subset problem. |
| `ToleranceExceeded` | §4's certified δ is above the caller's budget, naming the entity and both numbers. |

### 6.1 The Bézier question is a real workflow risk

Many SVG exporters (Illustrator, Inkscape's default, anything that went through a font pipeline)
turn every circle into cubics. Fab-oriented tools (KiCad, Altium, mechanical CAD) emit arcs. Refusing
cubics by default is correct — a cubic silently chorded is a curve the kernel never agreed to carry —
but the escape hatch must exist and must be explicit: an opt-in chord tolerance that reports the
chord deviation as its own δ. Named here so that the decision is not made under deadline pressure by
whoever first hits a cubic.

## 7. The diagnostic dump (IO.3), and the plane it must occupy

Two deliverables that answer different questions and must not be conflated.

**(a) The authored sketch face — "did you read my file right?"** The profile as authored, as a
planar face (`FaceSurface::Plane`, `EdgeGeom::{Line, RationalBezier}` — no new surface kind, no shim
change) at its true 3-D position.

> **Normative: an emitted cutter sketch occupies the plane it cuts from.** Every vertex is
> `Frame::point(a, b)` — the same exact map the wall equations are built from — never a plane
> re-derived for the picture and never the profile drawn at the origin. A picked plane is a *search
> result* (AUTH.1c), and the one thing a picture can check that a certificate cannot is whether the
> pick landed where the author meant. A sketch rendered skew to the surface it cut says the pick is
> wrong; a sketch rendered anywhere else cannot say anything at all.

Arc edges: chord densely and say so, because a rational-quadratic circular arc needs weight
`cos(Δθ/2)`, which is rational only when `1 + tan²(Δθ/2)` is a rational square — i.e. only at
**Pythagorean** angles. Exact conic edges are therefore achievable by subdividing each arc at
Pythagorean rotations of its start point (`t = 3/4 → w = 4/5`, and such `t` are dense), and that is
the escalation if the eyeball check turns out to need it. Not first.

**(b) The cutter body — "where did it actually cut?"** `author::dump::cutter_bodies`. Built from
the surface the cut actually reached, back toward the sketch:

- **far cap** — the traced footprint loop (`HoleLoop` in `(σ, µ̂)`) lifted to 3-D by the chart, at
  `w = 0`, which with the default `Part::neutral` of `1/2` is the stack's **mid-plane**. A viewer
  shows the far cap buried mid-thickness, and that is the honest place for it: the footprint is
  exactly where the cut meets the surface the part is *developed* on, and the solid's walls are
  ruled along `n` from there — so the cut is exact on that surface and `±t/2` either side of it;
- **near cap** — each far-cap vertex cast *back* along its own generatrix onto the sketch plane, an
  exact bijection between the caps (matching traced vertices against profile corners would not be).
  It therefore inherits the tracer's sampling and is **not** the authored data of (a);
- **walls** — ruled between corresponding points, so each wall *is* a generatrix segment.

The route, as built:

| step | the call |
|---|---|
| resolve | `Part::build_regions()` + `resolve::sweep()` — the prelude `Part::solid()` already runs |
| which loops, on which chart | `realize::footprints` over `certify_holes` → `Vec<CertifiedHole>`; `structure.holes` carries `(op, **region**, window)`, so the region travels with the loop and the chart is `built.charts[region]` directly, with no σ-band search |
| the polygon | `export::trim::hole_poly` — **shared with the solid path**, not re-derived. See the correction below |
| far cap lift | `chart.surface(&µ̂, &0).eval(&σ)` — `µ̂` **is** the chart's ruling parameter `µ`, not a normalized one |
| near cap | `Cast::coords(&X)` → `(a, b)`, then `Frame::point(a, b)` |
| faces | **triangles throughout** — a ruled quad between two generatrices is not coplanar and a traced far cap is not planar, so `FaceSurface::Plane` is only honest on triangles. A visibly-triangulated body also reads as a diagnostic rather than a part, which is the right signal. Ear-clipped in exact rational arithmetic, because a traced footprint is routinely non-convex (that is the content of AUTH.2) and a fan lays triangles outside it |

**Two scouting claims the implementation corrected.** First, `hole_poly` was recorded as unusable
because it returns `None` for any loop that is not all-traced-`Curve` — true of the type, false of
this input: `certify_holes` produces all-`Curve` loops on both its branches, so the converter is not
merely usable but *the right one*, because the diagnostic should show the polygon the part was cut
with rather than a parallel sampling of its own. Sharing it also inherits the sub-`MIN_STEP` vertex
merge for free, which the body needs for exactly the reason the solid does: the tracer parks a pair
of vertices ~10⁻⁹ apart at every cell boundary, and a triangle on such a pair is unbuildable by any
`f64` consumer. Second, the AUTH.2 traced-slot fixtures were priced at ~30 s of certification
apiece; measured, `sketch_panel` with the L-slot costs **1.8 s** and the self-lapping seam drill
**6.8 s**, so the tests run on the real devices rather than on a reduction of them.

The lap case falls out as predicted, and in the sharper form: the self-lapping seam drill's two
footprints land on **two different regions** (the body and the tail plateau), which is what makes
the region worth carrying rather than searching for.

**The body closes.** Near cap, walls and far cap sew into a sphere — `free_edges = 0`,
`non-manifold = 0`, `V − E + F = 2` — because the caps share one triangulation and the walls share
the caps' boundary edges, all by edge *identity*, with no coordinate compared. OCCT's independent
audit agrees (`closed`, `BRepCheck valid`). That is a real check on the tracer: a footprint that
self-crossed or dropped a vertex could not produce it, whatever ε it certified at. It is **not** a
warrant for the geometry, and the guard against reading it as one is structural — see Packaging.

A **metric** cutter (a drill, a half-space) has no sketch plane to cast back to, so it gets its far
cap and nothing else: an open patch that says so via `BodyReport::solid`, rather than a silently
omitted hole or a wall ruled to an invented plane.

**Packaging.** One compound with the folded sheet, per the user's call, assembled with
`Brep::absorb` — an id-shift, so what was watertight stays watertight and what was separate stays
separate. The FFI question is now measured rather than assumed: **zero shim work**, the existing
`write_brep` takes the 325-face compound as is (20 226 STEP entities). **Diagnostic geometry must
never route through `emit_certified_step`**: a picture that carries a certificate is a lie about
what was checked. The compound is open regardless — one sketch face is all boundary — so it cannot
pass a closed-shell check even by accident.

**The differential that makes the pair worth emitting.** The near cap and the sketch face are the
same closed curve reached by two computations that share no code: one from the authored profile
edges through `Frame::point`, one from the traced footprint through the chart and back down the
generatrices. Measured on the L-slot device, every near-cap vertex lies **1.3 · 10⁻⁹** from the
authored outline — the `hole_poly` snap grid, *not* the cut's certified ε ≈ 7 · 10⁻⁴, because the
tracer walks the exact wall equations and casting back lands on the profile itself. Nothing in any
certificate would report these two disagreeing; ε bounds the cut, not the correspondence.

## 8. Where the code lives

A new shell-tier crate, **`interchange`** (DXF is literally the Drawing *Interchange* Format), on
`lattice`, `geom`, `arrange2d`, `develop`, `export`, `author`.

The point of a separate crate is **dependency containment**: `dxf`, `roxmltree` and `svgtypes` enter
the workspace in exactly one place and no certified path acquires a third-party dependency. The
`no_float` lint scopes to `lattice`/`certify_core`/`arrange2d`, so nothing there needs changing; the
floats in this crate are parse transport (§2.1) and writer output, both above the quarantine.

Three small API additions the dump needs, made as general accessors rather than as dump-shaped
special cases (no-ossification): a public read-only `Part::cutters()` over the `(OpKind, Cutter)`
list; `PartSolid::into_brep()`, the by-value counterpart of `brep()` for a caller assembling a
compound; and `Brep::absorb`, the compound operation itself. Nothing from the tracer needed
widening — `realize::footprints` sits inside `author` alongside the dump, so the resolver internals
stayed `pub(crate)`.

**SVG** is read through `roxmltree` + `svgtypes` (not `usvg` — it drags fonts and a rasterizer):
XML is a real grammar and the path mini-language is genuinely fiddly. Cost: **8 crates**.

**DXF is read directly**, reversing the earlier pick on a measurement taken before building on it.
`dxf 0.6` costs **27 crates** — `image`, `chrono`, `uuid`, `serde`, `moxcms`, `getrandom`, `libc` —
to read five entity types whose entire ASCII grammar is "pairs of lines". Size is the smaller half
of the argument: the crate hands coordinates over as `f64`, which discards the file's own decimal
text and with it §2.1's exact number bridge. Reading the text ourselves is both smaller **and more
exact** — it is what makes DXF's transport error zero rather than ulp-bounded (§2.3). The reader is
~250 lines and refuses binary DXF by name.

Writers (IO.2) are hand-rolled for both formats, since we control our own output and the DXF
dependency is now gone either way.

### 8.1 The SVG writer is a drawing, not a viewer

`export::svg::polys_svg` pads the frame by 8% and sizes in pixels: it is a diagnostic viewer and
should stay one. A fab SVG is a different artifact — 1:1 physical units (`width="120mm"` with a
matching `viewBox`), no padding, outline and holes on distinguishable layers, and the pattern's own
certified ε plus its worst vertex-box radius in a header comment. Both paths ship; the demos choose.
Do not ossify the viewer into a drawing.

## 9. Slices

| slice | task | what lands |
|---|---|---|
| **IO.0** | #282 | this document + vv-guide criteria + vv-matrix rows + pinned pre-state |
| **IO.1** | #283 | the `interchange` crate, DXF + SVG reading, §4 arcs, §5 units, §6 refusals, the report type |
| **IO.2** | #284 | DXF + SVG writing: flat-out (§8.1 drawing) and sketch-out |
| **IO.3** | #285 | the sketch face (§7a) + the cutter bodies (§7b), one compound, `write_brep` only |

**IO.1's acceptance criterion is a real device outline file from the user cutting a part end to
end** — not a synthetic one. A synthetic fixture proves the parser handles what we thought to write;
a real file is the only thing that proves it handles what a fab house emits. Until that file exists
IO.1 can be *implemented* but not *accepted*.

## 10. Scope exclusions

Named so that "we could also…" does not expand the milestone from inside:

- **No healing.** No gap closing beyond §4.4's junction rule, no self-intersection repair, no
  duplicate-entity merge, no arc/line tangency snapping. A file that does not describe a closed
  outline is refused with the gap reported.
- **No DXF blocks, INSERTs, layers-as-semantics, or 3D entities.** A flat outline in model space.
- **No SVG styling, text, groups-with-semantics, `<use>`, clip paths, or CSS.** Geometry only.
- **No splines or ellipses in either direction.** §6, and lifting it means a new `Edge` carrier —
  an `arrange2d` question, not a translator one.
- **No unit *inference*.** §5.
- **No ECAD formats** (Gerber, IPC-2581, ODB++). Later, and a different shape of problem: those
  carry a stackup, which is exactly what this stage of the flow does not have yet.
- **The dump is not a certified artifact.** §7.

## 11. Pinned pre-state (what is true before IO lands)

Measured on `main` at `de9a31e`, so that "IO changed this" is checkable rather than remembered:

- **No crate reads geometry from a file.** The only `fs::read*` calls in `crates/` are
  `export::step` re-reading a `.step` it just wrote (its own round-trip check) and the fuzz-replay
  corpus loader. Every device is a Rust literal in `crates/acceptance`.
- The only geometry writers are `export::svg` (diagnostic — 8% padding, pixel width) and the STEP
  bridge.
- `Part::ops` is `pub(crate)` — no public way to walk a part's cutters.
- The workspace's entire third-party surface is `dashu` (lattice's bignum backend), `cxx` +
  `cxx-build` + `pkg-config` (the `step` feature), and `proptest` / `num-bigint` / `num-rational`
  (dev only). Whatever IO adds should stay inside one crate, and the count is checkable.
- `Profile::arc` is total: an arc whose endpoints are off its circle is emitted as drawn.
