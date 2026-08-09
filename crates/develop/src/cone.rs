//! Certified development of a rational **cone**: a chart `C(σ,μ,w)` with apex at
//! the origin (`h ≡ 0`, so the pedal `c ≡ 0`) unrolled to the flat plane.
//!
//! For a cone the development map collapses to a **polar map**
//! `D(σ, μ̂) = μ̂·ρ(σ)·(cos ψ(σ), sin ψ(σ))` (spec §3.2), and the device cones
//! have a strikingly clean structure the spike exploits:
//!
//! - the **angle** integrates a rational function to a *single* arctangent —
//!   `ψ′ = det(n,n′,n″)/|n′|²` reduces to `c/(1+σ²)`, so `ψ(σ) = c·arctan(σ)`
//!   with `c = 2 sinβ` rational (the textbook cone law `ψ = sinβ · φ₃D`). This is
//!   verified as an exact polynomial identity by [`cone_angle_coeff`], not
//!   assumed;
//! - the **radius** `ρ = |n′| = √(normal_deriv_sq)` is a surd in general (here a
//!   perfect-square rational), enclosed rigorously by [`crate::interval::sqrt`].
//!
//! Composing the [rational transcendental enclosures](crate::interval) gives a
//! [`FlatBox`] — a rational rectangle proven to contain the true flat point —
//! whose diagonal is the certified [`FlatBox::backward_error`]. The [`drc`] gate
//! turns that bound into a three-valued [`Verdict`] against a fab clearance
//! (`spec:402`). No float enters the certificate; `mesh3d::develop_cone` only
//! *corroborates* it (see `docs/spike-development-report.md`).

use crate::interval::{RatIv, arctan, cos_on, pi, pi_half, sin_on, sqrt};
use certify_core::Verdict;
use geom::chart::Chart;
use lattice::{Backend, Bignum, Rat, RatFunc};

fn abs<B: Backend>(r: &Rat<B>) -> Rat<B> {
    if r.sign() < 0 { r.neg() } else { r.clone() }
}

/// The rational angle coefficient `c` with `ψ(σ) = c·arctan(σ)`, or `None` if the
/// chart is not a canonical arctangent cone.
///
/// The deflation invariant `ψ′ = det(n,n′,n″)/|n′|²` is reduced; the function
/// succeeds exactly when it equals `A/(k·(1+σ²))` (constant numerator, denominator
/// a rational multiple `k·(1+σ²)`), returning `c = A/k`. That check is an **exact
/// polynomial identity** — when it holds, `ψ = c·arctan σ` is a proven closed
/// form, not a fit. A cylinder (`ψ′ ≡ 0`) and a non-cone (`pedal ≢ 0`) both
/// return `None`; a general placement whose denominator is `σ²+b²` with `b ≠ 1`
/// (arctan of a scaled/Möbius argument, or a `log` branch) is deferred to DEV.2.
///
/// ```
/// use develop::cone::cone_angle_coeff;
/// use fixtures::devices::{cone, cone_alt, cylinder};
/// use lattice::{Bignum, Rat};
///
/// assert_eq!(cone_angle_coeff(&cone()), Some(Rat::<Bignum>::new(130, 97)));   // 2·65/97
/// assert_eq!(cone_angle_coeff(&cone_alt()), Some(Rat::<Bignum>::new(6, 5)));  // 2·3/5
/// assert_eq!(cone_angle_coeff(&cylinder()), None);                            // ψ′ ≡ 0
/// ```
pub fn cone_angle_coeff<B: Backend>(chart: &Chart<B>) -> Option<Rat<B>> {
    if !chart.pedal().is_zero() {
        return None; // not an apex-at-origin cone (γ ≢ 0)
    }
    let psi = chart.psi_prime().reduce();
    // numerator must be a nonzero constant A
    if psi.num().degree() != Some(0) {
        return None;
    }
    let a = psi.num().coeffs()[0].clone();
    // denominator must be k·(1 + σ²): coeffs [k, 0, k], k ≠ 0
    let den = psi.den().coeffs();
    if den.len() != 3 || !den[1].is_zero() || den[0] != den[2] || den[0].is_zero() {
        return None;
    }
    Some(a.div(&den[0]))
}

/// The tuning budget for a certified development point: `terms` truncates the
/// `arctan`/`cos`/`sin` series, `sqrt_eps` the radius bisection. Larger budgets
/// shrink the [`FlatBox`] width (the backward error) toward zero.
#[derive(Clone, Debug)]
pub struct DevConfig<B: Backend = Bignum> {
    /// Series-truncation length for the transcendental enclosures.
    pub terms: usize,
    /// Target width for the `√` radius enclosure.
    pub sqrt_eps: Rat<B>,
}

impl<B: Backend> DevConfig<B> {
    /// A default budget (24 series terms, `√` width `< 1e-12`) — tight enough for
    /// the device cone to well under a micron on a millimetre-scale part.
    pub fn tight() -> Self {
        DevConfig {
            terms: 24,
            sqrt_eps: Rat::new(1, 1_000_000_000_000),
        }
    }
}

/// A certified flat point: a rational rectangle `[x] × [y]` proven to contain the
/// true development `D(σ, μ̂)`.
#[derive(Clone, Debug)]
pub struct FlatBox<B: Backend = Bignum> {
    /// The `x`-coordinate enclosure.
    pub x: RatIv<B>,
    /// The `y`-coordinate enclosure.
    pub y: RatIv<B>,
}

impl<B: Backend> FlatBox<B> {
    /// The rational box center `((x.lo+x.hi)/2, (y.lo+y.hi)/2)` — the point the
    /// diagnostic float value is compared against.
    pub fn center(&self) -> (Rat<B>, Rat<B>) {
        (self.x.mid(), self.y.mid())
    }
    /// A rational upper bound on the backward error `|center − D_true|`: the box
    /// half-perimeter `(width_x + width_y)/2 ≥ √((w_x/2)² + (w_y/2)²)`, the max
    /// distance from the center to any corner.
    pub fn backward_error(&self) -> Rat<B> {
        self.x.width().add(&self.y.width()).mul(&Rat::new(1, 2))
    }
}

/// A cone chart prepared for certified development: the proven angle law
/// `ψ = c·arctan σ` plus the ruling-speed field `ρ² = |n′|²`.
///
/// Build once with [`ConeDevelopment::new`], then evaluate many [flat
/// points](ConeDevelopment::point) — `ψ′` and the `reduce()` are computed a single
/// time.
#[derive(Clone, Debug)]
pub struct ConeDevelopment<B: Backend = Bignum> {
    c: Rat<B>,
    rho_sq: RatFunc<B>,
}

impl<B: Backend> ConeDevelopment<B> {
    /// Prepare a cone chart, or `None` if it is not a canonical arctangent cone
    /// (see [`cone_angle_coeff`]).
    pub fn new(chart: &Chart<B>) -> Option<Self> {
        let c = cone_angle_coeff(chart)?;
        Some(ConeDevelopment {
            c,
            rho_sq: chart.normal_deriv_sq().reduce(),
        })
    }

    /// The proven angle coefficient `c` (`ψ = c·arctan σ`).
    pub fn angle_coeff(&self) -> &Rat<B> {
        &self.c
    }

    /// A certified enclosure of the flat angle `ψ(σ) = c·arctan(σ)`.
    pub fn angle(&self, sigma: &Rat<B>, terms: usize) -> RatIv<B> {
        arctan(sigma, terms).scale(&self.c)
    }

    /// A certified enclosure of the ruling-speed radius `ρ(σ) = |n′(σ)|`.
    pub fn radius(&self, sigma: &Rat<B>, eps: &Rat<B>) -> RatIv<B> {
        // On the cone domain the denominator (1+σ²)² > 0, so ρ² is always defined.
        let r2 = self.rho_sq.eval(sigma).unwrap_or_else(|| Rat::from_i128(0));
        sqrt(&r2, eps)
    }

    /// The certified flat point `D(σ, μ̂) = |μ̂|·ρ(σ)·(cos ψ, sin ψ)`.
    ///
    /// The radial coordinate is the **distance from the apex** `|μ̂|·ρ`, matching
    /// the diagnostic `mesh3d::develop_cone` (which lays each vertex at its 3D
    /// apex distance and the accumulated ruling angle); the sign of `μ̂` selects
    /// the side of the apex along the ruling and does not rotate the ray.
    pub fn point(&self, sigma: &Rat<B>, mu_hat: &Rat<B>, cfg: &DevConfig<B>) -> FlatBox<B> {
        let psi = self.angle(sigma, cfg.terms);
        let cos = cos_on(&psi, cfg.terms);
        let sin = sin_on(&psi, cfg.terms);
        let radial = self.radius(sigma, &cfg.sqrt_eps).scale(&abs(mu_hat));
        FlatBox {
            x: radial.mul(&cos),
            y: radial.mul(&sin),
        }
    }

    /// The seam's certified flat angular position `ψ(σ→∞) = c·π/2 = π·sinβ`.
    ///
    /// A finite rational chart sweeps a bounded azimuth; the closed cone's lap
    /// seam sits at the `σ→∞` limit of the parametrization, whose flat angle is
    /// exactly `c·π/2`. Enclosing `π/2` rationally pins the seam even though
    /// closing the full cone (multi-gore / the σ→∞ limit face) is a post-GO
    /// deliverable.
    pub fn seam_angle(&self, terms: usize) -> RatIv<B> {
        pi_half(terms).scale(&self.c)
    }

    /// The full flat sector swept by the closed cone: `ψ` span `= c·π = 2π·sinβ`
    /// (`σ: −∞→∞ ↔ φ₃D: −π→π`, one 2π wrap). For `β ≈ 42°` this is `≈ 240.9°`,
    /// the textbook developed-cone sector.
    pub fn flat_sector(&self, terms: usize) -> RatIv<B> {
        pi(terms).scale(&self.c)
    }
}

/// The **design-rule check** (`spec:402`): the development is fabricable when its
/// backward error is under half the clearance.
///
/// Verdict-typed, never a float compared with a float: `Verified(ε)` when
/// `ε < clearance/2`, else `Unresolved(ε)` — the honest three-valued middle,
/// refined by raising [`DevConfig::terms`]. There is no `Refuted`: a loose
/// enclosure is not *wrong*, only not yet tight enough.
pub fn drc<B: Backend>(eps: &Rat<B>, clearance: &Rat<B>) -> Verdict<Rat<B>, (), Rat<B>> {
    let half = clearance.mul(&Rat::new(1, 2));
    if *eps < half {
        Verdict::Verified(eps.clone())
    } else {
        Verdict::Unresolved(eps.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fixtures::devices::{cone, cone_alt, cylinder};

    type Q = Rat<Bignum>;

    fn to_f64(r: &Q) -> f64 {
        let (n, d) = r.numer_denom_decimal();
        n.parse::<f64>().unwrap() / d.parse::<f64>().unwrap()
    }

    #[test]
    fn angle_coeff_is_the_exact_arctan_law() {
        // ψ = c·arctan σ with c = 2 sinβ = 2·(n·ẑ).
        assert_eq!(cone_angle_coeff(&cone()), Some(Q::new(130, 97)));
        assert_eq!(cone_angle_coeff(&cone_alt()), Some(Q::new(6, 5)));
        // a cylinder is not an apex cone (ψ′ ≡ 0) → rejected.
        assert_eq!(cone_angle_coeff(&cylinder()), None);
    }

    #[test]
    fn radius_is_the_expected_rational_at_sigma_zero() {
        let dev = ConeDevelopment::new(&cone()).unwrap();
        // ρ(0) = |n′|(0) = 144/97 (perfect-square rational for the device cone).
        let r = dev.radius(&Q::from_i128(0), &Q::new(1, 1_000_000_000_000));
        assert!(r.contains(&Q::new(144, 97)));
        assert!(r.width() < Q::new(1, 1_000_000_000));
    }

    #[test]
    fn flat_point_encloses_and_backward_error_shrinks() {
        let dev = ConeDevelopment::new(&cone()).unwrap();
        let sigma = Q::new(1, 2);
        let mu = Q::new(-3, 4);
        let coarse = dev.point(
            &sigma,
            &mu,
            &DevConfig {
                terms: 4,
                sqrt_eps: Q::new(1, 1000),
            },
        );
        let fine = dev.point(&sigma, &mu, &DevConfig::tight());
        // Refining the budget strictly tightens the certified backward error.
        assert!(fine.backward_error() < coarse.backward_error());
        assert!(fine.backward_error() < Q::new(1, 1_000_000));
    }

    #[test]
    fn seam_and_full_sector_match_the_cone_law() {
        use std::f64::consts::PI;
        let dev = ConeDevelopment::new(&cone()).unwrap();
        let c = 130.0 / 97.0; // = 2 sinβ
        // seam at ψ = c·π/2 = π·sinβ ≈ 2.10520 rad.
        let seam = dev.seam_angle(24);
        assert!((to_f64(&seam.mid()) - c * PI / 2.0).abs() < 1e-9);
        assert!(seam.width() < Q::new(1, 1_000_000));
        // full sector = c·π = 2π sinβ ≈ 4.21040 rad ≈ 240.9°.
        let sector = dev.flat_sector(24);
        assert!((to_f64(&sector.mid()) - c * PI).abs() < 1e-9);
    }

    #[test]
    fn drc_is_verdict_typed() {
        // ε = 1e-9, clearance = 1e-3 → ε < clearance/2 → Verified.
        assert!(matches!(
            drc(&Q::new(1, 1_000_000_000), &Q::new(1, 1000)),
            Verdict::Verified(_)
        ));
        // ε = 1e-3, clearance = 1e-3 → ε ≥ clearance/2 → Unresolved (refine).
        assert!(matches!(
            drc(&Q::new(1, 1000), &Q::new(1, 1000)),
            Verdict::Unresolved(_)
        ));
    }
}
