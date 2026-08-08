//! Trim/clip **searcher**: build a joint's retained-side field `G_i` and hand the CLIP-DOM
//! ladder and TRIM-LOCAL certificates to the pure-tier `certify_core::certify1d` checkers.
//!
//! `closure` is the untrusted searcher: it *constructs* the bisector `b_J = s_J·(n_A − n_B)`
//! and the retained-side field
//!
//! ```text
//! G_i(σ, μ, w) = (C_i(σ, μ, w) − x₀) · b_i,   C_i = pedal_i + μ·ruling_i + w·normal_i
//! ```
//!
//! from the two flank charts (`b_A = b_J`, `b_B = −b_J`), then *proposes* the positivity and
//! transversality certificates. The pure-tier checkers re-verify every Sturm chain and decide:
//! [`trim_local`](certify_core::certify1d::trim_local) (the clip stays local),
//! [`clip`](certify_core::certify1d::clip) (the CLIP-W → CLIP-μ → per-zero transversality
//! ladder), and [`clip_dom`](certify_core::certify1d::clip_dom) (the retained-support census).
//! No new checker is minted here — C3 is the missing *producer* for checkers that already exist.
//!
//! **`G_i` is affine in `(μ, w)` and rational in `σ`**, so it is carried by three σ-rational
//! coefficients — `g0 = (pedal − x₀)·b`, `g_mu = ruling·b`, `g_w = normal·b` — assembled by
//! [`field_a`] / [`field_b`]. Everything the checkers need is an evaluation or a squared gauge
//! of these three: box corners for [`TrimLocalCert`] and
//! the census, the cleared `(∂_wG)² = g_w²` gauge for CLIP-W, `(∂_μG)² = g_mu²` for CLIP-μ, and
//! `∂_σG` corners for the signed CLIP-σ rung.
//!
//! **SIDE and COLLAR (deferred from C2) land here.** SIDE(b_J)'s support-level *wrong-side* test
//! — the retained side is uniformly `G_i > 0` over the support — **is** TRIM-LOCAL's outer-fiber
//! corner positivity ([`trim_local`](certify_core::certify1d::trim_local) refutes a `G_i ≤ 0`
//! corner with [`RegFault::OuterFiber`](certify_core::certify1d::RegFault::OuterFiber)); no
//! separate checker is needed. COLLAR's cross-`t` TUBE padding `D²_collar = 4w²s²|V|²/(1+s²|V|²)`
//! is **vacuous on the straight-crease scope** (`κ_max = 0` ⇒ zero tube width, spec §13), so it
//! carries no runtime obligation here and TUBE-LOCAL / TUBE-SELF discharge totally.
//!
//! Nothing keys on the flank *type*: [`field_a`] / [`field_b`] consume arbitrary
//! [`geom::chart::Chart`]s, and the CLIP verdict falls out of the checkers' Sturm counts, never a
//! Rust branch. On a cylinder the rulings are crease-parallel so `g_mu ≡ 0` and `G_i` is w-only;
//! a cone's fanning rulings give a genuinely `μ`-dependent `G_i` through the same code.

use certify_core::MarginSq;
use certify_core::certify1d::{RegCert, TrimLocalCert};
use lattice::{Backend, Interval, Poly, Rat, RatFunc, SturmChain, Vec3Rat};

use crate::{Joint, JointSign, MuRange};

/// The retained-side field `G_i` of one flank, as its three σ-rational coefficients in the
/// affine `(μ, w)` expansion `G_i(σ, μ, w) = g0(σ) + μ·g_mu(σ) + w·g_w(σ)`.
///
/// `g_w = normal·b = ∂_wG` and `g_mu = ruling·b = ∂_μG` are exactly the CLIP-W and CLIP-μ
/// gauges; `g0 = (pedal − x₀)·b` is the crease-wall value. Build one with [`field_a`] /
/// [`field_b`].
pub struct GField<B: Backend> {
    /// The crease-wall coefficient `g0 = (pedal − x₀)·b` (the value of `G_i` at `μ = w = 0`).
    pub g0: RatFunc<B>,
    /// The ruling coefficient `g_mu = ruling·b = ∂_μG` (the CLIP-μ gauge; `≡ 0` for a cylinder).
    pub g_mu: RatFunc<B>,
    /// The normal coefficient `g_w = normal·b = ∂_wG` (the CLIP-W gauge).
    pub g_w: RatFunc<B>,
}

impl<B: Backend> GField<B> {
    /// The field restricted to one fiber `(μ, w)`: the σ-rational function `G_i(·, μ, w)`.
    pub fn fiber(&self, mu: &Rat<B>, w: &Rat<B>) -> RatFunc<B> {
        self.g0.add(&self.g_mu.scale(mu)).add(&self.g_w.scale(w))
    }

    /// `G_i(σ, μ, w)`, or `None` if the field is singular at `σ` (no rational value there).
    pub fn eval(&self, sigma: &Rat<B>, mu: &Rat<B>, w: &Rat<B>) -> Option<Rat<B>> {
        self.fiber(mu, w).eval(sigma)
    }

    /// The four box-corner values of `G_i` at a fixed station `σ`, over the ruling box
    /// `μ ∈ [μ⁻, μ⁺] × w ∈ [w⁻, w⁺]` — the corner array [`classify_fiber`] and
    /// [`trim_local`] consume. `G_i` is affine on the box, so the four corners fix the cell.
    /// `None` if the field is singular at `σ`.
    ///
    /// [`classify_fiber`]: certify_core::certify1d::classify_fiber
    /// [`trim_local`]: certify_core::certify1d::trim_local
    pub fn corners(&self, sigma: &Rat<B>, mu: &MuRange<B>, w: &Interval<B>) -> Option<[Rat<B>; 4]> {
        Some([
            self.eval(sigma, &mu.lo, &w.lo)?,
            self.eval(sigma, &mu.lo, &w.hi)?,
            self.eval(sigma, &mu.hi, &w.lo)?,
            self.eval(sigma, &mu.hi, &w.hi)?,
        ])
    }

    /// The four box-corner values of the signed `∂_σG` at a fixed station `σ` — the corner
    /// array a [`ClipSigmaCert`] carries for the per-zero CLIP-σ rung. `∂_σG = g0′ + μ·g_mu′ +
    /// w·g_w′` is affine in `(μ, w)`, so its box range is its corner range. `None` if singular.
    ///
    /// [`ClipSigmaCert`]: certify_core::certify1d::ClipSigmaCert
    pub fn sigma_deriv_corners(
        &self,
        sigma: &Rat<B>,
        mu: &MuRange<B>,
        w: &Interval<B>,
    ) -> Option<[Rat<B>; 4]> {
        let d0 = self.g0.derivative().eval(sigma)?;
        let dmu = self.g_mu.derivative().eval(sigma)?;
        let dw = self.g_w.derivative().eval(sigma)?;
        let at = |mu: &Rat<B>, w: &Rat<B>| d0.add(&dmu.mul(mu)).add(&dw.mul(w));
        Some([
            at(&mu.lo, &w.lo),
            at(&mu.lo, &w.hi),
            at(&mu.hi, &w.lo),
            at(&mu.hi, &w.hi),
        ])
    }
}

/// A constant vector as a degree-0 [`Vec3Rat`] (denominator `1`), so it can be subtracted from
/// or dotted with a chart's σ-rational vector fields.
fn const_vec3<B: Backend>(v: &[Rat<B>; 3]) -> Vec3Rat<B> {
    Vec3Rat::new(
        [
            Poly::constant(v[0].clone()),
            Poly::constant(v[1].clone()),
            Poly::constant(v[2].clone()),
        ],
        Poly::constant(Rat::from_i128(1)),
    )
}

/// The bisector `b_J = s_J·(n_A − n_B)`, evaluated from the two crease-station normals.
///
/// This is the A-side retained normal `b_A = b_J`; the B-side is `b_B = −b_J` ([`field_b`]
/// applies the sign). Returns `None` if either chart's normal is singular at its crease station
/// — the searcher declines rather than fabricating a normal.
pub fn bisector<B: Backend>(joint: &Joint<B>) -> Option<[Rat<B>; 3]> {
    let n_a = joint
        .flank_a()
        .chart()
        .normal()
        .eval(&joint.crease().sigma_a)?;
    let n_b = joint
        .flank_b()
        .chart()
        .normal()
        .eval(&joint.crease().sigma_b)?;
    let diff = [
        n_a[0].sub(&n_b[0]),
        n_a[1].sub(&n_b[1]),
        n_a[2].sub(&n_b[2]),
    ];
    Some(match joint.orientation() {
        JointSign::Plus => diff,
        JointSign::Minus => [diff[0].neg(), diff[1].neg(), diff[2].neg()],
    })
}

/// The crease anchor `x₀` — a point of the trim plane Π — taken as the A-flank pedal at its
/// crease station (the crease point the flanks share). `None` if the pedal is singular there.
pub fn crease_anchor<B: Backend>(joint: &Joint<B>) -> Option<[Rat<B>; 3]> {
    joint
        .flank_a()
        .chart()
        .pedal()
        .eval(&joint.crease().sigma_a)
}

/// Build the retained-side field `G_i` of a flank chart against the trim plane through `x₀` with
/// retained normal `b`: the three coefficients `g0 = (pedal − x₀)·b`, `g_mu = ruling·b`,
/// `g_w = normal·b`.
fn g_field<B: Backend>(
    chart: &geom::chart::Chart<B>,
    x0: &[Rat<B>; 3],
    b: &[Rat<B>; 3],
) -> GField<B> {
    let bv = const_vec3(b);
    GField {
        g0: chart.pedal().sub(&const_vec3(x0)).dot(&bv),
        g_mu: chart.ruling().dot(&bv),
        g_w: chart.normal().dot(&bv),
    }
}

/// The A-side retained field `G_A` (normal `b_A = b_J`), against the trim plane through `x0`.
/// `None` if `b_J` is undefined (a singular crease normal).
///
/// # Example
///
/// Drive TRIM-LOCAL and CLIP-W from a 90° cylinder self-fold: `G_A = w·(1 − 2σ − σ²)/(1 + σ²)`
/// is strictly retained (`> 0`) on `σ ∈ [0, 1/4]`, `w ∈ [1, 2]`, so both checkers certify.
///
/// ```
/// use closure::{Crease, Flank, Joint, JointSign, MuRange};
/// use closure::trim::{clip_w_cert, crease_anchor, field_a, trim_local_cert};
/// use certify_core::MarginSq;
/// use certify_core::certify1d::{clip, trim_local, ClipVerdict};
/// use certify_core::verdict::Verdict;
/// use geom::chart::Chart;
/// use lattice::{Bignum, Interval, Poly, Rat, RatFunc};
///
/// let poly = |cs: &[i128]| Poly::<Bignum>::from_coeffs(cs.iter().map(|&c| Rat::from_i128(c)).collect());
/// let cyl = || Chart::new([poly(&[1]), poly(&[0, 1]), poly(&[0]), poly(&[0])], RatFunc::zero());
/// let mu = MuRange { lo: Rat::from_i128(-1), hi: Rat::from_i128(1) };
/// let joint = Joint::new(
///     Flank::new(cyl(), mu.clone()),
///     Flank::new(cyl(), mu.clone()),
///     Crease { sigma_a: Rat::from_i128(0), sigma_b: Rat::from_i128(1) },
///     JointSign::Plus,
/// );
/// let iv = |lo: (i128, i128), hi: (i128, i128)| Interval {
///     lo: Rat::new(lo.0, lo.1),
///     hi: Rat::new(hi.0, hi.1),
/// };
/// let x0 = crease_anchor(&joint).expect("regular pedal");
/// let g_a = field_a(&joint, &x0).expect("regular bisector");
///
/// let (w, sigma) = (iv((1, 1), (2, 1)), iv((0, 1), (1, 4)));
/// let trim = trim_local_cert(
///     &g_a, &mu, &w, &sigma,
///     &Rat::from_i128(0), &Rat::from_i128(1), MarginSq(Rat::new(1, 4)),
/// )
/// .expect("regular field");
/// assert!(matches!(trim_local(&trim), Verdict::Verified(_)));
///
/// let clip_w = clip_w_cert(&g_a, MarginSq(Rat::new(1, 16)), sigma);
/// assert_eq!(clip(&clip_w, &[], &[], None), ClipVerdict::Certified);
/// ```
pub fn field_a<B: Backend>(joint: &Joint<B>, x0: &[Rat<B>; 3]) -> Option<GField<B>> {
    let b = bisector(joint)?;
    Some(g_field(joint.flank_a().chart(), x0, &b))
}

/// The B-side retained field `G_B` (normal `b_B = −b_J`), against the trim plane through `x0`.
/// `None` if `b_J` is undefined (a singular crease normal).
pub fn field_b<B: Backend>(joint: &Joint<B>, x0: &[Rat<B>; 3]) -> Option<GField<B>> {
    let b = bisector(joint)?;
    let b_b = [b[0].neg(), b[1].neg(), b[2].neg()];
    Some(g_field(joint.flank_b().chart(), x0, &b_b))
}

/// A [`RegCert`] for the cleared squared gauge `g² ≥ m` on `span`: `num = (g.num)²`,
/// `den = (g.den)²`, with honest Sturm chains of `den` and the residual. This is the shape both
/// CLIP-W and CLIP-μ take (each a `reg_q` positivity on a squared `∂G` gauge).
fn reg_sq<B: Backend>(g: &RatFunc<B>, m: MarginSq<Rat<B>>, span: Interval<B>) -> RegCert<B> {
    let num = g.num().mul(g.num());
    let den = g.den().mul(g.den());
    let r = num.sub(&den.scale(&m.0));
    RegCert {
        den_chain: SturmChain::new(&den),
        res_chain: SturmChain::new(&r),
        num,
        den,
        m,
        span,
    }
}

/// A [`RegCert`] for the raw positivity `g > m` on `span` (`num = g.num`, `den = g.den`), with
/// honest Sturm chains — the shape TRIM-LOCAL's interior confinement takes on a single fiber.
fn reg_positive<B: Backend>(g: &RatFunc<B>, m: MarginSq<Rat<B>>, span: Interval<B>) -> RegCert<B> {
    let num = g.num().clone();
    let den = g.den().clone();
    let r = num.sub(&den.scale(&m.0));
    RegCert {
        den_chain: SturmChain::new(&den),
        res_chain: SturmChain::new(&r),
        num,
        den,
        m,
        span,
    }
}

/// The CLIP-W certificate: the cleared `(∂_wG)² = g_w² ≥ m` positivity on `span`, decided by
/// [`clip`](certify_core::certify1d::clip)'s first rung. A `Verified` CLIP-W means the trim
/// boundary crosses every fiber's `w`-line transversally — the whole ladder certifies.
pub fn clip_w_cert<B: Backend>(
    field: &GField<B>,
    m: MarginSq<Rat<B>>,
    span: Interval<B>,
) -> RegCert<B> {
    reg_sq(&field.g_w, m, span)
}

/// The CLIP-μ certificate: the cleared `(∂_μG)² = g_mu² ≥ m` positivity on `span`, the ladder's
/// second rung (supplied per failing sub-span). `≡ 0` for a cylinder (crease-parallel rulings),
/// so CLIP-μ is only informative on a fanning-ruling flank.
pub fn clip_mu_cert<B: Backend>(
    field: &GField<B>,
    m: MarginSq<Rat<B>>,
    span: Interval<B>,
) -> RegCert<B> {
    reg_sq(&field.g_mu, m, span)
}

/// Assemble a [`TrimLocalCert`]: `G_i` at the four corners of the two outer support fibers (the
/// σ-support ends `[sigma.lo, sigma.hi]`, each over the box `mu × w`), plus the interior
/// confinement `G_i > m` on the fiber `(confine_mu, confine_w)` across the σ-support.
///
/// The confinement fiber must be the box-minimizing `(μ, w)` corner for the confinement to bound
/// the whole box — a searcher aptness obligation (the checker soundly certifies *that* fiber's
/// positivity regardless). For a w-only field (`g_mu ≡ 0`) the minimizer is fixed by `sgn g_w`,
/// so a single fiber suffices. Returns `None` if the field is singular at a support end.
pub fn trim_local_cert<B: Backend>(
    field: &GField<B>,
    mu: &MuRange<B>,
    w: &Interval<B>,
    sigma: &Interval<B>,
    confine_mu: &Rat<B>,
    confine_w: &Rat<B>,
    m: MarginSq<Rat<B>>,
) -> Option<TrimLocalCert<B>> {
    let outer_fibers = vec![
        field.corners(&sigma.lo, mu, w)?,
        field.corners(&sigma.hi, mu, w)?,
    ];
    let confinement = reg_positive(&field.fiber(confine_mu, confine_w), m, sigma.clone());
    Some(TrimLocalCert {
        outer_fibers,
        confinement,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Crease, Flank};
    use certify_core::certify1d::{
        ClipVerdict, FiberCell, classify_fiber, clip, clip_dom, trim_local,
    };
    use certify_core::verdict::Verdict;
    use geom::chart::Chart;
    use lattice::Bignum;

    fn poly(cs: &[i128]) -> Poly<Bignum> {
        Poly::from_coeffs(cs.iter().map(|&c| Rat::from_i128(c)).collect())
    }
    fn cyl() -> Chart<Bignum> {
        Chart::new(
            [poly(&[1]), poly(&[0, 1]), poly(&[0]), poly(&[0])],
            RatFunc::zero(),
        )
    }
    fn mu() -> MuRange<Bignum> {
        MuRange {
            lo: Rat::from_i128(-1),
            hi: Rat::from_i128(1),
        }
    }
    fn span(lo: (i128, i128), hi: (i128, i128)) -> Interval<Bignum> {
        Interval {
            lo: Rat::new(lo.0, lo.1),
            hi: Rat::new(hi.0, hi.1),
        }
    }
    /// The 90° cylinder self-fold: crease at σ_a = 0 (normal ẑ), σ_b = 1 (normal −ŷ),
    /// s_J = +1 ⇒ b_J = (0, 1, 1), pedal ≡ 0.
    fn joint(sign: JointSign) -> Joint<Bignum> {
        Joint::new(
            Flank::new(cyl(), mu()),
            Flank::new(cyl(), mu()),
            Crease {
                sigma_a: Rat::from_i128(0),
                sigma_b: Rat::from_i128(1),
            },
            sign,
        )
    }

    #[test]
    fn the_bisector_is_the_normal_difference() {
        // b_J = (0,0,1) − (0,−1,0) = (0, 1, 1).
        let b = bisector(&joint(JointSign::Plus)).expect("regular normals");
        assert_eq!(b, [Rat::from_i128(0), Rat::from_i128(1), Rat::from_i128(1)]);
        // s_J = −1 flips it.
        let bm = bisector(&joint(JointSign::Minus)).expect("regular");
        assert_eq!(
            bm,
            [Rat::from_i128(0), Rat::from_i128(-1), Rat::from_i128(-1)]
        );
    }

    #[test]
    fn the_retained_field_is_w_only_on_a_cylinder() {
        // Crease-parallel rulings (ruling ∥ x̂) ⇒ g_mu = ruling·b_J ≡ 0: G_A is w-only.
        let j = joint(JointSign::Plus);
        let x0 = crease_anchor(&j).expect("regular pedal");
        assert_eq!(
            x0,
            [Rat::from_i128(0), Rat::from_i128(0), Rat::from_i128(0)]
        );
        let f = field_a(&j, &x0).expect("regular field");
        assert!(f.g_mu.is_zero());
        // G_A(0, μ, w) = w·1 (normal ẑ · b_J = 1); independent of μ.
        let one = Rat::from_i128(1);
        assert_eq!(
            f.eval(&Rat::from_i128(0), &Rat::from_i128(-5), &one),
            Some(one.clone())
        );
        assert_eq!(
            f.eval(&Rat::from_i128(0), &Rat::from_i128(7), &one),
            Some(one)
        );
    }

    #[test]
    fn trim_local_certifies_the_retained_side() {
        // G_A = w·(1 − 2σ − σ²)/(1+σ²) > 0 on σ ∈ [0, 1/4], w ∈ [1, 2].
        let j = joint(JointSign::Plus);
        let x0 = crease_anchor(&j).unwrap();
        let f = field_a(&j, &x0).unwrap();
        let w = span((1, 1), (2, 1));
        let sig = span((0, 1), (1, 4));
        let cert = trim_local_cert(
            &f,
            &mu(),
            &w,
            &sig,
            &Rat::from_i128(0), // μ irrelevant (w-only)
            &Rat::from_i128(1), // binding w-corner (min w, g_w > 0)
            MarginSq(Rat::new(1, 4)),
        )
        .expect("regular field");
        assert!(matches!(trim_local(&cert), Verdict::Verified(_)));
    }

    #[test]
    fn trim_local_refutes_a_wrong_side_fiber() {
        // Extend the support past the g_w root σ = −1 + √2 ≈ 0.414: at σ = 1/2, g_w < 0,
        // so the outer fiber is on the deleted side ⇒ SIDE's wrong-side test refutes.
        let j = joint(JointSign::Plus);
        let x0 = crease_anchor(&j).unwrap();
        let f = field_a(&j, &x0).unwrap();
        let w = span((1, 1), (2, 1));
        let sig = span((0, 1), (1, 2));
        let cert = trim_local_cert(
            &f,
            &mu(),
            &w,
            &sig,
            &Rat::from_i128(0),
            &Rat::from_i128(1),
            MarginSq(Rat::new(1, 4)),
        )
        .unwrap();
        assert!(matches!(trim_local(&cert), Verdict::Refuted(_)));
    }

    #[test]
    fn clip_w_certifies_a_transverse_trim() {
        // g_w = (1 − 2σ − σ²)/(1+σ²) ≥ 7/17 on [0, 1/4] ⇒ g_w² ≥ 1/16 ⇒ CLIP-W Verified.
        let j = joint(JointSign::Plus);
        let x0 = crease_anchor(&j).unwrap();
        let f = field_a(&j, &x0).unwrap();
        let sig = span((0, 1), (1, 4));
        let w_cert = clip_w_cert(&f, MarginSq(Rat::new(1, 16)), sig);
        assert_eq!(clip(&w_cert, &[], &[], None), ClipVerdict::Certified);
    }

    #[test]
    fn clip_dom_reports_a_connected_retained_support() {
        // Sweep representative fibers across [0, 1/4]: all Full (w·g_w > 0) ⇒ one component,
        // no partial clip — the w-only cylinder fold has no fiber the trim plane cuts.
        let j = joint(JointSign::Plus);
        let x0 = crease_anchor(&j).unwrap();
        let f = field_a(&j, &x0).unwrap();
        let w = span((1, 1), (2, 1));
        let stations = [Rat::from_i128(0), Rat::new(1, 8), Rat::new(1, 4)];
        let cells: Vec<[Rat<Bignum>; 4]> = stations
            .iter()
            .map(|s| f.corners(s, &mu(), &w).unwrap())
            .collect();
        for c in &cells {
            assert_eq!(classify_fiber(c), FiberCell::Full);
        }
        let census = clip_dom(&cells);
        assert_eq!(census.retained_components, 1);
        assert!(!census.has_clip);
    }

    #[test]
    fn the_b_side_field_uses_the_negated_bisector() {
        // G_B against b_B = −b_J = (0,−1,−1); at σ_b = 1 the retained side is still positive.
        let j = joint(JointSign::Plus);
        let x0 = crease_anchor(&j).unwrap();
        let fb = field_b(&j, &x0).unwrap();
        // G_B(1, μ, 1) = normal(1)·b_B = (0,−1,0)·(0,−1,−1) = 1 > 0.
        assert_eq!(
            fb.eval(&Rat::from_i128(1), &Rat::from_i128(0), &Rat::from_i128(1)),
            Some(Rat::from_i128(1))
        );
    }
}
