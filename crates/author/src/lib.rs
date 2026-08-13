#![forbid(unsafe_code)]
//! `author` — the construction facade (shell tier).
//!
//! One authoring context over the one general engine: declare a [`Part`](part::Part) from a
//! [`construct`] entry point, describe it with **regions** (piecewise support on one frame) and
//! **material ops** (solid [`Cutter`](part::Cutter)s — roles are *derived*, never authored),
//! then evaluate: [`develop()`](part::Part::develop) certifies the flat pattern. The recipe is
//! exact data throughout; approximate product inputs (degrees, poses) are snapped and echoed.
//!
//! The trust story is the engine's, unchanged: floats here only *search* (the resolver, the rail
//! oracle); every geometric claim is decided by the exact certificates downstream
//! (`cut_fit`, the anchor-chord unroll, the exact 2-D boolean), and an inconclusive resolution
//! is a typed fault, never a guess.

pub mod construct;
pub mod part;

pub(crate) mod realize;
pub(crate) mod resolve;

pub use develop::place::Placement;
pub use part::{Cutter, FlatPattern, OpKind, Part, PartFault, PartSolid, RegionPick, SupportFn};
