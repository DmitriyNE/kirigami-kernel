//! **Writing** — the drawing model, and the error that goes out rather than in.
//!
//! Export is not import run backwards, and the asymmetry is the thing to get right.
//!
//! On the way **in**, a circular arc's data is over-determined and one datum has to move
//! ([`crate::arc`]). On the way **out** the geometry is already exact and the *format* is the thing
//! that cannot hold it: a decimal file has no way to write `√r²`, and no way to write the DXF bulge
//! `tan(Δθ/4)`, which is irrational for almost every arc a kernel produces. So a written arc always
//! costs something, and — mirroring the import table — **which datum it costs differs by format**:
//!
//! | route | what the file can hold exactly | what it rounds |
//! |---|---|---|
//! | DXF `POLYLINE` + bulge | the two vertices | `tan(Δθ/4)`, so the **centre and radius** move |
//! | SVG `A` | the two endpoints | `√r²`, so the **radius** moves |
//!
//! Both are reported, and neither is *estimated*: the writer re-reads its own decimals through the
//! importer and measures what came back ([`ExportReport::curve`]). A round trip therefore composes
//! two genuinely different errors, and the tests report them separately — averaging an exact leg
//! with an inexact one would say nothing about either.
//!
//! One exactness that survives outbound and is worth knowing: for an arc with rational centre and
//! endpoints, `cos Δθ` and `sin Δθ` are **exact rationals** (`u·v/r²` and `u×v/r²`). The turn is
//! exact; only its quarter-tangent is not.

use crate::arc::ExactArc;
use crate::element::Element;
use crate::num::{round_decimal, sqrt_rational, to_decimal};
use crate::unit::Unit;
use lattice::{Backend, Rat};

/// One named layer of a drawing — a DXF layer, an SVG group.
pub struct Layer<B: Backend> {
    /// Layer name, as it appears in the file.
    pub name: String,
    /// The closed loops on it.
    pub loops: Vec<Vec<Element<B>>>,
}

impl<B: Backend> Layer<B> {
    /// A layer holding one set of loops.
    pub fn new(name: impl Into<String>, loops: Vec<Vec<Element<B>>>) -> Self {
        Layer {
            name: name.into(),
            loops,
        }
    }
}

/// A drawing to be written: named layers of closed loops, in one unit.
///
/// Layers are how the outline and the holes stay separable in the emitted file — a fab house reads
/// them as different things, and collapsing them into one layer loses information the kernel had.
pub struct Drawing<B: Backend> {
    /// The layers, in emission order.
    pub layers: Vec<Layer<B>>,
}

impl<B: Backend> Default for Drawing<B> {
    fn default() -> Self {
        Drawing { layers: Vec::new() }
    }
}

impl<B: Backend> Drawing<B> {
    /// An empty drawing.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a layer.
    pub fn layer(mut self, name: impl Into<String>, loops: Vec<Vec<Element<B>>>) -> Self {
        self.layers.push(Layer::new(name, loops));
        self
    }

    /// Every element, in emission order.
    pub fn elements(&self) -> impl Iterator<Item = &Element<B>> {
        self.layers
            .iter()
            .flat_map(|l| l.loops.iter().flat_map(|c| c.iter()))
    }

    /// The drawing's bounding box, `(minx, miny, maxx, maxy)`, or `None` when it is empty.
    ///
    /// Arcs contribute their **whole circle**, which over-covers rather than under-covers: an
    /// extent that clipped an arc's bulge would crop the drawing, and a slightly generous frame
    /// costs nothing.
    pub fn bounds(&self) -> Option<[Rat<B>; 4]> {
        let mut b: Option<[Rat<B>; 4]> = None;
        let mut grow = |x: Rat<B>, y: Rat<B>| {
            b = Some(match b.take() {
                None => [x.clone(), y.clone(), x, y],
                Some([lx, ly, hx, hy]) => [
                    if x < lx { x.clone() } else { lx },
                    if y < ly { y.clone() } else { ly },
                    if x > hx { x } else { hx },
                    if y > hy { y } else { hy },
                ],
            });
        };
        for e in self.elements() {
            match e {
                Element::Segment { start, end } => {
                    grow(start[0].clone(), start[1].clone());
                    grow(end[0].clone(), end[1].clone());
                }
                Element::Arc(a) => {
                    let r = radius(a);
                    grow(a.cx.sub(&r), a.cy.sub(&r));
                    grow(a.cx.add(&r), a.cy.add(&r));
                }
                Element::Circle { cx, cy, r2 } => {
                    let r = sqrt_rational(r2, 32);
                    grow(cx.sub(&r), cy.sub(&r));
                    grow(cx.add(&r), cy.add(&r));
                }
            }
        }
        b
    }
}

/// How to write: the unit to declare, and how many decimals to spend.
#[derive(Clone, Debug)]
pub struct WriteOptions {
    /// The unit the geometry is in, and the one the file declares. Written, never assumed — the
    /// reader on the other end refuses a file that does not say.
    pub unit: Unit,
    /// Decimal places per coordinate. The default spends far more than any fab process needs, on
    /// the principle that a *file* is cheap and a re-import that has to guess is not.
    pub places: usize,
    /// A provenance line written into the file as a comment — the place a flat pattern's own
    /// certified `ε` and worst vertex-box radius belong, so the drawing carries how good it is
    /// rather than implying an exactness it does not have.
    pub note: Option<String>,
}

impl Default for WriteOptions {
    fn default() -> Self {
        WriteOptions {
            unit: Unit::Millimetre,
            places: 12,
            note: None,
        }
    }
}

/// What one write cost — measured against the file's own decimals, not bounded from the format.
#[derive(Debug)]
pub struct ExportReport<B: Backend> {
    /// The unit declared in the file.
    pub unit: Unit,
    /// Decimal places spent per coordinate.
    pub places: usize,
    /// How many file entities were emitted.
    pub entities: usize,
    /// The largest rounding applied to any coordinate.
    pub coord: Rat<B>,
    /// The largest error in a **curve's** own data — the centre-plus-radius displacement of an arc
    /// re-read from the decimals actually written. Zero for a drawing with no arcs, and zero for an
    /// arc whose format-native parameter happens to be rational.
    pub curve: Rat<B>,
}

impl<B: Backend> ExportReport<B> {
    /// One line, the format the demos print.
    pub fn summary(&self) -> String {
        format!(
            "{} entities   {} @ {} places   coord={}  curve={}",
            self.entities,
            self.unit.name(),
            self.places,
            to_decimal(&self.coord, self.places + 3),
            to_decimal(&self.curve, self.places + 3),
        )
    }

    /// Whether the write was **exact** — nothing the format could not hold.
    pub fn is_exact(&self) -> bool {
        self.coord.is_zero() && self.curve.is_zero()
    }
}

/// Accumulates the two measured errors while a writer runs.
pub(crate) struct Cost<B: Backend> {
    pub(crate) coord: Rat<B>,
    pub(crate) curve: Rat<B>,
    pub(crate) entities: usize,
    pub(crate) places: usize,
}

impl<B: Backend> Cost<B> {
    pub(crate) fn new(places: usize) -> Self {
        Cost {
            coord: Rat::from_i128(0),
            curve: Rat::from_i128(0),
            entities: 0,
            places,
        }
    }

    /// Round a coordinate for emission, recording what that cost.
    pub(crate) fn coord(&mut self, q: &Rat<B>) -> Rat<B> {
        let r = round_decimal(q, self.places);
        let moved = abs(&r.sub(q));
        if moved > self.coord {
            self.coord = moved;
        }
        r
    }

    /// Emit a coordinate as text, recording what that cost.
    pub(crate) fn text(&mut self, q: &Rat<B>) -> String {
        let r = self.coord(q);
        to_decimal(&r, self.places)
    }

    pub(crate) fn curve(&mut self, moved: Rat<B>) {
        if moved > self.curve {
            self.curve = moved;
        }
    }

    pub(crate) fn into_report(self, unit: Unit) -> ExportReport<B> {
        ExportReport {
            unit,
            places: self.places,
            entities: self.entities,
            coord: self.coord,
            curve: self.curve,
        }
    }
}

pub(crate) fn abs<B: Backend>(q: &Rat<B>) -> Rat<B> {
    if q.sign() < 0 { q.neg() } else { q.clone() }
}

/// An arc's radius — **exact** when `r²` is a rational square, an upper bound otherwise.
pub(crate) fn radius<B: Backend>(a: &ExactArc<B>) -> Rat<B> {
    sqrt_rational(&a.r2, 32)
}

/// `(cos Δθ, sin Δθ)` for the arc's own sweep — **exactly rational**, for any exact arc.
///
/// With `u = start − c` and `v = end − c`, both of squared length `r²`, the dot and cross products
/// give `cos` and `sin` directly. No transcendental appears: the *turn* of an exact arc is exact,
/// and only its quarter-tangent (what DXF wants) is not.
///
/// The pair describes the counter-clockwise turn from `start` to `end`; a clockwise arc's own sweep
/// is its negation, which the callers apply.
pub(crate) fn sweep_cos_sin<B: Backend>(a: &ExactArc<B>) -> (Rat<B>, Rat<B>) {
    let u = [a.start[0].sub(&a.cx), a.start[1].sub(&a.cy)];
    let v = [a.end[0].sub(&a.cx), a.end[1].sub(&a.cy)];
    let dot = u[0].mul(&v[0]).add(&u[1].mul(&v[1]));
    let cross = u[0].mul(&v[1]).sub(&u[1].mul(&v[0]));
    (dot.div(&a.r2), cross.div(&a.r2))
}

/// Whether the arc's own sweep exceeds a half turn — SVG's `large-arc-flag`, and the branch that
/// decides the sign of `cos(Δ/2)` for the DXF bulge.
pub(crate) fn is_major<B: Backend>(a: &ExactArc<B>) -> bool {
    let (_, s) = sweep_cos_sin(a);
    // The counter-clockwise sweep is in `(0, 2π)`; it exceeds `π` exactly when its sine is
    // negative. A clockwise arc sweeps the other way, so the test flips with it.
    if a.ccw { s.sign() < 0 } else { s.sign() > 0 }
}

/// The DXF **bulge** `tan(Δ/4)` of an arc, signed (positive counter-clockwise).
///
/// `cos Δ` is exact, so the two half-angle steps are two square roots and nothing else:
///
/// ```text
/// cos(Δ/2) = ±√((1 + cos Δ)/2)          (negative exactly when Δ > π)
/// tan(Δ/4) =  √((1 − cos(Δ/2)) / (1 + cos(Δ/2)))
/// ```
///
/// Each root is exact when the data admits one (`Δ = π` gives `tan 45° = 1`, which a semicircle
/// really does write exactly) and an outward-rounded bound otherwise.
pub(crate) fn bulge<B: Backend>(a: &ExactArc<B>) -> Rat<B> {
    let (c, _) = sweep_cos_sin(a);
    let one = Rat::from_i128(1);
    let two = Rat::from_i128(2);
    let half_cos = sqrt_rational(&one.add(&c).div(&two), 32);
    let half_cos = if is_major(a) {
        half_cos.neg()
    } else {
        half_cos
    };
    let ratio = one.sub(&half_cos).div(&one.add(&half_cos));
    let t = sqrt_rational(&ratio, 32);
    if a.ccw { t } else { t.neg() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arc::from_bulge;
    use lattice::Bignum;

    type Q = Rat<Bignum>;

    fn p(x: i128, y: i128) -> [Q; 2] {
        [Q::from_i128(x), Q::from_i128(y)]
    }

    /// The turn of an exact arc is **exact** — no enclosure, no tolerance. This is what lets the
    /// writers decide `large-arc` and the bulge branch by an exact sign test.
    #[test]
    fn the_sweep_of_an_exact_arc_is_exactly_rational() {
        // A semicircle: cos Δ = −1, sin Δ = 0, exactly.
        let semi = from_bulge::<Bignum>(p(1, 0), p(-1, 0), &Q::from_i128(1)).expect("exact");
        let (c, s) = sweep_cos_sin(&semi);
        assert_eq!(c, Q::from_i128(-1));
        assert_eq!(s, Q::from_i128(0));
        // A 3-4-5 quarter-ish arc: still exactly rational, and on the unit circle.
        let arc = from_bulge::<Bignum>(p(1, 0), [Q::new(3, 5), Q::new(4, 5)], &Q::new(1, 2))
            .expect("exact");
        let (c, s) = sweep_cos_sin(&arc);
        assert_eq!(
            c.mul(&c).add(&s.mul(&s)),
            Q::from_i128(1),
            "cos² + sin² = 1"
        );
    }

    /// A semicircle's bulge is exactly `1`, and its own round trip is the identity — the case
    /// where the format *can* hold the curve, asserted so the general lossy case has a control.
    #[test]
    fn a_semicircle_writes_its_bulge_exactly() {
        let semi = from_bulge::<Bignum>(p(1, 0), p(-1, 0), &Q::from_i128(1)).expect("exact");
        assert_eq!(bulge(&semi), Q::from_i128(1));
        assert!(!is_major(&semi), "a half turn is not a major arc");
        let back = from_bulge::<Bignum>(p(1, 0), p(-1, 0), &bulge(&semi)).expect("re-read");
        assert_eq!(back.r2, semi.r2);
        assert_eq!([back.cx, back.cy], [semi.cx.clone(), semi.cy.clone()]);
    }

    /// The bulge sign carries the direction, and the major/minor branch is decided by an exact
    /// sign rather than by a magnitude comparison.
    #[test]
    fn the_bulge_carries_direction_and_the_major_branch() {
        let minor = from_bulge::<Bignum>(p(1, 0), p(0, 1), &Q::new(1, 2)).expect("exact");
        assert!(bulge(&minor).sign() > 0 && !is_major(&minor));
        let cw = from_bulge::<Bignum>(p(1, 0), p(0, 1), &Q::new(-1, 2)).expect("exact");
        assert!(bulge(&cw).sign() < 0);
        // |bulge| > 1 is exactly the signature of a major arc.
        let major = from_bulge::<Bignum>(p(1, 0), p(0, 1), &Q::from_i128(2)).expect("exact");
        assert!(is_major(&major), "b = 2 is more than a half turn");
        assert!(abs(&bulge(&major)) > Q::from_i128(1));
    }

    /// A drawing's extent covers an arc's bulge, not just its endpoints — a frame that cropped it
    /// would clip the drawing in the emitted file.
    #[test]
    fn the_extent_covers_an_arc_rather_than_its_chord() {
        let semi = from_bulge::<Bignum>(p(1, 0), p(-1, 0), &Q::from_i128(1)).expect("exact");
        let d = Drawing::<Bignum>::new().layer("outline", vec![vec![Element::Arc(semi)]]);
        let [lx, ly, hx, hy] = d.bounds().expect("nonempty");
        assert!(lx <= Q::from_i128(-1) && hx >= Q::from_i128(1));
        assert!(
            ly <= Q::from_i128(-1) && hy >= Q::from_i128(1),
            "the bulge is inside"
        );
    }

    /// The cost accumulator measures what rounding actually did rather than assuming the format's
    /// worst case.
    #[test]
    fn the_cost_accumulator_measures_rather_than_bounds() {
        let mut c = Cost::<Bignum>::new(3);
        assert_eq!(c.text(&Q::new(1, 2)), "0.500");
        assert_eq!(c.coord, Q::from_i128(0), "an exact decimal costs nothing");
        assert_eq!(c.text(&Q::new(1, 3)), "0.333");
        assert_eq!(c.coord, Q::new(1, 3).sub(&Q::new(333, 1000)));
        assert!(c.coord < Q::new(1, 2000), "under half a place");
    }
}
