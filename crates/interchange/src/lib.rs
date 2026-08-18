//! **The file boundary** — read a real outline in, write a real drawing out.
//!
//! Until a fab-house file could enter the kernel, every device in this repo was a Rust literal.
//! This crate is the translator layer that ends that, and its whole design turns on one fact that
//! the reflex reading of "CAD files are floats" gets backwards:
//!
//! > **A decimal literal *is* a rational.** `12.345` is `12345/1000`. Reading it loses nothing; the
//! > only approximation is the `f64` a parser transports it through, and Rust's shortest-round-trip
//! > `Display` hands the literal back exactly for anything under 17 significant digits.
//!
//! So most of an import is **exact**: every straight segment, every circle, every polyline vertex,
//! every unit conversion (`1 mm = 480/127 px`, exactly) and every `matrix`/`translate`/`scale`
//! transform arrives with backward error `δ = 0`. A uniform "tolerance" would be a weaker claim
//! than the data supports, and would bury the one construction that genuinely cannot be exact.
//!
//! Where `δ ≠ 0` does arise it is a **consistency** failure rather than a rounding: a DXF `ARC`
//! states centre, radius *and* two angles — four exact rationals describing an irrational point —
//! so exactly one datum has to move. Which one differs per source form, and the ranking is not the
//! obvious one; see [`arc`].
//!
//! Design: `docs/interchange-design.md`. Criteria: `docs/vv-guide.md`, "IO acceptance criteria".

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod arc;
pub mod dxf;
pub mod element;
pub mod num;
pub mod report;
pub mod svg;
pub mod unit;
pub mod write;

use arrange2d::profile::Profile;
use element::Element;
use lattice::{Backend, Rat};
use report::ImportReport;

/// What a read produced: closed loops of exact geometry, and the numbers describing how it got
/// there.
#[derive(Debug)]
pub struct Imported<B: Backend> {
    /// One entry per closed loop, elements head-to-tail. Nesting needs no ordering — the profile's
    /// fill rule is even-odd, so a loop drawn inside another is a hole.
    pub loops: Vec<Vec<Element<B>>>,
    /// What the read did, in numbers.
    pub report: ImportReport<B>,
}

impl<B: Backend> Imported<B> {
    /// The arrangement edges a `Cutter::extrude` profile consumes — arcs kept as arcs.
    pub fn profile(&self) -> Profile<B> {
        element::to_profile(&self.loops)
    }
}

/// The larger of two exact rationals (`Rat` has no `max`, and a `max` written three ways would
/// drift).
pub(crate) fn max_of<B: Backend>(a: Rat<B>, b: Rat<B>) -> Rat<B> {
    if a > b { a } else { b }
}
