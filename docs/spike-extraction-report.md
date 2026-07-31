# §7 extraction spike — report & go/no-go

*Status: **COMPLETE — decision: GO** (task 3, `vv-guide §7`). Records which tool lifted more cleanly, real
proof effort, Mathlib coverage gaps, semantic-fidelity surprises, the per-checker template, and the go/no-go.
The representative lifted-model refinement is **fully proven** end-to-end and axiom-clean
(`sign_variations_spec`, §5/§8) — the extraction pipeline is demonstrated all the way from real
`crates/lattice` Rust to a machine-checked Lean proof.*

## 0. What the spike prices

The kernel's soundness plan (`vv-guide §0/§1/§4`) lifts `certify-core`'s pure checkers into Lean 4 and
proves them there. This is the highest-*variance* unknown, so we price it on one representative function —
the sign-variation counter (`crates/lattice/src/sturm.rs::sign_variations`) — plus the Sturm
hypothesis-checker (`SturmChain::verify_chain`), and emit a go/no-go + a template every later checker follows.

## 1. Resolved version pins (the `[spike]` anchors)

The pin chain is forced by `env-doc §3` (Aeneas → Lean → Mathlib). Resolved this session:

| Component | Pin | How determined |
|---|---|---|
| **Aeneas** | `3a8586facab25b31bdb1e1f5f45acd60d1cc5ff0` | recommended Lean backend |
| **Charon** | `527ea8e3b5dcb52edd6aef0f7bc34cc09c11dd59` | co-versioned pair — Aeneas's `flake.lock` |
| **Lean** | `leanprover/lean4:v4.31.0` | Aeneas `backends/lean/lean-toolchain` |
| **Mathlib** | tag `v4.31.0` (rev `fabf563a7c95a166b8d7b6efca11c8b4dc9d911f`) | matches Lean; **and** matches the Aeneas Lean package's own `require mathlib @ v4.31.0` — no conflict |
| **hax** | `5b0ba8be6da3c313fdfed1c19dd0f0721a29f4b3` (F★ `v2025.10.06` via its flake) | latest; see convergence finding below |
| **Kani** | TBD (Phase 4) | independent; brings its own rustc |

## 2. Headline findings so far

- **hax and Aeneas have converged (2026).** hax's *recommended* Lean backend is `cargo hax into lean`,
  which runs **through Charon + Aeneas**; the old direct transpiler is now `cargo hax into legacy-lean`.
  The `vv-guide §4` premise of "two independent Lean backends" is partly outdated. Consequence for **D1**
  ("lift both ways"): the meaningful comparison is **Aeneas (charon→aeneas→Lean)** — the real pipeline,
  which modern hax also wraps — versus **hax `legacy-lean`** (the direct route). Recorded; adapted.
- **No aarch64-darwin binary caches.** Neither the aeneas nor the hax flake declares a cachix substituter,
  so "native on the Mac" means **building Charon (Rust), Aeneas (OCaml), hax (Rust+OCaml) and F★ from
  source**. Determinate Nix puts `/nix` on its own APFS volume (ample free space), so this is a compute/time
  cost, not the boot-volume-disk risk first feared.
- **Mathlib has Descartes, not Sturm.** `Mathlib.Algebra.Polynomial.RuleOfSigns` provides
  `Polynomial.signVariations` (coefficient sign changes) and Descartes' rule, but **no Sturm's theorem**
  (the complete formalization is Isabelle/AFP, Eberl). This shapes **D2**: cite the deep theorem
  (BPR Thm 2.50 / Eberl), prove the reduction the runtime checker discharges. (Details in Phase 2.)

## 3. Native build feasibility (Phase 0) — ✅ **gate cleared**

All native on **aarch64-darwin** (macOS 26.3.1), from source (`/nix` is its own APFS volume,
ample space). Determinate Nix 3.8.2. **The env-doc §5 "Apple-Silicon OCaml stack" risk did not
materialize as a stopper** — everything built, with two minor workarounds noted below.

| Component | Native build | Notes |
|---|---|---|
| **Charon** `527ea8e3` | ✅ | brings a nightly rustc driver (`nightly-2026-06-01`) + a full-MIR sysroot |
| **Aeneas** `3a8586fa` | ✅ | OCaml + Jane Street `core` stack (bundles a co-versioned `charon`) |
| **hax** `5b0ba8be` | ✅ | Rust frontend + OCaml engine + F★ `v2025.10.06`; provides `cargo-hax` |
| **Lean v4.31.0** | ✅ | elan installed the darwin_aarch64 toolchain cleanly |
| **Mathlib v4.31.0** | ✅ | `lake exe cache get` → 6.3 GiB oleans, 8542 files (Azure cache) |
| **Aeneas Lean lib** | ✅ | `require aeneas` (backends/lean); the lifted model typechecks against it |

**End-to-end lifts, both routes** (first smoke-tested on a verbatim probe crate, then the committed
Aeneas artifact re-lifted **directly from `crates/lattice`**):
- **Aeneas:** `charon cargo --preset=aeneas --start-from crate::sturm::sign_variations` (run in
  `crates/lattice`; compiles dashu + lattice) → LLBC → `aeneas -backend lean -split-files` →
  `certify-check/Lattice/{Types,Funs}.lean` — typechecks against the Aeneas Lean lib; the generated
  `lattice.sturm.sign_variations` (its `Source` comments cite `crates/lattice/src/sturm.rs:26–38`) is
  **`sorry`-free**.
- **hax:** `cargo hax into legacy-lean` (on the probe) → `proofs/legacy-lean/extraction/lift_probe.lean`
  (a `RustM`/`core_models.fold` model). hax's `into lean` = the charon+aeneas pipeline (convergence, §2).

**Two workarounds (minor, documented):**
1. The darwin C/C++ toolchain (present for the CBMC/CGAL FFI) exports `DEVELOPER_DIR`/`SDKROOT`
   pointing at an SDK-only store path, which the nix `xcbuild` `xcrun` rejects, breaking `git`
   inside `lake`. Fix: **unset `DEVELOPER_DIR SDKROOT`** for lake invocations (Phase 4 folds this
   into the devShell/CI). A representative "OCaml-stack on Apple-Silicon" friction point.
2. `nix build --no-link` leaves no GC root, and Determinate Nix auto-GC reclaimed the tool outputs
   as `/nix` filled. Fix: GC-root the tools (flake inputs in Phase 4; `--out-link` in the interim).

**Charon semantic detail (feeds Phase 1):** the pretty-printed LLBC shows `v += 1` as
`checked.+ … assert(overflow) else panic` — so the Aeneas model of `sign_variations` carries fallible
(`Result`) semantics; the refinement proof must discharge "overflow never fires" (`v ≤ len − 1 < 2³²`).

**Devshell wrinkle found + fixed:** the darwin C/C++ toolchain (present for the CBMC/CGAL FFI) exports
`DEVELOPER_DIR`/`SDKROOT` pointing at an SDK-only store path, which the nix `xcbuild` `xcrun` rejects,
breaking `git` inside `lake`. Fix: **unset `DEVELOPER_DIR SDKROOT`** for lake invocations (Phase 4 folds
this into the devShell/CI). A representative "OCaml-stack on Apple-Silicon" friction point, exactly what
`env-doc §5` flagged — minor, not a stopper.

**Charon semantic detail (feeds Phase 1):** the pretty-printed LLBC shows `v += 1` as
`checked.+ … assert(overflow) else panic` — so the Aeneas model of `sign_variations` carries fallible
(`Result`) semantics; the Phase-1 proof must discharge "overflow never fires" (`v ≤ len − 1 < 2³²`).

## 4. Tool comparison — Aeneas vs hax `legacy-lean`

Both lifted the *verbatim* `sign_variations` from `crates/lattice/src/sturm.rs` and produced typechecking
Lean. They differ sharply in model shape and proof framework:

| | **Aeneas** (charon→aeneas; also what `hax into lean` wraps) | **hax `legacy-lean`** (direct) |
|---|---|---|
| Command | `charon cargo --preset=aeneas …` then `aeneas -backend lean …` | `cargo hax into legacy-lean` |
| Monad | `Result` (`Aeneas.Std`) | `RustM` (Hax prelude) |
| Loop model | `loop` combinator over a `ControlFlow` body + concrete slice iterator `IteratorSliceIter.next` | `core_models.iter…Iterator.fold` over the slice, `Tuple2` accumulator |
| Prelude dep | the `Aeneas` Lean lib (`require aeneas`, backends/lean) | the `Hax` prelude (`proof-libs/legacy-lean`) |
| Proof framework | `progress`/`step` + `loop.spec_decr_nat` (established) | Lean's newer `Std.Do`/`mvcgen` (`@[spec]`) |
| Maturity | **recommended**, mature Lean backend; emitted def is `sorry`-free | **experimental** (its own `--help` says so) |

**Which lifted more cleanly: Aeneas.** It is the recommended, more mature route; the model is idiomatic and
`sorry`-free at the definition; the proof idiom (`loop.spec_decr_nat` + invariant) is documented and stable.
hax `legacy-lean` works and is arguably a more literal transcription of the Rust surface, but it is flagged
experimental and targets the newer `mvcgen` framework. **Because `hax into lean` *is* the charon+aeneas
pipeline (the 2026 convergence), the practical recommendation is a single lift path — Aeneas — with hax
retained for its F★ backend and its ergonomic front-end, not as a second independent Lean route.**

## 5. Proof effort — *partial (the tool-independent proofs are done)*

- **`signVariations` spec equivalence** (`CertifyCheck/SignVariations.lean`, core Lean, no Mathlib):
  the mathematical spec `signVariations`, the streaming transcription `signVariationsImp` (mirrors the
  Rust), and `signVariationsImp_eq_signVariations` — proven by one generalized induction (`svAux_eq`,
  ~30 lines). **Axiom-clean**: `[propext, Classical.choice, Quot.sound]`. The one real proof is front-loaded
  and tool-independent, so it stands regardless of the lift.
- **Sturm hypothesis-checker** (`CertifyCheck/SturmChecker.lean`, Mathlib v4.31.0): `IsSturmChainData`
  formalizes exactly what `verify_chain` checks; `variationAt` reuses the proven `signVariations`;
  `sturm_root_count` states Sturm's theorem as **the single labelled cited axiom**; `verify_chain_sound`
  is the interface corollary. `#print axioms verify_chain_sound` = `[propext, sturm_root_count,
  Classical.choice, Quot.sound]` — soundness reduces to exactly one named citation, nothing hidden.
- **Aeneas-lifted model, integrated** (`CertifyCheck/LiftedAeneas.lean` + `Lattice/`): the generated
  `lattice.sturm.sign_variations` typechecks against the Aeneas Lean lib; `lifted_sign_variations_eq_loop`
  proves it reduces to its loop; `#print axioms` on the generated def = `[propext, Classical.choice,
  Quot.sound]` (**no `sorry`**).
- **Lifted-model refinement — PROVEN** (`CertifyCheck/Refine.lean`, `sign_variations_spec`). The end-to-end
  `lattice.sturm.sign_variations signs ⦃ r => r.val = signVariations (sliceInts signs) ⦄` is closed the
  *intended* Aeneas way: `Aeneas.Std.loop.spec_decr_nat` for the loop + the **`step` tactic** for the body
  (measure `len − i`; invariant `v + svAux last (drop i) = svAux 0 I`; `U32` overflow discharged by
  `len ≤ U32.max`), chained to the spec through `signVariationsImp_eq_signVariations`. **Axiom-clean**:
  `#print axioms sign_variations_spec` = `[propext, Classical.choice, Quot.sound]` — no `sorryAx`, and
  **off the Aeneas Std `get_unchecked` sorries**. So the Aeneas-lifted model of the real `crates/lattice`
  code is now *proven* to compute the mathematical spec — a fully end-to-end, sound Rust→Lean result.
- **What closing it took (the concrete per-checker cost + a semantic-fidelity finding):** the manual route
  (reduce the `Result` monad by hand) is a trap — unfolding `Bind.bind` breaks Lean's `instances`
  transparency. The framework's `step`/`loop.spec_decr_nat` tactics avoid that and are the right grain.
  The **one real gap**: Aeneas's library ships a `@[step]` `next` spec for `RangeIter`/`StepBy` but **not
  for the shared-slice iterator** (`IteratorSliceIter.next`) — so `step` couldn't drive a slice loop
  out of the box. We supplied that ~15-line spec (`sliceIter_next_spec`, mirroring
  `IteratorRange.next_'S_spec`); it's reusable and worth upstreaming to Aeneas.

**Effort headline:** the *hard, reusable* proof (algorithm ≡ mathematical spec) is small and clean (~30
lines, one induction). The *per-lift* refinement of the monadic model to the pure algorithm is ~120 lines
of `step`-driven Aeneas proof — mechanical once you (a) drive it with `step`/`loop.spec_decr_nat` rather
than reducing the monad by hand, and (b) have the iterator's `@[step]` `next` spec. Both amortize: the
tactics and the slice-`next` spec are one-time investments reused across `certify-core`.

## 6. Semantic-fidelity surprises

- **The Aeneas Lean std lib ships `sorry`s.** `Aeneas.Std.Slice` has two (`get_unchecked` and its spec) and
  `StringIter` two more. They are **confined to `get_unchecked` (unsafe indexing)** — *off* the
  iterator/loop path `sign_variations` uses, and the generated def is axiom-clean. But it is a real **TCB
  caveat**: a checker whose proof touches an unproven Aeneas lemma would silently inherit `sorryAx`. Policy:
  `#print axioms` every checker theorem in CI and reject `sorryAx`; upstream or locally prove any needed
  `sorry`'d lemma before relying on it.
- **Overflow is monadic.** `v += 1` lifts to a fallible `Result` (`v + 1#u32`); the model is faithful, and
  the proof must *prove* non-overflow rather than getting it for free — a small but real obligation per
  arithmetic op.
- **hax↔Aeneas convergence** (§2): "two independent Lean backends" (vv-guide §4) is outdated; `hax into
  lean` routes through Aeneas. Adapted: Aeneas is the one Lean route.
- **hax `legacy-lean` targets `Std.Do`/`mvcgen`**, a different (newer) proof framework than Aeneas's
  `progress`/`Spec`. Two lift styles ⇒ two proof ecosystems; committing to Aeneas avoids straddling both.
- **Namespace/layout quirks:** the Aeneas Lean namespace is the snake_case crate name + Rust module path
  (`lattice.sturm.sign_variations`), while the *module* names are PascalCase (`Lattice.Funs`); `-split-files`
  emits flat `Types.lean`/`Funs.lean` that must be placed under a `Lattice/` dir to match the import paths.
- **Build/host friction:** no aarch64-darwin binary cache (from-source OCaml/Rust), the
  `DEVELOPER_DIR`/`SDKROOT` `xcrun` breakage, and Determinate-Nix auto-GC of `--no-link` outputs (§3).

## 7. Per-checker template

The repeatable recipe every `certify-core` checker follows (validated on `sign_variations`):

1. **Write the checker** in `certify-core` as pure, panic-free, `no_std` Rust (the existing discipline).
2. **Lift** (Aeneas route):
   `charon cargo --preset=aeneas --start-from crate::<fn> --dest <dir>` →
   `aeneas -backend lean <crate>.llbc -dest <dir> -split-files -gen-lib-entry`.
   (Run with `DEVELOPER_DIR`/`SDKROOT` unset.)
3. **Register** the generated modules as a `lean_lib` and `require aeneas` (git @ the pinned rev,
   `subDir=backends/lean`) + `require mathlib` (git @ v4.31.0, matching Aeneas).
4. **Hand-write the Lean spec** — the mathematical definition — in a core-Lean module (add Mathlib only
   when the domain needs it, e.g. `Polynomial`). This *is* the certificate's formalization (vv-guide §4).
5. **Prove algorithm ≡ spec** (tool-independent, front-loaded): transcribe the Rust algorithm as a pure
   Lean function and prove it equals the spec. This is the reusable crux; usually a clean induction.
6. **Prove lifted ≡ algorithm**: `loop.spec_decr_nat` + invariant for loops (`progress`/`step` for
   straight-line code); chain to the spec. Amortize with shared iterator/loop lemmas.
7. **Audit axioms**: `#print axioms <thm>` — accept only `[propext, Classical.choice, Quot.sound]` plus any
   *deliberate, labelled* cited axiom; reject `sorryAx` (guards against Aeneas-Std `sorry`s).
8. **Runtime-checked hypotheses** (Sturm-style): formalize the checked conditions as a predicate, state the
   deep theorem as a single cited `axiom`, and add a `proofs/ledger.md` entry (statement · citation ·
   hypotheses-checked-at-runtime vs structural · the axiom name in the checker's footprint).

## 8. Go / no-go — **GO**

The extraction approach is viable; adopt it. Evidence:
- The full **Rust → Charon → Aeneas → Lean** pipeline **builds and runs natively on the target platform**
  and lifts real `certify-core` code to `sorry`-free Lean models that typecheck against the Aeneas lib.
- The **reusable proof crux** (algorithm ≡ mathematical spec) is small, clean, and **axiom-clean**.
- The **runtime-checked-hypothesis pattern** (the project's main proof-effort reducer) works exactly as
  designed: the Sturm checker's soundness reduces to **one labelled cited axiom**, with the Mathlib gap
  (no Sturm, only Descartes) handled by citation + reduction — no hidden assumptions.
- A concrete, validated **per-checker template** (§7) exists.

**Adopt with these standing controls** (none blocking):
1. **`#print axioms` gate in CI** — reject `sorryAx`; the Aeneas Std lib's `get_unchecked` `sorry`s make
   this non-optional.
2. **Invest once** in shared loop/iterator refinement lemmas + a `progress`-driven tactic to amortize the
   per-lift cost (§5).
3. **One Lean route (Aeneas)**; keep hax for F★ / front-end ergonomics, not as a second Lean backend.
4. **Build cost**: no darwin binary cache ⇒ from-source OCaml/Rust; publish a cache or run the lift on
   x86_64-linux in CI if local build time bites.
5. Reproducibility is "pinned manifest + Mathlib cache," not bit-pure (env-doc §3) — accepted.

**The `vv-guide §7` fallback (hand-transcribed models + heavy Kani/property/differential) is NOT needed** —
end-to-end extraction works and the representative checker (`sign_variations`) is proven all the way from
real `crates/lattice` Rust to Lean (`sign_variations_spec`, axiom-clean). Phase 5 applies the same,
now-validated, `step`/`loop.spec_decr_nat` template to the remaining checkers (gcd/reduce,
`verify_common_factor`).
