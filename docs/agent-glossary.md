# Agent glossary — terms you need before reading the spec

Operational definitions, not full statements (the spec §8 has those). Grouped by layer. Read once; refer back.

## The device (the thing being modeled)

- **Flex substrate / flex board**: a thin multilayer flexible PCB. Here, 4 layers, 240 µm thick, rolled into a **cone** (half-angle β=42°, inner Ø 5 mm, wrapping ~1.49 turns, 1.6 mm lap seam). The **petal** is a second co-normative test shape with a genuinely conical flank — used because the cone alone is too structurally simple to catch many bugs.
- **Developable surface**: a surface that unrolls to the plane without distortion (zero Gaussian curvature) — cones, cylinders, planes, and tangent-developables. The whole representation is about developables and their offsets (the board has thickness).
- **Chart**: one developable patch in the atlas, parametrized in (σ, μ, w): σ along a directrix, μ along the ruling, w through the thickness. `C(σ,μ,w)` is the 3D point.

## Lattice / arithmetic (crate `lattice`)

- **L0 / L1 / L2 / L3**: the coefficient-precision tiers. L0 = quantum rationals (plain ℚ on bounded limbs, the fast path). L2 = degree-2 algebraic numbers (√-extensions — where circle intersections live). L3 = higher algebraic (deferred; v1 rejects things that would need it). "Fast path / slow path" = L0 fixed-limb vs BigInt.
- **Exact / certified path**: any computation whose result feeds a predicate or certificate. Must be exact (rational/algebraic). Floats are forbidden here.
- **Interval-plus-separation**: how you compare two algebraic numbers exactly — bound each by a rational interval, refine until disjoint, or invoke a separation bound proving they differ. Returns a sign, never a float.
- **Sturm sequence**: the exact tool for counting/isolating real roots of a polynomial in an interval. Load-bearing everywhere.
- **Resultant**: eliminates a variable between two polynomials; its vanishing ⇔ a common root. Used for MITER-FIT pairing and EDGE-EMB self-intersection.
- **MarginSq**: the squared-margin newtype. Because clearing `|x| ≥ m` to a polynomial gives `x² ≥ m²`, margins on √-carrying quantities are stored squared. Comparing against the wrong power is a bug the review committed and caught.

## Verdicts / certificates (the soundness spine)

- **Verdict**: `Verified(Evidence) | Refuted(Witness) | Unresolved(Margin)`. Never a bare bool for a geometric decision.
- **Certificate**: a stored proof object. A "certifying algorithm" returns `(claim, certificate)`; a separate **checker** verifies the claim from the certificate. The searcher is untrusted.
- **Runtime-checked hypothesis**: when a checker relies on a deep theorem (Sturm, resultant, Sylvester), verify the theorem's *hypotheses* exactly at runtime on the instance and cite the theorem — shrinks what must be formally proven.
- **A / T class**: spec shorthand. **A** = decidable by exact algebraic identity (the "A-fragment," totally decidable). **T** = general class-T (needs the full machinery, may return Unresolved). When the spec says "an A-identity," it means an exactly-checkable equality.

## Charts & fields (crate `geom`)

- **Directrix / ruling / offset**: σ runs along the directrix curve, μ along the straight ruling, w through thickness. Rulings are the straight lines that make a surface developable.
- **Hatted calculus (p̂, μ̂, r̂, n̂′, Ĵ)**: the reparametrized ("hatted") coordinates that stay regular at **stall ends** (where the naive parametrization degenerates). Key identity: `J_raw = p̂·Ĵ` (positive factor). If you see a "/p" somewhere expecting "/p̂", that's the fossil bug — it must be p̂.
- **Stall / stall-end / flat generator**: a ruling where the surface momentarily flattens (shape operator drops rank). Handled by the hatted frame; a **flat generator** is the rank-0 case (κ₁=κ₂=0).
- **Substitution / removability**: machinery for canceling apparent singularities in the rational fields (a factor that looks singular but removes). Transported with orientation bookkeeping.

## Closure (crate `closure`) — joining two flanks at a crease

- **Flank**: one of the two developable faces meeting at a fold/crease. A **joint** joins flank A and flank B.
- **Crease / dihedral / V**: the fold line; the dihedral turn is encoded by vector **V** (|V| relates to the fold angle). **Zero-dihedral** (V=0) means no real fold — the joint record is deleted.
- **REG-V**: the certificate `|V|² ≥ m > 0` — licenses the clip's clearing (the clip divides by a factor that vanishes at V=0). Straight-crease v1 population: V constant, so this is one comparison.
- **b_J / b_i / G_i / N_i^cut**: the oriented bisector normal `b_J = s_J(n_A−n_B)`; per-flank inward normals `b_A=b_J, b_B=−b_J`; retained-side field `G_i=(C_i−x₀)·b_i` (keep side `G_i≥0`); the cut face's outward normal `N_i^cut = −b_i/|b_i|` — **the sole orientation authority** (not any parametrization sign). s_J is the ±1 "which side" bit.
- **Q-clip**: the fan bevel clip `Q(t,s)=1−2s−|V|²s²` on the oriented bisector. **H_i vs G_i**: H_i is the raw trim field (diagnostic only); G_i is the retained-side field (used in predicates). Never predicate on H_i.
- **WEDGE / EXT-WEDGE / COLLAR / D_collar**: the fan-sector certificates (sub-π sweep; the extended "collar" is a quotient wedge with its w=0 fiber collapsed; D_collar is the uniform reach scalar for padding).
- **CLIP-DOM / CLIP-W / CLIP-μ / CLIP-a / CLIP-σ / TRIM-LOCAL**: the clipped-domain certificate and its ladder. The domain `D∩{G_i≥0}` is non-product; per-σ fibers are convex polygons typed by four corner signs. The ladder handles transversality (CLIP-W when `n_i·b_J≠0`, CLIP-μ when `r_i·b_J≠0`, CLIP-a/CLIP-σ at common zeros; CLIP-σ is a **signed disjunction**, not a |·| test — critical). TRIM-LOCAL keeps the clip support-scoped (checked at the four corners of each outer support fiber).
- **CAP-IN-D24**: the *input* license for the ledge arrangement — every source cap boundary is a line or circular arc. (A planar flank against an oblique plane can still give an ellipse — so this is checked on actual boundaries, not surface tags.)

## Arrangement / boolean kernel (crate `arrange2d`) — the beast

- **DCEL**: doubly-connected edge list, the half-edge data structure for planar subdivisions.
- **D24**: the v1 curve class — **lines + circular arcs** (degree-≤2 intersections stay in-lattice). Everything the kernel handles is D24. Conics need L3 (deferred).
- **Canonical decomposition** (pending-v0.25): before insertion, split every circle/arc into simple x-monotone pieces at exact extremal points; no half-edge spans a whole circle or crosses the tag pole.
- **CARRIER-COINCIDENT / PARALLEL / COINCIDENT**: `COINCIDENT` (lines) = all 2×2 minors of the homogeneous triples vanish (same line). `PARALLEL` = direction cross zero (may be distinct). Do not confuse "proportional pair" with "proportional triple." Coincidence is decided in **two stages**: same carrier, *then* a 1D interval-overlap on that carrier (identical carriers can still be disjoint arcs).
- **Winding / half-angle tag**: arcs are stored as half-angle-tag intervals with a winding sign; overlap of arc intervals is winding-aware (an arc can wrap the pole). This is why "which arc of the circle" is a real question.
- **Event spine**: the classifier order — most-degenerate-first (coincidence → carrier intersection → **interval membership before classification** → transverse/tangent). Membership-before-classification prevents phantom vertices (a tangency point outside both arcs).
- **Eight-step boolean / quotient emission**: build DCEL → seed (0,0) → operand sidedness bits → propagate + **ℤ₂² cocycle check** → coincident-edge incidence vectors → pluggable select (⊕/∧/∨) → emit only separating edges → **faces = π₀** (connected components of selected cells, *not* one-face-per-cell).
- **CAP-OUT / CAP-OUT-LINK**: the *output* postcondition (cycles close, intervals exact, completeness bijections, no duplicates) and the post-selection vertex-link check (selected sectors form one interval/circle/none; this also computes V_∂ membership).
- **V_cand / V_∂**: candidate vertex classes (all exact event points, quotiented by identity) vs boundary vertices (those actually incident to surviving shell geometry). The bijection `V_∂ ↔ emitted vertices` is the completeness statement; link certificates quantify over V_∂. Don't emit interior candidates.
- **Link_emitted ≅ Link_geometric**: the manifold check compares the *stored* incidence walk against the *geometrically reconstructed* cyclic order — an isomorphism, not just "both are cycles" (an abstract cycle can encode a crossing).

## Sewing (crate `sew`) — stitching caps to the 3D shell

- **Cap / ledge / clean miter**: `∂(S_A∪S_B)∩Π = F_A△F_B` always. Clean miter = the empty case (no cap). Ledge = the nonempty symmetric-difference cells. Coincident regions are suppressed; their side faces sew directly A-to-B.
- **EDGE-OCCUPANCY**: the four adjacent-cell occupancy bits `(A_L,A_R,B_L,B_R)` + a frame bit, per edge. **Not** two "interior side" signs (those alias one-vs-three occupied quadrants). Two constructors: ARRANGEMENT-BITS (from cell labels) and MITER-REGION-IDENTITY (from the miter branch, no arrangement).
- **Sewing classifier / four quadrants**: each edge's transverse occupancy `(A_R,A_L,B_L,B_R)` must form one cyclic interval / all four / none. Opposite quadrants = a **pinch**, reject. Rows: cap-to-flank / flank-to-flank / internal-suppressed / pinch.
- **PAIR-IDENTICAL / OUTPUT-SOURCE-IDENTICAL**: mode-indexed edge-identity proofs. PAIR-IDENTICAL (two boundaries coincide — flank-to-flank) has two constructors (D24-STAGE2-EQUALITY, MITER-BRANCH-IDENTITY). OUTPUT-SOURCE-IDENTICAL (an emitted cap subedge = a subedge of one source flank boundary — cap-to-flank) has constructor ARRANGEMENT-PROVENANCE. Which one is demanded is dispatched by the occupancy packet.
- **ε_φ**: the orientation bit of the cross-flank correspondence — defined as the **order sign** of the monotone pairing (NOT the derivative sign, which can be 0 for a monotone map like σ³).
- **MITER-EDGE-LEDGER / MITER-OUT**: the clean-miter branch's edge inventory (materializes MITER-FIT's identities) and its output postcondition (EDGE-REG + **EDGE-EMB** [injectivity — regularity ≠ embeddedness; a regular curve can self-cross] + EDGE-EDGE [pairwise non-crossing] + CYCLE + coverage + vertex isolation).
- **EDGE-REG / REPARAM**: `|e′|²≥m` on the open interval, with verdicts {pass | fail (geometric cusp → vertex → reject) | stall (isolated derivative zero, regular point set → **Pending**)}. **REPARAM** is a compiler pass that regenerates a stalled edge as a canonical regular record; it is NOT a truth-predicate ("reparametrize and recertify" inside a predicate was a bug).
- **SEW-LINK / FACE-GERM / invariant jet**: the sewn-shell vertex check — an *embedded spherical* link (not abstract cycle), quantified over V_∂, compared to the stored records. **FACE-GERM** licenses the 2D tangent sector per face species (cap/flank/fan/apex); edge regularity alone does NOT (a surface can be rank-1 where its boundary curves are regular). Coincident-ray ties break on an **invariant jet** (normalized curvature — raw second derivatives are gauge-dependent and certify nothing).
- **SEW = SEW-EDGES ∧ SEW-LINK**: the bundled sewing certificate. Defined once (in §8.5); the gate cites it.

## Gate / validity (crate `gate`)

- **CLOSURE-CAP(j)**: `MITER-BRANCH(j) ∨ LEDGE-BRANCH(j)` — the per-joint cap disjunction. Each branch = its constructions ∧ its OUT-postcondition ∧ SEW. **No "band or fail" disjunct** — a failed CLOSURE-CAP means the treatment is invalid; re-treating is an authoring edit whose destination gate runs fresh. Gate formulas contain only truth-valued certificate expressions (no imperatives).
- **CLOSURE_VALID / VALID_material / VALID_solid-closure**: the joint-level and atlas-level gates. VALID_complement is evaluated over the *clipped* domains. Treatments: SMOOTH (band), DEFERRED (fitted band + import obligations), CLOSURE (the full closure suite).
- **FRESH**: the freshness/regeneration doctrine — derived state (fitted bands, sewing records) must not survive its inputs; stale = fails. The sewing reverse-inventory equality is FRESH at shell granularity.

## §14 backlog (do NOT implement in v1 — reject-to-band where they'd apply)

Curved-crease closures; the collapse operators (LEDGE-COLLAPSE/EDGE-COLLAPSE — withdrawn, obligations documented); the conic planar-arrangement class / D24 envelopes; singular plane-preimage + curve-cusp + osculation higher-order events; the COMPSOLID two-sided-interface export contract.

## Process terms (from the review, now CI rules)

- **`:=` census**: every defined name defines exactly once per commit (composites in the gate section, certificates in §8.5, geometry in its home section). A same-commit twin definition is a lint failure.
- **Tuple-predicate rule**: predicates on multi-component objects name the tuple (the displayed minor/cross form is the predicate; the adjective "proportional" is ambiguous and banned).
- **Pattern ledger**: the accumulated list of ~114 named failure patterns in the spec deltas — the code-review checklist. (You don't need to know them all; the delta files carry them, and each corpus fixture embodies one.)
