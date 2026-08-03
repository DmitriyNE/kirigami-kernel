//! The counterexample corpus (`fixtures/corpus.md`): one module per entry, its required
//! verdict asserted, transcribed as the checkers land — the day-one regression suite.
//! Each entry is a configuration that once fooled, or could fool, a checker.
//!
//! Landed so far: the M2 CLIP transversality-ladder entries. The remaining entries land
//! with their checkers.

/// CLIP-ladder counterexamples (spec §8.5) — the transversality traps.
#[cfg(test)]
mod clip {
    use certify_core::certify1d::{
        ClipACert, ClipSigmaCert, ClipVerdict, RegCert, ZeroClip, clip, clip_sigma,
    };
    use certify_core::{MarginSq, Verdict};
    use lattice::{Bignum, Interval, Poly, Rat, SturmChain};

    type Q = Rat<Bignum>;

    fn poly(cs: &[i128]) -> Poly<Bignum> {
        Poly::from_coeffs(cs.iter().map(|&c| Q::from_i128(c)).collect())
    }

    /// A REG-Q certificate that `reg_q` **refutes**: `num/den = x²+1 > 2` fails at `x = 0`
    /// on `[−2, 2]`. Used to drive the CLIP ladder past its CLIP-W rung to a common zero.
    fn failing_clip_w() -> RegCert<Bignum> {
        let (num, den) = (poly(&[1, 0, 1]), poly(&[1]));
        let m = Q::from_i128(2);
        let r = num.sub(&den.scale(&m));
        RegCert {
            den_chain: SturmChain::new(&den),
            res_chain: SturmChain::new(&r),
            num,
            den,
            m: MarginSq(m),
            span: Interval {
                lo: Q::from_i128(-2),
                hi: Q::from_i128(2),
            },
        }
    }

    /// `cx-sigma-mu-crossing` (soundness-critical). `G = σ·μ` at the singular crossing
    /// `σ* = 0`, where `a = b = d = 0` and `∂_σG = μ`. The **signed** CLIP-σ disjunction
    /// sees the affine corner range `[−1, +1]` straddling zero and returns `Unresolved`;
    /// a four-corner `|∂_σG|²` test would falsely `Verify` with margin 1 (the interior
    /// minimizer `μ = 0` is invisible to a squared corner test).
    #[test]
    fn cx_sigma_mu_crossing_is_unresolved() {
        let cert = ClipSigmaCert::<Bignum> {
            corners: [
                Q::from_i128(-1),
                Q::from_i128(1),
                Q::from_i128(-1),
                Q::from_i128(1),
            ],
            m_sigma: Q::new(1, 2),
        };
        assert!(matches!(clip_sigma(&cert), Verdict::Unresolved(_)));
    }

    /// `cx-clip-common-zero`. A fiber where `b(σ*) = d(σ*) = 0` but `a(σ*) ≠ 0`. The
    /// ladder must terminate via the CLIP-a branch (`|a|` separated ⇒ the fiber misses Π)
    /// — never loop forever on subdivision.
    #[test]
    fn cx_clip_common_zero_certifies_via_clip_a() {
        let w = failing_clip_w(); // CLIP-W fails, forcing the ladder to the common zero
        let zeros = [ZeroClip::ByA(ClipACert {
            a: Q::from_i128(3),
            m_a: MarginSq(Q::from_i128(4)), // 9 ≥ 4 ⇒ |a| separated ⇒ CLIP-a resolves it
        })];
        assert_eq!(clip(&w, &[], &zeros), ClipVerdict::Certified);
    }
}
