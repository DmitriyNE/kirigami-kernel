//! `construct` — the geometry entry points (free functions returning a [`Part`]).
//!
//! Entry points are **free functions, not `Part` methods**: a part never enumerates surface
//! kinds, and future *solved* constructions (`loft` through given rails, `wrap` around a
//! reference body, mediator strips) land here as siblings without touching the recipe type.
//!
//! ```
//! use author::construct;
//! use lattice::{Bignum, Rat};
//!
//! // A right circular cone with half-angle ≈ 42° — the float is INTERPRETED and snapped to an
//! // exact rational chart (read the exact frame back off the recipe's report after evaluate).
//! let part = construct::cone::<Bignum>(42.0);
//! ```

use crate::part::Part;
use lattice::{Backend, Poly, Rat, RatFunc};

/// A right circular cone: apex at the origin, axis `ẑ`, the given **half-angle in degrees**
/// (an approximate product coordinate, snapped to a nearby exact rational cone — the same
/// doctrine as the azimuth snap). The exact chart is `q(σ) = (1, u, uσ, σ)` with
/// `u = tan(45° − β/2)` on the dyadic grid; its normal satisfies `n·ẑ = (1−u²)/(1+u²)` exactly
/// (a rational point on the circle — `sin β` snapped, not approximated downstream).
///
/// Azimuth follows the exact Stage-1 law `φ = 2·arctan σ`, so
/// [`region_azimuth`](Part::region_azimuth) authors regions in degrees directly.
pub fn cone<B: Backend>(half_angle_deg: f64) -> Part<B> {
    let u = ((45.0 - half_angle_deg / 2.0).to_radians()).tan();
    let uq: Rat<B> = export::approx::f64_to_rat(u, 30);
    let one = Poly::constant(Rat::from_i128(1));
    let q = [
        one.clone(),
        Poly::constant(uq.clone()),
        Poly::from_coeffs(vec![Rat::from_i128(0), uq]),
        Poly::from_coeffs(vec![Rat::from_i128(0), Rat::from_i128(1)]),
    ];
    Part::from_frame(q, RatFunc::zero())
}

/// A part over an arbitrary `(q, h)` chart — the general exact entry (fixtures, folded frames,
/// reparametrized charts). The chart's own support becomes the
/// [`SupportFn::inherit`](crate::part::SupportFn::inherit) target; whether each region's chart
/// actually develops is validated at evaluate time ([`PartFault::NotDevelopable`]).
///
/// [`PartFault::NotDevelopable`]: crate::part::PartFault::NotDevelopable
pub fn from_chart<B: Backend>(chart: &geom::chart::Chart<B>) -> Part<B> {
    Part::from_frame(chart.quaternion().clone(), chart.support().clone())
}
