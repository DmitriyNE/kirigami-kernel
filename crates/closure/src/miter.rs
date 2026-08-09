//! MITER branch **searcher**: pair a joint's two flanks' projected cut edges into a clean
//! miter and license the pairing through the pure-tier `certify_core::miter` checkers.
//!
//! When the two flanks fold so their trimmed cut faces **coincide** in the cap plane Π, the
//! closure caps as a *clean miter*: no planar ledge to arrange, only the shared cut edges,
//! each traced by *both* flanks (PAIR-IDENTICAL). This searcher builds each flank's cut edge
//! as a degree-1 (line-carrier) [`CutEnds`] — projecting a real ruling with
//! [`ruling_cut_ends`], or taking two already-projected cap-outline points with
//! [`segment_cut_ends`] — assembles the pairings, and drives
//! [`certify_core::miter::miter_fit`] → [`certify_core::miter::miter_edge_ledger`] →
//! [`certify_core::miter::miter_out`]. The searcher decides nothing: every pass is the
//! checker's, and a MITER-FIT refusal is the searcher's signal to fall back to the LEDGE
//! branch ([`crate::ledge`]).
//!
//! `ε_φ` — the order sign of the cross-flank pairing — is minted inside `miter_fit` from one
//! exact endpoint comparison, never a derivative; the searcher only *claims* a sign and the
//! checker refuses a wrong claim. Nothing here keys on the flank *type*: the projection
//! consumes an arbitrary [`geom::chart::Chart`], and the degree-1 corollary applies to any
//! line-carrier flank (a cylinder's rulings, a planar cut), never a Rust branch.

use certify_core::Verdict;
use certify_core::miter::{
    CutEnds, MiterFault, MiterFit, MiterOutCert, MiterOutFault, MiterOutWitness, Occupancy,
    OrderSign, degree1_edge_reg_cert, miter_edge_ledger, miter_fit, miter_out,
};
use geom::chart::Chart;
use lattice::{Backend, Bignum, Interval, Poly, Rat, RatFunc, Vec3Rat};

use crate::cap_in::{PiFrame, project, project_point};

/// A constant [`Vec3Rat`] from a rational 3-vector.
fn const_vec3<B: Backend>(a: &[Rat<B>; 3]) -> Vec3Rat<B> {
    Vec3Rat::from_polys([
        Poly::constant(a[0].clone()),
        Poly::constant(a[1].clone()),
        Poly::constant(a[2].clone()),
    ])
}

/// The **transverse cut-ruling family** of a flank in the cap plane Π `(b_J, frame)` — the
/// searcher side of the curved (rotating-ruling) MITER-FIT. At each σ the flank's tangent
/// plane meets Π in the cut ruling `F(σ, t) = P(σ) + t·D(σ)`, with `P`, `D` rational in σ.
/// Returns the crease-line coordinate `ℓ(σ)` (the frame-`u` coordinate where the ruling
/// meets `L = {frame-v = 0}`) and the ruling direction `(dx(σ), dy(σ))` in the frame — all
/// rational functions of σ. `None` if `g_w = n·b_J ≡ 0` (CLIP-W fails — the cut is not
/// transverse to the `w`-fibres) or `dy ≡ 0` (the ruling is parallel to `L` — the parallel
/// regime, out of the transverse slice).
///
/// Derivation: `G_i = (C − x₀)·b_J = g0 + μ·g_mu + w·g_w` (affine in `(μ, w)`); solving
/// `G_i = 0` for `w` gives the cut ruling `P = c − (g0/g_w)·n`, `D = r − (g_mu/g_w)·n`, both
/// in Π by construction. This is the rotating-ruling analogue of [`ruling_cut_ends`]'s
/// single degree-1 ruling.
pub fn transverse_cut_family<B: Backend>(
    chart: &Chart<B>,
    b_j: &[Rat<B>; 3],
    frame: &PiFrame<B>,
) -> Option<(RatFunc<B>, RatFunc<B>, RatFunc<B>)> {
    let c = chart.pedal();
    let r = chart.ruling();
    let n = chart.normal();
    let bv = const_vec3(b_j);
    let uv = const_vec3(&frame.u);
    let vv = const_vec3(&frame.v);
    let ov = const_vec3(&frame.origin);
    let g0 = c.sub(&ov).dot(&bv);
    let g_mu = r.dot(&bv);
    let g_w = n.dot(&bv);
    if g_w.is_zero() {
        return None;
    }
    // P = c − (g0/g_w)·n ; D = r − (g_mu/g_w)·n — the cut ruling, both in Π.
    let p = c.sub(&n.scale(&g0.div(&g_w)));
    let dir = r.sub(&n.scale(&g_mu.div(&g_w)));
    let (px, py) = project(&p, frame);
    let dx = dir.dot(&uv);
    let dy = dir.dot(&vv);
    if dy.is_zero() {
        return None;
    }
    // ℓ = px − py·(dx/dy): the u-coordinate where the ruling crosses L = {v = 0}.
    let ell = px.sub(&py.mul(&dx).div(&dy));
    Some((ell, dx, dy))
}

/// A degree-1 cut edge between two already-projected cap-plane points, over the parameter
/// support `[sigma_lo, sigma_hi]` — the building block of a polygonal clean-miter cap outline.
///
/// The straight cut edges of the degree-1 slice (a cylinder's ruling images, a planar cut)
/// are exactly these; a curved cut (a cone's σ-arc) is a [`certify_core::cap_in::Carrier::Circle`]
/// and is out of the line-edge slice.
pub fn segment_cut_ends<B: Backend>(
    start: (Rat<B>, Rat<B>),
    end: (Rat<B>, Rat<B>),
    sigma_lo: Rat<B>,
    sigma_hi: Rat<B>,
) -> CutEnds<B> {
    CutEnds {
        start,
        end,
        sigma_lo,
        sigma_hi,
    }
}

/// Build a degree-1 cut edge from a flank's **ruling** at the crease station `σ*`: the line
/// `P(μ) = (c(σ*) + w·n(σ*)) + μ·r(σ*)` swept over `μ ∈ [mu_lo, mu_hi]`, projected into the
/// cap plane through `frame`. The support parameter is `μ`.
///
/// A ruling is straight for *every* developable, so this is the faithful degree-1 cut edge a
/// cylinder (or any ruled flank) contributes. Returns `None` if the chart's `σ`-fields are
/// singular at `σ*` — the searcher declines rather than fabricating an edge.
pub fn ruling_cut_ends<B: Backend>(
    chart: &Chart<B>,
    sigma_star: &Rat<B>,
    w: &Rat<B>,
    mu_lo: Rat<B>,
    mu_hi: Rat<B>,
    frame: &PiFrame<B>,
) -> Option<CutEnds<B>> {
    let c0 = chart.pedal().eval(sigma_star)?;
    let r0 = chart.ruling().eval(sigma_star)?;
    let n0 = chart.normal().eval(sigma_star)?;
    // P(μ) = (c0 + w·n0) + μ·r0 — degree-1 in μ; evaluate at the two endpoints and project.
    let point = |mu: &Rat<B>| {
        let coord = |i: usize| c0[i].add(&n0[i].mul(w)).add(&r0[i].mul(mu));
        project_point(&[coord(0), coord(1), coord(2)], frame)
    };
    Some(CutEnds {
        start: point(&mu_lo),
        end: point(&mu_hi),
        sigma_lo: mu_lo,
        sigma_hi: mu_hi,
    })
}

/// Why the clean-miter searcher could not license the cap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MiterSearchError<B: Backend = Bignum> {
    /// MITER-FIT refused the pairing at edge index `at`: the two flanks' cut edges do not
    /// coincide as an order-consistent point set, so the miter is **not clean**. The caller
    /// falls back to the LEDGE branch ([`crate::ledge`]).
    NotClean {
        /// The offending edge index.
        at: usize,
        /// The checker's refutation.
        fault: MiterFault<B>,
    },
    /// MITER-FIT returned `Unresolved` — it is total (spec §8.5), so this never occurs; folded
    /// here defensively so the searcher stays panic-free.
    Inconclusive {
        /// The edge index.
        at: usize,
    },
    /// The A/B edge, claim, and occupancy lists have mismatched lengths — malformed input.
    Ragged,
}

/// The EDGE-REG certificate for a degree-1 cut edge `e(σ) = start + (σ − σ_lo)/(σ_hi − σ_lo)·
/// (end − start)`: the constant squared speed `|e′|² = |end − start|² / (σ_hi − σ_lo)²`,
/// cleared against the separation margin `m`.
fn edge_reg_for<B: Backend>(e: &CutEnds<B>, m: &Rat<B>) -> certify_core::certify1d::EdgeRegCert<B> {
    let dx = e.end.0.sub(&e.start.0);
    let dy = e.end.1.sub(&e.start.1);
    let len_sq = dx.mul(&dx).add(&dy.mul(&dy));
    let dsig = e.sigma_hi.sub(&e.sigma_lo);
    let speed_sq = len_sq.div(&dsig.mul(&dsig));
    degree1_edge_reg_cert(
        speed_sq,
        m.clone(),
        Interval {
            lo: e.sigma_lo.clone(),
            hi: e.sigma_hi.clone(),
        },
    )
}

/// Drive the clean-miter branch end to end: pair the two flanks' cap-outline cut edges,
/// license each pairing with [`miter_fit`], materialize the [`MiterLedger`](certify_core::miter::MiterLedger)
/// with [`miter_edge_ledger`], and audit the inventory with [`miter_out`].
///
/// `a[i]` and `b[i]` are flank A's and flank B's cut edges at cyclic position `i` (head to
/// tail around the cap); `crease_dir` is the shared crease-line direction `m̂`; `claimed[i]`
/// the searcher's order-sign claim for pairing `i`; `occ[i]` its transverse occupancy; and
/// `margin` the EDGE-REG separation each edge must clear. All slices must have equal length.
///
/// On success returns [`miter_out`]'s [`Verdict`] (a [`Verified`](Verdict::Verified)
/// [`MiterOutWitness`] or a [`Refuted`](Verdict::Refuted) [`MiterOutFault`]). A MITER-FIT
/// refusal short-circuits to [`MiterSearchError::NotClean`] — the searcher's cue to try the
/// LEDGE branch instead.
///
/// ```
/// use closure::miter::{clean_miter_cap, segment_cut_ends};
/// use certify_core::Verdict;
/// use certify_core::miter::{Occupancy, OrderSign};
/// use lattice::{Bignum, Rat};
///
/// let p = |x: i128, y: i128| (Rat::<Bignum>::from_i128(x), Rat::from_i128(y));
/// let q = |v: i128| Rat::<Bignum>::from_i128(v);
/// // A diamond cap outline whose every edge is transverse to the crease direction m̂ = ŷ,
/// // each edge traced identically by both flanks (a clean, order-preserving miter).
/// let verts = [p(2, 0), p(0, 2), p(-2, 0), p(0, -2)];
/// let mut a = Vec::new();
/// for k in 0..4 {
///     let s = verts[k].clone();
///     let e = verts[(k + 1) % 4].clone();
///     a.push(segment_cut_ends(s, e, q(0), q(1)));
/// }
/// let b = a.clone();
/// let occ = vec![Occupancy { a_l: true, a_r: false, b_l: false, b_r: true, frame: false }; 4];
/// let claimed = vec![OrderSign::Preserving; 4];
/// let out = clean_miter_cap(&a, &b, &p(0, 1), &claimed, &occ, &q(1)).expect("a clean miter");
/// assert!(matches!(out, Verdict::Verified(_)));
/// ```
#[allow(clippy::too_many_arguments)]
pub fn clean_miter_cap<B: Backend>(
    a: &[CutEnds<B>],
    b: &[CutEnds<B>],
    crease_dir: &(Rat<B>, Rat<B>),
    claimed: &[OrderSign],
    occ: &[Occupancy],
    margin: &Rat<B>,
) -> Result<Verdict<MiterOutWitness<B>, MiterOutFault, ()>, MiterSearchError<B>> {
    let n = a.len();
    if b.len() != n || claimed.len() != n || occ.len() != n {
        return Err(MiterSearchError::Ragged);
    }
    let mut fits: Vec<MiterFit<B>> = Vec::with_capacity(n);
    for i in 0..n {
        let cert = certify_core::miter::MiterFitCert {
            crease_dir: crease_dir.clone(),
            a: a[i].clone(),
            b: b[i].clone(),
            claimed: claimed[i],
        };
        match miter_fit(&cert) {
            Verdict::Verified(fit) => fits.push(fit),
            Verdict::Refuted(fault) => return Err(MiterSearchError::NotClean { at: i, fault }),
            Verdict::Unresolved(()) => return Err(MiterSearchError::Inconclusive { at: i }),
        }
    }
    let ledger = miter_edge_ledger(&fits, occ).ok_or(MiterSearchError::Ragged)?;
    let edge_regs = a.iter().map(|e| edge_reg_for(e, margin)).collect();
    Ok(miter_out(&MiterOutCert { ledger, edge_regs }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice::{Poly, RatFunc};

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
    /// A cylinder about the x-axis (`q = 1 + σi`) — straight rulings, so its projected cut
    /// edges are genuine degree-1 lines.
    fn cylinder() -> Chart<Bignum> {
        Chart::new(
            [poly(&[1]), poly(&[0, 1]), poly(&[0]), poly(&[0])],
            RatFunc::zero(),
        )
    }
    /// The `xy`-plane frame.
    fn xy_frame() -> PiFrame<Bignum> {
        let e = |i: usize| {
            let mut arr = [q(0), q(0), q(0)];
            arr[i] = q(1);
            arr
        };
        PiFrame {
            origin: [q(0), q(0), q(0)],
            u: e(0),
            v: e(1),
        }
    }
    fn bb() -> Occupancy {
        Occupancy {
            a_l: true,
            a_r: false,
            b_l: false,
            b_r: true,
            frame: false,
        }
    }

    /// A real cylinder ruling projects to a degree-1 cut edge; pairing that edge with itself
    /// (a symmetric clean miter) fits order-preserving. This exercises the projection path
    /// (`project_point` on `c + w·n + μ·r`) that feeds MITER-FIT.
    #[test]
    fn a_projected_cylinder_ruling_pairs_as_a_clean_miter() {
        let edge = ruling_cut_ends(
            &cylinder(),
            &q(1), // σ* = 1
            &q(0), // w = 0
            q(-1), // μ⁻
            q(1),  // μ⁺
            &xy_frame(),
        )
        .expect("cylinder ruling non-singular at σ*=1");
        // The ruling of this cylinder projects along x̂; the crease coordinate ℓ = e·m̂ is
        // monotone iff m̂ has a component along the edge, so m̂ = x̂ is the transverse choice
        // (m̂ = ŷ would be the parallel regime, ℓ constant → LEDGE).
        let cert = certify_core::miter::MiterFitCert {
            crease_dir: p(1, 0),
            a: edge.clone(),
            b: edge,
            claimed: OrderSign::Preserving,
        };
        match miter_fit(&cert) {
            Verdict::Verified(fit) => assert_eq!(fit.eps_phi, OrderSign::Preserving),
            other => panic!("a coincident ruling pairing must fit: {other:?}"),
        }
    }

    /// A diamond cap outline, every edge traced identically by both flanks, transverse to the
    /// crease direction ŷ — the clean-miter branch end to end: MITER-FIT ×4 → ledger →
    /// MITER-OUT `Verified` with four cleared edge margins.
    #[test]
    fn a_clean_miter_cap_cycle_certifies() {
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
        let occ = vec![bb(); 4];
        let claimed = vec![OrderSign::Preserving; 4];
        match clean_miter_cap(&a, &b, &p(0, 1), &claimed, &occ, &q(1)).expect("clean miter") {
            Verdict::Verified(w) => assert_eq!(w.edge_margins.len(), 4),
            other => panic!("a simple transverse miter cycle must certify: {other:?}"),
        }
    }

    /// CM.4 geometry: a **cone** (rotating rulings) has a genuinely-rational, **non-affine**
    /// crease-line coordinate `ℓ(σ)` (here `num` deg 1 / `den` deg 2), so the correspondence
    /// `R = ℓ_A(σ_A) = ℓ_B(σ_B)` **factors** — `(2, −1)` is an off-diagonal (spurious) solution.
    /// The decisive fact for the transverse MITER-FIT: the carrier `X = D_A × D_B` vanishes on
    /// that spurious branch **too** — because two rulings of one cone meeting `L` at a shared
    /// point pass through {apex, point} and so are the *same line* (shared-apex geometry). So
    /// the **full `R` divides `X`**, and CM.1's cofactor check certifies the reflection-mate
    /// cone miter as-is — no branch-refinement needed for it.
    #[test]
    fn a_cone_transverse_cut_family_certifies_through_the_full_r() {
        use lattice::Biv;
        // A cone (h = 0) with rotating rulings: q = (1, σ, 1, 0).
        let cone = Chart::new(
            [poly(&[1]), poly(&[0, 1]), poly(&[1]), poly(&[0])],
            RatFunc::zero(),
        );
        // A transversal bisector plane Π = {x = 1}, frame (u, v) = (ŷ, ẑ), L = {v = 0}.
        let bj = [q(1), q(0), q(0)];
        let frame = PiFrame {
            origin: [q(1), q(0), q(0)],
            u: [q(0), q(1), q(0)],
            v: [q(0), q(0), q(1)],
        };
        let (ell, dx, dy) = transverse_cut_family(&cone, &bj, &frame).expect("transverse cut");
        assert_eq!(
            dx.den(),
            dy.den(),
            "the two direction components share a denominator"
        );

        // ℓ is genuinely rational and NON-affine (reduced num deg 1 / den deg 2) ⇒ R factors.
        let g = ell.num().gcd(ell.den());
        let (rn, _) = ell.num().divrem(&g);
        let (rd, _) = ell.den().divrem(&g);
        assert_eq!(rn.degree(), Some(1), "ℓ numerator is degree 1");
        assert_eq!(
            rd.degree(),
            Some(2),
            "ℓ denominator is degree 2 (ℓ non-affine)"
        );

        // Reflection-mate miter (ℓ_B = ℓ_A, D_B = D_A): the correspondence R and carrier X.
        let bx = |p: &Poly<Bignum>| Biv::from_x_poly(p);
        let by = |p: &Poly<Bignum>| Biv::from_y_poly(p);
        let r_corr = bx(&rn).mul(&by(&rd)).sub(&by(&rn).mul(&bx(&rd)));
        let x_carrier = bx(dx.num())
            .mul(&by(dy.num()))
            .sub(&bx(dy.num()).mul(&by(dx.num())));

        // R factors: (2, −1) is an off-diagonal solution (ℓ(2) = ℓ(−1)), so R ≠ const·(σ_A−σ_B).
        assert_eq!(r_corr.eval(&q(2), &q(-1)), q(0), "(2,−1) lies on {{R=0}}");
        // The carrier holds there too (shared-apex ⇒ same line) ⇒ full R divides X.
        assert_eq!(
            x_carrier.eval(&q(2), &q(-1)),
            q(0),
            "carrier vanishes on the spurious branch ⇒ full-R cofactor check certifies the cone miter"
        );
        // And on the diagonal (antisymmetric), as always.
        assert_eq!(x_carrier.eval(&q(3), &q(3)), q(0));
    }

    /// A reversed pairing on one edge (B traces it end-to-start) claimed order-preserving is
    /// refused by the geometry-minted `ε_φ` — the searcher's cue to route to LEDGE.
    #[test]
    fn a_reversed_pairing_claimed_preserving_routes_to_ledge() {
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
        let mut b = a.clone();
        // Reverse flank B's second edge: same segment, opposite orientation ⇒ true ε_φ is
        // Reversing, but we (wrongly) claim Preserving.
        let rev = &a[1];
        b[1] = segment_cut_ends(rev.end.clone(), rev.start.clone(), q(0), q(1));
        let occ = vec![bb(); 4];
        let claimed = vec![OrderSign::Preserving; 4];
        assert!(matches!(
            clean_miter_cap(&a, &b, &p(0, 1), &claimed, &occ, &q(1)),
            Err(MiterSearchError::NotClean {
                at: 1,
                fault: MiterFault::OrderMismatch {
                    minted: OrderSign::Reversing
                }
            })
        ));
    }
}
