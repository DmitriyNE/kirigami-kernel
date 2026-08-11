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
use geom::chart::Chart;
use lattice::{Backend, Bignum, Interval, Rat, RatFunc};

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
}
