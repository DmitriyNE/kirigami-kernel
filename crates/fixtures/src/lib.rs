#![forbid(unsafe_code)]
//! `fixtures` — device instances + the counterexample corpus.
//!
//! The two normative device instances live under `fixtures/devices/`: the cone
//! (β = 42°, ID 5 mm, 240 µm 4-layer, 1.49 wrap — fully specified in spec §13)
//! and the petal with its conical flank (the general-case adversary). The
//! corpus (`fixtures/corpus.md`, ~30 entries) is transcribed here one module per
//! entry, the required verdict asserted.
//!
//! NOTE: the petal's exact geometry is not yet pinned — spec §13 gives only
//! qualitative parameters. It must be supplied (or a spec-consistent instance
//! synthesized and reviewed) before milestone B/C. The cone is reconstructible
//! now. See `docs/environment-and-crate-layout.md §6` / the handoff gap list.
