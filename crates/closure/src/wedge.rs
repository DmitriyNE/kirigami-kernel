//! Regularity-bundle **searcher**: read a joint's two crease normals and hand them, with
//! the authored bevel slope and a proposed margin, to [`certify_core::wedge::regularity`].
//!
//! `closure` is the untrusted searcher: it *extracts* the two unit normals `n_A`, `n_B` at
//! the crease stations `σ_a`, `σ_b` from the flank charts and *proposes* the REG-V margin
//! `m`; the pure-tier checker re-derives `d = n_A·n_B` and decides WEDGE ∧ REG-V ∧
//! EXT-WEDGE. On the straight-crease scope `V` is constant, so one crease-station evaluation
//! per flank is the whole certificate (module `certify_core::wedge` docs).
//!
//! The bevel slope `s_bev` and the margin `m` are **authored closure-treatment data**
//! threaded through the searcher call — not stored on [`crate::Joint`] (the joint is the
//! geometric input; the bevel is a treatment parameter). They join the joint's
//! `{s_J, b_J, φ_J}` closure bundle when the full treatment record is assembled (C6).
//!
//! Nothing here keys on the flank *type*: [`wedge_cert`] evaluates an arbitrary
//! [`geom::chart::Chart`]'s unit normal, and the fault (over-π, below-margin, ext-wedge)
//! falls out of the checker's ring comparisons, never a Rust branch.

use certify_core::MarginSq;
use certify_core::wedge::WedgeCert;
use lattice::{Backend, Rat};

use crate::Joint;

/// Build the regularity certificate from a joint: evaluate each flank chart's unit normal at
/// its crease station, and attach the authored bevel slope `s_bev` and proposed REG-V margin
/// `m`.
///
/// Returns `None` if either chart's normal is singular at its crease station (no rational
/// value there) — the searcher declines rather than fabricating a normal. The returned
/// certificate is decided by [`certify_core::wedge::regularity`].
///
/// # Example
///
/// ```
/// use closure::{Crease, Flank, Joint, JointSign, MuRange};
/// use closure::wedge::wedge_cert;
/// use certify_core::MarginSq;
/// use certify_core::verdict::Verdict;
/// use certify_core::wedge::regularity;
/// use geom::chart::Chart;
/// use lattice::{Bignum, Poly, Rat, RatFunc};
///
/// let poly = |cs: &[i128]| Poly::<Bignum>::from_coeffs(cs.iter().map(|&c| Rat::from_i128(c)).collect());
/// let cyl = || Chart::new([poly(&[1]), poly(&[0, 1]), poly(&[0]), poly(&[0])], RatFunc::zero());
/// let mu = MuRange { lo: Rat::from_i128(-1), hi: Rat::new(-1, 2) };
/// // A cylinder folded against itself: crease stations σ_a = 0 (normal ẑ) and σ_b = 1
/// // (normal −ŷ) give d = 0 — a 90° fold, |V|² = 1.
/// let joint = Joint::new(
///     Flank::new(cyl(), mu.clone()),
///     Flank::new(cyl(), mu),
///     Crease { sigma_a: Rat::from_i128(0), sigma_b: Rat::from_i128(1) },
///     JointSign::Plus,
/// );
/// let cert = wedge_cert(&joint, Rat::new(1, 4), MarginSq(Rat::new(1, 2)))
///     .expect("normals are regular at the crease");
/// assert!(matches!(regularity(&cert), Verdict::Verified(_)));
/// ```
pub fn wedge_cert<B: Backend>(
    joint: &Joint<B>,
    s_bev: Rat<B>,
    reg_v_margin: MarginSq<Rat<B>>,
) -> Option<WedgeCert<B>> {
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
    Some(WedgeCert {
        n_a,
        n_b,
        s_bev,
        reg_v_margin,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Crease, Flank, JointSign, MuRange};
    use certify_core::verdict::Verdict;
    use certify_core::wedge::{WedgeFault, regularity};
    use geom::chart::Chart;
    use lattice::{Bignum, Poly, RatFunc};

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
            hi: Rat::new(-1, 2),
        }
    }
    fn joint(sigma_a: i128, sigma_b: i128) -> Joint<Bignum> {
        Joint::new(
            Flank::new(cyl(), mu()),
            Flank::new(cyl(), mu()),
            Crease {
                sigma_a: Rat::from_i128(sigma_a),
                sigma_b: Rat::from_i128(sigma_b),
            },
            JointSign::Plus,
        )
    }

    #[test]
    fn the_searcher_extracts_unit_crease_normals() {
        // A cylinder folded against itself at σ_a = 0 (ẑ) and σ_b = 1 (−ŷ).
        let cert = wedge_cert(&joint(0, 1), Rat::new(1, 4), MarginSq(Rat::new(1, 2)))
            .expect("regular normals");
        let unit = |n: &[Rat<Bignum>; 3]| {
            n[0].mul(&n[0]).add(&n[1].mul(&n[1])).add(&n[2].mul(&n[2])) == Rat::from_i128(1)
        };
        assert!(unit(&cert.n_a) && unit(&cert.n_b));
    }

    #[test]
    fn a_ninety_degree_cylinder_fold_certifies() {
        // d = ẑ·(−ŷ) = 0 ⇒ |V|² = 1: WEDGE, REG-V (≥ 1/2), EXT-WEDGE ((1/4)(5/4) < 1) all clear.
        let cert = wedge_cert(&joint(0, 1), Rat::new(1, 4), MarginSq(Rat::new(1, 2)))
            .expect("regular normals");
        assert!(matches!(regularity(&cert), Verdict::Verified(_)));
    }

    #[test]
    fn a_zero_dihedral_joint_is_refused() {
        // Both flanks meet at the same station ⇒ n_A = n_B ⇒ d = 1 ⇒ |V|² = 0: the searcher
        // faithfully hands over a flat joint and the checker deletes it (never certifies).
        let cert =
            wedge_cert(&joint(0, 0), Rat::new(1, 4), MarginSq(Rat::new(1, 2))).expect("regular");
        assert!(matches!(
            regularity(&cert),
            Verdict::Refuted(WedgeFault::BelowMarginV { .. })
        ));
    }
}
