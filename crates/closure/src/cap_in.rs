//! CAP-IN-D24 **searcher**: project a joint's flank charts into the cap plane and hand
//! the resulting boundary components to [`certify_core::cap_in::cap_in_d24`] for
//! licensing.
//!
//! `closure` is the untrusted searcher: it *proposes* a carrier and a parametrization
//! per component, and the pure-tier checker *decides*. A developable ruling projects to
//! a straight line ([`ruling_edge`]); a curved surface cut projects to a conic
//! ([`sigma_edge`]). The searcher's naive move is to guess a [`Carrier::Line`] through
//! a component's endpoints ([`line_through`]) — correct for a ruling, and correctly
//! **refused** ([`CapInFault::OffCarrier`](certify_core::cap_in::CapInFault::OffCarrier))
//! for a conic. That refusal is the
//! representation-level reason the M4 vertical slice is cylinder-first: a cylinder's
//! straight ruling images pass CAP-IN-D24, a cone's curved cut image does not.
//!
//! Nothing here keys on the flank *type* — the projection consumes an arbitrary
//! [`geom::chart::Chart`], and the carrier verdict falls out of the exact identity test
//! in the checker, never a Rust branch.

use certify_core::cap_in::{BoundaryComponent, Carrier, FlankId};
use geom::chart::Chart;
use lattice::{Backend, Bignum, Poly, Rat, RatFunc, Vec3Rat};

/// The cap plane's rational 2D frame: an origin `o` and two in-plane basis vectors `u`,
/// `v`. Projecting a space curve `P(t)` through it yields the plane parametrization
/// `x(t) = (P(t) − o)·u`, `y(t) = (P(t) − o)·v` that CAP-IN-D24 licenses.
///
/// In the full pipe the frame is derived from the joint's bisector `b_J` and collar
/// (C3); here it is supplied directly so the projection can be exercised in isolation.
pub struct PiFrame<B: Backend = Bignum> {
    /// The cap-plane origin `o`.
    pub origin: [Rat<B>; 3],
    /// The first in-plane basis vector — the `x` axis of the cap chart.
    pub u: [Rat<B>; 3],
    /// The second in-plane basis vector — the `y` axis.
    pub v: [Rat<B>; 3],
}

// Manual `Clone` (no `B: Clone` bound — `Backend` implementors are marker types, as in
// `geom::content`); `Rat`'s own unconditional `Clone` does the element work.
impl<B: Backend> Clone for PiFrame<B> {
    fn clone(&self) -> Self {
        PiFrame {
            origin: clone3(&self.origin),
            u: clone3(&self.u),
            v: clone3(&self.v),
        }
    }
}

fn clone3<B: Backend>(a: &[Rat<B>; 3]) -> [Rat<B>; 3] {
    [a[0].clone(), a[1].clone(), a[2].clone()]
}

/// Lift a constant point/vector to a [`Vec3Rat`] of constant components.
fn const_vec<B: Backend>(p: &[Rat<B>; 3]) -> Vec3Rat<B> {
    Vec3Rat::from_polys([
        Poly::constant(p[0].clone()),
        Poly::constant(p[1].clone()),
        Poly::constant(p[2].clone()),
    ])
}

/// Project a rational space curve `P(t)` into the cap plane: `((P − o)·u, (P − o)·v)`.
/// Both outputs are rational functions in the same parameter `t` as `curve` — the exact
/// composed parametrization the census tests against its carrier.
pub fn project<B: Backend>(curve: &Vec3Rat<B>, frame: &PiFrame<B>) -> (RatFunc<B>, RatFunc<B>) {
    let rel = curve.sub(&const_vec(&frame.origin));
    (rel.dot(&const_vec(&frame.u)), rel.dot(&const_vec(&frame.v)))
}

/// Project a concrete space point into the cap plane, as `((p − o)·u, (p − o)·v)`.
pub fn project_point<B: Backend>(p: &[Rat<B>; 3], frame: &PiFrame<B>) -> (Rat<B>, Rat<B>) {
    let d = [
        p[0].sub(&frame.origin[0]),
        p[1].sub(&frame.origin[1]),
        p[2].sub(&frame.origin[2]),
    ];
    let dot = |w: &[Rat<B>; 3]| d[0].mul(&w[0]).add(&d[1].mul(&w[1])).add(&d[2].mul(&w[2]));
    (dot(&frame.u), dot(&frame.v))
}

/// The searcher's line-carrier **guess** through two plane points: the directed line
/// `a·x + b·y + c = 0` with `a = y0 − y1`, `b = x1 − x0`, `c = −(a·x0 + b·y0)`. Exact
/// through both endpoints; whether the *whole* component lies on it is the checker's
/// call ([`certify_core::cap_in::on_carrier`]) — true for a ruling, false for a conic.
pub fn line_through<B: Backend>(p0: &(Rat<B>, Rat<B>), p1: &(Rat<B>, Rat<B>)) -> Carrier<B> {
    let a = p0.1.sub(&p1.1);
    let b = p1.0.sub(&p0.0);
    let c = a.mul(&p0.0).add(&b.mul(&p0.1)).neg();
    Carrier::Line { a, b, c }
}

/// A straight cap-boundary segment between two plane points, parametrized on `t ∈ [0, 1]`
/// as `p0 + t·(p1 − p0)` and carried by the line through them ([`line_through`]). The
/// building block of a polygonal cap cycle: a cylinder's ruling images and the shared
/// crease are all straight, so the LEDGE cap of the M4 slice is a polygon of these.
pub fn segment_edge<B: Backend>(
    p0: &(Rat<B>, Rat<B>),
    p1: &(Rat<B>, Rat<B>),
    flank: FlankId,
) -> BoundaryComponent<B> {
    let lin = |c0: &Rat<B>, c1: &Rat<B>| {
        RatFunc::from_poly(Poly::from_coeffs(vec![c0.clone(), c1.sub(c0)]))
    };
    BoundaryComponent {
        x: lin(&p0.0, &p1.0),
        y: lin(&p0.1, &p1.1),
        t_lo: Rat::from_i128(0),
        t_hi: Rat::from_i128(1),
        carrier: line_through(p0, p1),
        flank,
    }
}

/// Build the cap-boundary component from a flank's **ruling** at the crease station
/// `σ*`: the line `P(μ) = (c(σ*) + w·n(σ*)) + μ·r(σ*)` swept over `μ ∈ [mu_lo, mu_hi]`,
/// projected into the cap plane. A ruling is straight for *every* developable, so the
/// searcher's [`line_through`] guess is exact and the component passes CAP-IN-D24.
///
/// Returns `None` if the chart's `σ`-fields are singular at `σ*` (no rational ruling
/// there) — the searcher declines rather than fabricating a carrier.
pub fn ruling_edge<B: Backend>(
    chart: &Chart<B>,
    sigma_star: &Rat<B>,
    w: &Rat<B>,
    mu_lo: Rat<B>,
    mu_hi: Rat<B>,
    frame: &PiFrame<B>,
    flank: FlankId,
) -> Option<BoundaryComponent<B>> {
    let c0 = chart.pedal().eval(sigma_star)?;
    let r0 = chart.ruling().eval(sigma_star)?;
    let n0 = chart.normal().eval(sigma_star)?;
    // P(μ) = (c0 + w·n0) + μ·r0 — degree-1 in μ, one coefficient pair per axis.
    let base = |i: usize| c0[i].add(&n0[i].mul(w));
    let curve = Vec3Rat::from_polys([
        Poly::from_coeffs(vec![base(0), r0[0].clone()]),
        Poly::from_coeffs(vec![base(1), r0[1].clone()]),
        Poly::from_coeffs(vec![base(2), r0[2].clone()]),
    ]);
    let (x, y) = project(&curve, frame);
    let p0 = (x.eval(&mu_lo)?, y.eval(&mu_lo)?);
    let p1 = (x.eval(&mu_hi)?, y.eval(&mu_hi)?);
    let carrier = line_through(&p0, &p1);
    Some(BoundaryComponent {
        x,
        y,
        t_lo: mu_lo,
        t_hi: mu_hi,
        carrier,
        flank,
    })
}

/// Build the cap-boundary component from a flank's **surface cut** at fixed `μ`, `w`:
/// the `σ`-curve `surface(μ, w)` over `σ ∈ [sigma_lo, sigma_hi]`, projected into the cap
/// plane, with the searcher's [`line_through`] guess through the endpoints.
///
/// This is the honest failure path: a cylinder's cut is a circle and a cone's is a
/// non-circular conic, so the line guess is *wrong* — the checker refuses it with
/// [`CapInFault::OffCarrier`](certify_core::cap_in::CapInFault::OffCarrier). A correct
/// searcher would fit a [`Carrier::Circle`] where
/// one exists; deriving that (and rejecting the cone conic that fits neither) is the
/// C3/C4 work. Returns `None` if the surface is singular at an endpoint.
pub fn sigma_edge<B: Backend>(
    chart: &Chart<B>,
    mu: &Rat<B>,
    w: &Rat<B>,
    sigma_lo: Rat<B>,
    sigma_hi: Rat<B>,
    frame: &PiFrame<B>,
    flank: FlankId,
) -> Option<BoundaryComponent<B>> {
    let curve = chart.surface(mu, w);
    let (x, y) = project(&curve, frame);
    let p0 = (x.eval(&sigma_lo)?, y.eval(&sigma_lo)?);
    let p1 = (x.eval(&sigma_hi)?, y.eval(&sigma_hi)?);
    let carrier = line_through(&p0, &p1);
    Some(BoundaryComponent {
        x,
        y,
        t_lo: sigma_lo,
        t_hi: sigma_hi,
        carrier,
        flank,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use certify_core::Verdict;
    use certify_core::cap_in::{CapInFault, cap_in_d24, on_carrier};

    fn poly(cs: &[i128]) -> Poly<Bignum> {
        Poly::from_coeffs(cs.iter().map(|&c| Rat::from_i128(c)).collect())
    }
    /// A cylinder about the x-axis (`q = 1 + σi`) — straight ruling, circular cut.
    fn cylinder() -> Chart<Bignum> {
        Chart::new(
            [poly(&[1]), poly(&[0, 1]), poly(&[0]), poly(&[0])],
            RatFunc::zero(),
        )
    }
    /// A cone through the origin (`q = (9, 4, 4σ, 9σ)`) — a different developable class.
    fn cone() -> Chart<Bignum> {
        Chart::new(
            [poly(&[9]), poly(&[4]), poly(&[0, 4]), poly(&[0, 9])],
            RatFunc::zero(),
        )
    }
    /// The `xy`-plane frame: origin at the world origin, `u = x̂`, `v = ŷ`.
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

    #[test]
    fn a_cylinder_ruling_projects_to_a_line() {
        // A ruling is straight for every developable, so the searcher's line guess is
        // exact — the component lies on its claimed carrier.
        let edge = ruling_edge(
            &cylinder(),
            &Rat::from_i128(1),
            &Rat::from_i128(0),
            Rat::from_i128(-1),
            Rat::new(-1, 2),
            &xy_frame(),
            FlankId::A,
        )
        .expect("ruling non-singular at σ*=1");
        assert!(on_carrier(&edge));
    }

    #[test]
    fn a_cone_surface_cut_is_off_a_line_carrier() {
        // The cone's σ-cut is a genuine conic; the searcher's line guess through the
        // endpoints is refused by the exact identity test — falsely, not vacuously.
        // μ = 1, w = 0: the surface sweeps a genuine (non-degenerate) conic away from
        // the apex, so the endpoints are distinct and the line guess is non-degenerate.
        let edge = sigma_edge(
            &cone(),
            &Rat::from_i128(1),
            &Rat::from_i128(0),
            Rat::from_i128(0),
            Rat::from_i128(1),
            &xy_frame(),
            FlankId::A,
        )
        .expect("surface non-singular on [0,1]");
        assert!(!on_carrier(&edge));
        assert!(matches!(
            cap_in_d24(std::slice::from_ref(&edge)),
            Verdict::Refuted(CapInFault::OffCarrier { at: 0 })
        ));
    }

    #[test]
    fn a_projected_cylinder_polygon_mints_a_license() {
        let cyl = cylinder();
        let frame = xy_frame();
        // Four cylinder points at μ ∈ {0,1}, σ ∈ {0,1}, offset off the axis (w = 1) so the
        // xy-projection is non-degenerate — the corners of a straight-edged cap quad
        // (μ sweeps the ruling along x̂, σ sweeps the directrix into ŷ).
        let pt = |mu: i128, sigma: i128| {
            let p = cyl
                .surface(&Rat::from_i128(mu), &Rat::from_i128(1))
                .eval(&Rat::from_i128(sigma))
                .expect("cylinder surface is regular");
            project_point(&p, &frame)
        };
        let (a, b, c, d) = (pt(0, 0), pt(1, 0), pt(1, 1), pt(0, 1));
        // Distinct corners (else a line guess would degenerate).
        assert!(a != b && b != c && c != d && d != a);
        // A closed cycle spanning both flanks: A, A, B, then the crease closes it.
        let quad = [
            segment_edge(&a, &b, FlankId::A),
            segment_edge(&b, &c, FlankId::A),
            segment_edge(&c, &d, FlankId::B),
            segment_edge(&d, &a, FlankId::Crease),
        ];
        let d24 = match cap_in_d24(&quad) {
            Verdict::Verified(v) => v,
            other => panic!("cylinder polygon must license: {other:?}"),
        };
        assert_eq!(d24.len(), 4);
    }
}
