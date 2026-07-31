# Proof ledger

Documentation *about* the proofs — the V&V companion to `../../vv-matrix.md`. The
machine-checked Lean proofs themselves live in `../../certify-check/`; this
directory tracks **the theorems the `certify-core` checkers rely on**.

Each certificate the kernel emits is sound because a checker's runtime
verification discharges the hypotheses of a theorem (`../vv-guide.md §0`, the
runtime-checked-hypothesis pattern — the project's main proof-effort reducer).
Every obligation records:

- **Statement** — the mathematical theorem.
- **Citation** — a Mathlib lemma where one exists, else the literature.
- **Checked at runtime** — the hypotheses the Rust checker verifies exactly
  (which fn, which conditions).
- **Structural** — hypotheses baked into the setup, not runtime-checked.
- **Lean** — how it is formalized in `certify-check/`, plus the axiom footprint.
  A clean footprint is `[propext, Classical.choice, Quot.sound]`; a single
  labelled cited `axiom` (e.g. `sturm_root_count`) is the honest assumption of the
  runtime-checked-hypothesis pattern. Every proven/cited obligation is in the CI
  `#print axioms` gate.

## Layout

- [`ledger.md`](ledger.md) — the index: one row per obligation, linking to detail.
- one file per checker/obligation — [`sturm.md`](sturm.md),
  [`resultant.md`](resultant.md), [`fast-path.md`](fast-path.md), …

The ledger starts small and grows with `certify-core` (M2–M5 add MITER-FIT,
EDGE-EMB, the Sylvester / germ / cap certificates); a new obligation gets a row in
`ledger.md` and its own detail file.
