# Spike report — the γ≠0 flat-directrix integrator (DD.2 / DEV.3 "method (b)")

*Authored 2026-08-11 · branch `driving-demo` · decision: **GO***

## The frontier

The Driving Demo's **ramp flap** (`fixtures::devices::cone_seam_ramp`) is a *curved-support*
developable: it shares the device cone's Gauss circle (same `q`) but its scalar support ramps
`h(σ′) = 1/4 − σ′/2`, so its pedal `c(σ) ≠ 0` — a **γ ≠ 0** chart. Its angle law stays closed-form
(`ψ = c·arctan σ`, `c = 130/97`; `ψ` is `h`-independent), but its **flat pattern gains a directrix**

```
γ(σ) = ∫₀^σ [ a·e(ψ) + b·e⊥(ψ) ] ds ,   a = (c′·r)/ρ ,  b = −(c′·n′)/ρ ,
       e(ψ) = (cos ψ, sin ψ) ,  e⊥(ψ) = (−sin ψ, cos ψ)
```

(spec §Tier C — the development maps the *positively oriented* tangent pair `(r/ρ, −n′/ρ)` to the
flat frame `(e, e⊥)`). The integrand is `rational × {cos, sin}(c·arctan σ)` — **non-elementary**.
`develop::cone` previously hard-coded the pure-radial polar map `D = µ̂·ρ·e(ψ)` (γ ≡ 0) and rejected
`h ≠ 0` at the `cone_angle_coeff` pedal gate; `develop::interval` had no quadrature. This spike prices
the validated-quadrature route (DEV.3 "method (b)") before building the seam device on it.

## The method

- **`develop::interval::integrate_on`** — a verified interval Riemann sum: each panel `[s_i, s_{i+1}]`
  contributes `f([s_i, s_{i+1}]) · width`, sound because `f(s) ∈ f([s_i, s_{i+1}])` for every `s` in
  the panel, so the sum **contains** the true integral. The enclosure *width* is the certified
  quadrature error; it shrinks `∝ 1/panels`. `f` is any interval-valued integrand (here the directrix
  velocity, built from `eval_ratfunc_on` + `sqrt_on` + `cos_on`/`sin_on`).
- **`ConeDevelopment` generalized in place** — an optional `Directrix { c′·r, c′·n′ }`; `point`/
  `point_on` add the `+γ(σ)` term with **signed** µ̂ (the directrix breaks the apex symmetry). The
  `γ ≡ 0` branch is a **byte-identical fast path** (`new_developable` on the apex cone reproduces
  `new` exactly across the gore — a unit test). The pedal gate is lifted via `arctan_coeff` (the
  pedal-free half of `cone_angle_coeff`); `unroll`/`anchor` ride unchanged (the chokepoint is
  `dev.point`).

## Results (device seam ramp `cone_seam_ramp`)

| Check | Result | Meaning |
|---|---|---|
| **Local isometry** `max\|D_σ\|² − \|X_σ\|²` | **7.1 × 10⁻¹⁵** | machine-exact — the §Tier C **frame/sign is correct** (a wrong flat-frame sign gives the non-isometric defect `4bℓψ′ ~ O(0.1)`, which this would have caught) |
| **γ enclosure** ε at 64 panels | 1.45 × 10⁻³ | converging… |
| **γ enclosure** ε at 1024 panels | **9.0 × 10⁻⁵** | ~linear convergence (16× panels → ~16× tighter); **fab-plausible** (sub-micron on the mm-scale part) and far under the demo DRC (clearance = 1 ⇒ ε < 0.5) |

The isometry check is the load-bearing corroboration: it is computed from the directrix **velocity**
`γ′` (no quadrature error), against the 3-D surface's *own* first fundamental form `|X_σ|²`, so it
independently validates the integrand's frame and sign — exactly the thing the paper flags an earlier
draft got wrong. That it lands at `7e-15` (float epsilon) is the decisive GO signal: the formula is
right, and the only remaining error is the refinable quadrature ε, which converges.

## Decision: GO

The γ≠0 development is certified: a converging, `Verdict`-shaped enclosure with a machine-exact
isometry and a fab-plausible ε on the ramp flap. DD.3 (the γ≠0 fold — the coupled 2-D inversion
`flat = γ(σ) + µ̂·ρ·e(ψ)`) and DD.4 (the seam device) build on it.

## Scope / deferred

- **Convergence is linear** (the naive interval Riemann sum). It clears the fab tolerance comfortably
  at 1024 panels; a higher-order rule (midpoint + curvature-bounded remainder, or adaptive panels)
  would tighten it if ever needed — logged, not built (mirrors the CLEAR-subdivision tech-debt note).
- **`point_on` over an interval σ** uses the sound hull `γ(σ_lo) + γ′([σ_lo,σ_hi])·[0,width]` and
  re-integrates from 0 per call — correct, not optimized.
- **Two-sided σ** (σ < 0) is out of scope here (the ramp flap is one-sided `σ′ ∈ [0, 1/2]`); the
  integrator requires `σ ≥ lo`.
