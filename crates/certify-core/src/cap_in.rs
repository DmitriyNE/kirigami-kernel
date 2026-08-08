//! CAP-IN-D24 — the input-license census (spec §8.5).
//!
//! The LEDGE branch of a closure builds a planar cap region by the §6 boolean
//! arrangement, which is only total on **well-formed D24 content**: lines and
//! circular arcs, exact intervals, a closed boundary cycle. CAP-IN-D24 is the
//! *input license* that gate — run on the **source boundary components** before any
//! arrangement, so a malformed or non-D24 boundary is refused up front rather than
//! panicking (or worse, silently mis-arranging) downstream.
//!
//! The one census that matters for generality is the **carrier identity test**: each
//! component claims to lie on a line or a circle ([`Carrier`]), and the checker
//! verifies that identity holds *as a rational-function identity in the curve
//! parameter* — not sampled, not approximate. A planar or cylinder-type flank
//! contributes line images (a ruling is a line; the axis lies in the cap plane), and
//! those pass; a **cone / oblique / generalized** flank contributes a **conic** cut
//! image, which satisfies no line and no circle identity, so it is refused
//! ([`CapInFault::OffCarrier`]) — *falsely*, not vacuously (spec §8.5). This is the
//! representation-level reason the closure vertical slice is cylinder-first: the
//! cylinder passes CAP-IN-D24, the cone is correctly turned away.
//!
//! CAP-IN-D24 is **consulted only on the LEDGE (arrangement) branch** — a clean miter
//! pairs cut edges directly and never constructs a [`ValidatedD24`].
//!
//! This is the full census the minimal `arrange2d` totality guard (`validate_d24`)
//! deferred: per source component, carrier by identity test, exact finite interval,
//! rational endpoints, a closed cycle, and flank correspondence. A [`ValidatedD24`] is
//! **minted only** by [`cap_in_d24`], so possessing one *is* the proof the boundary is
//! licensed input.

use alloc::vec::Vec;
use core::cmp::Ordering;

use lattice::{Backend, Bignum, Poly, Rat, RatFunc};

use crate::Verdict;

/// Which flank (or the shared crease) contributed a cap-boundary component — the
/// provenance CAP-IN-D24's flank-correspondence check audits.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FlankId {
    /// The A-side flank (`b_A = b_J`).
    A,
    /// The B-side flank (`b_B = −b_J`).
    B,
    /// The shared straight crease (a ruling common to both flanks).
    Crease,
}

/// The algebraic carrier a boundary component claims to lie on — the only two D24
/// carrier classes (spec §6): a directed line, or a circle stored by **squared**
/// radius (predicates never touch the irrational `r`).
#[derive(Debug)]
pub enum Carrier<B: Backend = Bignum> {
    /// The line `a·x + b·y + c = 0`; well-formed iff `(a, b) ≠ (0, 0)`.
    Line {
        /// The `x` coefficient.
        a: Rat<B>,
        /// The `y` coefficient.
        b: Rat<B>,
        /// The constant term.
        c: Rat<B>,
    },
    /// The circle `(x − cx)² + (y − cy)² = r2`; well-formed iff `r2 > 0`.
    Circle {
        /// The center `x`.
        cx: Rat<B>,
        /// The center `y`.
        cy: Rat<B>,
        /// The **squared** radius `r²`.
        r2: Rat<B>,
    },
}

// Manual `Clone` (no `B: Clone` bound — `Backend` implementors are marker types, as in
// `geom::content`); the fields' own manual `Clone` does the work.
impl<B: Backend> Clone for Carrier<B> {
    fn clone(&self) -> Self {
        match self {
            Carrier::Line { a, b, c } => Carrier::Line {
                a: a.clone(),
                b: b.clone(),
                c: c.clone(),
            },
            Carrier::Circle { cx, cy, r2 } => Carrier::Circle {
                cx: cx.clone(),
                cy: cy.clone(),
                r2: r2.clone(),
            },
        }
    }
}

/// A source boundary component of the cap region, *before any arrangement*: the
/// composed rational parametrization `(x(t), y(t))` over the exact parameter interval
/// `[t_lo, t_hi]`, the [`Carrier`] it claims to lie on, and the [`FlankId`] that
/// contributed it. The searcher (`closure`) composes these from the joint's flank
/// charts (projecting a chart curve into the cap plane); CAP-IN-D24 licenses them.
#[derive(Debug)]
pub struct BoundaryComponent<B: Backend = Bignum> {
    /// The `x` parametrization `x(t)`.
    pub x: RatFunc<B>,
    /// The `y` parametrization `y(t)`.
    pub y: RatFunc<B>,
    /// The inclusive lower parameter bound.
    pub t_lo: Rat<B>,
    /// The inclusive upper parameter bound.
    pub t_hi: Rat<B>,
    /// The carrier the component claims to lie on.
    pub carrier: Carrier<B>,
    /// The flank (or crease) that contributed this component.
    pub flank: FlankId,
}

impl<B: Backend> Clone for BoundaryComponent<B> {
    fn clone(&self) -> Self {
        BoundaryComponent {
            x: self.x.clone(),
            y: self.y.clone(),
            t_lo: self.t_lo.clone(),
            t_hi: self.t_hi.clone(),
            carrier: self.carrier.clone(),
            flank: self.flank,
        }
    }
}

/// The exact residual of a component against its claimed carrier: the rational
/// function that is **identically zero** iff the parametrization lies on the carrier.
/// For a line this is `a·x(t) + b·y(t) + c`; for a circle `(x−cx)² + (y−cy)² − r2`.
fn carrier_residual<B: Backend>(comp: &BoundaryComponent<B>) -> RatFunc<B> {
    let konst = |k: &Rat<B>| RatFunc::from_poly(Poly::constant(k.clone()));
    match &comp.carrier {
        Carrier::Line { a, b, c } => comp.x.scale(a).add(&comp.y.scale(b)).add(&konst(c)),
        Carrier::Circle { cx, cy, r2 } => {
            let dx = comp.x.sub(&konst(cx));
            let dy = comp.y.sub(&konst(cy));
            dx.mul(&dx).add(&dy.mul(&dy)).sub(&konst(r2))
        }
    }
}

/// Whether a component's parametrization lies on its claimed [`Carrier`] — the
/// **carrier identity test**, exact (a rational-function identity in the parameter,
/// never sampled). A genuine conic satisfies no line and no circle identity, so this
/// is `false` for any carrier it could claim — the generality gate that turns a cone
/// cut image away while admitting a cylinder ruling image.
///
/// A degenerate line (`a = b = 0`) or non-positive circle radius is **not** a carrier,
/// so this returns `false` for them regardless of the parametrization; [`cap_in_d24`]
/// reports the precise reason.
pub fn on_carrier<B: Backend>(comp: &BoundaryComponent<B>) -> bool {
    match &comp.carrier {
        Carrier::Line { a, b, .. } if a.sign() == 0 && b.sign() == 0 => return false,
        Carrier::Circle { r2, .. } if r2.sign() <= 0 => return false,
        _ => {}
    }
    carrier_residual(comp).is_zero()
}

/// Does a boundary edge with endpoint `end` hand off to the next edge's `next_start`? — the
/// per-link test of the CAP-IN-D24 cycle-closure census (step 4): both coordinates coincide
/// exactly. The census ANDs this over **every** cyclic consecutive pair, not merely the wrap
/// `edge[n-1] → edge[0]`; checking only the wrap would admit a chain with a broken internal
/// link (two disjoint sub-loops whose free ends happen to meet). Generic over the coordinate
/// order so the ★ soundness property ([`crate::proof`]'s `cap_in_cycle_census_sound`) runs on
/// `i128` — the exact ordering `cap_in_d24` applies at `T = Rat`.
pub fn edge_hands_off<T: Ord>(end: &(T, T), next_start: &(T, T)) -> bool {
    end.0.cmp(&next_start.0) == Ordering::Equal && end.1.cmp(&next_start.1) == Ordering::Equal
}

/// A licensed cap-boundary edge: a [`Carrier`], the concrete rational endpoints the
/// census evaluated, and the [`FlankId`] provenance. **Minted only** as part of a
/// [`ValidatedD24`] by [`cap_in_d24`] — the fields are private, so a `CanonicalEdge`
/// cannot be forged from unlicensed parts. Read it through the accessors.
#[derive(Debug)]
pub struct CanonicalEdge<B: Backend = Bignum> {
    carrier: Carrier<B>,
    start: (Rat<B>, Rat<B>),
    end: (Rat<B>, Rat<B>),
    flank: FlankId,
}

impl<B: Backend> Clone for CanonicalEdge<B> {
    fn clone(&self) -> Self {
        CanonicalEdge {
            carrier: self.carrier.clone(),
            start: (self.start.0.clone(), self.start.1.clone()),
            end: (self.end.0.clone(), self.end.1.clone()),
            flank: self.flank,
        }
    }
}

impl<B: Backend> CanonicalEdge<B> {
    /// The carrier this edge lies on (verified by identity, not claimed).
    pub fn carrier(&self) -> &Carrier<B> {
        &self.carrier
    }
    /// The start point `(x(t_lo), y(t_lo))`.
    pub fn start(&self) -> &(Rat<B>, Rat<B>) {
        &self.start
    }
    /// The end point `(x(t_hi), y(t_hi))`.
    pub fn end(&self) -> &(Rat<B>, Rat<B>) {
        &self.end
    }
    /// The flank (or crease) that contributed this edge.
    pub fn flank(&self) -> FlankId {
        self.flank
    }
}

/// The CAP-IN-D24 **input license**: the ordered cycle of [`CanonicalEdge`]s the
/// census admitted. Opaque and **minted only** by [`cap_in_d24`], after every
/// component lies on its carrier, has an exact finite interval and rational endpoints,
/// the cycle closes, and the flank correspondence holds — so possessing a
/// `ValidatedD24` *is* the proof the cap boundary is well-formed, licensed D24. It is
/// the LEDGE branch's entry token; a clean miter never constructs one.
#[derive(Debug)]
pub struct ValidatedD24<B: Backend = Bignum> {
    edges: Vec<CanonicalEdge<B>>,
}

impl<B: Backend> Clone for ValidatedD24<B> {
    fn clone(&self) -> Self {
        ValidatedD24 {
            edges: self.edges.clone(),
        }
    }
}

impl<B: Backend> ValidatedD24<B> {
    /// The licensed boundary cycle, in cyclic order.
    pub fn edges(&self) -> &[CanonicalEdge<B>] {
        &self.edges
    }
    /// The number of boundary components.
    pub fn len(&self) -> usize {
        self.edges.len()
    }
    /// Whether the cycle is empty. A minted license is never empty (a cap needs a
    /// closed boundary spanning both flanks); provided to satisfy the `len`/`is_empty`
    /// pairing.
    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }
}

/// Why CAP-IN-D24 refused a cap boundary — a malformed or non-D24 input, indexed to
/// the offending component. This is the full census the `arrange2d` totality guard
/// (`validate_d24`) deferred: a genuine conic (a cone/oblique cap cut image) fails
/// [`OffCarrier`](CapInFault::OffCarrier), *falsely* — not vacuously (spec §8.5).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CapInFault {
    /// No components — a cap has no boundary.
    Empty,
    /// The component at `at` claims a line with `a = b = 0` (no direction).
    DegenerateLine {
        /// Index of the offending component.
        at: usize,
    },
    /// The component at `at` claims a circle with `r² ≤ 0` (not a real circle).
    NonPositiveRadius {
        /// Index of the offending component.
        at: usize,
    },
    /// The component at `at` does not lie on its claimed carrier — the carrier
    /// identity fails. A conic cut image (cone / oblique / generalized flank) lands
    /// here for any line or circle it could claim.
    OffCarrier {
        /// Index of the offending component.
        at: usize,
    },
    /// The component at `at` has an empty or reversed interval (`t_lo ≥ t_hi`).
    EmptyInterval {
        /// Index of the offending component.
        at: usize,
    },
    /// The parametrization at `at` is singular at an endpoint (its denominator
    /// vanishes there), so it has no rational endpoint.
    SingularEndpoint {
        /// Index of the offending component.
        at: usize,
    },
    /// The cycle does not close: the end of component `at` is not the start of the
    /// next (cyclically). The input boundary is not a closed loop.
    OpenCycle {
        /// Index of the component whose end is dangling.
        at: usize,
    },
    /// Flank correspondence failed — the cycle does not span both flanks (a cap must
    /// be bounded by content from the A side and the B side).
    FlankCorrespondence,
}

/// CAP-IN-D24 (spec §8.5): license a cap's **source boundary components** as
/// well-formed D24, minting the [`ValidatedD24`] the LEDGE branch consumes.
///
/// Per component, in order: the [`Carrier`] is well-formed (a line has a direction, a
/// circle has `r² > 0`) and the parametrization **lies on it** by exact identity
/// ([`on_carrier`]); the interval is finite (`t_lo < t_hi`); the endpoints are rational
/// (the parametrization is non-singular there). Then across the cycle: consecutive
/// endpoints coincide (the loop closes), and both flanks are represented. Any failure
/// is [`Refuted`](Verdict::Refuted) with the precise [`CapInFault`]; success mints the
/// license. The verdict is two-valued (never `Unresolved`) — the census is total.
///
/// ```
/// use certify_core::cap_in::{cap_in_d24, BoundaryComponent, CapInFault, Carrier, FlankId};
/// use certify_core::Verdict;
/// use lattice::{Bignum, Poly, Rat, RatFunc};
///
/// let rf = |cs: &[i128]| RatFunc::<Bignum>::from_poly(
///     Poly::from_coeffs(cs.iter().map(|&c| Rat::from_i128(c)).collect()),
/// );
/// let seg = |x: &[i128], y: &[i128], carrier, flank| BoundaryComponent {
///     x: rf(x), y: rf(y), t_lo: Rat::from_i128(0), t_hi: Rat::from_i128(1), carrier, flank,
/// };
/// let line = |a, b, c| Carrier::Line {
///     a: Rat::<Bignum>::from_i128(a), b: Rat::from_i128(b), c: Rat::from_i128(c),
/// };
/// // A triangle (0,0)→(1,0)→(0,1): crease on y=0, flank A on x+y=1, flank B on x=0.
/// let tri = [
///     seg(&[0, 1], &[0],     line(0, 1, 0),  FlankId::Crease), // y ≡ 0
///     seg(&[1, -1], &[0, 1], line(1, 1, -1), FlankId::A),      // (1−t, t): x+y ≡ 1
///     seg(&[0], &[1, -1],    line(1, 0, 0),  FlankId::B),      // (0, 1−t): x ≡ 0
/// ];
/// let d24 = match cap_in_d24(&tri) {
///     Verdict::Verified(v) => v,
///     other => panic!("expected a license, got {other:?}"),
/// };
/// assert_eq!(d24.len(), 3);
///
/// // A parabola (t, t²) claiming to be a line is a conic — refused, not licensed.
/// let conic = [seg(&[0, 1], &[0, 0, 1], line(0, 1, 0), FlankId::A)];
/// assert!(matches!(cap_in_d24(&conic), Verdict::Refuted(CapInFault::OffCarrier { at: 0 })));
/// ```
pub fn cap_in_d24<B: Backend>(
    components: &[BoundaryComponent<B>],
) -> Verdict<ValidatedD24<B>, CapInFault, ()> {
    if components.is_empty() {
        return Verdict::Refuted(CapInFault::Empty);
    }
    let mut edges: Vec<CanonicalEdge<B>> = Vec::with_capacity(components.len());
    let mut i = 0;
    while i < components.len() {
        let comp = &components[i];
        // (1) carrier well-formedness + the exact identity test.
        match &comp.carrier {
            Carrier::Line { a, b, .. } => {
                if a.sign() == 0 && b.sign() == 0 {
                    return Verdict::Refuted(CapInFault::DegenerateLine { at: i });
                }
            }
            Carrier::Circle { r2, .. } => {
                if r2.sign() <= 0 {
                    return Verdict::Refuted(CapInFault::NonPositiveRadius { at: i });
                }
            }
        }
        if !carrier_residual(comp).is_zero() {
            return Verdict::Refuted(CapInFault::OffCarrier { at: i });
        }
        // (2) exact finite interval.
        if comp.t_lo.cmp(&comp.t_hi) != Ordering::Less {
            return Verdict::Refuted(CapInFault::EmptyInterval { at: i });
        }
        // (3) rational endpoints (the parametrization is non-singular at the ends).
        let start = match (comp.x.eval(&comp.t_lo), comp.y.eval(&comp.t_lo)) {
            (Some(x), Some(y)) => (x, y),
            _ => return Verdict::Refuted(CapInFault::SingularEndpoint { at: i }),
        };
        let end = match (comp.x.eval(&comp.t_hi), comp.y.eval(&comp.t_hi)) {
            (Some(x), Some(y)) => (x, y),
            _ => return Verdict::Refuted(CapInFault::SingularEndpoint { at: i }),
        };
        edges.push(CanonicalEdge {
            carrier: comp.carrier.clone(),
            start,
            end,
            flank: comp.flank,
        });
        i += 1;
    }
    // (4) endpoint ownership — the cycle closes: edge[k].end == edge[k+1].start (cyclic).
    let n = edges.len();
    let mut k = 0;
    while k < n {
        if !edge_hands_off(&edges[k].end, &edges[(k + 1) % n].start) {
            return Verdict::Refuted(CapInFault::OpenCycle { at: k });
        }
        k += 1;
    }
    // (5) flank correspondence — the cap is bounded by both flanks' content.
    let has_a = edges.iter().any(|e| e.flank == FlankId::A);
    let has_b = edges.iter().any(|e| e.flank == FlankId::B);
    if !(has_a && has_b) {
        return Verdict::Refuted(CapInFault::FlankCorrespondence);
    }
    Verdict::Verified(ValidatedD24 { edges })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn rf(cs: &[i128]) -> RatFunc<Bignum> {
        RatFunc::from_poly(Poly::from_coeffs(
            cs.iter().map(|&c| Rat::from_i128(c)).collect(),
        ))
    }
    fn line(a: i128, b: i128, c: i128) -> Carrier<Bignum> {
        Carrier::Line {
            a: Rat::from_i128(a),
            b: Rat::from_i128(b),
            c: Rat::from_i128(c),
        }
    }
    fn seg(
        x: &[i128],
        y: &[i128],
        t_lo: i128,
        t_hi: i128,
        carrier: Carrier<Bignum>,
        flank: FlankId,
    ) -> BoundaryComponent<Bignum> {
        BoundaryComponent {
            x: rf(x),
            y: rf(y),
            t_lo: Rat::from_i128(t_lo),
            t_hi: Rat::from_i128(t_hi),
            carrier,
            flank,
        }
    }
    /// The unit triangle (0,0)→(1,0)→(0,1): crease `y≡0`, flank A `x+y≡1`, flank B `x≡0`.
    fn unit_triangle() -> vec::Vec<BoundaryComponent<Bignum>> {
        vec![
            seg(&[0, 1], &[0], 0, 1, line(0, 1, 0), FlankId::Crease),
            seg(&[1, -1], &[0, 1], 0, 1, line(1, 1, -1), FlankId::A),
            seg(&[0], &[1, -1], 0, 1, line(1, 0, 0), FlankId::B),
        ]
    }

    #[test]
    fn licenses_a_closed_line_triangle() {
        let d24 = match cap_in_d24(&unit_triangle()) {
            Verdict::Verified(v) => v,
            other => panic!("triangle must license: {other:?}"),
        };
        assert_eq!(d24.len(), 3);
        assert_eq!(d24.edges()[0].flank(), FlankId::Crease);
        assert_eq!(
            *d24.edges()[1].start(),
            (Rat::from_i128(1), Rat::from_i128(0))
        );
    }

    #[test]
    fn licenses_a_rational_circle_arc() {
        // The Weierstrass half-circle: x = (1−t²)/(1+t²), y = 2t/(1+t²) on the unit
        // circle. A genuine circle carrier (r² = 1) — the identity holds exactly.
        let x = RatFunc::<Bignum>::new(
            Poly::from_coeffs(vec![
                Rat::from_i128(1),
                Rat::from_i128(0),
                Rat::from_i128(-1),
            ]),
            Poly::from_coeffs(vec![
                Rat::from_i128(1),
                Rat::from_i128(0),
                Rat::from_i128(1),
            ]),
        );
        let y = RatFunc::<Bignum>::new(
            Poly::from_coeffs(vec![Rat::from_i128(0), Rat::from_i128(2)]),
            Poly::from_coeffs(vec![
                Rat::from_i128(1),
                Rat::from_i128(0),
                Rat::from_i128(1),
            ]),
        );
        let arc = BoundaryComponent {
            x,
            y,
            t_lo: Rat::from_i128(0),
            t_hi: Rat::from_i128(1),
            carrier: Carrier::Circle {
                cx: Rat::from_i128(0),
                cy: Rat::from_i128(0),
                r2: Rat::from_i128(1),
            },
            flank: FlankId::A,
        };
        assert!(on_carrier(&arc));
    }

    #[test]
    fn refutes_a_conic_as_off_carrier() {
        // A parabola (t, t²) claims to lie on y = 0 — it is a conic, on no line.
        let conic = [seg(&[0, 1], &[0, 0, 1], 0, 1, line(0, 1, 0), FlankId::A)];
        assert!(!on_carrier(&conic[0]));
        assert!(matches!(
            cap_in_d24(&conic),
            Verdict::Refuted(CapInFault::OffCarrier { at: 0 })
        ));
    }

    #[test]
    fn refutes_a_degenerate_line() {
        let bad = [seg(&[0], &[0], 0, 1, line(0, 0, 0), FlankId::A)];
        assert!(matches!(
            cap_in_d24(&bad),
            Verdict::Refuted(CapInFault::DegenerateLine { at: 0 })
        ));
    }

    #[test]
    fn refutes_a_non_positive_radius() {
        let bad = [seg(
            &[0],
            &[0],
            0,
            1,
            Carrier::Circle {
                cx: Rat::from_i128(0),
                cy: Rat::from_i128(0),
                r2: Rat::from_i128(0),
            },
            FlankId::A,
        )];
        assert!(matches!(
            cap_in_d24(&bad),
            Verdict::Refuted(CapInFault::NonPositiveRadius { at: 0 })
        ));
    }

    #[test]
    fn refutes_a_reversed_interval() {
        let bad = [seg(&[0, 1], &[0], 1, 0, line(0, 1, 0), FlankId::A)];
        assert!(matches!(
            cap_in_d24(&bad),
            Verdict::Refuted(CapInFault::EmptyInterval { at: 0 })
        ));
    }

    #[test]
    fn refutes_a_singular_endpoint() {
        // x = 1/t is singular at t = 0 — no rational endpoint there.
        let x = RatFunc::<Bignum>::new(
            Poly::constant(Rat::from_i128(1)),
            Poly::from_coeffs(vec![Rat::from_i128(0), Rat::from_i128(1)]),
        );
        let bad = [BoundaryComponent {
            x,
            y: rf(&[0]),
            t_lo: Rat::from_i128(0),
            t_hi: Rat::from_i128(1),
            carrier: line(0, 1, 0),
            flank: FlankId::A,
        }];
        assert!(matches!(
            cap_in_d24(&bad),
            Verdict::Refuted(CapInFault::SingularEndpoint { at: 0 })
        ));
    }

    #[test]
    fn refutes_an_open_cycle() {
        // Two collinear segments that do not close back to the start.
        let open = [
            seg(&[0, 1], &[0], 0, 1, line(0, 1, 0), FlankId::A),
            seg(&[1, 1], &[0], 0, 1, line(0, 1, 0), FlankId::B),
        ];
        assert!(matches!(
            cap_in_d24(&open),
            Verdict::Refuted(CapInFault::OpenCycle { .. })
        ));
    }

    #[test]
    fn refutes_a_cap_missing_a_flank() {
        // A closed triangle whose edges are all tagged to one flank — no B side.
        let mut tri = unit_triangle();
        for c in &mut tri {
            c.flank = FlankId::A;
        }
        assert!(matches!(
            cap_in_d24(&tri),
            Verdict::Refuted(CapInFault::FlankCorrespondence)
        ));
    }

    #[test]
    fn empty_input_is_refused() {
        let none: [BoundaryComponent<Bignum>; 0] = [];
        assert!(matches!(
            cap_in_d24(&none),
            Verdict::Refuted(CapInFault::Empty)
        ));
    }
}
