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

use crate::interval::{RatIv, arctan, arctan_on, cos_on, log, pi, pi_half, sin_on, sqrt};
use certify_core::Verdict;
use geom::chart::Chart;
use lattice::{Backend, Bignum, Poly, Rat, RatFunc};

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

/// Why the general closed-form angle integrator deferred — the honest `Unresolved`
/// middle of [`angle_enclosure`]. The reduced `ψ′ = P/Q` fell outside the degree-2
/// positive-definite denominator core that DEV.2b certifies; each variant points at
/// the flagged partial-fractions-over-[`AlgReal`](lattice) extension (DEV.2b/DEV.3),
/// **not** at a wrong result. An angle enclosure is never *refuted*: a chart whose
/// `ψ′` we cannot yet close is undecided, not disproved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AngleDefer {
    /// `Q` reduced to a denominator of degree `d ∉ {0, 2}` (e.g. `1`, or `≥ 3`):
    /// its closed form needs partial fractions over the algebraic roots of `Q`
    /// (`lattice::AlgReal`, the Curved MITER-FIT machinery) — not yet wired in.
    DenominatorDegree(usize),
    /// `Q` is a degree-2 polynomial with real roots (discriminant `≥ 0`): the
    /// integration path `[0, σ]` may cross a pole, so the elementary `arctan`/`log`
    /// closed form of the positive-definite case does not apply. Needs the
    /// real-root `log|σ − r|` path with pole-crossing checks — deferred.
    RealRoots,
    /// The surd radius `q₀ = √(q₀²)` could not be signed strictly positive at the
    /// configured `sqrt_eps` (its enclosure still touches zero), so `1/q₀` is not
    /// yet bounded. Refine the [`DevConfig::sqrt_eps`] budget.
    RadiusNotSigned,
}

/// The exact rational `∫₀^σ M(s) ds` for a polynomial `M` — the antiderivative
/// `Σ Mᵢ·σⁱ⁺¹/(i+1)` evaluated at `σ` (the lower limit `σ = 0` contributes 0).
fn integrate_poly<B: Backend>(m: &Poly<B>, sigma: &Rat<B>) -> Rat<B> {
    let mut acc = Rat::from_i128(0);
    let mut pow = sigma.clone(); // σⁱ⁺¹, i = 0 → σ
    for (i, mi) in m.coeffs().iter().enumerate() {
        acc = acc.add(&mi.mul(&pow).div(&Rat::from_i128((i + 1) as i128)));
        pow = pow.mul(sigma);
    }
    acc
}

/// A certified enclosure of `ψ(σ) = ∫₀^σ (P/Q) dσ` for the reduced rational
/// `ψ′ = P/Q`, over the **degree-2 positive-definite** denominator core.
///
/// Splits `P = M·Q + R` (`deg R < deg Q`); `∫M` is exact-rational, and for a
/// degree-2 `Q` with no real roots (`disc < 0`) completes the square
/// `Q = A[(σ − p₀)² + q₀²]` (`q₀² = −disc/(4A²) > 0`) so that
/// `∫(aσ+b)/Q = (a/2A)·log((σ−p₀)²+q₀²) + ((ap₀+b)/(Aq₀))·arctan((σ−p₀)/q₀)`,
/// both enclosed rigorously (`log`/`arctan_on`; `q₀` a surd via `sqrt`). Anything
/// outside that core is a clean [`AngleDefer`], never a silent failure.
fn integrate_ratfunc<B: Backend>(
    psi_prime: &RatFunc<B>,
    sigma: &Rat<B>,
    cfg: &DevConfig<B>,
) -> Verdict<RatIv<B>, (), AngleDefer> {
    let psi = psi_prime.reduce();
    let p = psi.num();
    let q = psi.den();
    // ψ′ ≡ 0 (a cylinder): the flat angle is identically zero, exactly.
    if p.is_zero() {
        return Verdict::Verified(RatIv::point(Rat::from_i128(0)));
    }
    let qdeg = match q.degree() {
        Some(d) => d,
        None => return Verdict::Unresolved(AngleDefer::DenominatorDegree(0)),
    };
    // Polynomial part P = M·Q + R (deg R < deg Q); ∫₀^σ M is exact-rational.
    let (m_poly, r_poly) = p.divrem(q);
    let poly_part = integrate_poly(&m_poly, sigma);
    if qdeg != 2 {
        return Verdict::Unresolved(AngleDefer::DenominatorDegree(qdeg));
    }
    // Q = A·σ² + B·σ + C, coefficients low → high.
    let qc = q.coeffs();
    let (cc, bb, aa) = (qc[0].clone(), qc[1].clone(), qc[2].clone());
    // disc = B² − 4AC; real roots (disc ≥ 0) are deferred.
    let disc = bb.mul(&bb).sub(&aa.mul(&cc).mul(&Rat::from_i128(4)));
    if disc.sign() >= 0 {
        return Verdict::Unresolved(AngleDefer::RealRoots);
    }
    // Complete the square: p₀ = −B/(2A), q₀² = −disc/(4A²) > 0.
    let two_a = aa.mul(&Rat::from_i128(2));
    let p0 = bb.neg().div(&two_a);
    let q0_sq = disc.neg().div(&two_a.mul(&two_a));
    let q0 = sqrt(&q0_sq, &cfg.sqrt_eps);
    let inv_q0 = match q0.recip_pos() {
        Some(iv) => iv,
        None => return Verdict::Unresolved(AngleDefer::RadiusNotSigned),
    };
    // R = a·σ + b (deg R < 2).
    let rc = r_poly.coeffs();
    let a = rc.get(1).cloned().unwrap_or_else(|| Rat::from_i128(0));
    let b = rc.first().cloned().unwrap_or_else(|| Rat::from_i128(0));
    // log part: (a/2A)·[ln((σ−p₀)²+q₀²) − ln(p₀²+q₀²)] — both arguments are rational.
    let coeff_log = a.div(&two_a);
    let sp = sigma.sub(&p0);
    let arg_hi = sp.mul(&sp).add(&q0_sq);
    let arg_lo = p0.mul(&p0).add(&q0_sq);
    let log_part = log(&arg_hi, cfg.terms)
        .sub(&log(&arg_lo, cfg.terms))
        .scale(&coeff_log);
    // arctan part: ((a·p₀+b)/A)·(1/q₀)·[arctan((σ−p₀)/q₀) − arctan(−p₀/q₀)].
    let coeff_at = a.mul(&p0).add(&b).div(&aa);
    let full_at = RatIv::point(coeff_at).mul(&inv_q0);
    let up = RatIv::point(sp).mul(&inv_q0);
    let dn = RatIv::point(p0.neg()).mul(&inv_q0);
    let at_part = full_at.mul(&arctan_on(&up, cfg.terms).sub(&arctan_on(&dn, cfg.terms)));
    let total = RatIv::point(poly_part)
        .add(&log_part)
        .add(&at_part)
        .rounded();
    Verdict::Verified(total)
}

/// A certified enclosure of the development angle `ψ(σ) = ∫₀^σ ψ′` for **any**
/// chart whose reduced `ψ′ = det(n,n′,n″)/|n′|²` has a degree-2 positive-definite
/// denominator — cones at *any* placement/parametrization, not just the canonical
/// `c/(1+σ²)` fast path of [`cone_angle_coeff`].
///
/// The general closed-form angle (DEV.2b): where [`ConeDevelopment::angle`] handles
/// only the origin-canonical single-arctan law, this integrates the rational `ψ′`
/// through the complete-the-square arctan/log formula, so a shifted or reparametrized
/// cone (whose `Q` is a shifted quadratic `σ²+bσ+c`) certifies too. Returns a
/// [`Verdict`]: `Verified` with the enclosure, or a clean [`AngleDefer`] deferral for
/// the higher-degree / real-root cases that need the `AlgReal` extension.
///
/// ```
/// use develop::cone::{angle_enclosure, DevConfig};
/// use fixtures::devices::{cone, cylinder};
/// use certify_core::Verdict;
///
/// // A cylinder has ψ′ ≡ 0, so its flat angle is exactly 0.
/// let cyl = angle_enclosure(&cylinder(), &lattice::Rat::from_i128(1), &DevConfig::tight());
/// assert!(matches!(cyl, Verdict::Verified(ref iv) if iv.contains(&lattice::Rat::from_i128(0))));
///
/// // The device cone matches the DEV.1 arctan law ψ = (130/97)·arctan σ.
/// let ang = angle_enclosure(&cone(), &lattice::Rat::from_i128(1), &DevConfig::tight());
/// assert!(matches!(ang, Verdict::Verified(_)));
/// ```
pub fn angle_enclosure<B: Backend>(
    chart: &Chart<B>,
    sigma: &Rat<B>,
    cfg: &DevConfig<B>,
) -> Verdict<RatIv<B>, (), AngleDefer> {
    integrate_ratfunc(&chart.psi_prime(), sigma, cfg)
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
        arctan(sigma, terms).scale(&self.c).rounded()
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
        let radial = self
            .radius(sigma, &cfg.sqrt_eps)
            .scale(&abs(mu_hat))
            .rounded();
        FlatBox {
            x: radial.mul(&cos).rounded(),
            y: radial.mul(&sin).rounded(),
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
        pi_half(terms).scale(&self.c).rounded()
    }

    /// The full flat sector swept by the closed cone: `ψ` span `= c·π = 2π·sinβ`
    /// (`σ: −∞→∞ ↔ φ₃D: −π→π`, one 2π wrap). For `β ≈ 42°` this is `≈ 240.9°`,
    /// the textbook developed-cone sector.
    pub fn flat_sector(&self, terms: usize) -> RatIv<B> {
        pi(terms).scale(&self.c).rounded()
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

    fn poly(cs: &[i128]) -> Poly<Bignum> {
        Poly::from_coeffs(cs.iter().map(|&c| Q::from_i128(c)).collect())
    }

    #[test]
    fn angle_enclosure_matches_arctan_law_on_device_cones() {
        // The general integrator reproduces the DEV.1 closed form ψ = c·arctan σ on the
        // canonical cones (c = 130/97 and 6/5) across the gore, *without* the fast-path
        // recognizer — the float value is the independent oracle, no float in the cert.
        for (chart, c) in [(cone(), 130.0 / 97.0), (cone_alt(), 6.0 / 5.0)] {
            for &s in &[0.0f64, 0.25, 0.5, 0.75, 1.0] {
                let sigma = Q::new((s * 1000.0) as i128, 1000);
                match angle_enclosure(&chart, &sigma, &DevConfig::tight()) {
                    Verdict::Verified(iv) => {
                        let want = c * s.atan();
                        assert!(
                            to_f64(iv.lo()) - 1e-9 <= want && want <= to_f64(iv.hi()) + 1e-9,
                            "ψ enclosure must contain c·arctan σ at σ={s}"
                        );
                        assert!(iv.width() < Q::new(1, 1_000_000), "tight at σ={s}");
                    }
                    other => panic!("device cone angle must certify, got {other:?}"),
                }
            }
        }
    }

    #[test]
    fn angle_enclosure_certifies_a_shifted_placement_cone() {
        use std::f64::consts::FRAC_PI_4;
        // A reparametrized device cone q̃(σ) = q_device(σ−1): the same apex-origin cone,
        // but ψ′ = (130/97)/(σ²−2σ+2) — a *shifted* quadratic denominator. The canonical
        // recognizer declines (denominator ≠ k·(1+σ²)); the general integrator certifies
        // ψ(σ) = (130/97)·(arctan(σ−1) + π/4).
        let shifted = Chart::new(
            [poly(&[9]), poly(&[4]), poly(&[-4, 4]), poly(&[-9, 9])],
            RatFunc::zero(),
        );
        assert_eq!(cone_angle_coeff(&shifted), None, "fast path must decline");
        let c = 130.0 / 97.0;
        for &s in &[0.0f64, 0.5, 1.0, 1.5, 2.0] {
            let sigma = Q::new((s * 1000.0) as i128, 1000);
            match angle_enclosure(&shifted, &sigma, &DevConfig::tight()) {
                Verdict::Verified(iv) => {
                    let want = c * ((s - 1.0).atan() + FRAC_PI_4);
                    assert!(
                        to_f64(iv.lo()) - 1e-9 <= want && want <= to_f64(iv.hi()) + 1e-9,
                        "shifted-cone ψ enclosure must contain the closed form at σ={s}"
                    );
                    assert!(iv.width() < Q::new(1, 1_000_000), "tight at σ={s}");
                }
                other => panic!("shifted cone angle must certify, got {other:?}"),
            }
        }
    }

    #[test]
    fn integrate_ratfunc_log_branch_on_constructed_integrand() {
        // ∫₀^σ s/(1+s²) ds = ½·ln(1+σ²) — a *log*-only closed form (numerator degree 1),
        // exercising the branch cones themselves never reach (a cone's ψ′ has a constant
        // numerator ⇒ pure arctan). Constructed directly, since no cone yields it.
        let integrand = RatFunc::new(poly(&[0, 1]), poly(&[1, 0, 1])); // σ / (1+σ²)
        for &s in &[0.5f64, 1.0, 2.0, 3.0] {
            let sigma = Q::new((s * 1000.0) as i128, 1000);
            match integrate_ratfunc(&integrand, &sigma, &DevConfig::tight()) {
                Verdict::Verified(iv) => {
                    let want = 0.5 * (1.0 + s * s).ln();
                    assert!(
                        to_f64(iv.lo()) - 1e-9 <= want && want <= to_f64(iv.hi()) + 1e-9,
                        "½·ln(1+σ²) enclosure must contain the closed form at σ={s}"
                    );
                }
                other => panic!("log-branch integrand must certify, got {other:?}"),
            }
        }
    }

    #[test]
    fn angle_enclosure_defers_outside_the_degree2_core() {
        // The honest `Unresolved` middle: outside the degree-2 positive-definite core the
        // integrator defers (never a wrong `Verified`, never a bare `Refuted`).
        let cfg = DevConfig::tight();
        let one = Q::from_i128(1);
        // Degree-3 denominator → needs AlgReal partial fractions (the flagged extension).
        let cubic = RatFunc::new(poly(&[1]), poly(&[1, 0, 0, 1])); // 1/(1+σ³)
        assert!(matches!(
            integrate_ratfunc(&cubic, &one, &cfg),
            Verdict::Unresolved(AngleDefer::DenominatorDegree(3))
        ));
        // Degree-2 with real roots (σ²−1, disc = 4 > 0) → pole-crossing path, deferred.
        let real_roots = RatFunc::new(poly(&[1]), poly(&[-1, 0, 1])); // 1/(σ²−1)
        assert!(matches!(
            integrate_ratfunc(&real_roots, &one, &cfg),
            Verdict::Unresolved(AngleDefer::RealRoots)
        ));
    }
}
