# Thickened Developable Atlases with Certified Development: An Exact Rational Representation for Formed Flexible Circuit Substrates

*Working design note in paper form, v23 — July 2026, synced to "Flex Substrate Representation — Spec v0.24 (consolidated, standalone)"; where the two disagree, the spec's full text wins.*

*Figures are referenced as sibling files (`fig*.png`); a self-contained HTML render accompanies this source.*

---

## Abstract

Flexible printed circuits are authored flat in ECAD and formed into three-dimensional shells — rolled, folded, petaled — sometimes closing on themselves through bonded lap seams. Simulation and generative design both require a faithful, invertible correspondence between the flat artwork and the formed solid, including across seams and folds. We propose a representation in which the correspondence, not the geometry, is primary: a *thickened development map* over an atlas of developable charts, each chart stored as a rational curve in oriented-plane space via a quaternion (Hopf) parametrization of its normal field. Developability, unit normals, and constant-thickness offsets are algebraic identities; chart geometry, the stackup, tooling surfaces, and lap-seam face pairs are exact objects over a constructible coefficient field. Exactness is allocated deliberately: **chart geometry is exact, authored flat content is exact, and the material correspondence is certified** — every content-induced 3D object carries an anchored backward-error guarantee: it is the exact image of a flat item within one refinable stamp of the authored one, with exact fiber-field geometry over its anchor. Charts connect through an interface layer organized by material continuity; straightness of a joint is derived, not declared, every curved joint is a fold, and a material-continuous fold has *no free functions*: its mate flank is determined by reflection in the crease's osculating plane, making zero net concentrated curvature a theorem of the construction. Fold smoothing is governed by finite-band Gauss–Bonnet audits over the deformed metric, with the sharp skeleton as the immovable authoring datum. Validity is a definedness–immersion–embedding ladder whose certificates are typed by expression class and quantifier dimension: algebraic statements decide totally in one variable and margin-conditionally in several (Sturm; Bernstein), development-composed statements decide refinably to stamps, and global embedding is certified by interval subdivision with *witnessed* algebraic discharge rules eliminating most chart pairs — on the driving instance, all but a thin seam-ramp neighbourhood, which subdivision certifies. That instance is a conically rolled four-layer board with a bonded overlap, whose seam transition reduces, within a constant-slope ansatz, to a scalar control problem with a closed-form conservative solution.

---

## 1. Introduction

### 1.1 Motivation

Conformal electronics increasingly rely on flexible circuit boards designed as planar artwork and formed into shell-like 3D configurations. The driving application is a rolled conical substrate for a miniature wireless platform: a four-layer, 240 µm flex board wrapped into a cone of 42° half-angle and 5 mm inner diameter, closed by a 1.6 mm bonded lap seam (ACF, 0.5 mm pad pitch). The platform's roadmap includes bent-out petals, rigid-flex islands, and doubly-curved sensor regions — the representation must not be a cone-shaped special case.

The toolchain stress is specific: artwork lives flat (ECAD); physics lives formed (residual strain, EM, contact); iteration moves both directions. The correspondence must be (a) constructible before the artwork exists, (b) exactly consistent under thickness offsets, (c) meaningful across seams and folds, and (d) certifiable, because downstream consumers silently amplify representation error into tolerance conflicts.

### 1.2 Why the obvious approaches fail

Numerical flattening (LSCM/ARAP, commercial unfolders) is a posteriori, conformal-not-isometric, mesh-inverse, seam-blind, and workflow-reversed. Ambient volumetric maps have no isometry semantics. A forming-FEA displacement field is mesh-tied, expensive, and exists only after the design it should define. Kernel workflows fail at exactly the seam: lap faces are tangent-parallel offsets, the pathological case for floating-point booleans.

It is tempting to demand an ambient diffeomorphism of ℝ³ carrying the flat plate to the formed solid. The representation makes no such demand and would gain nothing from one: nothing in the pipeline evaluates the map off the material slab, so ambient data is unused where it exists and unmissed where it doesn't — and whether a given formed configuration is reachable from the flat state at all is a property of a *forming path*, owned by the forming layer, not by validity. The zero-thickness variant is worse than useless at the seam: no single-valued graph over the selected reference surface represents both overlap sheets — and the surface formulations that *are* injective achieve it precisely by encoding thickness in the support offset, the information the zero-thickness framing claims to do without. The correct requirement is both weaker and checkable: injectivity of the map on the thickened plate.

### 1.3 Contributions

1. **The thickened development map as the semantic core.** The device is the image of a map Ĉ from a flat, thickened domain; validity is injectivity on the slab — locally a signed one-sided Jacobian condition, globally clearance — precisely the level at which a lap seam is expressible: over the joint no single-valued graph over a chosen reference surface represents both overlap sheets; the separation is thickness data, whether carried in w or encoded in supports.

2. **A plane-space representation of developable charts.** Each chart is (q(σ), h(σ)): a polynomial quaternion spline for the unit normal n = q e₃ q̄/|q|² and a rational support spline. Developability and |n| = 1 are identities of the formulas; validity is a separate open condition — a ladder of strict inequalities — changing the *kind* of constraint a generative loop faces. Surfaces, rulings, normals, and all offsets (h ↦ h + w) are exact; the second fundamental form is closed-form.

3. **Exactness allocation with anchored backward error.** Chart geometry exact; authored flat content exact over a constructible field lattice; the material correspondence certified, confined to two 1D quadratures per chart. Every content item — point or curve — carries an **anchor** (rational representative or rational spline) with a uniform refinable stamp: the computed 3D object is the exact image of a flat item within ε of the authored one, and everything over the anchor (thickness fibers, trace faces, sidewalls) is exact relationally, along the whole curve. One stamp per item; no compounding.

4. **An interface layer organized by material continuity.** Joints are 1-cells with per-side ranges; planar charts are hubs with explicitly stored exact datum frames. The load-bearing distinction is whether fibers cross the joint locus (MONO) or the locus is a cut realized by assembly (BONDED). Straightness is a theorem — straight joints are forced onto rulings — and straight-joint contracts are enforced by sharing, making 3D accumulation structurally impossible. Seams remain a disjoint thickness-face mechanism, valid across separately authored blanks.

5. **Fold theory with a determined mate.** Tangent-continuous non-ruling joins are impossible (envelope lemma), so every curved joint carries a dihedral — and a material-continuous fold has **zero free functions beyond its crease**: material identity forces the mate's tangent plane to be the reflection of the host's in the crease's osculating plane, a rational, normalization-free formula. Zero net concentrated curvature becomes a theorem of the construction; the free-profile variant is the *bonded* fold, reserved with its mismatch accounting.

6. **Curvature accounting on honest ground.** Concentrated curvature splits into **charges** (intrinsic, cut-and-bond, conserved — reserved) and **loads** (extrinsic pinning residuals — all of v1). Band attachment, not a per-station integral, is the normative constraint; finite-band Gauss–Bonnet over the *deformed* metric is the audit, telescoping hierarchically and localizing its own failures; the sharp skeleton is the immovable datum, so re-relaxation moves no artwork.

7. **Certificates typed by expression class × quantifier dimension.** Algebraic statements (over the coefficient field) decide totally in one variable (Sturm) and margin-conditionally in several (Bernstein: proved on positive margin, unresolved at the boundary — where a design has zero fabrication tolerance anyway); development-composed statements decide refinably to stamps, terminating on open margins; integral functionals route through validated quadrature into function models. Identity-shaped validity conditions must be algebraic or structural. The design's measurable quality claim: it maximizes the algebraic-univariate population — on the developable stratum, everything in validity except residual pair-clearance, and even there *witnessed* algebraic discharges eliminate most pairs before subdivision (on the driving device, all but the ramp neighbourhood — the one honest residual, certified by subdivision over a thin box).

---

## 2. Problem setting and design axioms

### 2.1 Pipeline

(1) declarative spec; (2) compilation into atlas + interfaces + development + certificates; (3) flat export to ECAD with datums, keep-outs, ghosts, fold lines; (4) artwork authoring; (5) exact import; (6) forward mapping to solids and meshes; (7) analysis; (8) generative design with pullback — and every certified→authored transition passing through one freeze gate.

### 2.2 The driving instance, quantitatively

β = 42°, ID 5 mm: κ₁ = cos β/r ≈ 0.297 mm⁻¹ at the inner edge (R₁ ≈ 3.36 mm, r/t ≈ 14). Copper at w = ±120 µm sits at ≈ 3.6% — fully plastic, one-time forming, large springback. Consequences: the isometric fiber's position in the stack is a *calibratable* datum — a 10 µm error in z_N produces **2π cos β × 10 ≈ 46.7 µm** of azimuthal seam misregistration per wrap (the 2π-per-wrap figure is the cylinder limit; the cone's factor is cos β) — and springback compensation gets a first-class slot. One wrap develops to 2π sin β ≈ 240.9°; the seam identification is that rotation composed with a radial shift Δ cot β ≈ 0.28 mm for a midplane-to-midplane offset Δ ≈ 0.25 mm. Manual "rotate the sector" pad placement is wrong by a quarter millimeter.

![Formed device](fig1a_device3d.png)
*Figure 1a. The formed device at true scale, evaluated through X = c + μr: base cone (h = 0), the 60° constant-slope ramp, the tail on the offset cone (h = Δ).*

![Axial section](fig1b_crosssection.png)
*Figure 1b. Axial section through the bonded overlap: two thickened slabs sharing one normal field, separated in w by the bond gap; normal projection onto a chosen reference cone is two-to-one (gap exaggerated).*

### 2.3 Axioms

**P1** map primary. **P2** chart geometry exact · authored flat content exact · correspondence certified, anchored backward-error form. **P3** flat authoritative, always; 3D-side authoring is a projection gesture terminating in a freeze; marks are advisory. **P4** developability a stratum. **P5** authored content closed under exact booleans; fab geometry from authored operands only, the freeze being the sole certified→authored gate; marks participate only in three-valued tube predicates. **P6** every non-exact quantity certified.

---

## 3. The thickened development map

With Ω the developed domain, F the mid-surface map, N its normal:

```
F̂(ξ, η, w) = F + w·N,        w ∈ [w⁻, w⁺].
```

Validity is injectivity of F̂ on the material region: locally a positive thickened Jacobian — on the *developable stratum* the signed one-sided condition **inf(R₁ + w) > 0** (regression threatens only the concave side; the +w direction is unconditionally safe by the structural coefficient; the symmetric |w|κ₁ < 1 is the special case for symmetric ranges with R₁ > 0), while on the strained stratum it is a genuine two-sided quadratic-in-w condition with neither side safe — globally a self-clearance condition outside intrinsic adjacency.

The lap seam is where injectivity becomes intrinsically thickened. With π the normal projection onto a reference surface, π ∘ F̂ is two-to-one over the bonded overlap: the device is not a graph over its own shape, and no zero-thickness description carries the joint. Whether the mid-surface map is injective is parameterization bookkeeping — the canonical convention (every chart's w = 0 its own stackup midplane, assembly as relative support data) makes it injective outright, the offset being thickness data carried in the support; the two-to-one projection survives as a derived view onto a declared reference chart. Invariant content either way: sheets separate purely in w, and the nontrivial certified facts are thickened — face separation ≡ bond gap (SEP, exact by construction, §7) and the one-sided slab condition (SLAB).

The representation owes physics only kinematics: F̂ − id as forming boundary condition; the closed-form isometric roll family as a BC schedule. The pullback metric is a compile-time validity gate; physics enters authoring as scalar budgets and targets.

---

## 4. Plane-space representation of developable charts

### 4.1 Envelopes, candidates, and validity

A developable is the envelope of its tangent-plane family; a chart stores the family: σ ↦ (n(σ), h(σ)). The envelope conditions give the ruling r = n × n′ and the pedal point

```
c = h n + (h′/|n′|²) n′,        c·r = 0,
```

the foot of the perpendicular from the origin to the ruling — gauge-invariant, a property of the line. Then X = c + μr, C = X + wn, rational in σ, linear in (μ, w).

Storing planes changes the *kind* of constraint. **Developability is an identity** — every (q, h) candidate satisfies K = 0 wherever immersed, unconditionally — **while validity is the ladder**: definedness (regularity and denominator separation), immersion with orientation (the one-sided slab condition), embedding (clearance). Each rung is a strict inequality, so the valid set is *open in stored-parameter space*: the data model eliminates structural equalities rather than constraining them, stored parameters are coordinates on the structural stratum, and editors cannot leave it — unconstrained coefficient space is reachable only through imports, which enter by canonical projection plus a ledgered residual. Certificate **slacks** are barrier values with exact-or-certified gradients, consumable directly by interior-point loops; a parameter-space trust radius is available as slack over a certified sensitivity bound — slacks are *not* coefficient-space distances, and treating them as step radii would fail exactly near the degenerate strata where sensitivities spike. The contrast with constrained ruled NURBS stands: there developability is an equality variety forcing projection-onto-manifold optimization; here the equality vanished into the formulas and inequality barriers remain. Degeneracies are handled, not wished away: q-zeros deflate; isolated n′-zeros are classified by exact vanishing orders — removable parametrization stalls (the *complete* chart record factors through a polynomial substitution **with its reparametrization weights and the full coordinate pullback** — the substitution emits a *standard* chart over the canonical parameter s = εχ (signs normalized at birth, never runtime map data), and every field pulls back through the base map (σ, μ) ↦ (s, |χ′|μ) with its value-law on top: joint fields u(σ,μ) = U(s, |χ′|μ), director coefficients |χ′|pᵢ = Pᵢ(s, |χ′|μ), ruling-linear bounds |χ′|·g = Ĝ(s), jets by the corrected chain-rule triangle; composition in the first argument alone samples the quotient at the wrong ruling coordinate, and the acceptance test is that substitution commutes with every physical query at corresponding material points — reparametrized exactly, parity-splitting first where the stall factor reverses sign, since those branches cover opposite sides of the same rulings; residual stall *endpoints* are carried by an orientation-faithful hatted deflated calculus), genuine envelope singularities (trimmed along the singular ruling), or Gauss-curve singularities (split into an analysis-discovered interface, bridged by a planar wedge, or rejected); a planar chart requires n′ ≡ 0 on a span, never a zero at a point; slab failures are often domain overreach, cured by Sturm-localized trimming.

![Envelope](fig2a_envelope.png)
*Figure 2a. 2D analogue: a line family, its envelope, the pedal hn, the contact point hn + h′n⊥; curvature radius h + h″.*

### 4.2 Quaternion parametrization; the gauge fiber resolved

n = q e₃ q̄/|q|², q a polynomial quaternion spline — rational of degree 2·deg(q), exactly unit. Alternatives: raw rational curves need |n|² = 1 as a fragile quadric; stereographic has a pole; homogeneous dual Bézier is polynomial but its offsets leave the ring, and exact offsets are the point.

The two-dimensional gauge fiber (scale, and the right e₃-phase that rotates R(q)'s tangent axes about n) is resolved semantically, not by canonicalization. On strips a **phase-blindness lemma** holds: every derived quantity is a function of the n-path and h alone — the full frame never appears — so the fiber is semantics-free there. On planar charts the in-plane orientation is *design data* and is stored explicitly as an exact frame (origin, in-plane direction d with d·n = 0), R(q) demoted to a constructor's convenience — killing the failure mode a coefficient-derived canonicalization would re-import, where a remote edit re-picks the phase and datums silently rotate. Fold rotations are safe (axes pinned to crease tangents); interface handles share n-level data only.

The stackup is structural: every layer, mandrel, and thermode is the same chart with shifted h. Calibration splits into two exact quantities with different invalidation costs. **z_N**, the isometric-fiber position, re-points the development source and the strain reference without touching 3D geometry: the flat frame becomes the development of the member h + z_N, and strain budgets bind about that fiber — the geometric midplane is a bookkeeping surface, not the unstretched one. **d_shape**, measured normal placement, is spatial surgery h ↦ h + d_shape, legal as atlas surgery on crease-free panels only: the exact offsets of a creased surface share no common crease line, and measured shape near hinges belongs in a stamped as-built overlay rather than the design atlas. Both are per-sheet constants (G1 handles force equality by sharing); z_N may additionally step across bonded cuts, and across straight creases with a declared swept-profile transition strip (equal values reduce to the single-arc allowance), while across a *curved* material fold any nonzero z_N routes through a strained band — the off-neutral connecting strip is provably non-developable, so no exact flat identification exists to certify; same-sheet seam data is invariant under both.

### 4.3 Second-order structure

```
ψ′ = det(n, n′, n″)/|n′|²,        det J = (c′ + μr′)·n′ + w|n′|² = |n′|²(R₁ + w).
```

In the analysis gauge, ψ′ = κ_g (flat ruling turning = geodesic curvature of the Gauss image), det J₀ = (h + h″) − μκ_g, regression at μ* = (h + h″)/κ_g. The slab condition is the signed one-sided **inf(R₁ + w) > 0** — the classical offset rule |w| < |R₁| is its symmetric-positive special case, and on the R₁ < 0 stratum that gloss certifies uniformly reversed orientation, so the honest form matters. 1/κ₁ = det J/|n′|² is exact and serves the mesh size field through the normative cap min(s_max, 1/κ₁) — on stall-end spans κ₁ is the regular field and 1/κ₁ is extended-real (+∞ along the flat generator), and planar charts always needed the cap anyway.

### 4.4 The degree-1 stratum

Degree-1 q makes n exactly a circular arc: the four components of |q|²·(1, n) are quadratics in span{1, σ, σ²}, hence linearly dependent — a + b·n ≡ 0 with b ≠ 0 (b = 0 forces a|q|² ≡ 0) — so n is planar, plane ∩ S² is a circle, and a conic sharing a continuum with a circle is the circle. (Unit normality is load-bearing: otherwise a rational quadratic is merely a conic.) Structurally, the line q₀ + σv projects to a great-circle arc of S³, and conjugation carries great circles to circles at doubled angle, degenerating exactly on the gauge-fiber directions. A single span covers the full circle minus one point; full wraps need several spans; |n′| never vanishes on a nondegenerate span. Classification: great circle ⇒ cylinder; small circle with h ≡ n·A ⇒ cone (an exact linear solve — a derived certificate); small circle with free h ⇒ constant-slope developable. The seam ramp is the third case: **the entire driving device is degree-1 in q**.

![Gauss image](fig2b_gauss.png)
*Figure 2b. The atlas as a curve on S² with h along it: the cone and all offsets share one circle; a great circle is a cylinder; deg-1 spans are exactly circular arcs.*

---

## 5. Exactness allocation and the number tower

### 5.1 Tiers and the coefficient field

**Tier E (exact).** Chart data, authored content, all chart-coordinate evaluations. Coefficients live in a **provenance lattice**: L0 declared-quantum rationals for bulk/ECAD content — each source declares its exact quantum (integer nanometres, Gerber steps, Altium's 25.4/10⁷ mm, per-value dyadic for binary floats), so faithfulness is per-source data rather than a global ring assertion, and native-quantum content round-trips to its source grid exactly; L1 constructible-field towers K = ℚ(α₁,…,α_m) for chart-grade and spec-authored constructions, under a tower-height budget (the compass-and-straightedge diet; 42° is constructible and carried exactly — the earlier "snap all angles" policy wasn't even self-consistent); L2 lazy expression DAGs as the escape hatch (real-algebraic isolating-interval numbers are reserved beyond degree two — moot while content curves are lines and circular arcs, whose intersections stay in the tower). Identities decide by sharing; strict inequalities by interval refinement, terminating because validity's set is open; exact zero-tests confine themselves to identity checks on imports via separation bounds. Predicate engineering keeps the expensive tier starved: clearance predicates compare squared distances (no √), and computed conservative regions round one-sidedly into the ring — semantically exact for conservative roles. One denominator discipline underwrites the tier: stored rationals default to positive-weight form (denominators bounded by the minimum weight structurally); the few derived atoms (Hopf denominator, Gauss-regularity numerator, fold osculating norms) are certified once per chart, and every derived field's denominator is a monomial in the atoms — pole-freedom is compositional.

**Tier A.** |n′| — the sole non-rational algebraic atom the S0 kinematic formulas themselves introduce (the μ ↔ ℓ conversion) — evaluated as certified intervals, never stored; coefficients own their own irrationals through the field lattice, and the formulas leak none.

**Tier C.** The development: two 1D quadratures per chart — the turning from a rational integrand, the directrix from the pedal velocity resolved in the oriented flat frame (rational components over the single algebraic speed atom; the pedal velocity is structurally tangent, c′·n = 0, and the development maps the *positively oriented* tangent pair (r/ρ, −n′/ρ) to the flat frame — the naively derived pair is left-handed for n, and an earlier draft's sign made the map non-isometric off the apex-cone/cylinder kernel, |D_σ|² − |X_σ|² = 4bℓψ′) — stored as refinable models. Everything composed through them inherits the certified class.

### 5.2 The allocation, precisely

Item by item: chart surfaces and offsets — exact; chart-native curves, seam face pairs, tooling, and reflection-fold curves over authored creases — exact (the last, exact-over-anchor); authored artwork — exact; mapped copper edges, vias, footprints, sidewall placement, mesh vertices — certified, each crossing D⁻¹ exactly once; STEP face surfaces exact, artwork-induced trims fitted and stamped. "Exact 3D" without the chart-coordinate qualifier is false under the architecture's own discipline; the correction is not a retreat because the certified class has structure generic certified geometry lacks:

**Anchors are function objects.** A point gets a rational representative in the certified inverse enclosure; a *curve* g(t) gets a rational spline â(t) sharing g's knots, with shared endpoint anchors at junctions (networks lift watertight structurally), Sturm regularity and transversal monotonicity on â's own data, and one **uniform** bound sup_t |D(â(t)) − g(t)| ≤ ε — a refinable model norm. Crossing safety is a comparison, not a computation: ε less than half the item's exact flat clearance (its DRC margin), so refinement is demanded exactly where content is tight. Downstream of â everything is exact again *along the whole curve*: the 3D edge is the exact image of the exact flat curve D(â) within ε of the authored one; sidewalls are exactly ruled with exactly straight fibers of exact along-fiber length; both trace faces are exact images of one perturbed curve (zero top/bottom registration — the guarantee thin-layer EM meshing needs). One stamp per item, no compounding. Two certified sub-tiers persist: inversion *to chart coordinates* is algebraic (Sturm-decidable predicates, pole-free bracketing); anything *through the development* is transcendental-certified. Floating point stays quarantined at the kernel and mesh boundaries; nothing round-trips.

---

## 6. Atlas structure: interfaces, hubs, and material continuity

### 6.1 Interfaces are 1-cells; charts are administrative

Strips are arcs in plane space, planar charts are vertices, interfaces are the meeting data: 1-cells with per-side ranges, storing the shared line as the handle (pedal + direction, line-invariant); 0-cells at range and fold endpoints; at most two sides per 1-cell. Hubs author their interface lines exactly in their stored datum frames. An interior C¹ knot and a straight G1 interface are the same geometry; charts are certification units, split where bookkeeping differs.

### 6.2 The material-continuity axis

A joint is **MONO** if fibers cross its locus, **BONDED** if the locus is a cut realized by assembly; blank count is irrelevant (a dart is one blank, BONDED). MONO pairing is material identity — exact, zero registration, content flows across; BONDED pairing is a design declaration realized by assembly. Version 1 implements MONO; BONDED is reserved — deferring less than it appears, since two-piece assemblies bond physically through laps, which is the seam mechanism, already first-class across blanks. Genuinely deferred: metric identifications and their analysis overlay.

### 6.3 Straight joints: straightness is a theorem

A straight line on a developable is asymptotic, hence a ruling; planar charts accept any direction. **G1** = shared plane and ruling line: with n′_B = λ_q n′_A, h′_B = λ_h h′_A, pedals coincide iff λ_h = λ_q — and the trap, correctly stated: independent factors translate the ruling line **perpendicular to itself within the shared tangent plane** (a gap if mismatched, a planar facet if an honest limit) — so the contract is enforced by one shared handle with one λ. **Crease** shares the line only: exact rational rotation about the unnormalized ruling, far-side support derived, fold angle derived. Both develop to straight, invisible flat identifications.

### 6.4 Graph structure; no placement graph

Per component: a forest; multi-component atlases legal; content holes free while one chart's forming spans them (chart-graph cycles deferred with their kinematic closure constraint). Charts are authored in world coordinates; interfaces make charts *coincide*: 3D accumulation is structurally impossible; all accumulation is flat-side, stamped, refinable.

---

## 7. Seams as assembly relations

A seam is not geometry: constraint node, identification, emission scope, at the thickness-face level.

**Constraint — the face identity.** The bonded configuration is the exact offsets-in-family statement *at the faces*:

```
h_A + w_{A,face} + g  =  h_B + w_{B,face}        identically on the bonded range,
```

constructed, not checked — the tail's support plateau is derived from the gap and the stackup fields. The midplane offset **Δ(σ) = g + w_{A,face} − w_{B,face} is a derived quantity**, constant iff the face stackups are uniform; layer-dropped or pad-topology faces give varying or piecewise Δ while the bond stays perfectly parallel and constant-gap. A vacuous earlier claim is retired here: with shared q, tangent planes at matched σ are parallel for *any* Δ — parallelism forces nothing. The correct correspondence lemma: the normal from face A lands on face B's same ruling iff the face-support difference is stationary. SEP becomes exact by construction: separation ≡ g, and the certificate compares two ring scalars.

**Identification J**, defined on the bonded range (where correspondence holds), in three tiers. Tier 0, always: identical turning and speed give σ↔σ, ℓ↔ℓ, so J is a **ruling-constant translation field** p ↦ p + Δγ(σ), the difference of two stored certified developments — certified free, the ramp's excess a certified constant at range entry. Tier 1: the translation collapses to rigid ∘ ruling-shear **iff κ_g ≡ k with a signed witness, k separated from zero, and Δ ≡ Δ₀** — the shear is **δ = −Δ₀/k** (the sign follows from the corrected development frame; the +Δ/κ_g of earlier drafts was consistent only with the pre-correction frame), and the separation hypothesis is what makes the cylinder fail *by certificate* rather than by prose (κ_g ≡ 0 passed the old written hypotheses into a division by zero; the correct map there is affine with cross-ruling scale (R+Δ)/R — Tier 2). All hypotheses are exactly decidable: constancy is a rational identity, the sign one sample evaluation, separation one ring comparison, Δ₀ a stackup-uniformity identity. The cone: κ_g = −tan β under the outward normal, so δ = −Δ₀/k = Δ cot β ≈ +0.28 mm — the positive number derived, not assumed — and every ghost-footprint figure stands.

**Emission.** Ghosts through the tier in force; tie/contact labels; EM junctions; exact inverse branch selection (support gap ≥ g — a sign test); pullback across the joint; tooling. Cross-component seams give two-blank assembly for free.

![Development and J](fig3_development.png)
*Figure 3. The development with the seam identification: one wrap is a 240.9° sector; mate pads land only through J — Tier-1 shear for the cone; the inset shows the 0.28 mm radial shift naive placement misses.*

---

## 8. The lap-seam transition

The outer end climbs by Δ ≈ 0.25 mm while remaining developable. Three lemmas: **(1)** the offset cone is coaxial with apex shifted d/sin β — same Gauss circle, supports differing by d; **(2)** generator creases fix the apex, so no fan of them reaches the offset cone; **(3)** support cannot change at a frozen normal.

These lemmas **select an ansatz, not exhaust admissibility**: they pick the family where n rides the cone's circle while h ramps — degree-1, minimal normal motion, reusing the turning the wrap already pays. Off-circle detours are admissible, conjecturally suboptimal (the first variation is two-sided; the question is open); the engineering case is independent: the ramp stays single-span-Sturm-certifiable, keeps shared q through the seam (feeding the identification its Tier-1 structure), and keeps the tail in the tag stratum.

Within the ansatz, the exact constraint is one-sided, state- and position-dependent:

```
R₁ = s tan β + (h + h″) ≥ s_binding(σ) tan β / f.
```

Strain bounds h″ only from below; flattening is free. The unconstrained one-sided optimum degenerates into an h″⁺ impulse — which is **not a crease**: an h″ concentration with continuous n inserts a *planar facet* between parallel rulings, tangent planes continuous, no dihedral (the crease, by contrast, is a Gauss-map jump — a different degeneration entirely, and an interface object, not a chart limit). The facet solution is a legal C¹ configuration — leave the cone tangentially, run a flat chord of width √(2A₀Δ) ≈ 0.9 mm, rejoin — and it *games* the Δσ objective because the facet has zero σ-measure but finite material width; under honest material-width accounting it survives as genuinely narrower (≈ 1.8 mm vs ≈ 2.7 mm at the binding station). What excludes it is neither strain nor mathematics but **conical fidelity**: the chord sits ~30 µm proud of the cone, won't seat on the mandrel, and breaks the constant-offset structure the seam's shared-q tier depends on over its extent. The tangent-chord bypass is therefore recorded as a legitimate, currently-excluded alternative. The working problem imposes the symmetric cap

```
|h″| ≤ A₀ = (1 − 1/f)·s_min tan β,
```

lower side owned by strain, upper side by conical fidelity — and its bang-bang solution

```
Δσ = 2√(Δ/A₀),        Δφ ≈ 60°  at f = 2
```

is a closed-form **conservative bound and initializer**, not the optimum: the state term relaxes only the deceleration arc, by at most √(A₀/(A₀+Δ)) ≈ 7%, and the exact one-sided optimum stays closed-form (parabola then sinusoid, (h + A₀)″ = −(h + A₀), one transcendental matching condition). Δφ ≈ 60° survives as the design number — keep-out sizing wants the conservative side. The membrane alternative (ε ≈ ½(Δ/L)²) needs ≈ 130° of azimuth at a 0.1% budget: the developable transition is what makes the joint feasible. The degenerate option h = Δ·φ/2π deletes the transition entirely at the price of a cross-section out-of-round by Δ per turn.

The crease-limit intuition is correct only with physics present: limits of many-crease isometries are merely C¹ (Nash–Kuiper); bounded bending energy regularizes to W²,² isometries, exactly the classical developables (Pakzad) — landing back on the h(σ) family, where the admissible degenerations remain the taxonomy above: facet (h″⁺), regression (R₁ → 0), and crease (n′-concentration, living at interfaces, not inside charts).

![Ramp](fig4_ramp.png)
*Figure 4. The symmetric-cap surrogate: h and h″ against the two-sided cap — strain owns the floor, conical fidelity the ceiling (top); the signed principal radius at s_min never enters the f = 2 floor (bottom). The bang-bang width is a conservative bound; the true one-sided optimum is at most a few percent narrower.*

---

## 9. Curved joints are folds: determinism, exactness, accounting

### 9.1 Two impossibility results

**Envelope lemma.** Developables meeting tangent-continuously along a non-ruling curve are locally identical — the curve samples the whole tangent-plane family. Every curved joint carries a dihedral.

**Flat-flank lemma.** A curved fold cannot keep one flank flat and unstrained: with crease curvature κ, each flank's flat trace satisfies κ_g,i = κ cos α_{F,i} (Fuchs–Tabachnikov); a flat flank pins κ_g = κ, the bent flank demands κ cos θ, and material continuity supplies the same trace to both. The deficit λ_eff = κ_g(1 − cos θ) is a **load** — a boundary-incompatibility residual payable only by a strain band, vanishing when the pin is released — as distinct from a **charge**, an intrinsic conserved defect created only by cut-and-bond (reserved with the bonded machinery). A second load lives one level down: even an unconstrained material fold strains its off-neutral fibers — the reference trace at height z_N carries an irreducible line density ℓ = −2κ(n_A·B̂), zero exactly for straight creases or vanishing dihedral — and any nonzero z_N across a curved fold therefore lives in a strained band — the connecting strip is provably non-developable. Midsurface and fiber accountings are different books: the material fold has λ ≡ 0 with ℓ ≠ 0; the bonded pillow, the reverse.

### 9.2 The mate is determined

A material-continuous fold has **no free functions beyond its crease**. Material identity — congruent flat traces, κ_g^A + κ_g^B ≡ 0 inward-signed — forces the mate's tangent plane to be the *reflection of the host's in the crease's osculating plane* (the classical curved-fold condition), in rational, normalization-free form:

```
B = x′ × x″,        n_B = n_A − 2 (n_A·B / B·B) · B,        h_B = n_B·x.
```

The trace-curvature identity (x″·n_A)² = (x″·n_B)² factors into exactly two branches: the osculating reflection (curvatures cancel — MONO) and the rectifying-plane mirror (curvatures add — the **pillow branch**, i.e. two separately cut blanks, reserved as the bonded fold with its mismatch field). So λ ≡ 0 for material folds is a *theorem of the construction*, certified structurally on the generated path and by a rational identity (MONO_CURVE) on imports — closing a real hole: the free-profile construction this replaces could pass every per-flank certificate while violating material identity; the instructive near-miss ("hold one flank flat, constant dihedral along a circular crease") does precisely that, generating a valid cone whose traces give 1/R versus cos θ/R. Where B ≡ 0 the crease is straight, the reflection constraint evaporates, and dihedral freedom returns — recovering the straight-crease interface as the B-degenerate stratum of one statement. Fold feasibility κ ≥ |κ_g| holds identically on the generated path (κ_g = κ cos α_F) and survives as an explicit check only on imports.

### 9.3 Authoring: flat-authored, freeze-gated

The crease is authored flat content — exact, clipping along it exact. Its chart lift is an anchor spline (transversality is the anchor's monotonicity condition; osculating regularity |x′×x″|² ≥ m is a Sturm atom on the anchor's data), and the whole fold — 3D crease, reflected mate, the MONO identity — is **exact-over-anchor**: the exact fold of a flat crease within ε of the authored one. 3D-side intent (mandrel-pinned creases, alignment features) is a *projection gesture*: project through the mark machinery, then **freeze** — snap to exact flat content with a provenance record {source, atlas revision, projection stamp, declared intent tolerance τ} — after which the curve is plain authored content and the DXF is exact; a FRESH check re-projects on demand and flags drift beyond τ, advisory by default, promotable into validity for fab-critical features. Prescribed dihedral on a material fold has two legal routes — solve the inverse crease problem (deferred), or a smooth band absorbing the load — and cannot be mis-authored as sharp: for a planar crease with a flat host the reflection degenerates to the 180° fold. Realizations: sharp — now honestly a **zero-thickness skeleton and authoring datum**: at finite thickness the flank slabs disagree by 2w sin(θ/2) per fiber, overlapping on one side and leaving a wedge void on the other, so a finite-thickness certified export requires each dihedral joint to declare a treatment: a smooth band, a deferred fit, or an explicit derived joint closure (a rational normal-fan patch — swept by a rational half-angle vector from the two flank normals, one-sided in w — with the bevel as a rational quadratic clip in the fan's own coordinates against the *oriented* bisector, plus exact, support-scoped plane-preimage trims on each actual flank (one fixed bisector plane, so the correspondence exists rationally — what the curved case lacks); the two flank cuts coincide only under an explicit miter-fit identity (paired through the crease line, since the flanks share no transverse parameter); absent it, the cap is built by an **exact planar symmetric-difference arrangement** in the bisector plane — common interior suppressed, exposed regions oriented, every vertex certified manifold from its cyclic link — a valid watertight B-rep with a labeled step: **only the symmetric-difference region is emitted as planar faces, one per connected component** (∂(S_A ∪ S_B) ∩ Π = F_A △ F_B always — the clean miter is the empty case), coincident cut regions are suppressed and their boundary side faces sewn **directly A-to-B**, a per-edge occupancy classifier assigning every exact edge its mode from the **transverse four-quadrant occupancy** — cap-to-flank, flank-to-flank, internal only where material fills the full transverse link (diagonal contact rejects as a pinch), or reject — with post-selection vertex-link certificates on both the planar cap and the sewn shell, and typed coverage counts making the emission complete, not merely correct (the 2D arrangement available when the *emitted* cap boundaries stay in the line/circular-arc class; conic-capped flanks take the miter identity or a band), not two coincident faces wearing an annotation: exact, free of authored data, strain-exempt by declaration, demoting the export to solid-closure semantics since the material correspondence is suspended on its support, and **supported for straight creases only** — a curved joint's synthetic collar has no structural attachment to the real flank, so curved joints take a band or fail the solid gate; the miter belongs to bonded joinery, since it welds distinct fibers). Sharp flat identifications additionally carry a derived bend allowance, split by role: the **material strip width** (≥ 0, band- or profile-sourced; on the gap side seeded by the frozen |z_N|·θ(t) arc, stamped by containment from certified dihedral enclosures) is the only field that may alter a material-semantics panel outline, while the **virtual-sharp setback** 2|z_N|·tan(θ/2) — field-exact at constant dihedral — is dimensioning data for flange tables and tooling, never blank geometry; an overlap-side joint without its band fails the material treatment gate rather than silently shortening the blank — closing a K-factor-shaped hole at the representation's own creases without reopening the miter's. Smooth (band + edge attachment, §10) and deferred (budgets + tie pairs, refit from FEA) complete the contract.

A clean exemplar: reflect part of a cylinder through a tilted plane — and this is not merely an example but the *planar-crease special case* of the general construction, since pointwise reflection in a varying osculating plane collapses to one global mirror exactly when the crease is planar. Both flanks bend; the flat trace is a sine with cancelling geodesic curvatures.

![Curved fold](fig5_fold.png)
*Figure 5. A material curved fold: reflecting a cylinder through a plane bends both flanks (left); the flat crease trace is a sine with cancelling geodesic curvatures (right) — the planar-crease case of the osculating-reflection construction.*

---

## 10. Controlled non-developability, relaxation, and the panel complex

### 10.1 Three frames

The **panel frame** (what gets cut; embeds by construction; content lives here), the **reference quotient** (panels modulo bonded identifications; carries charges; generally non-embeddable; derived analysis data only), and the **deformed state** (carries K and strain). Deriving strain and curvature from one map guarantees the exact **nonlinear** compatibility K = 𝒦(g₀ + 2E) — Brioschi applied to the actual metric, by construction; the linearization K ≈ −inc E is a small-strain estimator with an O(‖E‖²) remainder, never audit-grade. For an uncut forced sheet there are no identifications: the quotient equals the panel and all curvature is paid by strain. In the material-only version every component is one panel: a single embedded flat domain with interior marked fold curves.

### 10.2 The strained stratum and layer strain

C̃ = M + wT, M = X + u, with the director in tangential two-parameter form T = [(1 − |v|²)n + 2v]/(1 + |v|²), v·n = 0 — two parameters for two degrees of freedom, hence no gauge fiber (an earlier three-parameter Cayley field carried a redundant axial component and a hidden one-dimensional fiber, the exact disease the datum-frame round existed to prevent). T·T ≡ 1 is an identity, w is exactly along-fiber arclength, and the tilt is *represented* rather than bounded conservatively: tan(ϑ_T/2) = |v| exactly — ϑ_T the director tilt, a distinct symbol from the fold dihedral θ. The construction introduces one denominator, 1 + |v|², structurally positive and certificate-free. Layer parallelism, however, is an S0 property only; on bands the exact-surviving set is straight fibers, exact along-fiber separation, and fiber-wise correspondence — which, since seams are pinned S0 and bands are small, means the strong guarantee holds exactly where copper and thin-layer meshing live.

The strain that certificates bind is the **layer strain**, exactly quadratic in w:

```
E_w = ½(A_wᵀA_w − g₀) = E + w·B + w²·C,        A_w = DM + wDT,
```

three exact coefficient fields; every layer is an evaluation. On the developable stratum B = −II, C = ½III — classical bending strain, and the cone's copper reads its 3.6% *in-model* rather than by hand (the midsurface-only E, identically zero there, was blind to the device's dominant strain). Budgets reference the isometric fiber: with the calibrated z_N the physical layer strain is the shifted quadratic about ĝ = g₀ − 2z_N·II + z_N²·III, and the copper cap binds that field; the cap of record is the per-layer scaled eigenvalue cap with asymmetric tensile and compressive limits — the one-sided version bounds tension only and passes arbitrary compression, and the device's own inner copper at −3.6% is the counterexample — certified through two trace and two determinant conditions (degree four in w) behind cheap two-sided quadratic prefilters. Two corollaries of unit T: transverse shear γ = DMᵀT is w-independent (≡ 0 on S0 — Kirchhoff as an identity), and the transverse metric block is ≡ 1 — the kinematics are transverse-inextensible by construction, with a fitted-import stretch field as the only reserved entry point. Per-trace assessment is **directional**: ε_{γ,w} = γ̇ᵀE_w^phys γ̇ / γ̇ᵀĝ γ̇ — strain against the fabricated length² of the trace element; a g₀ denominator would re-crown the geometric midplane, off by ~2z_N·κ relatively, the very habit z_N exists to break — rational in (t, w); peak over trace × layer is a pointwise Bernstein object; stretch reports as Λ² = 1 + 2ε; RMS and accumulated measures are quadrature-class; plastic history belongs to FEA, fed by final-state fields and the roll schedule.

### 10.3 Attachment, audits, and the estimator

The wanted object at a curved joint is a strained band spreading the load. The normative constraint is **attachment**: band boundary rows meet the flank jets to declared order — structural-by-sharing on authored bands, certified-residual on FEA-fitted imports. Given incompatibly pinned flanks and satisfied attachment, the band's aggregate curvature is a *consequence*. The audit is **finite-band Gauss–Bonnet over the deformed metric**:

```
∬_B K dA_g + ∮_{∂B} κ_g^{(g)} ds_g + Σ θᵢ^{(g)} = 2πχ(B),
```

with flat exact turning substitutable per boundary piece *iff* the displacement's support is disjoint from that piece — a structural set-intersection check, not a judgment. Cells telescope (shared internal edges cancel as the same certified integral), so the audit is hierarchical and localizes its own failures; in the material-only version it certifies a known zero — interior-supported loads integrate away exactly, which is the charge/load distinction made quantitative; charges would not. The per-station integral survives as an *estimator* in relative form — |∫K dζ − λ_eff| ≤ C·Λ·(κ_g ε + ‖E‖ + ε/L_s), Λ = sup|λ_eff| + sup θ|κ_g| over an O(L_s) window — absolute values because the sources are signed, and the θ|κ_g| floor because a zero-net-curvature fold still forces sign-alternating pointwise K — feeding the width economics e ≈ λ_eff ε/4 and pre-FEA trace numbers, never validity; the area weight alone accounts for the ±25% figure at petal scale. Computationally, pointwise rational bounds run Bernstein on denominator-cleared forms (with the slab certificate doubling as the curvature field's denominator certificate), while integral functionals route through validated quadrature into function models — integrands carry √det g and antiderivatives leave the ring, so there is no cleared polynomial form to Bernstein.

Widths are physics-seeded and refined (ε* ≈ √(tθ/λ_eff)); vertex defects do not amortize (≈ δ/2π at any radius). The crease stays the **authoring datum**: the skeleton carries all invariants exactly, artwork and pairings live on its flat frame, and re-relaxation moves no content.

![Curvature carriers](fig6_carriers.png)
*Figure 6. Curvature carriers on the flat assembly — smooth field, line density, point defect — with the deformed-metric audit. Charges (BONDED, reserved) are conserved; the material-only version carries only loads, which integrate away exactly.*

### 10.4 The allocation problem

∬K of a doubly-curved target is fixed by Gauss and allocated between charges (cuts and bonds) and strain (plasticity against copper's ceiling). An uncut lens-scale dome wants 9–13% membrane strain — copper says no — forcing the budget into cuts; a shallow 2 mm window at ≈ 0.26% stays uncut. N gores on a hemispherical cap put λ ≈ 4/(NR) per join: N = 12, R = 8 mm, ε = 0.5 mm gives e ≈ 0.5% — the design rules follow from three numbers before any FEA. The reserved audit closes the loop with every term exact-or-certified: ∬K_def dA = Σ∫λ + Σδ + ∬𝒦(g₀ + 2E) dA — the *nonlinear* functional doing the strain-side accounting.

---

## 11. Certificates and the computational interface

Certificates are typed by **expression class × quantifier dimension**, computed from the statement's expression tree: **A** (algebraic over the coefficient field): identities and univariate inequalities decide *totally* (symbolic normal forms; Sturm), while multivariate strict-sign predicates are **margin-conditional** — Bernstein proves on positive margin, refutes on a witnessed sign change, and returns unresolved on the boundary, where a design has zero fabrication tolerance anyway (CAD exists as a forensics-only completeness backstop); **T** (algebraic composed with development-class atoms; interval/Chebyshev models; strict inequalities decide by refinement on open margins; identities to stamp only), with integration an operator into T. Fitted imports enter by canonical projection plus a ledgered residual; exact identity verification is reserved for field-exact data. Two laws: identity-shaped validity conditions must be class A or structural (T-identities are audit-only), and dimensions are recorded nominal → reduced (the slab certificate is A, 3D → 1D via the affine collapse). The design's measurable claim is maximizing the A × 1D cell.

| kind | class | statement |
|---|---|---|
| REG-Q / REG-RAT / REG-DOM | A | regularity; denominator atoms (structural for positive-weight forms); domain well-formedness |
| SLAB-S0 | A, 3D→1D | inf(R₁ + w) > 0 over dom × [min(w⁻, z_N), w⁺] — one-sided; +w safe by the structural coefficient; implies (strictly) the reference-metric condition ĝ ≻ 0 |
| SLAB-S1 | A, 3D→2D | inf det DC̃ > 0 over the full physical w-interval — neither side safe; w eliminated losslessly by the complete quadratic-positivity lemma (endpoints ∧ [concave ∨ negative discriminant ∨ vertex outside]); depends on the host's SLAB-S0 |
| SHEAR | A, 1D | κ_g ≡ k (signed witness) ∧ k² ≥ m > 0 ∧ Δ ≡ Δ₀; δ = −Δ₀/k |
| MONO_CURVE | structural / A | κ_g^A + κ_g^B ≡ 0 — osculating-reflection branch |
| EDGE | structural / T | band–flank jet attachment to declared order |
| ANCHOR | T,1D + A,1D | uniform lift bound; regularity; ε vs flat clearance |
| FRESH | T,1D | frozen-content drift ≤ τ (advisory; promotable) |
| EMB | A,2D + T,2D | flat embedding (exact on content; models on placements) |
| CLEAR | T + A discharges | pairwise/self clearance: **witnessed** discharges only, each a formal predicate record — tagged shell graphs about a witnessed axis *line*, applied per span (strict spatial span in a half-angle/winding calculus, radial injectivity from the slab certificate, one-sided ruling domain; non-adjacent spans by azimuth-interval disjointness), derivative-plane foliation (fibers lie in {x·n′ = h′} identically; within-plane injectivity structural; a strictly-decreasing level predicate over hull vertices — the branch is pinned, since the diagonal of that record is minus the slab Jacobian), and offset-pair reduction valid only on the constant-offset plateau — the ramp pair reverts to residual subdivision; intrinsic discharges prohibited (a two-wrap cylinder is isometric to a one-wrap cylinder with identical turning and slab data — no intrinsic rule can tell them apart); residual pairs by interval subdivision |
| SEP | A | corresponding-normal gap ≡ g ≥ g_min by the face identity (minimum face distance is CLEAR's; on the plateau the two coincide) |
| TILT / BUDGET | A, 2–3D | tan(ϑ_T/2) = \|v\| ≤ tan(ϑ_max/2), exact; E^phys per region × layer against asymmetric tensile/compressive eigenvalue caps — two-sided quadratic prefilters, four cap conditions (degree 4 in w) |
| GBAND / GBAUDIT | T (audit) | cellwise deformed Gauss–Bonnet residual ≤ stamps; telescoping |
| DEV | T,1D | quadrature remainders ≤ stamp |

**VALID** splits along the export semantics: a material-complement conjunction (regularity ∧ slab ∧ clearance ∧ separation ∧ interface certificates ∧ promoted freshness — evaluated over *clipped* domains where closure trims exist) ∧, per joint, the treatment's own certificate suite — a band's certificates for the material grade, the full closure suite per joint for solid-closure, the clean-miter branch (the miter identity with its materialized edge ledger) and the arrangement branch (input license, construction, per-run postcondition) forming an explicit per-joint disjunction — CLOSURE-CAP = MITER-BRANCH ∨ LEDGE-BRANCH, each closed by its own output postcondition and the bundled sewing certificate — a per-edge classifier over occupancy-dispatched, mode-indexed proof packets, and embedded spherical vertex links over the boundary vertex classes — each link checked as an isomorphism between the stored incidences and the geometric order; a failed disjunction invalidates the treatment, and re-treating is an authoring edit whose destination gate runs from scratch — a construction must pass at every stage, and neither a declared ledge nor a small one is an exact miter (the collapse operators were withdrawn as named targets without constructions); a *declared* treatment is no substitute for a valid one — class T overall, total on the A-identity/1D fragment. Exports carry a **two-field stamp**: semantics ∈ {*material* — the flat↔material correspondence valid and invertible everywhere, requiring a band or fitted physical transition at every dihedral joint, and the only grade forming analysis may consume; *solid-closure* — a watertight derived idealization whose sharp joints are filled by exact rational fan-and-bevel closures, with no material inverse on the closure supports (marks emitted there)} × status ∈ {*certified*, *embedding-unresolved* (labeled verbatim), *diagnostic*} — and lower grades say so rather than counting as validity. Ledger margins are **predicate slacks** with exact-or-certified gradients (barrier functions for optimizers); parameter-space trust radii require dividing by certified sensitivity bounds; raw slacks never aggregate across kinds. VALID certifies the as-designed configuration; reachability from flat is a forming-path property with a diagnostic sampled check. Queries follow the same split; inverse queries report chart-local coordinates, and the "which sheet over the reference cone" view exists only as an explicit projection query.

---

## 12. Downstream artifacts

All exports are stamped one-way and carry their class. The **atlas record** is the authority. The **flat drawing**: authored content only as fab geometry, marks on advisory layers, datums anchored to stored planar frames, fold lines with dihedral schedules, compensation applied; class-2 export is legal mid-iteration and labeled. The **BREP**: face surfaces exact; artwork-induced trims fitted once from anchor splines, shared, stamped; sidewalls exactly ruled with exact along-fiber thickness over anchors, placement certified; layer parallelism claimed on the developable stratum only; no kernel booleans; sub-certified simulation handoff is opt-in. The **mesh**: flat meshing with the exact size field; flat-vertex lifting through the inverse development is transcendental-certified — boundary vertices ride stored anchor splines, interior vertices amortize by marching, and a lifted mesh is a discrete anchor cover — while closest-point projection of *3D* points to chart coordinates is the algebraic-certified operation; prisms exactly straight with exact along-fiber thickness over their anchors. **Forming/tooling**: mandrel and thermode exact from tags; fold dihedral schedules; the roll family as a BC schedule with its diagnostic clearance check. Generative design parameterizes in flat coordinates or the band fields, evaluates through the certified forward map against ledger slacks, and re-enters the artwork only through the freeze gate.

---

## 13. Related work

Shell maps (Porumbescu et al. 2005) as the thickened-map precedent, here promoted to the part's definition. Plane-space duality classical (Blaschke; Pottmann & Wallner 2001); dual Bézier developables (Bodduluri & Ravani 1993); our axis is the sphere-normalized quaternion form keeping offsets in the ring, plus the allocation and certificate machinery. Folded developables: Duncan & Duncan (1982); the flat-trace relation and the osculating-plane reflection condition: Fuchs & Tabachnikov (1999); computational curved folding: Kilian et al. (2008) — §9.2 transplants the classical condition into the plane-space form, where the crease and mate come out exact. Curvature accounting is incompatible elasticity: non-Euclidean plates (Efrati, Sharon & Kupferman 2009), disclination buckling (Seung & Nelson 1988), stress focusing (Witten 2007). Discrete developable models (Rabinovich et al. 2018; Stein et al. 2018) as optimization substrates. Regularity dichotomy: Nash–Kuiper; Pakzad (2004). Exact geometric computation over constructible fields: the LEDA/CORE line of separation-bound arithmetic underlies the field lattice.

---

## 14. Limitations and open decisions

The representation does not predict physics: springback lives in the compensation slot plus calibration with a named-but-deferred rest-curvature slot; relaxed fold shapes are fitted from FEA. The correspondence is refinable, not exact; content-induced 3D geometry is certified in anchored backward-error form; frozen 3D-authored features are exact but can drift from their generating intent, which the freshness check detects rather than prevents. Exports below full validity exist and are labeled. Bonded joints, metric identifications, charges, and the allocation audit are reserved; chart-graph cycles, the inverse crease problem, inflection-crossing folds, and transverse stretch are deferred; the optimality of the constant-slope seam ansatz and of the tangent-chord bypass class is open, as is a witnessed discharge for the ramp pair (residual subdivision meanwhile); sharp-joint closures are exact, strain-exempt idealizations carrying solid-closure export semantics (no material inverse on their supports) and are supported for straight creases only — curved joints take bands — with forming handoff requiring the material grade at every dihedral joint. Strained-stratum certification is multivariate and heavier, and multivariate algebraic certificates are margin-conditional rather than total (boundary cases return unresolved, a verdict physics endorses); kernel and mesh boundaries remain floating-point, quarantined.

---

## 15. Conclusion

The system's leverage comes from five refusals: refusing to make geometry primary (the map is); refusing to make developability an axiom (an identity of a rational family, with validity a separate open ladder); refusing to let exactness be uniform (chart geometry and authored content exact over a constructible lattice; the correspondence certified with anchored, non-compounding backward error); refusing to let relaxation move the datum (loads distribute what the design committed to; the skeleton anchors the artwork); and refusing free parameters that physics does not grant (the fold mate is determined by reflection; the director is unit by construction; the seam offset is derived from the face identity). Under those choices the awkward parts become the well-behaved parts: the stackup is an exact offset family; the seam identification's closed form is a certificate with stated hypotheses, not an assumption; the transition ramp is a scalar control problem with a conservative closed-form solution inside a certified ansatz; curved joints are determined reflections whose zero net curvature is a theorem; and validity is an honest ladder — algebraic where the quantifiers allow, certified-refinable where they do not, witnessed where it is global, and never optimism serialized as JSON.

---

## Appendix A. Worked instances

**The 42° device.** One strip chart (3–4 G1 degree-1 spans), h ≡ 0, CONE(0); w ∈ [−120, +120] µm about the local midplane; sector 240.9°; the blank embeds to ≈ 1.49 wraps. Slab slack R₁ + w⁻ ≈ 3.24 mm. Copper layer strain E^phys(±120 µm) ≈ +3.6% tensile outboard and −3.6% compressive inboard of the calibrated fiber, in-model — both caps of the asymmetric budget bind. Seam: shared-q tail, degree-1 constant-slope ramp, Δφ ≈ 60° at f = 2 (surrogate-conservative; exact one-sided optimum a few percent narrower); face identity with uniform stacks gives Δ ≡ 0.25 mm, so **SHEAR holds** with the signed constant κ_g = −tan β (outward normal, increasing azimuth), and J = rigid ∘ shear with δ = −Δ/κ_g = Δ cot β ≈ +0.28 mm — derived, not assumed; SEP ≡ ACF gap by construction. **Clearance is an itemized ledger with one honest residual.** The wrap discharges per deg-1 span by the tagged-cone record about the apex line — a full wrap is 360° of *spatial* azimuth across three to four spans (the 240.9° figure is the *developed* sector, a different frame; an earlier ledger conflated them) — with strict per-span spans in the half-angle/winding calculus, radial injectivity from the slab certificate, one-sided ruling domains, and non-adjacent spans azimuth-disjoint. The pair discharges on the constant-offset plateau by the offset-pair record with the corresponding-normal gap. The ramp neighborhood — pair and self — is certified by interval subdivision over a thin Δφ ≈ 60° box, the one place subdivision runs, with the ramp-start tangency excluded as material continuation. Neutral-fiber sensitivity **2π cos β ≈ 46.7 µm of azimuth per 10 µm of z_N per wrap** (true fiber outboard ⇒ blank cut short ⇒ seam gap); the coupon is a *pure* z_N instrument on cone strata — the offset family is rigid there, so d_shape is blank-invisible — and three observables decouple onto three slots: azimuthal gap → z_N, radial ghost offset → Δ, mandrel metrology → d_shape. Single sheet ⇒ seam data calibration-invariant. Compensation identity pending coupons.

**Petal disk.** Planar hub with stored datum frame + petal. Straight root: crease, zero cost. Curved root pinned flat: load λ_eff = (1/R)(1 − cos θ), band amplitude e ≈ λ_eff ε/4 — R = 4 mm, θ = 45°, ε = 1 mm gives ≈ 1.8% (copper: no); θ = 15° gives ≈ 0.2%. Mis-authoring as a sharp fold is impossible (the reflection degenerates to the 180° fold); the legal routes are a straight root, a shallow dihedral (quadratic in θ), letting both flanks bend (crease shape becomes a design DOF), or budgeting the band with copper kept off it. Freezes on the hub are lossless (projection lands in the datum frame's ring).

## References

- W. Blaschke. *Vorlesungen über Differentialgeometrie*.
- M. R. Bodduluri, B. Ravani. "Design of developable surfaces using duality between plane and point geometries." *CAD*, 1993.
- J. P. Duncan, J. L. Duncan. "Folded developables." *Proc. R. Soc. A*, 1982.
- E. Efrati, E. Sharon, R. Kupferman. "Elastic theory of unconstrained non-Euclidean plates." *JMPS*, 2009.
- D. Fuchs, S. Tabachnikov. "More on paperfolding." *Amer. Math. Monthly*, 1999.
- M. Kilian, S. Flöry, Z. Chen, N. J. Mitra, A. Sheffer, H. Pottmann. "Curved folding." *SIGGRAPH*, 2008.
- J. Nash, 1954; N. Kuiper, 1955. C¹ isometric imbeddings.
- M. R. Pakzad. "On the Sobolev space of isometric immersions." *J. Diff. Geom.*, 2004.
- S. D. Porumbescu, B. Budge, L. Feng, K. I. Joy. "Shell maps." *SIGGRAPH*, 2005.
- H. Pottmann, J. Wallner. *Computational Line Geometry*. Springer, 2001.
- M. Rabinovich, T. Hoffmann, O. Sorkine-Hornung. "Discrete geodesic nets." *ACM TOG*, 2018.
- H. S. Seung, D. R. Nelson. "Defects in flexible membranes with crystalline order." *Phys. Rev. A*, 1988.
- O. Stein, E. Grinspun, K. Crane. "Developability of triangle meshes." *SIGGRAPH*, 2018.
- T. A. Witten. "Stress focusing in elastic sheets." *Rev. Mod. Phys.*, 2007.
