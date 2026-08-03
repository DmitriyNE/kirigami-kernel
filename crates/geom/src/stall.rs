//! The hatted stall calculus (spec §3.2.2): the regular chart fields at a
//! parametrization **stall**, where the normal derivative `n′` vanishes.
//!
//! On a span where `n′` has an order-`k` zero at `σ*`, write `p = (σ−σ*)^k` and record
//! its sign `ε = ±1` on the span. The **hatted** fields deflate that zero away:
//! `p̂ = ε·p > 0`, `n̂′ = n′/p̂`, `r̂ = r/p̂`, all regular at `σ*`. The hatted Jacobian is
//! `Ĵ = (c′+μ̂r̂′)·n̂′ + w·p̂|n̂′|²` with `μ̂ = p̂μ`.
//!
//! The load-bearing identity is **`J_raw = p̂·Ĵ`** — the raw Jacobian factors through the
//! *positive* `p̂`, never `p`. (Dividing by `p` instead is the "`/p̂` vs `/p`" fossil bug:
//! on an `ε = −1` span `p < 0`, so `/p` reverses orientation.) It holds because the stray
//! chain-rule term carries `r̂·n̂′ = (r·n′)/p̂² = 0` — the `r ⊥ n′` fact.
//!
//! [`hatted`] builds the deflated fields (exactly, or `None` if `p̂ ∤ n′` — the wrong
//! order); [`hatted_det_j`] forms `Ĵ`; [`raw_jacobian`] reconstructs `J_raw = p̂·Ĵ`.
//!
//! # Example
//!
//! ```
//! use geom::chart::Chart;
//! use geom::stall::{hatted, hatted_det_j, raw_jacobian, Stall};
//! use lattice::{Bignum, Poly, Rat, RatFunc};
//!
//! let poly = |cs: &[i128]| Poly::<Bignum>::from_coeffs(cs.iter().map(|&c| Rat::from_i128(c)).collect());
//! // A cone reparametrized by σ² folds at σ = 0: n′ gains a simple (order-1) zero there.
//! let q = [poly(&[9]), poly(&[4]), poly(&[0, 0, 4]), poly(&[0, 0, 9])]; // (9, 4, 4σ², 9σ²)
//! let chart = Chart::new(q, RatFunc::zero());
//! let stall = Stall { sigma_star: Rat::from_i128(0), order: 1, epsilon: 1 };
//!
//! let h = hatted(&chart, &stall).expect("an order-1 stall deflates exactly");
//! // The raw Jacobian factors through the positive p̂: J_raw = p̂·Ĵ.
//! assert_eq!(raw_jacobian(&h, &hatted_det_j(&chart, &h)).w, chart.det_j().w);
//! ```

use crate::chart::{Chart, DetJ};
use lattice::{Backend, Bignum, Poly, Rat, RatFunc, Vec3Rat};

/// A stall span: the normal derivative has an order-`order` zero at `sigma_star`, and `p`
/// keeps the constant sign `epsilon` (`±1`) across the span's interior.
pub struct Stall<B: Backend = Bignum> {
    /// The stall location `σ*`.
    pub sigma_star: Rat<B>,
    /// The zero order `k` of `n′` at `σ*`.
    pub order: usize,
    /// The sign `ε = ±1` of `p = (σ−σ*)^k` on the span (constant per span).
    pub epsilon: i8,
}

/// The deflated (hatted) fields at a stall — all regular at `σ*` (spec §3.2.2).
pub struct Hatted<B: Backend = Bignum> {
    /// The stall location `σ*` (carried from the [`Stall`]).
    pub sigma_star: Rat<B>,
    /// `p̂ = ε·(σ−σ*)^k`, strictly positive on the span interior.
    pub p_hat: Poly<B>,
    /// `n̂′ = n′/p̂` — the deflated normal derivative (nonzero at `σ*`).
    pub n1_hat: Vec3Rat<B>,
    /// `r̂ = r/p̂` — the deflated ruling.
    pub r_hat: Vec3Rat<B>,
    /// `|n̂′|²`.
    pub n1_hat_sq: RatFunc<B>,
}

/// `(σ − σ*)^k` as a polynomial.
fn stall_poly<B: Backend>(sigma_star: &Rat<B>, k: usize) -> Poly<B> {
    let base = Poly::from_coeffs(vec![sigma_star.neg(), Rat::from_i128(1)]);
    let mut acc = Poly::constant(Rat::from_i128(1));
    for _ in 0..k {
        acc = acc.mul(&base);
    }
    acc
}

/// Divide every numerator of `v` by `p` exactly (keeping the shared denominator), or
/// `None` if any division leaves a remainder — i.e. `p ∤ v`.
fn deflate<B: Backend>(v: &Vec3Rat<B>, p: &Poly<B>) -> Option<Vec3Rat<B>> {
    let mut num = [Poly::zero(), Poly::zero(), Poly::zero()];
    for (i, slot) in num.iter_mut().enumerate() {
        let (q, r) = v.num()[i].divrem(p);
        if !r.is_zero() {
            return None;
        }
        *slot = q;
    }
    Some(Vec3Rat::new(num, v.den().clone()))
}

/// Build the hatted fields at a stall, or `None` if `n′` does not have an order-`k` zero
/// at `σ*` (the deflation leaves a remainder — the removability order condition `m ≥ k`).
pub fn hatted<B: Backend>(chart: &Chart<B>, stall: &Stall<B>) -> Option<Hatted<B>> {
    let p = stall_poly(&stall.sigma_star, stall.order);
    let p_hat = p.scale(&Rat::from_i128(stall.epsilon as i128)); // ε·p
    let n1_hat = deflate(chart.normal_deriv(), &p_hat)?;
    let r_hat = deflate(chart.ruling(), &p_hat)?;
    let n1_hat_sq = n1_hat.dot(&n1_hat);
    Some(Hatted {
        sigma_star: stall.sigma_star.clone(),
        p_hat,
        n1_hat,
        r_hat,
        n1_hat_sq,
    })
}

/// The hatted Jacobian `Ĵ`, affine in `(μ, w)` after `μ̂ = p̂μ` (spec §3.2.2):
/// `Ĵ = c′·n̂′ + μ·p̂(r̂′·n̂′) + w·p̂|n̂′|²`.
pub fn hatted_det_j<B: Backend>(chart: &Chart<B>, h: &Hatted<B>) -> DetJ<B> {
    let p_hat = RatFunc::from_poly(h.p_hat.clone());
    DetJ {
        constant: chart.pedal().derivative().dot(&h.n1_hat),
        mu: h.r_hat.derivative().dot(&h.n1_hat).mul(&p_hat),
        w: h.n1_hat_sq.mul(&p_hat),
    }
}

/// Reconstruct the raw Jacobian from the hatted one via the identity `J_raw = p̂·Ĵ`
/// (spec §3.2.2 — the *positive* factor `p̂`, never `p`). Equals [`Chart::det_j`].
pub fn raw_jacobian<B: Backend>(h: &Hatted<B>, j_hat: &DetJ<B>) -> DetJ<B> {
    let p_hat = RatFunc::from_poly(h.p_hat.clone());
    DetJ {
        constant: j_hat.constant.mul(&p_hat),
        mu: j_hat.mu.mul(&p_hat),
        w: j_hat.w.mul(&p_hat),
    }
}

/// The stall-limit datum `(c′+μ̂r̂′)·n̂′` at `σ*`, evaluated at the two `μ̂` endpoints
/// (spec §3.2.2). Both must be `> 0` for the hatted chart to stay regular at the
/// endpoint; returns `None` if a denominator vanishes at `σ*`.
pub fn stall_limit<B: Backend>(
    chart: &Chart<B>,
    h: &Hatted<B>,
    mu_hat_lo: &Rat<B>,
    mu_hat_hi: &Rat<B>,
) -> Option<(Rat<B>, Rat<B>)> {
    // `reduce()` first: for h ≡ 0 the pedal is structurally zero but carries a
    // |n′|²-factor denominator that vanishes at σ*, so the raw form evaluates 0/0.
    let base = chart
        .pedal()
        .derivative()
        .dot(&h.n1_hat)
        .reduce()
        .eval(&h.sigma_star)?;
    let slope = h
        .r_hat
        .derivative()
        .dot(&h.n1_hat)
        .reduce()
        .eval(&h.sigma_star)?;
    Some((
        base.add(&slope.mul(mu_hat_lo)),
        base.add(&slope.mul(mu_hat_hi)),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn poly(cs: &[i128]) -> Poly<Bignum> {
        Poly::from_coeffs(cs.iter().map(|&c| Rat::from_i128(c)).collect())
    }
    /// A cone reparametrized by σ²: q(σ) = (9, 4, 4σ², 9σ²). n′ has a simple zero at σ=0.
    fn stalled_cone() -> Chart<Bignum> {
        let q = [poly(&[9]), poly(&[4]), poly(&[0, 0, 4]), poly(&[0, 0, 9])];
        Chart::new(q, RatFunc::zero())
    }
    fn stall(epsilon: i8) -> Stall<Bignum> {
        Stall {
            sigma_star: Rat::from_i128(0),
            order: 1,
            epsilon,
        }
    }
    fn detj_eq(a: &DetJ<Bignum>, b: &DetJ<Bignum>) -> bool {
        a.constant == b.constant && a.mu == b.mu && a.w == b.w
    }
    fn scale_detj(j: &DetJ<Bignum>, p: &Poly<Bignum>) -> DetJ<Bignum> {
        let rf = RatFunc::from_poly(p.clone());
        DetJ {
            constant: j.constant.mul(&rf),
            mu: j.mu.mul(&rf),
            w: j.w.mul(&rf),
        }
    }

    #[test]
    fn deflation_is_exact_and_regular() {
        let c = stalled_cone();
        let h = hatted(&c, &stall(1)).expect("order-1 stall deflates");
        // n̂′ is regular (nonzero) at the stall σ* = 0.
        let n_hat_0 = h.n1_hat.eval(&Rat::from_i128(0)).unwrap();
        assert!(n_hat_0.iter().any(|c| c.sign() != 0), "n̂′(σ*) ≠ 0");
        // A wrong (too-high) order does not divide exactly.
        let too_high = Stall {
            sigma_star: Rat::from_i128(0),
            order: 3,
            epsilon: 1,
        };
        assert!(hatted(&c, &too_high).is_none(), "p̂ ∤ n′ at the wrong order");
    }

    #[test]
    fn raw_equals_phat_times_jhat_both_signs() {
        let c = stalled_cone();
        let j_raw = c.det_j();
        for eps in [1i8, -1] {
            let h = hatted(&c, &stall(eps)).unwrap();
            let j_hat = hatted_det_j(&c, &h);
            assert!(
                detj_eq(&raw_jacobian(&h, &j_hat), &j_raw),
                "J_raw = p̂·Ĵ must hold for ε = {eps}",
            );
        }
    }

    #[test]
    fn fossil_p_breaks_the_identity_on_negative_epsilon() {
        // On an ε = −1 span, dividing by p (not p̂) flips the sign: p·Ĵ = −J_raw ≠ J_raw.
        let c = stalled_cone();
        let j_raw = c.det_j();
        let h = hatted(&c, &stall(-1)).unwrap();
        let j_hat = hatted_det_j(&c, &h);
        let p_raw = stall_poly(&Rat::from_i128(0), 1); // the fossil divisor p = σ (not p̂ = −σ)
        // The correct p̂ identity holds…
        assert!(detj_eq(&raw_jacobian(&h, &j_hat), &j_raw));
        // …but the fossil p does not: its w-coefficient has the wrong sign.
        assert!(
            scale_detj(&j_hat, &p_raw).w != j_raw.w,
            "p (not p̂) must break the identity",
        );
    }

    #[test]
    fn stall_limit_reports_both_endpoints() {
        let c = stalled_cone();
        let h = hatted(&c, &stall(1)).unwrap();
        let (lo, hi) = stall_limit(&c, &h, &Rat::from_i128(-1), &Rat::from_i128(1)).unwrap();
        // c ≡ 0 here (h ≡ 0), so the datum is μ̂·(r̂′·n̂′)|σ* — antisymmetric in ±μ̂.
        assert_eq!(lo, hi.neg());
    }
}
