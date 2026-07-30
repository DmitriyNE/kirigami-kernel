# Flex-substrate kernel — handoff package

Cold-start entry point for the coding agent: **read `AGENT.md` first.**

## Contents
- `AGENT.md` — onboarding, invariants, task queue, definition of done. START HERE.
- `docs/agent-glossary.md` — terms/acronyms; read before the spec.
- `docs/flex-substrate-rep-spec-v0.24-full.md` — **the sole normative spec**.
- `docs/flex-substrate-rep-spec-v0.24-delta.md` — most-recent changelog (explains *why* each rule exists).
- `docs/spec-pending-v025.md` — queued edits (canonical arc decomposition — implement from the start).
- `docs/paper.md` — gentler narrative overview.
- `docs/implementation-plan-v1.md` — module decomposition, milestones, effort.
- `docs/vv-guide.md` — **verification & validation architecture (unified, authoritative)**.
- `fixtures/corpus.md` — ~30 counterexamples with required verdicts = the day-one regression suite.
- `proofs/` — Lean proof ledger (stub; grows with `certify-core`).

## The one-paragraph orientation
Certified-exact geometry kernel in Rust; no floats in certified paths; every claim ships a checkable certificate. Soundness = verified *checkers* (pure `certify-core`, proven in Lean 4 via Rust→Lean lifting + Kani on the lattice) + tested *searchers* (`kernel-search`, checked at runtime). The math is frozen at spec v0.24 after 24 adversarial review rounds — implement it, don't redesign it. Build order: lattice → arrangement kernel → charts → closure → sewing → gate. First task: repo skeleton + M0 lattice + the Lean-extraction spike (vv-guide §7).
