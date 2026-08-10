//! The float **cut-curve oracle** (G2) — proposes a rational ruling-rail `μ̂(σ)` for a
//! cone∩surface cut, so the certified pipeline has a rail to verify and develop.
//!
//! For an **offset plane** the cut is exactly rational, so this delegates to the pure
//! [`develop::cut::plane_cut_rail`]. For a **cylinder** the cone∩cylinder cut `μ(σ)` is
//! a surd (a quadratic root), so the oracle *fits* a rational approximation: at
//! Chebyshev nodes it solves the cone∩cylinder quadratic in μ (in `f64`), picks a
//! branch, and interpolates a polynomial `μ̂(σ)`, whose coefficients are snapped to
//! exact rationals ([`crate::approx::f64_to_rat`]).
//!
//! **Floats propose; they never decide.** The returned `μ̂(σ)` is an *exact* `RatFunc`,
//! and [`develop::cut::cut_fit`] is the sole arbiter of whether it lies on the cut
//! within the fab clearance. A loose fit or a wrong branch can only be judged
//! `Unresolved` there, never a wrong `Verified`; no float touches a certificate.

use crate::approx::{f64_to_rat, rat_to_f64, vec3_to_f64};
use develop::cut::{CutSurface, plane_cut_rail};
use geom::chart::Chart;
use lattice::{Backend, Interval, Poly, Rat, RatFunc};

/// Which root of the cone∩cylinder quadratic in μ to trace — the retained branch of a
/// σ-monotone cut arc (a cut that turns back in σ is split into monotone arcs upstream).
#[derive(Clone, Copy, Debug)]
pub enum RootPick {
    /// The smaller root.
    Lower,
    /// The larger root.
    Upper,
}

/// Propose a rational cut-rail `μ̂(σ)` for the given cut over `span`.
///
/// A [`CutSurface::Plane`] returns the *exact* rail (no fit, no float). A
/// [`CutSurface::Cylinder`] fits a degree-`degree` polynomial (interpolated at
/// `degree + 1` Chebyshev nodes) to the chosen branch of the cone∩cylinder quadratic,
/// snapping coefficients to a `2^bits` dyadic grid. Returns `None` if the cut is not
/// real at some node (no branch there), a chart field is singular, or the linear solve
/// is degenerate — the oracle declines rather than fabricating a rail. The caller then
/// certifies the result with [`develop::cut::cut_fit`].
pub fn fit_cut_rail<B: Backend>(
    chart: &Chart<B>,
    surface: &CutSurface<B>,
    span: &Interval<B>,
    degree: usize,
    pick: RootPick,
    bits: u32,
) -> Option<RatFunc<B>> {
    match surface {
        CutSurface::Plane { n, d } => Some(plane_cut_rail(chart, n, d)),
        CutSurface::Cylinder {
            axis_point,
            axis_dir,
            r2,
        } => {
            let p = vec3_to_f64(axis_point);
            let ax = vec3_to_f64(axis_dir);
            let a2 = dot(&ax, &ax);
            if a2 <= 0.0 {
                return None;
            }
            let rr = rat_to_f64(r2);
            let (lo, hi) = (rat_to_f64(&span.lo), rat_to_f64(&span.hi));

            // Sample the branch at degree+1 Chebyshev nodes (snapped to Rat for exact
            // chart evaluation, then read back as f64 for the fit — same number).
            let n_nodes = degree + 1;
            let mut xs = Vec::with_capacity(n_nodes);
            let mut ys = Vec::with_capacity(n_nodes);
            for k in 0..n_nodes {
                let node = cheb_node(lo, hi, k, n_nodes);
                let sq = f64_to_rat::<B>(node, bits);
                let pedal = vec3_to_f64(&chart.pedal().eval(&sq)?);
                let ruling = vec3_to_f64(&chart.ruling().eval(&sq)?);
                let mu = solve_cut_quadratic(&pedal, &ruling, &p, &ax, a2, rr, pick)?;
                xs.push(rat_to_f64(&sq));
                ys.push(mu);
            }

            // Interpolate monomial coefficients, then snap to exact rationals.
            let coeffs_f = interpolate(&xs, &ys, degree)?;
            let coeffs_q: Vec<Rat<B>> =
                coeffs_f.iter().map(|&c| f64_to_rat::<B>(c, bits)).collect();
            Some(RatFunc::from_poly(Poly::from_coeffs(coeffs_q)))
        }
    }
}

fn dot(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The `k`-th Chebyshev node (of `n`) on `[lo, hi]`: `mid + half·cos(π(2k+1)/2n)`.
fn cheb_node(lo: f64, hi: f64, k: usize, n: usize) -> f64 {
    let mid = 0.5 * (lo + hi);
    let half = 0.5 * (hi - lo);
    let theta = core::f64::consts::PI * (2 * k + 1) as f64 / (2 * n) as f64;
    mid + half * theta.cos()
}

/// Solve the cone∩cylinder quadratic `A μ² + B μ + C = 0` for the ruling coordinate at
/// one station, returning the picked branch — or `None` if the cut is not real there.
///
/// With `X(μ) = pedal + μ·ruling`, `v0 = pedal − p`, `u = ruling`, and the squared
/// distance to the cylinder axis `perp2(μ) = |X−p|² − ((X−p)·â)²/(â·â)`, the equation
/// `perp2 = R²` expands to a quadratic in μ.
fn solve_cut_quadratic(
    pedal: &[f64; 3],
    ruling: &[f64; 3],
    p: &[f64; 3],
    ax: &[f64; 3],
    a2: f64,
    rr: f64,
    pick: RootPick,
) -> Option<f64> {
    let v0 = [pedal[0] - p[0], pedal[1] - p[1], pedal[2] - p[2]];
    let u = ruling;
    let (v0v0, v0u, uu) = (dot(&v0, &v0), dot(&v0, u), dot(u, u));
    let (v0a, ua) = (dot(&v0, ax), dot(u, ax));

    let a = uu - ua * ua / a2;
    let b = 2.0 * v0u - 2.0 * v0a * ua / a2;
    let c = v0v0 - v0a * v0a / a2 - rr;

    if a.abs() < 1e-30 {
        if b.abs() < 1e-30 {
            return None;
        }
        return Some(-c / b);
    }
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return None;
    }
    let sq = disc.sqrt();
    let r1 = (-b - sq) / (2.0 * a);
    let r2 = (-b + sq) / (2.0 * a);
    let (lower, upper) = if r1 <= r2 { (r1, r2) } else { (r2, r1) };
    Some(match pick {
        RootPick::Lower => lower,
        RootPick::Upper => upper,
    })
}

/// Interpolate the monomial coefficients (low-degree first) of the degree-`degree`
/// polynomial through the `degree + 1` samples `(xs, ys)`, by solving the Vandermonde
/// system. `None` if the system is singular (repeated / degenerate nodes).
fn interpolate(xs: &[f64], ys: &[f64], degree: usize) -> Option<Vec<f64>> {
    let n = degree + 1;
    if xs.len() != n || ys.len() != n {
        return None;
    }
    let mut v = vec![vec![0.0f64; n]; n];
    for (i, row) in v.iter_mut().enumerate() {
        let mut power = 1.0;
        for slot in row.iter_mut() {
            *slot = power;
            power *= xs[i];
        }
    }
    solve_linear(v, ys.to_vec())
}

/// Solve `A x = b` (dense, `n×n`) by Gauss–Jordan elimination with partial pivoting.
/// `None` if a pivot is ~0 (singular).
fn solve_linear(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for col in 0..n {
        let mut piv = col;
        for r in (col + 1)..n {
            if a[r][col].abs() > a[piv][col].abs() {
                piv = r;
            }
        }
        if a[piv][col].abs() < 1e-30 {
            return None;
        }
        a.swap(col, piv);
        b.swap(col, piv);
        let pivot_row = a[col].clone(); // clone once so the elimination zips, not double-indexes
        let (d, b_pivot) = (pivot_row[col], b[col]);
        for r in 0..n {
            if r == col {
                continue;
            }
            let f = a[r][col] / d;
            for (ar, pv) in a[r].iter_mut().zip(pivot_row.iter()).skip(col) {
                *ar -= f * pv;
            }
            b[r] -= f * b_pivot;
        }
    }
    Some((0..n).map(|i| b[i] / a[i][i]).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use certify_core::Verdict;
    use develop::cone::DevConfig;
    use develop::cut::{CutFitCert, CutSurface, ValidCutFit, cut_fit};
    use fixtures::devices::cone;
    use lattice::Bignum;

    type Q = Rat<Bignum>;

    fn ivl(lo: Q, hi: Q) -> Interval<Bignum> {
        Interval { lo, hi }
    }
    // A y-axis cylinder through the origin (the "extruded annulus" cut, axis orthogonal
    // to the cone axis) of radius 1/2.
    fn y_cylinder() -> CutSurface<Bignum> {
        CutSurface::Cylinder {
            axis_point: [Q::from_i128(0), Q::from_i128(0), Q::from_i128(0)],
            axis_dir: [Q::from_i128(0), Q::from_i128(1), Q::from_i128(0)],
            r2: Q::new(1, 4),
        }
    }

    fn certify(
        chart: &Chart<Bignum>,
        mu_hat: &RatFunc<Bignum>,
        span: &Interval<Bignum>,
        subdiv: usize,
        clearance: Q,
    ) -> Verdict<ValidCutFit<Bignum>, develop::cut::CutFitFault, Q> {
        cut_fit(
            chart,
            &CutFitCert {
                mu_hat: mu_hat.clone(),
                w: Q::from_i128(0),
                surface: y_cylinder(),
                span: span.clone(),
                subdiv,
                clearance,
                cfg: DevConfig::tight(),
            },
        )
    }

    /// The fitted cone∩cylinder rail is Verified at a generous clearance, and the
    /// certified ε upper-bounds a float distance-to-cylinder audit at dense σ.
    #[test]
    fn fitted_cylinder_rail_certifies_and_corroborates() {
        let chart = cone();
        let span = ivl(Q::new(1, 5), Q::new(4, 5)); // σ ∈ [0.2, 0.8]
        let mu_hat = fit_cut_rail(&chart, &y_cylinder(), &span, 4, RootPick::Upper, 44)
            .expect("oracle proposes a rail");

        let eps = match certify(&chart, &mu_hat, &span, 48, Q::from_i128(1000)) {
            Verdict::Verified(v) => v.eps,
            other => panic!("expected Verified, got {}", tag(&other)),
        };

        // Float audit: max distance-to-cylinder along the fitted rail ≤ ε.
        let r = 0.5f64;
        let (lo, hi) = (0.2f64, 0.8f64);
        for i in 0..=60 {
            let sf = lo + (hi - lo) * (i as f64) / 60.0;
            let sq = f64_to_rat::<Bignum>(sf, 44);
            let mu = mu_hat.eval(&sq).unwrap();
            let x = vec3_to_f64(&chart.surface(&mu, &Q::from_i128(0)).eval(&sq).unwrap());
            let perp = (x[0] * x[0] + x[2] * x[2]).sqrt(); // ⊥ to the y-axis
            let dist = (perp - r).abs();
            assert!(
                dist <= rat_to_f64(&eps) + 1e-9,
                "certified ε {} must dominate float dist {dist} at σ={sf}",
                rat_to_f64(&eps)
            );
        }
    }

    /// The certified ε is an interval bound, so its refinement handle is `subdiv` (like
    /// `anchor_dev`/`unroll`): finer σ-subdivision tightens it for a fixed rail.
    /// (Raising the *degree* improves the true fit but not necessarily this bound —
    /// interval Horner of a high-degree σ-polynomial overestimates more; see the
    /// engineering log.)
    #[test]
    fn finer_subdivision_tightens_epsilon() {
        let chart = cone();
        let span = ivl(Q::new(1, 5), Q::new(4, 5));
        let mu_hat = fit_cut_rail(&chart, &y_cylinder(), &span, 4, RootPick::Upper, 44).unwrap();
        let read_eps = |subdiv: usize| -> Q {
            match certify(&chart, &mu_hat, &span, subdiv, Q::from_i128(1000)) {
                Verdict::Verified(v) => v.eps,
                Verdict::Unresolved(e) => e,
                other => panic!("unexpected {}", tag(&other)),
            }
        };
        let coarse = read_eps(12);
        let fine = read_eps(192);
        assert!(
            fine <= coarse,
            "subdiv-192 ε {fine:?} ≤ subdiv-12 ε {coarse:?}"
        );
    }

    /// A tight clearance the rational fit cannot meet is Unresolved (fail-closed), never
    /// a wrong Verified.
    #[test]
    fn a_tight_clearance_is_unresolved() {
        let chart = cone();
        let span = ivl(Q::new(1, 5), Q::new(4, 5));
        let mu_hat = fit_cut_rail(&chart, &y_cylinder(), &span, 2, RootPick::Upper, 44).unwrap();
        match certify(&chart, &mu_hat, &span, 48, Q::new(1, 1_000_000_000)) {
            Verdict::Unresolved(_) => {}
            other => panic!(
                "expected Unresolved at a tight clearance, got {}",
                tag(&other)
            ),
        }
    }

    /// The plane branch delegates to the exact rail (no float): Verified with ε ≈ 0.
    #[test]
    fn plane_branch_is_exact() {
        let chart = cone();
        let n = [Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)];
        let surface = CutSurface::Plane {
            n: n.clone(),
            d: Q::from_i128(1),
        };
        let span = ivl(Q::from_i128(1), Q::from_i128(3));
        let mu_hat = fit_cut_rail(&chart, &surface, &span, 0, RootPick::Upper, 44).unwrap();
        let cert = CutFitCert {
            mu_hat,
            w: Q::from_i128(0),
            surface,
            span,
            subdiv: 8,
            clearance: Q::new(1, 100),
            cfg: DevConfig::tight(),
        };
        match cut_fit(&chart, &cert) {
            Verdict::Verified(v) => assert!(v.eps <= Q::new(1, 1_000_000)),
            other => panic!("expected Verified, got {}", tag(&other)),
        }
    }

    fn tag<E, W: core::fmt::Debug, M>(v: &Verdict<E, W, M>) -> String {
        match v {
            Verdict::Verified(_) => "Verified".into(),
            Verdict::Refuted(w) => format!("Refuted({w:?})"),
            Verdict::Unresolved(_) => "Unresolved".into(),
        }
    }
}
