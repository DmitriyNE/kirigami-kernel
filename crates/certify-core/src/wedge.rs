//! The per-joint fan/collar **regularity bundle** (spec §8.5, the `REG-V / WEDGE /
//! EXT-WEDGE / SIDE / COLLAR` row at :382): the crease-local gauges that certify a
//! closure joint's fan wedge is a well-formed sub-π sector with a genuine (non-flat)
//! dihedral, embeddable over the extended bevel range.
//!
//! # The constant-V reduction (straight-crease scope)
//!
//! The fan is carried by the rational half-angle vector `V(t) = (n_A × n_B)/(1 + n_A·n_B)`
//! (spec §8.5:266). On the v1 **straight-crease** population `V` is constant along the
//! crease, so every gauge here is a single ring comparison at the crease station rather
//! than a Sturm-over-span check. Writing `d = n_A·n_B` for the two **unit** crease normals,
//!
//! ```text
//! |V|² = |n_A × n_B|² / (1 + d)²  =  (1 − d²)/(1 + d)²  =  (1 − d)/(1 + d)   (unit n_A, n_B)
//! ```
//!
//! so `|V|²` is rational in `d` alone — and, crucially, its denominator `1 + d` is exactly
//! the WEDGE quantity. Every predicate that would divide by `1 + d` is instead **cleared**
//! against it (spec invariant 7, the [`crate::margin`] doctrine): with WEDGE giving `1 + d > 0`,
//!
//! - **WEDGE** (fan sector sub-π on `[0,1]`, spec:382): `1 + d > 0`  (`= 2cos²(θ/2) > 0 ⟺ θ < π`).
//! - **REG-V** (`|V|² ≥ m > 0`; `V = 0` deletes the record): `(1 − d) − m·(1 + d) ≥ 0`.
//! - **EXT-WEDGE** (`s_bev(1 + s_bev)·|V|² < 1`, sub-π over the *extended* range): the
//!   `[0,1]` WEDGE bound does **not** certify the extension, so this is a separate strict
//!   comparison — `(1 + d) − s_bev(1 + s_bev)·(1 − d) > 0`.
//!
//! No rational division is ever taken; the checker is √-free and `Rat`-total.
//!
//! # SIDE and COLLAR: crease-local witness here, support content in C3
//!
//! [`regularity`] also stands as the crease-local witness for two bundle members whose
//! *independently-refutable* content is support-level, not crease-local:
//!
//! - **SIDE(b_J)** — "one-sided-w sign + trim complementarity on `{Q ⋛ 0}`" (spec:382). The
//!   oriented bisector `b_J = s_J(n_A − n_B)` is nonzero exactly when `|b_J|² = 2(1 − d) > 0`,
//!   and the bevel parameter split `Q(s) = 1 − 2s − |V|²s²` has `Q(0) = 1 > 0 > Q(1) =
//!   −(1 + |V|²)` for free once `|V|² ≥ 0` — so the crease-local complementarity is *implied*
//!   by REG-V ∧ WEDGE. The refutable "wrong-side" content (the retained side `G_i ≥ 0` holding
//!   over the actual flank support) is **TRIM-LOCAL**, built in C3 where the `G_i` fields exist.
//! - **COLLAR** — "quotient-wedge embedding: WEDGE per-t ∧ TUBE cross-t padded by `D_collar`;
//!   scope straight creases only" (spec:382). Per-t (constant V ⇒ one t) is WEDGE, and the
//!   extended-range embedding is EXT-WEDGE; the cross-t **TUBE** padding by the reach
//!   `D²_collar = 4w²·s_bev²·|V|²/(1 + s_bev²|V|²)` (spec:266) is **TUBE-LOCAL**, C3.
//!
//! So the C2 surface certifies the three independent crease-local atoms; the two
//! support-scoped members join their sibling checkers in C3. See `docs/vv-guide.md §8`.

use crate::margin::MarginSq;
use crate::verdict::Verdict;
use lattice::{Backend, Bignum, Rat};

/// Which crease normal a [`WedgeFault::NonUnitNormal`] refers to.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum WedgeFlank {
    /// Flank A (`b_A = b_J`).
    A,
    /// Flank B (`b_B = −b_J`).
    B,
}

/// The crease-station regularity certificate: the two **unit** flank normals at the crease,
/// the bevel slope, and the proposed REG-V lower margin.
///
/// The searcher (`closure`) evaluates `n_A`, `n_B` from the two flank charts' unit normals
/// at their crease stations `σ_a`, `σ_b`; `s_bev` is the authored bevel slope (dimensionless,
/// projective — `≥ 0`); `reg_v_margin` is the proposed `|V|²` lower bound `m > 0`. The
/// checker re-derives `d = n_A·n_B` and clears every predicate against `1 + d` itself.
pub struct WedgeCert<B: Backend = Bignum> {
    /// Unit normal of flank A at its crease station `σ_a`.
    pub n_a: [Rat<B>; 3],
    /// Unit normal of flank B at its crease station `σ_b`.
    pub n_b: [Rat<B>; 3],
    /// The bevel slope `s_bev ≥ 0` (dimensionless).
    pub s_bev: Rat<B>,
    /// The proposed REG-V squared margin `m > 0` (`|V|² ≥ m`).
    pub reg_v_margin: MarginSq<Rat<B>>,
}

/// The evidence a [`regularity`] `Verified` carries: the cleared crease dot `d = n_A·n_B`
/// and the REG-V margin the fan cleared. `|V|² = (1 − d)/(1 + d)` is recoverable from `d`
/// (never stored divided).
pub struct WedgeWitness<B: Backend = Bignum> {
    /// The crease dot `d = n_A·n_B` (`∈ (−1, 1)` on a certified joint).
    pub n_dot: Rat<B>,
    /// The REG-V squared margin the fan cleared (`|V|² ≥ m`).
    pub reg_v_margin: MarginSq<Rat<B>>,
}

/// Why the regularity bundle refused a certificate.
///
/// Separates **malformed paperwork** ([`NonUnitNormal`](WedgeFault::NonUnitNormal),
/// [`NegativeBevel`](WedgeFault::NegativeBevel), [`NonPositiveMargin`](WedgeFault::NonPositiveMargin)
/// — the inputs are not a well-formed certificate) from a genuine **geometric refutation**
/// ([`OverPiWedge`](WedgeFault::OverPiWedge), [`BelowMarginV`](WedgeFault::BelowMarginV),
/// [`ExtWedgeExceeded`](WedgeFault::ExtWedgeExceeded) — a real degeneracy of the fan).
pub enum WedgeFault<B: Backend = Bignum> {
    /// A crease normal is not a unit vector (`|n|² ≠ 1`) — malformed searcher input.
    NonUnitNormal {
        /// Which normal failed.
        flank: WedgeFlank,
        /// The offending `|n|²`.
        norm_sq: Rat<B>,
    },
    /// The bevel slope is negative (`s_bev` is a dimensionless slope, `≥ 0`).
    NegativeBevel {
        /// The offending `s_bev`.
        s_bev: Rat<B>,
    },
    /// The REG-V margin is not strictly positive — not a regularity certificate at all
    /// (mirrors [`crate::certify1d`]'s `NonPositiveMargin`: a negative `m` would `Verified`
    /// a degenerate zero-dihedral fold).
    NonPositiveMargin,
    /// WEDGE: the fan sector reached or exceeded π (`1 + n_A·n_B ≤ 0`, i.e. `θ ≥ π`).
    OverPiWedge {
        /// The non-positive `1 + n_A·n_B`.
        one_plus_dot: Rat<B>,
    },
    /// REG-V: `|V|²` fell below the margin `m` — a near-flat or zero-dihedral fold (`V = 0`
    /// deletes the record; there is no wedge to fill).
    BelowMarginV {
        /// The cleared residual `(1 − d) − m·(1 + d) < 0`.
        residual: Rat<B>,
    },
    /// EXT-WEDGE: the extended bevel sweep reached π (`s_bev(1 + s_bev)·|V|² ≥ 1`).
    ExtWedgeExceeded {
        /// The cleared residual `(1 + d) − s_bev(1 + s_bev)(1 − d) ≤ 0`.
        cleared: Rat<B>,
    },
}

/// `n_A · n_B` for two rational 3-vectors.
fn dot<B: Backend>(a: &[Rat<B>; 3], b: &[Rat<B>; 3]) -> Rat<B> {
    a[0].mul(&b[0]).add(&a[1].mul(&b[1])).add(&a[2].mul(&b[2]))
}

/// `|n|² − 1` is zero iff `n` is a unit vector. Returns the offending `|n|²` when it is not.
fn unit_check<B: Backend>(n: &[Rat<B>; 3]) -> Option<Rat<B>> {
    let norm_sq = dot(n, n);
    if norm_sq.sub(&Rat::from_i128(1)).is_zero() {
        None
    } else {
        Some(norm_sq)
    }
}

/// Well-formedness common to every atom: both normals unit, bevel non-negative. Returns the
/// first fault, or the crease dot `d = n_A·n_B` when the paperwork is clean.
fn wellformed<B: Backend>(cert: &WedgeCert<B>) -> Result<Rat<B>, WedgeFault<B>> {
    if let Some(norm_sq) = unit_check(&cert.n_a) {
        return Err(WedgeFault::NonUnitNormal {
            flank: WedgeFlank::A,
            norm_sq,
        });
    }
    if let Some(norm_sq) = unit_check(&cert.n_b) {
        return Err(WedgeFault::NonUnitNormal {
            flank: WedgeFlank::B,
            norm_sq,
        });
    }
    if cert.s_bev.sign() < 0 {
        return Err(WedgeFault::NegativeBevel {
            s_bev: cert.s_bev.clone(),
        });
    }
    Ok(dot(&cert.n_a, &cert.n_b))
}

/// WEDGE (spec §8.5): the fan sector is sub-π on `[0,1]` — `1 + n_A·n_B > 0`. Total.
///
/// This is the denominator of `|V|²`; the REG-V and EXT-WEDGE clearings are sound only
/// once it is positive, so [`regularity`] runs WEDGE first.
pub fn wedge<B: Backend>(cert: &WedgeCert<B>) -> Verdict<(), WedgeFault<B>, ()> {
    let d = match wellformed(cert) {
        Ok(d) => d,
        Err(f) => return Verdict::Refuted(f),
    };
    let one_plus_dot = Rat::from_i128(1).add(&d);
    if one_plus_dot.sign() <= 0 {
        return Verdict::Refuted(WedgeFault::OverPiWedge { one_plus_dot });
    }
    Verdict::Verified(())
}

/// REG-V (spec §8.5): `|V|² ≥ m > 0` at the crease — a genuine, non-flat dihedral. Total.
///
/// With WEDGE clearing the denominator (`1 + d > 0`), `|V|² = (1 − d)/(1 + d) ≥ m` becomes
/// the ring comparison `(1 − d) − m·(1 + d) ≥ 0`. A non-positive margin is rejected outright
/// ([`WedgeFault::NonPositiveMargin`]) — a regularity atom exists to bound `|V|²` *away* from
/// zero, and `V = 0` (a zero-dihedral joint) deletes the record rather than certifying it.
pub fn reg_v<B: Backend>(cert: &WedgeCert<B>) -> Verdict<MarginSq<Rat<B>>, WedgeFault<B>, ()> {
    let d = match wellformed(cert) {
        Ok(d) => d,
        Err(f) => return Verdict::Refuted(f),
    };
    if cert.reg_v_margin.0.sign() <= 0 {
        return Verdict::Refuted(WedgeFault::NonPositiveMargin);
    }
    let one_plus_dot = Rat::from_i128(1).add(&d);
    if one_plus_dot.sign() <= 0 {
        return Verdict::Refuted(WedgeFault::OverPiWedge { one_plus_dot });
    }
    let one_minus_dot = Rat::from_i128(1).sub(&d);
    // R = (1 − d) − m·(1 + d) ≥ 0  ⟺  |V|² ≥ m  (since 1 + d > 0).
    let residual = one_minus_dot.sub(&cert.reg_v_margin.0.mul(&one_plus_dot));
    if residual.sign() < 0 {
        return Verdict::Refuted(WedgeFault::BelowMarginV { residual });
    }
    Verdict::Verified(cert.reg_v_margin.clone())
}

/// EXT-WEDGE (spec §8.5): the extended bevel sweep stays sub-π — `s_bev(1 + s_bev)·|V|² < 1`.
/// Total.
///
/// The `[0,1]` WEDGE bound does **not** certify the extension (spec §8.5:266), so this is a
/// separate, strict comparison. Clearing `1 + d > 0`: `(1 + d) − s_bev(1 + s_bev)(1 − d) > 0`.
pub fn ext_wedge<B: Backend>(cert: &WedgeCert<B>) -> Verdict<(), WedgeFault<B>, ()> {
    let d = match wellformed(cert) {
        Ok(d) => d,
        Err(f) => return Verdict::Refuted(f),
    };
    let one_plus_dot = Rat::from_i128(1).add(&d);
    if one_plus_dot.sign() <= 0 {
        return Verdict::Refuted(WedgeFault::OverPiWedge { one_plus_dot });
    }
    let one_minus_dot = Rat::from_i128(1).sub(&d);
    // s_bev·(1 + s_bev): the extended-range coefficient.
    let s_coeff = cert.s_bev.mul(&Rat::from_i128(1).add(&cert.s_bev));
    // cleared = (1 + d) − s_bev(1 + s_bev)(1 − d) > 0  ⟺  s_bev(1 + s_bev)|V|² < 1.
    let cleared = one_plus_dot.sub(&s_coeff.mul(&one_minus_dot));
    if cleared.sign() <= 0 {
        return Verdict::Refuted(WedgeFault::ExtWedgeExceeded { cleared });
    }
    Verdict::Verified(())
}

/// The regularity bundle: WEDGE ∧ REG-V ∧ EXT-WEDGE at the crease, short-circuiting to the
/// first refutation. Total — `Verified(`[`WedgeWitness`]`)` or `Refuted(`[`WedgeFault`]`)`.
///
/// This is also the crease-local witness for SIDE(b_J) and COLLAR (module docs): the oriented
/// bisector is nonzero and the bevel split is complementary once REG-V ∧ WEDGE hold, and the
/// quotient-wedge embeds once WEDGE ∧ EXT-WEDGE hold. Their support-scoped content
/// (retained-side `G_i ≥ 0`; cross-t TUBE padding) is certified in C3.
pub fn regularity<B: Backend>(cert: &WedgeCert<B>) -> Verdict<WedgeWitness<B>, WedgeFault<B>, ()> {
    // WEDGE first: it clears the denominator the other two divide by.
    if let Verdict::Refuted(f) = wedge(cert) {
        return Verdict::Refuted(f);
    }
    let reg_v_margin = match reg_v(cert) {
        Verdict::Verified(m) => m,
        Verdict::Refuted(f) => return Verdict::Refuted(f),
        Verdict::Unresolved(u) => return Verdict::Unresolved(u),
    };
    if let Verdict::Refuted(f) = ext_wedge(cert) {
        return Verdict::Refuted(f);
    }
    Verdict::Verified(WedgeWitness {
        n_dot: dot(&cert.n_a, &cert.n_b),
        reg_v_margin,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(n: i128, d: i128) -> Rat<Bignum> {
        Rat::new(n, d)
    }
    fn e(i: usize) -> [Rat<Bignum>; 3] {
        let mut a = [Rat::from_i128(0), Rat::from_i128(0), Rat::from_i128(0)];
        a[i] = Rat::from_i128(1);
        a
    }
    /// A 90° fold: n_A = x̂, n_B = ŷ ⇒ d = 0, |V|² = 1.
    fn right_angle(s_bev: Rat<Bignum>, m: Rat<Bignum>) -> WedgeCert<Bignum> {
        WedgeCert {
            n_a: e(0),
            n_b: e(1),
            s_bev,
            reg_v_margin: MarginSq(m),
        }
    }

    #[test]
    fn a_right_angle_fold_clears_the_bundle() {
        // d = 0 ⇒ |V|² = 1: WEDGE (1 > 0), REG-V (1 ≥ 1/2), EXT-WEDGE ((1/4)(5/4) = 5/16 < 1).
        let cert = right_angle(r(1, 4), r(1, 2));
        let w = match regularity(&cert) {
            Verdict::Verified(w) => w,
            _ => panic!("a 90° fold with a small bevel is regular"),
        };
        assert!(w.n_dot.is_zero());
        assert_eq!(w.reg_v_margin.0, r(1, 2));
    }

    #[test]
    fn an_over_pi_fold_fails_wedge() {
        // Antipodal normals: d = −1 ⇒ 1 + d = 0, the fan sector has reached π.
        let cert = WedgeCert {
            n_a: e(0),
            n_b: [Rat::from_i128(-1), Rat::from_i128(0), Rat::from_i128(0)],
            s_bev: r(1, 4),
            reg_v_margin: MarginSq(r(1, 2)),
        };
        assert!(matches!(
            wedge(&cert),
            Verdict::Refuted(WedgeFault::OverPiWedge { .. })
        ));
        // The bundle refutes at WEDGE, before REG-V clears the (now non-positive) denominator.
        assert!(matches!(
            regularity(&cert),
            Verdict::Refuted(WedgeFault::OverPiWedge { .. })
        ));
    }

    #[test]
    fn a_zero_dihedral_fails_reg_v() {
        // n_A = n_B ⇒ d = 1 ⇒ |V|² = 0: WEDGE passes (1 + 1 = 2 > 0) but REG-V does not —
        // the record should be deleted, never certified.
        let cert = WedgeCert {
            n_a: e(0),
            n_b: e(0),
            s_bev: r(1, 4),
            reg_v_margin: MarginSq(r(1, 2)),
        };
        assert!(matches!(wedge(&cert), Verdict::Verified(())));
        assert!(matches!(
            reg_v(&cert),
            Verdict::Refuted(WedgeFault::BelowMarginV { .. })
        ));
        assert!(matches!(
            regularity(&cert),
            Verdict::Refuted(WedgeFault::BelowMarginV { .. })
        ));
    }

    #[test]
    fn a_large_bevel_fails_ext_wedge_while_wedge_and_reg_v_hold() {
        // d = 0 ⇒ |V|² = 1; s_bev = 1 ⇒ s_bev(1+s_bev)|V|² = 2 ≥ 1: sub-π on [0,1] but the
        // extended sweep reaches π. WEDGE and REG-V pass; EXT-WEDGE is the one that refuses.
        let cert = right_angle(Rat::from_i128(1), r(1, 2));
        assert!(matches!(wedge(&cert), Verdict::Verified(())));
        assert!(matches!(reg_v(&cert), Verdict::Verified(_)));
        assert!(matches!(
            ext_wedge(&cert),
            Verdict::Refuted(WedgeFault::ExtWedgeExceeded { .. })
        ));
        assert!(matches!(
            regularity(&cert),
            Verdict::Refuted(WedgeFault::ExtWedgeExceeded { .. })
        ));
    }

    #[test]
    fn a_non_unit_normal_is_rejected_as_malformed() {
        let cert = WedgeCert {
            n_a: [Rat::from_i128(2), Rat::from_i128(0), Rat::from_i128(0)],
            n_b: e(1),
            s_bev: r(1, 4),
            reg_v_margin: MarginSq(r(1, 2)),
        };
        assert!(matches!(
            regularity(&cert),
            Verdict::Refuted(WedgeFault::NonUnitNormal {
                flank: WedgeFlank::A,
                ..
            })
        ));
    }

    #[test]
    fn a_non_positive_margin_is_rejected() {
        let cert = right_angle(r(1, 4), Rat::from_i128(0));
        assert!(matches!(
            reg_v(&cert),
            Verdict::Refuted(WedgeFault::NonPositiveMargin)
        ));
    }
}
