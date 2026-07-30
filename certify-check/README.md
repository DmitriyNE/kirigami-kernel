# certify-check — Lean 4 extraction target

The deductive-verification surface for the flex-substrate kernel. Hosts:
- the **hand-written Lean specs** of the `certify-core` checkers (each spec *is* the formalization of its certificate definition — `vv-guide §4`), and
- the **hax / Aeneas lifted models** of those checkers (Rust → Lean, always; there is no Lean → Rust codegen).

A `lake` project, **not** a cargo crate — hence it lives outside the cargo workspace.

## Provisional pins

`lean-toolchain` and the Mathlib rev in `lakefile.toml` are **provisional**. They are locked at the `vv-guide §7` extraction spike, **downstream of the chosen Aeneas release** (the Lean version must match Aeneas's Lean backend; Mathlib follows Lean). See `docs/environment-and-crate-layout.md §3`.

## First real content (the spike)

1. `certify_core`'s sign-variation counter, lifted by both hax and Aeneas, proven against its Lean spec.
2. The Sturm hypothesis-checker (chain identities ⇒ Sturm chain) proven against a Mathlib citation.

Both produce the go/no-go and the per-checker template every later `certify-core` function follows. `proofs/ledger.md` records the theorems (citation, hypotheses-checked-at-runtime vs structural).

## Build

Requires `elan` (provided by the dev flake). Once the pins are locked:

```
cd certify-check
lake exe cache get   # Mathlib prebuilt oleans
lake build
```
