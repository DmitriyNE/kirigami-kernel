# Engineering log — todos, tech debt, findings, deferred items

A low-ceremony running log for things we hit *during* other work and don't want to stop
for: **todos**, **tech debt / sketchy things** (including soundness-scope caveats),
**findings** (surprises worth remembering), and **deferred** items (punted to a later
milestone, with a reason). See something mid-task → add a bullet under the right section,
keep going. The point is to not interrupt the main task, and to not lose the thread either.

This is the *general* log. Two siblings own narrower scopes — cross-referenced, never
duplicated: [`docs/proofs/ledger.md`](proofs/ledger.md) (formal proof obligations, Lean/Kani
status per checker) and [`../vv-matrix.md`](../vv-matrix.md) (the V&V coverage matrix,
CI-gated).

**How to add an entry:** a bold one-line title, a sentence or two of substance, then a
trailing italic line `date · status · ref`. Status ∈ `open · deferred(→Mx) · watching ·
resolved`. On resolve, move the bullet to **Resolved** at the bottom, keeping its date and a
one-line outcome. New sections (e.g. a **To do** that grows, or a per-milestone bucket) are
fine — this is a log, not a schema.

## To do

- **The driving requirement (product north star): the bidirectional multilayer flex-PCB transform.** The kernel
  exists to (**① develop** 3D→flat — generate the flat PCB outline by intersecting the generating shape with 3D
  geometry, then unroll) and (**② fold** flat→3D — fold flat ECAD data into folded 3D geometry). Framing + the
  exact-vs-transcendental mapping now live in `docs/implementation-plan-v1.md §6`. Two first-class threads fall out,
  and neither is a tail deferral: (a) **certified development** (the flat↔3D isometry, the `develop` crate / M-E) —
  the keystone *both* directions pivot on, today only a float diagnostic (`export::mesh3d`); (b) the **exact-
  intersection → outline** path (direction ①) — `resultant`/`AlgReal`/`arrange2d`/CLIP produce the PCB outline as a
  3D-intersection result, feeding the free-boundary emit machinery (`export::brep_build::brep_freeboundary`) already
  built in M-D. Multilayer = the `w` thickness dimension (native to the chart; layers at distinct `w`-offsets).
  *2026-08-10 · open (product framing recorded; DEV reprioritized in Deferred) · `docs/implementation-plan-v1.md §6`*

- **DEV / M-E = certified development (the flat↔3D layer); the chosen next big bet, opened as a GO-gated spike.** Product-decision (with the user): after M-D's exact 3D closed solids, the next thread is **DEV**, not the D4.4 atlas — because DEV is the product bottleneck (both directions pivot on it) and the highest-risk unknown (retire-highest-risk-first). Reasoning captured in the exchange: "exactness is a representation property, not a shape property" — the closed cone / **seam** / full 2π wrap is *transcendental* (a rational chart sweeps a bounded azimuth `<2π`, so one chart = a gore), so the seam and general shapes are DEV + rational-input approximation, not algebraic intersection. GO-gate criteria authored in `docs/vv-guide.md` (Milestone E (DEV)). **The spike (DEV.1)** = a certified rational enclosure of the cone's development angle `ψ(σ)=∫ψ′` (`ψ′=chart.psi_prime`, rational ⇒ arctan/log; radius `ρ=|n′|` is a surd, already in `lattice::Surd`), checked against the float ground-truth `export::mesh3d::develop_cone`, verdict-typed, with the backward-error `sup|D(â)−g|≤ε` + DRC `ε<clearance/2` scaffold and the seam as the acceptance case; it **selects the enclosure method** (closed-form arctan/log + certified rational bounds ∣ interval integration ∣ Taylor models) and GO/no-go's the tier. Additive (its own spike boundary; the pure exact tier untouched). *2026-08-10 · **DEV.0 + DEV.1 met — decision GO** (`docs/spike-development-report.md`); the cone development reduces to a single `arctan` of a rational (`ψ = 2 sinβ · arctan σ`, verified as an exact polynomial identity), method (a) closed-form arctan + rational alternating-series bounds selected, certified backward error `≈1e-11` corroborated to `≈1.5e-8` — see the DEV.1 finding below · `docs/vv-guide.md` Milestone E (DEV), `docs/implementation-plan-v1.md §6`*

- **DEV.1 spike GO — the cone development is a single `arctan`, and the one wall is digit-growth.** The spike (`crate develop`: `develop::interval` rational enclosures of `arctan`/`π`/`cos`/`sin`/`√`; `develop::cone` composing them into a certified `FlatBox`) priced the certified flat↔3D development on the device cone and **GOes**. Findings: (1) **the transcendental core is minimal** — `ψ′ = det(n,n′,n″)/|n′|²` reduces to `c/(1+σ²)`, so `ψ(σ) = c·arctan σ` with `c = 2 sinβ` rational (the textbook `ψ = sinβ·φ₃D`); `cone_angle_coeff` **verifies** `ψ′·(1+σ²) ≡ c` as an exact polynomial identity, and the radius `ρ = |n′|` is a surd (perfect-square-rational for the device fixtures). So DEV is "certify `∫(rational)` = an arctan/log," not "certify arbitrary transcendentals." (2) **Method (a)** (closed-form arctan + alternating-series rational brackets, argument-reduced to `|t|≤½` for geometric convergence) beats interval integration (`O(1/N)`, kept as the DEV.2 fallback for the non-elementary `γ=∫e(ψ)`) and Taylor models. (3) **The wall: naive exact-rational composition of the `cos`/`sin` series over a many-digit `arctan` argument blows the endpoint digit count to hundreds–thousands** (values `O(1)`, representation huge). This bit the corroboration harness — a `numer/denom→f64` cast overflowed both to `∞`, `∞/∞=NaN`, and `f64::max` silently dropped every `NaN`, so a *broken* test read green (checking only the trivial `σ=0` row). Caught by challenging the too-perfect `max_diag==max_analytic`; fixed with a leading-digits `big_rat_to_f64`, and it re-surfaced the real numbers (backward error `1e-11`, corroboration `1.5e-8`). **Remedy: fixed-precision interval arithmetic with directed (outward) rounding** — a DEV.2 build item (wants a small additive `floor`/`ceil` on `lattice::Rat`), *not* a viability risk. *2026-08-10 · GO · `docs/spike-development-report.md`, `crates/develop/**`, `export::mesh3d::certified_flat_point_corroborates_develop_cone`*

- **DEV.2 planned — the certified development tier for the *closed-form* developable class; two scope decisions with the user.** After the DEV.1 GO, the next milestone builds out the tier. Two framing decisions locked with the user: **(1) generality = the closed-form class, now.** DEV.1's foundation is already general (the `interval` enclosures, `ρ=√(‖n′‖²)` surd, `ψ=∫ψ′` arctan/log-class for any chart); DEV.2 broadens from the device cone to every developable whose development is *elementary* — cones at any placement (`ψ=∫P/Q` = a sum of arctans/logs, needs a new `log` enclosure + partial fractions; higher-degree `Q` over `AlgReal` flagged) and cylinders (`ψ′≡0` ⇒ `e(ψ)` const ⇒ `γ` elementary). The genuinely non-elementary case — `γ=∫[rational]·e(arctan)` with a **curved directrix** (tangent-developables / arbitrary ruled) — is deferred to **DEV.3** (verified interval integration, the DEV.1-selected method (b), its own GO). **(2) creases = atlas, not `develop`.** `develop` certifies the flat↔3D isometry of a *single* chart; the multi-panel **creases / fold-mates** (spec §5.3 MONO; the reflection mate `n_B=n_A−2(n_A·B/B·B)·B`, already built for one joint in the M-D closure/sew layer) are the **atlas** (D4.4) + `closure`/`sew`. So direction ② splits: `develop` supplies the per-panel `D⁻¹`+chart-eval map, the atlas assembles across creases. Slice arc: DEV.2.0 (docs) · DEV.2a fixed-precision outward rounding (the digit-growth remedy — `Rat::floor`/`ceil` pure + Kani, `RatIv::round_out`) · DEV.2b general closed-form angle · DEV.2c ANCHOR backward-error certificate (T-part, `develop`, composes with the pure `certify_core` A-part) · DEV.2d unroll ① · DEV.2e fold-inversion ②. *2026-08-10 · **DEV.2.0 + DEV.2a + DEV.2b met**. DEV.2a (`3e18c61`) retired the digit-growth wall: `lattice::Rat::floor`/`ceil` (pure, Kani panic-freedom `floor_ceil_fast_path_panic_free_full_domain`) + `develop::interval::round_out`/`ROUND_BITS=60` carried through every series accumulator → device-cone endpoints ≤ 19 digits at 40 terms, backward error `≈6e-12`, corroboration `1.5e-8` (the `big_rat_to_f64` workaround gone). DEV.2b generalized the angle: `develop::cone::angle_enclosure` integrates `ψ=∫P/Q` by completing the square on a degree-2 positive-definite `Q` → `(a/2A)·log((σ−p₀)²+q₀²) + ((ap₀+b)/Aq₀)·arctan((σ−p₀)/q₀)` (surd `q₀` via `sqrt` + `RatIv::recip_pos`), enclosed by the new `interval::log` (`atanh` series + power-of-two reduction + geometric tail bound) and `interval::arctan_on` (interval argument). `Verdict`-shaped with `AngleDefer` (higher-degree → `DenominatorDegree`, real-roots → `RealRoots`, unsigned radius → `RadiusNotSigned`) so a non-closed-form chart is a clean `Unresolved` pointing at the `AlgReal` extension / DEV.3, never a silent `None`. Reproduces DEV.1's `c·arctan σ` on `cone()`/`cone_alt()` across the gore, certifies a reparametrized cone `q(σ−1)` (`Q=σ²−2σ+2` ⇒ `(130/97)(arctan(σ−1)+π/4)`) the canonical recognizer declines, and validates the log branch on `σ/(1+σ²)=½ln(1+σ²)`; all float-corroborated to `≈1e-9`. Full gate green (fmt, clippy `-D warnings`, nextest ws 413 + export/step+diagnostics 57, doctests, `-D missing_docs` develop, `xtask lint`, no_std thumbv7em). DEV.2c (ANCHOR T-part) next · branch `dev-go-gate` · `docs/vv-guide.md` Milestone E DEV.2, plan `plan-first-and-then-twinkly-minsky.md`*

- **Curved MITER-FIT = the transverse-rational `φ_J` correspondence (L3 activation); the machinery D4.2 needs, pursued as its own milestone.** D4.2 (a two-flank closed solid on `one_joint()`) is **fixture-obstructed, not blocked by missing code** (see Findings) — so per the standing "build the incomplete machinery, don't manufacture demo geometry" directive, this milestone builds the deferred **curved MITER-FIT**: the *transverse* regime where two flanks' cut rulings are **rationally** (not affinely) parametrized and their coincidence in the bisector plane Π is certified through the correspondence `R(σ_A,σ_B)=0` (spec §5.3; `certify-core/src/miter.rs:31-32` + `docs/closure-scoping.md:52-54` defer it). First downstream wiring of `lattice`'s built-but-unused `resultant`/`resultant_bivariate` (and, later, `AlgReal` + conic carriers). **Earned, not oracle (OCCT never enters):** the certificate is a resultant-conditioned **divisibility identity** — on `{R=0}` (paired rulings share their crease-line point, so position identity is free) certify carrier identity `D_A ∥ D_B` + extents `E_{A,±}=E_{B,π(±)}` by an **exact cofactor** `X == R·Q` (`X=R·Q ⇒ X≡0 on {R=0}`, an exact implication); the only trusted lemma is resultant⇔common-root (Lean, out of Kani per vv-guide §5 — `verify_common_factor` is "exactly the spec's resultant-conditioned A-identity"). Watertightness does not hinge on it (a non-coincident cut is a valid exposed LEDGE, spec §5.3). Slices: CM.0 (criteria) → CM.1 (`miter_fit_transverse` checker + Kani, **additive** to `certify-core` beside the degree-1 `miter_fit`) → CM.2 (conic carriers) → CM.3 (`AlgReal` wiring) → CM.4 (closure searcher + minimal cone-flank sub-fixture) → CM.5 (Lean frontier, non-gating). Criteria in `docs/vv-guide.md` (Curved MITER-FIT). *2026-08-09 · CM.0 + CM.1 met — CM.1 landed `lattice::Biv` (bivariate polynomial over ℚ, the first consumer of the `resultant_bivariate` convention) + `certify_core::miter::miter_fit_transverse` (forms `R(σ_A,σ_B)` from `ℓ_A = ℓ_B`; carrier + extent identities by the exact cofactor `X == R·Q`; `ℓ_i` monotonicity by Sturm; `ε_φ` from slope signs via the Kani-proven `eps_from_slopes`), additive beside the degree-1 `miter_fit`; genuinely-rational symmetric pair certifies, curvature-order / extent-counterexample / parallel-regime / wrong-cofactor refused. Full gate green (nextest ws 375, export/step 37, doctests, `-D missing_docs`, `xtask lint`, no_std thumbv7em, Kani `eps_from_slopes_is_slope_agreement`). **CM.2 (conic carriers) SKIPPED** — unsound as framed (see Findings): a conic is non-D24 content CAP-IN-D24 correctly refuses, and the clean-miter path uses straight rulings, not conic carriers; deferred to the conic-arrangement L3. **CM.3 met** — first downstream use of `lattice::AlgReal`: `AlgReal::sign_of` + `AlgReal::count_roots_upto` (polynomial sign / root-count at & up-to an algebraic σ) + `certify_core::miter::strictly_monotone_upto_alg` (transverse monotonicity certificate over an algebraic cut-face σ-bound — the cone's cut-exit σ). Full gate green (nextest ws 378, export/step 37, doctests, `-D missing_docs`, `xtask lint`, no_std thumbv7em). CM.4 (cone searcher + minimal fixture) next · branch `curved-miter-fit`*

- **M-D slice 4 = atlas assembly → the certified closed solid; the spine is a new *proven* `certify-core` checker.** Slice 3 left every solid *certified-seam, honest-open*: closedness of the whole solid is decided only by OpenCASCADE `BRepCheck` — an **oracle, not the certificate** (spec §8.2:332). The spec has **no predicate certifying whole-solid closedness** (`VALID_solid-closure` §8.6:439 is only `VALID_complement ∧ ⋀_j CLOSURE_VALID(j)`, joint-local); the docs pre-name the missing layer ("ruled sidewalls carrying their own CAP-OUT/SEW-LINK coverage → whole-solid watertightness certified") but flag it unbuilt → **atlas assembly**. Slice 4 builds it, spine-first: a `certify_core::shell::closed_shell` **closed-2-manifold** checker (checks 1 range, 2 wires-closed, 3 **∂²=0** oriented edge census, 4 **vertex-link single-cycle** via a rotation-system permutation) — the assembly-scale analogue of the `CapOut.lean:25-30` frontier theorem — Kani-proven bounded, composed into an **additive** `valid_closed_solid` gate, and *corroborated* (never overturned) by the OCCT oracle. Two doctrines bake in: **incidence not proximity** (spec:192, faces share an exact edge *id*, never a tolerance) and **earned not oracle** (a forced `closed=true` is oracle-instead-of-audit). **Single-flank first (geometry forces it):** M-D D.1 *proves* the two flanks' crease coincides only at the neutral sheet `w=0`, so a two-flank watertight slab is obstructed (the `w=t` outer creases diverge → gluing yields a non-manifold edge), while a single-flank bent box (top `w=0` + bottom `w=t` + four ruled sidewalls over the **support box** — a legitimate free-boundary contour, spec:151) *is* an exact closed 2-manifold; the two-flank union is its own phase (D4.2). The "exact closed slab by-construction" slice 3 **declined** becomes legitimate here precisely because D4.1 now supplies the missing certificate — no anchors / authored contour / multi-joint machinery needed for the first closed solid (those are D4.3/D4.4). The TCB edit is **purely additive** (new `shell` module + Kani harness + `valid_closed_solid`; `arrange.rs`/`sew.rs`/`boolean.rs`/`valid.rs` untouched in D4.1). Phases: D4.0 (criteria) → D4.1 (checker + single-flank closed slab, the "both" slice) → D4.2 (two-flank union / the `w=0` obstruction) → D4.3 (contour + anchors, spec §4.6) → D4.4 (multi-joint / atlas container) → D4.5 (sew sidewall coverage, additive) → D4.6 (Lean 2-manifold theorem, frontier, non-gating). Criteria in `docs/vv-guide.md §8` (Milestone D slice 4). *2026-08-09 · D4.0 met (`2ffceda`) + **D4.1 met** — the first certified closed solid: `certify_core::shell::closed_shell` + 2 Kani harnesses (`5763183`), `valid_closed_solid` gate (`77d39a7`), `export::brep_slab_from_closure` + `Brep::to_shell_certificate` (`0b40734`), rational-patch surface FFI + `Vec3Rat::reduce` + e2e OCCT corroboration (`aaea9c0`). Two findings below (degree-inflation reduce; `Geom_BezierSurface` vs segfault). D4.2 (two-flank union) next · branch `milestone-d-atlas`*

- **Milestone D scoped as a sequence of slices; slice 1 = the physical joint fixture.** The roadmap's D
  (`implementation-plan-v1.md:53`) is the whole device (cone + lap-seam + petal atlas → lens-assembly
  solid) — a culmination, not one vertical slice. Decomposed into three threads (physical fixture / audit +
  `V_∂`-guided seam + OCC oracle / atlas breadth); criteria in `docs/vv-guide.md §8` (Milestone D). **Slice 1**
  discharges the three M6 fixture warts — `h ≡ 0` cone → true `h ≠ 0` cylinder, disjoint-support gap → two
  distinct flanks sharing one crease, stretched cap → metric-faithful `Surd(a,b,s)` lift — with the joint
  still certifying through both the MITER and LEDGE branches. Two readings locked: **`VALID_material` → M-E**
  (needs SMOOTH/DEFERRED bands + FRESH, both E — consistent with the FRESH deferral below); **the
  external-kernel audit is an *oracle*, not the certificate** (spec "no kernel CSG"; region/shell
  manifoldness is CAP-OUT-LINK / SEW-LINK; "oracle ∧ audit, never oracle-instead" §8.2:332) — that governs
  thread 2, not slice 1. *2026-08-09 · open · `docs/vv-guide.md §8` (Milestone D), branch `milestone-d`*

- **M-D slice 2 = the OpenCASCADE differential oracle (thread-2 half b); the watertight V_∂ seam is slice 3.**
  Wire OCCT `BRepCheck` as an oracle **compared** against the internal verdict (a strings-only
  `occt_shell_audit` reporting free-edge / non-manifold-edge / closedness facts beyond bare `IsValid()`, a
  test-only `export::differential` harness mirroring `difftest`), and have `export` **consume** the certified
  `v_boundary()`/`pinches()` read-only (comparison layer + a `cap_tris` gate on `pinches().is_empty()`).
  Scope split forced by geometry: slice 1's 2:1 ruling-speed overhang means a geometrically-coincident V_∂
  seam does not exist at the sampled band, so the oracle's headline output is *surfacing* that overhang as a
  documented, CI-enforced divergence (OCC free-edges/non-watertight vs internal manifold) — never overturning
  the certificate. The geometry-changing seam (indexed-shell FFI + geometry-derived `SewInput`) is **slice 3**.
  Criteria in `docs/vv-guide.md §8` (Milestone D slice 2). *2026-08-09 · DONE — slice 2 met (`c5800e8` criteria, `1eb404a` shim+audit+CI leg, `9d59418` differential harness + `pinches()` gate; agreement + documented overhang divergence asserted for both cap branches) · `docs/vv-guide.md §8`, branch `milestone-d`*

- **M-D slice 3 = exact ruled-surface STEP emission (certified-seam, honest-open); the slice-2 "indexed-shell / V_∂-welding" framing is superseded.** The STEP *body* the spec mandates is **exact rational surfaces**, not triangles (§10:464 "face surfaces exact; sidewalls exactly ruled …; no kernel CSG"; §11:470 makes discrete meshes an explicit **non-peer** export). The current triangle soup (`shell.rs` σ-grid samples of `chart.surface` + D24-square cap, `occt_shim.cc` float-tolerance sewing) is therefore a stopgap — and it *manufactured* the 2:1 overhang: sampling the untrimmed `μ∈[−1,1]` rectangle never applies the certified plane trim, so the band is unavoidably open. The exact object already exists (`Chart::surface(μ,w) = c(σ)+μ·r(σ)+w·n(σ)` is a `Vec3Rat`); the gap is the emission path. Fix per §5.3: emit each flank as an exact ruled face **trimmed by the exact bisector plane Π**, with the shared Π-cut edge referenced **by identity** (watertight-by-construction) — MITER where the trims coincide (empty ledge), an exposed planar LEDGE (`face_A △ face_B`, a boundary step not a hole) where they don't. **Scope decision (with the user): certified-seam / honest-open** — the certificate is joint-local (SEW/CAP-OUT cover only the seam edges + `V_∂` links, nothing certifies the substrate outer boundary; by P1:12 a joint is a slice of an atlas, closing sidewalls are "ruled over anchors" §:192/:464 = unbuilt machinery `one_joint()` has no contour to feed), so emit only certificate-backed exact faces and leave the substrate boundary honestly open (annotated), never a fabricated `closed=true` (that would be oracle-instead-of-audit). Representation = **Strategy B** (emit exact rational-Bézier boundary curves, let OCCT build the ruled/linear-extrusion surface; the watertight object is the shared 1D edge). Order MITER→LEDGE. **Explicitly declined this slice:** the "exact closed slab by-construction" (support-box sidewalls to force `closed=true`, closedness uncertified away from the joint) and the certified closed solid (anchored contour → ruled sidewalls with their own SEW/CAP-OUT coverage), the latter deferred to **atlas assembly**. Phases D3.0 (criteria) → D3.1 (`bezier.rs` monomial→Bernstein + `brep.rs` exact IR) → D3.2 (MITER ruled flanks + surface FFI, GO/NO-GO on `Geom_*`/`MakeEdge` linkage) → D3.3 (LEDGE cap from `region().faces[].outer`) → D3.4 (differential flip + mesh retention). Criteria in `docs/vv-guide.md §8` (Milestone D slice 3). *2026-08-09 · DONE — slice 3 met. D3.0 criteria (`vv-guide §8`, `vv-matrix`); D3.1 exact primitives (`bezier.rs` + `brep.rs`, `9b3a320`); D3.2a surface FFI (`3db3746`); D3.2b fixture flip so the `w=0` neutral sheet is retained (`f9eebee`) + the mesh-cap fan fix (`8ee76a4`); D3.2 MITER two ruled sheets sharing the crease middle `M` by identity (`7fc1fb5`); D3.3 LEDGE exact body = the same two flanks, exact cap deferred (Option B — see the three-way `BRepCheck` box in Findings, `34843cf`); D3.4 differential harness audits **both** paths (watertight crease seam on the exact body, retained overhang on the mesh) + STEP body routed through `brep` + doc gate. Scope certified-seam / honest-open; the exact LEDGE cap + a genuinely-closed solid remain deferred (`V_∂` real-cut slice / atlas assembly). Gate green each phase (real exit codes, `--features step` leg included). · branch `milestone-d`*

- **Finding + pivot: D3.2's "share the Π-cut edge by identity" has no honest realization on `one_joint()` within the certified box — the phase was started before the machinery to place a shared 3D seam existed.** Grounded by exact computation over the fixture (throwaway `scratch_explore`, since deleted). The crease line is `L = {(x, 0, 1)} ⊂ Π`, anchor `x₀ = (0,0,1)`, bisector `b_J = (0,1,1)`. **At the fold crease `w = 0`:** flank A's crease ruling covers `x ∈ [−2, 2]` on `L`, flank B's covers `x ∈ [−1, 1]`; they overlap only on `x ∈ [−1, 1]` and A overhangs 2:1 — but this is *at `w = 0`*, which is **outside** the certified `w`-box `[1, 2]`. **Within the certified box** (`shell.rs` samples at `w = w_lo = 1`): the two flanks have **diverged entirely** — A rides out to `z = 2`, B down to `y = −1` — and share **nothing**. `G_A(σ=0, w=0) = 0` exactly (flanks touch Π only along the crease, at `w = 0`); `G_A(w=1) > 0` throughout (Full-retained). So the flanks are Full-retained and **disjoint** everywhere the certificate actually covers; the miter diamond and the SEW seam are **2D cap-frame licensing artifacts** (coincidence in the projected `(μ, w)` frame), not shared 3D edges. Emitting a single shared `TopoDS_Edge` between them would be **oracle-instead-of-audit** — fabricating watertightness the certificate does not back. **Root cause:** the plan assumed the certified region touches the crease where the flanks meet; it does not (`w`-box floor is `1`, crease is at `0`), and no existing machinery derives a certificate-backed 3D seam curve away from `w = 0`. **Decision (with the user):** land the surface-FFI infrastructure now (`occt_write_brep`/`occt_brep_audit` + `brep_to_buffers`/`write_brep`/`audit_brep`, proving `Geom_*`/`MakeEdge`/`BRep_Builder` link and that a hand-built two-face brep audits as 2-incidence / `nonmanifold==0` / valid), then **re-think the D3.2–D3.4 implementation plan** before wiring `brep_from_closure(MITER)`. The FFI is unblocked and independently valuable; the closure-wiring is what lacked machinery. *2026-08-09 · superseded by the D3.2b resolution below — FFI landed (`3db3746`); re-think done · branch `milestone-d`*

- **D3.2b resolution — the certified box is a *retention window*, not the face extent; the shared edge is the crease `L` (MITER-certified), and the emission is re-framed around it (Path A, with the user).** Re-derived the fixture's `w`-box mechanically (throwaway `scratch_wbox`, since deleted; drove the real `closure::trim` `field_a`/`GField::eval` API, numbers below match the algebra exactly). For flank A (`h≡1` ⇒ `pedal_A = n_A`, `x₀ = n_A(0) = (0,0,1)`, `b_J = (0,1,1)`): `G_A(σ,μ,w) = g0(σ) + w·g_w(σ)` with `g_w = (1−2σ−σ²)/(1+σ²)` and `g0 = g_w − 1 = −2σ(1+σ)/(1+σ²)`. **Why `w`-box `[1,2]` not `[0,2]`:** TRIM-LOCAL needs `G_A > 0` (with margin) over the whole box, but over the support `σ∈[0,1/8]` the neutral sheet `w=0` is on the *deleted* side for every `σ>0` (`g0(1/8) = −18/65 ≈ −0.277`); `w=0` and `w=¼` go negative, only `w≥1` clears with margin — the floor is forced by geometry + margin, not an arbitrary constant, and `[0,2]` is refuted at the `σ=1/8,w=0` corner (`RegFault::OuterFiber`). **Is `w=0` meant to be certified? No, and it structurally can't be:** the crease `(σ=0,w=0)` sits *exactly on* Π (`G_A=0`); the retained region `G>0` is an open half-space and the crease is a boundary point, so any product-of-intervals box strictly inside `G>0` is necessarily bounded away from `w=0`. **Consequence:** inside `[1,2]` `G_A>0` strictly ⇒ CLIP-DOM Full-retained, *no cut in the box*; the Π-cut curve `{G_A=0}` runs from the crease `(0,0)` up to `≈(1/8, 0.38)`, entirely below the `w=1` floor. The one real shared 3D edge is the crease line `L={(x,0,1)}` at `w=0`, and that is exactly what MITER-FIT/SEW certify as PAIR-IDENTICAL. So SEW/MITER (seam at `L`, `w=0`) and TRIM/CLIP (retained slab, `w∈[1,2]`) certify **disjoint regions with an uncertified `w∈[0,1)` gap**; the earlier NO-GO was correct that no Π-cut is shared *in the slab*, but wrong to conclude nothing is shared — the crease `L` is. **Decision (Path A, with the user):** re-frame D3.2 emission so the shared edge is the MITER-certified crease `L` and the `[1,2]` box is read as the *retention witness*, not the face boundary — each flank ruled face carries `L` (or its common sub-segment, split at the 2:1 overhang into a shared middle + free tips) as a boundary edge referenced by identity. No fixture surgery (the flip-support alternative — support `σ∈[−1/8,0]` makes `G_A≥0` at `w=0`, confirmed 0.215/0.117/0.060/0, so the box *could* reach `w=0` — was declined: it re-keys the MITER/SEW/LEDGE packets and changes which flap is retained, for no gain over consuming the seam MITER already certifies). D3.2–D3.4 re-planned around `L`. *2026-08-09 · resolved — Path A chosen; D3.2 emission re-plan next · branch `milestone-d`*

- **D3.2b landed (fixture flip) + a stale-comment finding.** Deeper `surface(μ,w)` probing (throwaway `scratch_surf`, since deleted) settled two things. (1) **The `w=0` neutral sheet is a genuine 2D face** for the `h=1` cylinder (as σ sweeps, the ruling — an x-segment — moves through `(y,z)`: `(∓1.97,+0.25,.97) → (∓2,0,1)=L → (∓1.97,−0.25,.97)`), *not* degenerate — so `shell.rs`'s rationale "the ruled patch degenerates to a line at `w=0`" is **stale** (true for Milestone C's `h≡0` cone whose pedal collapses to the axis, false for the current cylinder), and that stale note is exactly what pushed the mesh emission to `w=1` where the flanks are disjoint. Fix `shell.rs` when the emission moves to `w=0` in D3.2. (2) **Pure Path A (keep the fixture, emit `w=0`) is not honest:** on the *current* support `σ∈[0,1/8]` the `w=0` sheet is on the *deleted* side of Π (`G_A<0`), so it would poke through into flank B's half-space. Path A's honest form therefore **converges with the flip:** support `σ_a∈[−1/8,0]`, `σ_b∈[1,9/8]` puts the `w=0` sheet on the retained side (`G_A≥0`, touching Π only at the crease), meeting along `L` (A's `x∈[−2,2]` ⊇ B's `x∈[−1,1]`, the 2:1 overhang → shared middle + free tips). **Change (with the user):** flipped `boxes()`'s σ-supports in `fixtures::closure_joint`; re-verified `closure_valid` still `Verified` via **both** the MITER and LEDGE caps (`confine=(0,1)`, `w=[1,2]` unchanged — `g_w>0`, `g_mu≡0` keep `(μ=0,w=1)` the box-minimizing corner, and MITER/SEW/LEDGE are Π-frame 2D, independent of the σ-support), full workspace `nextest` 350/350, fmt/clippy/xtask/doctests/`missing_docs=0` all green. The `w`-box `[1,2]` is retained as the retention *window*; the emission (D3.2) will carry `L` as the shared edge. *2026-08-09 · fixture flip landed; D3.2 emission next · branch `milestone-d`*

- **Finding: the `export` `step` feature is not exercised in CI at all.** `.github/workflows/ci.yml` runs
  `cargo nextest run --workspace` and the doctests with **no** `--features step`, and `export::step` is
  `#[cfg(feature = "step")]` — so the slice-1 STEP end-to-end suite (`one_joint_{ledge,miter}_writes_a_reloadable_step_shell`)
  is green only when run locally under `nix develop --features step`, never in CI. M-D slice 2 adds the
  missing dedicated `nix develop --features step` leg (mirroring the CGAL oracle leg at `ci.yml:65-66`), which
  retroactively covers slice 1. *2026-08-09 · DONE (`1eb404a`: the `--features step` CI leg runs `clippy` + `nextest -p export` + `step` doctests inside `nix develop`, covering the slice-1 `one_joint_*` tests and slice-2's `export::differential`) · `.github/workflows/ci.yml`*

- **CAP-OUT completeness bijections — source-ID permutation DONE; the two *further* bijections remain.**
  *Done (debt-sprint item 7, `56accab`; vv-matrix 🚧→✅):* the scalar coverage count
  `separating_count == region_boundary_count` is replaced by a real **source-ID permutation**
  certificate — `emit_region` stamps each emitted boundary edge with its `SubEdge` arena id, and the
  pure `certify_core::arrange::boundary_bijection_ok` checks the emitted-id multiset is a permutation of
  the separating-edge id set (so a drop-one/duplicate-another pair a scalar count misses is caught).
  *Remaining:* the other two spec bijections — {selected components} ↔ {emitted faces} (component ids)
  and V_∂ ↔ {emitted shell vertices} — plus per-loop closure/orientation. These fold into **item 8**:
  they reuse its per-component gauge + per-pair emission plumbing. *2026-08-06 · deferred(→8) ·
  `vv-matrix.md` completeness-bijections row*

- **CAP-IN-D24 input license — minimal totality guard DONE; the full newtype census remains M4.** *Done
  (debt-sprint item 5, `5ffbe34`):* `validate_d24(&[Edge]) -> Result<(), CapInFault>` runs *before*
  `Dcel::build` in `ledge_dom_certified`, checking per edge `r² > 0` (circles), `a²+b² > 0` (lines),
  each endpoint on its carrier (residual = 0), and canonical `x_lo < x_hi` — so a hand-crafted malformed
  edge (r² ≤ 0 circle, endpoint off carrier, degenerate line, non-canonical piece) now returns
  `CapOutFault::InvalidInput(CapInFault)` instead of panicking. The certified entry is now total over
  arbitrary `&[Edge]`. *Remaining:* the full spec §8.5 input license — `CanonicalEdge`/`ValidatedD24`
  newtypes minted only by a CAP-IN-D24 checker, so validity is carried in the type rather than
  re-checked at the boundary — lands with `closure`/M4, where the census already lives
  (`closure/src/lib.rs`). *2026-08-06 · deferred(→M4) · spec §8.5 CAP-IN-D24*
  - **DONE (C1, milestone-c):** `certify_core::cap_in` mints `ValidatedD24` (opaque, private-field
    `CanonicalEdge` cycle) only via `cap_in_d24`, which runs the full census — carrier identity by
    exact `on_carrier` rational-function residual (a conic satisfies no line/circle identity →
    `OffCarrier`, *falsely* not vacuously), finite interval, rational endpoints, closed cycle, and
    A/B flank correspondence — returning a two-valued `Verdict`. The `closure::cap_in` searcher
    projects a flank chart into the cap plane (`PiFrame`, `project`, `ruling_edge`, `sigma_edge`,
    `line_through`): a cylinder ruling → line passes; a cone σ-cut → conic is refused. Consumed on
    the LEDGE branch only. The `arrange2d::validate_d24` boundary guard stays as the totality
    net; the type-level license supersedes it as the *input* gate. *2026-08-08 · done(C1) · spec §8.5*
  - **DONE (C2, milestone-c) — regularity bundle + a SIDE/COLLAR scope split.** `certify_core::wedge`
    checks REG-V ∧ WEDGE ∧ EXT-WEDGE at the crease. On the straight-crease **constant-V** scope
    `|V|² = (1 − d)/(1 + d)` with `d = n_A·n_B`, so all three are **division-free `Rat` ring
    comparisons** clearing `1 + d > 0` — no Sturm/span (simpler than `reg_q`, same `MarginSq`/`Verdict`
    idiom). The searcher `closure::wedge::wedge_cert` evaluates the two flank charts' unit normals at
    the crease stations; the checker re-derives `d` and verifies the normals are unit before clearing.
    **Scope decision (feeds C3):** SIDE(b_J) and COLLAR are bundle members whose crease-local witness is
    *implied* by REG-V ∧ WEDGE (`|b_J|² = 2(1 − d) > 0`; the `Q(s)` split is complementary for free) and
    WEDGE ∧ EXT-WEDGE (quotient-wedge embeds) respectively — so C2 delivers three *independent*
    crease-local atoms, not five. SIDE's independently-refutable "wrong-side" content (retained side
    `G_i ≥ 0` over the actual support) is **TRIM-LOCAL** and COLLAR's cross-t **TUBE** padding by
    `D²_collar = 4w²s_bev²|V|²/(1+s_bev²|V|²)` is **TUBE-LOCAL** — both need the `G_i`/tube fields, so
    they land in C3 with their siblings, not fabricated as thin crease-local predicates here. `s_bev`
    and the REG-V margin are authored treatment data threaded through the searcher call (not on
    `Joint`), to be folded into the `{s_J, b_J, φ_J}` closure bundle at C6. *2026-08-08 · done(C2) ·
    spec §8.5 :266/:382*

- **CAP-OUT strict-manifold entry (`ShellReady`) — decide when SEW lands.** `ledge_dom_certified` is
  deliberately *relaxed*: a pinch (non-manifold vertex, e.g. a transverse `△`) is a valid, reported
  result (`CapOut.pinches`), not a refusal — the manifold requirement is owned by the downstream SEW-LINK
  gate, and there is no pre-SEW consumer today (confirmed in review batch 1). When SEW (M4/M5) is built,
  reconsider a typed strict entry `ledge_dom_manifold → ShellReady<B>` that additionally gates
  `pinches.is_empty()` and returns a type only a no-pinch region inhabits — so "forgot to check
  manifoldness" is a compile error and the proven `link_ok` is used in production. Deferred (not now)
  because the newtype's contract is SEW's to specify; building it blind risks guessing wrong.
  *2026-08-06 · deferred(→M4) · `certify_core::arrange::link_ok`* **· C4 review (2026-08-08):**
  re-confirmed the deferral — `closure::ledge::ledge_cap_certified` (the C4 LEDGE driver) returns the
  **relaxed** `Verdict<CapOut>` verbatim, reporting `pinches()` rather than gating on them, and there is
  still no pre-SEW consumer (the cylinder-flank cap is convex ⇒ `pinches().is_empty()` holds, asserted
  in the unit test, but the driver does not *require* it). The `ShellReady` newtype stays SEW's to mint;
  C4 introduces no new checker (pure wiring over the proven `ledge_dom_certified` + CAP-OUT-LINK).
  *2026-08-08 · still deferred(→M5/SEW) · `closure::ledge`*

- **Front-half geometry is trusted — add per-pair D24 intersection certificates.** The
  arrangement checkers (`certify_core::arrange`) read only the *combinatorial* certificate — indices,
  labels, cyclic orders — never coordinates, so the geometric front-half (`carrier`/`decompose`/
  `membership`/`classify`/`spine`) is *trusted* (differentially validated vs CGAL + property tests, not
  checker-certified; vv-guide §6 "trusted front-half"). A carrier-solver bug — a dropped intersection, a
  wrong point, a misclassified coincidence — can yield a self-consistent DCEL that passes every checker.
  Honest stamp today: "combinatorially self-consistent, geometry differentially validated." Fix: emit
  per-pair certificates (discriminant sign, exhaustive candidate count, carrier residuals, interval-
  membership decisions) for line/line, line/circle, circle/circle — cheap exact D24 algebra — and check
  them. README/AGENT reconciled to state this scope (batch 2). **This is debt-sprint item 8** (with the
  multi-component gauge anchor #9b and the folded `CoincEdge`/`CoincSet` deletion) — a genuine
  *geometric-checker* slice, not a quick fix: it needs independence from the solver (re-derive the
  discriminant/residuals over the output, don't re-solve), binding the per-pair evidence to the input edges
  (the #6-`CertifiedChart` transplant risk), and a re-verifiable per-component point-location for the
  gauge. Given its own **focused design pass** rather than rushed at the tail of the sprint; a natural fold
  into Milestone C, where `closure` builds the CAP-IN-D24 census + per-pair geometry and SEW-LINK needs the
  gauge-anchored labels anyway. *2026-08-06 · deferred(→ own pass / C) · spine.rs, carrier.rs, witness.rs*

- **`CertifiedChart` digest-binding — the *remaining* (persistence-only) half.** The in-memory
  claim/evidence binding is **done** (batch 2b): `CertifiedChart::certify` now re-derives the checked
  quantities (`|q|²`, `|n′|²`, det J at the `(μ,w)` box corners) from `chart + domain` via
  `regularity_targets`, recomputes the tag, verifies the evidence (Sturm chains + margins) against those
  derived targets, and stores the domain — so a certificate built for one chart cannot be attached to
  another (the chains fail to verify), and a margin is qualified by its domain. `CapOut` never had the
  transplant problem (it wraps the region `ledge_dom_certified` just checked, not independent args). What
  *remains* deferred is only cross-boundary integrity: binding a verdict to a canonical **digest** of its
  claim so it can't be transplanted across a serialize/deserialize boundary, and retaining the certificate
  for offline re-checking — meaningful only once a persistence path exists (there is none today; building
  it now = inventing a serialization format speculatively, the `ShellReady` YAGNI). `kappa_cap` also rides
  on `CertifiedChart` as searcher-derived, uncertified data (documented). *2026-08-06 · deferred ·
  `geom::record`*

- **CLIP ladder coverage — common-zero census DONE; μ-coverage + fiber-census remain.** *Census done
  (debt-sprint item 6, `60e890e`):* `ZeroCensus`/`census_ok` — `clip()` now certifies the per-zero path
  only if the supplied zeros are the complete isolated-root set of `b²+d²` (independently re-counted;
  disjoint, σ-ordered, one-per-interval), closing the omit-an-awkward-zero hole. *Remaining (deferred with
  rationale):* **(a)** μ-subspan coverage of the CLIP-W failing set `{R_W ≤ 0}` — sound-ly relating the
  failing region (whose boundaries are *irrational* R_W roots) to the searcher's *rational* μ-spans, with
  open/closed boundary handling under half-open Sturm counts, is genuinely hard, and there is **no CLIP
  searcher** yet to validate against (all CLIP certs are hand-built fixtures; the producer is M4/closure).
  Shipping an unvalidated coverage checker in the sprint meant to *fix* coverage was judged too risky.
  **(b)** the `trim_local`/`clip_dom` sign-event fiber census (needs the chart-domain sign-event
  polynomial). Both best done alongside C's searcher. *2026-08-06 · deferred(→C searcher) · spec §8.5 CLIP*
  - **Producer landed (C3, milestone-c).** `closure::trim` is the missing CLIP producer: it builds
    `b_J` and the retained-side field `G_i = (C_i − x₀)·b_i` as three σ-rational coefficients
    (`g0`, `g_mu = ∂_μG`, `g_w = ∂_wG`) and drives all three reused checkers from a real joint —
    `clip_w_cert`/`clip_mu_cert` (the cleared `g_w²`/`g_mu²` `reg_q` gauges), `trim_local_cert` (outer
    corners + one confinement fiber), `sigma_deriv_corners` (the signed CLIP-σ leaf), and
    `field.corners` → `clip_dom` (the fiber census). The 90° cylinder self-fold certifies TRIM-LOCAL +
    CLIP-W end-to-end, so the checkers are no longer only hand-built fixtures. **Still deferred as
    *searcher-completeness* refinements** (the checkers are sound regardless; this is about the searcher
    *automatically* supplying complete inputs): **(a)** deriving the CLIP-μ failing sub-spans from the
    *irrational* `R_W` roots (the caller currently supplies sub-spans), and **(b)** Sturm-isolating the
    fiber sign-event σ's rather than sampling representative stations. *2026-08-08 · producer done,
    coverage-completeness deferred · spec §8.5 CLIP*

- **Multi-component cocycle gauge — the release-silent defaults are DONE; the gauge anchor remains (→8b).**
  *Done (debt-sprint item 2, `116ef78`):* `slab_locate` no longer silently defaults on the certified
  path — an incomplete slab decomposition or an unassigned cycle is now an explicit
  `CapOutFault::Incomplete` (the release-gone `debug_assert!` genericity check and the
  `unwrap_or((false,false))`/`(0,0)` defaults no longer sit on the certified route). *Remaining (the
  deeper half):* `cocycle_ok` pins the ℤ₂² gauge only in the seed's connected dual-component — for a
  disconnected dual graph (disjoint operands; holes, where one region is bounded by several edge-disjoint
  cycles) every other component can be uniformly XOR-shifted and still satisfy all edge equations, so its
  absolute labels come from *point-location* (trusted), uncertified — a point-location bug on a disjoint
  component would pass certification. Fix: per-component anchoring the checker re-verifies — **item 8b**,
  tied to the per-pair certificate work. *2026-08-06 · deferred(→8b) · boolean.rs, arrange.rs*

- **`link_iso` — permutation guard DONE; the unbounded (N>4) proof remains a frontier.** *Done
  (debt-sprint item 3, `4b94a53`):* `link_iso_ok` now validates its own precondition — a `has_duplicate`
  in-range/no-duplicates guard rejects the non-permutation inputs the Kani harness had only *assumed*;
  the Aeneas-lifted Lean model was regenerated and re-audited axiom-clean. *Remaining:*
  `link_iso_matches_cyclic_adjacency` still proves only length-4 permutations (vv-matrix labels the cell
  "N=4"); degree-6 vertices are property-tested, but the unbounded statement wants a Lean induction (the
  `link_ok`/pinch harness, by contrast, is already N=6). Research frontier. *2026-08-06 ·
  deferred(→frontier) · proof.rs, arrange.rs*

- **Coincidence lattice — `CoincSet` edge-list is dead; deletion folds into 8a.** Verified (debt sprint,
  item 4): `coincide` is *load-bearing* — its `touches` become `Coincident` incidences (`spine.rs:77`) that
  seed the overlap-boundary vertices `Dcel::build`'s step-3 merge depends on, so the **live merge is correct
  for *partial* overlap** (proven by the new `boolean_over_partially_overlapping_edges` fixture — two
  horizontally-offset rectangles, ∪/∩/△). Only the `CoincEdge`/`CoincSet` **edge-list** is dead (dropped as
  `_coinc`; `CoincOutcome` is `usize` counts, decoupled from `CoincEdge`). Its physical removal (`event.rs`
  `CoincEdge`/`Operand`, `spine.rs` `CoincSet` + `arrange_events` return, the randomized
  `coincident_edges_match_cgal` differential) **folds into item 8a's `PairWitness` rework** — one coherent
  witness change rather than double-churn. *2026-08-06 · deferred(→8a) · spine.rs, coincide.rs, difftest*

- **Differential-fuzz — harness + real fuzz run DONE (`differential-fuzz` branch); one wiring follow-up.**
  Op-chain differential (`crates/lattice/src/ratfuzz.rs`: `dashu` ≡ the *proven* `RefBackend` over
  size-bucketed operands + metamorphic mul identities) closes the two gaps the old single-op
  `rat::differential` had (no op-chains; i128-only ≤2-limb seeds ⇒ dashu never left schoolbook). **Done:**
  **(1)** seed buckets pinned to dashu-int 0.4.3's real mul thresholds — schoolbook ≤24 / Karatsuba 25–96 /
  Toom-3 97–4000 / NTT >4000 limbs (dispatch keys on the *smaller* operand, `mul/mod.rs`), straddled ±1.
  **(2)** seed corpus via `fuzz`'s `gen_corpus` bin (authoritative encoder `ratfuzz::corpus_seeds()`; 7 seeds
  across the thresholds) + a **real `cargo fuzz run`** — 2652–3118 coverage-guided runs, clean. **Key
  mechanism:** the fuzz build enables dashu's `tuning` feature (`fuzzing = ["dashu/tuning"]`) and the target
  lowers the thresholds via env vars (SIMPLE=2/KARATSUBA=16/NTT=160) so tiny operands route through
  Karatsuba/Toom-3/**NTT** at oracle-cheap sizes (no need for 4000-limb operands). **Finding (first run
  earned its keep):** thresholds MUST respect each algorithm's own `MIN_LEN` (Karatsuba 3, Toom-3 16) — my
  first values (KARATSUBA=6) routed 7–15-limb operands into Toom-3 and tripped *dashu's own*
  `assert!(b.len() >= MIN_LEN)`; not a dashu bug, a mis-config. **CI split (DONE):** per-PR = the
  *deterministic replay* (stable, no libFuzzer) — `replay_seed_corpus` unit test (in `nextest`) + the
  `fuzz regression replay` step (`cargo test -p lattice --features fuzzing --test fuzz_replay`, replays the
  committed crash corpus under the fuzzer's tuning); nightly = the *coverage-guided search*
  (`.github/workflows/fuzz-nightly.yml`, cron + `workflow_dispatch`, cargo-fuzz on the runner's rustup with
  a cached/persisted corpus, uploads crash artifacts). **STATUS:** the nightly cron is unvalidated on a
  real runner (nightly + `rust-src` + cargo-fuzz provisioning, same rustup-outside-nix pattern as dylint) —
  **watch the first scheduled run**; and provision the nightly fenix-natively if we want it inside nix.
  (A rational op-chain variant stays deferred — RefBackend's bit-serial `divrem`/`gcd` are too slow as a
  big-operand oracle; use metamorphic there.) *2026-08-06 · watching*

- **`RefBackend::int_from_le_bytes` must be proven if it ever leaves the test/fuzz harness.** It's a
  TEST/FUZZ-ONLY seed constructor (`#[cfg(any(test, feature = "fuzzing"))]`, banner on the fn), NOT a
  `Backend` trait method and NOT proven — its correctness is runtime-checked in the harness (seed
  byte-compared against dashu), never relied on for soundness. If it ever enters the `Backend` trait or
  any Aeneas-lifted / production path, it MUST first be proven `den(result) = value` in
  `certify-check/CertifyCheck/RefBackend.lean`, exactly like `from_i128` (`int_from_i128_eq`). The cfg
  gate keeps it physically out of the trait + the lift until then. *2026-08-06 · watching · `refbackend.rs`*

- **R.5 — finalize the algebra-trust rehaul (the `RefBackend = ℤ/ℚ` surface is DONE).** The whole reference
  `Backend` trait is now proven axiom-clean on `algebra-rehaul-r4` (`certify-check/CertifyCheck/RefBackend.lean`):
  RefNat = ℕ, RefInt = ℤ (ordered ring + gcd/lcm/divrem + i128 both directions), RefRat = ℚ (reduce + all
  arithmetic mul/div/add/sub + neg/numer/denom/is_zero/sign/cmp/from_ints/from_i128). Remaining is the V&V
  finalization: **(1)** promote the audit surface to a public `Backend`-instance corollary (the current
  `#print axioms` block lists the *private* op refinements; add a public theorem so `ci.yml`'s axiom-audit
  guards `RefBackend = ℤ/ℚ` at the trait level) + wire it into `.github/workflows/ci.yml`; **(2)** the dashu
  differential — make it a *proof-backed* oracle now that the reference is proven `= ℤ/ℚ` (`rat::differential`);
  **(3)** `vv-matrix.md` rows + `docs/algebra-trust.md` TCB update (dashu trust shrunk to the differential) +
  extraction-drift for the generated files; **(4)** merge-to-main review of `algebra-rehaul-r4`. Findings +
  the full method-by-method recipe in memory `algebra-rehaul.md`. *2026-08-05 · open*

- **Restore the `Backend` associated-type `Clone + Eq` bounds when Charon disambiguates
  trait parent-clauses.** The pinned Charon (`0.1.225`) lifts the `Backend` trait to a Lean
  `structure` whose parent-clause witnesses for *both* associated types (`type Int: Clone + Eq`
  and `type Rat: Clone + Eq`) are named identically (`corecloneCloneInst` / `corecmpEqInst`),
  so the structure has duplicate fields and does not typecheck — which blocks lifting any
  `Rat`-using checker (all `<B: Backend>`). Investigated exhaustively: no charon flag fixes it
  (`--remove-adt-clauses` targets ADTs not trait decls; `--exclude` leaves a dangling `sorry`
  and collapses the assoc-type lifting; `--hide-allocator` / `--opaque` variants keep the
  colliding structure). **Workaround (algebra-rehaul R.3):** dropped the `Clone + Eq` bounds
  from the trait's associated types (`Eq` was unused; `Clone` had exactly 4 call sites in
  `rat.rs`) and expressed clone as explicit `Backend::int_clone` / `rat_clone` methods (one impl,
  `Bignum`). This is contained and semantically inert, but it is a workaround: when Charon names
  those witnesses distinctly (check its releases past `0.1.225`), restore the associated-type
  bounds, delete `int_clone`/`rat_clone` + their impl, revert the 4 `rat.rs` sites to `.clone()`,
  and re-extract. Coordinated with a charon/aeneas pin bump (drags Lean/Mathlib). *2026-08-04 · open*

- **Make the dylint CI step fenix-native.** `cargo dylint`'s toolchain management is
  rustup-centric — it reads `lints/no_float/rust-toolchain` and runs that nightly *via rustup* —
  so the CI step (`.github/workflows/ci.yml`) runs on the **runner's rustup, outside nix** (like
  the Kani `cargo install` step) rather than a fenix-pinned toolchain. Functional and verified
  locally, but not consistent with the rest of the toolchain (fenix / `flake.nix`). Follow-up:
  supply `nightly-2026-05-28` + `rustc-dev`/`llvm-tools` via `fenix.toolchainOf` and run dylint
  inside `nix develop` — needs a rustup shim or dylint toolchain-env plumbing (the fiddly part
  I couldn't verify from the sandbox). Also: watch the first real CI run of the step.
  *2026-08-04 · open*

- **Pin `nixpkgs` off `nixos-unstable`.** `flake.nix` floats `nixos-unstable`, so a toolchain
  bump silently regresses CI with **no code change** — e.g. `gcc-15.3`/`glibc-2.42` emitting a
  `.debug_gdb_scripts` section that `rust-lld` rejected when linking the CGAL oracle shim (worked
  around with `.debug(false)` in `crates/difftest/build.rs`, commit `5276957`). Pin to a
  known-good rev (or a stable release) so CI is reproducible; the `.debug(false)` band-aid can
  then be reconsidered. Revisit floating-vs-pinned deliberately.
  *2026-08-04 · open*

## Tech debt / sketchy

- **The mesh κ-cap corner-min is representation-conditional.** It is sound only while `R₁`
  is affine/monotone in the chart parameters. Before promoting it to a *general* κ-cap
  checker (milestone C — the petal flank, where `R₁` is neither), Sturm-isolate the
  curvature extrema over the domain, or carry a certified convexity/monotonicity rider. This
  is the same class as the CLIP-σ / `corner_range` convexity rider. The cone golden is
  hand-verified, not unsound.
  *2026-08-04 · open · `fixtures::devices::certified_cone`, `certify1d::corner_range`*

- **The invariant lints scan code/doc-comments only, not spec text.** `tuple-predicate` and
  `:= census` (now in `cargo xtask lint`) should also scan `spec/`, but that needs an
  allow-list for the frozen spec's own meta-discussion of the rules (it explains *why*
  "proportional" is banned, using the word), and the spec is frozen — so we can't sprinkle
  inline allow markers. Deferred until that allow-list is designed.
  *2026-08-04 · open · `xtask/src/main.rs`*

- **`certify_core.lattice.backend.Backend` qualification in the certify-core externals.** The
  `refbackend` lift (algebra-rehaul R.4b) adds a *concrete* `lattice.backend.Backend` (the trait,
  pulled in by `impl Backend for RefBackend`) to the **Lattice** model. The certify-core model
  independently carries an *opaque* `Backend` (bound to ℚ), which — because Aeneas wraps a crate's
  model in `namespace <crate>` — is named `certify_core.lattice.backend.Backend`. The two coexist
  fine (different namespaces), but the hand-written `open certify_core` files (`CertifyCore/
  FunsExternal.lean`, `CertifyCheck/ClipSigma.lean`) referenced `Backend` *bare*, and bare now
  resolves to the Lattice model's concrete one (exact global match beats an `open`) instead of the
  intended opaque one. Worked around by fully-qualifying those references to
  `certify_core.lattice.backend.Backend`. This is explicit-and-correct but couples the hand-written
  externals to Aeneas's namespace-wrapping convention (stable at the pins; drift-checked). Cleaner
  long-term options if it ever bites: (a) extract `refbackend` into its own Lean lib so `Backend`
  never enters the shared `Lattice` model, or (b) move `impl Backend for RefBackend` to a sibling
  Rust module excluded from the `crate::refbackend` start-from (needs `pub(crate)` on the `RefInt`/
  `RefRat`/`RefNat` internals it touches).
  *2026-08-04 · open · `certify-check/CertifyCore/FunsExternal.lean`, `CertifyCheck/ClipSigma.lean`*

## Findings

- **DEV.2a committed fmt-unclean — a false-green the DEV.2b gate caught.** At DEV.2b start, `cargo fmt --all --check` (run to a real exit code across the whole workspace) failed on four hunks that had already landed in the DEV.2a commit (`3e18c61`): long single-line `if`/`assert!`/struct-literal expressions in `crates/lattice/src/rat.rs` (the `floor`/`ceil` fast path) and `crates/export/src/mesh3d.rs` (the corroboration test) that rustfmt wraps. Pure whitespace, no semantic change — but it means the DEV.2a "full gate green (fmt …)" claim did **not** actually run `fmt --all --check` to a checked exit (the exact [[verify-green-rigorously]] failure mode: a `| tail`-masked or scoped fmt invocation reads green while the real check fails). Fixed in a dedicated `fmt:` commit (`5a652b8`) reformatting only those two files, so the DEV.2b commit's diff stays clean and the workspace fmt gate is honestly green. Lesson reinforced: the fmt gate is `--all --check` with `EXIT=$?`, never a filtered or per-crate run. *2026-08-10 · resolved (`5a652b8`) · branch `dev-go-gate`*

- **D4.3 scope: the σ-band free-boundary form is forced by a missing polynomial-composition primitive — and it is the tractable, spec-sanctioned one.** The spec's substrate free boundary (`spec §3.4:151`) is a **σ-band with rational μ-boundary splines** `μ⁻(σ), μ⁺(σ)` over `[σ_lo, σ_hi]`, *not* an arbitrary planar outline `(σ(t), μ(t))`. This is not a shortcut — it is exactly what lets D4.3 close a solid with the existing machinery: every boundary rail is `c(σ) + μ±(σ)·r(σ) + w·n(σ)`, all functions of the **same** σ, so lifting is `Vec3Rat::scale` by a `RatFunc` (the direct generalization of `brep_slab`'s constant-μ `scale_rat`). A general `(σ(t), μ(t))` outline would need `c ∘ σ(t)` — **polynomial/RatFunc composition**, which the repo lacks (`geom::Chart::surface(μ, w)` takes a *scalar* μ; there is no `Poly::compose` / `RatFunc::compose`). So the general contour is deferred to a follow-on (composition primitive + an N-edge top/bottom cap subdivision), and D4.3 Stage 1 builds the σ-band. The **σ̂-monotonicity** arm of the ANCHOR checker is the one piece of the general-anchor obligation that is composition-*free* (it is purely about the σ-projection `σ̂(t)`), so it is implemented + refutation-tested now (a fold-back `σ̂` refuses) even though the σ-graph makes it trivial — real, forward-compatible verified code, not demo. *2026-08-09 · recorded (D4.3.0) · branch `d4.3`*

- **CM.4 finding (corrects a feared CM.1 gap): the full-`R` cofactor check certifies a reflection-mate cone miter — no branch-refinement needed for it.** A cone (rotating rulings) has a genuinely-rational, **non-affine** crease-line coordinate `ℓ(σ)` (measured `num` deg 1 / `den` deg 2 for `q=(1,σ,1,0)`, h=0, Π={x=1}), so the correspondence `R = ℓ_A(σ_A)=ℓ_B(σ_B)` **factors**: `(2,−1)` is an off-diagonal solution, so `R ≠ const·(σ_A−σ_B)`. The feared gap: CM.1's cofactor check `X == R·Q` uses the full `R`, so it needs the carrier `X = D_A×D_B` to vanish on the spurious branch too, which for *two different* cones it wouldn't. **But for a reflection-mate (shared-apex) miter it DOES** — empirically `X(2,−1)=0`: two rulings of *one* cone meeting `L` at a shared point pass through {apex, point} and are therefore the **same line** ⇒ parallel ⇒ `X=0`. So the full `R` divides `X` (shared-apex, structural), and `miter_fit_transverse` handles the cone's carrier identity as-is. **Consequence:** the branch-refinement I feared (searcher-supplied `R_φ`) is *not* required for the achievable cone fixture; it is needed only for exotic two-different-cut-family (different-apex) miters with a nontrivial `φ_J` — deferred/documented. Verified by `closure::miter::a_cone_transverse_cut_family_certifies_through_the_full_r` + the new `transverse_cut_family` searcher. **RESOLVED (CM.4a): the adversarial case IS reachable and now certified.** The adversarial config is **two cones over a shared base conic from different apexes** — realized as the unit circle's tangent-line families at `t = σ` vs `t = 2σ`: `ℓ_A = (1+σ²)/(1−σ²)`, `ℓ_B = (1+4σ²)/(1−4σ²)`, `D_A = (2σ, σ²−1)`, `D_B = (4σ, 4σ²−1)`. `ℓ` is degree-2 ⇒ `R = 2(σ_A−2σ_B)(σ_A+2σ_B)` factors; `X_carrier` vanishes on the real branch `σ_A = 2σ_B` (same tangent) but **not** on the spurious `σ_A = −2σ_B` (the *other* tangent through the shared L-point) — so the full `R ∤ X` (verified: `x_carrier.div_exact(R_full) == None`) and CM.1's full-`R` check refuses, while `R_φ = σ_A − 2σ_B` divides `X` and certifies. **Built:** `certify_core::miter::TransverseBranch` + the branch-aware `miter_fit_transverse` (verifies `R_φ·C == R` by multiplication, `R_φ` single-valued/degree-1-in-σ_B, `R_φ` vanishes at the `ε_φ`-paired support corners — which rejects the spurious branch — then discharges `X == R_φ·Q`), and `lattice::Biv::div_exact` (the searcher's cofactor tool). Test `a_two_cone_adversarial_miter_certifies_via_the_branch`: full-`R` → `CarrierMismatch`, branch → `Verified`. **Honest scope:** the adversarial cut families are built from the conic tangent-line geometry directly (genuine geometry); the searcher-from-`Chart` link for the *adversarial pair* is not established (the arbitrary-apex-cone chart inverse problem is unsolved), though the single-cone `transverse_cut_family` searcher is validated separately. *2026-08-09 · CM.4a done · branch `curved-miter-fit`*

- **CM.2 finding: "conic carriers so a cone's cut image passes CAP-IN-D24" is unsound as framed — skipped.** Traced every `Carrier` consumer: it is used **only** by CAP-IN-D24 → the LEDGE arrangement, and the clean-miter path (CM.1 `miter_fit_transverse`) never touches `Carrier` — spec §5.3 makes the transverse cut faces **ruled by straight lines in Π** (`F_i = P_i + μ·D_i`), parametrized rationally in σ, which is exactly the `ℓ_i`/`D_i` data CM.1 already handles. A conic is **not D24 content** (D24 = lines + circular arcs, spec §6); CAP-IN-D24 refusing it is **correct** — `cap_in.rs:19` "the cone is correctly turned away" ("*falsely, not vacuously*" = a genuine-false predicate, i.e. the conic really lies on no line/circle, not an erroneous refusal). Two confirmations it would be unsound to "make it pass": (1) the `closure::ledge` bridge already declines even a **Circle** (`Carrier::Circle → LedgeError::UnsupportedCarrier`, `ledge.rs:70-72`) — so CAP-IN-D24 licensing a carrier never means the line/circle-only arrangement can build it; (2) `arrange2d` handles line/circle *intersection points* (degree-2, one radical), not conic **curves** — no conic arrangement exists. So a `Conic` that passed CAP-IN-D24 would license non-D24 content into an engine that cannot arrange it. Genuine conic support is the deferred conic-**arrangement** L3 (spec §484), the LEDGE branch — orthogonal to the clean-miter thrust. **Decision (with the user):** skip CM.2, keep the plan's `Conic`-carrier idea filed under the conic-arrangement L3, and proceed to CM.3 (`AlgReal` wiring). *2026-08-09 · skipped; conic support → conic-arrangement L3 · branch `curved-miter-fit`*

- **D4.2 finding: `one_joint()` cannot close into a two-flank solid — a *fixture* obstruction, not a code gap.** Two exhaustive surveys (the trim / Π-cut machinery; the V_∂ / CAP-OUT cap machinery) established two independent obstructions: (1) the **2:1 ruling-speed overhang** — flank A's crease spans `x∈[−2,2]`, B's `x∈[−1,1]` (`|r|=2` at σ=0 vs `1` at σ=1) — leaves free tips, and equalizing them needs the **irrational** station `σ=√2−1`, unavailable to a rational crease; (2) a single joint's **substrate boundary is honestly open** ("no contour to feed"; closing sidewalls need an anchored contour — atlas-scale). Even a real `V_∂`-projected cut cap would fix only the *cap↔flank* seam, leaving those *flank↔flank* overhang tips + the open substrate free. Geometry verified symbolically: the crossing locus `w*(σ) = −g0/g_w = 2σ(1+σ)/(1−2σ−σ²)` is **rational** (not algebraic — no `AlgReal` needed to *emit* `flank∩Π`), and the neutral sheet `w=0` is **regular** (`ψ'=0` for the cylinder ⇒ `det J = |n'|²(1+w) > 0`); but the flanks meet only along the crease **line** `L` (`w=0`, on Π), which sits on the certificate's **trim boundary** (`G=0`, not the strict-retained `G>0`) — so there is no 2D shared interface to make a closed manifold. **Decision (with the user):** do not dodge with a symmetric demo fixture; build the machinery the closure genuinely needs — the curved Π-cut miter — as the **Curved MITER-FIT** milestone (To do above; `docs/vv-guide.md`). *2026-08-09 · recorded; pivot to Curved MITER-FIT · branch `curved-miter-fit`*

- **D4.1: a curved flank slab's `Vec3Rat` surface MUST be reduced before the Bézier/`f64` cast, or
  OCCT segfaults.** `geom::Chart::surface(μ,w) = c.add(&r.scale_rat(μ)).add(&n.scale_rat(w))` uses
  `Vec3Rat::add`, which *multiplies* denominators (not lcm) — so `c + μr + wn` piles up a common factor
  and the μ-wall rational reaches **degree ~18**. Converting that to a rational Bézier produces enormous
  Bernstein coefficients that overflow to `±∞` on the exact→`f64` cast; `new Geom_BezierSurface` (or
  `Geom_BSplineSurface`) with `±∞` poles then **crashes with SIGSEGV** (below the C++ try/catch, so no
  error string). Fix: added `lattice::Vec3Rat::reduce()` (common-gcd cancellation, value-preserving,
  keeps the shared-denominator form) and build the slab's σ-rails as reduced `base_j + w_k·n` — degree
  drops to ~4, poles finite, OCCT happy. Reducing `base` and `n` first also keeps a μ-wall's two
  `w`-rails sharing one denominator (the shared-weights condition the ruled patch needs to be exact).
  Diagnosed by stderr-marker bisection (lldb has no `debugserver` in the nix shell). *2026-08-09 ·
  resolved (`aaea9c0`) · `crates/lattice/src/ratfunc.rs`, `crates/export/src/brep_build.rs`*

- **D4.1: emit a rational patch as `Geom_BezierSurface`, not `Geom_BSplineSurface`.** Even at the
  reduced degree 4 with finite, sane poles, `new Geom_BSplineSurface(poles, weights, uknots, vknots,
  umults, vmults, udeg, vdeg)` **segfaulted** in the constructor (the knot/multiplicity relation checked
  out — `Σmults = nPoles + degree + 1` — but something in the clamped-single-span setup faulted). A
  single-span rational Bézier patch is exactly `Geom_BezierSurface(poles, weights)` — no knots to
  author — and it builds, faces, and passes `BRepCheck` cleanly. The rational-Bézier *edge* path still
  uses `Geom_BSplineCurve` (works); only the surface switched. *2026-08-09 · resolved (`aaea9c0`) ·
  `crates/export/src/occt_shim.cc`*

- **The three-way `BRepCheck` box: for the one-joint fixture an exact LEDGE cap face cannot join a valid
  shell — so we don't emit one (M-D D3.3, Option B).** The exact §10 LEDGE body ships only the two
  certified flank sheets (identical to MITER) and emits **no exact cap face**; the cap survives only in
  the §11 mesh path. Why: the sole cap outline `export` has is the CAP-IN-D24 **licensing square** — a
  placeholder, not the real `V_∂`-projected cut — and for this fixture its crease edge coincides with
  the certified A+B miter seam `M` (the crease line `L`). A cap face can therefore meet the flanks only
  along `L`, and every way of expressing that in OCCT was empirically pinned to a dead end with minimal
  diagnostic breps (OCCT 7.9.3, `BRepCheck_Analyzer`): **(1)** share the crease **edge** → `M` becomes
  3-incident → **non-manifold**; **(2)** share only the crease **vertex** → a cone-point junction →
  `BRepCheck`-invalid; **(3)** share **nothing** inside one shell → disconnected shell →
  `BRepCheck`-invalid (`BRepCheck_NotConnected`). So a single 3-face shell for this fixture *cannot* be
  `brepcheck_valid` — a topological fact, not a bug. The certificate (`CLOSURE_VALID`:
  SEW-EDGES/SEW-LINK/CAP-OUT-LINK) proves seam-local, honest-open facts, **not** a valid
  closed/connected solid; the friction surfaces in `export` because that is where the abstract
  certificate becomes concrete OCCT-checked coordinates, and there is genuinely no certificate-backed
  flank↔cap seam to emit while the cap is the placeholder square. *Decision (with the user):* export
  whatever geometry is certificate-backed rather than fabricate a seam to get a good-looking STEP
  ("oracle ∧ audit, never oracle-instead"); the exact cap is **deferred to the `V_∂` real-cut slice**,
  which projects the true cut and gives a seam identity can share. The prior-session plan to emit a
  "vertex-shared exact planar cap" is retracted (outcome 2 above). *2026-08-09 · decided ·
  `crates/export/src/brep_build.rs`, `docs/vv-guide.md §8`*

- **Mesh LEDGE cap fanned unordered `face.outer` edges → degenerate cap, hidden by a false-green.**
  `shell::cap_tris` fanned `edge_start` of each `CapOut::region().faces[].outer` edge, assuming the
  boundary arrived loop-ordered head-to-tail. The arrangement (`arrange2d`) stores those edges
  **unordered** and with mixed orientation: the D24 cap square arrives as four segments whose starts
  are (2,0), (0,2), (0,0), (0,0). Fanning the starts alone dropped the (2,2) corner and doubled (0,0),
  so the cap covered only its lower-left half-triangle plus one zero-area triangle — which OCCT's
  BRepCheck rejects (`brepcheck_valid=false`). Fixed by `shell::ordered_ring`, which walks the edges
  through their shared endpoints (matching either orientation) into the true corner loop before
  `cap_tris` fans it. The bug was **latent, not a D3.2b regression** (the same degenerate triangle
  reproduces on the pre-flip fixture). *Process finding:* it went uncaught because the `--features
  step` leg — `occt_audits_the_one_joint_shell`, `one_joint_ledge_writes_a_reloadable_step_shell`,
  `differential::ledge_oracle`, all of which assert `brepcheck_valid` — was **never run with a real
  exit code** when M-D.2 and D3.2a were called green (a "step 28/28"-style claim that had not actually
  executed). Carried forward as a standing-gate requirement for all remaining D3.x phases: the
  `--features step` nextest leg must be in every green check with `${PIPESTATUS[0]}` verified.
  *2026-08-09 · resolved · `crates/export/src/shell.rs` (`ordered_ring`; commit `8ee76a4`)*

- **OCCT STEP-export shim de-risk (thin-M6 GO/NO-GO) — GO.** The `export` crate's off-by-default
  `step` feature builds a `cxx` shim to OpenCASCADE's `STEPControl_Writer`; the M6.0 smoke writes a
  unit box, reads it back, and `BRepCheck`s the reload — green under `nix develop` (OCCT 7.9.3, 350
  STEP entities). Two darwin gotchas worth remembering: (1) OCCT nests its headers in
  `<occt>/include/opencascade/` and they `#include` each other **unqualified**, so that dir must be on
  the include path *directly* (the nix cc-wrapper only injects `<occt>/include`); `build.rs` derives it
  from the `-isystem` token in `NIX_CFLAGS_COMPILE` (or `OCCT_INCLUDE_DIR`). (2) The STEP reader/writer
  moved to `libTKDESTEP` in OCCT 7.8+ (was `libTKSTEP`). The **stdlib-ABI question** resolved the
  opposite way from a naïve read: OCCT is prebuilt *libc++*, but the shim + the `cxx` runtime compile
  with the nix-default g++/**libstdc++** (forcing `clang++` fails — the shim's libc++ `rust::String`
  ctors don't match the libstdc++ cxx runtime). It links anyway because the shim only crosses the OCCT
  boundary via `const char*`/`double`/OCCT types — no `std::` object crosses — so libc++ and libstdc++
  coexist at load time. *Caveat carried to M6.3:* an OCCT exception (`Standard_Failure`, a libc++
  `std::exception`) caught by the libstdc++ `catch` in the shim is technically cross-ABI; fine for
  valid geometry, but the M6.3 writer should prefer status codes / validity pre-checks over relying on
  the catch for malformed input. *2026-08-08 · resolved · `crates/export/{build.rs,src/occt_shim.cc,src/step.rs}`*

- **CGAL boolean oracle extended to segment (polygon) operands — the C4 LEDGE cap lane.** The
  `cgal_boolean_*` shim only parsed disk operands (`C cx cy r2 operand`), so the LEDGE cap — a
  straight-edge *polygon* (cylinder rulings + crease) — had no CGAL region differential. Added an
  `L x1 y1 x2 y2 operand` boundary-edge line: per operand the edges accumulate into a CCW list and
  build a `Gps_circle_segment_traits_2::Polygon_2` via **direct** `X_monotone_curve_2(source, target)`
  construction (the traits' *linear*-segment ctor takes `Kernel::Point_2`, **not** the traits
  `Point_2` — that was the compile fight). Direct construction is required over `make_x_monotone`
  per-edge: the latter sorts each segment left-to-right and would flip right-to-left boundary edges,
  breaking the loop. Two live gotchas: CGAL **hard-aborts** (`SIGABRT`, uncatchable) on a non-simple
  polygon (a self-intersecting "bowtie" quad triggered it — the differential inputs must be verified
  simple + CCW), and axis-aligned (vertical) edges hit CGAL's vertical-segment special case, so the
  `ledge_cap_region_matches_cgal_polygon_boolean` cases use generic convex + simple-concave quads with
  no axis-aligned edge. Face count **and** exact `a+b√d` boundary geometry now match CGAL for the C4
  cap. *2026-08-08 · resolved · `difftest::cgal_shim` `boolean_components`*

- **Developable ≠ constant curvature.** A cone's nonzero principal radius is `R₁ = ρ·tan β`
  (ρ = slant distance from the apex) — *not* constant; only the cylinder has constant `κ₁`.
  This is why the mesh κ-cap is the domain minimum (the tightest radius, nearest the apex),
  not a value read off a fixed parameter station. A one-line property test caught the wrong
  "cone ⇒ symmetric ⇒ constant radius" assumption.
  *2026-08-04 · watching · `fixtures::devices::cone_principal_radius_shrinks_along_sigma`*

- **An exact cone development is transcendental — so the flat↔rolled morph is diagnostics-only.**
  The device cone's azimuth is `φ(σ) = 2·arctan σ − 90°` and its half-angle β is constant with
  `sin β` irrational (`n·ẑ ≡ 65/97`), so the isometric unrolling (flat angle `θ = sin β · Δφ`,
  flat radius = apex distance) lands outside ℚ. A *certified* development therefore cannot live
  in the rational kernel; it is future `develop`-layer work (M7). The viewer's morph is an honest
  `f64` unrolling in `export::mesh3d::develop_cone` (apex at the origin since `c ≡ 0`; the flat
  angle accumulates the true 3D angle between successive rulings = the directrix arc length on the
  unit sphere, which reduces to `sin β · Δφ` here). Empirically the certified strip develops to a
  **60.3° annular sector** (`= sin β · 90°`), and radius is preserved to machine epsilon. The
  development is float — it never touches a predicate, so it stays inside spec invariant 1.
  *2026-08-08 · watching · `export::mesh3d::flat_development_is_isometric_along_rulings`*

- **A genuine plane is not a `Chart` — the closure vertical slice is cylinder-first, not plane-first.**
  C0 recon (M4/closure) found the approved plan's "two-planar-flank" / `plane()` assumption false at the
  representation level: the spec (§, line 81) distinguishes a **`strip`** span (`|n′| > 0`) from a
  **`planar`** span (`n′ ≡ 0`, a coefficient identity), and `geom::chart::Chart` implements only the
  *strip* case — `Chart::new` debug-asserts `|n′|² ≢ 0` (`chart.rs:106`) and its whole field calculus
  divides by `|n′|²`. A plane (constant normal) has `n′ ≡ 0`, so it cannot be a `Chart`, and `geom` has
  no planar-span type. **Resolution:** the **cylinder** is the representable developable whose ruling
  cut-edges are straight *lines* (so CAP-IN-D24 passes and both closure branches run) *and* it carries a
  moving normal for the regularity bundle — so the M4 slice is built on it, with the **cone** as the
  contrasting conic class. The genuine planar-hub §13 petal disk waits on the planar-span type.
  *2026-08-08 · finding · `docs/closure-scoping.md §8`, `docs/vv-guide.md §8` (M4)*

- **EDGE-OCCUPANCY is asymmetric across the two closure branches — the LEDGE side recomputes it, no
  new `arrange2d` private surface.** M5.0 recon: the MITER branch carries the four-bit occupancy + frame
  bit *first-class* (`certify_core::miter::LedgerEdge.occupancy`, minted at M4), so its constructor
  (MITER-REGION-IDENTITY) just reads it. The LEDGE branch does **not**: the emitted `arrange2d::boolean::
  CapOut`/`Region` carry only `source: CurveId` + `orient` per edge; the transverse occupancy lives
  transiently in `boolean.rs` (`sector_mask`/`edge_flips` are private). **Resolution for ARRANGEMENT-BITS
  (adopted):** recompute the four bits in the `sew` searcher from the **public** surface — `label_cells`
  → `CellLabeling { labels, adj, seed }` (`boolean.rs:242`) + `separating_ids` (`:997`) — the spec's
  "projection of the §6 cell labels, four lookups". No `arrange2d` visibility change is needed for the
  cylinder slice; a minimal public `CapOut` accessor is added only if the recompute later proves to need
  the DCEL directly. The occupancy packet stays a `sew`-searcher product; `certify_core::sew` consumes it
  origin-agnostic. *2026-08-08 · finding · `crates/sew/src/*`, `docs/vv-guide.md §8` (M5)*

## Deferred (by milestone)

- **DEV / Tier-C — the transcendental ANCHOR backward-error bound (its own milestone, M-E).** D4.3 Stage 1
  certifies the **exact** part of ANCHOR (`spec:372`: positive width, boundary regularity, σ̂-monotonicity —
  all Sturm) and closes the exact-over-anchor solid. The **transcendental** part — the backward-error bound
  `sup|D(â) − g| ≤ ε` (fidelity of the developed boundary `D` to an authored flat drawing `g`) via the
  development map `D = γ + μ̂·ρ·e(ψ)`, where `ψ = ∫ψ′` (→ arctan/log), `γ = ∫e(ψ)` (a nested transcendental),
  `ρ = |n′|` (a radical) — plus the DRC `ε < clearance/2` (`spec:402`) need a rigorous
  transcendental-enclosure tier (interval/Taylor-model `ψ`/`γ`/`D` bounds). That is a whole new tier, gated on
  its own GO, **not** a D4.3 slice. **Priority note (driving requirement, see `docs/implementation-plan-v1.md §6`):
  this is HALF THE PRODUCT, not a fidelity nicety — the certified flat↔3D development is the keystone *both*
  product directions pivot on (① develop 3D→flat generates the flat PCB outline; ② fold flat ECAD→3D). Weight it
  as a primary thread (co-equal with the D4.4 atlas), not a tail deferral.** *2026-08-09 · deferred(→DEV / M-E),
  reprioritized 2026-08-10 · `spec:372`,`spec:402`, `develop` crate, `certify_core::free_boundary`*

- **General `(σ(t), μ(t))` authored outlines + the polynomial-composition primitive.** D4.3 Stage 1 builds the
  **σ-band** free boundary (`μ⁻(σ), μ⁺(σ)`, all functions of the same σ). An arbitrary planar outline needs
  `c ∘ σ(t)` — a `Poly`/`RatFunc` **composition** primitive the repo lacks — plus an N-edge top/bottom cap
  subdivision (the σ-band's 2 `Plane` σ-caps become an N-gon fan). Build the composition primitive when the
  general contour is genuinely needed (atlas-scale authored petals), not before. See the D4.3 finding above.
  *2026-08-09 · deferred(→D4.3 general-contour follow-on) · `lattice::{Poly,RatFunc}`, `export::brep_build`*

- **STEP export of a *certified-closed* body as a `CLOSED_SHELL` / `MANIFOLD_SOLID_BREP`.** D4.1's
  `write_brep` emits the closed slab as a surface model with an `OPEN_SHELL` (the C++ builder never
  stamps the `TopoDS_Shell` `Closed` flag), even though `closed_shell` certifies it closed and OCCT
  agrees `free_edges == 0`. Since the certificate is the authority, a caller holding
  `Verified(ClosedShell)` may legitimately stamp `shell.Closed(true)` + wrap in a solid so the STEP
  declares it closed — gated on that certificate so honest-open MITER/LEDGE bodies stay `OPEN_SHELL`.
  Do this the next time STEP emission is touched, not as its own slice.
  *2026-08-09 · deferred (next STEP-emission pass) · `crates/export/src/{occt_shim.cc,step.rs}`*

- **The planar-span representation (`n′ ≡ 0`) — a `PlanarChart` / relaxed `Chart` with its own
  pedal/ruling calculus.** A `geom`/M1-adjacent feature; unblocks the *genuine* §13 planar-hub petal disk
  (the closure slice uses the cylinder as the representable line-carrier stand-in meanwhile). See the
  finding above.
  *2026-08-08 · deferred(→M-C petal pass / M1-adjacent) · `crates/geom/src/chart.rs`*

- **Petal conical-flank fixture + the `cx-cone-flank-trim-mu` corpus entry.** Spec §13
  geometry is not yet pinned; needed for closure/sew.
  *2026-08-04 · deferred(→M-C) · `fixtures/corpus.md`*

- **Algebra-trust rehaul.** Opaque `Int=ℤ` / `Rat=ℚ`, a reference bignum, its Lean
  equivalence proof, and a dashu differential stress-test.
  *2026-08-04 · deferred(post-B) · `docs/algebra-trust.md`*

- **SLAB-S1 / QPOS Bernstein positivity.** No Bernstein primitive yet.
  *2026-08-04 · deferred(→M4) · vv-guide §8 (B deferrals)*

- **FRESH promotion (three-way containment re-test) — deferred out of thin M6.** FRESH keys on the
  fab-gating stamp fields (`materialStripWidth`): a regenerated enclosure ⊆ stamp ⇒ green, disjoint ⇒
  stale hard-fail, partial ⇒ undecided (spec:203). That is a `VALID_material` / material-grade concern,
  not `VALID_solid-closure` — the thin-M6 gate proves solid-closure only. The M6.2 certificate store
  ships a documented FRESH *stub* (the provenance chain rule it enforces — a stamp bounded below by its
  sources' certified enclosures, never a naked float — is the FRESH precondition, but the re-test
  itself is not built). *2026-08-08 · deferred(→M-E material grade) · spec:203, `certify_core::gate` / `gate`*

- **The full EDGE-REG verdict logic + EDGE-EMB / EDGE-EDGE (embeddedness).** The Milestone-B
  version is only the `Pass | Fail | Stall` core plus `to_verdict`.
  *2026-08-04 · deferred(→M5/sew) · `certify1d::edge_reg`*

- **M0 `lattice` perf/robustness follow-ups.** Subresultant PRS (vs the naive resultant), a
  bivariate resultant, `AlgReal` refinement, proportional-lint softening.
  *2026-08-04 · deferred · M0 task-2 deferrals*

## Frontier (research-open)

- **Two open theorems, both tracked in the proof ledger.** Sturm's theorem is cited as an
  axiom (`sturm_root_count`, absent from Mathlib); CAP-OUT ⇒ 2-manifold-with-boundary is
  open. Runtime-checked hypotheses / bounded Kani cover soundness in the interim.
  *2026-08-04 · watching · `docs/proofs/ledger.md`*

## Resolved

- **Invariant 1 consolidated onto one type-aware dylint lint — the `xtask` token scan is gone.** *Done
  2026-08-08 (branch `no-float-dylint-only`):* invariant 1 (no floats in certified paths) was guarded by
  **two** overlapping lints — the `cargo xtask lint` `f32`/`f64` **token** scan and the `no_float` dylint
  **literal** check. The token scan was comment-blind (it false-positived on Phase-1 doc prose — `` `f64`
  cast ``, which prompted this) and couldn't tell lib code from test code, so it over-policed
  `#[cfg(test)]`/`tests/` and needed a `testgen.rs` carve-out that only existed to escape it. Fix: extend
  the dylint lint with `check_ty` (matches `Res::PrimTy(PrimTy::Float(_))`) so it now catches float
  **types** — fn sigs, fields, casts, generic args (`Vec<f64>`), and type-relative paths (`f64::EPSILON`,
  caught as a `Ty` node — no extra `check_expr` arm needed) — **and** the existing literal check; then
  delete `fn no_float`, its report call, its unit test, and the now-dead `contains_word`/`is_word_byte`
  helpers from `xtask`. The dylint lint is now the sole invariant-1 gate. UI fixture `ui/lattice.rs`
  exercises all nine cases (3 literals + fn param/return + field + generic + cast + assoc). *Decision —
  floats are allowed in tests:* the ban's real scope is the certified **predicate path** (AGENT.md inv 1;
  vv-guide §6), not test code. A test float can't reach a predicate (`cfg(test)` is compiled out), and
  floats are *useful* in tests — independent `f64` **oracles** (compute expected, assert the exact result
  matches — the highest-value float use in an exact kernel), input **generators**, readable expectations.
  Scoping to lib targets (`-p`, no `--all-targets`) is not a gap but the intended policy. *Accepted edge:*
  dylint doesn't lint **doctests**, so a float in a `///` example is unguarded — style, not soundness (a
  doctest float can't reach a certified predicate either). *Trade-off:* dylint is now the only gate, on the
  rustup-nightly CI leg; a broken dylint toolchain surfaces as **red CI** (loud, blocks merge — not a
  silent gap), and local float-checking now means running `cargo dylint`, not `cargo xtask lint` (noted in
  AGENT.md). *2026-08-08 · resolved · `lints/no_float/`, `xtask/src/main.rs`, `ci.yml`, AGENT.md inv 1,
  vv-guide §6*

- **Debt sprint — 7 of 8 review-batch items paid down (branch `debt-sprint`).** *Done 2026-08-06:*
  **(1)** structured `RegFault` splitting bad-paperwork from real degeneracy through the REG-Q family +
  `ChartFault` (`5834d0e`). **(2)** `slab_locate` no longer silently defaults an unassigned cycle /
  non-generic decomposition on the certified path — `CapOutFault::Incomplete` (`116ef78`). **(3)**
  `link_iso_ok` permutation guard (`has_duplicate`), with the Aeneas-lifted Lean model regenerated +
  `lake build` + axiom audit re-run clean (`4b94a53`); finding: Aeneas rejects `return` inside nested
  loops. **(4)** verified the *live* coincidence merge handles **partial** overlap (`coincide`'s touches
  seed the vertices; new fixture) — the dead `CoincEdge`/`CoincSet` deletion folds into item 8's witness
  rework (`abbdf7a`). **(5)** CAP-IN-D24 minimal totality guard `validate_d24` — `ledge_dom_certified` is
  now total over malformed input (`5ffbe34`). **(6, part 1)** CLIP-ladder **common-zero census**
  (`ZeroCensus`/`census_ok`) closing the omitted-zero hole (`60e890e`). **(7)** CAP-OUT **source-ID
  permutation bijection** replacing the coverage count (`boundary_bijection_ok`; `56accab`; vv-matrix
  🚧→✅). Each a full-workspace-green commit (nextest/doctests/fmt/clippy -D/xtask; #3 also lake+audit).
  **Deferred (judgment, with rationale):** item 6's **μ-coverage + fiber-census** — relating the CLIP-W
  failing region (irrational R_W roots) to rational μ-spans is hard to do *soundly* and there is no CLIP
  searcher to validate against; risky to ship an unvalidated coverage checker in the sprint meant to fix
  coverage. Best done with C's searcher (see the CLIP To-do entry). Item 8 (per-pair certs + gauge) is a
  genuine geometric-checker slice — given its own focused pass (see To-do). To-do reconciled
  (2026-08-07): the RegFault entry was removed (fully done); the CAP-OUT-bijection, CAP-IN
  totality-guard, slab_locate, and link_iso entries were rewritten to record what shipped vs. the
  genuine remainder (the two further bijections → 8, the gauge anchor → 8b, the unbounded link_iso
  proof → frontier).

- **`divrem` op refinement (algebra-rehaul R.4b.4).** *Done 2026-08-05:* `divrem_loop_spec` (the
  bit-serial restoring-division loop = Euclidean identity, `704196e`) + `divrem_eq` (the wrapper =
  `den self / den d`, `den self % den d`, `d26ba39`), both axiom-clean + CI-audited, full `lake
  build` green. New reusable lemmas: `nat_or_pow2_add`/`u64_or_pow2_add` (set-a-clear-bit = add),
  `den_head`. Strengthened `divrem_loop_spec`'s post with `Normalized result.2` (needed by the
  wrapper). Next: R.4b.5 `gcd`, R.4b.6 `RefInt`/`RefRat`→ℤ/ℚ.

- **Launder the bench build-artifact blobs out of git history.** *Done 2026-08-04:* rewrote
  `main` + `milestone-b` with `git filter-branch --index-filter` (filter-repo unavailable),
  stripping all 853 `benchmarks/two-tier-vs-dashu/target/` blobs — branch histories clean, the
  milestone merge + tree intact, `cargo build` green. Objects survive only via the
  `refs/original` backup + a bundle (won't be pushed; gc when convenient). The rewritten `main`
  still descends from `gho/main`, so the push was a clean fast-forward.

- **Push the laundered `main` to `gho`.** *Done 2026-08-04:* the remote fast-forwarded
  `c2a73c4 → 98005ea` (Milestone B + engineering-log + tidy Phases A/B), verified blob-free.

- **dylint `no_float` lint (float literals in certified paths).** *Done 2026-08-04:* a
  `rustc_private` `LateLintPass` (`lints/no_float`, pinned `nightly-2026-05-28`) flags float
  *literals* in `lattice`/`certify_core`/`arrange2d` — the type-aware complement to the
  `cargo xtask lint` token scan. Verified locally: compiles clean, the UI test fires on
  `1.5`/`2.0`/`3.0`, and the real certified crates are float-literal-free (it correctly ignores
  the `.1.0` tuple access at `boolean.rs:550`, where a text scan would false-flag `1.0`). CI
  wiring is written but pending a real-runner check (see To do).
