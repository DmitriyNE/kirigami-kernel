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

use crate::interval::{
    RatIv, abs_on, arctan, arctan_on, cos_on, eval_ratfunc_on, integrate_on_slope, log, pi,
    pi_half, sin_on, sqrt, sqrt_on,
};
use certify_core::Verdict;
use core::cell::RefCell;
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
    arctan_coeff(&chart.psi_prime())
}

/// The rational `c` with `ψ = c·arctan σ` extracted from `ψ′ = c/(1+σ²)`, when `ψ′` has that
/// canonical shape (constant numerator `A`, denominator a rational multiple `k·(1+σ²)`), else
/// `None`. The check is an exact polynomial identity.
///
/// Unlike [`cone_angle_coeff`] this does **not** require an apex cone (`pedal ≡ 0`): the angle law
/// is `h`-independent — `ψ′` reads only `n, n′, n″` (spec §3.2), all from `q` — so a curved-support
/// developable that shares the cone's Gauss circle (the seam ramp, `cone_seam_ramp`) has the same
/// `c·arctan σ` angle; only its flat *directrix* `γ ≠ 0`. [`ConeDevelopment::new_developable`] uses
/// this to admit that ramp, where [`cone_angle_coeff`]'s pedal gate turns it away.
fn arctan_coeff<B: Backend>(psi_prime: &RatFunc<B>) -> Option<Rat<B>> {
    let psi = psi_prime.reduce();
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
#[derive(Debug)]
pub struct DevConfig<B: Backend = Bignum> {
    /// Series-truncation length for the transcendental enclosures.
    pub terms: usize,
    /// Target width for the `√` radius enclosure.
    pub sqrt_eps: Rat<B>,
}

// Hand-written so `B` need not be `Clone` (the backend markers are not), like `RatIv`.
impl<B: Backend> Clone for DevConfig<B> {
    fn clone(&self) -> Self {
        DevConfig {
            terms: self.terms,
            sqrt_eps: self.sqrt_eps.clone(),
        }
    }
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
#[derive(Debug)]
pub struct ConeDevelopment<B: Backend = Bignum> {
    c: Rat<B>,
    rho_sq: RatFunc<B>,
    /// `(ρ²)′` — the exact derivative of the ruling-speed field, precomputed for the flat directrix
    /// *acceleration* `γ″` (the slope-remainder integrand of [`integrate_on_slope`]); unused when
    /// `γ ≡ 0`. `ρ′ = (ρ²)′/(2ρ)` (spec §Tier C).
    rho_sq_prime: RatFunc<B>,
    /// `Some` for a curved-support developable (`γ ≠ 0`); `None` for the apex cone (`γ ≡ 0`), whose
    /// development is the byte-identical polar map [`ConeDevelopment::new`] produced before DD.2.
    directrix: Option<Directrix<B>>,
    /// Quadrature budget for the flat directrix `γ` (subintervals of `[0, σ]`); unused when `γ ≡ 0`.
    panels: usize,
    /// Memoized prefix sums of `γ` (OPT.1) — see [`DirectrixPrefix`]. Purely a cache: every value
    /// it returns is a sum of the same per-panel enclosures the direct quadrature would build, so
    /// it changes cost, not soundness. Not part of the value's identity, hence skipped by `Clone`
    /// (a clone starts cold) and by any comparison.
    tables: RefCell<Vec<DirectrixPrefix<B>>>,
}

/// How many origins are memoized at once. The development is queried from the region's own `σ`
/// window (`directrix_between`) *and* from 0 (`directrix_at`), and a fold alternates between them,
/// so a single-entry cache would thrash — rebuilding on every call and costing more than no cache
/// at all. Four covers the acceptance device's three regions plus the origin.
const GAMMA_TABLES: usize = 4;

/// How far past its reach a table may be extended before it is cheaper to re-anchor. Bounds the
/// pathological case where the first query lands very close to `lo` (making the step tiny) and a
/// later one lands far away.
const GAMMA_MAX_CELLS: usize = 8;

/// Prefix sums of the flat directrix over a uniform grid anchored at `lo`.
///
/// `γ` is an *integral*, so it is additive: `γ(σ) = Σ_{k<j} ∫_{cell k} γ′ + ∫_{grid_j}^{σ} γ′`.
/// The direct quadrature recomputed the whole thing from `lo` on every query — `N` queries at
/// `panels` subintervals each cost `N·panels` integrand evaluations, and since a boundary or a
/// bisection asks for many `σ` against one `lo`, that product was the dominant cost of the whole
/// pipeline (OPT.0). Accumulating the grid once turns it into `cells + N`.
///
/// **Soundness is unaffected**: summing adjacent panel enclosures *is* the composite rule, so a
/// prefix lookup plus one partial panel is a valid enclosure of the same integral. **Accuracy is
/// comparable, not identical** — the partition differs. The grid step is set by the first query as
/// `(σ₁ − lo)/panels`, which makes the error at `σ₁` exactly the direct rule's; farther queries get
/// *more* cells than the direct rule would have used (so they are tighter), nearer ones fewer (so
/// they are looser, but bounded by the error at `σ₁`). Since the reported ε is a maximum over the
/// queried points, the worst case is preserved — the ε pins on the acceptance parts are what hold
/// this claim honest.
struct DirectrixPrefix<B: Backend> {
    /// The integration origin this table is anchored at.
    lo: Rat<B>,
    /// Grid spacing.
    step: Rat<B>,
    /// `prefix[j] = ∫_lo^{lo + j·step} γ′`, so `prefix[0] = [0, 0]` and `prefix.len() - 1` cells
    /// have been accumulated.
    prefix: Vec<[RatIv<B>; 2]>,
}

impl<B: Backend> core::fmt::Debug for DirectrixPrefix<B> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DirectrixPrefix")
            .field("cells", &(self.prefix.len() - 1))
            .finish()
    }
}

/// The exact rational data of a curved-support developable's flat directrix `γ` (the `γ ≠ 0` case):
/// the pedal-velocity dot products `c′·r` and `c′·n′`, the numerators of the flat-frame resolution
/// `a = (c′·r)/ρ`, `b = −(c′·n′)/ρ` (spec §Tier C — the development maps the positively oriented
/// tangent pair `(r/ρ, −n′/ρ)` to `(e(ψ), e⊥(ψ))`). `ρ = √ρ²` is the shared surd atom.
#[derive(Debug)]
struct Directrix<B: Backend> {
    cr: RatFunc<B>,       // c′·r
    cn: RatFunc<B>,       // c′·n′
    cr_prime: RatFunc<B>, // (c′·r)′  — for the acceleration γ″
    cn_prime: RatFunc<B>, // (c′·n′)′ — for the acceleration γ″
}

// Hand-written so `B` need not be `Clone` (the backend markers are not).
impl<B: Backend> Clone for Directrix<B> {
    fn clone(&self) -> Self {
        Directrix {
            cr: self.cr.clone(),
            cn: self.cn.clone(),
            cr_prime: self.cr_prime.clone(),
            cn_prime: self.cn_prime.clone(),
        }
    }
}

// Hand-written so `B` need not be `Clone` (the backend markers are not), like `RatIv`.
impl<B: Backend> Clone for ConeDevelopment<B> {
    fn clone(&self) -> Self {
        ConeDevelopment {
            c: self.c.clone(),
            rho_sq: self.rho_sq.clone(),
            rho_sq_prime: self.rho_sq_prime.clone(),
            directrix: self.directrix.clone(),
            panels: self.panels,
            tables: RefCell::new(Vec::new()),
        }
    }
}

impl<B: Backend> ConeDevelopment<B> {
    /// Prepare a cone chart, or `None` if it is not a canonical arctangent cone
    /// (see [`cone_angle_coeff`]). The apex-cone development (`γ ≡ 0`).
    pub fn new(chart: &Chart<B>) -> Option<Self> {
        let c = cone_angle_coeff(chart)?;
        let rho_sq = chart.normal_deriv_sq().reduce();
        let rho_sq_prime = rho_sq.derivative();
        Some(ConeDevelopment {
            c,
            rho_sq,
            rho_sq_prime,
            directrix: None,
            panels: 0,
            tables: RefCell::new(Vec::new()),
        })
    }

    /// Prepare a **curved-support** developable for certified development — the `γ ≠ 0` case (DD.2 /
    /// DEV.3 method b). Admits a chart whose angle law is `ψ = c·arctan σ` ([`arctan_coeff`]
    /// succeeds — it shares the cone's Gauss circle) but whose pedal `c(σ) ≠ 0`, so the flat pattern
    /// gains a *directrix* `γ(σ) = ∫₀^σ [a·e(ψ) + b·e⊥(ψ)]`, `a = (c′·r)/ρ`, `b = −(c′·n′)/ρ`,
    /// enclosed by validated quadrature over `panels` subintervals. The apex cone (`pedal ≡ 0`) gets
    /// `directrix = None` and develops **identically** to [`new`]. `None` if the chart's angle law is
    /// not the shared-Gauss-circle `c·arctan σ`.
    ///
    /// ```
    /// use develop::cone::{ConeDevelopment, DevConfig};
    /// use fixtures::devices::{cone, cone_seam_ramp};
    ///
    /// // The apex cone: no directrix (γ ≡ 0), develops exactly as `new`.
    /// let apex = ConeDevelopment::new_developable(&cone(), 128).unwrap();
    /// let base = ConeDevelopment::new(&cone()).unwrap();
    /// let s = lattice::Rat::new(1, 2);
    /// let m = lattice::Rat::from_i128(-1);
    /// assert_eq!(
    ///     apex.point(&s, &m, &DevConfig::tight()).center(),
    ///     base.point(&s, &m, &DevConfig::tight()).center(),
    /// );
    /// // The seam ramp is a γ ≠ 0 developable — admitted here, refused by the apex-only `new`.
    /// assert!(ConeDevelopment::new_developable(&cone_seam_ramp(), 128).is_some());
    /// assert!(ConeDevelopment::new(&cone_seam_ramp()).is_none());
    /// ```
    pub fn new_developable(chart: &Chart<B>, panels: usize) -> Option<Self> {
        let c = arctan_coeff(&chart.psi_prime())?;
        let rho_sq = chart.normal_deriv_sq().reduce();
        let rho_sq_prime = rho_sq.derivative();
        let directrix = if chart.pedal().is_zero() {
            None
        } else {
            let cp = chart.pedal().derivative();
            let cr = cp.dot(chart.ruling()).reduce();
            let cn = cp.dot(chart.normal_deriv()).reduce();
            let cr_prime = cr.derivative();
            let cn_prime = cn.derivative();
            Some(Directrix {
                cr,
                cn,
                cr_prime,
                cn_prime,
            })
        };
        Some(ConeDevelopment {
            c,
            rho_sq,
            rho_sq_prime,
            directrix,
            panels,
            tables: RefCell::new(Vec::new()),
        })
    }

    /// Whether this development's exact flat data re-derives from `chart` — the piecewise fold's
    /// chart-pairing guard: the angle coefficient `c`, the ruling-speed field `ρ²`, and the
    /// directrix dot products `c′·r`, `c′·n′` must all re-derive equal (value equality, so
    /// reduced and unreduced forms agree). Necessary, not sufficient: data the development never
    /// reads (a pedal component along the surface normal) is the chart's alone — but any
    /// mispairing that alters the *flat* map is refused.
    pub(crate) fn derives_from(&self, chart: &Chart<B>) -> bool {
        let Some(c) = arctan_coeff(&chart.psi_prime()) else {
            return false;
        };
        if c != self.c || *chart.normal_deriv_sq() != self.rho_sq {
            return false;
        }
        match &self.directrix {
            None => chart.pedal().is_zero(),
            Some(d) => {
                let cp = chart.pedal().derivative();
                cp.dot(chart.ruling()) == d.cr && cp.dot(chart.normal_deriv()) == d.cn
            }
        }
    }

    /// The flat directrix velocity `γ′(s) = a·e(ψ) + b·e⊥(ψ)` enclosed over a σ-*panel*, with
    /// `a = (c′·r)/ρ`, `b = −(c′·n′)/ρ`, `e(ψ) = (cos ψ, sin ψ)`, `e⊥(ψ) = (−sin ψ, cos ψ)`. `None`
    /// on a pole (`ρ²` or a component denominator straddles zero — never on a nondegenerate span).
    fn directrix_velocity(
        &self,
        d: &Directrix<B>,
        panel: &RatIv<B>,
        cfg: &DevConfig<B>,
    ) -> Option<[RatIv<B>; 2]> {
        crate::counters::bump_gamma_velocity();
        let cr = eval_ratfunc_on(&d.cr, panel)?;
        let cn = eval_ratfunc_on(&d.cn, panel)?;
        let rho2 = eval_ratfunc_on(&self.rho_sq, panel)?;
        let inv_rho = sqrt_on(&rho2, &cfg.sqrt_eps).recip_pos()?;
        let a = cr.mul(&inv_rho); // (c′·r)/ρ
        let b = cn.neg().mul(&inv_rho); // −(c′·n′)/ρ
        let psi = self.angle_on(panel, cfg.terms);
        let cos = cos_on(&psi, cfg.terms);
        let sin = sin_on(&psi, cfg.terms);
        Some([
            a.mul(&cos).sub(&b.mul(&sin)).rounded(),
            a.mul(&sin).add(&b.mul(&cos)).rounded(),
        ])
    }

    /// The flat directrix **acceleration** `γ″(s) = A·e(ψ) + B·e⊥(ψ)` enclosed over a σ-*panel* — the
    /// slope-remainder integrand of the quadratic quadrature [`integrate_on_slope`]. `γ″` shares `γ′`'s
    /// rotating-frame structure (spec §Tier C) with coefficients
    ///
    /// ```text
    ///   A = a′ − b·ψ′ ,   B = a·ψ′ + b′ ,
    /// ```
    ///
    /// where `a = (c′·r)/ρ`, `b = −(c′·n′)/ρ`, `ψ′ = c/(1+σ²)`, and `a′,b′` are formed from the exact
    /// derivatives `cr′,cn′,(ρ²)′` via `a′ = (cr′ − cr·(ρ²)′/(2ρ²))/ρ`, `b′ = (−cn′ + cn·(ρ²)′/(2ρ²))/ρ`
    /// (using `ρ′ = (ρ²)′/(2ρ)`). Enclosing `γ″` *soundly* is what licenses the slope remainder
    /// `R ∈ [−(h²/8)·width(γ″), (h²/8)·width(γ″)]` — a `γ″` that under-bounds the true second derivative
    /// would make the quadrature certificate unsound. `None` on a pole (never on a nondegenerate cone
    /// span, where `ρ² > 0` and `1+σ² ≥ 1`).
    fn directrix_accel(
        &self,
        d: &Directrix<B>,
        panel: &RatIv<B>,
        cfg: &DevConfig<B>,
    ) -> Option<[RatIv<B>; 2]> {
        let half = Rat::new(1, 2);
        let cr = eval_ratfunc_on(&d.cr, panel)?;
        let cn = eval_ratfunc_on(&d.cn, panel)?;
        let cr_p = eval_ratfunc_on(&d.cr_prime, panel)?;
        let cn_p = eval_ratfunc_on(&d.cn_prime, panel)?;
        let rho2 = eval_ratfunc_on(&self.rho_sq, panel)?;
        let rho2_p = eval_ratfunc_on(&self.rho_sq_prime, panel)?;
        let inv_rho = sqrt_on(&rho2, &cfg.sqrt_eps).recip_pos()?; // 1/ρ
        let inv_rho2 = rho2.recip_pos()?; // 1/ρ²
        // a = (c′·r)/ρ, b = −(c′·n′)/ρ — the same flat-frame coefficients as γ′.
        let a = cr.mul(&inv_rho);
        let b = cn.neg().mul(&inv_rho);
        // a′ = (cr′ − cr·(ρ²)′/(2ρ²))/ρ,  b′ = (−cn′ + cn·(ρ²)′/(2ρ²))/ρ.
        let corr_a = cr.mul(&rho2_p).mul(&inv_rho2).scale(&half);
        let corr_b = cn.mul(&rho2_p).mul(&inv_rho2).scale(&half);
        let a_p = cr_p.sub(&corr_a).mul(&inv_rho);
        let b_p = cn_p.neg().add(&corr_b).mul(&inv_rho);
        // ψ′ = c/(1+σ²); `1+σ²` computed via |σ|² so it never dips below 1 on a 0-straddling panel.
        let abs_s = abs_on(panel);
        let one_plus_s2 = abs_s.mul(&abs_s).add(&RatIv::point(Rat::from_i128(1)));
        let psi_p = one_plus_s2.recip_pos()?.scale(&self.c);
        let am = a_p.sub(&b.mul(&psi_p)); // A = a′ − b·ψ′
        let bm = a.mul(&psi_p).add(&b_p); // B = a·ψ′ + b′
        let psi = self.angle_on(panel, cfg.terms);
        let cos = cos_on(&psi, cfg.terms);
        let sin = sin_on(&psi, cfg.terms);
        Some([
            am.mul(&cos).sub(&bm.mul(&sin)).rounded(),
            am.mul(&sin).add(&bm.mul(&cos)).rounded(),
        ])
    }

    /// Whether this developable carries a nonzero flat directrix (`γ ≠ 0`) — the curved-support
    /// case. `false` for the apex cone (`γ ≡ 0`). Used by the fold to branch the residual handling.
    pub(crate) fn has_directrix(&self) -> bool {
        self.directrix.is_some()
    }

    /// γ over an *interval* σ (the fold's residual `(x, y) − γ(σ)` needs it), `[0, 0]` when `γ ≡ 0`.
    pub(crate) fn directrix_on_iv(
        &self,
        sigma: &RatIv<B>,
        cfg: &DevConfig<B>,
    ) -> Option<[RatIv<B>; 2]> {
        match &self.directrix {
            None => {
                let z = RatIv::point(Rat::from_i128(0));
                Some([z.clone(), z])
            }
            Some(d) => self.directrix_on(d, sigma, cfg),
        }
    }

    /// A certified enclosure of the flat directrix `γ(σ) = ∫₀^σ γ′` at a rational `σ ≥ 0`, or the
    /// point `[0, 0]` when the chart has no directrix (`γ ≡ 0`, the apex cone). `None` on a pole or
    /// `σ < 0`.
    pub(crate) fn directrix_at(&self, sigma: &Rat<B>, cfg: &DevConfig<B>) -> Option<[RatIv<B>; 2]> {
        let d = match &self.directrix {
            None => {
                let zero = RatIv::point(Rat::from_i128(0));
                return Some([zero.clone(), zero]);
            }
            Some(d) => d,
        };
        self.directrix_accumulated(d, &Rat::from_i128(0), sigma, cfg)
    }

    /// A certified enclosure of the flat directrix accumulated over a σ-**sub-range**,
    /// `∫_{lo}^{σ} γ′` — the primitive for gluing a **piecewise-support** development. A region's
    /// support is a rational whose *global* extension may explode outside its σ-window (a smootherstep
    /// ramp blows up beyond `[σ_a, σ_b]`); integrating `γ` from `lo = σ_a` rather than `0` keeps the
    /// enclosure over the region's own domain, where the support is tame. `[0, 0]` when `γ ≡ 0`;
    /// `None` on a pole. (`directrix_at` is this with `lo = 0`.)
    pub fn directrix_between(
        &self,
        lo: &Rat<B>,
        sigma: &Rat<B>,
        cfg: &DevConfig<B>,
    ) -> Option<[RatIv<B>; 2]> {
        let d = match &self.directrix {
            None => {
                let zero = RatIv::point(Rat::from_i128(0));
                return Some([zero.clone(), zero]);
            }
            Some(d) => d,
        };
        self.directrix_accumulated(d, lo, sigma, cfg)
    }

    /// `∫_lo^σ γ′`, served from the [prefix table](DirectrixPrefix) anchored at `lo`.
    ///
    /// The whole point of OPT.1: the direct quadrature spent `panels` integrand evaluations on
    /// *every* query, and a boundary walk or a fold bisection issues many queries against one
    /// `lo`. Here the grid is accumulated once and a query costs one partial panel.
    fn directrix_accumulated(
        &self,
        d: &Directrix<B>,
        lo: &Rat<B>,
        sigma: &Rat<B>,
        cfg: &DevConfig<B>,
    ) -> Option<[RatIv<B>; 2]> {
        use core::cmp::Ordering;
        let zero_iv = || RatIv::point(Rat::from_i128(0));
        match sigma.cmp(lo) {
            Ordering::Less => return None,
            Ordering::Equal => return Some([zero_iv(), zero_iv()]),
            Ordering::Greater => {}
        }
        // A zero quadrature budget is refused, as the direct rule refused it — a curved-support
        // development with no panels has no defined γ, and answering with one coarse cell would
        // quietly invent an enclosure the caller never asked for.
        let panels = match self.panels {
            0 => return None,
            n => n,
        };
        // One cell of the grid, by the same rule the direct quadrature applies to a subinterval.
        let cell = |a: &Rat<B>, b: &Rat<B>| -> Option<[RatIv<B>; 2]> {
            crate::counters::bump_gamma_cell();
            let gx = integrate_on_slope(
                |p| self.directrix_velocity(d, p, cfg).map(|f| f[0].clone()),
                |p| self.directrix_accel(d, p, cfg).map(|f| f[0].clone()),
                a,
                b,
                1,
            )?;
            let gy = integrate_on_slope(
                |p| self.directrix_velocity(d, p, cfg).map(|f| f[1].clone()),
                |p| self.directrix_accel(d, p, cfg).map(|f| f[1].clone()),
                a,
                b,
                1,
            )?;
            Some([gx, gy])
        };

        // Anchor a table at `lo`, sized from this query, unless a usable one is already cached.
        // "Usable" excludes a table whose step is so fine that reaching σ would cost more cells
        // than integrating directly — re-anchoring then keeps the worst case at the old cost.
        let reach = |t: &DirectrixPrefix<B>| {
            t.lo.add(
                &t.step
                    .mul(&Rat::from_i128((panels * GAMMA_MAX_CELLS) as i128)),
            )
        };
        let have = {
            let tables = self.tables.borrow();
            tables.iter().position(|t| {
                t.lo.cmp(lo) == Ordering::Equal && sigma.cmp(&reach(t)) != Ordering::Greater
            })
        };
        let idx = match have {
            Some(i) => i,
            None => {
                let step = sigma.sub(lo).div(&Rat::from_i128(panels as i128));
                let fresh = DirectrixPrefix {
                    lo: lo.clone(),
                    step,
                    prefix: vec![[zero_iv(), zero_iv()]],
                };
                let mut tables = self.tables.borrow_mut();
                tables.retain(|t| t.lo.cmp(lo) != Ordering::Equal);
                if tables.len() >= GAMMA_TABLES {
                    tables.remove(0);
                }
                tables.push(fresh);
                tables.len() - 1
            }
        };

        // Extend the grid until the last grid point at or below σ is accumulated.
        //
        // The cell borrow is released around each integration, so the table is re-located by `lo`
        // rather than by a held index: the integrand does not currently compute γ, but if it ever
        // did, a stale index would silently read someone else's table where a fresh lookup simply
        // finds the right one. `idx` is only the initial hint.
        let locate = |tables: &[DirectrixPrefix<B>]| {
            tables
                .iter()
                .position(|t| t.lo.cmp(lo) == Ordering::Equal)
                .unwrap_or(idx)
        };
        loop {
            let (cells, step, origin) = {
                let tables = self.tables.borrow();
                let t = &tables[locate(&tables)];
                (t.prefix.len() - 1, t.step.clone(), t.lo.clone())
            };
            let next_grid = origin.add(&step.mul(&Rat::from_i128((cells + 1) as i128)));
            if next_grid.cmp(sigma) == Ordering::Greater {
                break;
            }
            let a = origin.add(&step.mul(&Rat::from_i128(cells as i128)));
            let add = cell(&a, &next_grid)?;
            let mut tables = self.tables.borrow_mut();
            let at = locate(&tables);
            let t = &mut tables[at];
            let last = t.prefix[t.prefix.len() - 1].clone();
            t.prefix.push([
                last[0].add(&add[0]).rounded(),
                last[1].add(&add[1]).rounded(),
            ]);
        }

        // The largest grid point at or below σ. A table built by an earlier, *farther* query
        // reaches past σ, so this must be searched for — taking the last entry would integrate
        // backwards from beyond σ.
        let (base, grid_j) = {
            let tables = self.tables.borrow();
            let t = &tables[locate(&tables)];
            let grid = |j: usize| t.lo.add(&t.step.mul(&Rat::from_i128(j as i128)));
            let (mut lo_i, mut hi_i) = (0usize, t.prefix.len() - 1);
            while lo_i < hi_i {
                let mid = lo_i + (hi_i - lo_i).div_ceil(2);
                if grid(mid).cmp(sigma) == Ordering::Greater {
                    hi_i = mid - 1;
                } else {
                    lo_i = mid;
                }
            }
            (t.prefix[lo_i].clone(), grid(lo_i))
        };
        // The remainder from the last grid point up to σ — at most one cell wide, so a single
        // panel here carries the same error as any one cell of the grid.
        if grid_j.cmp(sigma) == Ordering::Equal {
            return Some(base);
        }
        let tail = cell(&grid_j, sigma)?;
        Some([
            base[0].add(&tail[0]).rounded(),
            base[1].add(&tail[1]).rounded(),
        ])
    }

    /// The certified flat point `D = base + ∫_{lo}^{σ} γ′ + µ̂·ρ·e(ψ)` — [`point_signed`] with an
    /// explicit directrix **base** and **lower limit**, the workhorse for a piecewise-support
    /// development: each region integrates `γ` only over its own σ-window (from `lo`) on top of the
    /// running flat-frame `base` carried from the previous region's end, so the pieces butt into one
    /// connected outline with a tight enclosure. `None` on a directrix pole.
    ///
    /// ```
    /// use develop::cone::{ConeDevelopment, DevConfig};
    /// use develop::interval::RatIv;
    /// use fixtures::devices::cone_wrap;
    /// use lattice::{Bignum, Rat};
    ///
    /// // With a zero base and lo = σ, the accumulated γ is empty, so this is the plain signed point.
    /// let dev = ConeDevelopment::new(&cone_wrap()).unwrap();
    /// let cfg = DevConfig::<Bignum>::tight();
    /// let (s, m) = (Rat::new(1, 3), Rat::from_i128(-1));
    /// let z = RatIv::point(Rat::from_i128(0));
    /// let a = dev.point_from(&[z.clone(), z], &s, &s, &m, &cfg).unwrap().center();
    /// let b = dev.point_signed(&s, &m, &cfg).center();
    /// assert_eq!(a, b);
    /// ```
    pub fn point_from(
        &self,
        base: &[RatIv<B>; 2],
        lo: &Rat<B>,
        sigma: &Rat<B>,
        mu_hat: &Rat<B>,
        cfg: &DevConfig<B>,
    ) -> Option<FlatBox<B>> {
        let g = self.directrix_between(lo, sigma, cfg)?;
        let psi = self.angle(sigma, cfg.terms);
        let cos = cos_on(&psi, cfg.terms);
        let sin = sin_on(&psi, cfg.terms);
        let radial = self.radius(sigma, &cfg.sqrt_eps).scale(mu_hat).rounded();
        Some(FlatBox {
            x: base[0].add(&g[0]).add(&radial.mul(&cos)).rounded(),
            y: base[1].add(&g[1]).add(&radial.mul(&sin)).rounded(),
        })
    }

    /// The certified flat point `D = base + ∫_{lo}^{σ} γ′ + µ̂·ρ·e(ψ)` for *interval* σ and µ̂ —
    /// [`point_from`](Self::point_from) lifted to intervals, the ANCHOR-side workhorse for a
    /// **piecewise-support** development: the γ-tail integrates only from `lo` (the region's own
    /// σ-window, where the support is tame) on top of the running flat-frame `base` carried from
    /// the previous region's end. The γ enclosure over the interval is the sound hull
    /// `γ(σ_lo) + γ′([σ_lo, σ_hi])·[0, σ_hi − σ_lo]` with `γ(σ_lo)` accumulated from `lo`. The
    /// ruling coordinate is **signed** (the canonical development, [`point_signed`](Self::point_signed))
    /// — the piecewise gluing's requirement. Requires `lo ≤ σ_lo`; `None` on a pole.
    ///
    /// ```
    /// use develop::cone::{ConeDevelopment, DevConfig};
    /// use develop::interval::RatIv;
    /// use fixtures::devices::cone_wrap;
    /// use lattice::{Bignum, Rat};
    ///
    /// // With a zero base, lo = 0, and a thin σ-interval, it encloses the plain signed point.
    /// let dev = ConeDevelopment::new(&cone_wrap()).unwrap();
    /// let cfg = DevConfig::<Bignum>::tight();
    /// let (s, m) = (Rat::new(1, 3), Rat::from_i128(-1));
    /// let z = RatIv::point(Rat::from_i128(0));
    /// let (si, mi) = (RatIv::point(s.clone()), RatIv::point(m.clone()));
    /// let a = dev
    ///     .point_from_on(&[z.clone(), z], &Rat::from_i128(0), &si, &mi, &cfg)
    ///     .unwrap();
    /// let b = dev.point_signed(&s, &m, &cfg);
    /// assert!(a.x.contains(&b.x.mid()) && a.y.contains(&b.y.mid()));
    /// ```
    pub fn point_from_on(
        &self,
        base: &[RatIv<B>; 2],
        lo: &Rat<B>,
        sigma: &RatIv<B>,
        mu_hat: &RatIv<B>,
        cfg: &DevConfig<B>,
    ) -> Option<FlatBox<B>> {
        let psi = self.angle_on(sigma, cfg.terms);
        let cos = cos_on(&psi, cfg.terms);
        let sin = sin_on(&psi, cfg.terms);
        let radial = self.radius_on(sigma, &cfg.sqrt_eps)?.mul(mu_hat).rounded();
        let g = self.directrix_between_on(lo, sigma, cfg)?;
        Some(FlatBox {
            x: base[0].add(&g[0]).add(&radial.mul(&cos)).rounded(),
            y: base[1].add(&g[1]).add(&radial.mul(&sin)).rounded(),
        })
    }

    /// γ accumulated **from `lo`** over an *interval* σ: the sound hull
    /// `γ_lo(σ_lo⁺) + γ′(σ)·[0, width]` — the from-`lo` counterpart of [`directrix_on`], shared by
    /// the piecewise interval development ([`point_from_on`](Self::point_from_on)) and the
    /// piecewise fold's directrix residual. `[0, 0]` when `γ ≡ 0`. `None` on a pole.
    pub(crate) fn directrix_between_on(
        &self,
        lo: &Rat<B>,
        sigma: &RatIv<B>,
        cfg: &DevConfig<B>,
    ) -> Option<[RatIv<B>; 2]> {
        // γ over the interval, accumulated from `lo`: γ(σ_lo) + γ′(σ)·[0, width] (the same sound
        // hull as the from-0 interval directrix, with the from-`lo` lower limit).
        match &self.directrix {
            None => {
                let z = RatIv::point(Rat::from_i128(0));
                Some([z.clone(), z])
            }
            Some(d) => {
                // `sigma` can be an outward-rounded enclosure whose `lo()` dips *below* the
                // window start `lo` (e.g. the ANCHOR checker's rounded σ(t) image at a
                // non-dyadic `lo` like 7/24). True rail points never lie below `lo`, so clamp
                // the γ base point: the enclosure then covers `sigma ∩ [lo, ∞)` — sound for
                // every caller, since the from-`lo` development is only defined there.
                let glo = if sigma.lo().cmp(lo) == core::cmp::Ordering::Less {
                    lo.clone()
                } else {
                    sigma.lo().clone()
                };
                let width = sigma.hi().sub(&glo);
                let width = if width.sign() < 0 {
                    Rat::from_i128(0)
                } else {
                    width
                };
                let at_lo = self.directrix_between(lo, &glo, cfg)?;
                let vel = self.directrix_velocity(d, sigma, cfg)?;
                let tail = RatIv::new(Rat::from_i128(0), width);
                Some([
                    at_lo[0].add(&vel[0].mul(&tail)).rounded(),
                    at_lo[1].add(&vel[1].mul(&tail)).rounded(),
                ])
            }
        }
    }

    /// A certified enclosure of `γ(σ)` over an *interval* σ: `γ(σ_lo)` plus the tail `γ′([σ_lo,
    /// σ_hi]) · [0, σ_hi − σ_lo]` — a sound hull, since `γ(σ) = γ(σ_lo) + ∫_{σ_lo}^{σ} γ′` and the
    /// integral lies in `γ′`-enclosure × `[0, width]`. `None` on a pole.
    fn directrix_on(
        &self,
        d: &Directrix<B>,
        sigma: &RatIv<B>,
        cfg: &DevConfig<B>,
    ) -> Option<[RatIv<B>; 2]> {
        // An outward-rounded σ enclosure can dip below the domain start 0 (where `directrix_at`
        // is undefined); clamp — sound, the development covers `sigma ∩ [0, ∞)`.
        let zero = Rat::from_i128(0);
        let glo = if sigma.lo().sign() < 0 {
            zero.clone()
        } else {
            sigma.lo().clone()
        };
        let width = sigma.hi().sub(&glo);
        let width = if width.sign() < 0 { zero } else { width };
        let base = self.directrix_at(&glo, cfg)?;
        let vel = self.directrix_velocity(d, sigma, cfg)?;
        let tail = RatIv::new(Rat::from_i128(0), width);
        Some([
            base[0].add(&vel[0].mul(&tail)).rounded(),
            base[1].add(&vel[1].mul(&tail)).rounded(),
        ])
    }

    /// The proven angle coefficient `c` (`ψ = c·arctan σ`).
    pub fn angle_coeff(&self) -> &Rat<B> {
        &self.c
    }

    /// The exact ruling-speed field `ρ²(σ)` — with [`angle_coeff`](Self::angle_coeff) it pins the
    /// development's *frame* (`ρ`, `ψ` are support-independent), which is how a piecewise-support
    /// gluing validates that its regions share one frame.
    pub(crate) fn rho_sq(&self) -> &RatFunc<B> {
        &self.rho_sq
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
        match &self.directrix {
            // γ ≡ 0 fast path — byte-identical to the apex-cone development.
            None => {
                let radial = self
                    .radius(sigma, &cfg.sqrt_eps)
                    .scale(&abs(mu_hat))
                    .rounded();
                FlatBox {
                    x: radial.mul(&cos).rounded(),
                    y: radial.mul(&sin).rounded(),
                }
            }
            // γ ≠ 0: D(σ, µ̂) = γ(σ) + µ̂·ρ·e(ψ), with **signed** µ̂ (the directrix breaks the apex
            // symmetry, so the ruling coordinate is a signed offset, not a distance). A directrix
            // pole is unreachable on a nondegenerate span; it degrades to the ruling-only term.
            Some(_) => {
                let g = self.directrix_at(sigma, cfg).unwrap_or_else(|| {
                    let zero = RatIv::point(Rat::from_i128(0));
                    [zero.clone(), zero]
                });
                let radial = self.radius(sigma, &cfg.sqrt_eps).scale(mu_hat).rounded();
                FlatBox {
                    x: g[0].add(&radial.mul(&cos)).rounded(),
                    y: g[1].add(&radial.mul(&sin)).rounded(),
                }
            }
        }
    }

    /// The certified flat point in the **canonical signed development** `D(σ, µ̂) = γ(σ) + µ̂·ρ·e(ψ)`
    /// — the single isometric formula (spec §Tier C) that holds for **every** support, `γ ≡ 0` on the
    /// apex cone included. Unlike [`point`], the ruling coordinate `µ̂` is always **signed** (a point
    /// at `µ̂ < 0` lands on the `ψ + π` side of the flat apex), so a `γ ≡ 0` body and a `γ ≠ 0` ramp
    /// that share one frame develop into **one continuous flat region** — the self-lapping demo's
    /// whole requirement. (`point`'s `|µ̂|` fast path places a one-sided gore at angle `ψ` for either
    /// sign; correct in isolation, but it flips a `µ̂ < 0` gore across the apex relative to this
    /// formula, so the two must not be mixed along a connected boundary.)
    ///
    /// `point(σ, 0, cfg)` and `point_signed(σ, 0, cfg)` both return exactly `γ(σ)` — the demo reads
    /// that to stitch piecewise-support regions by a constant flat-frame offset.
    ///
    /// ```
    /// use develop::cone::{ConeDevelopment, DevConfig};
    /// use fixtures::devices::cone_wrap;
    /// use lattice::{Bignum, Rat};
    ///
    /// let dev = ConeDevelopment::new(&cone_wrap()).unwrap();
    /// let cfg = DevConfig::<Bignum>::tight();
    /// let (s, m) = (Rat::new(1, 3), Rat::from_i128(-1));
    /// // On the apex cone (γ ≡ 0) the signed point is the negation of `point`'s |µ̂| point for µ̂ < 0.
    /// let signed = dev.point_signed(&s, &m, &cfg).center();
    /// let abs = dev.point(&s, &m, &cfg).center();
    /// assert_eq!(signed.0, abs.0.neg());
    /// assert_eq!(signed.1, abs.1.neg());
    /// ```
    pub fn point_signed(&self, sigma: &Rat<B>, mu_hat: &Rat<B>, cfg: &DevConfig<B>) -> FlatBox<B> {
        let psi = self.angle(sigma, cfg.terms);
        let cos = cos_on(&psi, cfg.terms);
        let sin = sin_on(&psi, cfg.terms);
        let g = self.directrix_at(sigma, cfg).unwrap_or_else(|| {
            let zero = RatIv::point(Rat::from_i128(0));
            [zero.clone(), zero]
        });
        let radial = self.radius(sigma, &cfg.sqrt_eps).scale(mu_hat).rounded();
        FlatBox {
            x: g[0].add(&radial.mul(&cos)).rounded(),
            y: g[1].add(&radial.mul(&sin)).rounded(),
        }
    }

    /// A certified enclosure of `ψ(σ) = c·arctan σ` for an *interval* σ — the hull of
    /// the two endpoint enclosures (`ψ` is monotone in σ, `c > 0`). Used by the ANCHOR
    /// sup-bound to develop over a σ-sub-interval.
    pub fn angle_on(&self, sigma: &RatIv<B>, terms: usize) -> RatIv<B> {
        self.angle(sigma.lo(), terms)
            .hull_with(&self.angle(sigma.hi(), terms))
    }

    /// A certified enclosure of the ruling-speed radius `ρ(σ) = |n′(σ)|` for an
    /// *interval* σ, via interval evaluation of `ρ²` then `√`. `None` if `ρ²`'s
    /// denominator enclosure straddles zero on the sub-interval (a pole risk — never
    /// for a cone, whose denominator `(1+σ²)²` is positive-definite).
    pub fn radius_on(&self, sigma: &RatIv<B>, eps: &Rat<B>) -> Option<RatIv<B>> {
        Some(sqrt_on(&eval_ratfunc_on(&self.rho_sq, sigma)?, eps))
    }

    /// The certified flat point `D(σ, μ̂)` for *interval* σ and μ̂ — [`point`](Self::point)
    /// lifted to intervals. `None` when the radius enclosure is undefined (pole risk).
    pub fn point_on(
        &self,
        sigma: &RatIv<B>,
        mu_hat: &RatIv<B>,
        cfg: &DevConfig<B>,
    ) -> Option<FlatBox<B>> {
        let psi = self.angle_on(sigma, cfg.terms);
        let cos = cos_on(&psi, cfg.terms);
        let sin = sin_on(&psi, cfg.terms);
        let rho = self.radius_on(sigma, &cfg.sqrt_eps)?;
        match &self.directrix {
            // γ ≡ 0 fast path — byte-identical to the apex-cone development.
            None => {
                let radial = rho.mul(&abs_on(mu_hat)).rounded();
                Some(FlatBox {
                    x: radial.mul(&cos).rounded(),
                    y: radial.mul(&sin).rounded(),
                })
            }
            // γ ≠ 0: D = γ(σ) + µ̂·ρ·e(ψ), signed µ̂.
            Some(d) => {
                let radial = rho.mul(mu_hat).rounded();
                let g = self.directrix_on(d, sigma, cfg)?;
                Some(FlatBox {
                    x: g[0].add(&radial.mul(&cos)).rounded(),
                    y: g[1].add(&radial.mul(&sin)).rounded(),
                })
            }
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
    use fixtures::devices::{cone, cone_alt, cone_seam, cone_seam_ramp, cone_wrap, cylinder};

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
    fn wrapping_cone_develops_with_double_the_arctan_coeff_at_finite_seam() {
        // The degree-2 wrapping cone (traverses the Gauss circle twice) keeps the single-arctan
        // angle law — with coefficient c = 260/97 = 2·(130/97), the whole point of staying in the
        // cone's 2-plane. The canonical recognizer accepts it verbatim, no new integrator.
        assert_eq!(cone_angle_coeff(&cone_wrap()), Some(Q::new(260, 97)));
        let dev =
            ConeDevelopment::new(&cone_wrap()).expect("wrapping cone is a canonical arctan cone");
        assert_eq!(dev.angle_coeff(), &Q::new(260, 97));

        // The seam ruling (φ₃D = ±π) that stranded the degree-1 chart at σ = ±∞ is the FINITE,
        // regular σ = 1 here: it develops to an ordinary well-conditioned flat point (no blow-up).
        let seam = dev.point(&Q::from_i128(1), &Q::from_i128(-1), &DevConfig::tight());
        assert!(seam.backward_error() < Q::new(1, 1_000_000));
        // ρ(1) = |n′|(1) is a finite positive surd — the stall is gone.
        let rho1 = dev.radius(&Q::from_i128(1), &Q::new(1, 1_000_000_000_000));
        assert!(rho1.lo().sign() > 0);

        // One full turn + lap lives in the finite window σ ∈ [−1, 1] (φ₃D ∈ [−π, π]): the flat
        // angle swept is c·(arctan 1 − arctan(−1)) = c·π/2 = (260/97)·(π/2) ≈ 240.9° — matching the
        // classic developed-cone sector for a single 2π wrap of the SAME cone.
        let span = dev
            .angle(&Q::from_i128(1), 24)
            .sub(&dev.angle(&Q::from_i128(-1), 24));
        use std::f64::consts::PI;
        assert!((to_f64(&span.mid()) - (260.0 / 97.0) * (PI / 2.0)).abs() < 1e-9);
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
    fn the_seam_develops_at_the_finite_recentered_point() {
        // The seam ruling (φ₃D = ±π, at σ = ±∞ in the canonical chart) is UNREACHABLE by the
        // arctan development there. In the re-centered seam chart (`cone_seam`) it is the regular
        // finite point σ' = 0 — and the SAME `ConeDevelopment` machinery develops it, unchanged
        // (the seam chart is a canonical apex cone, γ ≡ 0). This is the whole point of the
        // re-centering: it conditions the seam so the certificate lives at finite σ'.
        let dev = ConeDevelopment::new(&cone_seam()).expect("the re-centered cone is canonical");
        // Same proven angle law c = 2 sinβ = 130/97 as the canonical chart.
        assert_eq!(cone_angle_coeff(&cone_seam()), Some(Q::new(130, 97)));
        assert_eq!(dev.angle_coeff(), &Q::new(130, 97));

        // At the seam σ' = 0: ψ = c·arctan 0 = 0 (the seam is this view's reference ray) and
        // ρ_seam(0) = 144/97, so D(0, −1) = (144/97, 0) — finite, exact, well-conditioned.
        let seam = dev.point(&Q::from_i128(0), &Q::new(-1, 1), &DevConfig::tight());
        assert!(seam.x.contains(&Q::new(144, 97)));
        assert!(seam.y.contains(&Q::from_i128(0)));
        assert!(seam.backward_error() < Q::new(1, 1_000_000));
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

    // ---- DD.2: the γ ≠ 0 flat-directrix integrator (DEV.3 method b) ---------------------------

    /// The `γ ≡ 0` fast path is byte-identical: `new_developable` on the apex cone (`pedal ≡ 0`)
    /// produces exactly the same flat point as the pre-DD.2 `new`, across the gore.
    #[test]
    fn new_developable_reproduces_new_on_the_apex_cone() {
        let base = ConeDevelopment::new(&cone()).unwrap();
        let devd = ConeDevelopment::new_developable(&cone(), 128).unwrap();
        let cfg = DevConfig::tight();
        for &(sn, sd) in &[(0i128, 1i128), (1, 2), (3, 4), (1, 1)] {
            for &m in &[-1i128, -2] {
                let (s, mu) = (Q::new(sn, sd), Q::from_i128(m));
                assert_eq!(
                    base.point(&s, &mu, &cfg).center(),
                    devd.point(&s, &mu, &cfg).center(),
                    "γ≡0 fast path must reproduce `new` at σ={sn}/{sd}, µ̂={m}"
                );
            }
        }
    }

    /// The seam ramp (`cone_seam_ramp`, γ ≠ 0) develops as a **local isometry**: the certified
    /// development's first fundamental form matches the 3-D surface's, `|D_σ|² = |X_σ|²` at sample
    /// points — the exact check the paper's §Tier C flags (a wrong flat-frame sign gives the
    /// non-isometric defect `|D_σ|² − |X_σ|² = 4bℓψ′`). Computed from the directrix *velocity* `γ′`
    /// (no quadrature error), so it isolates the frame/sign correctness of the integrand.
    #[test]
    fn the_ramp_development_is_a_local_isometry() {
        let chart = cone_seam_ramp();
        let dev = ConeDevelopment::new_developable(&chart, 64).unwrap();
        let c = to_f64(dev.angle_coeff());
        // exact-rational fields, evaluated to f64 (audit only).
        let eval3 = |v: &lattice::Vec3Rat<Bignum>, s: &Q| -> [f64; 3] {
            let p = v.eval(s).unwrap();
            [to_f64(&p[0]), to_f64(&p[1]), to_f64(&p[2])]
        };
        let dot3 = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let cp = chart.pedal().derivative(); // c′
        let rp = chart.ruling().derivative(); // r′
        let rho_sq = chart.normal_deriv_sq();
        let rho_sq_p = rho_sq.derivative();

        let mut max_abs = 0f64;
        for &(sn, sd) in &[(0i128, 1i128), (1, 8), (1, 4), (3, 8), (1, 2)] {
            let s = Q::new(sn, sd);
            let sf = to_f64(&s);
            let cpv = eval3(&cp, &s);
            let rv = eval3(chart.ruling(), &s);
            let rpv = eval3(&rp, &s);
            let npv = eval3(chart.normal_deriv(), &s);
            let rho2 = to_f64(&rho_sq.eval(&s).unwrap());
            let rho = rho2.sqrt();
            let rhop = to_f64(&rho_sq_p.eval(&s).unwrap()) / (2.0 * rho); // ρ′ = (ρ²)′/(2ρ)
            let psi = c * sf.atan();
            let psip = c / (1.0 + sf * sf); // ψ′ = c/(1+σ²)
            let (cs, sn_) = (psi.cos(), psi.sin());
            let a = dot3(cpv, rv) / rho; // (c′·r)/ρ
            let b = -dot3(cpv, npv) / rho; // −(c′·n′)/ρ
            for &m in &[-1.0f64, -1.5, -2.0] {
                // D_σ = γ′ + µ̂·(ρ′·e + ρ·ψ′·e⊥),  e=(cos,sin), e⊥=(−sin,cos)
                let gx = a * cs - b * sn_;
                let gy = a * sn_ + b * cs;
                let tx = rhop * cs - rho * psip * sn_;
                let ty = rhop * sn_ + rho * psip * cs;
                let (dsx, dsy) = (gx + m * tx, gy + m * ty);
                let e_d = dsx * dsx + dsy * dsy; // |D_σ|²
                // X_σ = c′ + µ̂·r′  (3-D)
                let xs = [
                    cpv[0] + m * rpv[0],
                    cpv[1] + m * rpv[1],
                    cpv[2] + m * rpv[2],
                ];
                let e_x = dot3(xs, xs); // |X_σ|²
                max_abs = max_abs.max((e_d - e_x).abs());
            }
        }
        assert!(
            max_abs < 1e-6,
            "development must be a local isometry (|D_σ|²−|X_σ|² = {max_abs:e}); a nonzero value \
             is the §Tier C frame-sign defect 4bℓψ′"
        );
    }

    /// The certified directrix `γ` converges: the flat point's backward error on the ramp flap
    /// shrinks as the quadrature budget grows, and clears a fab-plausible DRC — the DD.2 GO signal.
    #[test]
    fn the_ramp_directrix_converges_and_is_fab_plausible() {
        let chart = cone_seam_ramp();
        let cfg = DevConfig::tight();
        let (s, mu) = (Q::new(1, 4), Q::new(-3, 2)); // mid-ramp, in the flap band
        let eps = |panels: usize| {
            ConeDevelopment::new_developable(&chart, panels)
                .unwrap()
                .point(&s, &mu, &cfg)
                .backward_error()
        };
        let coarse = eps(64);
        let fine = eps(1024);
        assert!(
            fine.cmp(&coarse) == core::cmp::Ordering::Less,
            "γ enclosure must converge"
        );
        // Fab-plausible: well under the demo clearance's half (clearance = 1 ⇒ DRC ε < 1/2).
        assert!(
            fine.cmp(&Q::new(1, 100)) == core::cmp::Ordering::Less,
            "the ramp develops to a fab-plausible ε (< 0.01) at 1024 panels, got {fine:?}"
        );
    }

    /// SOUNDNESS (task #216): the certified acceleration `directrix_accel` (`γ″`) genuinely encloses
    /// the derivative of the velocity `γ′` — the exact property the slope-remainder quadrature relies
    /// on (a `γ″` that under-bounds `dγ′/ds` would make the enclosure unsound, not merely loose).
    /// Checked against an **independent** central finite-difference of the analytic float `γ′`; a wrong
    /// frame or sign in `γ″` (the §Tier C failure mode) lands the finite-difference outside the box.
    #[test]
    fn directrix_accel_encloses_the_derivative_of_velocity() {
        let chart = cone_seam_ramp();
        let dev = ConeDevelopment::new_developable(&chart, 64).unwrap();
        let d = dev.directrix.as_ref().unwrap();
        let cfg = DevConfig::tight();
        let c = to_f64(dev.angle_coeff());
        let cp = chart.pedal().derivative(); // c′
        let rho_sq = chart.normal_deriv_sq();
        let eval3 = |v: &lattice::Vec3Rat<Bignum>, s: &Q| -> [f64; 3] {
            let p = v.eval(s).unwrap();
            [to_f64(&p[0]), to_f64(&p[1]), to_f64(&p[2])]
        };
        let dot3 = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        // The analytic float velocity γ′(s) = a·e(ψ) + b·e⊥(ψ), rebuilt independently of the kernel.
        let gp = |s: &Q| -> [f64; 2] {
            let sf = to_f64(s);
            let cpv = eval3(&cp, s);
            let rv = eval3(chart.ruling(), s);
            let npv = eval3(chart.normal_deriv(), s);
            let rho = to_f64(&rho_sq.eval(s).unwrap()).sqrt();
            let a = dot3(cpv, rv) / rho;
            let b = -dot3(cpv, npv) / rho;
            let psi = c * sf.atan();
            [a * psi.cos() - b * psi.sin(), a * psi.sin() + b * psi.cos()]
        };
        let delta = Q::new(1, 10_000);
        let two_delta = 2.0 * to_f64(&delta);
        for &(sn, sd) in &[(1i128, 8i128), (1, 4), (3, 8), (1, 2)] {
            let s = Q::new(sn, sd);
            let (sp, sm) = (s.add(&delta), s.sub(&delta));
            let acc = dev
                .directrix_accel(d, &RatIv::point(s.clone()), &cfg)
                .unwrap();
            let (dsp, dsm) = (gp(&sp), gp(&sm));
            for (i, acc_i) in acc.iter().enumerate() {
                let fd = (dsp[i] - dsm[i]) / two_delta;
                let (lo, hi) = (to_f64(acc_i.lo()), to_f64(acc_i.hi()));
                assert!(
                    lo - 1e-3 <= fd && fd <= hi + 1e-3,
                    "γ″[{i}] enclosure [{lo:e},{hi:e}] must contain the finite-diff {fd:e} at σ={sn}/{sd}"
                );
            }
        }
    }

    /// The slope-remainder quadrature (task #216) converges **quadratically** on the ramp flap — the
    /// win over the DD.2 first-order Riemann sum (linear). Doubling panels shrinks the certified `γ`
    /// enclosure ≈ 4×, and at just 64 panels it is already ~250× tighter than the old rule's 1.45e-3
    /// (16 panels here beat the old 1024). The absolute floor stays fab-plausible throughout.
    #[test]
    fn the_directrix_quadrature_is_quadratic() {
        let chart = cone_seam_ramp();
        let cfg = DevConfig::tight();
        let s = Q::new(1, 4); // mid-ramp
        let width = |panels: usize| {
            let g = ConeDevelopment::new_developable(&chart, panels)
                .unwrap()
                .directrix_at(&s, &cfg)
                .unwrap();
            to_f64(&g[0].width()).max(to_f64(&g[1].width()))
        };
        let ws: Vec<f64> = [32usize, 64, 128, 256].iter().map(|&n| width(n)).collect();
        for pair in ws.windows(2) {
            let ratio = pair[0] / pair[1];
            assert!(
                (3.5..=4.5).contains(&ratio),
                "expected ≈4× per doubling (quadratic), got {ratio}"
            );
        }
        // 64 panels already ≥100× tighter than the first-order rule's documented 1.45e-3.
        assert!(
            width(64) < 1.45e-3 / 100.0,
            "64-panel γ width={:e}",
            width(64)
        );
    }
}
