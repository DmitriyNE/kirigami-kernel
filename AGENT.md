# AGENT.md — start here

You are a coding agent picking up a project with no prior context. This file is your entry point. Read it fully before touching code. It is short on purpose; the depth is in `docs/`.

## What this is

**Kirigami** — a **certified-exact geometry kernel** in Rust for flexible-PCB flat↔3D correspondence — the representation behind a smart contact-lens flex assembly (a 4-layer 240 µm board rolled into a cone). "Certified-exact" is the whole point: no floating point in any result that carries a certificate; every geometric claim ships with a checkable proof object; every predicate is exact rational/algebraic arithmetic.

The mathematics was specified and adversarially reviewed across 24 spec revisions before any code. That review is done. **Your job is implementation, not redesign.** The spec is `spec/flex-substrate-rep-spec-v0.24-full.md` and it is the sole normative authority. If code and spec disagree, the spec wins; if the spec seems wrong, stop and flag it — do not silently "fix" it (twenty-four rounds of people cleverer about this than a fresh read will be have already passed over it).

## Read order (do not skip)

1. **`docs/agent-glossary.md`** — the 30 terms and acronyms you must know to parse the spec (REG-V, CLIP-DOM, MITER-FIT, EDGE-OCCUPANCY, V_cand/V_∂, the lattice tiers, ...). Read this first or the spec is noise.
2. **`docs/implementation-plan-v1.md`** — module decomposition, dependency order, milestones. This is your map. Then **`docs/environment-and-crate-layout.md`** — the resolved crate layout, toolchain/edition pins, Lean/Mathlib, and Nix flake; read it before you scaffold anything.
3. **`docs/vv-guide.md`** — the verification & validation architecture. **Non-negotiable; it constrains how you write every function.** The core idea: verified *checkers*, tested *searchers*, a hard pure-core/imperative-shell split.
4. **`fixtures/corpus.md`** — ~30 counterexamples with required verdicts. These are your regression suite from commit one. Each is a real bug a real reviewer found; reproducing its verdict is how you know a module works.
5. **`spec/flex-substrate-rep-spec-v0.24-full.md`** — the spec itself. Dense. §2 symbol table, §3 charts/domains, §4 development, §5 folds/closures, §6 the arrangement kernel, §8 the certificate tables. Read the section you're implementing; do not try to read it all at once.
6. **`spec/flex-substrate-rep-spec-v0.24-delta.md`** and `spec/spec-pending-v025.md` — the most recent changelog and the queued profile edits (canonical arc decomposition — implement it in the arrangement from the start). `docs/paper.md` is a gentler narrative overview if the spec is too terse.

## The invariants — violating any of these is a defect, not a style choice

1. **No floats in certified paths.** Floats live behind a `diagnostics` feature flag, for plots and viewers only. A float that reaches a predicate is a bug.
2. **Three-valued verdicts, always.** `Verified(Evidence) | Refuted(Witness) | Unresolved(Margin)`. Never a bare `bool` for a geometric decision. A result whose checker cannot run is `Unresolved`, never `Verified`.
3. **Certifying algorithms.** Constructors (searchers) return `(claim, certificate)`. Checkers are separate, pure, and verify the claim from the certificate. The searcher is never trusted; its output always flows through its checker. This is the soundness architecture — see vv-guide §0–1.
4. **Pure-core / imperative-shell.** `certify-core` = checkers + their algebra: pure, total, panic-free, `no_std`, the Lean-extraction surface. `kernel-search` = the clever imperative code: only tested + differentially checked. Keep the boundary clean; it is simultaneously the TCB boundary and the extraction boundary.
5. **Two-tier lattice.** L0 fixed-limb fast path (Kani-verified) + BigInt slow path (the semantic reference, matches Lean's `Int`/`Rat`). Never inline raw bignum ops elsewhere; go through `lattice`.
6. **Proof types are enums; every demand stratum has a constructor.** An uninhabited obligation must be a compile error, not a runtime surprise. (This mechanically catches an entire class of bug the review kept finding — see the batch-19–21 delta entries.)
7. **Squared-margin convention.** Separation margins on √-carrying quantities are declared in squared form (`MarginSq`). Do not compare a value against an unsquared margin it was cleared against.
8. **The corpus verdicts are assertions.** If a change makes a corpus fixture stop reproducing its verdict, you broke something; the fixture is right.

## Toolchain

- **Rust, edition 2024** (MSRV floor 1.85). `certify-core` + `lattice` are hard `#![no_std]` + `alloc`; the bignum backend sits behind a `lattice` trait so `no_std` lives at the API, not the backend. Bignum: benchmark `malachite` vs `num-rational` at M0 — winner must be **no_std + alloc** *and* fast on the yardstick (`lattice` doc). No `unsafe`. Environment is pinned via a **Nix flake** (fenix-managed toolchain). **Crate layout, edition/tool pins, Lean/Mathlib, and the flake are all resolved in `docs/environment-and-crate-layout.md` — read it before scaffolding.**
- **Kani** (bounded model checking): the lattice fast≡slow bridge, finite combinatorial functions, bounded DCEL bookkeeping. See vv-guide §5.
- **Lean 4 + Mathlib** (deductive): the certificate theorems, via **Rust→Lean** lifting (hax for pure parts, Aeneas for locally-mutable parts — direction is Rust→Lean, there is no Lean→Rust codegen). See vv-guide §4. **Run the §7 spike before committing the extraction approach.**
- **proptest / cargo-fuzz**: stratum-weighted generators (degenerate-heavy — the bugs live on degenerate strata). **CGAL** and **OpenCascade** are differential oracles in `difftest/`, never in a certified path.
- CI runs: the corpus, property tests, Kani harnesses, the `:=` census + tuple-predicate greps over `spec/` and doc-comments, and the milestone-gate matrix check (vv-guide §6).

## Current task queue

Milestone 0, in order. **Tasks 1–3 are complete; current work is task 4 (M3a).** In a fresh clone, run `nix develop` first (it locks `flake.lock` and fetches `rust-src`), then confirm green with `cargo check --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, and `bash scripts/lint/run-all.sh`. The Lean proof surface builds with `cd certify-check && lake exe cache get && lake build` (unset `DEVELOPER_DIR`/`SDKROOT` on darwin); `nix develop .#extraction` adds the hax/charon/aeneas binaries for re-generating lifted models.

1. ✅ **DONE (`4c4ca7b`) — Repo skeleton**: cargo workspace with crate stubs per the resolved layered layout in `docs/environment-and-crate-layout.md §1` — pure tier `lattice` + `certify-core` (`certify1d` is absorbed as `certify_core::certify1d`, not its own crate); shell tier `geom arrange2d closure sew gate develop export fixtures difftest`; Lean in `certify-check/` (a lake project). Add `flake.nix` / `flake.lock`, `rust-toolchain.toml` (edition 2024, fenix), and `lean-toolchain`. Copy `docs/`, `fixtures/`, and create `vv-matrix.md` and `proofs/ledger.md` as stubs. `spec/` holds v0.24-full + delta + pending-v025.
2. ✅ **DONE — M0 lattice.** *Slice 1*: backend chosen by benchmark (**dashu**; passes the hard `no_std`+`alloc` gate, ties malachite for fastest — `docs/lattice-backend-benchmark.md`) behind `lattice::backend::Backend`; two-tier `Int`/`Rat` L0 fast path + promotion + exact `cmp`/`sign`/`gcd`, differentially checked (fast≡slow + a 2nd backend) and Kani-verified panic-free. *Slice 2*: exact division, ℚ[x] polynomial arithmetic, **Sturm** (isolation + sign-on-interval; `verify_chain` runtime-checked, Kani `sign_variations`), **resultants** (numeric Euclidean + bivariate Bareiss; `verify_common_factor`), and **total** algebraic-number comparison — L2 `Surd` (a+b√d) + general L3 `AlgReal` (defining poly + isolating interval). gcd/reduce correctness is owned by **Lean at the §7 spike** (the settled tool-fit decision — Kani keeps the gcd-free bridge + panic-freedom; no algorithm bandage). Acceptance: `vv-guide §8`.
3. ✅ **DONE — the extraction spike (vv-guide §7). Decision: GO** (`docs/spike-extraction-report.md`). Charon/Aeneas/hax + Lean v4.31.0/Mathlib v4.31.0 build natively on aarch64-darwin; `sign_variations` lifts both ways (Aeneas — the recommended route, which `hax into lean` now wraps — and hax `legacy-lean`); its spec-equivalence proof is **axiom-clean**; the Sturm hypothesis-checker is formalized with the theorem as a **single cited axiom** (Mathlib has Descartes, not Sturm); the Aeneas-lifted model is integrated + typechecks; pins are locked in `flake.lock` + `certify-check/{lean-toolchain,lakefile.toml,lake-manifest.json}`, with `nix develop .#extraction` for the binaries. The representative lifted-model refinement is **fully proven** end-to-end and axiom-clean (`certify-check/CertifyCheck/Refine.lean`, `sign_variations_spec` — `loop.spec_decr_nat` + the `step` tactic; needed the missing `@[step]` slice-`next` spec). **Phase 5 (gated apply) is now complete**, all axiom-clean and in the CI `#print axioms` gate: `gcd_u128_spec` (the CBMC-intractable 128-bit Euclidean loop = `Nat.gcd`, `GcdReduce.lean`), `reduce_spec` (`SmallRat::reduce` canonical over the full `i128` range, `Reduce.lean`; its Std-gap `core` models are the only hand-written TCB surface, `Lattice/FunsExternal.lean`), and `verify_common_factor_sound` (`Resultant.lean` — proven with **no cited axiom**, since Mathlib's `resultant_eq_zero_iff` closed the resultant⇔common-root gap).
4. ⟵ **CURRENT** — M3a (arrangement decomposition + event spine) — every arrangement test needs it — with the CGAL difftest shim wired from the start.

Write each milestone's **acceptance criteria into vv-guide §8 before implementing it.**

## How to work

- Small commits, each with the corpus/property test that proves it. TDD against the corpus is the default.
- When you implement a certificate, write its checker and its Lean spec together; they define each other.
- When you're unsure what the spec means, the delta files explain *why* each rule exists (each is a fixed bug) — that usually disambiguates. If still unclear, flag it rather than guessing.
- Do not add a geometric feature the spec defers to §14 (curved closures, collapse operators, conic arrangements, COMPSOLID). Those are labeled backlog; v1 rejects-to-band where they'd apply, and that is correct.
- Keep `vv-matrix.md` current: a row per certificate/operation, a cell per verification method. CI gates on it.
