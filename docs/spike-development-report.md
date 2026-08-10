# DEV.1 development spike — report & go/no-go

*Status: **COMPLETE — decision: GO** (`vv-guide` "Milestone E (DEV) · DEV.1"). Prices the certified
flat↔3D **development** tier on the device cone: isolates the transcendental core, selects the enclosure
method, ships a rigorous rational enclosure corroborated by the float diagnostic, and records the one
engineering wall (digit growth) with its known remedy. The certified backward error on the device cone is
`≈ 1e-11` (sub-nanometre on a millimetre part) and the certified flat point agrees with the independent
`export::mesh3d::develop_cone` oracle to `≈ 1.5e-8`. All numbers are reproduced by
`export … certified_flat_point_corroborates_develop_cone` and the `develop` crate's own tests.*

## 0. What the spike prices

DEV is **half the product** (`implementation-plan-v1.md §6`): the certified isometry between a 3D
developable and its flat pattern — product direction ① unrolls with it, ② folds with it. It is a genuinely
new tier (rigorous transcendental enclosure), so — like the §7 extraction spike — it opens with a scoped
spike that GO/no-go's the whole tier on one representative instance: the **device cone** (`fixtures::devices::cone`,
`n·ẑ = 65/97 ≈ sin 42°`), before we commit to building the tier out.

The spike must produce (the four DEV.1 criteria): (1) a certified **rational** enclosure of the development
angle `ψ(σ)` with a rational width; (2) a certified flat point that **encloses / agrees with** the float
diagnostic across the gore (oracle ∧ audit); (3) a verdict-typed backward-error + **DRC** scaffold; (4) the
**seam / closure** scoped, not hand-waved. And it must **select the enclosure method** and justify it.

## 1. The transcendental core, isolated

For a cone (`h ≡ 0` ⇒ pedal `c ≡ 0`, apex at the origin) the development map `D = γ + μ̂·ρ·e(ψ)` collapses
to a **polar map** `D(σ, μ̂) = μ̂·ρ(σ)·(cos ψ(σ), sin ψ(σ))` (`γ ≡ 0`). Reducing the exact rational fields
of the device charts (measured, not assumed — `develop::cone` verifies it as a polynomial identity):

| field | `cone()` (`sinβ = 65/97`) | `cone_alt()` (`sinβ = 3/5`) | general |
|---|---|---|---|
| **angle** `ψ′ = det(n,n′,n″)/\|n′\|²` | `(130/97)·1/(1+σ²)` | `(6/5)·1/(1+σ²)` | `c/(1+σ²)`, `c = 2 sinβ` |
| ⇒ `ψ(σ) = ∫₀^σ ψ′` | `(130/97)·arctan σ` | `(6/5)·arctan σ` | **`2 sinβ · arctan σ`** |
| **radius** `ρ = \|n′\| = √(normal_deriv_sq)` | `(144/97)/(1+σ²)` | `(8/5)/(1+σ²)` | surd `√(rational)` |

Two facts fall out, and they are the whole reason the cone is the right spike target:

- **The sole genuinely-new transcendental is a single `arctan` of a rational argument.** `ψ′` is a rational
  function whose integral is elementary; for the cone it reduces to `c/(1+σ²)`, so `ψ = c·arctan σ` exactly.
  This is the **textbook cone-development law** `ψ = sinβ · φ₃D` (flat sector angle = 3D azimuth × sin of the
  half-angle), since `φ₃D = 2 arctan σ` for the quaternion parametrization and `c = 2 sinβ`. `develop::cone::cone_angle_coeff`
  **verifies** `ψ′·(1+σ²) ≡ c` as an exact polynomial identity — so the closed form is a *proven* fact, not a fit.
- **The radius is a surd** (`√` of a rational) in general — already in `lattice::Surd`'s wheelhouse — and for the
  device fixtures it is a *perfect-square* rational (`(144/97)²`, `(8/5)²`), i.e. exactly rational; no new arithmetic.

So DEV is not "certify arbitrary transcendentals." It is "certify `∫(rational)` — an `arctan`/`log` — with a
rational error bound," radius handled. That is a bounded, well-understood problem.

## 2. Method selection

The DEV.1 gate lists three candidate enclosure methods. **Selected: (a) closed-form `arctan` with certified
rational bounds**, via alternating-series brackets over ℚ.

| method | verdict | why |
|---|---|---|
| **(a) closed-form arctan + rational bounds** | **SELECTED** | the integrand is rational and integrates to a *single* `arctan`; the alternating Maclaurin series gives a rigorous rational bracket `[Sₙ, Sₙ₊₁]` with width `≤ \|t\|²ⁿ⁺¹/(2n+1)`; argument reduction (odd symmetry; `arctan x = arctan ½ + arctan y`, `\|y\| ≤ ⅓`; `arctan x = π/2 − arctan 1/x`) keeps every argument `≤ ½` so convergence is geometric. Reuses only ℚ arithmetic — no new trusted primitive. |
| (b) verified interval integration | fallback | fully general (any rational `ψ′`), but a monotone-sandwich enclosure converges only `O(1/N)` — far looser than (a) for the same cost. Kept as the general path for DEV.2's **`γ = ∫e(ψ)`** (the nested directrix integral of general charts), which is *not* elementary. |
| (c) Taylor models | rejected | overkill for a rational integrand that integrates in closed form. |

The primitives live in `develop::interval` (`arctan`, `pi`, `cos`, `sin` over intervals, `sqrt` by bisection);
`develop::cone` composes them into `ConeDevelopment::point → FlatBox` (a rational rectangle proven to contain
the true flat point). **No float enters any certificate** — endpoints and the width are rationals; the float
`develop_cone` only corroborates (`vv-guide` "Milestone E · Doctrine").

## 3. Results (all reproduced by tests)

- **Criterion 1 — certified rational angle enclosure.** `ConeDevelopment::angle(σ)` returns `[ψ_lo, ψ_hi] ⊆ ℚ`
  with a rational width that shrinks geometrically in the term budget (`develop::interval` and
  `develop::cone` unit tests). `seam_angle`/`flat_sector` enclose the `σ→∞` limit (see §4).
- **Criterion 2 — certified flat point corroborates the oracle.** Across the certified gore `σ ∈ [0,1]`,
  `μ̂ ∈ {−1, −½}`, the certified `FlatBox` center agrees with the independent float `develop_cone` (radius =
  apex distance, angle = accumulated `acos` of successive unit rulings) to **`max_diag ≈ 1.48e-8`** — which is
  the *diagnostic's own* discretization accuracy at 2000 σ-rows, not the certificate's error. The certificate
  is centered on its intended analytic value to **`≈ 2e-12`** (the f64-readout limit). Oracle ∧ audit: the
  float diagnostic *checks* the rational certificate; it never defines it.
- **Criterion 3 — backward error + DRC, verdict-typed.** `FlatBox::backward_error` is a rational upper bound
  on `\|center − D_true\|` (the box half-perimeter). On the device cone at 16 series terms it is
  **`max_backward_error ≈ 1.0e-11`** — sub-nanometre on a millimetre-scale part, orders of magnitude under any
  flex-PCB clearance. `develop::cone::drc(ε, clearance)` returns `Verified(ε)` when `ε < clearance/2`
  (`spec:402`), else `Unresolved(ε)` (refine by more terms) — never a float compared with a float, and never a
  bare `Refuted` (a loose enclosure is not *wrong*, only not yet tight).

## 4. Seam / closure scoping

A finite rational chart sweeps a **bounded** azimuth, so one chart is a *gore*, never a closed cone. The full
wrap is `σ: −∞→∞ ↔ φ₃D: −π→π` (one 2π turn); the closed cone develops to a flat **sector of angle `2π sinβ`**
(≈ 240.9° for β ≈ 42° — the textbook developed-cone "pac-man" sector, `< 360°`). The lap **seam** sits at the
`σ→∞` limit of the parametrization, whose certified flat angular position is `ψ(σ→∞) = c·π/2 = π sinβ`
(`ConeDevelopment::seam_angle`; `flat_sector` gives the full `c·π`). Both are rational enclosures via the same
`π` bound. Closing the full cone (multi-gore assembly / the `σ→∞` limit face + the overlap seam geometry) is a
**post-GO (DEV.2)** deliverable — but its angular position is already pinned rationally, so "cone with a seam"
is scoped, not hand-waved.

## 5. The one engineering wall — and its remedy (**resolved in DEV.2a**)

> **Update (DEV.2a, done).** The remedy below is implemented: `lattice::Rat` gained `floor`/`ceil`
> (panic-free, Kani-covered), and `develop::interval` now carries every series accumulator as an interval
> rounded *outward* to `2^60` (`RatIv::round_out`). At a 40-term budget the device-cone `FlatBox` endpoints
> are **≤ 19 digits** (was hundreds–thousands), the backward error is `≈6e-12`, and the corroboration still
> holds at `1.5e-8` — with the plain `rat_to_f64` (the `big_rat_to_f64` workaround is gone). The rest of this
> section is the original finding, kept for the record.

**Finding: naive exact-rational composition of the power series blows up the digit count.** The angle argument
`ψ` is itself a many-digit rational (from the `arctan` series), and feeding it into the `cos`/`sin` series
raises it to high powers — at 16–20 terms the exact endpoints of the device-cone `FlatBox` carry **hundreds to
thousands of decimal digits**, even though their *values* are `O(1)`. Consequences: the certificate is
unwieldy to store/verify, and a naive `numerator/denominator → f64` cast overflows *both* to `∞`, yielding
`∞/∞ = NaN` (this bit the first corroboration harness — caught because a `NaN` is silently dropped by
`f64::max`, so a broken test looked green; the fix was a leading-digits `big_rat_to_f64`).

**Remedy (standard, well-understood): fixed-precision interval arithmetic with directed (outward) rounding.**
After each series operation, round the interval endpoints *outward* to a bounded denominator (e.g. a power of
two). This keeps every intermediate to a fixed digit budget while preserving rigor — outward rounding only
ever *grows* the enclosure, never loses containment. This is exactly how rigorous-numerics libraries operate;
it is a DEV.2 build item, **not** a viability risk. (Its clean implementation wants `floor`/`ceil` on `Rat`,
which `lattice` does not yet expose — a small, additive `lattice` addition tracked for DEV.2.)

The GO stands on the method being *proven correct and corroborated*; tractability-at-scale is a known
engineering follow-on, not an open question.

## 6. Decision: **GO**, and what DEV.2 builds

**GO.** The certified development converges (geometric in the term budget), is verdict-typed, stays float-free
in the certificate, and is corroborated by the independent float diagnostic to `1.5e-8` with a certified
backward error of `1e-11` on the device cone. The transcendental core is a single `arctan` of a rational — the
mildest possible new tier.

DEV.2 (post-GO, each its own slice) — sketched, not authored here:

1. **Fixed-precision outward rounding** in `develop::interval` (§5) + the small `lattice` `floor`/`ceil`
   addition, so certificates stay bounded-digit at any term budget.
2. The **`develop` crate proper**: `unroll (σ,μ)→flat` (direction ①) and `fold flat→(σ,μ)` (②), with the
   backward-error certificate + DRC wired into `certify_core`.
3. **General charts**: `γ = ∫e(ψ)` (the nested directrix integral — where interval integration, method (b),
   earns its place) and non-canonical placements (`arctan`/`log` of a Möbius argument).
4. The **angular closure + seam**: multi-gore assembly / the `σ→∞` limit face, and the overlap geometry of the
   physical lap seam.
5. Both **product pipelines end-to-end**: intersect→outline→unroll (①) and ECAD→fold→solid (②).
