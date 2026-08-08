# Closure (M4) scoping report — the CLOSURE treatment for one joint

*Milestone C, phase C0. The recon/de-risking gate before any soundness code. Companion to the
approved plan and to `vv-guide.md §8` (M4 acceptance criteria). Records the per-conjunct
disposition, the generality analysis, the MITER-FIT-planar tractability argument, the joint data
model, and the GO decision.*

## 1. What CLOSURE is

`closure` (M4) implements the **searcher** for the `CLOSURE` treatment obligation `CLOSURE_VALID(j)`
(spec §8.6, lines 411–439): given a **joint** — two developable flank charts meeting along a
**straight crease** — produce the fields and certificates that show the joint closes into a
watertight cap. Soundness lives in the pure-tier `certify_core` **checkers**; `closure` is untrusted
(the arrange2d/certify_core split, applied again).

```
treatment CLOSURE ⇒ REG-V ∧ WEDGE ∧ EXT-WEDGE ∧ SIDE(b_J) ∧ COLLAR
                   ∧ CLIP-DOM(G_A) ∧ CLIP-DOM(G_B) ∧ TRIM-LOCAL ∧ FLANK-FIT
                   ∧ TUBE-LOCAL ∧ TUBE-SELF ∧ REMOTE(j) ∧ VERTEX(j) ∧ straight-crease scope
                   ∧ CLOSURE-CAP(j) := MITER-BRANCH(j) ∨ LEDGE-BRANCH(j)
                         MITER-BRANCH := MITER-FIT ∧ MITER-EDGE-LEDGER ∧ MITER-OUT ∧ SEW
                         LEDGE-BRANCH := CAP-IN-D24 ∧ LEDGE-DOM ∧ CAP-OUT ∧ SEW
```

The two branches are a genuine **disjunction of constructions** (§8.6): a *clean miter* pairs the two
flanks' cut edges directly (branch 1, emits no planar cap cells); a *forced ledge* builds a planar
cap region by boolean arrangement (branch 2, emits the connected components of the selected region).
`SEW` closes both — it is **M5**, out of this plan; M4 produces its input signature (EDGE-OCCUPANCY).

## 2. Generality — the cone is not enough, and the spec says so

The user's challenge ("we are building a general kernel, not a 42°-cone kernel") is spec doctrine.
**§13 is titled "the petal disk is co-normative — three bug families were invisible on the cone"** and
states the cone device "is structurally blind to **fold, stall, and directrix** bugs, hence the
petal's status." Verified this session: the *engine* is already device-agnostic
(`geom::chart::Chart::new(q, h)` is parametric over any rational quaternion spline + support;
`geom::tags` classifies with offset apexes; `lattice`/`certify-core`/`arrange2d` carry no device
constants). The gap is the **corpus**: the only shipped device is the cone; `cylinder()` lives only
in a `tags.rs` unit test; there is no plane and no second angle.

**Consequences for M4, adopted as constraints:**
- The closure searcher and the new checkers take **any two `geom` charts** — flank *type* (cone,
  cylinder, …) is data, never a control-flow branch.
- **A genuine plane is not representable as a `Chart` today** (see §8 below — a C0 finding). The
  representable developable with straight cut-edges is therefore the **cylinder** (rulings ⇒ ruling
  *lines*, spec §8.5 line 383), which — unlike a plane — also has a genuine moving normal to drive the
  per-flank regularity bundle. So the **representable co-normative first slice is the cylinder-flank
  joint**: its cut-edges are lines, so CAP-IN-D24 passes and it exercises *both* the LEDGE branch and
  the degree-1 MITER branch. The true §13 planar-hub petal disk is deferred with the planar-span
  representation.
- CAP-IN-D24 passes for **line-carrier** flanks (planar or cylinder-type: w±-images ⇒ lines/rulings)
  and *fails* for **conic** (cone/oblique/generalized ⇒ conic, spec §8.5 line 383). So the **cone** is
  the second, contrasting class: it demonstrates CAP-IN-D24 *correctly refusing* a conic cap (the
  class distinction is real, not cone-hardcoded), and MITER-via-resultant where it lands.
- The corpus grows to ≥2 representable developable classes (**cylinder + cone**) and ≥2 cone angles
  (≠ 65/97) so generality is *exercised*, not merely *claimed*. The petal's **conic** flank (the
  fold/stall/directrix adversary) and the genuine **planar hub** are the second pass, blocked on the
  §13 petal geometry / the planar-span representation — out of this plan.

## 3. Per-conjunct disposition

Legend: **reuse** = an existing checker verifies it; **new** = a new `certify_core` checker;
**searcher** = new `closure` code that produces the certificate/fields; **trivial** = discharged by
the straight-crease/planar scope; **out** = M5 or the conic second pass.

| conjunct | disposition | where |
|---|---|---|
| REG-V (\|V\|² ≥ m, fan sector sub-π) | **new** checker + searcher | `certify_core` (gauge like `reg_q`) · `closure` builds V |
| WEDGE (per-t wedge embedding) | **new** checker + searcher | `certify_core` · `closure` |
| EXT-WEDGE (s_bev(1+s_bev)\|V\|² < 1) | **new** checker | `certify_core` |
| SIDE(b_J) (one-sided-w sign + trim complementarity on {Q⋛0}) | **new** checker + searcher | `certify_core` · `closure` builds b_J |
| COLLAR (quotient-wedge; straight-crease scope) | **new** checker | `certify_core` (WEDGE per-t ∧ TUBE cross-t + D_collar) |
| CLIP-DOM(G_A), CLIP-DOM(G_B) | **reuse** + searcher | `certify1d::clip`/`clip_dom` · `closure` builds G_i |
| TRIM-LOCAL | **reuse** + searcher | `certify1d::trim_local` · `closure` |
| FLANK-FIT (wall id ∧ gap-side G_A>0 ∧ local disjointness) | **new** checker | `certify_core` (tube proxy in CLEAR pair matrix) |
| TUBE-LOCAL, TUBE-SELF | **trivial** (κ_max = 0 on a straight crease ⇒ total discharge / vacuous, §13) | `closure` records the discharge |
| REMOTE(j), VERTEX(j) | **thin** (single isolated joint; VERTEX reuses the link classifier) | `closure` · `certify_core::arrange` |
| straight-crease scope | **new** (scope gate: crease is a line) | `closure` |
| **MITER-FIT** | **new** — the deep part; planar = degree-1 corollary | `certify_core` + `closure` searcher |
| MITER-EDGE-LEDGER | **new** (materialize PAIR-IDENTICAL + EDGE-OCCUPANCY) | `closure` searcher |
| MITER-OUT (EDGE-REG / EDGE-EMB / EDGE-EDGE / CYCLE) | **reuse** EDGE-REG (`edge_reg`) + **new** rest | `certify_core` |
| **CAP-IN-D24** (input license + `CanonicalEdge`/`ValidatedD24`) | **new** — full census over `validate_d24` seed | `certify_core` mint · `closure` searcher |
| **LEDGE-DOM** (§6 steps 1–8) | **reuse** — already built | `arrange2d::boolean::ledge_dom_certified` |
| **CAP-OUT** (region postcondition, CAP-OUT-LINK) | **reuse** — already built | `certify_core::arrange` (inside `ledge_dom_certified`) |
| **SEW** (SEW-EDGES ∧ SEW-LINK) | **out** — M5; M4 emits its EDGE-OCCUPANCY input | `certify_core::sew` (stub today) |

**Reuse is the headline.** LEDGE-DOM = the §6 cell-construction steps (1)–(8) — half-edge
arrangement → seed (0,0) → operand sidedness → ℤ₂² cocycle → coincident incidence → boolean select →
emit separating edges → emit on the π₀ quotient — is *exactly* what M3d/M3e already built and Kani-
proved (`slab_locate`, `Face{outer,holes}`, `emit_region`, `cocycle_ok`, `ledge_dom_certified`, which
already calls the `validate_d24` totality seed internally). The CLIP-DOM ladder, TRIM-LOCAL, and
EDGE-REG are the M2 `certify1d` checkers. **The genuinely new work is the joint searcher, the
CAP-IN-D24 license census, the per-flank regularity bundle, and MITER-FIT.**

## 4. The joint data model (built in `closure`, from two charts)

Per-chart fields (`n`, `r`, pedal `c`, `C`, `det J`) come from `geom`. The **joint-level** fields are
new and computed in `closure`:

- `x₀` — the closure origin, a rational point on the straight crease line.
- `b_J = s_J·(n_A − n_B)` — the oriented bisector; `s_J ∈ {±1}` the joint orientation. `b_B = −b_J`,
  `b_A = b_J`.
- `G_i = (C_i − x₀)·b_i` — the **retained-side field** per flank; kept side uniformly `G_i ≥ 0`. The
  raw `H_i` is diagnostic only — never in a predicate or gate (spec §3.4, §8.5).
- `V`, `s_bev` — the fan/wedge generator and bevel slope (REG-V gauge `|V|² ≥ m`; EXT-WEDGE
  `s_bev(1+s_bev)|V|² < 1`). Exact geometric formulas pinned in C2.
- The **crease line** and each flank's cut-line / w±-image / μ̂±-sidewall images, as `geom::content`
  `Edge`s (`Line`/`Circle` carriers) — the input to CAP-IN-D24 and (planar) to `ledge_dom_certified`.

`closure`'s public entry takes `(Flank_A, Flank_B, crease, orientation, retained-μ-ranges)`, where a
`Flank` wraps an arbitrary `geom::Chart` (a **strip** span — cone, cylinder, …). Nothing keys on the
flank being a cone. A planar-span flank (`n′ ≡ 0`) is a deferred `Flank` variant (§8).

## 5. MITER-FIT, planar = the degree-1 corollary (tractability)

MITER-FIT pairs the two flanks' cut edges via the crease line: in the transverse regime `ℓ_i(σ)` is
rational and monotone (Sturm), and `φ_J` is implicit as the resultant `R(σ_A, σ_B) = 0`. **For
line-edge flanks (planar or cylinder-type) the cut edges are lines, `ℓ_i` is affine, and `R` is
degree-1 — so `φ_J` is an explicit rational map and no resultant machinery is needed.** `ε_φ` (the
branch's orientation bit) is the
**order sign of the monotone correspondence**, minted by *one exact oriented-endpoint comparison* — a
theorem on the regular locus, never the derivative-sign definition (the `σ_B = σ_A³` fossil: strictly
monotone, positive endpoint order, derivative zero at 0 — the ★ soundness point Kani will guard).
Tractable; this is the degree-1 base case before the conic pass. **GO.**

## 6. Searcher / checker boundary

- `closure` (searcher, shell tier): joint model + `b_J`/`G_i`/`V`; builds flank `Edge`s; runs the
  CLIP/MITER searches; emits `(claim, certificate)` bundles. Untrusted.
- `certify_core` (checkers, pure tier / the extraction & TCB surface): CAP-IN-D24 mint
  (`CanonicalEdge`/`ValidatedD24`), REG-V/WEDGE/EXT-WEDGE/SIDE/COLLAR/FLANK-FIT, MITER-FIT, MITER-OUT;
  reuses `certify1d::{clip, clip_dom, trim_local, edge_reg, reg_q, slab_s0}` and
  `arrange::{link_ok, boundary_bijection_ok, cocycle_ok, …}`.
- `arrange2d` (searcher, already built): `ledge_dom_certified` = LEDGE-DOM + CAP-OUT.

`no_float` dylint scope decision: `closure` is shell tier (floats already impossible via the exact
`Rat`/`geom` types it consumes), so it is **not** added to the `no_float` scope in C0; the pure-tier
checkers it feeds (`certify_core`) remain in scope, which is where a float would be a soundness bug.

## 7. Decision: **GO**

Every CLOSURE_VALID conjunct maps to reuse, a bounded new checker, a trivial straight-crease
discharge, or the M5/conic/planar out-of-slice. The hard part (MITER-FIT) is degree-1 for the
line-edge (cylinder) exit fixture, which drives *both* branches. The exit device is a representable
spec-grounded stand-in for the §13 petal disk (straight-root joint, line cut-edges); the genuine
planar-hub petal is deferred with the planar-span representation (§8). Proceed to C1 (CAP-IN-D24
census). The phased build (C1–C6) is in the approved plan; each phase is a green commit with a pause +
report, and the per-phase acceptance criteria are in `vv-guide.md §8`.

## 8. C0 finding — the plane is not a `Chart` (the vertical slice is cylinder-first)

The approved plan assumed a "two-planar-flank" first slice with a `plane()` fixture. **C0 recon
refutes that at the representation level:**
- The spec (§ line 81) distinguishes a **`strip`** span (`|n′| > 0`) from a **`planar`** span
  (`n′ ≡ 0`, a coefficient identity). `geom::chart::Chart` implements only the *strip* case — it
  debug-asserts `|n′|² ≢ 0` (`chart.rs:106`, *"no ruling"*) and its whole field calculus divides by
  `|n′|²` (pedal, `det J`). A genuine plane (constant normal) has `n′ ≡ 0`, so it **cannot** be a
  `Chart`, and `geom` has no planar-span type today.
- The crease itself is, per spec (§ line 106), *"a Gauss-map jump across a shared ruling — an
  interface object, inexpressible inside one smooth chart"* — i.e. the joint is genuinely between two
  charts, which is the model here.

**Resolution (adopted, low-regret):** the representable developable whose cut-edges are straight
**lines** is the **cylinder** (spec §8.5 line 383, *cylinder-type ⇒ ruling lines*), and unlike a plane
it carries a moving normal that drives REG-V/WEDGE/SIDE/COLLAR. So the vertical slice is **cylinder-
first**: a cylinder-flank joint has line cut-edges (CAP-IN-D24 passes) and exercises *both* the LEDGE
and the degree-1 MITER branch — the same both-branches exit the plan intended, on representable
geometry. The **cone** is the contrasting second class (conic cut-edges ⇒ CAP-IN-D24 correctly
*fails*). This directly serves the generality mandate (§2): cone **and** cylinder, two genuinely
different developable classes.

**Deferred (recorded, not dropped):** the **planar-span representation** (`n′ ≡ 0` — a `PlanarChart`
or a relaxed `Chart`, with its own pedal/ruling calculus) and hence the *genuine* §13 planar-hub petal
disk. This is a `geom` (M1-adjacent) feature, logged for a dedicated pass; it should not balloon M4.
This deviates from the approved plan's "plane()" / "two-planar-flank" wording — surfaced at the C0
pause for confirmation before C1.
