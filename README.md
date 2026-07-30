# Kirigami kernel

A certified-exact geometry kernel for formed flexible-PCB substrates — the implementation of the *Flex Substrate Representation* spec v0.24. (Kirigami: cut *and* fold — the flat artwork is cut, then folded/rolled, where origami would only fold.) Cold-start entry point for the coding agent: **read `AGENT.md` first.**

## Contents
- `AGENT.md` — onboarding, invariants, task queue, definition of done. START HERE.
- `docs/agent-glossary.md` — terms/acronyms; read before the spec.
- `spec/flex-substrate-rep-spec-v0.24-full.md` — **the sole normative spec**.
- `spec/flex-substrate-rep-spec-v0.24-delta.md` — most-recent changelog (explains *why* each rule exists).
- `spec/spec-pending-v025.md` — queued edits (canonical arc decomposition — implement from the start).
- `docs/paper.md` — gentler narrative overview.
- `docs/implementation-plan-v1.md` — module decomposition, milestones, effort.
- `docs/environment-and-crate-layout.md` — **resolved engineering decisions: crate layout, edition/MSRV/tool pins, Lean/Mathlib, Nix flake. Read before scaffolding the repo.**
- `docs/vv-guide.md` — **verification & validation architecture (unified, authoritative)**.
- `fixtures/corpus.md` — ~30 counterexamples with required verdicts = the day-one regression suite.
- `proofs/` — Lean proof ledger (stub; grows with `certify-core`).

*The normative spec lineage — v0.24 full + delta + pending — lives **only** under `spec/` (one copy; the CI lints scan it). Everything under `docs/` is informative companion material.*

## The one-paragraph orientation
Certified-exact geometry kernel in Rust; no floats in certified paths; every claim ships a checkable certificate. Soundness = verified *checkers* (pure `certify-core`, proven in Lean 4 via Rust→Lean lifting + Kani on the lattice) + tested *searchers* (`kernel-search`, checked at runtime). The math is frozen at spec v0.24 after 24 adversarial review rounds — implement it, don't redesign it. Build order: lattice → arrangement kernel → charts → closure → sewing → gate. First task: repo skeleton + M0 lattice + the Lean-extraction spike (vv-guide §7).
