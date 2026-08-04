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

- **Push the laundered `main` to `gho`.** Deferred until the tidy sweep lands, so we push one
  clean state. The history rewrite is done (see Resolved); the push is a clean fast-forward.
  *2026-08-04 · open*

## Tech debt / sketchy

- **The mesh κ-cap corner-min is representation-conditional.** It is sound only while `R₁`
  is affine/monotone in the chart parameters. Before promoting it to a *general* κ-cap
  checker (milestone C — the petal flank, where `R₁` is neither), Sturm-isolate the
  curvature extrema over the domain, or carry a certified convexity/monotonicity rider. This
  is the same class as the CLIP-σ / `corner_range` convexity rider. The cone golden is
  hand-verified, not unsound.
  *2026-08-04 · open · `fixtures::devices::certified_cone`, `certify1d::corner_range`*

- **The code-invariant lints scan code only.** `no_repr_leak`, `tuple-predicate`, and the
  `:= census` each carry a stated TODO to also scan spec text; today they cover only code and
  doc-comments.
  *2026-08-04 · open · `scripts/lint/*.sh`*

## Findings

- **Developable ≠ constant curvature.** A cone's nonzero principal radius is `R₁ = ρ·tan β`
  (ρ = slant distance from the apex) — *not* constant; only the cylinder has constant `κ₁`.
  This is why the mesh κ-cap is the domain minimum (the tightest radius, nearest the apex),
  not a value read off a fixed parameter station. A one-line property test caught the wrong
  "cone ⇒ symmetric ⇒ constant radius" assumption.
  *2026-08-04 · watching · `fixtures::devices::cone_principal_radius_shrinks_along_sigma`*

## Deferred (by milestone)

- **Petal conical-flank fixture + the `cx-cone-flank-trim-mu` corpus entry.** Spec §13
  geometry is not yet pinned; needed for closure/sew.
  *2026-08-04 · deferred(→M-C) · `fixtures/corpus.md`*

- **Algebra-trust rehaul.** Opaque `Int=ℤ` / `Rat=ℚ`, a reference bignum, its Lean
  equivalence proof, and a dashu differential stress-test.
  *2026-08-04 · deferred(post-B) · `docs/algebra-trust.md`*

- **SLAB-S1 / QPOS Bernstein positivity.** No Bernstein primitive yet.
  *2026-08-04 · deferred(→M4) · vv-guide §8 (B deferrals)*

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

- **Launder the bench build-artifact blobs out of git history.** *Done 2026-08-04:* rewrote
  `main` + `milestone-b` with `git filter-branch --index-filter` (filter-repo unavailable),
  stripping all 853 `benchmarks/two-tier-vs-dashu/target/` blobs — branch histories clean, the
  milestone merge + tree intact, `cargo build` green. Objects survive only via the
  `refs/original` backup + a bundle (won't be pushed; gc when convenient). The rewritten `main`
  still descends from `gho/main`, so the pending push is a clean fast-forward.
