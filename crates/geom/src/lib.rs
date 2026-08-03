#![forbid(unsafe_code)]
//! `geom` — the kernel's geometry primitives: exact 2D content types and the exact
//! σ-parametric chart layer. Everything is exact over `lattice` numbers; no floating
//! point.
//!
//! - [`content`] — 2D flat-content primitives (directed lines, circles, arc/segment
//!   pieces) that the `arrange2d` §6 arrangement operates on.
//! - [`chart`] — the exact chart field layer (spec §3.2): from a quaternion spline
//!   `q(σ)` and support spline `h(σ)`, the surface normal `n`, ruling `r`, pedal `c`,
//!   the thickened map `C(σ,μ,w)`, and `det J` — all exact rational functions of σ.
//! - [`tags`] — primitive-tag classification of a chart (cone, cylinder, …) with an
//!   exact witness (spec §3.6).
//! - [`stall`] — the hatted stall calculus at a parametrization stall (spec §3.2.2),
//!   including the `J_raw = p̂·Ĵ` identity.
//! - [`reparam`] — the REPARAM verb: regenerate a stalled record as a regular one
//!   (spec §7).

pub mod chart;
pub mod content;
pub mod reparam;
pub mod stall;
pub mod tags;
