#![forbid(unsafe_code)]
//! `fixtures` — device instances + the counterexample corpus.
//!
//! - [`devices`] — the normative device instances (spec §13). The cone is a rational
//!   surrogate available now via [`devices::cone`]; the petal conical flank lands with
//!   milestone C (its exact geometry is not yet pinned by spec §13).
//! - The counterexample corpus (`fixtures/corpus.md`, ~30 entries) is transcribed here
//!   one module per entry, the required verdict asserted, as the checkers land.

pub mod corpus;
pub mod devices;
