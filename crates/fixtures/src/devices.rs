//! The normative device instances (spec §13).
//!
//! Currently the **cone** ([`cone`]): a rational right circular cone with apex at the
//! origin (`CONE(0)`) and half-angle ≈ 42° (`n·ẑ = 65/97 ≈ sin 42.07°`). The kernel is
//! exact over ℚ, so the spec's β = 42° is realized by the nearest convenient rational
//! cone; the device is a golden/validation instance, its geometry checked to tolerance.
//!
//! The petal conical flank (the general-case adversary) is not yet pinned by spec §13
//! and lands with milestone C.

use certify_core::MarginSq;
use certify_core::Verdict;
use certify_core::certify1d::{RegCert, SlabS0Cert};
use geom::chart::Chart;
use geom::record::CertifiedChart;
use geom::tags::classify;
use lattice::{Bignum, Interval, Poly, Rat, RatFunc, SturmChain};

/// The device cone (spec §13): a rational right circular cone, apex at the origin, axis
/// `ẑ`, half-angle ≈ 42°.
///
/// Built from `q(σ) = (9, 4, 4σ, 9σ)` with `h ≡ 0` (rulings through the origin). Its
/// normal satisfies the exact cone invariant `n·ẑ ≡ 65/97`.
pub fn cone() -> Chart<Bignum> {
    let poly = |cs: &[i128]| Poly::from_coeffs(cs.iter().map(|&c| Rat::from_i128(c)).collect());
    let q = [poly(&[9]), poly(&[4]), poly(&[0, 4]), poly(&[0, 9])];
    Chart::new(q, RatFunc::zero())
}

/// A REG-Q positivity certificate on `num/den ≥ m` over `span`, with searcher-honest Sturm
/// chains of `den` and of the residual `R = num − m·den`.
fn reg_cert(
    num: Poly<Bignum>,
    den: Poly<Bignum>,
    m: Rat<Bignum>,
    span: Interval<Bignum>,
) -> RegCert<Bignum> {
    let r = num.sub(&den.scale(&m));
    RegCert {
        den_chain: SturmChain::new(&den),
        res_chain: SturmChain::new(&r),
        num,
        den,
        m: MarginSq(m),
        span,
    }
}

/// The certified single-chart record for the device cone (spec §8) — the Milestone-B exit
/// artifact. Builds [`cone`]'s M2 certificates and the mesh curvature cap over the
/// representative support arc `σ ∈ [0, 1]`, `μ ∈ [−1, −1/2]`, `w ∈ [−1/4, 1/4]`, then mints
/// a [`CertifiedChart`] through [`CertifiedChart::certify`] (which re-runs every checker):
///
/// - CONE tag (rulings through the origin, `h ≡ n·0`);
/// - REG-Q on `|q|² = 97(1 + σ²)` (the quaternion spline never degenerates);
/// - REG-Q on `|n′|² = 20736 / (9409(1 + σ²)²)` (the ruling never stalls);
/// - SLAB-S0 on `det J` at the box's inf corner (the offset slab stays regular);
/// - mesh κ-cap `min(s_max, 1/κ₁) = min(1, 65/194) = 65/194` (the tightest principal radius,
///   at the corner `σ = 1, μ⁺`; `R₁` is non-uniform in σ though the half-angle is constant).
pub fn certified_cone() -> CertifiedChart<Bignum> {
    let chart = cone();
    let tag = classify(&chart).expect("the device cone classifies as CONE");
    let span = Interval {
        lo: Rat::from_i128(0),
        hi: Rat::from_i128(1),
    };

    // REG-Q on |q|² = 97 + 97σ² ≥ 90 (den = 1): the quaternion spline never degenerates.
    let q_cert = reg_cert(
        chart.normal().den().clone(),
        Poly::constant(Rat::from_i128(1)),
        Rat::from_i128(90),
        span.clone(),
    );

    // REG-Q on |n′|² = 20736 / (9409(1+σ²)²) ≥ 1/2 on [0,1]: the ruling never stalls.
    let n1_sq = chart.normal_deriv_sq().reduce();
    let ruling_cert = reg_cert(
        n1_sq.num().clone(),
        n1_sq.den().clone(),
        Rat::new(1, 2),
        span.clone(),
    );

    // SLAB-S0: det J > 0 at the box's inf corner (μ⁺ = −1/2, w⁻ = −1/4). det J is affine in
    // (μ, w); with the pedal at the apex (h ≡ 0), det J = μ·(r′·n′) + w·|n′|².
    let dj = chart.det_j();
    let (mu_hi, w_lo) = (Rat::new(-1, 2), Rat::new(-1, 4));
    let det_j_inf = dj
        .constant
        .add(&dj.mu.scale(&mu_hi))
        .add(&dj.w.scale(&w_lo))
        .reduce();
    let slab_cert = SlabS0Cert {
        core: reg_cert(
            det_j_inf.num().clone(),
            det_j_inf.den().clone(),
            Rat::new(1, 100),
            span.clone(),
        ),
        stall_end: None,
    };

    // Mesh κ-cap = min(s_max, 1/κ₁). The principal radius R₁ = μ·(r′·n′)/|n′|² is affine in
    // μ and monotone in σ on the support — `g(σ) = (r′·n′)/|n′|² = −130/(97(1+σ²))`, so R₁ is
    // *not* σ-constant (the σ-parametrization is non-uniform though the half-angle is fixed).
    // Its box minimum — the support's tightest radius — is therefore attained at a corner.
    let g = |s: &Rat<Bignum>| dj.mu.eval(s).unwrap().div(&n1_sq.eval(s).unwrap());
    let (mu_lo, sig_lo, sig_hi) = (Rat::new(-1, 1), Rat::from_i128(0), Rat::from_i128(1));
    let corners = [
        (&sig_lo, &mu_lo),
        (&sig_lo, &mu_hi),
        (&sig_hi, &mu_lo),
        (&sig_hi, &mu_hi),
    ];
    let mut r1_min = mu_hi.mul(&g(&sig_hi)); // (σ=1, μ⁺): the tightest corner
    for (s, mu) in corners {
        let r1 = mu.mul(&g(s));
        if r1.cmp(&r1_min) == core::cmp::Ordering::Less {
            r1_min = r1;
        }
    }
    let s_max = Rat::from_i128(1);
    let kappa_cap = if r1_min.cmp(&s_max) == core::cmp::Ordering::Less {
        r1_min
    } else {
        s_max
    };

    // Mint the certified record: `certify` re-runs REG-Q / SLAB-S0 over the certificates
    // and hands back a `CertifiedChart` only if all verify. The device cone is a golden
    // artifact, so a refutation here is a fixture/kernel bug, not an admissible outcome.
    match CertifiedChart::certify(chart, tag, q_cert, ruling_cert, slab_cert, kappa_cap) {
        Verdict::Verified(c) => c,
        _ => panic!("the device cone must certify"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use certify_core::Verdict;
    use geom::record::ChartFault;
    use geom::tags::Tag;

    #[test]
    fn cone_is_a_cone_through_the_origin() {
        let apex = [Rat::from_i128(0), Rat::from_i128(0), Rat::from_i128(0)];
        assert_eq!(classify(&cone()), Some(Tag::Cone { apex }));
    }

    #[test]
    fn certified_cone_record_is_fully_verified() {
        // `certified_cone()` returns a `CertifiedChart` only by passing every M2 checker in
        // `CertifiedChart::certify` — its existence *is* the certification (else it panics).
        let rec = certified_cone();
        assert!(matches!(rec.tag(), Tag::Cone { .. }));
        // Mesh κ-cap = min(s_max, 1/κ₁) = min(1, 65/194) = 65/194 — the tightest radius.
        assert_eq!(*rec.kappa_cap(), Rat::new(65, 194));
    }

    #[test]
    fn certify_refutes_a_bad_certificate() {
        // The gate is real: a |q|² ≥ 10⁹ margin is false on the support, so REG-Q refutes and
        // no `CertifiedChart` is minted — the forgeable "put Verified in a field" path is gone
        // (only `certify` builds one, and only when the checkers pass).
        let chart = cone();
        let tag = classify(&chart).unwrap();
        let span = Interval {
            lo: Rat::from_i128(0),
            hi: Rat::from_i128(1),
        };
        let bad_q = reg_cert(
            chart.normal().den().clone(),
            Poly::constant(Rat::from_i128(1)),
            Rat::from_i128(1_000_000_000),
            span.clone(),
        );
        // Trivially-valid ruling/slab certs (never reached — q refutes first).
        let ok = || {
            reg_cert(
                Poly::constant(Rat::from_i128(1)),
                Poly::constant(Rat::from_i128(1)),
                Rat::new(1, 2),
                span.clone(),
            )
        };
        let slab = SlabS0Cert {
            core: ok(),
            stall_end: None,
        };
        assert!(matches!(
            CertifiedChart::certify(chart, tag, bad_q, ok(), slab, Rat::from_i128(0)),
            Verdict::Refuted(ChartFault::QReg(_))
        ));
    }

    #[test]
    fn cone_principal_radius_shrinks_along_sigma() {
        // R₁ per unit μ is `g(σ) = (r′·n′)/|n′|² = −130/(97(1+σ²))` — *not* σ-constant: the
        // half-angle is fixed (n·ẑ ≡ 65/97) but the σ-parametrization is non-uniform, so the
        // radius shrinks with σ and the κ-cap must be the domain minimum, not a fixed station.
        let c = cone();
        let (dj, n1_sq) = (c.det_j(), c.normal_deriv_sq().reduce());
        let g = |s: Rat<Bignum>| dj.mu.eval(&s).unwrap().div(&n1_sq.eval(&s).unwrap());
        assert_eq!(g(Rat::from_i128(0)), Rat::new(-130, 97)); // −130/(97·1)
        assert_eq!(g(Rat::from_i128(1)), Rat::new(-65, 97)); //  −130/(97·2)
        assert_eq!(g(Rat::new(1, 3)), Rat::new(-117, 97)); // −130/(97·10/9)
    }

    #[test]
    fn cone_axis_angle_exact_and_near_42_degrees() {
        let c = cone();
        // Exact cone invariant: n·ẑ ≡ 65/97 (constant along and across rulings).
        let nz = c.normal().comp(2);
        let want = RatFunc::from_poly(Poly::<Bignum>::constant(Rat::new(65, 97)));
        assert_eq!(nz, want);

        // Validation to tolerance (exact rationals, no floats): 65/97 vs sin 42° ≈ 669/1000.
        let diff = Rat::<Bignum>::new(65, 97).sub(&Rat::new(669, 1000));
        let tol = Rat::new(1, 100);
        assert!(
            diff < tol && diff > tol.neg(),
            "half-angle within ~1° of 42°"
        );
    }
}
