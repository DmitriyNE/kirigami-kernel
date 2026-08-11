# S2 seam-closure spike — report & go/no-go

*Status: **COMPLETE — decision: GO** (`vv-guide` "Stage 2 (BONDED lap seam) acceptance criteria · S2").
Prices the full-2π closure of the rolled cone: the transcendental frontier `DEV.3-β`, on the device
cone (`fixtures::devices::cone`, `q=(9,4,4σ,9σ)`, `n·ẑ≡65/97`). All numbers reproduced by tests on
branch `stage-2-seam`.*

## 0. What the spike prices

A single rational chart is a **gore, not a closed cone**: `σ ∈ ℝ ↔ φ₃D = 2·arctan σ ∈ (−π,π)`, so it
sweeps a bounded azimuth and misses exactly one ruling — the **lap seam** at `φ₃D = ±π`, i.e. `σ = ±∞`.
The full-2π closure (S2) has to name and certify that ruling before the BONDED lap (S3) can bond across
it. The spike GO/no-go's the closure on the device cone: can the seam be brought to a **finite,
well-conditioned** parameter, exactly and certified?

## 1. The problem: the seam sits at the chart's coordinate singularity

The seam is **representable** in one chart — it is just the compactification point `σ = ±∞`. It is not
**certifiable** there: any subdivision certificate (the S3 tool) needs finite interval widths, but at
`σ = ±∞` every cut rail's `µ̂ ∝ 1+σ²` is unbounded and no refinement (`--segments`/`subdiv`) converges.
**Representation ≠ certification-conditioning.** The seam must be moved off the singularity — without
changing the surface, and provably so.

## 2. Method: a re-centered rational chart + the general `SeamFrame` reduction

The axis **half-turn** `φ₃D → φ₃D + π` is the exact rational Möbius `σ = −1/σ'` (from
`arctan σ' = arctan σ + π/2`). Substituting into `q` and clearing the denominator (the
quaternion→rotation map is scale-invariant, `R(λq)=R(q)`) gives `q'(σ')=(9σ',4σ',−4,−9)` —
**still a degree-1 rational cone**, `n·ẑ≡65/97`, same development coefficient `c=130/97`. The seam
`σ=±∞` becomes the regular finite point `σ'=0`.

This is packaged as the chart-agnostic `develop::seam_frame` reduction: a `SeamFrame{view, transition,
seam_param}` and a checker `seam_frame_exact(base, frame)` that discharges the re-centering as an
**exact rational identity** — `view.normal ≡ base.normal ∘ transition`, pedal too (the ruling then
follows, rescaled by `transition′`). Float-free (`RatFunc` composition over the public ops; the
`lattice`/TCB core untouched); fail-closed (no `Unresolved` — a reparametrization's exactness is a
decidable identity). Method chosen **over** compactified-σ (∞-arithmetic in the interval core — more
invasive, no advantage) and an angle-coordinate chart (`σ=tan(τ/c)` reintroduces transcendentals into
the rails). "No single-chart shortcut": the seam genuinely needs the second view; it is a certified
*view*, not a second surface.

## 3. Results (all reproduced by tests)

- **Exact reparametrization** (`fixtures::…::cone_seam_is_the_device_cone_recentered_on_the_seam`,
  `develop::seam_frame::cone_seam_frame_is_an_exact_reparametrization`): `n_seam(−1/σ) ≡ n_cone(σ)`
  on all three components; `seam_frame_exact` `Verified`. Refutations fire (an identity transition; a
  different cone → `Refuted(NotAReparametrization)`).
- **The seam develops at finite σ'** (`develop::cone::the_seam_develops_at_the_finite_recentered_point`):
  the existing `ConeDevelopment` recognizes `cone_seam` (`cone_angle_coeff = 130/97`, unchanged) and
  develops the seam ruling — unreachable at `σ=±∞` — to the exact `(144/97, 0)`, backward error `< 1e-6`.
  No development-machinery change (the re-centered chart is a canonical apex cone, `γ≡0`).
- **Oracle ∧ audit** (`export::mesh3d::certified_seam_development_corroborates_develop_cone`): over a
  σ' grid anchored at the seam `σ'=0`, the certified re-centered development is corroborated by the
  independent float diagnostic `develop_cone` to `max_diag ≈ 1.5e-8`, the certified center matching the
  analytic `|µ|ρ·e(c·atan σ')` to `≈ 1.8e-12`, backward error `≈ 6e-12`. `ρ_seam(σ')=144/(97(1+σ'²))`,
  the same functional form as the canonical chart.

## 4. What S3 builds on this

The shared finite frame is the substrate for the **§14 BONDED** certificate. On it, the two lapping
edges are the two `σ'`-sides of `σ'=0`, and the ramp / bond are certified at finite σ' by the rational
3D machinery: **SEP** (≡ gap `g`, exact), **SLAB** (`R₁>0`, Sturm), **SHEAR** (`δ=−Δ₀/k`, exact), and
the one new adaptive piece **CLEAR** (interval subdivision of the 3D distance between the two rational
sheets over the Δφ≈60° ramp box). The γ≠0 seam ramp (`Chart::new(cone_q, ramp_h)`) is the **second**
`SeamFrame` instance that pins the reduction interface. None of this needs the flat directrix `γ` (the
certificate is 3D-rational); the transcendental `ψ` stays confined to flat-pattern emission.

## 5. Decision: **GO**

The seam is brought to a finite, regular parameter by an **exact, certified** rational reparametrization;
the existing certified development applies verbatim there; and the float oracle corroborates. The one
frontier the roadmap feared — the "seam-ramp subdivision" — is **rational, not transcendental**, and is
scoped to S3's CLEAR. No wall. **GO** to S3 (§14 BONDED).
