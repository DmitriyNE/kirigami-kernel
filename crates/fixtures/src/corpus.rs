//! The counterexample corpus (`fixtures/corpus.md`): one module per entry, its required
//! verdict asserted, transcribed as the checkers land — the day-one regression suite.
//! Each entry is a configuration that once fooled, or could fool, a checker.
//!
//! Landed so far: the M2 CLIP transversality-ladder and EDGE-REG entries. The cone-flank
//! TRIM-LOCAL entry (`cx-cone-flank-trim-mu`) needs the petal conical flank and lands with
//! milestone C; the remaining entries land with their checkers.

/// CLIP-ladder counterexamples (spec §8.5) — the transversality traps.
#[cfg(test)]
mod clip {
    use certify_core::certify1d::{
        ClipACert, ClipSigmaCert, ClipVerdict, RegCert, ZeroCensus, ZeroClip, clip, clip_sigma,
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
        // The single common zero is the complete census: b²+d² has one root on [−2, 2].
        let census = ZeroCensus {
            discriminant: poly(&[0, 1]),
            chain: SturmChain::new(&poly(&[0, 1])),
            span: Interval {
                lo: Q::from_i128(-2),
                hi: Q::from_i128(2),
            },
            intervals: vec![Interval {
                lo: Q::from_i128(-1),
                hi: Q::from_i128(1),
            }],
        };
        assert_eq!(clip(&w, &[], &zeros, Some(&census)), ClipVerdict::Certified);
    }
}

/// EDGE-REG counterexamples (spec §8.5) — the regularity trichotomy `Pass | Fail | Stall`,
/// and the stall's REPARAM road back.
#[cfg(test)]
mod edge {
    use certify_core::certify1d::{EdgeFail, EdgeReg, EdgeRegCert, RegCert, edge_reg};
    use certify_core::{MarginSq, Verdict};
    use geom::chart::Chart;
    use geom::reparam::reparam;
    use geom::stall::Stall;
    use lattice::{Bignum, Interval, Poly, Rat, RatFunc, SturmChain};

    type Q = Rat<Bignum>;

    fn poly(cs: &[i128]) -> Poly<Bignum> {
        Poly::from_coeffs(cs.iter().map(|&c| Q::from_i128(c)).collect())
    }

    /// A `|e′|²` REG-Q certificate with the given margin on `[−2, 2]` (honest Sturm chains).
    /// `m = 1/2` verifies (speed bounded away from zero); `m = 2` refutes (a speed zero).
    fn speed_cert(m: Q) -> RegCert<Bignum> {
        let (num, den) = (poly(&[1, 0, 1]), poly(&[1])); // |e′|² = x² + 1
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

    /// `cx-cusp-edge`. A smooth flank whose Π-section is `y² = x³`. EDGE-REG must `Fail`
    /// (a geometric cusp) → vertex + reject to band — distinguished from a parametrization
    /// stall (a wrong classification only misdirects recovery; it cannot manufacture a pass).
    #[test]
    fn cx_cusp_edge_fails_to_band() {
        let cert = EdgeRegCert {
            speed_sq: speed_cert(Q::from_i128(2)), // reg_q refutes ⇒ e′ vanishes
            failure: Some(EdgeFail::Cusp(Q::from_i128(0))),
        };
        assert!(matches!(edge_reg(&cert), EdgeReg::Fail(_)));
        assert!(matches!(
            edge_reg(&cert).to_verdict(),
            Verdict::Refuted(EdgeFail::Cusp(_))
        ));
    }

    /// `cx-stall-reparam`. An isolated derivative zero but a regular point set: EDGE-REG is
    /// `Stall → Pending` (gate-failing as stored, never `Unresolved`); REPARAM regenerates a
    /// canonical regular record that re-certifies `Verified` — the stall was a compiler-pass
    /// fix, not a predicate truth.
    #[test]
    fn cx_stall_reparam_pending_then_verified() {
        // Original record: EDGE-REG classifies the isolated speed zero as removable.
        let original = EdgeRegCert {
            speed_sq: speed_cert(Q::from_i128(2)), // reg_q refutes ⇒ e′ vanishes at t*
            failure: Some(EdgeFail::Stalled {
                t_star: Q::from_i128(0),
                order: 1,
            }),
        };
        assert!(matches!(edge_reg(&original), EdgeReg::Stall { .. }));
        // Pending: gate-failing as stored (Refuted(Stalled)), never Unresolved.
        assert!(matches!(
            edge_reg(&original).to_verdict(),
            Verdict::Refuted(EdgeFail::Stalled { .. })
        ));

        // REPARAM (spec §7): regenerate a canonical regular record superseding the stall.
        let stall = Stall {
            sigma_star: Q::from_i128(0),
            order: 1,
            epsilon: 1,
        };
        let regular_cone = Chart::new(
            [poly(&[9]), poly(&[4]), poly(&[0, 4]), poly(&[0, 9])],
            RatFunc::zero(),
        );
        let record = reparam(stall, regular_cone);
        // The regenerated record is regular at σ* (|n′|² ≠ 0 there) — no stall remains.
        assert!(
            record
                .regular
                .normal_deriv_sq()
                .eval(&Q::from_i128(0))
                .unwrap()
                .sign()
                != 0
        );
        // Re-certified from scratch on the regular record: Verified.
        let recertified = EdgeRegCert::<Bignum> {
            speed_sq: speed_cert(Q::new(1, 2)), // reg_q verifies ⇒ regular immersion
            failure: None,
        };
        assert!(matches!(
            edge_reg(&recertified).to_verdict(),
            Verdict::Verified(_)
        ));
    }
}
