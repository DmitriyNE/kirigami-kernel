# Environment & crate layout — resolved engineering decisions

Status: decided 2026-07-30 · authoritative for the repo skeleton (M0 task 1). Resolves the engineering-layer gaps the handoff left open (crate-layout reconciliation, toolchain/edition pins, reproducible environment). **The spec `flex-substrate-rep-spec-v0.24-full.md` is untouched by this document** — this is build/structure scaffolding, not math. On any conflict about *geometry or certificates*, the spec still wins; on *repo shape and tooling*, this file is the record.

These choices refine, and where noted supersede, the crate-list phrasing in `implementation-plan-v1.md §1` and `AGENT.md` (task queue + toolchain). Version numbers marked **[spike]** are anchors to be locked by the `vv-guide §7` extraction spike, not authorities.

---

## 1. Crate layout — layered pure-core / imperative-shell

The 11-crate domain list (`implementation-plan §1`) and the pure-core/shell split (`vv-guide §1`) are orthogonal cuts of the same code. They are reconciled as **layers**: checkers are extracted *up* into the pure tier; searchers/constructors/stores stay in the shell tier. This makes invariant 4 literal and gives hax/Aeneas a single crate to lift.

```
PURE TIER — no_std · panic-free · total · the TCB + the Rust→Lean extraction surface
  lattice        L0 fixed-limb fast path + BigInt slow path + polynomial arithmetic
                 · Sturm (isolation + sign-on-interval) · bivariate resultants
                 no_std + alloc · Kani: fast≡slow bridge, promotion-trigger, panic-freedom
  certify-core   Verdict{Verified|Refuted|Unresolved} · Evidence/Witness/Margin · MarginSq
                 + EVERY pure checker, organized in modules by domain:
                   certify_core::certify1d  CLIP ladder (incl. CLIP-σ signed disjunction),
                                            REG/SLAB determinant forms, corner min/max
                                            (declared min-or-max per convexity rider),
                                            EDGE-REG three-way verdict, REPARAM (pure fn)
                   certify_core::arrange    ℤ₂² cocycle check, CAP-OUT bijections,
                                            Link_emitted ≅ Link_geometric predicate,
                                            occupancy→cell-bit projection
                   certify_core::sew        occupancy→row classifier, SEW-LINK compare,
                                            EDGE-EMB/EDGE-EDGE verdict logic, ε_φ order sign
                   certify_core::gate       VALID / CLOSURE-CAP verdict propagation
                                            (pure enum algebra), unresolved-propagation
                 depends: lattice  ·  THE single hax / Aeneas / Lean target

SHELL TIER — the "kernel-search" role · std · tested + differentially checked, never trusted
  geom       charts, hatted stall calculus (J_raw = p̂·Ĵ as a tested identity)   → lattice, certify-core
  arrange2d  DCEL, canonical decomposition, event spine, 8-step boolean         → certify-core, geom
  closure    fan/collar, Q-clip on b_J, MITER-FIT resultant search              → + arrange2d
  sew        EDGE-OCCUPANCY construction, sewing, ledger materialization        → + closure
  gate       certificate store (append-only, provenance, FRESH), evaluation     → certify-core, sew
  develop    D map, γ ODE per tag, flat booleans (reuses arrange2d), folds       → certify-core, geom
  export     STEP / mesh / marks (floats ONLY here, behind `diagnostics` flag)  → all above
  fixtures   the two device instances + corpus loader                          → geom, arrange2d, closure, sew
  difftest   CGAL Arrangement_2 + OpenCascade oracles (C++ FFI; never certified) → fixtures

LEAN
  certify-check/   lake project — hax + Aeneas lifted models + hand-written Lean specs.
                   This is AGENT.md's "certify-check target for Lean." Not a cargo crate.
```

**Resolved deviations from the literal handoff crate list (approved 2026-07-30):**

- **`certify1d` is absorbed into `certify_core::certify1d`** — it is ~entirely pure checker material, so a standalone shell crate would be near-empty. It is a *module*, not a top-level crate.
- **`certify-core` is a single real crate**, not a facade over per-domain `-core` crates. It is the one extraction surface (`vv-guide §1`: "This crate is the deductive-verification surface").
- The `arrange2d` / `sew` / `gate` domain crates keep only their imperative searcher / constructor / store code; their checkers live in the corresponding `certify_core::*` module.
- `lattice` stays a separate pure crate *below* `certify-core` (its own two-backend benchmark and Kani story warrant it); both `lattice` and `certify-core` are in the pure/no_std TCB.

**`no_std` handling (approved).** `certify-core` and `lattice` are hard `#![no_std]` (invariant 4) with explicit `extern crate alloc`. The bignum backend does **not** get to relax this: `lattice` exposes its integer/rational as **newtypes over a backend trait**, so the `no_std + alloc` requirement lives at the `lattice` API and the `malachite`-vs-`num-rational` benchmark winner is swappable behind it. **`no_std + alloc` compatibility is therefore a hard gate in the M0 backend benchmark** — added to the yardstick, which as written measured only Sturm speed on degree-12 over 256-bit rationals.

---

## 2. Edition / MSRV / toolchains

The binding reality: **Kani, hax, and Charon/Aeneas each pin their own toolchain**, and they generally cannot be forced onto one compiler. Policy therefore: the pure crates stay on the stable-language subset so they build under whatever toolchain each tool brings; we pin *tool versions*, not one universal compiler.

| Component | Decision | Status |
|---|---|---|
| **Edition** | **2024**, all crates | firm — hax and Charon ingest edition-2024 Rust; no reason to carry a 2021 constraint |
| **MSRV floor** | **1.85** (the edition-2024 stabilization release) | firm |
| **Dev / CI toolchain** | one pinned stable via `rust-toolchain.toml` (anchor: latest stable ≥ 1.85 at scaffold time) + components `rust-src`, `clippy`, `rustfmt`; managed by fenix | anchor **[spike-adjacent]** |
| **MSRV contract** | this is a kernel, not a crates.io library — MSRV = the pinned dev toolchain; set `workspace.package.rust-version = "1.85"` and treat CI's pin as the real contract | firm policy |
| **Pure-crate constraint** | `certify-core` + `lattice` use only stable-language features (no tool-specific nightly features), `#![no_std]` + `extern crate alloc`, `#![forbid(unsafe_code)]` | firm |
| **Kani** | latest release, pinned by version; brings its own toolchain via `cargo kani` | **[spike]** |
| **hax** | pinned git tag (pre-1.0, fast-moving) | **[spike]** |
| **Charon + Aeneas** | pinned as a **co-versioned pair** (Charon is the rustc frontend, Aeneas consumes its output — the two revs must match) | **[spike]** |

The `[spike]` versions are locked by the `vv-guide §7` extraction spike, whose exit deliverable already includes "which tool lifted more cleanly, proof effort, Mathlib coverage gaps, semantic-fidelity surprises." This document owns the *pinning scaffold*; the spike fills the exact revs into `flake.lock` and the toolchain files.

---

## 3. Lean 4 + Mathlib

- Lean pinned via a `lean-toolchain` file (`leanprover/lean4:v4.xx.0`); Mathlib pinned via a committed `lake-manifest.json` at a matching rev.
- **The Lean version is downstream of Aeneas, not chosen freely** — it must match whatever the pinned Aeneas release's Lean backend (`aeneas` / `Base` lib) targets. Order of resolution: pick the Aeneas rev → it dictates Lean → Mathlib follows Lean.
- Use `lake exe cache get` for Mathlib's prebuilt oleans (building Mathlib from source is hours). Consequence, stated honestly: Lean/Mathlib reproducibility is "pinned manifest + Mathlib cache," which is as reproducible as the Lean ecosystem currently gets — not bit-pure the way the Rust side is.
- Lean version + Mathlib rev are **[spike]** — locked once the Aeneas pair is chosen.

---

## 4. Nix flake (reproducible environment)

`flake.nix` at repo root; `flake.lock` committed; `.envrc` (`use flake`) for direnv auto-load.

- **Inputs (all rev-pinned via `flake.lock`):**
  - `nixpkgs` — CGAL, `opencascade-occt`, `cmake`, `pkg-config`, C++ toolchain (for the `difftest` shim), `elan`.
  - **`fenix`** (nix-community) — exact Rust toolchain pinning incl. `rust-src` for `no_std` / Kani. *(chosen over oxalica/rust-overlay; approved.)*
  - `hax`, `charon`, `aeneas` — as their own pinned flake inputs (each ships a flake); compose their binaries into the shell. This is the reproducible route since they are not reliably in nixpkgs. **Fallback** if any proves flaky to build: vendor its release binary in the shell — less pure, still pinned.
- **`devShells.default`** exposes: pinned rustc/cargo + components, `cargo-kani`, `cargo-fuzz`, `elan` + `lake` (Lean managed from `lean-toolchain`), `hax` / `charon` / `aeneas`, CGAL + OpenCascade + build tooling.
- **CI parity:** CI runs *inside* this devShell (Determinate Systems nix installer + a nix cache action) so "CI == local" is literally true — matters because CI gates on the corpus, Kani harnesses, and the `:=` / tuple-predicate greps (`vv-guide §6`).
- **Caveats stated up front:** (1) Lean/Mathlib purity is partial (§3); (2) the hax/Aeneas OCaml stack on Apple Silicon is validated by ordinary dev use, not a dedicated harness (§5).

---

## 5. Platform support

- **First-class targets: `aarch64-darwin` and `x86_64-linux`.** Both listed in the flake's `systems`.
- **No dedicated Apple-Silicon smoke test.** Any darwin-specific breakage in the OCaml-based hax/Aeneas stack will surface early in normal dev flow; if it does and is not cheaply fixable, the fallback is to run the extraction toolchain on `x86_64-linux` (CI already covers that target). Decision rationale: a bespoke smoke harness is not worth its upkeep versus early organic discovery.

---

## 6. Deferred to the extraction spike (`vv-guide §7`)

Locked once the spike runs; tracked here so they are not mistaken for oversights:

- Exact Kani / hax / Charon+Aeneas revs and the Lean + Mathlib pins (§2, §3).
- Whether edition 2024 needs any pure-crate accommodation for the Charon/hax frontends (expected: none, since the pure code avoids nightly/edition-sensitive features).
- Which of hax vs Aeneas lifts `certify-core` more cleanly, and the resulting per-checker template (the spike's stated exit criterion).
- The M0 bignum backend winner under the extended criteria (speed **and** `no_std + alloc`), locked behind the `lattice` backend trait (§1).
