//! The normative device instances (spec §13).
//!
//! Currently the **cone** ([`cone`]): a rational right circular cone with apex at the
//! origin (`CONE(0)`) and half-angle ≈ 42° (`n·ẑ = 65/97 ≈ sin 42.07°`). The kernel is
//! exact over ℚ, so the spec's β = 42° is realized by the nearest convenient rational
//! cone; the device is a golden/validation instance, its geometry checked to tolerance.
//!
//! The petal conical flank (the general-case adversary) is not yet pinned by spec §13
//! and lands with milestone C.

use certify_core::Verdict;
use geom::chart::Chart;
use geom::record::{CertifiedChart, ChartDomain, ChartEvidence, RegEvidence, regularity_targets};
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

/// The searcher's [`RegEvidence`] for a derived target `(num, den)`: the claimed margin plus
/// honest Sturm chains of `den` and of the residual `R = num − m·den`. The checker re-derives
/// `(num, den)` from the chart and re-verifies these chains against them.
fn reg_evidence(target: &(Poly<Bignum>, Poly<Bignum>), m: Rat<Bignum>) -> RegEvidence<Bignum> {
    let (num, den) = target;
    let r = num.sub(&den.scale(&m));
    RegEvidence {
        margin: m,
        den_chain: SturmChain::new(den),
        res_chain: SturmChain::new(&r),
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
    // The certified domain: σ ∈ [0,1], μ ∈ [−1, −1/2], w ∈ [−1/4, 1/4].
    let domain = ChartDomain {
        sigma: Interval {
            lo: Rat::from_i128(0),
            hi: Rat::from_i128(1),
        },
        mu: (Rat::from_i128(-1), Rat::new(-1, 2)),
        w: (Rat::new(-1, 4), Rat::new(1, 4)),
    };

    // The searcher's evidence, built for the SAME targets the checker re-derives from
    // (chart, domain) — so the checker's re-derivation and these Sturm chains cannot diverge:
    //   |q|² = 97(1+σ²) ≥ 90, |n′|² ≥ 1/2, and det J > 0 at each (μ,w) box corner (≥ 1/100 —
    //   the inf corner (μ⁺, w⁻) bounds the other three pointwise, so one margin covers all).
    let t = regularity_targets(&chart, &domain);
    let evidence = ChartEvidence {
        q: reg_evidence(&t[0], Rat::from_i128(90)),
        ruling: reg_evidence(&t[1], Rat::new(1, 2)),
        slab: [
            reg_evidence(&t[2], Rat::new(1, 100)),
            reg_evidence(&t[3], Rat::new(1, 100)),
            reg_evidence(&t[4], Rat::new(1, 100)),
            reg_evidence(&t[5], Rat::new(1, 100)),
        ],
    };

    // Mesh κ-cap = min(s_max, 1/κ₁). R₁ = μ·(r′·n′)/|n′|² is affine in μ and non-σ-constant
    // (g(σ) = (r′·n′)/|n′|² = −130/(97(1+σ²))), so its box minimum — the tightest principal
    // radius — is attained at a corner. Searcher-derived; not part of the certified guarantee.
    let dj = chart.det_j();
    let n1_sq = chart.normal_deriv_sq().reduce();
    let g = |s: &Rat<Bignum>| dj.mu.eval(s).unwrap().div(&n1_sq.eval(s).unwrap());
    let (mu_lo, mu_hi) = (Rat::from_i128(-1), Rat::new(-1, 2));
    let (sig_lo, sig_hi) = (Rat::from_i128(0), Rat::from_i128(1));
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

    // Mint the certified record: `certify` re-derives |q|²/|n′|²/det J from (chart, domain),
    // recomputes the tag, and verifies this evidence against them — a `CertifiedChart` only if
    // all pass. The device cone is a golden artifact, so a refutation here is a bug, not an
    // admissible outcome.
    match CertifiedChart::certify(chart, domain, evidence, kappa_cap) {
        Verdict::Verified(c) => c,
        _ => panic!("the device cone must certify"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use certify_core::Verdict;
    use geom::record::ChartFault;
    use geom::tags::{Tag, classify};

    #[test]
    fn cone_is_a_cone_through_the_origin() {
        let apex = [Rat::from_i128(0), Rat::from_i128(0), Rat::from_i128(0)];
        assert_eq!(classify(&cone()), Some(Tag::Cone { apex }));
    }

    fn cone_domain() -> ChartDomain<Bignum> {
        ChartDomain {
            sigma: Interval {
                lo: Rat::from_i128(0),
                hi: Rat::from_i128(1),
            },
            mu: (Rat::from_i128(-1), Rat::new(-1, 2)),
            w: (Rat::new(-1, 4), Rat::new(1, 4)),
        }
    }
    fn cone_evidence(t: &[(Poly<Bignum>, Poly<Bignum>); 6]) -> ChartEvidence<Bignum> {
        ChartEvidence {
            q: reg_evidence(&t[0], Rat::from_i128(90)),
            ruling: reg_evidence(&t[1], Rat::new(1, 2)),
            slab: [
                reg_evidence(&t[2], Rat::new(1, 100)),
                reg_evidence(&t[3], Rat::new(1, 100)),
                reg_evidence(&t[4], Rat::new(1, 100)),
                reg_evidence(&t[5], Rat::new(1, 100)),
            ],
        }
    }
    fn poly(cs: &[i128]) -> Poly<Bignum> {
        Poly::from_coeffs(cs.iter().map(|&c| Rat::from_i128(c)).collect())
    }

    #[test]
    fn certified_cone_record_is_fully_verified() {
        // `certified_cone()` returns a `CertifiedChart` only by passing every M2 check in
        // `CertifiedChart::certify` — its existence *is* the certification (else it panics).
        let rec = certified_cone();
        assert!(matches!(rec.tag(), Tag::Cone { .. }));
        // Mesh κ-cap = min(s_max, 1/κ₁) = min(1, 65/194) = 65/194 — the tightest radius.
        assert_eq!(*rec.kappa_cap(), Rat::new(65, 194));
        // The certified domain is retained — a margin is meaningless without its domain.
        assert_eq!(rec.domain().sigma.hi, Rat::from_i128(1));
    }

    #[test]
    fn certify_refutes_a_bad_margin() {
        // The gate is real: a |q|² ≥ 10⁹ margin is false on the support, so REG-Q refutes and
        // no `CertifiedChart` is minted.
        let chart = cone();
        let domain = cone_domain();
        let t = regularity_targets(&chart, &domain);
        let mut ev = cone_evidence(&t);
        ev.q = reg_evidence(&t[0], Rat::from_i128(1_000_000_000));
        assert!(matches!(
            CertifiedChart::certify(chart, domain, ev, Rat::from_i128(0)),
            Verdict::Refuted(ChartFault::QReg(_))
        ));
    }

    #[test]
    fn certify_rejects_transplanted_evidence() {
        // Certificate transplantation — which private fields alone did NOT prevent: the
        // q-evidence's Sturm chains are for a DIFFERENT chart's |q|² ((x²+1)/1, not the cone's
        // 97+97σ²). `certify` re-derives the cone's |q|² and the transplanted chain fails to
        // verify against it ⇒ Refuted.
        let chart = cone();
        let domain = cone_domain();
        let t = regularity_targets(&chart, &domain);
        let mut ev = cone_evidence(&t);
        let wrong = (poly(&[1, 0, 1]), poly(&[1]));
        ev.q = reg_evidence(&wrong, Rat::new(1, 2));
        assert!(matches!(
            CertifiedChart::certify(chart, domain, ev, Rat::from_i128(0)),
            Verdict::Refuted(ChartFault::QReg(_))
        ));
    }

    #[test]
    fn certify_refutes_a_reversed_domain() {
        // A reversed σ interval (lo > hi) is malformed: `reg_q`'s Sturm count saturates, so it
        // could report zero roots and verify on the value at `lo` alone. The domain
        // well-formedness gate rejects it before any check runs.
        let chart = cone();
        let good = cone_domain();
        let t = regularity_targets(&chart, &good);
        let ev = cone_evidence(&t);
        let reversed = ChartDomain {
            sigma: Interval {
                lo: Rat::from_i128(1),
                hi: Rat::from_i128(0),
            },
            mu: (Rat::from_i128(-1), Rat::new(-1, 2)),
            w: (Rat::new(-1, 4), Rat::new(1, 4)),
        };
        assert!(matches!(
            CertifiedChart::certify(chart, reversed, ev, Rat::from_i128(0)),
            Verdict::Refuted(ChartFault::InvalidDomain)
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
