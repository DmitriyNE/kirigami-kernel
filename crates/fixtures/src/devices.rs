//! The normative device instances (spec §13).
//!
//! - The **cone** ([`cone`]): a rational right circular cone with apex at the origin
//!   (`CONE(0)`) and half-angle ≈ 42° (`n·ẑ = 65/97 ≈ sin 42.07°`). The kernel is exact
//!   over ℚ, so the spec's β = 42° is realized by the nearest convenient rational cone;
//!   the device is a golden/validation instance, its geometry checked to tolerance.
//! - A **second-angle cone** ([`cone_alt`], `n·ẑ ≡ 3/5`) and the device **cylinder**
//!   ([`cylinder`], about the `x`-axis) — the generality corpus: two developable classes
//!   and two cone angles, so nothing downstream is locked to the 42° cone.
//!
//! The cylinder is the line-carrier flank the closure (M4) vertical slice is built on; a
//! genuine plane (a `planar` span, `n′ ≡ 0`) is not yet a [`Chart`] and the petal conical
//! flank is not yet pinned by spec §13 — both land in a later milestone-C pass
//! (`docs/closure-scoping.md §8`).

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

/// The device cone **re-centered on the seam** — the same cone [`cone`] reparametrized by the
/// exact rational Möbius `σ' = −1/σ`, so the lap-seam ruling (`φ₃D = ±π`, at `σ = ±∞` in the
/// canonical chart) becomes the **regular finite point `σ' = 0`** (Stage-2 S2 / DEV.3-β).
///
/// A half-turn about the axis is `φ₃D → φ₃D + π`, i.e. `arctan σ' = arctan σ + π/2 ⇒ σ' = −1/σ`.
/// Substituting `σ = −1/σ'` into `q = (9, 4, 4σ, 9σ)` and clearing the denominator (the
/// quaternion→rotation map is scale-invariant, `R(λq) = R(q)`) gives `q'(σ') = (9σ', 4σ', −4, −9)`
/// — still a degree-1 rational cone, `n·ẑ ≡ 65/97`, same development coefficient `c = 130/97`. The
/// normal fields coincide exactly under the reparametrization: `cone_seam().normal()(−1/σ) ≡
/// cone().normal()(σ)`. This is a **certification view**, not a second surface — it conditions the
/// seam so a subdivision certificate converges at finite σ' rather than the `σ → ±∞` singularity.
///
/// ```
/// use fixtures::devices::cone_seam;
/// use geom::tags::{classify, Tag};
/// use lattice::{Bignum, Rat};
///
/// // A cone through the origin — regular at the seam σ' = 0 (φ₃D = ±π, the back ruling).
/// let apex = [Rat::<Bignum>::from_i128(0), Rat::from_i128(0), Rat::from_i128(0)];
/// assert_eq!(classify(&cone_seam()), Some(Tag::Cone { apex }));
/// ```
pub fn cone_seam() -> Chart<Bignum> {
    let poly = |cs: &[i128]| Poly::from_coeffs(cs.iter().map(|&c| Rat::from_i128(c)).collect());
    let q = [poly(&[0, 9]), poly(&[0, 4]), poly(&[-4]), poly(&[-9])];
    Chart::new(q, RatFunc::zero())
}

/// The **lap-flap seam ramp** — the device cone's seam chart [`cone_seam`] carrying a nonzero
/// support ramp `h(σ') = ¼ − σ'/2` (Stage-2 S3 / spec §14 BONDED, `docs/paper.md §8`).
///
/// The lap flap climbs `Δ = ¼` to seat on the mated edge at the seam (`σ' = 0`), ramping back
/// down to the base cone (`h = 0`) at `σ' = ½`. The frame `q` is the cone's (the normal rides
/// the cone's Gauss circle), only the **support ramps** — a genuine **γ≠0** developable, yet
/// `ψ = c·arctan σ'` stays closed-form (`ψ` is `h`-independent). It is representable today
/// (`Chart::new` accepts any support) and its 3D surface `c + µr + wn` is exactly rational (the
/// pedal `c = h·n + (h′/|n′|²)·n′` carries the ramp). The BONDED certificate reads this surface
/// directly; the `develop::cone` pedal-nonzero rejection bites only the *flat* development
/// (emission), not the 3D bond. The normal-component of the pedal is exactly the support
/// (`c·n ≡ h`, since `n·n = 1` and `n′·n = 0`), so the normal separation from the base sheet
/// (`h = 0`) is precisely `h(σ')` — the quantity SEP/CLEAR certify.
///
/// ```
/// use fixtures::devices::cone_seam_ramp;
///
/// // A nonzero support ⇒ nonzero pedal ⇒ NOT an apex cone (γ ≠ 0).
/// assert!(!cone_seam_ramp().pedal().is_zero());
/// ```
pub fn cone_seam_ramp() -> Chart<Bignum> {
    let poly = |cs: &[i128]| Poly::from_coeffs(cs.iter().map(|&c| Rat::from_i128(c)).collect());
    let q = [poly(&[0, 9]), poly(&[0, 4]), poly(&[-4]), poly(&[-9])];
    // h(σ') = 1/4 − σ'/2: Δ = 1/4 at the seam σ' = 0, rejoining h = 0 at σ' = 1/2.
    let h = RatFunc::from_poly(Poly::from_coeffs(vec![Rat::new(1, 4), Rat::new(-1, 2)]));
    Chart::new(q, h)
}

/// The **wrapping cone** — the *same* device cone surface as [`cone`] (apex at the origin,
/// `n·ẑ ≡ 65/97`, half-angle ≈ 42°), but parametrized to traverse its Gauss circle **twice** over
/// `σ ∈ ℝ`, so a single chart covers **more than one full turn** of azimuth. This is the chart the
/// self-lapping demo needs: the body, the seam, and the lapped tail all live in **one** connected
/// σ-window, with **no coordinate singularity**.
///
/// **Why degree-2.** The device cone [`cone`] is `q(σ) = q_a + σ·q_b` with `q_a = (9,4,0,0)`,
/// `q_b = (0,0,4,9)` — an affine *line* in the 2-plane `span(q_a, q_b)` (`|q_a|² = |q_b|² = 97`,
/// `q_a·q_b = 0`). Its Hopf image sweeps azimuth `φ₃D = 2·arctan σ`, exactly **one** `2π` turn as
/// `σ: −∞→∞`, with the seam (`φ₃D = ±π`) stranded at `σ = ±∞` where `n′ → 0` *stalls* (a removable
/// coordinate singularity, not geometric). Any curve confined to that **same 2-plane** Hopf-maps to
/// the **same** latitude circle regardless of how fast it moves; the degree-2 curve
///
/// > `q(σ) = (1 − σ²)·q_a + 2σ·q_b = (9 − 9σ², 4 − 4σ², 8σ, 18σ)`
///
/// stays in the plane (so `n·ẑ ≡ 65/97` still holds, `|q|² = 97(1+σ²)²`) but sweeps
/// `φ₃D = 4·arctan σ` — a full `4π`, **two** turns, as `σ: −∞→∞`. One turn-plus-lap therefore fits a
/// **finite** window straddling `σ = 0` (e.g. `σ ∈ [−1.14, 1.14]` → ≈ 390° = one turn + 30° lap),
/// the seam rulings sitting at the **finite, regular** `σ = ±1` (`φ₃D = ±π`) where `|n′|` is bounded
/// away from zero (`min |n′| ≈ 1.29` on the window). No `σ → ∞`, no stall.
///
/// **The angle stays closed-form.** Because the normal still rides the same circle, the textbook cone
/// law `ψ = sinβ·φ₃D` gives `ψ = (65/97)·4·arctan σ = (260/97)·arctan σ` — the exact same single-
/// arctangent shape as [`cone`], only with coefficient `c = 260/97` (twice the degree-1 `130/97`).
/// `develop::cone::cone_angle_coeff` recognises it verbatim (`ψ′ = (260/97)/(1+σ²)`), so the whole
/// `ConeDevelopment` machinery develops the wrapping cone with **no new angle integrator**; only the
/// tail's support ramp needs the DD.2 flat-directrix quadrature.
///
/// ```
/// use fixtures::devices::cone_wrap;
/// use geom::tags::{classify, Tag};
/// use lattice::{Bignum, Rat};
///
/// // Still a cone with apex at the origin (h ≡ 0) — the same surface, wrapped.
/// let apex = [Rat::<Bignum>::from_i128(0), Rat::from_i128(0), Rat::from_i128(0)];
/// assert_eq!(classify(&cone_wrap()), Some(Tag::Cone { apex }));
/// ```
pub fn cone_wrap() -> Chart<Bignum> {
    wrap_cone(&Rat::from_i128(9), &Rat::from_i128(4))
}

/// The **wrapping cone of any rational half-angle** — [`cone_wrap`] with its generator exposed.
///
/// `q(σ) = (1 − σ²)·(a, b, 0, 0) + 2σ·(0, 0, b, a)`, so `|q_a|² = |q_b|² = a² + b²` and
/// `q_a · q_b = 0` for *any* `(a, b)`: the degree-2 wrap and the closed-form angle law survive the
/// generalization untouched, and only the half-angle moves. The exact invariant is
///
/// > `sin β = n·ẑ = (a² − b²)/(a² + b²)`,
///
/// which is the **Pythagorean generator**, and that is why a rational half-angle is not a lucky
/// special case: writing `t = b/a`, `sin β = (1 − t²)/(1 + t²)`, so `t = tan(45° − β/2)` and a
/// rational `t` gives an exact cone. Conversely a rational *direction* `(cos β, sin β)` — that is,
/// a Pythagorean pair — always yields a rational `t` by two half-angle steps, so **a Pythagorean
/// apex direction costs exactly zero**. [`cone_wrap`]'s `(9, 4)` is the 42° device:
/// `sin β = 65/97`, `tan(β/2) = 5/13`, `t = 4/9`.
///
/// Requires `a > b > 0` (a genuine cone, `0 < β < 90°`); other inputs are the caller's to reject —
/// this is the bare chart constructor, and [`acceptance`](https://docs.rs/acceptance) validates.
///
/// ```
/// use fixtures::devices::{cone_wrap, wrap_cone};
/// use lattice::{Bignum, Poly, Rat, RatFunc};
///
/// // The device chart is this one at (9, 4) — same q, coefficient for coefficient.
/// let g = wrap_cone::<Bignum>(&Rat::from_i128(9), &Rat::from_i128(4));
/// assert_eq!(g.quaternion(), cone_wrap().quaternion());
/// assert_eq!(g.support(), cone_wrap().support());
///
/// // A different half-angle: (2, 1) → sin β = 3/5, the `cone_alt` angle, wrapped.
/// let nz = wrap_cone::<Bignum>(&Rat::from_i128(2), &Rat::from_i128(1)).normal().comp(2);
/// assert_eq!(nz, RatFunc::from_poly(Poly::constant(Rat::new(3, 5))));
/// ```
pub fn wrap_cone<B: lattice::Backend>(a: &Rat<B>, b: &Rat<B>) -> Chart<B> {
    let zero = Rat::from_i128(0);
    // (1 − σ²)·a and (1 − σ²)·b — the q_a half, quadratic in σ.
    let scaled = |c: &Rat<B>| Poly::from_coeffs(vec![c.clone(), zero.clone(), c.neg()]);
    // 2σ·b and 2σ·a — the q_b half, linear in σ.
    let twice = |c: &Rat<B>| Poly::from_coeffs(vec![zero.clone(), c.mul(&Rat::from_i128(2))]);
    Chart::new([scaled(a), scaled(b), twice(b), twice(a)], RatFunc::zero())
}

/// A **second-angle** rational cone (apex at the origin, `h ≡ 0`, `n·ẑ ≡ 3/5 ≈ sin 36.87°`) —
/// a generality witness distinct from [`cone`]'s 65/97, so the closure pipe is demonstrably
/// not locked to one half-angle.
///
/// Built from `q(σ) = (2, 1, σ, 2σ)`: `|q|² = 5(1 + σ²)` and the normal's `z`-numerator is
/// `3(1 + σ²)`, so `n·ẑ ≡ 3/5` exactly, constant along and across rulings (the cone invariant).
///
/// ```
/// use fixtures::devices::cone_alt;
/// use lattice::{Bignum, Poly, Rat, RatFunc};
///
/// // n·ẑ ≡ 3/5 — a different cone than the device's 65/97.
/// let nz = cone_alt().normal().comp(2);
/// assert_eq!(nz, RatFunc::from_poly(Poly::<Bignum>::constant(Rat::new(3, 5))));
/// ```
pub fn cone_alt() -> Chart<Bignum> {
    let poly = |cs: &[i128]| Poly::from_coeffs(cs.iter().map(|&c| Rat::from_i128(c)).collect());
    let q = [poly(&[2]), poly(&[1]), poly(&[0, 1]), poly(&[0, 2])];
    Chart::new(q, RatFunc::zero())
}

/// The device **cylinder**: a right circular cylinder about the `x`-axis (`n_x ≡ 0`), from
/// `q(σ) = (1, σ, 0, 0)` with `h ≡ 0`.
///
/// A cylinder is the representable developable whose ruling cut-edges are straight **lines**
/// (spec §8.5, *cylinder-type ⇒ ruling lines*), so it is the line-carrier flank the closure
/// (M4) vertical slice is built on — unlike a genuine plane, which is a `planar` span
/// (`n′ ≡ 0`) not yet representable as a [`Chart`] (`docs/closure-scoping.md §8`), the cylinder
/// still carries a moving normal to drive the per-flank regularity checks.
///
/// ```
/// use fixtures::devices::cylinder;
/// use geom::tags::{classify, Tag};
/// use lattice::{Bignum, Rat};
///
/// match classify(&cylinder()) {
///     Some(Tag::Cylinder { axis }) => assert_eq!(
///         axis,
///         [Rat::<Bignum>::from_i128(1), Rat::from_i128(0), Rat::from_i128(0)],
///     ),
///     other => panic!("expected a cylinder, got {other:?}"),
/// }
/// ```
pub fn cylinder() -> Chart<Bignum> {
    let poly = |cs: &[i128]| Poly::from_coeffs(cs.iter().map(|&c| Rat::from_i128(c)).collect());
    let q = [poly(&[1]), poly(&[0, 1]), poly(&[0]), poly(&[0])];
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

    #[test]
    fn cone_alt_is_a_distinct_angle_cone() {
        // A cone through the origin, but a different half-angle than the device cone.
        let apex = [Rat::from_i128(0), Rat::from_i128(0), Rat::from_i128(0)];
        assert_eq!(classify(&cone_alt()), Some(Tag::Cone { apex }));
        // n·ẑ ≡ 3/5, not 65/97 — the generality witness.
        let nz = cone_alt().normal().comp(2);
        assert_eq!(
            nz,
            RatFunc::from_poly(poly(&[3])).div(&RatFunc::from_poly(poly(&[5])))
        );
        assert_ne!(nz, cone().normal().comp(2));
    }

    #[test]
    fn cone_wrap_is_the_same_cone_traversed_twice() {
        // Still a cone with apex at the origin — the same surface as `cone`, only wrapped.
        let apex = [Rat::from_i128(0), Rat::from_i128(0), Rat::from_i128(0)];
        assert_eq!(classify(&cone_wrap()), Some(Tag::Cone { apex }));
        // n·ẑ ≡ 65/97 — identical to the device cone (the normal rides the same latitude circle).
        assert_eq!(cone_wrap().normal().comp(2), cone().normal().comp(2));
        assert_eq!(
            cone_wrap().normal().comp(2),
            RatFunc::from_poly(poly(&[65])).div(&RatFunc::from_poly(poly(&[97]))),
        );
        // The ruling is non-degenerate on the whole finite window — no σ=∞ stall: |n′|² > 0 at the
        // seam ruling σ = 1 (φ₃D = π), where the degree-1 chart would have stalled at σ = ∞.
        let n1_sq_at_1 = cone_wrap()
            .normal_deriv_sq()
            .eval(&Rat::from_i128(1))
            .unwrap();
        assert!(n1_sq_at_1.sign() > 0);
    }

    #[test]
    fn cylinder_is_a_cylinder_about_x() {
        // A second developable class: the normal traces the great circle n_x ≡ 0.
        assert_eq!(
            classify(&cylinder()),
            Some(Tag::Cylinder {
                axis: [Rat::from_i128(1), Rat::from_i128(0), Rat::from_i128(0)],
            }),
        );
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

    #[test]
    fn cone_seam_is_the_device_cone_recentered_on_the_seam() {
        // The seam chart is the SAME cone reparametrized by σ' = −1/σ: identical half-angle, and
        // the normal fields coincide exactly under the reparametrization — n_seam(−1/σ) ≡ n_cone(σ).
        let (c, cs) = (cone(), cone_seam());

        // A cone through the origin, same exact invariant n·ẑ ≡ 65/97 as the canonical chart.
        assert_eq!(
            classify(&cs),
            Some(Tag::Cone {
                apex: [Rat::from_i128(0), Rat::from_i128(0), Rat::from_i128(0)]
            })
        );
        let want = RatFunc::from_poly(Poly::<Bignum>::constant(Rat::new(65, 97)));
        assert_eq!(cs.normal().comp(2), want);
        assert_eq!(cs.normal().comp(2), c.normal().comp(2));

        // The reparametrization identity at a sample: σ = 2 ↔ σ' = −1/2, all three components equal.
        let sigma = Rat::<Bignum>::from_i128(2);
        let sigma_p = Rat::<Bignum>::new(-1, 2);
        for i in 0..3 {
            assert_eq!(
                cs.normal().comp(i).eval(&sigma_p).unwrap(),
                c.normal().comp(i).eval(&sigma).unwrap(),
                "normal component {i} disagrees under σ' = −1/σ"
            );
        }

        // The seam ruling (σ = ±∞ canonically) is the REGULAR finite point σ' = 0:
        // n(0) = [0, 72, 65]/97 finite, and the ruling never stalls there (|n′|² > 0).
        let zero = Rat::<Bignum>::from_i128(0);
        assert_eq!(cs.normal().comp(0).eval(&zero).unwrap(), Rat::from_i128(0));
        assert_eq!(cs.normal().comp(1).eval(&zero).unwrap(), Rat::new(72, 97));
        assert_eq!(cs.normal().comp(2).eval(&zero).unwrap(), Rat::new(65, 97));
        assert!(cs.normal_deriv_sq().eval(&zero).unwrap() > Rat::from_i128(0));
    }

    #[test]
    fn cone_seam_ramp_is_a_gamma_nonzero_lap_flap() {
        let r = cone_seam_ramp();
        // γ ≠ 0: the support ramps, so the pedal is nonzero — NOT an apex cone.
        assert!(!r.pedal().is_zero());

        // The normal-component of the pedal is exactly the support ramp (c·n ≡ h) — the sheet
        // separation from the base cone (h = 0) that SEP/CLEAR read.
        let h =
            RatFunc::<Bignum>::from_poly(Poly::from_coeffs(vec![Rat::new(1, 4), Rat::new(-1, 2)]));
        assert_eq!(r.pedal().dot(r.normal()).reduce(), h.reduce());
        // Δ = 1/4 at the seam σ' = 0 (lapped); h = 0 at σ' = 1/2 (rejoins the base cone).
        assert_eq!(h.eval(&Rat::from_i128(0)).unwrap(), Rat::new(1, 4));
        assert_eq!(h.eval(&Rat::new(1, 2)).unwrap(), Rat::from_i128(0));

        // The 3D surface c + µr + wn is exactly rational and defined on the seam neighborhood.
        let surf = r.surface(&Rat::from_i128(-1), &Rat::from_i128(0)); // µ = −1 rail, w = 0
        assert!(surf.eval(&Rat::from_i128(0)).is_some());
    }
}
