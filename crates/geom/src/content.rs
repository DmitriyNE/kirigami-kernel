//! 2D content primitives — the D24 flat-content curve types the §6 arrangement
//! operates on (spec §6; `docs/vv-guide.md §8` M3a). Directed lines, circles
//! (stored by squared radius), and the simple x-monotone arc/segment pieces that
//! canonical decomposition produces. Exact over `lattice` rationals; intersection
//! coordinates are degree-≤2 [`Surd`]s.
//!
//! This is the M3a slice of `geom`, deliberately separate from the M1
//! chart/spline scope in the crate-root doc. The arrangement *algorithms*
//! (predicates, carrier solving, membership, classification) live in `arrange2d`;
//! these are just the shared primitive types (reused later by closure/sew).

use core::cmp::Ordering;
use lattice::{Backend, Bignum, Rat, Surd};

/// Identifier for a source input curve — carried as provenance on decomposed
/// pieces (winding is provenance on the source, never DCEL multiplicity).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CurveId(pub u32);

/// A directed line `a·x + b·y + c = 0` over exact ℚ, **not** normalized (`|n|` is
/// irrational). Direction `(b, −a)`; leftward normal `(a, b)`. Orientation is
/// meaningful — §6 re-reads the face-orientation bit, never recomputes it.
#[derive(Debug)]
pub struct Line<B: Backend = Bignum> {
    pub a: Rat<B>,
    pub b: Rat<B>,
    pub c: Rat<B>,
}

/// A circle by center and **squared** radius (spec §2.2: predicates use `r²`,
/// never the irrational `r`).
#[derive(Debug)]
pub struct Circle<B: Backend = Bignum> {
    pub cx: Rat<B>,
    pub cy: Rat<B>,
    pub r2: Rat<B>,
}

/// A planar point whose coordinates may be degree-2 algebraic (`a + b√d`);
/// rational points are the `b = 0` degenerate case, kept cheap by [`Surd`].
/// Ordered lexicographically (x then y) — the sweep order, exact via
/// cross-radical-safe [`Surd`] comparison.
#[derive(Debug)]
pub struct Point2<B: Backend = Bignum> {
    pub x: Surd<B>,
    pub y: Surd<B>,
}

impl<B: Backend> Point2<B> {
    /// A rational point `(x, y)`.
    pub fn from_rat(x: Rat<B>, y: Rat<B>) -> Self {
        Point2 {
            x: Surd::from_rat(x),
            y: Surd::from_rat(y),
        }
    }
}

impl<B: Backend> PartialEq for Point2<B> {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl<B: Backend> Eq for Point2<B> {}
impl<B: Backend> PartialOrd for Point2<B> {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl<B: Backend> Ord for Point2<B> {
    fn cmp(&self, o: &Self) -> Ordering {
        self.x.cmp(&o.x).then_with(|| self.y.cmp(&o.y))
    }
}

/// Which branch of a circle an x-monotone arc piece is, after the axis-aligned
/// decomposition: the graph of a function of x with `y ≥ cy` (`Upper`) or
/// `y ≤ cy` (`Lower`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Half {
    Upper,
    Lower,
}

/// Orientation of a source curve — provenance only in M3a; the §8.3 winding
/// integer (Sturm-isolated poles) is computed in slice 3c (`arrange2d::azimuth`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Orient {
    Ccw,
    Cw,
}

/// Winding as **provenance** on the original curve (pending-v0.25 §1.4):
/// orientation plus the source span this piece came from — never DCEL-edge
/// multiplicity.
#[derive(Debug)]
pub struct Winding<B: Backend = Bignum> {
    pub orient: Orient,
    pub source_span: Option<(Point2<B>, Point2<B>)>,
}

/// A simple x-monotone circular-arc piece — canonical decomposition output. The
/// arc of `circle` on `half` spanning `[x_lo, x_hi]`, with endpoints and source
/// provenance. No piece spans more than one simple point-set arc (pending-v0.25).
#[derive(Debug)]
pub struct ArcPiece<B: Backend = Bignum> {
    pub circle: Circle<B>,
    pub half: Half,
    pub x_lo: Surd<B>,
    pub x_hi: Surd<B>,
    pub start: Point2<B>,
    pub end: Point2<B>,
    pub winding: Winding<B>,
    pub source: CurveId,
}

/// A line-segment piece (compact content — line inputs are bounded segments).
/// x-monotone unless vertical; the arrangement's sweep order (lexicographic
/// (x, then y)) handles the vertical case. `orient` is the stored face-orientation
/// bit (provenance, re-read never recomputed — §6); a segment is never sub-split,
/// so it needs the bit but no source-span (unlike [`ArcPiece`]'s [`Winding`]).
#[derive(Debug)]
pub struct SegPiece<B: Backend = Bignum> {
    pub line: Line<B>,
    pub start: Point2<B>,
    pub end: Point2<B>,
    pub orient: Orient,
    pub source: CurveId,
}

/// An arrangement edge: a decomposed line segment or a simple x-monotone arc.
/// Both payloads are boxed — each piece holds several [`Surd`] coordinates (large),
/// so an unboxed variant would make every `Edge` slot in an edge list piece-sized.
/// Boxing keeps `Edge` a uniform small handle.
#[derive(Debug)]
pub enum Edge<B: Backend = Bignum> {
    Seg(Box<SegPiece<B>>),
    Arc(Box<ArcPiece<B>>),
}

/// A pre-decomposition input content curve (spec §6, D24: lines + circular arcs).
/// Canonical decomposition (`arrange2d::decompose`) turns each into simple
/// x-monotone [`Edge`]s: a segment passes through; a whole circle splits into two
/// arcs; a proper arc splits at whichever x-extrema fall inside its angular span.
#[derive(Debug)]
pub enum Curve<B: Backend = Bignum> {
    /// A line segment — already x-monotone (vertical handled by the sweep order).
    Seg(SegPiece<B>),
    /// A whole circle, oriented — splits into exactly two x-monotone arcs.
    Circle {
        circle: Circle<B>,
        orient: Orient,
        source: CurveId,
    },
    /// A circular arc from `start` to `end` (both on `circle`) traversed in
    /// `orient`; `start != end` (a whole circle is [`Curve::Circle`]).
    Arc {
        circle: Circle<B>,
        start: Point2<B>,
        end: Point2<B>,
        orient: Orient,
        source: CurveId,
    },
}

// Manual `Clone` (no `B: Clone` bound — mirrors `lattice`'s `Rat`/`Surd`), so the
// whole arrangement stays generic over any `Backend` without a spurious bound.
// (`Backend` implementors are marker types; the fields' own manual `Clone` does
// the work.) `Debug` stays derived — it is only ever formatted at concrete `Bignum`.
impl<B: Backend> Clone for Line<B> {
    fn clone(&self) -> Self {
        Line {
            a: self.a.clone(),
            b: self.b.clone(),
            c: self.c.clone(),
        }
    }
}
impl<B: Backend> Clone for Circle<B> {
    fn clone(&self) -> Self {
        Circle {
            cx: self.cx.clone(),
            cy: self.cy.clone(),
            r2: self.r2.clone(),
        }
    }
}
impl<B: Backend> Clone for Point2<B> {
    fn clone(&self) -> Self {
        Point2 {
            x: self.x.clone(),
            y: self.y.clone(),
        }
    }
}
impl<B: Backend> Clone for Winding<B> {
    fn clone(&self) -> Self {
        Winding {
            orient: self.orient,
            source_span: self.source_span.clone(),
        }
    }
}
impl<B: Backend> Clone for ArcPiece<B> {
    fn clone(&self) -> Self {
        ArcPiece {
            circle: self.circle.clone(),
            half: self.half,
            x_lo: self.x_lo.clone(),
            x_hi: self.x_hi.clone(),
            start: self.start.clone(),
            end: self.end.clone(),
            winding: self.winding.clone(),
            source: self.source,
        }
    }
}
impl<B: Backend> Clone for SegPiece<B> {
    fn clone(&self) -> Self {
        SegPiece {
            line: self.line.clone(),
            start: self.start.clone(),
            end: self.end.clone(),
            orient: self.orient,
            source: self.source,
        }
    }
}
impl<B: Backend> Clone for Edge<B> {
    fn clone(&self) -> Self {
        match self {
            Edge::Seg(s) => Edge::Seg(s.clone()),
            Edge::Arc(a) => Edge::Arc(a.clone()),
        }
    }
}
impl<B: Backend> Clone for Curve<B> {
    fn clone(&self) -> Self {
        match self {
            Curve::Seg(s) => Curve::Seg(s.clone()),
            Curve::Circle {
                circle,
                orient,
                source,
            } => Curve::Circle {
                circle: circle.clone(),
                orient: *orient,
                source: *source,
            },
            Curve::Arc {
                circle,
                start,
                end,
                orient,
                source,
            } => Curve::Arc {
                circle: circle.clone(),
                start: start.clone(),
                end: end.clone(),
                orient: *orient,
                source: *source,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Q = Rat<Bignum>;

    #[test]
    fn point2_lexicographic_order() {
        let p = |x, y| Point2::<Bignum>::from_rat(Q::from_i128(x), Q::from_i128(y));
        assert!(p(0, 0) < p(0, 1)); // tie on x → compare y
        assert!(p(0, 5) < p(1, 0)); // x dominates
        assert_eq!(p(2, 3), p(2, 3));
    }

    #[test]
    fn point2_surd_coords_order() {
        // (√2, 0) vs (17/12, 0): √2 < 17/12 ⇒ ordered by x
        let a = Point2::<Bignum> {
            x: Surd::new(Q::from_i128(0), Q::from_i128(1), Q::from_i128(2)),
            y: Surd::from_rat(Q::from_i128(0)),
        };
        let b = Point2::<Bignum> {
            x: Surd::from_rat(Q::new(17, 12)),
            y: Surd::from_rat(Q::from_i128(0)),
        };
        assert!(a < b);
    }
}
