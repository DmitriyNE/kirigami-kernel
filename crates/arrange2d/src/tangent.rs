//! The general outgoing-tangent azimuth order at an arrangement vertex — the DCEL
//! rotation system's angular comparator. Unlike [`super::azimuth`]'s
//! `tag_cmp` (positions of points on ONE fixed circle), this orders the outgoing
//! tangent **directions** of half-edges leaving a shared vertex, over the mixed
//! edge kinds (segments + arcs on different circles) that meet there.
//!
//! The comparator is the §8.3 discipline applied to directions: a half-plane split
//! at the `+x` ray plus an exact `Surd` cross-product sign (no angle materialized),
//! with a **curvature tie-break** so mutually-tangent edges (the
//! `TouchKind::Tangent` case — a line tangent to a circle, two mutually-tangent
//! circles) still receive a total order. Every direction at one vertex is built
//! from that vertex's coordinates and rational carrier data, so the products stay
//! in one radical and never escalate (mirrors [`super::classify`]).

use core::cmp::Ordering;
use geom::content::{Edge, Half};
use lattice::{Backend, Rat, Surd};

/// A half-edge's outgoing tangent at its origin vertex: the direction `(dx, dy)`
/// (a `Surd` vector, unnormalized) plus the signed-curvature key that breaks ties
/// between edges leaving with the **same** direction. `curv_sign` is `+1` if the
/// half-edge bends to the left of travel (turns CCW around its centre), `−1` if it
/// bends right, `0` for a straight segment; `r2` is the circle's squared radius
/// (only read when `curv_sign != 0`).
pub struct Outgoing<B: Backend> {
    pub dx: Surd<B>,
    pub dy: Surd<B>,
    pub curv_sign: i8,
    pub r2: Rat<B>,
}

/// `-1 | 0 | 1` from an `Ordering` (the sign of `a − b` for `a.cmp(&b)`).
fn ord_i8(o: Ordering) -> i8 {
    match o {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

/// The outgoing tangent of the half-edge on `edge` whose origin is `edge.start`
/// (`from_start = true`) or `edge.end` (`from_start = false`) — i.e. the direction
/// the half-edge travels away from that endpoint, along the edge.
///
/// A segment's tangent is the rational line direction `±(b, −a)`, signed toward the
/// far endpoint. An arc's tangent is `±(−n_y, n_x)` (`n = v − C`), the sign fixed by
/// the traversal sense around the centre: on the `Upper` half moving toward `+x` is
/// CW (θ decreasing), toward `−x` is CCW; on the `Lower` half the reverse. This
/// determination is exact and handles the vertical-tangent extrema (`L`/`R`) with no
/// special case (the endpoints of a semicircle piece have `n` horizontal, so the
/// tangent is `±(0, n_x)` = vertical, pointing into the half).
pub fn outgoing_tangent<B: Backend>(edge: &Edge<B>, from_start: bool) -> Outgoing<B> {
    match edge {
        Edge::Seg(s) => {
            let (start, end) = (&s.start, &s.end);
            let (a, b) = (&s.line.a, &s.line.b);
            // sign of the stored `start → end` direction relative to `(b, −a)`:
            // `end − start = λ·(b, −a)`, recovered from whichever component `(b,−a)`
            // is nonzero in (b for a non-vertical line, else −a for a vertical one).
            let s_start_to_end = if b.sign() != 0 {
                ord_i8(end.x.cmp(&start.x)) * b.sign()
            } else {
                ord_i8(end.y.cmp(&start.y)) * a.neg().sign()
            };
            let sgn = if from_start {
                s_start_to_end
            } else {
                -s_start_to_end
            };
            let f = Rat::from_i128(sgn as i128);
            Outgoing {
                dx: Surd::from_rat(b.mul(&f)),
                dy: Surd::from_rat(a.neg().mul(&f)),
                curv_sign: 0,
                r2: Rat::from_i128(0),
            }
        }
        Edge::Arc(arc) => {
            let (v, w) = if from_start {
                (&arc.start, &arc.end)
            } else {
                (&arc.end, &arc.start)
            };
            let nx =
                v.x.sub(&Surd::from_rat(arc.circle.cx.clone()))
                    .unwrap_surd();
            let ny =
                v.y.sub(&Surd::from_rat(arc.circle.cy.clone()))
                    .unwrap_surd();
            // is `v` the smaller-x endpoint of this x-monotone piece?
            let v_is_left = v.x.cmp(&w.x) == Ordering::Less;
            let ccw = match (arc.half, v_is_left) {
                (Half::Upper, true) => false, // toward +x on the top: θ decreasing = CW
                (Half::Upper, false) => true,
                (Half::Lower, true) => true, // toward +x on the bottom: θ increasing = CCW
                (Half::Lower, false) => false,
            };
            let (dx, dy) = if ccw {
                (ny.neg(), nx) // t_ccw = (−n_y, n_x)
            } else {
                (ny, nx.neg()) // t_cw  = ( n_y, −n_x)
            };
            Outgoing {
                dx,
                dy,
                curv_sign: if ccw { 1 } else { -1 },
                r2: arc.circle.r2.clone(),
            }
        }
    }
}

/// The half-plane class of a direction, keyed so the linear order runs CCW from the
/// `+x` ray: `+x (0) < upper (1) < −x (2) < lower (3)`.
fn phase<B: Backend>(dx: &Surd<B>, dy: &Surd<B>) -> u8 {
    match dy.sign() {
        1 => 1,  // upper half
        -1 => 3, // lower half
        _ => {
            if dx.sign() >= 0 {
                0 // +x ray (dy = 0, dx ≥ 0)
            } else {
                2 // −x ray
            }
        }
    }
}

/// `dx_a·dy_b − dy_a·dx_b`; both directions live at one vertex, so the products
/// stay in a single radical (mirrors [`super::classify`]'s `cross`).
fn cross_sign<B: Backend>(a: &Outgoing<B>, b: &Outgoing<B>) -> i8 {
    a.dx.mul(&b.dy)
        .unwrap_surd()
        .sub(&a.dy.mul(&b.dx).unwrap_surd())
        .unwrap_surd()
        .sign()
}

/// Tie-break between two half-edges leaving a vertex in the **same** direction, by
/// signed curvature `κ = curv_sign / r`: `a < b` (Less) iff `κ_a < κ_b`. A segment
/// (`κ = 0`) sits between a right-bending arc (`κ < 0`) and a left-bending arc
/// (`κ > 0`), matching how the edges deviate just past the vertex.
fn curv_cmp<B: Backend>(a: &Outgoing<B>, b: &Outgoing<B>) -> Ordering {
    if a.curv_sign != b.curv_sign {
        return a.curv_sign.cmp(&b.curv_sign);
    }
    match a.curv_sign {
        0 => Ordering::Equal, // both straight, same direction (a coincidence)
        s => {
            // κ = s/r; for s > 0 a smaller r² is a larger κ, for s < 0 the reverse.
            let by_r2 = a.r2.cmp(&b.r2);
            if s > 0 { by_r2.reverse() } else { by_r2 }
        }
    }
}

/// The CCW azimuth order of two outgoing tangents at a shared vertex, from the `+x`
/// ray. Half-plane split first; within a half the cross-product sign decides; equal
/// directions fall to the curvature tie-break — a strict total order over the
/// half-edges of one vertex (the DCEL rotation system).
pub fn dir_cmp<B: Backend>(a: &Outgoing<B>, b: &Outgoing<B>) -> Ordering {
    let (pa, pb) = (phase(&a.dx, &a.dy), phase(&b.dx, &b.dy));
    if pa != pb {
        return pa.cmp(&pb);
    }
    if pa == 1 || pa == 3 {
        // same half: a is CCW-before b iff the cross product is positive.
        match cross_sign(a, b) {
            s if s > 0 => return Ordering::Less,
            s if s < 0 => return Ordering::Greater,
            _ => {} // exactly parallel → tie-break
        }
    }
    // on the ±x axis (phases 0/2 are a single direction each) or exactly parallel.
    curv_cmp(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use geom::content::{ArcPiece, Circle, CurveId, Line, Orient, Point2, SegPiece, Winding};
    use lattice::Bignum;

    type Q = Rat<Bignum>;
    type S = Surd<Bignum>;
    type P = Point2<Bignum>;

    fn rp(x: i128, y: i128) -> P {
        Point2::from_rat(Q::from_i128(x), Q::from_i128(y))
    }
    /// A raw rational direction (curv 0), for the pure direction-order tests.
    fn dir(dx: i128, dy: i128) -> Outgoing<Bignum> {
        Outgoing {
            dx: S::from_rat(Q::from_i128(dx)),
            dy: S::from_rat(Q::from_i128(dy)),
            curv_sign: 0,
            r2: Q::from_i128(0),
        }
    }
    fn seg(sx: i128, sy: i128, ex: i128, ey: i128) -> Edge<Bignum> {
        // line through (sx,sy)-(ex,ey): a·x+b·y+c=0 with (a,b) = (−(ey−sy), ex−sx).
        let (a, b) = (Q::from_i128(-(ey - sy)), Q::from_i128(ex - sx));
        let c = a
            .mul(&Q::from_i128(sx))
            .add(&b.mul(&Q::from_i128(sy)))
            .neg();
        Edge::Seg(Box::new(SegPiece {
            line: Line { a, b, c },
            start: rp(sx, sy),
            end: rp(ex, ey),
            orient: Orient::Ccw,
            source: CurveId(0),
        }))
    }
    fn arc(cx: i128, cy: i128, r2: i128, half: Half, start: P, end: P) -> Edge<Bignum> {
        Edge::Arc(Box::new(ArcPiece {
            circle: Circle {
                cx: Q::from_i128(cx),
                cy: Q::from_i128(cy),
                r2: Q::from_i128(r2),
            },
            half,
            x_lo: start.x.clone().min(end.x.clone()),
            x_hi: start.x.clone().max(end.x.clone()),
            start,
            end,
            winding: Winding {
                orient: Orient::Ccw,
                source_span: None,
            },
            source: CurveId(1),
        }))
    }

    #[test]
    fn dir_cmp_ccw_order() {
        // CCW fan from +x: +x < 45° < +y < 135° < −x < 225° < −y < 315°.
        let fan = [
            dir(1, 0),
            dir(1, 1),
            dir(0, 1),
            dir(-1, 1),
            dir(-1, 0),
            dir(-1, -1),
            dir(0, -1),
            dir(1, -1),
        ];
        for w in fan.windows(2) {
            assert_eq!(dir_cmp(&w[0], &w[1]), Ordering::Less);
            assert_eq!(dir_cmp(&w[1], &w[0]), Ordering::Greater);
        }
        assert_eq!(dir_cmp(&dir(2, 0), &dir(5, 0)), Ordering::Equal); // same +x ray
    }

    #[test]
    fn outgoing_segment_points_toward_far_end() {
        // segment (−1,0)→(1,0): from start points +x, from end points −x.
        let e = seg(-1, 0, 1, 0);
        let from_s = outgoing_tangent(&e, true);
        let from_e = outgoing_tangent(&e, false);
        assert_eq!(phase(&from_s.dx, &from_s.dy), 0); // +x
        assert_eq!(phase(&from_e.dx, &from_e.dy), 2); // −x
        assert_eq!(from_s.curv_sign, 0);
        // a vertical segment (0,-1)→(0,1): from start points +y, from end −y.
        let v = seg(0, -1, 0, 1);
        assert_eq!(
            phase(
                &outgoing_tangent(&v, true).dx,
                &outgoing_tangent(&v, true).dy
            ),
            1
        );
        assert_eq!(
            phase(
                &outgoing_tangent(&v, false).dx,
                &outgoing_tangent(&v, false).dy
            ),
            3
        );
    }

    #[test]
    fn outgoing_arc_semicircle_both_ends_vertical() {
        // upper semicircle of the unit circle, L=(−1,0) → R=(1,0): the tangent at
        // either endpoint is vertical, pointing UP (into the upper half).
        let up = arc(0, 0, 1, Half::Upper, rp(-1, 0), rp(1, 0));
        for from_start in [true, false] {
            let t = outgoing_tangent(&up, from_start);
            assert_eq!(
                phase(&t.dx, &t.dy),
                1,
                "upper arc endpoint tangent points +y"
            );
            assert_eq!(t.dx.sign(), 0, "and is exactly vertical");
        }
        // lower semicircle: tangent points DOWN at either endpoint.
        let lo = arc(0, 0, 1, Half::Lower, rp(-1, 0), rp(1, 0));
        for from_start in [true, false] {
            let t = outgoing_tangent(&lo, from_start);
            assert_eq!(
                phase(&t.dx, &t.dy),
                3,
                "lower arc endpoint tangent points −y"
            );
        }
    }

    #[test]
    fn curvature_tie_break_right_lt_straight_lt_left() {
        // three half-edges leaving a vertex straight UP (0,1): a right-bending arc
        // (κ<0), a straight segment (κ=0), a left-bending arc (κ>0). CCW order just
        // past the vertex is right < straight < left.
        let mk = |curv_sign: i8| Outgoing::<Bignum> {
            dx: S::from_rat(Q::from_i128(0)),
            dy: S::from_rat(Q::from_i128(1)),
            curv_sign,
            r2: Q::from_i128(4),
        };
        let (right, straight, left) = (mk(-1), mk(0), mk(1));
        assert_eq!(dir_cmp(&right, &straight), Ordering::Less);
        assert_eq!(dir_cmp(&straight, &left), Ordering::Less);
        assert_eq!(dir_cmp(&right, &left), Ordering::Less);
        // same curvature sign, different radius: for κ>0 the tighter (smaller r²)
        // arc bends more, so it is the more-CCW (Greater).
        let tight = Outgoing::<Bignum> {
            r2: Q::from_i128(1),
            ..mk(1)
        };
        let wide = Outgoing::<Bignum> {
            r2: Q::from_i128(9),
            ..mk(1)
        };
        assert_eq!(dir_cmp(&wide, &tight), Ordering::Less);
    }

    #[test]
    fn rotation_preserves_cyclic_order() {
        // rotating every direction by the same rational rotation (3,4)/5 preserves
        // the CCW cyclic order (the linear cut-point moves, the cycle does not).
        let rot = |o: &Outgoing<Bignum>| {
            // (u,v)=(3,4): (dx,dy) ↦ (u·dx − v·dy, v·dx + u·dy).
            let (u, v) = (Q::from_i128(3), Q::from_i128(4));
            let dx = o.dx.scale(&u).sub(&o.dy.scale(&v)).unwrap_surd();
            let dy = o.dx.scale(&v).add(&o.dy.scale(&u)).unwrap_surd();
            Outgoing {
                dx,
                dy,
                curv_sign: 0,
                r2: Q::from_i128(0),
            }
        };
        let fan = [
            dir(1, 0),
            dir(1, 2),
            dir(0, 1),
            dir(-2, 1),
            dir(-1, 0),
            dir(0, -3),
            dir(2, -1),
        ];
        let mut i0: Vec<usize> = (0..fan.len()).collect();
        i0.sort_by(|&i, &j| dir_cmp(&fan[i], &fan[j]));
        let rotated: Vec<_> = fan.iter().map(rot).collect();
        let mut i1: Vec<usize> = (0..fan.len()).collect();
        i1.sort_by(|&i, &j| dir_cmp(&rotated[i], &rotated[j]));
        // i1 must be a cyclic rotation of i0.
        let n = i0.len();
        let is_cyclic_shift = (0..n).any(|k| (0..n).all(|t| i0[(k + t) % n] == i1[t]));
        assert!(
            is_cyclic_shift,
            "cyclic order changed: {:?} vs {:?}",
            i0, i1
        );
    }

    // --- property: dir_cmp is a strict total order at a vertex ---

    use crate::testgen::on_circle_pt;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// `dir_cmp` is antisymmetric and transitive over mixed directions — the
        /// invariant the DCEL rotation system relies on. Directions are taken from
        /// arcs (exact on-circle points) and rational segments at a shared vertex.
        #[test]
        fn dir_cmp_total_order(
            r in 1i128..=6,
            t1n in -6i128..=6, t1d in 1i128..=6,
            t2n in -6i128..=6, t2d in 1i128..=6,
            sx in -5i128..=5, sy in -5i128..=5,
        ) {
            // a shared vertex `V` on the circle centred at the origin, plus two
            // other on-circle points and a rational segment through `V`.
            let (zx, zy, rr) = (Q::from_i128(0), Q::from_i128(0), Q::from_i128(r));
            let vtx = on_circle_pt(&zx, &zy, &rr, 1, 1); // some fixed on-circle point
            let p1 = on_circle_pt(&zx, &zy, &rr, t1n, t1d);
            let p2 = on_circle_pt(&zx, &zy, &rr, t2n, t2d);
            prop_assume!(p1 != vtx && p2 != vtx);
            let a = outgoing_tangent(&arc(0, 0, r * r, Half::Upper, vtx.clone(), p1), true);
            let b = outgoing_tangent(&arc(0, 0, r * r, Half::Lower, vtx.clone(), p2), true);
            prop_assume!(!(sx == 0 && sy == 0));
            let c = outgoing_tangent(
                &seg(0, 0, sx, sy), // a rational direction (from the origin)
                true,
            );

            // antisymmetry
            prop_assert_eq!(dir_cmp(&a, &b), dir_cmp(&b, &a).reverse());
            prop_assert_eq!(dir_cmp(&a, &c), dir_cmp(&c, &a).reverse());
            prop_assert_eq!(dir_cmp(&b, &c), dir_cmp(&c, &b).reverse());
            // transitivity: a consistent sort of the three
            let mut v = [&a, &b, &c];
            v.sort_by(|x, y| dir_cmp(x, y));
            prop_assert!(dir_cmp(v[0], v[1]) != Ordering::Greater);
            prop_assert!(dir_cmp(v[1], v[2]) != Ordering::Greater);
            prop_assert!(dir_cmp(v[0], v[2]) != Ordering::Greater);
        }
    }
}
