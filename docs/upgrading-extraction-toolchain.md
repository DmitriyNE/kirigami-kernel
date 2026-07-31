# Regenerating the lifted model & upgrading the extraction toolchain

The Rust→Lean "crossing" (Charon `rustc → LLBC` + Aeneas `LLBC → pure Lean`) is
automated by `scripts/extract.sh` (`nix run .#extract`) and CI-enforced by the
`extraction-drift` workflow. This is the runbook for the two situations where you
touch it.

## What is generated vs hand-written

| File | Origin | Built? | Guarded by |
|---|---|---|---|
| `certify-check/Lattice/Funs.lean` | **generated** (Aeneas) | yes (proven about) | drift-check |
| `certify-check/Lattice/Types.lean` | **generated** (Aeneas) | yes | drift-check |
| `certify-check/extract/FunsExternal_Template.lean` | **generated** (Aeneas) | no (reference) | drift-check |
| `certify-check/Lattice/FunsExternal.lean` | **hand-written** (trusted Std-gap models) | yes | `check-externals.sh` |
| `certify-check/extract/lattice.startfrom` | **hand-written** (the `--start-from` set) | — | — |
| `certify-check/CertifyCheck/*.lean` | **hand-written** (the proofs) | yes | `lake build` + axiom audit |

`FunsExternal.lean` is the model's only hand-written TCB surface: faithful `def`
models of the `core`-library items Aeneas's Std lib cannot lift (see the §"Phase 5"
TCB note in `docs/spike-extraction-report.md`). It is **not** regenerated.

## CI gates (what enforces coherence)

- **`extraction-drift` workflow** (path-filtered to `crates/lattice/**`,
  `flake.{nix,lock}`, the scripts, `certify-check/{extract,Lattice}/**`):
  regenerates the model with the *pinned* Charon/Aeneas and `git diff`s it. Output
  is byte-deterministic at our pins, so drift = a real mismatch. Runs on
  `ubuntu-latest` only (the model is platform-independent; the `.#extraction` shell
  builds Charon/Aeneas from source, so we keep it off the hot path).
- **`check-externals.sh`** (in the main `ci` job + the drift workflow): every
  externalised `core` item has a faithful `def`; `FunsExternal.lean` contains **no
  `axiom`** (an axiom there would silently enter the certified footprint without
  being `sorryAx`).
- **`lake build` + axiom audit** (main `ci` job): the proofs typecheck and the
  certified theorems don't depend on `sorryAx`.

## Case 1 — you changed `crates/lattice` (routine)

If you edit a lifted function (or its dependency closure), the committed model is
now stale:

1. `nix run .#extract` — regenerates `Lattice/{Funs,Types}.lean` +
   `extract/FunsExternal_Template.lean`.
2. `cd certify-check && lake build` — **the proofs may break**: a semantics change
   in the Rust *should* surface as a broken proof (that's the point). Fix or
   re-examine the affected proofs.
3. If Aeneas now externalises a new `core` item, `check-externals.sh` (or the
   build) will flag it — add a faithful `def` to `FunsExternal.lean`.
4. Commit the regenerated model + any proof fixes together.

If you only added a *new* function to lift, first add its path to
`certify-check/extract/lattice.startfrom`, then do the above.

> **Charon `--start-from` gotcha:** naming an inherent impl method directly
> (`crate::small::SmallRat::reduce`) silently matches nothing. Lift the whole
> module (`crate::small`) — it pulls in the method and its closure. The manifest
> documents this.

## Case 2 — you bump the Charon/Aeneas pin (deliberate, coordinated)

A tool bump is a **dependency upgrade**, treated as one coordinated PR — not
something to auto-absorb. The drift-check failing on the bump is *correct*: it is
the prompt to regenerate. Both drift causes (Rust changed / tool bumped) resolve
the same way — regenerate + commit — a tool bump just additionally needs proof
fixes.

1. Update the `aeneas` (and, if separate, `charon`) input rev in `flake.nix` +
   `flake.lock`. **This usually drags the coupled Lean/Mathlib pins too** — Aeneas
   targets a specific Lean; update `certify-check/lean-toolchain`,
   `certify-check/lakefile.toml` (Mathlib + Aeneas `rev`), and refresh
   `lake-manifest.json` (`lake exe cache get`).
2. `nix run .#extract` — regenerate with the new tool.
3. `cd certify-check && lake build` — **expect proof breakage**: renamed generated
   defs, changed Aeneas Std-lib lemma names / tactic behaviour (`step`,
   `loop.spec_decr_nat`), and Mathlib API drift. Fixing these is the real cost of a
   bump and is unavoidable — the infra automates *model* regeneration, not the
   proofs. Re-run the axiom audit.
4. If the new Aeneas externalises different `core` items, add/adjust faithful
   models in `FunsExternal.lean` (`check-externals.sh` guides you).
5. **Review the regenerated model diff** — a new lifter version is a TCB change, so
   its output deserves eyes (this is where "cosmetic vs semantic" is judged; the
   check only tells you *that* it changed). Commit model + proof fixes + pin bump
   together; the drift / build / audit gates confirm the result is coherent.

Because the pins are fixed for reproducibility, tool bumps stay rare and
deliberate — which is exactly what makes this cost acceptable.
