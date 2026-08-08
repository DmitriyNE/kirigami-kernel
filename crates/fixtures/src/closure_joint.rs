//! The one-joint closure fixture — a public, `CLOSURE_VALID`-passing **physical fold**
//! for the STEP export end-to-end (Milestone D slice 1).
//!
//! A genuine **90° cylinder self-fold**: two *distinct* true-cylinder flanks that share
//! one crease line and meet at a right angle, certifying through **both** the MITER and
//! LEDGE cap branches. Nothing here keys on the flank type — the fold is two cylinder
//! charts, differing from a cone only in their quaternion spline and their support field
//! `h`.
//!
//! # The physical geometry (M-D slice 1)
//!
//! Milestone C's fixture was a certification *artifact* (`h ≡ 0` ⇒ a cone with all rulings
//! through the origin; both flanks the *same* chart over *disjoint* supports, so the crease
//! edges never met). M-D slice 1 replaces it with a device-recognizable fold, discharging
//! two of the three documented warts:
//!
//! - **Flank A** is a true unit cylinder about the x-axis: `q = 1 + σi`, `h ≡ 1`, so the
//!   pedal `c = h·n` traces a unit circle and the rulings stay parallel to x̂ (**not** a cone).
//! - **Flank B** is that same cylinder rigidly translated by `t = (0, 1, 1)` — a translation
//!   ⊥ the rulings, which is again a chart with the *same* `q` and support
//!   `h_B(σ) = 1 + t·n_A(σ) = 2(1 − σ)/(1 + σ²)`.
//! - The crease stations are `σ_a = 0` (normal ẑ) and `σ_b = 1` (normal −ŷ): a 90° dihedral
//!   (`n_A·n_B = 0`), with **both** crease neutral edges (`w = 0`) lying on the *shared* ruling
//!   line `L = {(x, 0, 1)}`. The two flanks abut on `L` — no gap, a real shared crease.
//!
//! One cosmetic residue remains: `|r(σ)| = |n′(σ)|` differs at the two 90°-apart crease
//! stations (2 at σ=0, 1 at σ=1), so at a shared μ-range flank A's crease edge spans
//! `x ∈ [−2, 2]` while flank B's spans `x ∈ [−1, 1]` — both on `L`, a 2:1 overhang. Equalising
//! it needs the irrational station `σ = √2 − 1`, unavailable to a rational crease. The third
//! wart (the metric-distorted LEDGE cap lift) is discharged in `export::shell::lift`, which
//! now lifts the cap through the orthonormal crease frame `{r₀/√s, n₀}` — a unit cap square
//! lifts to a unit (not stretched) world square (`s = |r₀|² = chart.normal_deriv_sq(σ*)`).
//!
//! The builders compose — the fold certifies through the **MITER** branch (clean mitered
//! corner, no separate cap face)…
//!
//! ```
//! use certify_core::Verdict;
//! use closure::valid::{CapWitness, closure_valid};
//! use fixtures::closure_joint::{miter_cap, one_joint, treatment_miter};
//!
//! let joint = one_joint();
//! let cap = miter_cap(); // owned so the treatment can borrow its edge/claim/occ lists
//! let t = treatment_miter(&cap);
//! assert!(matches!(closure_valid(&joint, &t), Verdict::Verified(_)));
//! ```
//!
//! …and, on the same fold, through the **LEDGE** branch (a spanning cap face):
//!
//! ```
//! use certify_core::Verdict;
//! use closure::valid::closure_valid;
//! use fixtures::closure_joint::{ledge_d24, one_joint, treatment};
//!
//! let joint = one_joint();
//! let d24 = ledge_d24(); // owned so the treatment can borrow it
//! let t = treatment(&d24);
//! assert!(matches!(closure_valid(&joint, &t), Verdict::Verified(_)));
//! ```

use certify_core::MarginSq;
use certify_core::Verdict;
use certify_core::cap_in::{FlankId, ValidatedD24, cap_in_d24};
use certify_core::miter::{CutEnds, Occupancy, OrderSign};
use certify_core::sew::{EdgeIdentity, EdgeProvenance, EdgeRecord, FaceGermSpecies, SewCounts};
use closure::cap_in::segment_edge;
use closure::miter::segment_cut_ends;
use closure::valid::{ClosureTreatment, MiterInput, SewInput, VertexLink};
use closure::{Crease, Flank, Joint, JointSign, MuRange};
use geom::chart::Chart;
use lattice::{Bignum, Interval, Poly, Rat, RatFunc};

type Q = Rat<Bignum>;

fn q(v: i128) -> Q {
    Q::from_i128(v)
}
fn p(x: i128, y: i128) -> (Q, Q) {
    (q(x), q(y))
}
fn poly(cs: &[i128]) -> Poly<Bignum> {
    Poly::from_coeffs(cs.iter().map(|&c| Q::from_i128(c)).collect())
}
fn iv(lo: (i128, i128), hi: (i128, i128)) -> Interval<Bignum> {
    Interval {
        lo: Rat::new(lo.0, lo.1),
        hi: Rat::new(hi.0, hi.1),
    }
}

/// **Flank A** — a true unit cylinder about the x-axis (`q = 1 + σi`, `h ≡ 1`): the pedal
/// `c = h·n` traces a unit circle, so the rulings stay parallel to x̂ (a genuine cylinder,
/// not the `h ≡ 0` cone). The line-carrier developable the slice is built on.
fn cylinder_a() -> Chart<Bignum> {
    Chart::new(
        [poly(&[1]), poly(&[0, 1]), poly(&[0]), poly(&[0])],
        RatFunc::one(),
    )
}

/// **Flank B** — flank A rigidly translated by `t = (0, 1, 1)` (⊥ the rulings), which is again
/// a chart with the *same* quaternion `q = 1 + σi` and support `h_B(σ) = 1 + t·n_A(σ) =
/// 2(1 − σ)/(1 + σ²)`. At the crease station σ_b = 1 its normal is −ŷ and its crease neutral
/// edge lands on the *shared* line `L = {(x, 0, 1)}` — the fold's real, gap-free crease.
fn cylinder_b() -> Chart<Bignum> {
    Chart::new(
        [poly(&[1]), poly(&[0, 1]), poly(&[0]), poly(&[0])],
        RatFunc::new(poly(&[2, -2]), poly(&[1, 0, 1])),
    )
}

/// The shared retained ruling range `[μ⁻, μ⁺] = [−1, 1]`.
fn mu() -> MuRange<Bignum> {
    MuRange {
        lo: q(-1),
        hi: q(1),
    }
}

/// The physical **90° cylinder self-fold**: flank A = [`cylinder_a`] (crease σ_a = 0, normal ẑ),
/// flank B = [`cylinder_b`] (crease σ_b = 1, normal −ŷ), s_J = +1 ⇒ b_J = (0, 1, 1). Both crease
/// neutral edges lie on the shared line `L = {(x, 0, 1)}`, so the flanks abut with no gap. Flank
/// A's retained support is σ ∈ [0, 1/8], flank B's is σ ∈ [7/8, 1] — each abutting its crease.
/// The input joint for the one-joint closure.
pub fn one_joint() -> Joint<Bignum> {
    Joint::new(
        Flank::new(cylinder_a(), mu()),
        Flank::new(cylinder_b(), mu()),
        Crease {
            sigma_a: q(0),
            sigma_b: q(1),
        },
        JointSign::Plus,
    )
}

/// The forced-ledge cap: a `2×2` square spanning both flanks (crease / A / A / B
/// edges), licensed through CAP-IN-D24. Own the returned value so the
/// [`treatment`] that borrows it outlives the borrow.
pub fn ledge_d24() -> ValidatedD24<Bignum> {
    let sq = [
        segment_edge(&p(0, 0), &p(2, 0), FlankId::Crease),
        segment_edge(&p(2, 0), &p(2, 2), FlankId::A),
        segment_edge(&p(2, 2), &p(0, 2), FlankId::A),
        segment_edge(&p(0, 2), &p(0, 0), FlankId::B),
    ];
    match cap_in_d24(&sq) {
        Verdict::Verified(v) => v,
        other => panic!("the square cap must license: {other:?}"),
    }
}

/// The owned backing for a [`MiterInput`]: the two flanks' cap-outline cut edges, the crease
/// direction, the per-edge order-sign claims, the transverse occupancy rows, and the EDGE-REG
/// margin. Held together so the borrowed [`MiterInput`] a [`treatment_miter`] builds outlives it.
pub struct MiterCap {
    a: Vec<CutEnds<Bignum>>,
    b: Vec<CutEnds<Bignum>>,
    crease_dir: (Q, Q),
    claimed: Vec<OrderSign>,
    occ: Vec<Occupancy>,
    margin: Q,
}

/// The clean-miter cap outline: a diamond transverse to the crease direction x̂, every edge
/// traced *identically* by both flanks (PAIR-IDENTICAL, order-preserving) — the mitered corner
/// where the two folded flanks' trimmed cut faces coincide, so no separate cap face is emitted.
/// Own the returned value so the [`treatment_miter`] that borrows it outlives the borrow.
pub fn miter_cap() -> MiterCap {
    let verts = [p(2, 0), p(0, 2), p(-2, 0), p(0, -2)];
    let mut a = Vec::new();
    for k in 0..4 {
        a.push(segment_cut_ends(
            verts[k].clone(),
            verts[(k + 1) % 4].clone(),
            q(0),
            q(1),
        ));
    }
    let b = a.clone();
    MiterCap {
        a,
        b,
        crease_dir: p(1, 0),
        claimed: vec![OrderSign::Preserving; 4],
        occ: vec![
            Occupancy {
                a_l: true,
                a_r: false,
                b_l: false,
                b_r: true,
                frame: false,
            };
            4
        ],
        margin: q(1),
    }
}

/// A SEW-passing packet for the sewn fold: one flank-to-flank clean-miter seam edge
/// (opposite-side boundary-boundary occupancy, a coincident PAIR-IDENTICAL pair) and one
/// boundary vertex whose link is a trivially-consistent one-arc boundary.
fn sew_ok() -> SewInput<Bignum> {
    let seam = EdgeRecord {
        occupancy: Occupancy {
            a_l: true,
            a_r: false,
            b_l: false,
            b_r: true,
            frame: false,
        },
        provenance: EdgeProvenance::FlankToFlank,
        identity: EdgeIdentity::PairIdentical {
            a_start: p(0, 0),
            a_end: p(4, 0),
            b_start: p(0, 0),
            b_end: p(4, 0),
            eps: OrderSign::Preserving,
        },
    };
    let link = VertexLink {
        emitted: vec![0, 1, 2, 3],
        geometric: vec![0, 1, 2, 3],
        sectors: vec![true, true, false, false],
        species: vec![FaceGermSpecies::Flank, FaceGermSpecies::Flank],
    };
    SewInput {
        records: vec![seam],
        counts: SewCounts {
            cap_to_flank: 0,
            flank_to_flank: 1,
        },
        links: vec![link],
    }
}

/// The regularity + trim boxes shared by both cap treatments of the [`one_joint`] fold,
/// tuned to the physical charts: `w ∈ [1, 2]`, and each flank's σ-support shrunk to abut its
/// crease (`σ_a ∈ [0, 1/8]`, `σ_b ∈ [7/8, 1]`) — the range over which TRIM-LOCAL stays cleared
/// for the true-cylinder pedal (the wider C-milestone boxes go negative here).
fn boxes() -> (
    MuRange<Bignum>,
    Interval<Bignum>,
    Interval<Bignum>,
    Interval<Bignum>,
) {
    (
        mu(),
        iv((1, 1), (2, 1)),
        iv((0, 1), (1, 8)),
        iv((7, 8), (1, 1)),
    )
}

/// The **MITER** treatment of the [`one_joint`] fold (M-D slice 1's headline): the regularity /
/// trim boxes tuned to the physical charts, a SEW-passing packet, and the clean-miter [`MiterCap`]
/// offered as the CLOSURE-CAP disjunct (`cap_miter: Some`, `cap_ledge: None`). The returned
/// [`ClosureTreatment`] borrows `cap`, so keep `cap` alive alongside it.
pub fn treatment_miter(cap: &MiterCap) -> ClosureTreatment<'_, Bignum> {
    let (mu, w, sigma_a, sigma_b) = boxes();
    ClosureTreatment {
        s_bev: Rat::new(1, 4),
        reg_v_margin: MarginSq(Rat::new(1, 2)),
        mu,
        w,
        sigma_a,
        sigma_b,
        confine_a: (q(0), q(1)),
        confine_b: (q(0), q(1)),
        trim_margin: MarginSq(Rat::new(1, 8)),
        clip_margin: MarginSq(Rat::new(1, 32)),
        cap_miter: Some(MiterInput {
            a: &cap.a,
            b: &cap.b,
            crease_dir: &cap.crease_dir,
            claimed: &cap.claimed,
            occ: &cap.occ,
            margin: &cap.margin,
        }),
        cap_ledge: None,
        sew: sew_ok(),
    }
}

/// The **LEDGE** treatment of the [`one_joint`] fold: the same physical-chart regularity / trim
/// boxes and SEW packet, but with the `d24` square offered as the LEDGE cap disjunct (no miter).
/// The returned [`ClosureTreatment`] borrows `d24`, so keep `d24` alive alongside it.
pub fn treatment(d24: &ValidatedD24<Bignum>) -> ClosureTreatment<'_, Bignum> {
    let (mu, w, sigma_a, sigma_b) = boxes();
    ClosureTreatment {
        s_bev: Rat::new(1, 4),
        reg_v_margin: MarginSq(Rat::new(1, 2)),
        mu,
        w,
        sigma_a,
        sigma_b,
        confine_a: (q(0), q(1)),
        confine_b: (q(0), q(1)),
        trim_margin: MarginSq(Rat::new(1, 8)),
        clip_margin: MarginSq(Rat::new(1, 32)),
        cap_miter: None,
        cap_ledge: Some(d24),
        sew: sew_ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use closure::valid::{CapWitness, closure_valid};

    #[test]
    fn the_one_joint_fold_is_closure_valid_via_the_miter() {
        let joint = one_joint();
        let cap = miter_cap();
        let t = treatment_miter(&cap);
        match closure_valid(&joint, &t) {
            Verdict::Verified(v) => assert!(matches!(v.cap, CapWitness::Miter(_))),
            other => panic!(
                "the fold must certify via the MITER branch: {}",
                matches!(other, Verdict::Verified(_))
            ),
        }
    }

    #[test]
    fn the_one_joint_fold_is_closure_valid_via_the_ledge() {
        let joint = one_joint();
        let d24 = ledge_d24();
        let t = treatment(&d24);
        match closure_valid(&joint, &t) {
            Verdict::Verified(v) => assert!(matches!(v.cap, CapWitness::Ledge(_))),
            other => panic!(
                "the fold must certify via the LEDGE branch: {}",
                matches!(other, Verdict::Verified(_))
            ),
        }
    }

    /// The physical geometry the certificate rides on: a real shared crease (both flanks'
    /// crease neutral edges coincide on the line `L = {(x, 0, 1)}`), a 90° dihedral, and
    /// parallel rulings off a *nonzero* pedal (a true cylinder, not the `h ≡ 0` cone).
    #[test]
    fn the_fold_is_a_physical_shared_crease_right_angle() {
        let a = cylinder_a();
        let b = cylinder_b();
        let zero = q(0);
        let one = q(1);

        // Nonzero pedal at each crease station ⇒ a cylinder, not a cone (whose pedal ≡ 0).
        let ca = a.pedal().eval(&zero).unwrap();
        let cb = b.pedal().eval(&one).unwrap();
        assert_eq!(ca, [q(0), q(0), q(1)], "flank A crease pedal on L");
        assert_eq!(
            cb,
            [q(0), q(0), q(1)],
            "flank B crease pedal on L (shared anchor)"
        );

        // 90° dihedral: n_A(σ_a) · n_B(σ_b) = 0 (ẑ · (−ŷ)).
        let na = a.normal().eval(&zero).unwrap();
        let nb = b.normal().eval(&one).unwrap();
        let dot = na[0]
            .mul(&nb[0])
            .add(&na[1].mul(&nb[1]))
            .add(&na[2].mul(&nb[2]));
        assert_eq!(dot, q(0), "the dihedral is a right angle");

        // Both crease neutral edges (w = 0) lie on the shared line L = {(x, 0, 1)}.
        for (name, ch, s) in [("A", &a, zero.clone()), ("B", &b, one.clone())] {
            for mu in [q(-1), q(1)] {
                let e = ch.surface(&mu, &zero).eval(&s).unwrap();
                assert_eq!(e[1], q(0), "{name} crease edge y = 0");
                assert_eq!(e[2], q(1), "{name} crease edge z = 1");
            }
        }

        // Parallel rulings: flank A's ruling stays along x̂ (y = z = 0) with nonzero x across
        // its retained support — no apex convergence.
        for s in [q(0), Rat::new(1, 8)] {
            let r = a.ruling().eval(&s).unwrap();
            assert_ne!(r[0], q(0), "ruling has a nonzero x component");
            assert_eq!(r[1], q(0), "ruling y = 0 (∥ x̂)");
            assert_eq!(r[2], q(0), "ruling z = 0 (∥ x̂)");
        }
    }
}
