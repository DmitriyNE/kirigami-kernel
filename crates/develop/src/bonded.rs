//! §14 BONDED lap-seam certificates (Stage-2 S3) — **CLEAR** first.
//!
//! The BONDED lap bonds two blank ends across the full-2π seam. The genuinely new
//! obligation is **CLEAR**: the two lapping sheets keep clear (minimum 3D distance
//! ≥ a keep-out) over the seam-ramp neighbourhood — the one place the offset-pair
//! reduction fails (the §7 correspondence is shifted by the support derivative, so a
//! same-ruling normal gap is *not* a sound min-distance), and the paper (`docs/paper.md`
//! §8/§11) hands off to interval subdivision.
//!
//! [`clear`] is that certificate: **adaptive** interval subdivision of the *true* 3D
//! distance between two rational curves — sound regardless of the tangential shift,
//! fail-closed (`Unresolved` when the node budget cannot separate them). It lives here,
//! beside `cut::cut_fit` and `anchor::anchor_dev` (the rigorous rational-interval checker
//! tier), not the pure `certify-core` combinatorial TCB.
//!
//! **Scope.** The certified curves are the sheets' `(µ,w)`-rails (`c + µr + wn` at a fixed
//! `µ,w`). Full-band *surface* clearance is the same adaptive scheme with the ruling `µ`
//! also subdivided (a world-AABB over a whole ruling is dominated by its length, so `µ`
//! must be split, not hulled) — the mechanical scaling step, deferred.

use crate::interval::{RatIv, eval_ratfunc_on};
use certify_core::Verdict;
use certify_core::certify1d::{RegCert, RegFault, reg_q};
use certify_core::margin::MarginSq;
use geom::chart::Chart;
use lattice::{Backend, Bignum, Interval, Poly, Rat, RatFunc, SturmChain};

/// A lapping curve over the seam σ'-box: a rail `c + µr + wn` at a fixed `(µ, w)`, held as
/// its three reduced component `RatFunc`s of σ'. This is what [`clear`] encloses and
/// subdivides.
pub struct LapRail<B: Backend = Bignum> {
    comp: [RatFunc<B>; 3],
}

impl<B: Backend> LapRail<B> {
    /// The chart's rail `C(·, µ, w) = c + µr + wn` (a σ'-parametric space curve), reduced.
    pub fn from_chart(chart: &Chart<B>, mu: &Rat<B>, w: &Rat<B>) -> Self {
        let s = chart.surface(mu, w);
        LapRail {
            comp: [s.comp(0).reduce(), s.comp(1).reduce(), s.comp(2).reduce()],
        }
    }

    /// The rail's 3D bounding box over the σ'-interval `iv`, or `None` if a component
    /// evaluation hits a possible pole (denominator enclosure straddles zero).
    fn bbox(&self, iv: &RatIv<B>) -> Option<[RatIv<B>; 3]> {
        Some([
            eval_ratfunc_on(&self.comp[0], iv)?,
            eval_ratfunc_on(&self.comp[1], iv)?,
            eval_ratfunc_on(&self.comp[2], iv)?,
        ])
    }
}

/// The evidence for a [`clear`] verdict: a certified lower bound on the **squared** minimum
/// 3D distance between the two lapping rails over the box (`≥ keep_out²`).
pub struct ClearWitness<B: Backend = Bignum> {
    /// The certified min-distance-squared lower bound.
    pub min_dist_sq: Rat<B>,
    /// Nodes expanded — the certificate's subdivision cost.
    pub nodes: usize,
}

/// Why a [`clear`] check refuted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClearFault {
    /// The σ'-box is empty or reversed (`hi ≤ lo`).
    DegenerateBox,
    /// A rail could not be enclosed on a sub-interval (a possible pole).
    PoleInEval,
}

/// Certify the two lapping rails keep clear over the seam σ'-box: minimum 3D distance
/// ≥ `keep_out`, by **adaptive** interval subdivision.
///
/// Both rails' σ'-domains are subdivided together: on a pair `(I_A, I_B)` the rails' 3D
/// boxes are enclosed and a lower bound on their distance is taken; a pair whose box-distance
/// ≥ `keep_out` is **pruned** (all its point-pairs are ≥ keep_out apart — sound, since each
/// arc lies inside its box), an inconclusive pair is split at the wider interval (covering
/// exactly the point-pairs the parent did), and the search stops when every pair is pruned
/// (`Verified`, carrying the min pruned distance²) or the node budget is spent (`Unresolved`,
/// refine `max_nodes` / loosen `keep_out`). Distances are compared **squared**, so no surd
/// enters; sound for any tangential shift.
pub fn clear<B: Backend>(
    a: &LapRail<B>,
    b: &LapRail<B>,
    sbox: &Interval<B>,
    keep_out: &Rat<B>,
    max_nodes: usize,
) -> Verdict<ClearWitness<B>, ClearFault, Rat<B>> {
    if sbox.hi <= sbox.lo {
        return Verdict::Refuted(ClearFault::DegenerateBox);
    }
    let k2 = keep_out.mul(keep_out);
    let whole = RatIv::new(sbox.lo.clone(), sbox.hi.clone());
    let mut stack = vec![(whole.clone(), whole)];
    let mut min_pruned: Option<Rat<B>> = None;
    let mut nodes = 0usize;
    while let Some((ia, ib)) = stack.pop() {
        let (ba, bb) = match (a.bbox(&ia), b.bbox(&ib)) {
            (Some(ba), Some(bb)) => (ba, bb),
            _ => return Verdict::Refuted(ClearFault::PoleInEval),
        };
        let d2 = box_dist_sq_lo(&ba, &bb);
        if d2 >= k2 {
            min_pruned = Some(match min_pruned {
                Some(m) if m <= d2 => m,
                _ => d2,
            });
            continue;
        }
        if nodes >= max_nodes {
            return Verdict::Unresolved(d2);
        }
        nodes += 1;
        // Split the wider interval; keep the other — the two children cover exactly the
        // point-pairs the parent `(I_A, I_B)` did, so pruning stays a partition of all pairs.
        if ia.width() >= ib.width() {
            let m = ia.mid();
            stack.push((RatIv::new(ia.lo().clone(), m.clone()), ib.clone()));
            stack.push((RatIv::new(m, ia.hi().clone()), ib));
        } else {
            let m = ib.mid();
            stack.push((ia.clone(), RatIv::new(ib.lo().clone(), m.clone())));
            stack.push((ia, RatIv::new(m, ib.hi().clone())));
        }
    }
    Verdict::Verified(ClearWitness {
        min_dist_sq: min_pruned.unwrap_or(k2),
        nodes,
    })
}

/// A lower bound on the squared distance between two 3D interval boxes (0 per axis where the
/// intervals overlap) — the exact box-box min-distance², hence a lower bound on the distance
/// between any point of one box and any point of the other.
fn box_dist_sq_lo<B: Backend>(a: &[RatIv<B>; 3], b: &[RatIv<B>; 3]) -> Rat<B> {
    let mut acc = Rat::from_i128(0);
    for i in 0..3 {
        let g = axis_gap(&a[i], &b[i]);
        acc = acc.add(&g.mul(&g));
    }
    acc
}

/// The nonnegative gap between two intervals on one axis (0 if they overlap).
fn axis_gap<B: Backend>(a: &RatIv<B>, b: &RatIv<B>) -> Rat<B> {
    if a.hi() < b.lo() {
        b.lo().sub(a.hi())
    } else if b.hi() < a.lo() {
        a.lo().sub(b.hi())
    } else {
        Rat::from_i128(0)
    }
}

/// The constant `RatFunc` `r`.
fn konst<B: Backend>(r: &Rat<B>) -> RatFunc<B> {
    RatFunc::from_poly(Poly::from_coeffs(vec![r.clone()]))
}

/// Evidence that SEP holds: the certified constant face-separation, equal to the bond gap `g`.
pub struct SepWitness<B: Backend = Bignum> {
    /// The certified constant separation (`= g`).
    pub gap: Rat<B>,
}

/// Why a [`sep`] check refuted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SepFault<B: Backend = Bignum> {
    /// The face separation is not constant on the bonded range — so this is not the bonded
    /// plateau (SEP is a plateau property; the ramp reverts to [`clear`]).
    NotConstant,
    /// The separation is a constant, but not the declared bond gap `g` (the actual value).
    GapMismatch(Rat<B>),
}

/// **SEP** — certify the corresponding-normal face separation ≡ the bond gap `g` on the
/// bonded range (§7 face identity `h_A + w_{A,face} + g = h_B + w_{B,face}`).
///
/// Two shared-frame sheets separate purely in the normal (`c·n ≡ h`, since `n·n = 1` and
/// `n′·n = 0`), so the corresponding-normal separation is `(h_B + w_B) − (h_A + w_A)`; SEP
/// holds iff that is the constant `g`. An **exact rational identity** — "compares two ring
/// scalars", spec §7 — no subdivision, no float, no `Unresolved`.
pub fn sep<B: Backend>(
    h_a: &RatFunc<B>,
    w_a: &Rat<B>,
    h_b: &RatFunc<B>,
    w_b: &Rat<B>,
    g: &Rat<B>,
) -> Verdict<SepWitness<B>, SepFault<B>, ()> {
    let gap = h_b.add(&konst(w_b)).sub(&h_a.add(&konst(w_a))).reduce();
    let is_const = gap.num().degree().is_none_or(|d| d == 0) && gap.den().degree() == Some(0);
    if !is_const {
        return Verdict::Refuted(SepFault::NotConstant);
    }
    // A constant `RatFunc` (nonzero constant denominator) evaluates anywhere; σ' = 0 is fine.
    let val = gap.eval(&Rat::from_i128(0)).unwrap_or_else(|| g.clone());
    if &val == g {
        Verdict::Verified(SepWitness { gap: val })
    } else {
        Verdict::Refuted(SepFault::GapMismatch(val))
    }
}

/// The constant value of `f` if it is constant on σ', else `None`.
fn as_constant<B: Backend>(f: &RatFunc<B>) -> Option<Rat<B>> {
    let f = f.reduce();
    let is_const = f.num().degree().is_none_or(|d| d == 0) && f.den().degree() == Some(0);
    if is_const {
        f.eval(&Rat::from_i128(0))
    } else {
        None
    }
}

/// Evidence that SHEAR holds: the Tier-1 identification `J = rigid ∘ ruling-shear`.
pub struct ShearWitness<B: Backend = Bignum> {
    /// The constant geodesic curvature `κ_g ≡ k` (signed witness).
    pub kappa_g: Rat<B>,
    /// The constant midplane offset `Δ ≡ Δ₀`.
    pub delta0: Rat<B>,
    /// The ruling-shear `δ = −Δ₀/k`.
    pub shear: Rat<B>,
}

/// Why a [`shear`] check refuted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShearFault {
    /// `κ_g` is not constant on the range — the Tier-1 collapse does not apply (Tier 0 stands).
    KappaNotConstant,
    /// `Δ` is not constant — a layer-dropped / pad-topology face (varying or piecewise `Δ`).
    DeltaNotConstant,
    /// `κ_g` is not separated from zero (`k² < m`, or `k = 0`) — the cylinder degeneracy, where
    /// the map is affine with cross-ruling scale (Tier 2), not rigid ∘ shear.
    KappaTooSmall,
}

/// **SHEAR** — certify the Tier-1 identification `J = rigid ∘ ruling-shear` collapses (§7).
///
/// Given the (searcher-supplied) geodesic curvature `κ_g(σ')` and midplane offset `Δ(σ')`,
/// SHEAR holds iff `κ_g ≡ k` (constant, signed), `k² ≥ m > 0` (separated from zero), and
/// `Δ ≡ Δ₀` (constant); then the shear is `δ = −Δ₀/k`. All hypotheses are **exactly
/// decidable rational identities** (constancy, one sign, one ring comparison) — no float, no
/// subdivision. For the device cone `κ_g = −tan β = −65/72`, `Δ₀ = 1/4` ⇒ `δ = Δ cot β = 18/65
/// ≈ 0.28 mm`, the number the ghost footprint needs.
pub fn shear<B: Backend>(
    kappa_g: &RatFunc<B>,
    delta: &RatFunc<B>,
    m: &Rat<B>,
) -> Verdict<ShearWitness<B>, ShearFault, ()> {
    let k = match as_constant(kappa_g) {
        Some(k) => k,
        None => return Verdict::Refuted(ShearFault::KappaNotConstant),
    };
    let d0 = match as_constant(delta) {
        Some(d) => d,
        None => return Verdict::Refuted(ShearFault::DeltaNotConstant),
    };
    if k.sign() == 0 || k.mul(&k) < *m {
        return Verdict::Refuted(ShearFault::KappaTooSmall);
    }
    let shear = d0.neg().div(&k);
    Verdict::Verified(ShearWitness {
        kappa_g: k,
        delta0: d0,
        shear,
    })
}

/// **SLAB** (SLAB-S0, spec §11) — certify the offset slab stays regular on the ramp: the
/// principal-radius datum `R₁ + w = det J / |n′|² > 0` over the σ'-span, at the `(µ, w)` corner.
///
/// `det J = c′·n′ + µ(r′·n′) + w|n′|²` is affine in `(µ, w)`, so its box minimum is a corner
/// (the caller passes the inf corner). Since `|n′|² > 0`, `R₁ + w > 0 ⟺ det J > 0`, so the check
/// is `det J ≥ m > 0` (the margin is in `det J` units), discharged by the reused **Sturm**
/// positivity checker `certify_core::certify1d::reg_q` (the searcher builds the σ'-numerator /
/// denominator and their Sturm chains; the checker re-verifies them).
/// Rational, no transcendental — the ramp's regularity is single-span-Sturm-certifiable (§8).
pub fn slab<B: Backend>(
    chart: &Chart<B>,
    mu: &Rat<B>,
    w: &Rat<B>,
    span: &Interval<B>,
    m: &Rat<B>,
) -> Verdict<MarginSq<Rat<B>>, RegFault<B>, ()> {
    let dj = chart.det_j();
    // det J at the fixed (µ, w) corner, as a σ'-rational function.
    let val = dj
        .constant
        .add(&dj.mu.scale(mu))
        .add(&dj.w.scale(w))
        .reduce();
    let num = val.num().clone();
    let den = val.den().clone();
    let r = num.sub(&den.scale(m)); // R = num − m·den
    let cert = RegCert {
        den_chain: SturmChain::new(&den),
        res_chain: SturmChain::new(&r),
        num,
        den,
        m: MarginSq(m.clone()),
        span: span.clone(),
    };
    reg_q(&cert)
}

/// The combined evidence for a certified BONDED lap seam — all four §14 invariants hold.
pub struct BondedSeam<B: Backend = Bignum> {
    /// SEP: the plateau separation ≡ the bond gap.
    pub sep: SepWitness<B>,
    /// SLAB-S0: the offset slab's certified positivity margin.
    pub slab: MarginSq<Rat<B>>,
    /// SHEAR: the Tier-1 identification (`κ_g`, `Δ₀`, `δ`).
    pub shear: ShearWitness<B>,
    /// CLEAR: the ramp min-distance² lower bound.
    pub clear: ClearWitness<B>,
}

/// Why [`valid_bonded_seam`] did not certify — the first failing invariant.
pub enum BondedSeamFault<B: Backend = Bignum> {
    /// SEP refuted (the plateau separation ≠ the bond gap, or it is not a plateau).
    Sep(SepFault<B>),
    /// SLAB-S0 refuted (the offset slab is not regular).
    Slab(RegFault<B>),
    /// SHEAR refuted (the Tier-1 identification does not collapse).
    Shear(ShearFault),
    /// CLEAR refuted (a degenerate ramp box or a pole in evaluation).
    Clear(ClearFault),
}

/// **VALID_bonded-seam** — the §14 BONDED conjunction: `SEP ∧ SLAB ∧ SHEAR ∧ CLEAR`.
///
/// Threads the four sub-verdicts as a strong-Kleene AND: the first `Refuted` wins (wrapped as
/// a [`BondedSeamFault`]); else a CLEAR `Unresolved` propagates (fail-closed — the seam-ramp
/// clearance was not established within budget, carrying its min-distance² handle); else all
/// four hold and the combined [`BondedSeam`] evidence is returned. SEP/SLAB/SHEAR are total
/// (never `Unresolved`); a defensive `Unresolved` from them fails closed with a zero handle.
pub fn valid_bonded_seam<B: Backend>(
    sep: Verdict<SepWitness<B>, SepFault<B>, ()>,
    slab: Verdict<MarginSq<Rat<B>>, RegFault<B>, ()>,
    shear: Verdict<ShearWitness<B>, ShearFault, ()>,
    clear: Verdict<ClearWitness<B>, ClearFault, Rat<B>>,
) -> Verdict<BondedSeam<B>, BondedSeamFault<B>, Rat<B>> {
    let zero = Rat::from_i128(0);
    let sep = match sep {
        Verdict::Verified(w) => w,
        Verdict::Refuted(f) => return Verdict::Refuted(BondedSeamFault::Sep(f)),
        Verdict::Unresolved(()) => return Verdict::Unresolved(zero),
    };
    let slab = match slab {
        Verdict::Verified(w) => w,
        Verdict::Refuted(f) => return Verdict::Refuted(BondedSeamFault::Slab(f)),
        Verdict::Unresolved(()) => return Verdict::Unresolved(zero),
    };
    let shear = match shear {
        Verdict::Verified(w) => w,
        Verdict::Refuted(f) => return Verdict::Refuted(BondedSeamFault::Shear(f)),
        Verdict::Unresolved(()) => return Verdict::Unresolved(zero),
    };
    let clear = match clear {
        Verdict::Verified(w) => w,
        Verdict::Refuted(f) => return Verdict::Refuted(BondedSeamFault::Clear(f)),
        Verdict::Unresolved(d2) => return Verdict::Unresolved(d2),
    };
    Verdict::Verified(BondedSeam {
        sep,
        slab,
        shear,
        clear,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fixtures::devices::{cone_seam, cone_seam_ramp};

    type Q = Rat<Bignum>;

    // The µ = −1 outer rail at the mid-surface w = 0.
    fn rail(chart: &Chart<Bignum>) -> LapRail<Bignum> {
        LapRail::from_chart(chart, &Q::from_i128(-1), &Q::from_i128(0))
    }

    fn ramp_box() -> Interval<Bignum> {
        // σ' ∈ [−1/4, 1/4] straddling the seam: the ramp support h = 1/4 − σ'/2 stays in
        // [1/8, 3/8] here (clear of the σ' = 1/2 rejoin, where the sheets would touch). Wide
        // enough that the coarse root boxes overlap, so the adaptive refinement engages.
        Interval {
            lo: Q::new(-1, 4),
            hi: Q::new(1, 4),
        }
    }

    #[test]
    fn the_lap_rails_clear_over_the_seam_ramp() {
        // Base rail (h = 0) vs the ramp flap rail (h = 1/4 − σ'/2): the true min distance is
        // ~0.16, above keep_out = 1/8. The certificate SUBDIVIDES (the coarse root boxes
        // overlap on this wide box) and converges — sound despite the tangential shift.
        let (a, b) = (rail(&cone_seam()), rail(&cone_seam_ramp()));
        match clear(&a, &b, &ramp_box(), &Q::new(1, 8), 1000) {
            Verdict::Verified(w) => {
                assert!(w.min_dist_sq >= Q::new(1, 64)); // ≥ (1/8)²
                assert!(
                    w.nodes > 0,
                    "the adaptive subdivision should engage on the wide box"
                );
                assert!(w.nodes < 200, "and converge fast (got {} nodes)", w.nodes);
                println!("CLEAR verified: nodes={}", w.nodes);
            }
            _ => panic!("expected the lap rails to certify clear"),
        }
    }

    #[test]
    fn too_large_a_keep_out_is_unresolved() {
        // The rails are at most ~1/2 apart; requiring 1/2 unit of clearance cannot be
        // certified — fail-closed to Unresolved (budget spent), never a wrong Verified.
        let (a, b) = (rail(&cone_seam()), rail(&cone_seam_ramp()));
        assert!(matches!(
            clear(&a, &b, &ramp_box(), &Q::new(1, 2), 200),
            Verdict::Unresolved(_)
        ));
    }

    #[test]
    fn a_reversed_box_is_refuted() {
        let (a, b) = (rail(&cone_seam()), rail(&cone_seam_ramp()));
        let bad = Interval {
            lo: Q::new(1, 4),
            hi: Q::new(-1, 4),
        };
        assert!(matches!(
            clear(&a, &b, &bad, &Q::new(1, 16), 10),
            Verdict::Refuted(ClearFault::DegenerateBox)
        ));
    }

    #[test]
    fn sep_holds_on_the_plateau_and_refutes_off_it() {
        let zero = RatFunc::<Bignum>::zero(); // base sheet, h_A = 0
        let plateau = konst(&Q::new(1, 4)); // bonded plateau, h_B ≡ 1/4
        let w = Q::from_i128(0);
        // The plateau separation is 1/4 = g → SEP holds, exactly.
        match sep(&zero, &w, &plateau, &w, &Q::new(1, 4)) {
            Verdict::Verified(s) => assert_eq!(s.gap, Q::new(1, 4)),
            _ => panic!("SEP should hold on the plateau"),
        }
        // A wrong declared gap → GapMismatch (with the true value).
        assert!(matches!(
            sep(&zero, &w, &plateau, &w, &Q::new(1, 8)),
            Verdict::Refuted(SepFault::GapMismatch(v)) if v == Q::new(1, 4)
        ));
        // The ramp (varying support) is not a plateau → NotConstant.
        let ramp =
            RatFunc::<Bignum>::from_poly(Poly::from_coeffs(vec![Q::new(1, 4), Q::new(-1, 2)]));
        assert!(matches!(
            sep(&zero, &w, &ramp, &w, &Q::new(1, 4)),
            Verdict::Refuted(SepFault::NotConstant)
        ));
    }

    #[test]
    fn shear_collapses_tier1_for_the_cone() {
        // Device cone: κ_g = −tan β = −65/72, Δ₀ = 1/4 ⇒ δ = Δ cot β = 18/65 ≈ 0.28 mm.
        let kappa = konst(&Q::new(-65, 72));
        let delta = konst(&Q::new(1, 4));
        match shear(&kappa, &delta, &Q::new(1, 100)) {
            Verdict::Verified(w) => {
                assert_eq!(w.kappa_g, Q::new(-65, 72));
                assert_eq!(w.shear, Q::new(18, 65)); // −(1/4)/(−65/72) = 72/260 = 18/65
            }
            _ => panic!("SHEAR should collapse to Tier 1 for the cone"),
        }
        // A σ'-varying κ_g → KappaNotConstant.
        let varying =
            RatFunc::<Bignum>::from_poly(Poly::from_coeffs(vec![Q::new(-65, 72), Q::new(1, 10)]));
        assert!(matches!(
            shear(&varying, &delta, &Q::new(1, 100)),
            Verdict::Refuted(ShearFault::KappaNotConstant)
        ));
        // κ_g too near zero (the cylinder degeneracy) → KappaTooSmall.
        assert!(matches!(
            shear(&konst(&Q::new(1, 100)), &delta, &Q::new(1, 100)),
            Verdict::Refuted(ShearFault::KappaTooSmall)
        ));
    }

    #[test]
    fn slab_certifies_the_ramp_stays_regular() {
        // The ramp's offset slab stays regular: det J > 0 (⟺ R₁ + w > 0) over the seam box at
        // the µ = −1 corner (w = 0), via the reused Sturm positivity checker.
        let ramp = cone_seam_ramp();
        let span = ramp_box();
        match slab(
            &ramp,
            &Q::from_i128(-1),
            &Q::from_i128(0),
            &span,
            &Q::new(1, 1000),
        ) {
            Verdict::Verified(_) => {}
            _ => panic!("the ramp offset slab should be regular"),
        }
        // An over-large margin the datum cannot meet → a genuine margin failure.
        assert!(matches!(
            slab(
                &ramp,
                &Q::from_i128(-1),
                &Q::from_i128(0),
                &span,
                &Q::from_i128(1000)
            ),
            Verdict::Refuted(_)
        ));
    }

    // The four invariants, threaded, for the device seam: SEP/SHEAR on the plateau
    // (h_B ≡ Δ = 1/4, κ_g = −65/72), SLAB on the ramp chart, CLEAR on the ramp rails.
    #[allow(clippy::type_complexity)]
    fn device_seam(
        gap: &Q,
    ) -> (
        Verdict<SepWitness<Bignum>, SepFault<Bignum>, ()>,
        Verdict<MarginSq<Q>, RegFault<Bignum>, ()>,
        Verdict<ShearWitness<Bignum>, ShearFault, ()>,
        Verdict<ClearWitness<Bignum>, ClearFault, Q>,
    ) {
        let w = Q::from_i128(0);
        (
            sep(
                &RatFunc::<Bignum>::zero(),
                &w,
                &konst(&Q::new(1, 4)),
                &w,
                gap,
            ),
            slab(
                &cone_seam_ramp(),
                &Q::from_i128(-1),
                &w,
                &ramp_box(),
                &Q::new(1, 1000),
            ),
            shear(
                &konst(&Q::new(-65, 72)),
                &konst(&Q::new(1, 4)),
                &Q::new(1, 100),
            ),
            clear(
                &rail(&cone_seam()),
                &rail(&cone_seam_ramp()),
                &ramp_box(),
                &Q::new(1, 8),
                1000,
            ),
        )
    }

    #[test]
    fn valid_bonded_seam_conjoins_all_four() {
        let (s, l, h, c) = device_seam(&Q::new(1, 4)); // correct bond gap
        match valid_bonded_seam(s, l, h, c) {
            Verdict::Verified(b) => {
                assert_eq!(b.sep.gap, Q::new(1, 4));
                assert_eq!(b.shear.shear, Q::new(18, 65)); // δ ≈ 0.28 mm
                assert!(b.slab.0 > Q::from_i128(0));
            }
            _ => panic!("the device bonded seam should certify"),
        }
        // A wrong bond gap fails SEP; the conjunction short-circuits to that fault.
        let (s, l, h, c) = device_seam(&Q::new(1, 8)); // wrong gap
        assert!(matches!(
            valid_bonded_seam(s, l, h, c),
            Verdict::Refuted(BondedSeamFault::Sep(_))
        ));
    }
}
