//! **What a read produced, and why one refused.**
//!
//! Two error quantities, kept apart on purpose:
//!
//! * **`delta`** — the *backward error*: how far the geometry we built is from the geometry the
//!   file states. Ours.
//! * **`closure_gap`** — how far the file's own adjacent entities were from meeting. The file's.
//!
//! A single "tolerance" would blur them, and the blur runs one way: a sloppy file would masquerade
//! as a lossy importer, and a real importer regression would hide behind whichever file happened to
//! be worst that week. A third, `transport`, bounds what a *parser* did before we saw the text (see
//! [`crate::num::transport_bound`]) — zero in practice, never claimed to be.

use crate::num::to_decimal;
use crate::unit::Unit;
use lattice::{Backend, Rat};

/// Why a file could not be read. Every variant is a **refusal**, and every one names the entity —
/// a translator that repairs its input silently is worse than one that refuses, because the repair
/// is invisible in the part that comes out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportFault {
    /// A number in the file is not a decimal literal.
    MalformedNumber {
        /// Which entity it was read from.
        entity: String,
        /// The offending text.
        text: String,
    },
    /// Binary DXF. The reader is an ASCII reader, and says so rather than reading noise.
    BinaryDxf,
    /// The document is not well-formed XML / not an SVG.
    NotSvg(String),
    /// An SVG `A` with `rx ≠ ry`, or an `<ellipse>`. `Profile::arc` is circular.
    EllipticalArc {
        /// Which entity.
        entity: String,
    },
    /// An SVG cubic/quadratic segment. `arrange2d::Edge` carries lines and circular arcs only; an
    /// opt-in chord tolerance is the sanctioned lift, and it is not the default because a cubic
    /// silently chorded is a curve the kernel never agreed to carry.
    BezierSegment {
        /// Which entity.
        entity: String,
    },
    /// A transform whose linear part is not a similarity, applied to geometry containing an arc —
    /// it would map the circle to an ellipse. All-straight geometry accepts any exact `matrix()`.
    NonSimilarityTransform {
        /// Which entity.
        entity: String,
    },
    /// A rotation/skew whose sine and cosine are not rational, with no tolerance supplied.
    IrrationalTransform {
        /// Which entity.
        entity: String,
    },
    /// An entity outside the supported subset.
    UnsupportedEntity {
        /// Its type name, as the file spells it.
        kind: String,
    },
    /// The file declares no unit and the caller supplied none. Never inferred: see [`Unit`].
    UnknownUnit {
        /// What the file said, if it said anything.
        declared: Option<String>,
    },
    /// The entities do not chain into closed loops. The worst gap is reported so a reader can tell
    /// a *data* problem from a *subset* problem.
    OpenLoop {
        /// A decimal rendering of the gap, in the target unit.
        gap: String,
        /// Where the chain ran out.
        at: String,
    },
    /// Two arcs must share a vertex and their exact endpoints differ. Neither may move without
    /// leaving its own circle, so this refuses rather than repairing — see the module docs of
    /// [`crate::element`].
    ArcJunctionGap {
        /// A decimal rendering of the gap.
        gap: String,
    },
    /// A certified backward error over the caller's budget, with both numbers.
    ToleranceExceeded {
        /// Which entity.
        entity: String,
        /// What it achieved.
        delta: String,
        /// What it had to meet.
        budget: String,
    },
    /// An arc the file describes is not a circular arc we can carry (degenerate chord, zero radius).
    DegenerateArc {
        /// Which entity.
        entity: String,
        /// The `ArcFault` variant name.
        reason: String,
    },
    /// The file contains no geometry at all.
    Empty,
}

/// What one read did, in numbers — the half of the result that is not geometry.
#[derive(Debug)]
pub struct ImportReport<B: Backend> {
    /// The unit the file declared (or the one the caller supplied for a file that declared none).
    pub source_unit: Unit,
    /// The unit the geometry is in.
    pub target_unit: Unit,
    /// The exact conversion factor between them.
    pub scale: Rat<B>,
    /// How many source entities were read.
    pub entities: usize,
    /// How many closed loops they assembled into.
    pub loops: usize,
    /// **Ours**: the largest certified backward error over all constructions. Zero for a file of
    /// straight geometry, circles and bulge arcs — which is most files.
    pub delta: Rat<B>,
    /// **The parser's**: an ulp bound on decimal-text → rational, over every coordinate read. Zero
    /// for a reader that sees the text (DXF); nonzero-but-negligible for one that does not (SVG).
    pub transport: Rat<B>,
    /// **The file's**: the largest gap between adjacent entities that assembly had to absorb.
    pub closure_gap: Rat<B>,
}

impl<B: Backend> ImportReport<B> {
    /// One line, the format the demos print.
    pub fn summary(&self) -> String {
        format!(
            "{} entities → {} loops   {} → {} (×{})   δ={}  transport={}  gap={}",
            self.entities,
            self.loops,
            self.source_unit.name(),
            self.target_unit.name(),
            to_decimal(&self.scale, 6),
            to_decimal(&self.delta, 12),
            to_decimal(&self.transport, 12),
            to_decimal(&self.closure_gap, 12),
        )
    }

    /// Whether the read was **exact** — nothing we did moved the geometry at all.
    pub fn is_exact(&self) -> bool {
        self.delta.is_zero()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice::Bignum;

    /// The summary names all three error quantities separately, because a reader who sees one
    /// number cannot tell whose fault it was.
    #[test]
    fn the_summary_keeps_the_three_errors_apart() {
        let r = ImportReport::<Bignum> {
            source_unit: Unit::Inch,
            target_unit: Unit::Millimetre,
            scale: Rat::new(127, 5),
            entities: 12,
            loops: 2,
            delta: Rat::from_i128(0),
            transport: Rat::new(1, 1_000_000_000_000i128),
            closure_gap: Rat::new(1, 1000),
        };
        let s = r.summary();
        assert!(s.contains("δ=0."), "{s}");
        assert!(s.contains("transport=0.000000000001"), "{s}");
        assert!(s.contains("gap=0.001000000000"), "{s}");
        assert!(s.contains("in → mm (×25.400000)"), "{s}");
        assert!(
            r.is_exact(),
            "a nonzero closure gap is not our backward error"
        );
    }
}
