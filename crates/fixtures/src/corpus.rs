//! The counterexample corpus (`fixtures/corpus.md`): one module per entry, its required
//! verdict asserted, transcribed as the checkers land — the day-one regression suite.
//! Each entry is a configuration that once fooled, or could fool, a checker.
//!
//! Landed so far: the M2 CLIP transversality-ladder and EDGE-REG entries, and the M4
//! [`closure`](self#closure) generality entries (the certified `CLOSURE_VALID` pipe run
//! across ≥2 developable classes and ≥2 cone angles — the C0 generality guard). The
//! cone-flank TRIM-LOCAL entry (`cx-cone-flank-trim-mu`) needs the petal conical flank and
//! lands with milestone C's petal pass; the remaining entries land with their checkers.

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

/// M4 **generality** entries (the C0 generality guard, `docs/vv-guide.md §8`): the certified
/// closure pipe run on **more than one developable class and more than one cone angle**, so
/// nothing in `closure`/`certify_core` is silently locked to the cylinder or to the device
/// cone's 65/97 half-angle.
///
/// The regularity bundle (REG-V ∧ WEDGE ∧ EXT-WEDGE) is the conjunct that directly consumes
/// each flank's crease *geometry* — the two unit normals `n_A(σ_a)`, `n_B(σ_b)` — so it is
/// exactly where an angle- or class-lock would surface. These entries fold each device against
/// itself at two distinct crease stations and drive the bundle through the real
/// `closure::wedge::wedge_cert` → `certify_core::wedge::regularity` path. The full
/// MITER/LEDGE cap pipe is exercised end-to-end on the cylinder in `closure::valid::tests`; a
/// genuine plane and the petal conical flank are deferred (`docs/closure-scoping.md §8`).
#[cfg(test)]
mod closure {
    use crate::devices::{cone, cone_alt, cylinder};
    use certify_core::wedge::regularity;
    use certify_core::{MarginSq, Verdict};
    use closure::wedge::wedge_cert;
    use closure::{Crease, Flank, Joint, JointSign, MuRange};
    use geom::chart::Chart;
    use lattice::{Bignum, Rat};

    type Q = Rat<Bignum>;

    fn mu() -> MuRange<Bignum> {
        MuRange {
            lo: Q::from_i128(-1),
            hi: Q::new(-1, 2),
        }
    }
    fn dot(a: &[Q; 3], b: &[Q; 3]) -> Q {
        a[0].mul(&b[0]).add(&a[1].mul(&b[1])).add(&a[2].mul(&b[2]))
    }

    /// Fold `chart` against itself at crease stations `σ_a ≠ σ_b`, derive a **real positive**
    /// REG-V margin from the fold's actual dihedral `d = n_A·n_B` (`m = |V|²/2` with
    /// `|V|² = (1−d)/(1+d)`), and certify the regularity bundle through the production path.
    /// Returns the certified crease dot `d` (asserted to equal the geometry, and to be a
    /// genuine, non-flat dihedral). Panics if the bundle refuses — a generality regression.
    fn fold_certifies(make: impl Fn() -> Chart<Bignum>, sa: Q, sb: Q) -> Q {
        // The two crease normals, straight from the chart — unit vectors by the exact
        // quaternion construction (a non-trivial fact the checker re-verifies as NonUnitNormal).
        let chart = make();
        let n_a = chart.normal().eval(&sa).expect("normal at σ_a");
        let n_b = chart.normal().eval(&sb).expect("normal at σ_b");
        assert!(
            dot(&n_a, &n_a).sub(&Q::from_i128(1)).is_zero()
                && dot(&n_b, &n_b).sub(&Q::from_i128(1)).is_zero(),
            "the chart's crease normals are unit vectors"
        );
        let d = dot(&n_a, &n_b);
        // A genuine, non-flat, sub-π dihedral: |V|² = (1−d)/(1+d) is finite and strictly positive.
        assert!(
            d != Q::from_i128(1) && Q::from_i128(1).add(&d).sign() > 0,
            "sub-π, non-flat fold"
        );
        let v_sq = Q::from_i128(1).sub(&d).div(&Q::from_i128(1).add(&d));
        let m = v_sq.mul(&Q::new(1, 2)); // a real positive margin below the true |V|²

        let joint = Joint::new(
            Flank::new(make(), mu()),
            Flank::new(make(), mu()),
            Crease {
                sigma_a: sa,
                sigma_b: sb,
            },
            JointSign::Plus,
        );
        // s_bev = 1/8: EXT-WEDGE clears (s_bev(1+s_bev)|V|² < 1) with room for any sub-π fold.
        let cert = wedge_cert(&joint, Q::new(1, 8), MarginSq(m.clone())).expect("crease normals");
        match regularity(&cert) {
            Verdict::Verified(w) => {
                assert_eq!(w.n_dot, d, "the witness carries the true crease dot");
                assert_eq!(w.reg_v_margin.0, m);
                d
            }
            other => panic!(
                "the regularity bundle must certify a regular fold (verified={})",
                matches!(other, Verdict::Verified(_))
            ),
        }
    }

    /// The cylinder — the line-carrier developable the slice is built on (class 1). A 90° fold
    /// (σ = 0 vs σ = 1) gives `d = 0`, `|V|² = 1`.
    #[test]
    fn cylinder_fold_is_regular() {
        let d = fold_certifies(cylinder, Q::from_i128(0), Q::from_i128(1));
        assert!(d.is_zero(), "the 90° cylinder self-fold has d = 0");
    }

    /// The device cone (class 2, half-angle `n·ẑ ≡ 65/97`). A different developable class than
    /// the cylinder, certified by the *same* checker — the cone is not special-cased.
    #[test]
    fn device_cone_fold_is_regular() {
        let d = fold_certifies(cone, Q::from_i128(0), Q::from_i128(1));
        assert!(!d.is_zero(), "the cone self-fold is a non-right dihedral");
    }

    /// The second-angle cone (`n·ẑ ≡ 3/5`, distinct from 65/97). Certifies through the same
    /// path with a *different* crease dot than the device cone — proof the bundle is not
    /// locked to one half-angle.
    #[test]
    fn second_angle_cone_fold_is_regular() {
        let d_alt = fold_certifies(cone_alt, Q::from_i128(0), Q::from_i128(1));
        let d_dev = fold_certifies(cone, Q::from_i128(0), Q::from_i128(1));
        assert_ne!(
            d_alt, d_dev,
            "the two cone angles fold to distinct dihedrals"
        );
    }
}

/// M5 **SEW** corpus (`docs/vv-guide.md §8`): the sewing obligation `SEW = SEW-EDGES ∧ SEW-LINK`
/// driven end-to-end through the searcher (`sew`) into the pure checkers (`certify_core::sew`).
///
/// The clean-miter seam feeds MITER-REGION-IDENTITY → SEW-EDGES; SEW-LINK runs over every `V_∂`
/// vertex of a genuine two-operand arrangement via `sew::check_vertex_link`; and the two canonical
/// non-manifold sewings — an opposite-quadrant **occupancy pinch** and a count-passing
/// **`a→c→b→d` crossing link** — are each refuted by name (the crossing is lesson #4: a multiset
/// match would wave it through, `link_iso_ok` does not).
#[cfg(test)]
mod sew {
    use arrange2d::boolean::{BoolOp, OperandId, label_cells, vertex_link};
    use arrange2d::dcel::Dcel;
    use certify_core::Verdict;
    use certify_core::miter::{LedgerEdge, MiterLedger, Occupancy, OrderSign};
    use certify_core::sew::{
        FaceGermSpecies, SewCounts, SewEdgesFault, SewLinkFault, sew_edges, sew_link,
    };
    use geom::content::{CurveId, Edge, Line, Orient, Point2, SegPiece};
    use lattice::{Bignum, Rat};
    use sew::{check_vertex_link, records_from_miter_ledger};

    type Q = Rat<Bignum>;

    fn p(x: i128, y: i128) -> (Q, Q) {
        (Q::from_i128(x), Q::from_i128(y))
    }

    /// A CCW polygon operand through `verts`, tagged `src` (source 0 → A, 1 → B).
    fn polygon(verts: &[(i128, i128)], src: u32) -> Vec<Edge<Bignum>> {
        let n = verts.len();
        (0..n)
            .map(|i| {
                let (sx, sy) = verts[i];
                let (ex, ey) = verts[(i + 1) % n];
                let (a, b) = (Q::from_i128(-(ey - sy)), Q::from_i128(ex - sx));
                let c = a
                    .mul(&Q::from_i128(sx))
                    .add(&b.mul(&Q::from_i128(sy)))
                    .neg();
                Edge::Seg(Box::new(SegPiece {
                    line: Line { a, b, c },
                    start: Point2::from_rat(Q::from_i128(sx), Q::from_i128(sy)),
                    end: Point2::from_rat(Q::from_i128(ex), Q::from_i128(ey)),
                    orient: Orient::Ccw,
                    source: CurveId(src),
                }))
            })
            .collect()
    }

    fn ab(src: CurveId) -> OperandId {
        if src.0 == 0 {
            OperandId::A
        } else {
            OperandId::B
        }
    }

    /// Two overlapping squares crossing at (4,2) and (2,4): the Dcel + its Or-selection (cycle-indexed).
    fn two_squares_union() -> (Dcel<Bignum>, Vec<bool>) {
        let mut edges = polygon(&[(0, 0), (4, 0), (4, 4), (0, 4)], 0);
        edges.extend(polygon(&[(2, 2), (6, 2), (6, 6), (2, 6)], 1));
        let d = Dcel::build(&edges);
        let cl = label_cells(&d, &ab);
        let sel: Vec<bool> = cl.labels.iter().map(|&l| BoolOp::Or.select(l)).collect();
        (d, sel)
    }

    /// A clean-miter seam sews: MITER-REGION-IDENTITY projects one coincident flank-to-flank ledger
    /// edge, and SEW-EDGES accepts its opposite-side boundary-boundary occupancy.
    #[test]
    fn clean_miter_seam_sews() {
        let ledger = MiterLedger {
            edges: vec![LedgerEdge {
                start: p(0, 0),
                end: p(4, 0),
                eps_phi: OrderSign::Preserving,
                occupancy: Occupancy {
                    a_l: true,
                    a_r: false,
                    b_l: false,
                    b_r: true,
                    frame: false,
                },
            }],
        };
        let records = records_from_miter_ledger(&ledger);
        assert!(matches!(
            sew_edges(
                &records,
                SewCounts {
                    cap_to_flank: 0,
                    flank_to_flank: 1
                }
            ),
            Verdict::Verified(_)
        ));
    }

    /// SEW-LINK over `V_∂` of a real two-square union: every boundary vertex's embedded link
    /// certifies through the searcher (`vertex_link` → `sew_link`).
    #[test]
    fn union_boundary_links_sew() {
        let (d, sel) = two_squares_union();
        let mut checked = 0;
        for v in 0..d.verts.len() {
            let (_, _, sectors) = vertex_link(&d, &sel, v);
            let picked = sectors.iter().filter(|&&b| b).count();
            if picked == 0 || picked == sectors.len() {
                continue; // interior/exterior — not on V_∂
            }
            let species = vec![FaceGermSpecies::Flank; picked];
            assert!(
                matches!(
                    check_vertex_link(&d, &sel, v, &species),
                    Verdict::Verified(_)
                ),
                "boundary vertex {v} must sew",
            );
            checked += 1;
        }
        assert!(checked >= 2, "the union crossing has two boundary vertices");
    }

    /// An opposite-quadrant occupancy — both sides of one flank occupied, neither of the other —
    /// is a non-manifold transverse pinch SEW-EDGES refuses.
    #[test]
    fn opposite_quadrant_occupancy_is_a_pinch() {
        let ledger = MiterLedger {
            edges: vec![LedgerEdge {
                start: p(0, 0),
                end: p(4, 0),
                eps_phi: OrderSign::Preserving,
                occupancy: Occupancy {
                    a_l: true,
                    a_r: true,
                    b_l: false,
                    b_r: false,
                    frame: false,
                },
            }],
        };
        let records = records_from_miter_ledger(&ledger);
        assert!(matches!(
            sew_edges(
                &records,
                SewCounts {
                    cap_to_flank: 0,
                    flank_to_flank: 1
                }
            ),
            Verdict::Refuted(SewEdgesFault::Pinch { .. })
        ));
    }

    /// A count-passing `a→c→b→d` crossing link: the same four rays as the emitted order, so a
    /// multiset match passes — but not a cyclic rotation, so SEW-LINK refuses it.
    #[test]
    fn crossing_link_is_refused() {
        assert!(matches!(
            sew_link(
                &[0, 1, 2, 3],
                &[0, 2, 1, 3],
                &[true, true, false, false],
                &[FaceGermSpecies::Flank, FaceGermSpecies::Flank],
            ),
            Verdict::Refuted(SewLinkFault::LinkMismatch)
        ));
    }
}
