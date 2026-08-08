//! The one-joint closure fixture — a public, `CLOSURE_VALID`-passing joint for the
//! STEP export end-to-end (Milestone C exit).
//!
//! The canonical **90° cylinder self-fold** with a **forced-ledge** square cap: the
//! same joint + treatment the `closure` crate certifies in its own tests, exposed as
//! public builders so downstream crates (the `export` STEP writer, the gate
//! end-to-end) can reconstruct and render a certified shell. Nothing here keys on the
//! flank type — the fold is two cylinder charts, differing from a cone only in their
//! quaternion spline.
//!
//! # This is a certification fixture, not a physically-authored joint
//!
//! It is tuned to make the *algebra* return `Verified`, not to look like a device. Its
//! assembled shell round-trips through STEP but does not render as a recognizable fold:
//! with `h ≡ 0` the "cylinder" is geometrically a **cone** (pedal `c ≡ 0`, rulings
//! through the origin); **both** flanks are that one chart over the *disjoint* retained
//! supports `σ ∈ [0, ¼]` and `[½, 1]` (a visible gap, and the two crease edges never
//! meet); and [`ledge_d24`]'s square is the CAP-IN-D24 *licensing* polygon, not the
//! real projected flank cut. A physical fixture is M-D work — see
//! `export::shell` and `docs/vv-guide.md §8`.
//!
//! The three builders compose:
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
use certify_core::miter::{Occupancy, OrderSign};
use certify_core::sew::{EdgeIdentity, EdgeProvenance, EdgeRecord, FaceGermSpecies, SewCounts};
use closure::cap_in::segment_edge;
use closure::valid::{ClosureTreatment, SewInput, VertexLink};
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

/// The canonical cylinder about the x-axis (`q = 1 + σi`) — the line-carrier
/// developable flank the M4 slice is built on.
fn cylinder() -> Chart<Bignum> {
    Chart::new(
        [poly(&[1]), poly(&[0, 1]), poly(&[0]), poly(&[0])],
        RatFunc::zero(),
    )
}

/// The shared retained ruling range `[μ⁻, μ⁺] = [−1, 1]`.
fn mu() -> MuRange<Bignum> {
    MuRange {
        lo: q(-1),
        hi: q(1),
    }
}

/// The canonical **90° cylinder self-fold**: σ_a = 0 (normal ẑ), σ_b = 1 (normal −ŷ),
/// s_J = +1 ⇒ b_J = (0, 1, 1). Flank A's retained support is σ ∈ [0, 1/4]; flank B's is
/// σ ∈ [1/2, 1]. The input joint for the one-joint closure.
pub fn one_joint() -> Joint<Bignum> {
    Joint::new(
        Flank::new(cylinder(), mu()),
        Flank::new(cylinder(), mu()),
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

/// The forced-ledge treatment of the [`one_joint`] fold: regularity + trim parameters
/// tuned to the 90° fold (the C2/C3 known-passing boxes) and a SEW-passing packet,
/// with the `d24` square offered as the LEDGE cap disjunct (no miter). The returned
/// [`ClosureTreatment`] borrows `d24`, so keep `d24` alive alongside it.
pub fn treatment(d24: &ValidatedD24<Bignum>) -> ClosureTreatment<'_, Bignum> {
    ClosureTreatment {
        s_bev: Rat::new(1, 4),
        reg_v_margin: MarginSq(Rat::new(1, 2)),
        mu: mu(),
        w: iv((1, 1), (2, 1)),
        sigma_a: iv((0, 1), (1, 4)),
        sigma_b: iv((1, 2), (1, 1)),
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
    fn the_one_joint_fold_is_closure_valid_via_the_ledge() {
        let joint = one_joint();
        let d24 = ledge_d24();
        let t = treatment(&d24);
        match closure_valid(&joint, &t) {
            Verdict::Verified(v) => assert!(matches!(v.cap, CapWitness::Ledge(_))),
            other => panic!(
                "the fold must certify: {}",
                matches!(other, Verdict::Verified(_))
            ),
        }
    }
}
