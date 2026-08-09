//! MITER branch checkers — the clean-miter cap, line-edge / degree-1 (spec §8.5, the
//! `MITER-FIT` / `MITER-EDGE-LEDGER → MITER-OUT` rows at :383–:384).
//!
//! When two flanks' trimmed cut faces **coincide** in the bisector plane Π, the closure
//! caps as a *clean miter*: there is no exposed planar ledge to arrange (the symmetric
//! difference `F_A △ F_B` is empty), only the shared cut edges, paired across the flanks.
//! This module certifies that pairing and audits the resulting edge inventory. It is the
//! disjoint alternative to the LEDGE branch (`crate::arrange` + `arrange2d`), and the
//! CLOSURE cap is `MITER-BRANCH ∨ LEDGE-BRANCH`.
//!
//! # `ε_φ` — the order sign, by endpoint comparison (never a derivative)
//!
//! The cross-flank pairing `φ_J` maps flank A's cut-edge parameter `σ_A` to flank B's
//! `σ_B` so the two edges trace the *same* points of Π. Its **order sign** `ε_φ` — whether
//! `φ_J` runs the two parametrizations the same way or opposite — is minted by [`eps_phi`]
//! from a *single exact oriented-endpoint comparison* of `φ_J`'s endpoint images. The
//! derivative form `sgn(dσ_B/dσ_A)` is a *theorem on the regular locus*, **never** the
//! definition: `σ_B = σ_A³` is strictly monotone with positive endpoint order yet has a
//! zero derivative at the origin, so a derivative-sign mint is a fossil that its own
//! computation cannot evaluate there. The endpoint comparison is total and constant for a
//! certified-monotone `φ_J`, and gets `σ_A³` right (see the module tests).
//!
//! # Degree-1 corollary (the line-edge / cylinder-flank slice)
//!
//! For line-carrier cut edges (planar or cylinder-type flanks) each edge is affine, so the
//! crease-line coordinate `ℓ_i(σ) = e_i(σ)·m̂` is affine and monotone iff its slope is
//! nonzero — the Sturm monotonicity test collapses to a single ring sign. `φ_J` is then an
//! explicit degree-1 rational map and **no resultant machinery is needed** (spec §8.5): the
//! two edges coincide as point sets iff their endpoints match, and the endpoint match
//! *is* `φ_J`'s endpoint image, so [`eps_phi`] reads `ε_φ` straight off it. [`miter_fit`]
//! certifies exactly this. The conic (cone-flank) pass — where `ℓ_i` is genuinely rational
//! and `φ_J` needs the resultant — is deferred with the §13 petal geometry.

use alloc::vec::Vec;
use core::cmp::Ordering;

use lattice::{AlgReal, Backend, Bignum, Biv, Interval, Poly, Rat, RatFunc, SturmChain};

use crate::certify1d::{EdgeReg, EdgeRegCert, edge_reg};
use crate::margin::MarginSq;
use crate::verdict::Verdict;

/// The order sign `ε_φ` of a monotone cross-flank correspondence `φ_J` (spec §8.5): whether
/// the pairing runs the two flanks' cut-edge parametrizations the same way or opposite.
///
/// Minted by [`eps_phi`] from one exact oriented-endpoint comparison — **never** a
/// derivative sign. Total and constant for a certified-monotone `φ_J`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OrderSign {
    /// `φ_J` preserves order: `σ_A` increasing pairs with `σ_B` increasing.
    Preserving,
    /// `φ_J` reverses order: `σ_A` increasing pairs with `σ_B` decreasing.
    Reversing,
}

/// Mint the order sign `ε_φ` from the correspondence's endpoint images alone — the spec's
/// **one exact oriented-endpoint comparison**, never the derivative-sign definition.
///
/// `phi_at_lo` and `phi_at_hi` are `φ_J(σ_A_lo)` and `φ_J(σ_A_hi)` for the *ordered* support
/// `σ_A_lo < σ_A_hi`. The order sign is `sign(phi_at_hi − phi_at_lo)`:
/// [`Preserving`](OrderSign::Preserving) if positive, [`Reversing`](OrderSign::Reversing) if
/// negative. Returns `None` when the images coincide — `φ_J` is not injective on the
/// endpoints, so there is no monotone correspondence to sign.
///
/// This is total and correct on the regular locus *and* off it: `σ_B = σ_A³` over `[0, 1]`
/// has `φ_J(0) = 0`, `φ_J(1) = 1`, so `eps_phi(0, 1) = Preserving` — the zero derivative at
/// the origin, which would sink a `sgn(dσ_B/dσ_A)` mint, never enters.
///
/// ```
/// use certify_core::miter::{eps_phi, OrderSign};
/// use lattice::{Bignum, Rat};
///
/// // The σ_A³ fossil: strictly monotone, positive endpoint order, zero derivative at 0.
/// // The endpoint comparison mints +1 regardless — the derivative never enters.
/// let lo = Rat::<Bignum>::from_i128(0); // φ_J(0) = 0³ = 0
/// let hi = Rat::<Bignum>::from_i128(1); // φ_J(1) = 1³ = 1
/// assert_eq!(eps_phi(&lo, &hi), Some(OrderSign::Preserving));
///
/// // A reversed pairing: φ_J maps the lower σ_A to the *higher* σ_B.
/// let hi_lo = Rat::<Bignum>::from_i128(5);
/// let hi_hi = Rat::<Bignum>::from_i128(2);
/// assert_eq!(eps_phi(&hi_lo, &hi_hi), Some(OrderSign::Reversing));
/// ```
pub fn eps_phi<B: Backend>(phi_at_lo: &Rat<B>, phi_at_hi: &Rat<B>) -> Option<OrderSign> {
    eps_from_cmp(phi_at_lo.cmp(phi_at_hi))
}

/// Mint the order sign from the *ordering* of the two endpoint images alone — the
/// backend-free core of [`eps_phi`], factored out so the ★ soundness property runs on `i128`
/// under Kani (the [`crate::proof`] `eps_phi_is_endpoint_order` harness). `order` is
/// `φ_J(σ_lo).cmp(φ_J(σ_hi))` for the ordered support `σ_lo < σ_hi`:
///
/// - [`Less`](Ordering::Less) (`φ_J(σ_lo) < φ_J(σ_hi)`) ⇒ [`Preserving`](OrderSign::Preserving),
/// - [`Greater`](Ordering::Greater) ⇒ [`Reversing`](OrderSign::Reversing),
/// - [`Equal`](Ordering::Equal) ⇒ `None` (the images coincide — no monotone correspondence).
///
/// The decision reads *only* the endpoint order, so any two **distinct** images mint a definite
/// sign — the exact guarantee a `sgn(dσ_B/dσ_A)` derivative mint lacks (it collapses to `None`
/// wherever `φ_J′` vanishes, e.g. the `σ_A³` fossil at the origin, though the endpoints are
/// strictly ordered). Total and `const`.
pub const fn eps_from_cmp(order: Ordering) -> Option<OrderSign> {
    match order {
        Ordering::Less => Some(OrderSign::Preserving),
        Ordering::Greater => Some(OrderSign::Reversing),
        Ordering::Equal => None,
    }
}

/// A degree-1 cut edge in the cap plane Π: the two exact endpoints `e(σ_lo)`, `e(σ_hi)` of
/// an affine parametrization `e(σ) = start + (σ − σ_lo)·D`, over the support `[σ_lo, σ_hi]`.
/// The searcher (`closure`) projects a flank's trimmed cut face boundary into Π to build it.
#[derive(Debug)]
pub struct CutEnds<B: Backend = Bignum> {
    /// The endpoint at `σ_lo`.
    pub start: (Rat<B>, Rat<B>),
    /// The endpoint at `σ_hi`.
    pub end: (Rat<B>, Rat<B>),
    /// The support lower bound `σ_lo`.
    pub sigma_lo: Rat<B>,
    /// The support upper bound `σ_hi`.
    pub sigma_hi: Rat<B>,
}

// Manual `Clone` (no `B: Clone` bound — `Backend` implementors are marker types, as in
// `crate::cap_in`'s carriers); `Rat`'s own unconditional `Clone` does the element work.
impl<B: Backend> Clone for CutEnds<B> {
    fn clone(&self) -> Self {
        CutEnds {
            start: (self.start.0.clone(), self.start.1.clone()),
            end: (self.end.0.clone(), self.end.1.clone()),
            sigma_lo: self.sigma_lo.clone(),
            sigma_hi: self.sigma_hi.clone(),
        }
    }
}

/// A MITER-FIT certificate: the crease-line direction `m̂` (the `ℓ_i` pairing axis in Π),
/// the two flanks' cut edges, and the searcher's **claimed** order sign of `φ_J`.
///
/// [`miter_fit`] re-derives everything the claim rests on — it mints `ε_φ` from the geometry
/// and refuses if the claim disagrees, so a wrong claim cannot manufacture a pass.
#[derive(Clone, Debug)]
pub struct MiterFitCert<B: Backend = Bignum> {
    /// The crease line direction `m̂` in Π (must be nonzero) — `ℓ_i(σ) = e_i(σ)·m̂`.
    pub crease_dir: (Rat<B>, Rat<B>),
    /// Flank A's cut edge.
    pub a: CutEnds<B>,
    /// Flank B's cut edge.
    pub b: CutEnds<B>,
    /// The searcher's claimed order sign of `φ_J`.
    pub claimed: OrderSign,
}

/// The evidence a [`miter_fit`] `Verified` carries: the shared carrier's coincident edge (a
/// single Π segment both flanks trace) and the geometry-minted `ε_φ`. This is exactly what
/// [`miter_edge_ledger`] materializes as a PAIR-IDENTICAL ledger entry.
#[derive(Clone, Debug)]
pub struct MiterFit<B: Backend = Bignum> {
    /// The coincident cut edge's start, in flank A's orientation.
    pub start: (Rat<B>, Rat<B>),
    /// The coincident cut edge's end, in flank A's orientation.
    pub end: (Rat<B>, Rat<B>),
    /// The order sign of `φ_J`, minted by [`eps_phi`] from the endpoint match.
    pub eps_phi: OrderSign,
}

/// Why MITER-FIT refused a pairing — the miter is not clean, so the closure falls back to
/// the LEDGE branch (or the input is malformed).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MiterFault<B: Backend = Bignum> {
    /// The crease direction `m̂` is the zero vector — no pairing axis.
    DegenerateCrease,
    /// A cut edge has an empty or reversed support (`σ_lo ≥ σ_hi`).
    EmptySupport {
        /// `false` for flank A, `true` for flank B.
        flank_b: bool,
    },
    /// A cut edge's `ℓ_i(σ) = e_i(σ)·m̂` has zero slope — the edge is parallel to the crease
    /// line, so `ℓ_i` is not monotone. This is the *parallel regime* (cylinder cut lines
    /// `∥ L`), out of the transverse degree-1 slice; it routes to LEDGE (spec §8.5).
    CreaseParallel {
        /// `false` for flank A, `true` for flank B.
        flank_b: bool,
    },
    /// The two cut edges are not collinear — their carriers differ, so no clean miter. The
    /// witness is the nonzero cross product `D_A × D_B` (nonparallel) or the off-line
    /// residual (parallel but distinct lines).
    CarrierMismatch {
        /// The witnessing residual (zero only when the carriers coincide).
        residual: Rat<B>,
    },
    /// The two cut edges are collinear but their extents do not coincide as point sets —
    /// the segments differ, so there is no PAIR-IDENTICAL to mint (a genuine ledge, not a
    /// miter).
    ExtentMismatch,
    /// The geometry-minted `ε_φ` disagrees with the searcher's claimed order sign — the
    /// claimed pairing is not the one the endpoints witness (e.g. a reversed pairing claimed
    /// as order-preserving).
    OrderMismatch {
        /// The order sign the geometry actually witnesses.
        minted: OrderSign,
    },
}

/// `u × v` for 2D vectors — the scalar `u_x·v_y − u_y·v_x`. Shared with [`crate::sew`]'s
/// exact collinearity checks (`pub(crate)` — one definition, no duplicate).
pub(crate) fn cross2<B: Backend>(u: &(Rat<B>, Rat<B>), v: &(Rat<B>, Rat<B>)) -> Rat<B> {
    u.0.mul(&v.1).sub(&u.1.mul(&v.0))
}

/// `u · v` for 2D vectors.
pub(crate) fn dot2<B: Backend>(u: &(Rat<B>, Rat<B>), v: &(Rat<B>, Rat<B>)) -> Rat<B> {
    u.0.mul(&v.0).add(&u.1.mul(&v.1))
}

/// `p − q`.
pub(crate) fn sub2<B: Backend>(p: &(Rat<B>, Rat<B>), q: &(Rat<B>, Rat<B>)) -> (Rat<B>, Rat<B>) {
    (p.0.sub(&q.0), p.1.sub(&q.1))
}

/// MITER-FIT, degree-1 (spec §8.5): certify that two flanks' line-carrier cut edges coincide
/// in Π and mint the order sign `ε_φ` of their cross-flank pairing `φ_J`.
///
/// The checks, in order (short-circuiting to the first fault):
/// 1. **crease well-formed** — `m̂ ≠ 0`.
/// 2. **finite supports** — `σ_lo < σ_hi` on both edges.
/// 3. **`ℓ_i` monotone** — the slope `(e_i(σ_hi) − e_i(σ_lo))·m̂` is nonzero on both edges
///    (degree-1 monotonicity is a single ring sign; a zero slope is the parallel regime,
///    [`CreaseParallel`](MiterFault::CreaseParallel), routed to LEDGE).
/// 4. **carrier identity** — the two edges are collinear: `D_A × D_B = 0` (parallel) and B's
///    start lies on A's line (coincident). Position identity is then free by construction.
/// 5. **extent identity + `ε_φ`** — the endpoint sets coincide; whichever B endpoint matches
///    A's start is `φ_J(σ_A_lo)`, so [`eps_phi`] reads `ε_φ` off it. The searcher's
///    [`claimed`](MiterFitCert::claimed) sign must match, else
///    [`OrderMismatch`](MiterFault::OrderMismatch).
///
/// Total: `Verified(`[`MiterFit`]`)` or `Refuted(`[`MiterFault`]`)`, never `Unresolved`.
///
/// ```
/// use certify_core::miter::{miter_fit, CutEnds, MiterFitCert, OrderSign};
/// use certify_core::Verdict;
/// use lattice::{Bignum, Rat};
///
/// let p = |x: i128, y: i128| (Rat::<Bignum>::from_i128(x), Rat::from_i128(y));
/// // Both flanks trace the same Π segment (0,0)→(4,0) with the same orientation:
/// // a clean, order-preserving miter along the x-axis crease.
/// let edge = |sl: i128, sh: i128| CutEnds {
///     start: p(0, 0), end: p(4, 0), sigma_lo: Rat::from_i128(sl), sigma_hi: Rat::from_i128(sh),
/// };
/// let cert = MiterFitCert {
///     crease_dir: p(1, 0),
///     a: edge(0, 4),
///     b: edge(0, 4),
///     claimed: OrderSign::Preserving,
/// };
/// match miter_fit(&cert) {
///     Verdict::Verified(fit) => assert_eq!(fit.eps_phi, OrderSign::Preserving),
///     other => panic!("a coincident miter must fit: {other:?}"),
/// }
/// ```
pub fn miter_fit<B: Backend>(cert: &MiterFitCert<B>) -> Verdict<MiterFit<B>, MiterFault<B>, ()> {
    let m = &cert.crease_dir;
    // (1) crease well-formed.
    if m.0.is_zero() && m.1.is_zero() {
        return Verdict::Refuted(MiterFault::DegenerateCrease);
    }
    // (2) finite supports.
    if cert.a.sigma_lo.cmp(&cert.a.sigma_hi) != Ordering::Less {
        return Verdict::Refuted(MiterFault::EmptySupport { flank_b: false });
    }
    if cert.b.sigma_lo.cmp(&cert.b.sigma_hi) != Ordering::Less {
        return Verdict::Refuted(MiterFault::EmptySupport { flank_b: true });
    }
    let d_a = sub2(&cert.a.end, &cert.a.start);
    let d_b = sub2(&cert.b.end, &cert.b.start);
    // (3) ℓ_i monotone: nonzero slope of ℓ_i(σ) = e_i(σ)·m̂ (degree-1 ⇒ one ring sign).
    if dot2(&d_a, m).is_zero() {
        return Verdict::Refuted(MiterFault::CreaseParallel { flank_b: false });
    }
    if dot2(&d_b, m).is_zero() {
        return Verdict::Refuted(MiterFault::CreaseParallel { flank_b: true });
    }
    // (4) carrier identity: parallel directions ∧ B's start on A's line.
    let cross = cross2(&d_a, &d_b);
    if !cross.is_zero() {
        return Verdict::Refuted(MiterFault::CarrierMismatch { residual: cross });
    }
    // B.start on line(A): D_A × (B.start − A.start) = 0.
    let off = cross2(&d_a, &sub2(&cert.b.start, &cert.a.start));
    if !off.is_zero() {
        return Verdict::Refuted(MiterFault::CarrierMismatch { residual: off });
    }
    // (5) extent identity: the endpoint sets coincide. The B endpoint matching A.start is
    // φ_J(σ_A_lo); mint ε_φ from its σ against φ_J(σ_A_hi) via `eps_phi`.
    let (phi_lo, phi_hi) = if cert.a.start == cert.b.start && cert.a.end == cert.b.end {
        // σ_A_lo ↦ σ_B_lo, σ_A_hi ↦ σ_B_hi.
        (cert.b.sigma_lo.clone(), cert.b.sigma_hi.clone())
    } else if cert.a.start == cert.b.end && cert.a.end == cert.b.start {
        // σ_A_lo ↦ σ_B_hi, σ_A_hi ↦ σ_B_lo.
        (cert.b.sigma_hi.clone(), cert.b.sigma_lo.clone())
    } else {
        return Verdict::Refuted(MiterFault::ExtentMismatch);
    };
    let minted = match eps_phi(&phi_lo, &phi_hi) {
        Some(sign) => sign,
        // φ_J's endpoint images coincide despite distinct edge endpoints — impossible once
        // the supports are finite (checked at (2)), but reported rather than panicked.
        None => return Verdict::Refuted(MiterFault::ExtentMismatch),
    };
    if minted != cert.claimed {
        return Verdict::Refuted(MiterFault::OrderMismatch { minted });
    }
    Verdict::Verified(MiterFit {
        start: cert.a.start.clone(),
        end: cert.a.end.clone(),
        eps_phi: minted,
    })
}

/// A ledger edge's transverse occupancy `(A_L, A_R, B_L, B_R)` plus the packet frame bit
/// (spec §8.5, EDGE-OCCUPANCY at :44/:385): the four adjacent-cell occupancies of the two
/// flanks' material, `left` being the cross-product side of `(t_e, n_Π × t_e)`.
///
/// On the clean-miter branch every ledger edge is boundary-boundary: `A_L ≠ A_R ∧ B_L ≠ B_R`
/// ([`is_boundary_boundary`](Occupancy::is_boundary_boundary)). SEW (M5) consumes these bits;
/// M4 only **materializes** them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Occupancy {
    /// Flank A material on the left of the edge.
    pub a_l: bool,
    /// Flank A material on the right.
    pub a_r: bool,
    /// Flank B material on the left.
    pub b_l: bool,
    /// Flank B material on the right.
    pub b_r: bool,
    /// The packet frame bit (orientation of `left` relative to `n_Π × t_e`).
    pub frame: bool,
}

impl Occupancy {
    /// Whether the edge is on the boundary-boundary stratum: material changes across it on
    /// *both* flanks (`A_L ≠ A_R ∧ B_L ≠ B_R`). The clean miter lives entirely here, so it
    /// is also EDGE-COVERAGE's two-sided witness.
    pub fn is_boundary_boundary(&self) -> bool {
        self.a_l != self.a_r && self.b_l != self.b_r
    }
}

/// A materialized clean-miter ledger edge (spec §8.5, MITER-EDGE-LEDGER): a PAIR-IDENTICAL
/// paired Π-edge with its order sign and transverse occupancy. `V_cand = V_∂` on this branch
/// — every ledger edge is boundary, so its endpoints are emitted shell vertices.
#[derive(Clone, Debug)]
pub struct LedgerEdge<B: Backend = Bignum> {
    /// The paired edge's start (flank A orientation).
    pub start: (Rat<B>, Rat<B>),
    /// The paired edge's end.
    pub end: (Rat<B>, Rat<B>),
    /// The order sign `ε_φ` of the pairing that minted this edge.
    pub eps_phi: OrderSign,
    /// The transverse occupancy bits (materialized for SEW).
    pub occupancy: Occupancy,
}

/// The clean-miter edge inventory: the ordered cycle of [`LedgerEdge`]s.
#[derive(Clone, Debug)]
pub struct MiterLedger<B: Backend = Bignum> {
    /// The paired Π-edges, in head-to-tail cyclic order.
    pub edges: Vec<LedgerEdge<B>>,
}

/// MITER-EDGE-LEDGER (spec §8.5): materialize passed MITER-FIT identities as PAIR-IDENTICAL
/// ledger edges, each stamped with the caller's transverse occupancy.
///
/// `fits[k]` (a [`miter_fit`] witness) pairs with `occupancy[k]`; the lengths must match. The
/// order sign travels from the fit onto the ledger edge verbatim — the ledger is a record,
/// not a re-derivation. MITER-OUT then audits the inventory as a whole. Returns `None` on a
/// length mismatch (malformed caller input).
pub fn miter_edge_ledger<B: Backend>(
    fits: &[MiterFit<B>],
    occupancy: &[Occupancy],
) -> Option<MiterLedger<B>> {
    if fits.len() != occupancy.len() {
        return None;
    }
    let mut edges = Vec::with_capacity(fits.len());
    for (fit, occ) in fits.iter().zip(occupancy.iter()) {
        edges.push(LedgerEdge {
            start: fit.start.clone(),
            end: fit.end.clone(),
            eps_phi: fit.eps_phi,
            occupancy: *occ,
        });
    }
    Some(MiterLedger { edges })
}

/// A MITER-OUT certificate: the [`MiterLedger`] and, per edge in the same order, the
/// [`EdgeRegCert`] that EDGE-REG re-verifies (spec §8.5 reuses `edge_reg`).
pub struct MiterOutCert<B: Backend = Bignum> {
    /// The edge inventory to audit.
    pub ledger: MiterLedger<B>,
    /// One EDGE-REG certificate per ledger edge, same order.
    pub edge_regs: Vec<EdgeRegCert<B>>,
}

/// Why MITER-OUT refused the ledger's output. All faults are located by edge index (the
/// geometry that failed is in the [`MiterOutCert`] the caller already holds), so this carries
/// no backend-typed payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MiterOutFault {
    /// The ledger is empty — a clean miter has at least one paired cut edge.
    Empty,
    /// The EDGE-REG certificate count does not match the ledger edge count.
    RegCountMismatch,
    /// EDGE-REG failed on the edge at `at`: its `|e′|²` is not bounded away from zero (a
    /// cusp or a removable stall — the road back is band or REPARAM, spec §7/§14).
    EdgeReg {
        /// The offending edge index.
        at: usize,
    },
    /// CYCLE: the inventory does not close head-to-tail — edge `at`'s end is not the next
    /// edge's start (cyclically).
    OpenCycle {
        /// The edge whose end is dangling.
        at: usize,
    },
    /// EDGE-EDGE: two non-adjacent edges (`i`, `j`) intersect somewhere other than a shared
    /// cycle vertex — the boundary is not simple.
    EdgeEdge {
        /// The first edge.
        i: usize,
        /// The second edge.
        j: usize,
    },
    /// EDGE-COVERAGE: the edge at `at` is not two-sided (its occupancy is not on the
    /// boundary-boundary stratum) — an uncovered or one-sided cut edge.
    Coverage {
        /// The offending edge index.
        at: usize,
    },
    /// VERTEX-ISOLATION: two cycle vertices coincide beyond the head-to-tail chaining — the
    /// vertex quotient is not injective (a pinch). Reports the repeated endpoint index.
    VertexCollision {
        /// The edge whose start collides with a non-adjacent vertex.
        at: usize,
    },
}

/// The evidence a MITER-OUT `Verified` carries: the per-edge EDGE-REG margins.
#[derive(Clone, Debug)]
pub struct MiterOutWitness<B: Backend = Bignum> {
    /// The cleared `|e′|² ≥ m_e` margin of each ledger edge, in order.
    pub edge_margins: Vec<MarginSq<Rat<B>>>,
}

/// Orientation of the ordered triple `(a, b, c)`: `sign((b − a) × (c − a))`.
fn orient<B: Backend>(a: &(Rat<B>, Rat<B>), b: &(Rat<B>, Rat<B>), c: &(Rat<B>, Rat<B>)) -> i8 {
    cross2(&sub2(b, a), &sub2(c, a)).sign()
}

/// Whether the point `p` lies on the closed segment `[a, b]` (assumes collinearity is
/// checked separately by the caller — this is the between-ness test after `orient == 0`).
fn on_segment<B: Backend>(
    a: &(Rat<B>, Rat<B>),
    b: &(Rat<B>, Rat<B>),
    p: &(Rat<B>, Rat<B>),
) -> bool {
    let min_x = if a.0.cmp(&b.0) == Ordering::Less {
        &a.0
    } else {
        &b.0
    };
    let max_x = if a.0.cmp(&b.0) == Ordering::Less {
        &b.0
    } else {
        &a.0
    };
    let min_y = if a.1.cmp(&b.1) == Ordering::Less {
        &a.1
    } else {
        &b.1
    };
    let max_y = if a.1.cmp(&b.1) == Ordering::Less {
        &b.1
    } else {
        &a.1
    };
    min_x.cmp(&p.0) != Ordering::Greater
        && p.0.cmp(max_x) != Ordering::Greater
        && min_y.cmp(&p.1) != Ordering::Greater
        && p.1.cmp(max_y) != Ordering::Greater
}

/// Whether the two closed segments `[a, b]` and `[c, d]` intersect at any point — exact, via
/// the orientation predicate with the collinear-overlap case handled explicitly.
fn segments_intersect<B: Backend>(
    a: &(Rat<B>, Rat<B>),
    b: &(Rat<B>, Rat<B>),
    c: &(Rat<B>, Rat<B>),
    d: &(Rat<B>, Rat<B>),
) -> bool {
    let d1 = orient(c, d, a);
    let d2 = orient(c, d, b);
    let d3 = orient(a, b, c);
    let d4 = orient(a, b, d);
    // Proper crossing: a, b straddle line(c,d) and c, d straddle line(a,b).
    if ((d1 > 0 && d2 < 0) || (d1 < 0 && d2 > 0)) && ((d3 > 0 && d4 < 0) || (d3 < 0 && d4 > 0)) {
        return true;
    }
    // Collinear / touching cases: an endpoint of one lies on the other segment.
    (d1 == 0 && on_segment(c, d, a))
        || (d2 == 0 && on_segment(c, d, b))
        || (d3 == 0 && on_segment(a, b, c))
        || (d4 == 0 && on_segment(a, b, d))
}

/// MITER-OUT (spec §8.5): the clean-miter branch's output postcondition over the ledger.
///
/// Runs, short-circuiting to the first fault:
/// - **EDGE-REG** — [`edge_reg`] on each edge's supplied certificate; only a `Pass` (cleared
///   `|e′|² ≥ m_e > 0`) admits the edge. This is the reuse point (spec §8.5).
/// - **EDGE-EMB** — injectivity, *discharged per edge for free by EDGE-REG*: a degree-1 edge
///   with `|e′|² > 0` is an affine immersion, hence globally injective (spec §8.5, "CLIP-W's
///   per-cell graph certificate discharges EDGE-EMB per cell for free"). No separate check.
/// - **CYCLE** — the inventory closes head-to-tail (`edge[k].end == edge[k+1].start`).
/// - **EDGE-EDGE** — non-adjacent edges are disjoint; adjacent edges meet only at their
///   shared cycle vertex (the boundary is simple).
/// - **VERTEX-ISOLATION** — the cycle vertices are pairwise distinct beyond the chaining (no
///   pinch), so `V_cand = V_∂` are isolated classes.
/// - **EDGE-COVERAGE** — every edge is two-sided (its occupancy is boundary-boundary).
///
/// Total: `Verified(`[`MiterOutWitness`]`)` or `Refuted(`[`MiterOutFault`]`)`.
pub fn miter_out<B: Backend>(
    cert: &MiterOutCert<B>,
) -> Verdict<MiterOutWitness<B>, MiterOutFault, ()> {
    let edges = &cert.ledger.edges;
    let n = edges.len();
    if n == 0 {
        return Verdict::Refuted(MiterOutFault::Empty);
    }
    if cert.edge_regs.len() != n {
        return Verdict::Refuted(MiterOutFault::RegCountMismatch);
    }
    // EDGE-REG (⇒ EDGE-EMB per edge for free) — a Pass is the only admitting verdict.
    let mut edge_margins = Vec::with_capacity(n);
    for (at, reg) in cert.edge_regs.iter().enumerate() {
        match edge_reg(reg) {
            EdgeReg::Pass(m) => edge_margins.push(m),
            _ => return Verdict::Refuted(MiterOutFault::EdgeReg { at }),
        }
    }
    // CYCLE: head-to-tail closure.
    for k in 0..n {
        if edges[k].end != edges[(k + 1) % n].start {
            return Verdict::Refuted(MiterOutFault::OpenCycle { at: k });
        }
    }
    // EDGE-COVERAGE: two-sided (boundary-boundary occupancy).
    for (at, e) in edges.iter().enumerate() {
        if !e.occupancy.is_boundary_boundary() {
            return Verdict::Refuted(MiterOutFault::Coverage { at });
        }
    }
    // VERTEX-ISOLATION: the cycle's start vertices are pairwise distinct (a repeat is a
    // pinch — the chaining already ties end[k] to start[k+1], so equal starts mean a
    // geometric collision, not the chain).
    for i in 0..n {
        for j in (i + 1)..n {
            if edges[i].start == edges[j].start {
                return Verdict::Refuted(MiterOutFault::VertexCollision { at: j });
            }
        }
    }
    // EDGE-EDGE: non-adjacent edges disjoint; adjacent share only their cycle vertex.
    for i in 0..n {
        for j in (i + 1)..n {
            let adjacent = j == i + 1 || (i == 0 && j == n - 1);
            if !segments_intersect(
                &edges[i].start,
                &edges[i].end,
                &edges[j].start,
                &edges[j].end,
            ) {
                continue;
            }
            // They intersect — legal only if adjacent and only at the shared vertex, which
            // CYCLE already established. Any intersection between non-adjacent edges, or a
            // second contact between adjacent ones, is a non-simple boundary.
            if !adjacent {
                return Verdict::Refuted(MiterOutFault::EdgeEdge { i, j });
            }
            // Adjacent: the only shared point must be the chaining vertex. If either edge's
            // far endpoint also lands on the other, it is a collinear overlap — reject.
            let (shared, far_i, far_j) = if j == i + 1 {
                (&edges[j].start, &edges[i].start, &edges[j].end)
            } else {
                // i == 0, j == n-1: edge[j].end == edge[0].start (the wrap vertex).
                (&edges[i].start, &edges[i].end, &edges[j].start)
            };
            if on_segment(&edges[j].start, &edges[j].end, far_i) && far_i != shared
                || on_segment(&edges[i].start, &edges[i].end, far_j) && far_j != shared
            {
                return Verdict::Refuted(MiterOutFault::EdgeEdge { i, j });
            }
        }
    }
    Verdict::Verified(MiterOutWitness { edge_margins })
}

/// Build the EDGE-REG certificate for a **degree-1** ledger edge: `e(σ) = start + (σ −
/// σ_lo)·D` has constant `e′ = D / (σ_hi − σ_lo)`... — the searcher supplies `|e′|²` as a
/// constant. This helper assembles the [`EdgeRegCert`] for a constant squared speed
/// `speed_sq > m` on `[σ_lo, σ_hi]`, with the (root-free) Sturm chains a constant needs.
///
/// It lives here so both the checker's tests and the `closure` searcher mint the reuse
/// certificate the same way; `speed_sq` and `m` are exact rationals with `speed_sq > m > 0`.
pub fn degree1_edge_reg_cert<B: Backend>(
    speed_sq: Rat<B>,
    m: Rat<B>,
    span: Interval<B>,
) -> EdgeRegCert<B> {
    let num = Poly::constant(speed_sq);
    let den = Poly::constant(Rat::from_i128(1));
    let r = num.sub(&den.scale(&m));
    EdgeRegCert {
        speed_sq: crate::certify1d::RegCert {
            den_chain: SturmChain::new(&den),
            res_chain: SturmChain::new(&r),
            num,
            den,
            m: MarginSq(m),
            span,
        },
        failure: None,
    }
}

// ============================================================================
// Transverse-rational MITER-FIT — the curved `φ_J` correspondence (spec §5.3)
// ============================================================================
//
// The conic / cone-flank pass the degree-1 [`miter_fit`] defers (module header): where
// CLIP-W holds each cut face is ruled in Π, `F_i(σ,μ) = P_i(σ) + μ·D_i(σ)`, and the two
// rulings meet the crease line `L` at a **genuinely rational** `ℓ_i(σ)`. Two rulings
// *correspond* when `ℓ_A(σ_A) = ℓ_B(σ_B)`; the correspondence `φ_J` is implicit as the
// bivariate `R(σ_A,σ_B) = 0`. On `{R=0}` position identity is free (paired rulings share
// their L-point), so the clean-miter conditions are the **carrier identity** `D_A ∥ D_B`
// and the **extent identities** `E_{A,±} = E_{B,π(±)}`, each vanishing on `{R=0}` — certified
// by an **exact cofactor** `X == R·Q` (`X = R·Q ⇒ X ≡ 0 on {R=0}`, an exact implication;
// the resultant⇔common-root theorem is the trusted foundation, cited/Lean, out of Kani per
// vv-guide §5). Watertightness does not hinge on this (a non-coincident cut is a valid
// exposed LEDGE, spec §5.3) — it is the disjoint clean-miter alternative to the LEDGE branch.

/// Mint the order sign `ε_φ` of a transverse correspondence from the two flanks'
/// crease-line-coordinate **slope signs** `sa = sign(ℓ_A′)`, `sb = sign(ℓ_B′)` on their
/// supports. `φ_J` tracks `σ_B` so that `ℓ_B(σ_B) = ℓ_A(σ_A)`, so it preserves order iff
/// `ℓ_A` and `ℓ_B` run the **same** direction. `None` if either slope is zero (a
/// non-monotone `ℓ_i` — the parallel regime, routed to LEDGE). Backend-free and `const` so
/// the ★ soundness property runs on `i128` under Kani (`eps_from_slopes_is_slope_agreement`),
/// the transverse analogue of [`eps_from_cmp`].
pub const fn eps_from_slopes(sa: i8, sb: i8) -> Option<OrderSign> {
    if sa == 0 || sb == 0 {
        None
    } else if (sa > 0) == (sb > 0) {
        Some(OrderSign::Preserving)
    } else {
        Some(OrderSign::Reversing)
    }
}

/// The **relevant-branch refinement** of the correspondence (spec §5.3, the per-CLIP-DOM-cell
/// identities). When `ℓ_i` is degree ≥ 2 (a cone — rotating rulings), the full correspondence
/// `R` **factors** into the real branch (the monotone `φ_J` over the support) and spurious
/// branches (a shared crease-line point reached by *different* rulings). The searcher supplies
/// the real branch `r_phi` and a cofactor `r_cofactor` witnessing `R = r_phi · r_cofactor`; the
/// checker then discharges the carrier/extent identities on `{r_phi = 0}` instead of the full
/// `{R = 0}`. The cylinder (affine `ℓ`, `R` irreducible) needs no branch — pass `None`.
#[derive(Clone, Debug)]
pub struct TransverseBranch<B: Backend = Bignum> {
    /// The relevant correspondence branch `R_φ` (single-valued over the support: degree 1 in σ_B).
    pub r_phi: Biv<B>,
    /// The cofactor `C` witnessing `R = R_φ · C` (verified by exact multiplication, not division).
    pub r_cofactor: Biv<B>,
}

/// A transverse-rational MITER-FIT certificate (spec §5.3): the two flanks' cut rulings in
/// Π with genuinely-rational crease-line coordinates `ℓ_i(σ)`, their ruling directions
/// `D_i(σ)` (cleared to polynomial components, since parallelism is scale-invariant), the
/// cut-segment extent endpoints `E_{i,±}(σ)` (a coordinate along the shared line), the
/// endpoint permutation `π`, the supports, and the searcher-supplied **cofactors** the
/// checker multiplies against the correspondence (its relevant branch, if it factors) to
/// discharge the identities.
pub struct TransverseMiterFitCert<B: Backend = Bignum> {
    /// Flank A's crease-line coordinate `ℓ_A(σ_A)`.
    pub ell_a: RatFunc<B>,
    /// Flank B's crease-line coordinate `ℓ_B(σ_B)`.
    pub ell_b: RatFunc<B>,
    /// Flank A's cut-ruling direction `D_A(σ_A)` in Π `(dx, dy)`, cleared to polynomials.
    pub dir_a: [Poly<B>; 2],
    /// Flank B's cut-ruling direction `D_B(σ_B)`.
    pub dir_b: [Poly<B>; 2],
    /// Flank A's cut-segment extent endpoints `E_{A,−}, E_{A,+}` along the shared line.
    pub ext_a: [RatFunc<B>; 2],
    /// Flank B's extent endpoints `E_{B,−}, E_{B,+}`.
    pub ext_b: [RatFunc<B>; 2],
    /// The endpoint permutation `π`: `false` ⇒ `(−↔−, +↔+)`, `true` ⇒ `(−↔+, +↔−)`.
    pub perm: bool,
    /// Flank A's σ-support.
    pub sigma_a: Interval<B>,
    /// Flank B's σ-support.
    pub sigma_b: Interval<B>,
    /// The carrier cofactor `Q_c`: the checker requires `X_carrier == R·Q_c`.
    pub carrier_cofactor: Biv<B>,
    /// The two extent cofactors, in `(−, +)` order (matched to B by `perm`).
    pub extent_cofactors: [Biv<B>; 2],
    /// The searcher's claimed order sign `ε_φ`.
    pub claimed: OrderSign,
    /// The relevant-branch refinement when the correspondence factors (a cone); `None` uses
    /// the full `R` (the cylinder / irreducible case).
    pub branch: Option<TransverseBranch<B>>,
}

/// The evidence a [`miter_fit_transverse`] `Verified` carries: the geometry-minted order
/// sign `ε_φ` and the endpoint permutation `π` the extents were matched under.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransverseMiterFit {
    /// The order sign of `φ_J`, minted from the two `ℓ_i` slope signs.
    pub eps_phi: OrderSign,
    /// The endpoint permutation the extent identities were discharged under.
    pub perm: bool,
}

/// Why transverse MITER-FIT refused — the miter is not a clean coincident cut, so the
/// closure falls back to LEDGE (or the input is malformed). Located by flank / endpoint
/// index only (the offending geometry is in the [`TransverseMiterFitCert`] the caller holds),
/// so this carries no backend-typed payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransverseMiterFault {
    /// A cut edge has an empty or reversed support (`σ_lo ≥ σ_hi`).
    EmptySupport {
        /// `false` for flank A, `true` for flank B.
        flank_b: bool,
    },
    /// `ℓ_i(σ) = e_i(σ)·m̂` is not strictly monotone on the support (its derivative has a
    /// zero or a sign change there) — a constant `ℓ_i` is the *parallel regime* (cut lines
    /// `∥ L`), out of the transverse slice; routed to LEDGE (spec §5.3).
    NonMonotone {
        /// `false` for flank A, `true` for flank B.
        flank_b: bool,
    },
    /// `ℓ_A` and `ℓ_B` are the same function of independent variables (`R ≡ 0`) — there is
    /// no transverse pairing to condition identities on.
    DegenerateCorrespondence,
    /// The supplied branch is not a factor of the correspondence: `R_φ · C ≠ R`.
    BranchNotAFactor,
    /// The supplied branch is not single-valued over the support (not degree 1 in σ_B), so it
    /// is not a `φ_J` graph.
    BranchNotSingleValued,
    /// The supplied branch does not connect the σ-supports at the `ε_φ`-paired corners — it is
    /// a spurious branch, not the fold's `φ_J`.
    BranchMissesSupport,
    /// The carrier identity fails: `D_A × D_B` does not vanish on `{R=0}` (the supplied
    /// cofactor does not reproduce it) — the two rulings are not the same line, so no clean
    /// miter (curvature-order disagreement; planar-vs-cylindrical lands here).
    CarrierMismatch,
    /// An extent identity fails: `E_{A,±} = E_{B,π(±)}` does not hold on `{R=0}` — the
    /// segments differ though the carriers coincide (discrete event agreement is not segment
    /// coincidence, spec §5.3). Reports the endpoint index (`0 = −`, `1 = +`).
    ExtentMismatch {
        /// The offending extent endpoint index.
        at: usize,
    },
    /// The geometry-minted `ε_φ` disagrees with the searcher's claimed order sign.
    OrderMismatch {
        /// The order sign the geometry witnesses.
        minted: OrderSign,
    },
}

/// The numerator of `a(σ_A) − b(σ_B)` as a [`Biv`] over `(σ_A, σ_B) = (x, y)`:
/// `num_a(x)·den_b(y) − num_b(y)·den_a(x)`. It is zero exactly where `a(σ_A) = b(σ_B)`
/// (denominators nonzero) — the correspondence `R` when `a, b = ℓ_A, ℓ_B`, and the
/// per-endpoint extent residual otherwise.
fn cross_ratfunc<B: Backend>(a: &RatFunc<B>, b: &RatFunc<B>) -> Biv<B> {
    Biv::from_x_poly(a.num())
        .mul(&Biv::from_y_poly(b.den()))
        .sub(&Biv::from_y_poly(b.num()).mul(&Biv::from_x_poly(a.den())))
}

/// The numerator of `ℓ′ = (num′·den − num·den′)/den²` — its sign is `ℓ`'s slope sign
/// (`den² > 0`), zero exactly where `ℓ` is stationary.
fn slope_num<B: Backend>(ell: &RatFunc<B>) -> Poly<B> {
    ell.num()
        .derivative()
        .mul(ell.den())
        .sub(&ell.num().mul(&ell.den().derivative()))
}

/// The constant slope sign of `ell` on `[lo, hi]` if it is **strictly monotone** there:
/// `ℓ′`'s numerator has no root in `(lo, hi]` (Sturm) and is nonzero at `lo`, so its sign
/// is constant across the closed support. `None` if `ell` stalls (a stationary point or a
/// sign change) — the non-transverse / parallel regime.
fn monotone_slope_sign<B: Backend>(ell: &RatFunc<B>, lo: &Rat<B>, hi: &Rat<B>) -> Option<i8> {
    let n = slope_num(ell);
    let s_lo = n.eval(lo).sign();
    if s_lo == 0 {
        return None; // `lo` is stationary (or `ℓ′ ≡ 0`)
    }
    if SturmChain::new(&n).count_in(lo, hi) != 0 {
        return None; // a stationary point in `(lo, hi]` — not strictly monotone
    }
    Some(s_lo)
}

/// The constant slope sign of `ell` on `[lo, α]` — `lo` rational, `α` an **algebraic** upper
/// bound with `lo < α` — if `ell` is strictly monotone there: its slope numerator is nonzero
/// at `lo` and has no root in `(lo, α]` (counted exactly via [`AlgReal::count_roots_upto`]).
/// `None` if `ell` stalls. The algebraic-support analogue of [`monotone_slope_sign`] — the
/// transverse miter's monotonicity certificate for a **cone** flank whose cut-face σ-range
/// ends at an algebraic event (e.g. the cut-exit σ where the cut curve leaves a rational box
/// face). This is the first downstream use of `lattice::AlgReal`; the transverse identities
/// themselves are support-independent, so only the monotonicity domain reaches for it.
///
/// ```
/// use certify_core::miter::strictly_monotone_upto_alg;
/// use lattice::{AlgReal, Bignum, Poly, Rat, RatFunc};
/// use core::cmp::Ordering;
///
/// let p = |cs: &[i128]| Poly::<Bignum>::from_coeffs(cs.iter().map(|&c| Rat::from_i128(c)).collect());
/// // α = √2, the positive root of x² − 2.
/// let sqrt2 = AlgReal::isolate_roots(&p(&[-2, 0, 1]))
///     .into_iter()
///     .find(|r| r.cmp_rat(&Rat::from_i128(0)) == Ordering::Greater)
///     .expect("√2");
/// // ℓ = σ² is strictly increasing on [1, √2] (slope 2σ > 0 there).
/// let ell = RatFunc::<Bignum>::new(p(&[0, 0, 1]), p(&[1]));
/// assert_eq!(strictly_monotone_upto_alg(&ell, &Rat::from_i128(1), &sqrt2), Some(1));
/// // but it stalls at σ = 0, so it is not strictly monotone on [0, √2].
/// assert_eq!(strictly_monotone_upto_alg(&ell, &Rat::from_i128(0), &sqrt2), None);
/// ```
pub fn strictly_monotone_upto_alg<B: Backend>(
    ell: &RatFunc<B>,
    lo: &Rat<B>,
    hi: &AlgReal<B>,
) -> Option<i8> {
    let n = slope_num(ell);
    let s_lo = n.eval(lo).sign();
    if s_lo == 0 {
        return None; // `lo` is stationary (or ℓ′ ≡ 0)
    }
    if hi.count_roots_upto(&n, lo) != 0 {
        return None; // a stationary point in `(lo, α]` — not strictly monotone
    }
    Some(s_lo)
}

/// Transverse-rational MITER-FIT (spec §5.3): certify that two flanks' rationally-ruled cut
/// faces **coincide** in Π along the correspondence `φ_J`, and mint the order sign `ε_φ`.
///
/// The checks, short-circuiting to the first fault:
/// 1. **finite supports** — `σ_lo < σ_hi` on both flanks.
/// 2. **`ℓ_i` monotone** — each crease-line coordinate is strictly monotone on its support
///    (Sturm on `ℓ_i′`'s numerator); a constant `ℓ_i` is the parallel regime → LEDGE.
/// 3. **correspondence** — form `R(σ_A,σ_B)` from `ℓ_A = ℓ_B` (the checker builds it, never
///    trusting a supplied `R`); `R ≡ 0` is [`DegenerateCorrespondence`](TransverseMiterFault::DegenerateCorrespondence).
/// 4. **carrier identity** — `X_carrier = (D_A × D_B)` vanishes on `{R=0}`, verified as the
///    exact bivariate equality `X_carrier == R · Q_carrier`.
/// 5. **extent identities** — for each endpoint, `E_{A,±} = E_{B,π(±)}` on `{R=0}`, the same
///    `X == R·Q` cofactor pattern.
/// 6. **`ε_φ`** — minted from the two slope signs ([`eps_from_slopes`]); the searcher's
///    [`claimed`](TransverseMiterFitCert::claimed) sign must match.
///
/// Total: `Verified(`[`TransverseMiterFit`]`)` or `Refuted(`[`TransverseMiterFault`]`)`.
/// Wires `lattice`'s [`Biv`] cofactor algebra (the first downstream use of the bivariate
/// resultant machinery). **No floating point, no OCCT** — the certificate is an exact
/// polynomial identity.
///
/// ```
/// use certify_core::miter::{miter_fit_transverse, OrderSign, TransverseMiterFault};
/// use certify_core::Verdict;
/// use lattice::{Bignum, Biv, Interval, Poly, Rat, RatFunc};
///
/// let p = |cs: &[i128]| Poly::<Bignum>::from_coeffs(cs.iter().map(|&c| Rat::from_i128(c)).collect());
/// let rf = |n: &[i128], d: &[i128]| RatFunc::<Bignum>::new(p(n), p(d));
/// let iv = |lo: i128, hi: i128| Interval { lo: Rat::from_i128(lo), hi: Rat::from_i128(hi) };
/// // Two flanks whose rulings rotate the *opposite* way at curvature order fail carrier
/// // identity: D_A = (1, σ), D_B = (1, 2σ) over the correspondence σ_A = σ_B (ℓ_i = σ).
/// let cert = certify_core::miter::TransverseMiterFitCert {
///     ell_a: rf(&[0, 1], &[1]), ell_b: rf(&[0, 1], &[1]),
///     dir_a: [p(&[1]), p(&[0, 1])], dir_b: [p(&[1]), p(&[0, 2])],
///     ext_a: [rf(&[0, 1], &[1]), rf(&[0, 1], &[1])],
///     ext_b: [rf(&[0, 1], &[1]), rf(&[0, 1], &[1])],
///     perm: false, sigma_a: iv(0, 1), sigma_b: iv(0, 1),
///     carrier_cofactor: Biv::zero(), extent_cofactors: [Biv::zero(), Biv::zero()],
///     claimed: OrderSign::Preserving, branch: None,
/// };
/// assert!(matches!(miter_fit_transverse(&cert), Verdict::Refuted(TransverseMiterFault::CarrierMismatch)));
/// ```
pub fn miter_fit_transverse<B: Backend>(
    cert: &TransverseMiterFitCert<B>,
) -> Verdict<TransverseMiterFit, TransverseMiterFault, ()> {
    // (1) finite supports.
    if cert.sigma_a.lo.cmp(&cert.sigma_a.hi) != Ordering::Less {
        return Verdict::Refuted(TransverseMiterFault::EmptySupport { flank_b: false });
    }
    if cert.sigma_b.lo.cmp(&cert.sigma_b.hi) != Ordering::Less {
        return Verdict::Refuted(TransverseMiterFault::EmptySupport { flank_b: true });
    }
    // (2) ℓ_i strictly monotone (transverse regime; a constant ℓ_i routes to LEDGE).
    let sa = match monotone_slope_sign(&cert.ell_a, &cert.sigma_a.lo, &cert.sigma_a.hi) {
        Some(s) => s,
        None => return Verdict::Refuted(TransverseMiterFault::NonMonotone { flank_b: false }),
    };
    let sb = match monotone_slope_sign(&cert.ell_b, &cert.sigma_b.lo, &cert.sigma_b.hi) {
        Some(s) => s,
        None => return Verdict::Refuted(TransverseMiterFault::NonMonotone { flank_b: true }),
    };
    // (3) the correspondence R = 0 from ℓ_A = ℓ_B (formed here, not trusted from the caller).
    let r = cross_ratfunc(&cert.ell_a, &cert.ell_b);
    if r.is_zero() {
        return Verdict::Refuted(TransverseMiterFault::DegenerateCorrespondence);
    }
    // (3.5) relevant branch: for a cone `R` factors, and the identities hold only on the real
    // branch `R_φ` (the monotone φ_J), not the spurious branches. Verify the searcher's `R_φ`
    // is a genuine factor (`R_φ·C == R`, by multiplication — the checker never divides), is
    // single-valued over the support (degree 1 in σ_B), and connects the supports at the
    // `ε_φ`-paired corners (which rejects the spurious branch); then discharge on `{R_φ = 0}`.
    let r_eff = match &cert.branch {
        None => r.clone(),
        Some(br) => {
            if br.r_phi.mul(&br.r_cofactor) != r {
                return Verdict::Refuted(TransverseMiterFault::BranchNotAFactor);
            }
            if br.r_phi.rows().iter().filter_map(|p| p.degree()).max() != Some(1) {
                return Verdict::Refuted(TransverseMiterFault::BranchNotSingleValued);
            }
            let (b_lo, b_hi) = match cert.claimed {
                OrderSign::Preserving => (&cert.sigma_b.lo, &cert.sigma_b.hi),
                OrderSign::Reversing => (&cert.sigma_b.hi, &cert.sigma_b.lo),
            };
            if !br.r_phi.eval(&cert.sigma_a.lo, b_lo).is_zero()
                || !br.r_phi.eval(&cert.sigma_a.hi, b_hi).is_zero()
            {
                return Verdict::Refuted(TransverseMiterFault::BranchMissesSupport);
            }
            br.r_phi.clone()
        }
    };
    // (4) carrier identity: X_carrier = D_A × D_B == R_eff · Q_carrier.
    let x_carrier = Biv::from_x_poly(&cert.dir_a[0])
        .mul(&Biv::from_y_poly(&cert.dir_b[1]))
        .sub(&Biv::from_x_poly(&cert.dir_a[1]).mul(&Biv::from_y_poly(&cert.dir_b[0])));
    if x_carrier != r_eff.mul(&cert.carrier_cofactor) {
        return Verdict::Refuted(TransverseMiterFault::CarrierMismatch);
    }
    // (5) extent identities: E_{A,±}(σ_A) = E_{B,π(±)}(σ_B) on {R_eff = 0}.
    let mut k = 0;
    while k < 2 {
        let pk = if cert.perm { 1 - k } else { k };
        let x_ext = cross_ratfunc(&cert.ext_a[k], &cert.ext_b[pk]);
        if x_ext != r_eff.mul(&cert.extent_cofactors[k]) {
            return Verdict::Refuted(TransverseMiterFault::ExtentMismatch { at: k });
        }
        k += 1;
    }
    // (6) ε_φ from the two slope signs; the searcher's claim must match.
    let minted = match eps_from_slopes(sa, sb) {
        Some(s) => s,
        // Unreachable — sa, sb are nonzero (checked at (2)) — but reported, not panicked.
        None => return Verdict::Refuted(TransverseMiterFault::NonMonotone { flank_b: false }),
    };
    if minted != cert.claimed {
        return Verdict::Refuted(TransverseMiterFault::OrderMismatch { minted });
    }
    Verdict::Verified(TransverseMiterFit {
        eps_phi: minted,
        perm: cert.perm,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    type Q = Rat<Bignum>;
    fn p(x: i128, y: i128) -> (Q, Q) {
        (Q::from_i128(x), Q::from_i128(y))
    }
    fn span(lo: i128, hi: i128) -> Interval<Bignum> {
        Interval {
            lo: Q::from_i128(lo),
            hi: Q::from_i128(hi),
        }
    }
    /// The boundary-boundary occupancy every clean-miter edge carries.
    fn bb() -> Occupancy {
        Occupancy {
            a_l: true,
            a_r: false,
            b_l: false,
            b_r: true,
            frame: false,
        }
    }

    #[test]
    fn eps_phi_signs_the_cube_fossil_by_endpoints() {
        // σ_B = σ_A³ over [0,1]: φ_J(0)=0, φ_J(1)=1 — monotone, positive endpoint order,
        // derivative 3σ² = 0 at σ=0. The endpoint comparison mints Preserving; the
        // derivative-at-0 sign is 0 (no sign) — the fossil the definition must not consult.
        assert_eq!(
            eps_phi(&Q::from_i128(0), &Q::from_i128(1)),
            Some(OrderSign::Preserving)
        );
        // The derivative of σ³ at 0 is 0 — sign 0, a fossil, never used by `eps_phi`.
        let deriv_at_0 = Q::from_i128(3).mul(&Q::from_i128(0)).mul(&Q::from_i128(0));
        assert_eq!(deriv_at_0.sign(), 0);
    }

    #[test]
    fn eps_phi_signs_a_reversed_correspondence() {
        assert_eq!(
            eps_phi(&Q::from_i128(7), &Q::from_i128(2)),
            Some(OrderSign::Reversing)
        );
        // Collapsed endpoints are not a correspondence.
        assert_eq!(eps_phi(&Q::from_i128(3), &Q::from_i128(3)), None);
    }

    fn edge(start: (Q, Q), end: (Q, Q), sl: i128, sh: i128) -> CutEnds<Bignum> {
        CutEnds {
            start,
            end,
            sigma_lo: Q::from_i128(sl),
            sigma_hi: Q::from_i128(sh),
        }
    }

    #[test]
    fn a_coincident_miter_fits_order_preserving() {
        let cert = MiterFitCert {
            crease_dir: p(1, 0),
            a: edge(p(0, 0), p(4, 0), 0, 4),
            b: edge(p(0, 0), p(4, 0), 0, 4),
            claimed: OrderSign::Preserving,
        };
        match miter_fit(&cert) {
            Verdict::Verified(fit) => {
                assert_eq!(fit.eps_phi, OrderSign::Preserving);
                assert_eq!(fit.start, p(0, 0));
            }
            other => panic!("coincident miter must fit: {other:?}"),
        }
    }

    #[test]
    fn a_reversed_pairing_claimed_preserving_is_refused() {
        // B traces the same segment but with reversed endpoints — the true ε_φ is Reversing.
        // The searcher claiming Preserving is caught by the geometry-minted sign.
        let cert = MiterFitCert {
            crease_dir: p(1, 0),
            a: edge(p(0, 0), p(4, 0), 0, 4),
            b: edge(p(4, 0), p(0, 0), 0, 4),
            claimed: OrderSign::Preserving,
        };
        assert!(matches!(
            miter_fit(&cert),
            Verdict::Refuted(MiterFault::OrderMismatch {
                minted: OrderSign::Reversing
            })
        ));
        // Claiming the correct Reversing sign fits.
        let ok = MiterFitCert {
            claimed: OrderSign::Reversing,
            ..cert
        };
        assert!(matches!(miter_fit(&ok), Verdict::Verified(_)));
    }

    #[test]
    fn a_non_collinear_pair_is_not_a_miter() {
        // B's cut edge crosses A's — different carriers, so no clean miter (⇒ LEDGE).
        let cert = MiterFitCert {
            crease_dir: p(1, 0),
            a: edge(p(0, 0), p(4, 0), 0, 4),
            b: edge(p(0, 0), p(2, 2), 0, 4),
            claimed: OrderSign::Preserving,
        };
        assert!(matches!(
            miter_fit(&cert),
            Verdict::Refuted(MiterFault::CarrierMismatch { .. })
        ));
    }

    #[test]
    fn a_crease_parallel_edge_is_out_of_the_transverse_slice() {
        // A's edge runs along the crease direction m̂ = x̂: ℓ_A has zero slope (parallel
        // regime, ℓ undefined) ⇒ CreaseParallel, routed to LEDGE.
        let cert = MiterFitCert {
            crease_dir: p(0, 1),
            a: edge(p(0, 0), p(4, 0), 0, 4),
            b: edge(p(0, 0), p(4, 0), 0, 4),
            claimed: OrderSign::Preserving,
        };
        assert!(matches!(
            miter_fit(&cert),
            Verdict::Refuted(MiterFault::CreaseParallel { flank_b: false })
        ));
    }

    /// A closed square miter cap outline, each edge a coincident A≡B pairing — the ledger
    /// MITER-OUT audits. Returns the four fits in head-to-tail order.
    fn square_fits() -> vec::Vec<MiterFit<Bignum>> {
        let corners = [p(0, 0), p(2, 0), p(2, 2), p(0, 2)];
        let mut fits = vec::Vec::new();
        for k in 0..4 {
            let a = corners[k].clone();
            let b = corners[(k + 1) % 4].clone();
            let dir = if a.0 == b.0 { p(0, 1) } else { p(1, 0) };
            let cert = MiterFitCert {
                crease_dir: dir,
                a: edge(a.clone(), b.clone(), 0, 2),
                b: edge(a, b, 0, 2),
                claimed: OrderSign::Preserving,
            };
            match miter_fit(&cert) {
                Verdict::Verified(fit) => fits.push(fit),
                other => panic!("square edge must fit: {other:?}"),
            }
        }
        fits
    }

    fn out_cert(fits: &[MiterFit<Bignum>]) -> MiterOutCert<Bignum> {
        let occ = vec![bb(); fits.len()];
        let ledger = miter_edge_ledger(fits, &occ).expect("lengths match");
        let edge_regs = fits
            .iter()
            .map(|fit| {
                // |e′|² for e(σ) = start + (σ−0)/2·(end−start): D = (end−start)/2, so
                // |e′|² = |end−start|²/4 = 4/4 = 1 for a length-2 axis edge; margin 1/2.
                let d = sub2(&fit.end, &fit.start);
                let len_sq = d.0.mul(&d.0).add(&d.1.mul(&d.1)); // = 4
                let speed_sq = len_sq.mul(&Q::new(1, 4)); // /(σ_hi−σ_lo)² = /4
                degree1_edge_reg_cert(speed_sq, Q::new(1, 2), span(0, 2))
            })
            .collect();
        MiterOutCert { ledger, edge_regs }
    }

    #[test]
    fn the_square_miter_ledger_passes_miter_out() {
        let fits = square_fits();
        match miter_out(&out_cert(&fits)) {
            Verdict::Verified(w) => assert_eq!(w.edge_margins.len(), 4),
            other => panic!("a simple closed miter cycle must pass MITER-OUT: {other:?}"),
        }
    }

    #[test]
    fn an_open_cycle_fails_miter_out() {
        let mut fits = square_fits();
        // Break the chain: move the last edge's end off the first edge's start.
        fits[3].end = p(9, 9);
        assert!(matches!(
            miter_out(&out_cert(&fits)),
            Verdict::Refuted(MiterOutFault::OpenCycle { .. })
        ));
    }

    #[test]
    fn a_one_sided_edge_fails_edge_coverage() {
        let fits = square_fits();
        let mut occ = vec![bb(); 4];
        occ[1] = Occupancy {
            a_l: true,
            a_r: true, // A_L == A_R: material does not change across the edge — one-sided.
            b_l: false,
            b_r: true,
            frame: false,
        };
        let ledger = miter_edge_ledger(&fits, &occ).unwrap();
        let edge_regs = out_cert(&fits).edge_regs;
        assert!(matches!(
            miter_out(&MiterOutCert { ledger, edge_regs }),
            Verdict::Refuted(MiterOutFault::Coverage { at: 1 })
        ));
    }

    #[test]
    fn a_degenerate_speed_fails_edge_reg() {
        let fits = square_fits();
        let occ = vec![bb(); 4];
        let ledger = miter_edge_ledger(&fits, &occ).unwrap();
        // A zero squared speed cannot clear any positive margin — EDGE-REG refuses.
        let edge_regs = (0..4)
            .map(|_| degree1_edge_reg_cert(Q::from_i128(0), Q::new(1, 2), span(0, 2)))
            .collect();
        assert!(matches!(
            miter_out(&MiterOutCert { ledger, edge_regs }),
            Verdict::Refuted(MiterOutFault::EdgeReg { .. })
        ));
    }

    // ---- transverse-rational MITER-FIT ------------------------------------

    fn pol(cs: &[i128]) -> Poly<Bignum> {
        Poly::from_coeffs(cs.iter().map(|&c| Q::from_i128(c)).collect())
    }
    fn rf(num: &[i128], den: &[i128]) -> RatFunc<Bignum> {
        RatFunc::new(pol(num), pol(den))
    }
    fn iv(lo: i128, hi: i128) -> Interval<Bignum> {
        Interval {
            lo: Q::from_i128(lo),
            hi: Q::from_i128(hi),
        }
    }
    /// A constant `Biv` cofactor `n/d`.
    fn bconst(n: i128, d: i128) -> Biv<Bignum> {
        Biv::from_x_poly(&Poly::constant(Q::new(n, d)))
    }

    /// A reflection-symmetric pair of **genuinely rational** flanks certifies as a clean
    /// transverse miter: `ℓ_i = 2σ/(1+σ)` (rational, monotone) ⇒ `R = 2(σ_A − σ_B)`; the
    /// carrier `D_i = (1, σ)` and the extents coincide on the correspondence via exact
    /// cofactors. This is the case the degree-1 `miter_fit` cannot represent (its `CutEnds`
    /// are affine), so it exercises the new resultant-conditioned path.
    #[test]
    fn a_symmetric_rational_pair_is_a_clean_transverse_miter() {
        let ell = || rf(&[0, 2], &[1, 1]); // 2σ/(1+σ)
        let cert = TransverseMiterFitCert {
            ell_a: ell(),
            ell_b: ell(),
            dir_a: [pol(&[1]), pol(&[0, 1])], // (1, σ)
            dir_b: [pol(&[1]), pol(&[0, 1])],
            ext_a: [rf(&[0, 1], &[1]), rf(&[1, 1], &[1])], // σ, σ+1
            ext_b: [rf(&[0, 1], &[1]), rf(&[1, 1], &[1])],
            perm: false,
            sigma_a: iv(0, 1),
            sigma_b: iv(0, 1),
            // R = 2(x−y); X_carrier = y−x = R·(−1/2); X_ext = x−y = R·(1/2).
            carrier_cofactor: bconst(-1, 2),
            extent_cofactors: [bconst(1, 2), bconst(1, 2)],
            claimed: OrderSign::Preserving,
            branch: None,
        };
        match miter_fit_transverse(&cert) {
            Verdict::Verified(fit) => {
                assert_eq!(fit.eps_phi, OrderSign::Preserving);
                assert!(!fit.perm);
            }
            other => panic!("the symmetric rational miter must fit: {other:?}"),
        }
    }

    /// Two flanks whose rulings rotate at different curvature order fail the carrier
    /// identity: `D_A = (1, σ)` vs `D_B = (1, 2σ)` agree to first order but not on `{R=0}`
    /// (`X_carrier = 2y − x`, nonzero at `x = y ≠ 0`), so no cofactor reproduces it —
    /// planar-vs-cylindrical routes to LEDGE.
    #[test]
    fn a_curvature_order_mismatch_fails_the_carrier_identity() {
        let cert = TransverseMiterFitCert {
            ell_a: rf(&[0, 1], &[1]),
            ell_b: rf(&[0, 1], &[1]),
            dir_a: [pol(&[1]), pol(&[0, 1])], // (1, σ)
            dir_b: [pol(&[1]), pol(&[0, 2])], // (1, 2σ)
            ext_a: [rf(&[0, 1], &[1]), rf(&[1, 1], &[1])],
            ext_b: [rf(&[0, 1], &[1]), rf(&[1, 1], &[1])],
            perm: false,
            sigma_a: iv(0, 1),
            sigma_b: iv(0, 1),
            carrier_cofactor: Biv::zero(), // no cofactor can work
            extent_cofactors: [bconst(1, 1), bconst(1, 1)],
            claimed: OrderSign::Preserving,
            branch: None,
        };
        assert!(matches!(
            miter_fit_transverse(&cert),
            Verdict::Refuted(TransverseMiterFault::CarrierMismatch)
        ));
    }

    /// The spec's stored extent counterexample: identical carriers and matching endpoint
    /// **events** do not prove segment coincidence. `E_{A,+} = 1` vs `E_{B,+} = 1 + σ(1−σ)`
    /// agree only at `σ ∈ {0, 1}` (the events), not on all of `{R=0}` — so the extent
    /// residual `y² − y` is not divisible by `R = x − y` and MITER-FIT refuses.
    #[test]
    fn matching_events_do_not_prove_segment_coincidence() {
        let cert = TransverseMiterFitCert {
            ell_a: rf(&[0, 1], &[1]),
            ell_b: rf(&[0, 1], &[1]),
            dir_a: [pol(&[1]), pol(&[0])], // (1, 0) — constant parallel carrier
            dir_b: [pol(&[1]), pol(&[0])],
            ext_a: [rf(&[0, 1], &[1]), rf(&[1], &[1])], // σ, 1
            ext_b: [rf(&[0, 1], &[1]), rf(&[1, 1, -1], &[1])], // σ, 1 + σ − σ²
            perm: false,
            sigma_a: iv(0, 1),
            sigma_b: iv(0, 1),
            carrier_cofactor: Biv::zero(), // X_carrier = 0 = R·0
            extent_cofactors: [bconst(1, 1), Biv::zero()],
            claimed: OrderSign::Preserving,
            branch: None,
        };
        assert!(matches!(
            miter_fit_transverse(&cert),
            Verdict::Refuted(TransverseMiterFault::ExtentMismatch { at: 1 })
        ));
    }

    /// A wrong carrier cofactor is caught by the exact bivariate equality (the clean-miter
    /// setup of the first test, but with `Q_carrier` replaced by zero).
    #[test]
    fn a_wrong_cofactor_is_refused() {
        let ell = || rf(&[0, 2], &[1, 1]);
        let cert = TransverseMiterFitCert {
            ell_a: ell(),
            ell_b: ell(),
            dir_a: [pol(&[1]), pol(&[0, 1])],
            dir_b: [pol(&[1]), pol(&[0, 1])],
            ext_a: [rf(&[0, 1], &[1]), rf(&[1, 1], &[1])],
            ext_b: [rf(&[0, 1], &[1]), rf(&[1, 1], &[1])],
            perm: false,
            sigma_a: iv(0, 1),
            sigma_b: iv(0, 1),
            carrier_cofactor: Biv::zero(), // should be −1/2
            extent_cofactors: [bconst(1, 2), bconst(1, 2)],
            claimed: OrderSign::Preserving,
            branch: None,
        };
        assert!(matches!(
            miter_fit_transverse(&cert),
            Verdict::Refuted(TransverseMiterFault::CarrierMismatch)
        ));
    }

    /// A constant `ℓ_i` (zero slope) is the parallel regime — out of the transverse slice,
    /// routed to LEDGE via `NonMonotone`.
    #[test]
    fn a_constant_crease_coordinate_is_the_parallel_regime() {
        let cert = TransverseMiterFitCert {
            ell_a: rf(&[5], &[1]), // constant 5 — stationary
            ell_b: rf(&[0, 1], &[1]),
            dir_a: [pol(&[1]), pol(&[0, 1])],
            dir_b: [pol(&[1]), pol(&[0, 1])],
            ext_a: [rf(&[0, 1], &[1]), rf(&[1, 1], &[1])],
            ext_b: [rf(&[0, 1], &[1]), rf(&[1, 1], &[1])],
            perm: false,
            sigma_a: iv(0, 1),
            sigma_b: iv(0, 1),
            carrier_cofactor: Biv::zero(),
            extent_cofactors: [Biv::zero(), Biv::zero()],
            claimed: OrderSign::Preserving,
            branch: None,
        };
        assert!(matches!(
            miter_fit_transverse(&cert),
            Verdict::Refuted(TransverseMiterFault::NonMonotone { flank_b: false })
        ));
    }

    /// The searcher's claimed order sign is checked against the geometry-minted one: the
    /// clean-miter pair mints `Preserving`, so a claimed `Reversing` is refused.
    #[test]
    fn a_wrong_claimed_order_sign_is_refused() {
        let ell = || rf(&[0, 2], &[1, 1]);
        let cert = TransverseMiterFitCert {
            ell_a: ell(),
            ell_b: ell(),
            dir_a: [pol(&[1]), pol(&[0, 1])],
            dir_b: [pol(&[1]), pol(&[0, 1])],
            ext_a: [rf(&[0, 1], &[1]), rf(&[1, 1], &[1])],
            ext_b: [rf(&[0, 1], &[1]), rf(&[1, 1], &[1])],
            perm: false,
            sigma_a: iv(0, 1),
            sigma_b: iv(0, 1),
            carrier_cofactor: bconst(-1, 2),
            extent_cofactors: [bconst(1, 2), bconst(1, 2)],
            claimed: OrderSign::Reversing, // geometry mints Preserving
            branch: None,
        };
        assert!(matches!(
            miter_fit_transverse(&cert),
            Verdict::Refuted(TransverseMiterFault::OrderMismatch {
                minted: OrderSign::Preserving
            })
        ));
    }

    /// The transverse monotonicity certificate generalizes to an **algebraic** support bound
    /// (the cone case) via `lattice::AlgReal` — `α = √2` here stands in for a cut-exit σ.
    #[test]
    fn transverse_monotonicity_over_an_algebraic_support_bound() {
        // α = √2, the positive root of x² − 2.
        let sqrt2 = AlgReal::isolate_roots(&pol(&[-2, 0, 1]))
            .into_iter()
            .find(|r| r.cmp_rat(&Q::from_i128(0)) == Ordering::Greater)
            .expect("√2");
        let ell_sq = rf(&[0, 0, 1], &[1]); // ℓ = σ²
        // strictly increasing on [1, √2]; stalls at σ = 0 on [0, √2].
        assert_eq!(
            strictly_monotone_upto_alg(&ell_sq, &Q::from_i128(1), &sqrt2),
            Some(1)
        );
        assert_eq!(
            strictly_monotone_upto_alg(&ell_sq, &Q::from_i128(0), &sqrt2),
            None
        );
        // ℓ = σ² − 2σ stalls at σ = 1 ∈ (0, √2] — caught inside the support.
        assert_eq!(
            strictly_monotone_upto_alg(&rf(&[0, -2, 1], &[1]), &Q::from_i128(0), &sqrt2),
            None
        );
    }

    /// The **adversarial** two-cone miter — the configuration the branch-refinement exists
    /// for. Two cones over the same base conic (the unit circle) from *different* apexes are
    /// the tangent-line cut families at `t = σ` and `t = 2σ`:
    /// `ℓ_A = (1+σ²)/(1−σ²)`, `ℓ_B = (1+4σ²)/(1−4σ²)`, `D_A = (2σ, σ²−1)`, `D_B = (4σ, 4σ²−1)`.
    /// `ℓ` is degree-2 rational, so `R = 2(σ_A−2σ_B)(σ_A+2σ_B)` **factors**; the carrier `X`
    /// vanishes on the real branch `σ_A = 2σ_B` (same tangent) but **not** on the spurious
    /// `σ_A = −2σ_B` (the *other* tangent through the shared L-point). So the **full `R` does
    /// not divide `X`** — CM.1's full-`R` check refuses — while the branch `R_φ = σ_A − 2σ_B`
    /// does, and the branch-aware certificate accepts it.
    #[test]
    fn a_two_cone_adversarial_miter_certifies_via_the_branch() {
        let ell_a = rf(&[1, 0, 1], &[1, 0, -1]);
        let ell_b = rf(&[1, 0, 4], &[1, 0, -4]);
        let dir_a = [pol(&[0, 2]), pol(&[-1, 0, 1])];
        let dir_b = [pol(&[0, 4]), pol(&[-1, 0, 4])];
        // Extents chosen to match on the branch: E_A(σ) = σ, E_B(σ) = 2σ (equal on σ_A = 2σ_B).
        let ext_a = [rf(&[0, 1], &[1]), rf(&[0, 1], &[1])];
        let ext_b = [rf(&[0, 2], &[1]), rf(&[0, 2], &[1])];

        let bx = |p: &Poly<Bignum>| Biv::from_x_poly(p);
        let by = |p: &Poly<Bignum>| Biv::from_y_poly(p);
        let r_full = bx(ell_a.num())
            .mul(&by(ell_b.den()))
            .sub(&by(ell_b.num()).mul(&bx(ell_a.den())));
        let r_phi = bx(&pol(&[0, 1])).sub(&by(&pol(&[0, 2]))); // σ_A − 2σ_B (the real branch)
        let r_cofactor = r_full.div_exact(&r_phi).expect("R_φ | R_full");
        let x_carrier = bx(&dir_a[0])
            .mul(&by(&dir_b[1]))
            .sub(&bx(&dir_a[1]).mul(&by(&dir_b[0])));
        // The crux: the full R does NOT divide X (the spurious branch breaks the carrier),
        // but the real branch R_φ does.
        assert!(
            x_carrier.div_exact(&r_full).is_none(),
            "the full R does not divide X (carrier fails on the spurious branch)"
        );
        let q_carrier = x_carrier.div_exact(&r_phi).expect("R_φ | X_carrier");
        let one = Biv::from_x_poly(&pol(&[1])); // X_ext = σ_A − 2σ_B = R_φ ⇒ Q_ext = 1

        // Support σ_a = 2·σ_b (Preserving), inside the poles (|σ_b| < 1/2, |σ_a| < 1).
        let make = |branch| TransverseMiterFitCert {
            ell_a: ell_a.clone(),
            ell_b: ell_b.clone(),
            dir_a: [dir_a[0].clone(), dir_a[1].clone()],
            dir_b: [dir_b[0].clone(), dir_b[1].clone()],
            ext_a: [ext_a[0].clone(), ext_a[1].clone()],
            ext_b: [ext_b[0].clone(), ext_b[1].clone()],
            perm: false,
            sigma_a: Interval {
                lo: Q::new(1, 2),
                hi: Q::new(3, 4),
            },
            sigma_b: Interval {
                lo: Q::new(1, 4),
                hi: Q::new(3, 8),
            },
            carrier_cofactor: q_carrier.clone(),
            extent_cofactors: [one.clone(), one.clone()],
            claimed: OrderSign::Preserving,
            branch,
        };
        // With the relevant branch, the cone miter certifies.
        match miter_fit_transverse(&make(Some(TransverseBranch {
            r_phi: r_phi.clone(),
            r_cofactor,
        }))) {
            Verdict::Verified(fit) => assert_eq!(fit.eps_phi, OrderSign::Preserving),
            other => panic!("the branch-aware cone miter must fit: {other:?}"),
        }
        // Without it (full R), the R_φ-cofactor cannot satisfy X == R_full·Q ⇒ refused.
        assert!(matches!(
            miter_fit_transverse(&make(None)),
            Verdict::Refuted(TransverseMiterFault::CarrierMismatch)
        ));
    }
}
