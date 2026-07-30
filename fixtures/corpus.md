# Counterexample corpus — the day-one regression suite

Every entry is a stored refutation from the spec review (v0.1–v0.24). Each is a test fixture: the geometry is the input, the **verdict** is the assertion. A fixture that does not reproduce its verdict is a kernel bug. Transcribe these into `fixtures/corpus/` (one module per entry, id as the test name). Format per entry: **id · module under test · geometry (exact) · certificate exercised · required verdict · spec origin**.

Coordinates are exact. "Reject" = the construction refuses and routes to band/refine; "Refuted(w)" = a checker returns a refutation witness; "Verified" = passes cleanly; "Unresolved" = the honest three-valued middle.

---

## Arrangement / boolean kernel (M3)

**cx-antipodal-arcs** · `arrange2d::coincidence` · unit circle, arc A = θ∈[0, π/4], arc B = θ∈[π, 5π/4]; identical circle carriers. · CARRIER-COINCIDENT then Stage-2 1D overlap. · **Verdict: disjoint — zero merged edges emitted.** (Carrier identity ≠ edge overlap; the two-stage coincidence discipline.) · v0.16 16.3.

**cx-coincident-vs-tangent-circles** · `arrange2d::classify` · two identical circles (d=0, r₁=r₂). · stratified classifier, most-degenerate-first. · **Verdict: classified COINCIDENT, not internal-tangent** (d²=(r₁−r₂)² also holds — strata overlap; COINCIDENT wins by priority). · v0.20 17.3.

**cx-tangent-outside-arc** · `arrange2d::membership` · unit-circle arc θ∈[−π/4, π/4]; tangent line x=−1 (carriers tangent at θ=π). · interval-membership gate before classification. · **Verdict: no vertex emitted** (tangency point outside the arc interval — phantom vertex otherwise). · v0.20 17.3.

**cx-parallel-distinct-lines** · `arrange2d::line_predicates` · L_A=(1,0,0), L_B=(1,0,−1) (direction pair proportional, triples not). · PARALLEL vs COINCIDENT. · **Verdict: PARALLEL ∧ ¬COINCIDENT — no intersection, no event.** · v0.21 18.7.

**cx-overlapping-disks-union** · `arrange2d::emit` · two disks A, B overlapping; operation ∪. Selected cells {A∖B, A∩B, B∖A}, mutual arcs selected|selected. · quotient emission (faces = π₀). · **Verdict: exactly one emitted face** (three cells, one component; NOT three faces). · v0.22 19.4.

**cx-internal-tangency-triple** · `arrange2d::cap_out_link` · B⊂A, ∂A tangent ∂B at one point; run ∪, ∩, △ on the same arrangement. · CAP-OUT-LINK (post-selection). · **Verdicts: ∪→valid disk; ∩→valid disk; △→Refuted (pinched at the tangency vertex).** (Same substrate, three verdicts — the check must be post-selection.) · v0.18 15-era / v0.21 18.5.

**cx-crosswise-link** · `sew::sew_link` (planar analogue in `cap_out_link`) · four incident rays cyclic order a,b,c,d; emitted records connect a→c→b→d. · `Link_emitted ≅ Link_geometric`. · **Verdict: Refuted** (abstract 4-cycle exists but the isomorphism to the geometric order fails). · v0.24 21.4.

**cx-full-circle-edge** · `arrange2d::decompose` · one input circle inserted without decomposition. · canonical x-monotone decomposition (pending-v0.25). · **Verdict: rejected/decomposed — no single DCEL half-edge spans a whole circle** (closed ≠ embedded interval). · pending-v0.25.

---

## Domain / clip ladder (M2)

**cx-sigma-mu-crossing** · `certify1d::clip_sigma` · G(σ,μ,w)=σ·μ at σ*=0 (a=b=d=0, ∂_σG=μ). · CLIP-σ signed disjunction [min_corners ∂_σG ≥ m] ∨ [max_corners ≤ −m]. · **Verdict: Unresolved/reject — the affine range [−1,1] contains 0** (a four-corner |·| test would falsely Verify with margin 1). · v0.19 16.1. *Soundness-critical: the falsely-certifying class.*

**cx-diag-sylvester** · `certify1d::sigma_min` · JᵀJ − mI = diag(1,0,−1), m=2. · strict-Sylvester (three leading minors strictly positive). · **Verdict: Refuted** (leading minors {1,0,0} nonneg but eigenvalue −1; strict test fails, σ²_min=1<2). · v0.16 15.4. *Soundness-critical.*

**cx-cone-flank-trim-mu** · `certify1d::trim_local` · cone-type flank, trim support where a ruling fans; check G_i at outer fiber over the μ-range. · TRIM-LOCAL four-corner (spline μ-bounds). · **Verdict: catches re-entry** — a w-only quantification passes, the four-corner test on the fiber Refutes at some μ. · v0.18 14.3.

**cx-clip-common-zero** · `certify1d::clip_ladder` · a fiber where b(σ*)=d(σ*)=0 with a(σ*)≠0. · ladder terminality (CLIP-a branch). · **Verdict: Verified via CLIP-a** (|a| separated ⇒ fiber misses Π; the ladder must not loop forever on subdivision). · v0.18 15.4.

---

## Closure geometry (M4)

**cx-zero-dihedral** · `closure::reg_v` · straight crease with V=0 (n_A=n_B). · REG-V atom. · **Verdict: record deleted** (zero-dihedral = G1 line; the Q-clip factor vanishes). · v0.15 12.1.

**cx-Q-not-H** · `closure::flank_clip` · symmetric planar flanks at half-angle α, s_J=+1. · the retracted "{H_i=0}=image{Q=0}" claim. · **Verdict: the two sets differ** — gap-side {H_A=0} is the apex alone; overlap-side cut at radius |w|/cos α on the opposite side of the crease. (Regression guard: the clip must be H_i, not Q.) · v0.16 13.1.

**cx-sJ-negative-orientation** · `closure::orientation` · any joint with s_J=−1. · oriented bisector b_J = s_J(n_A−n_B); N_i^cut=−b_i/|b_i|. · **Verdict: retained side and cap normal correct on both s_J** (an unsigned Q or a CLIP-W-sign normal flips here). · v0.15 12.1 / v0.24 20.x.

**cx-tilted-circle-ellipse** · `closure::cap_in_d24` · planar flank, circular domain x²+y²=R², oblique cap plane w=ax (a≠0). · CAP-IN-D24 carrier test on the μ̂±-sidewall image. · **Verdict: Refuted — the sidewall∩Π image is a noncircular ellipse** u²/(R²(1+a²)) + v²/R² = 1 (planar flank ⇏ D24 caps; routes to band/miter). · v0.19 16.2.

**cx-quadratic-extents** · `closure::miter_fit` · F_A∩L_σ=[0,1], F_B∩L_σ=[0,1+σ(1−σ)]; identical carriers, ranges, endpoint events. · per-cell branch identities E_{A,±}=E_{B,π(±)} on R. · **Verdict: Refuted** (event-sample agreement passes; the branch identity fails on the cell interior). · v0.18 15.3.

---

## Sewing / MITER-OUT (M5)

**cx-two-boxes-miter** · `sew::sewing_classifier` · S_A=[0,1]×[−1,1]×[0,1], S_B=[−1,0]×[−1,1]×[0,1]; common cut {0}×[−1,1]×[0,1]. · clean-miter cap suppression (∂(S_A∪S_B)∩Π = F_A△F_B = ∅). · **Verdict: no cap face emitted** (retaining it makes an internal partition; every ∂F point a three-face edge). · v0.20 17.1.

**cx-diagonal-pinch** · `sew::quadrant` · shared edge {y=z=0}; A occupies z≥0, B occupies z≤0; footprint bits (1,0) for y>0, (0,1) for y<0 (both planar neighbors XOR-selected). · four-quadrant occupancy (opposite quadrants). · **Verdict: Refuted (pinch)** — the transverse link is two arcs; a "two selected cells ⇒ internal" rule would wrongly suppress. · v0.21 18.4.

**cx-residual-arc-occupancy** · `sew::edge_occupancy` · a ∂F_A residual arc with no coincident ∂F_B; test (B_L,B_R)=(0,0) vs (1,1). · EDGE-OCCUPANCY four-bit packet. · **Verdict: the two cases are distinguished** — one occupied quadrant vs three, different cap-to-flank records (a two-sign packet aliases them). · v0.24 21.1.

**cx-nodal-cubic** · `sew::edge_emb` · e(t)=(t²−1, t(t²−1)), t∈[−2,2]; e′=(2t,3t²−1) never both zero; e(−1)=e(1)=(0,0). · EDGE-EMB (self-intersection). · **Verdict: node found at interior parameters, entered as a vertex, link-classified → reject** (EDGE-REG passes — regularity ≠ embeddedness). · v0.24 21.5.

**cx-order-sign-cubic** · `sew::epsilon_phi` · correspondence σ_B=σ_A³ (monotone, positive endpoint order, dσ_B/dσ_A=0 at 0). · ε_φ = order sign of the monotone correspondence. · **Verdict: ε_φ=+1 via endpoint order** (a derivative-sign definition is 0/undefined at the origin — the definition must be order, not derivative). · v0.24 21.6.

**cx-cusp-edge** · `sew::edge_reg` · a smooth flank whose Π-section is y²=x³. · EDGE-REG verdict. · **Verdict: fail (geometric cusp) → vertex + reject to band** (not a parametrization stall; distinguish the two). · v0.24 20.3.

**cx-stall-reparam** · `sew::edge_reg` + `certify1d::reparam` · a parametrization with an isolated derivative zero but regular point set. · EDGE-REG {stall→pending}, then REPARAM. · **Verdict: original → Pending (gate-fails); REPARAM'd record → Verified** (stall is a compiler-pass fix, not a predicate truth). · v0.24 21.6.

**cx-rank1-face-germ** · `sew::face_germ` · S(u,v)=(u,v²,uv); boundary curves S(t,±t)=(t,t²,±t²) regular; S_v(0,0)=0. · FACE-GERM branch index. · **Verdict: reject** (regular edges, rank-1 face germ; no FACE-GERM constructor inhabits — edge regularity licenses rays, not sectors). · v0.24 21.7.

**cx-gauge-jet** · `sew::invariant_jet` · one curve, two parametrizations differing by a nonlinear reparam; coincident-ray tie-break. · invariant-jet tie-breaker (cleared curvature numerator). · **Verdict: identical sort under both parametrizations** (raw e″ differs by the tangential φ″ term; the projected/normalized form is invariant). · v0.24 21.7. *Metamorphic.*

**cx-stale-sewing-record** · `sew::coverage` · a valid shell plus one sewing record on a fully deleted (empty-occupancy) edge. · reverse inventory equality {records} = {cap-to-flank} ⊔ {flank-to-flank}. · **Verdict: Refuted** (the stale record matches no forward count; empty ⇒ zero records asserted). · v0.24 20.5.

**cx-split-vertex** · `sew::vertex_bijection` · one exact geometric point emitted as two vertex records with the incident edges partitioned. · V_∂ ↔ emitted vertices bijection. · **Verdict: Refuted** (two records, one equivalence class — each locally cyclic, jointly a geometric pinch). · v0.24 20.6.

---

## Collapse / no-op guards (M4/M5)

**cx-nested-disks-hausdorff** · `closure::(withdrawn collapse)` · F_A={x²+y²≤1}, F_B={x²+y²≤(1+ε)²}; boundaries have no intersection events, d_H=ε. · guard: event enclosures cannot bound Hausdorff error. · **Verdict: the collapse operator is absent/withdrawn** — assert no code path stamps a Hausdorff bound from event boxes. · v0.19 16.5.

---

## Notes for the transcriber

- Entries marked *soundness-critical* get Kani and/or Lean coverage of their checker, not just the fixture.
- Entries marked *metamorphic* also become property tests (the invariance is the property).
- Where a verdict is "reject to band," the test asserts the routing, not a blessed output — v1 correctly refuses these.
- The two device instances (cone β=42° ID 5mm 1.49-wrap; petal with conical flank) live in `fixtures/devices/` and are the substrate many of the above are instantiated on. The petal's conical flank is the general-case adversary — it convicted the cylinder-picture assumptions three separate times; keep it in every closure test matrix.
