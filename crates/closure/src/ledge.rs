//! LEDGE branch **bridge**: turn a CAP-IN-D24-licensed cap boundary into the arrangement
//! input that `arrange2d`'s verified boolean consumes, and drive it to a certified planar
//! cap region.
//!
//! `LEDGE-BRANCH := CAP-IN-D24 ∧ LEDGE-DOM ∧ CAP-OUT ∧ SEW` (spec §8.5). This module builds the
//! first three; `SEW` — the shared final conjunct — is applied by
//! [`valid::closure_valid`](crate::valid::closure_valid). C1 mints the [`ValidatedD24`] license, and
//! [`arrange2d::boolean::ledge_dom_certified`] runs the whole §6 eight-step boolean *and* the
//! CAP-OUT / CAP-OUT-LINK postcondition internally (arrangement → seed `(0,0)` → operand
//! sidedness → ℤ₂² cocycle → coincident incidence → boolean select → separating edges →
//! π₀-quotient emit, then the Kani-proven cocycle / link checks and the boundary-bijection).
//! This module is the **wiring** between them: a licensed [`CanonicalEdge`] carries a
//! [`Carrier::Line`] and its two exact rational endpoints, which is exactly a
//! [`geom::content::SegPiece`] — so the bridge is a per-edge structural rewrite, no geometry
//! recomputed.
//!
//! **Operand model (straight-crease cylinder slice).** A [`ValidatedD24`] is *one* closed cap
//! boundary cycle spanning both flanks (the census demands flank correspondence in a single
//! loop), i.e. one simple polygon = the cap. Its interior is recovered as a **single-operand**
//! boolean: every edge is assigned [`OperandId::A`] and the cap is `A ∪ ∅` ([`BoolOp::Or`]) —
//! [`ledge_cap_certified`]. A single simple cycle cannot be split across two operands (an open
//! operand boundary flips the ℤ₂² label an odd number of times around some closed walk and the
//! cocycle check rejects it), so the genuine *two-operand* forced ledge — two flank footprints
//! that overlap or abut, unioned with π₀ merging across the shared crease — is exercised
//! directly against [`ledge_dom_certified`] in the tests, not funnelled through the
//! single-cycle license. Both land the same certified [`CapOut`].
//!
//! **Scope.** Only [`Carrier::Line`] edges bridge here: a cylinder's ruling images and the
//! straight crease are lines, so the M4 cylinder-flank cap is a polygon. A [`Carrier::Circle`]
//! edge (an arc cap boundary — the petal / curved-cut regime) is declined with
//! [`LedgeError::UnsupportedCarrier`]; bridging arcs is deferred with the §13 petal fixture.
//! Nothing keys on the flank *type*: the bridge reads carriers and endpoints the checker
//! already licensed, never a Rust branch on cone-vs-cylinder.

use arrange2d::boolean::{BoolOp, CapOut, CapOutFault, OperandId, ledge_dom_certified};
use certify_core::Verdict;
use certify_core::cap_in::{Carrier, ValidatedD24};
use geom::content::{CurveId, Edge, Line, Orient, Point2, SegPiece};
use lattice::Backend;

/// Why the LEDGE bridge could not turn a licensed cap boundary into arrangement input.
///
/// This is *not* a soundness verdict — the CAP-OUT decision is [`ledge_dom_certified`]'s
/// [`Verdict`]. It reports only that the bridge itself declined a component the current slice
/// does not represent.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LedgeError {
    /// The licensed edge at index `at` lies on a [`Carrier::Circle`] — an arc cap boundary.
    /// Bridging arcs (the curved-cut / petal regime) is deferred; only line carriers (a
    /// cylinder's straight rulings + crease) bridge in the M4 slice.
    UnsupportedCarrier {
        /// Index of the arc component in the licensed cycle.
        at: usize,
    },
}

/// Bridge a licensed cap boundary into `arrange2d` input edges: the i-th [`CanonicalEdge`]
/// becomes an [`Edge::Seg`] carrying source [`CurveId`]`(i)`, so an `operand_of` map can key on
/// the edge index (and thence the licensed [`FlankId`](certify_core::cap_in::FlankId), read from
/// `d24.edges()[i].flank()`).
///
/// The [`Carrier::Line`] `a·x + b·y + c = 0` becomes the segment's carrier verbatim, and the
/// exact rational endpoints become the segment's `start`/`end` — both already verified to lie on
/// the line by [`cap_in_d24`](certify_core::cap_in::cap_in_d24), so the arrangement's own
/// `validate_d24` totality guard passes. Returns [`LedgeError::UnsupportedCarrier`] at the first
/// arc component.
pub fn ledge_edges<B: Backend>(d24: &ValidatedD24<B>) -> Result<Vec<Edge<B>>, LedgeError> {
    let mut edges = Vec::with_capacity(d24.len());
    for (i, e) in d24.edges().iter().enumerate() {
        let (a, b, c) = match e.carrier() {
            Carrier::Line { a, b, c } => (a.clone(), b.clone(), c.clone()),
            Carrier::Circle { .. } => return Err(LedgeError::UnsupportedCarrier { at: i }),
        };
        let (sx, sy) = e.start();
        let (ex, ey) = e.end();
        edges.push(Edge::Seg(Box::new(SegPiece {
            line: Line { a, b, c },
            start: Point2::from_rat(sx.clone(), sy.clone()),
            end: Point2::from_rat(ex.clone(), ey.clone()),
            orient: Orient::Ccw,
            source: CurveId(i as u32),
        })));
    }
    Ok(edges)
}

/// The LEDGE branch, single-operand: certify the planar cap region enclosed by a licensed cap
/// boundary cycle (`CAP-IN-D24 → LEDGE-DOM → CAP-OUT`).
///
/// The one licensed cycle is a simple polygon = the cap, so every edge is [`OperandId::A`] and
/// the boolean is `A ∪ ∅` ([`BoolOp::Or`]) — recovering the polygon interior as one face. The
/// returned [`Verdict`] is [`ledge_dom_certified`]'s: [`Verified`](Verdict::Verified) mints the
/// [`CapOut`] (region + `V_∂` + pinch classification) once every CAP-OUT checker passes,
/// [`Refuted`](Verdict::Refuted) reports a [`CapOutFault`] (a constructor bug, since a licensed
/// boundary is well-formed input). The bridge error surfaces before any of that if the cap has
/// an arc component.
///
/// ```
/// use certify_core::Verdict;
/// use certify_core::cap_in::{cap_in_d24, FlankId};
/// use closure::cap_in::segment_edge;
/// use closure::ledge::ledge_cap_certified;
/// use lattice::{Bignum, Rat};
///
/// let p = |x: i128, y: i128| (Rat::<Bignum>::from_i128(x), Rat::from_i128(y));
/// // A square cap spanning both flanks: crease on the bottom, A up the far side + top, B
/// // closing the near side — the straight-edged cylinder-flank ledge cap.
/// let sq = [
///     segment_edge(&p(0, 0), &p(2, 0), FlankId::Crease),
///     segment_edge(&p(2, 0), &p(2, 2), FlankId::A),
///     segment_edge(&p(2, 2), &p(0, 2), FlankId::A),
///     segment_edge(&p(0, 2), &p(0, 0), FlankId::B),
/// ];
/// let d24 = match cap_in_d24(&sq) {
///     Verdict::Verified(v) => v,
///     Verdict::Refuted(fault) => panic!("cap boundary must license: {fault:?}"),
///     Verdict::Unresolved(()) => panic!("cap boundary census was inconclusive"),
/// };
/// match ledge_cap_certified(&d24).expect("all line carriers bridge") {
///     Verdict::Verified(cap) => assert_eq!(cap.region().faces.len(), 1),
///     Verdict::Refuted(fault) => panic!("CAP-OUT refused a licensed cap: {fault:?}"),
///     Verdict::Unresolved(()) => panic!("CAP-OUT was inconclusive"),
/// }
/// ```
pub fn ledge_cap_certified<B: Backend>(
    d24: &ValidatedD24<B>,
) -> Result<Verdict<CapOut<B>, CapOutFault, ()>, LedgeError> {
    let edges = ledge_edges(d24)?;
    let operand_of = |_c: CurveId| OperandId::A;
    Ok(ledge_dom_certified(&edges, &operand_of, BoolOp::Or))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cap_in::{PiFrame, project_point, segment_edge};
    use certify_core::cap_in::{FlankId, cap_in_d24};
    use geom::chart::Chart;
    use lattice::{Bignum, Poly, Rat, RatFunc};

    fn poly(cs: &[i128]) -> Poly<Bignum> {
        Poly::from_coeffs(cs.iter().map(|&c| Rat::from_i128(c)).collect())
    }
    /// A cylinder about the x-axis (`q = 1 + σi`) — straight rulings, so its cap boundary is a
    /// polygon of line carriers.
    fn cylinder() -> Chart<Bignum> {
        Chart::new(
            [poly(&[1]), poly(&[0, 1]), poly(&[0]), poly(&[0])],
            RatFunc::zero(),
        )
    }
    /// The `xy`-plane frame.
    fn xy_frame() -> PiFrame<Bignum> {
        let e = |i: usize| {
            let mut a = [Rat::from_i128(0), Rat::from_i128(0), Rat::from_i128(0)];
            a[i] = Rat::from_i128(1);
            a
        };
        PiFrame {
            origin: [Rat::from_i128(0), Rat::from_i128(0), Rat::from_i128(0)],
            u: e(0),
            v: e(1),
        }
    }
    fn p(x: i128, y: i128) -> (Rat<Bignum>, Rat<Bignum>) {
        (Rat::from_i128(x), Rat::from_i128(y))
    }

    /// The licensed cylinder-flank quad certifies as one planar cap face — the C4 chain
    /// `CAP-IN-D24 → LEDGE-DOM → CAP-OUT` end-to-end, from a real projected chart.
    #[test]
    fn the_cylinder_flank_ledge_cap_certifies() {
        let cyl = cylinder();
        let frame = xy_frame();
        // Four cylinder surface points (μ ∈ {0,1}, σ ∈ {0,1}, off-axis w = 1) projected into
        // the cap plane — the corners of a straight-edged cap quad.
        let pt = |mu: i128, sigma: i128| {
            let q = cyl
                .surface(&Rat::from_i128(mu), &Rat::from_i128(1))
                .eval(&Rat::from_i128(sigma))
                .expect("cylinder surface is regular");
            project_point(&q, &frame)
        };
        let (a, b, c, d) = (pt(0, 0), pt(1, 0), pt(1, 1), pt(0, 1));
        let quad = [
            segment_edge(&a, &b, FlankId::A),
            segment_edge(&b, &c, FlankId::A),
            segment_edge(&c, &d, FlankId::B),
            segment_edge(&d, &a, FlankId::Crease),
        ];
        let d24 = match cap_in_d24(&quad) {
            Verdict::Verified(v) => v,
            Verdict::Refuted(f) => panic!("cylinder cap must license: {f:?}"),
            Verdict::Unresolved(()) => panic!("cylinder cap census inconclusive"),
        };
        match ledge_cap_certified(&d24).expect("line carriers bridge") {
            Verdict::Verified(cap) => {
                assert_eq!(cap.region().faces.len(), 1, "one connected cap face");
                // A convex quad: four manifold boundary vertices, no self-touch pinch.
                assert_eq!(cap.v_boundary().len(), 4);
                assert!(cap.pinches().is_empty());
            }
            Verdict::Refuted(f) => panic!("CAP-OUT refused the licensed cylinder cap: {f:?}"),
            Verdict::Unresolved(()) => panic!("CAP-OUT inconclusive on the cylinder cap"),
        }
    }

    /// The genuine **two-operand forced ledge**: two flank footprint quads sharing the crease
    /// segment. Their union (`BoolOp::Or`) is one cap face — the shared crease is a separating
    /// edge between two selected cells, so π₀ merges them (spec §6: one face per connected
    /// component). This exercises the §6 boolean with *both* ℤ₂² bits live, which the
    /// single-cycle license cannot (see the module operand-model note).
    #[test]
    fn two_abutting_flanks_union_into_one_cap_across_the_crease() {
        // Operand A: the left square [0,2]×[0,2]. Operand B: the right square [2,4]×[0,2].
        // They share the edge x = 2 (the crease). Each is a closed CCW cycle; every edge of A
        // is source 0..4, every edge of B is source 4..8.
        let a_quad = [
            (p(0, 0), p(2, 0)),
            (p(2, 0), p(2, 2)),
            (p(2, 2), p(0, 2)),
            (p(0, 2), p(0, 0)),
        ];
        let b_quad = [
            (p(2, 0), p(4, 0)),
            (p(4, 0), p(4, 2)),
            (p(4, 2), p(2, 2)),
            (p(2, 2), p(2, 0)),
        ];
        let seg = |s: &(Rat<Bignum>, Rat<Bignum>),
                   e: &(Rat<Bignum>, Rat<Bignum>),
                   src: u32|
         -> Edge<Bignum> {
            let a = s.1.sub(&e.1);
            let b = e.0.sub(&s.0);
            let c = a.mul(&s.0).add(&b.mul(&s.1)).neg();
            Edge::Seg(Box::new(SegPiece {
                line: Line { a, b, c },
                start: Point2::from_rat(s.0.clone(), s.1.clone()),
                end: Point2::from_rat(e.0.clone(), e.1.clone()),
                orient: Orient::Ccw,
                source: CurveId(src),
            }))
        };
        let mut edges = Vec::new();
        for (i, (s, e)) in a_quad.iter().enumerate() {
            edges.push(seg(s, e, i as u32));
        }
        for (i, (s, e)) in b_quad.iter().enumerate() {
            edges.push(seg(s, e, 4 + i as u32));
        }
        let operand_of = |c: CurveId| if c.0 < 4 { OperandId::A } else { OperandId::B };
        match ledge_dom_certified(&edges, &operand_of, BoolOp::Or) {
            Verdict::Verified(cap) => {
                // A ∪ B is the single rectangle [0,4]×[0,2]: one face, the crease dropped.
                assert_eq!(
                    cap.region().faces.len(),
                    1,
                    "the two flanks merge into one cap"
                );
            }
            Verdict::Refuted(f) => panic!("two-operand ledge refused: {f:?}"),
            Verdict::Unresolved(()) => panic!("two-operand ledge inconclusive"),
        }
    }

    /// An arc cap boundary (a [`Carrier::Circle`] edge) is declined by the bridge — the
    /// curved-cut / petal regime is deferred to the §13 fixture, not silently mis-bridged.
    #[test]
    fn an_arc_cap_boundary_is_declined() {
        use certify_core::cap_in::{BoundaryComponent, Carrier};
        // A quarter-circle arc (cos t, sin t) on the unit circle, plus two radii closing it —
        // a valid licensed cap with one circular edge. We license it, then check the bridge
        // declines the arc rather than the whole certify path.
        // Parametrize the arc rationally as the (1-t²,2t)/(1+t²) half-angle map on t∈[0,1]
        // (quarter circle from (1,0) to (0,1)); carrier is the unit circle x²+y²=1.
        let num_x = Poly::from_coeffs(vec![
            Rat::from_i128(1),
            Rat::from_i128(0),
            Rat::from_i128(-1),
        ]);
        let num_y = Poly::from_coeffs(vec![Rat::from_i128(0), Rat::from_i128(2)]);
        let den = Poly::from_coeffs(vec![
            Rat::from_i128(1),
            Rat::from_i128(0),
            Rat::from_i128(1),
        ]);
        let arc = BoundaryComponent {
            x: RatFunc::new(num_x, den.clone()),
            y: RatFunc::new(num_y, den),
            t_lo: Rat::from_i128(0),
            t_hi: Rat::from_i128(1),
            carrier: Carrier::Circle {
                cx: Rat::from_i128(0),
                cy: Rat::from_i128(0),
                r2: Rat::from_i128(1),
            },
            flank: FlankId::A,
        };
        let radius_a = segment_edge(&p(0, 1), &p(0, 0), FlankId::B);
        let radius_b = segment_edge(&p(0, 0), &p(1, 0), FlankId::Crease);
        let comps = [arc, radius_a, radius_b];
        let d24 = match cap_in_d24(&comps) {
            Verdict::Verified(v) => v,
            Verdict::Refuted(f) => panic!("arc cap must license (the carrier IS a circle): {f:?}"),
            Verdict::Unresolved(()) => panic!("arc cap census inconclusive"),
        };
        // The arc is component 0, so the bridge declines it there.
        assert!(matches!(
            ledge_edges(&d24),
            Err(LedgeError::UnsupportedCarrier { at: 0 })
        ));
    }
}
