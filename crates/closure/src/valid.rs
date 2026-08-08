//! The closure-level obligation: the `CLOSURE-CAP(j)` disjunction (`MITER-BRANCH ∨
//! LEDGE-BRANCH`) and the full `CLOSURE_VALID(j)` conjunction (spec §8.5), SEW included.
//!
//! This is the C6 capstone — the wiring that composes the C1–C5 searchers into the single
//! treatment verdict. It mints no new soundness decision: every conjunct is still decided by a
//! pure-tier `certify_core` checker, and this module only *orchestrates* them (try MITER, fall
//! back to LEDGE; AND the regularity / trim / cap verdicts, short-circuiting to the first
//! refutation).
//!
//! # CLOSURE-CAP — the disjunction
//!
//! [`closure_cap`] attempts the **clean miter** first: when the two flanks' trimmed cut faces
//! coincide in the cap plane, [`clean_miter_cap`](crate::miter::clean_miter_cap) certifies the
//! cap with no planar ledge to arrange. A MITER refusal (the searcher's `NotClean` cue, or a
//! MITER-OUT refutation) falls back to the **forced ledge**
//! ([`ledge_cap_certified`](crate::ledge::ledge_cap_certified)), which builds a planar cap
//! region by the §6 boolean. The disjunction certifies iff *either* branch does; a caller may
//! offer one branch or both.
//!
//! # CLOSURE_VALID(j) — the conjunction
//!
//! [`closure_valid`] assembles, from a [`Joint`] and its authored treatment parameters:
//!
//! - **REG-V ∧ WEDGE ∧ EXT-WEDGE** (+ the crease-local witness for SIDE and COLLAR) via
//!   [`wedge_cert`](crate::wedge::wedge_cert) → [`regularity`](certify_core::wedge::regularity);
//! - **CLIP-DOM(G_A) ∧ CLIP-DOM(G_B)** — the trim boundary is transverse across each flank's
//!   σ-support (the CLIP-W rung of [`clip`](certify_core::certify1d::clip));
//! - **SIDE(b_J) ∧ TRIM-LOCAL** — each flank's retained side is uniformly `G_i > 0` over its
//!   support (the outer-fiber positivity of [`trim_local`](certify_core::certify1d::trim_local));
//! - **CLOSURE-CAP** — the disjunction above;
//! - **SEW = SEW-EDGES ∧ SEW-LINK** — the shared final conjunct of both cap branches: the sewn
//!   shell's edge-occupancy ledger and every boundary vertex's embedded spherical link, audited by
//!   the pure [`sew_edges`](certify_core::sew::sew_edges) / [`sew_link`](certify_core::sew::sew_link)
//!   checkers over the searcher's [`SewInput`].
//!
//! The remaining conjuncts are discharged by the **straight-crease scope** the M4 slice lives
//! on: FLANK-FIT, TUBE-LOCAL, TUBE-SELF and REMOTE/VERTEX are vacuous where `κ_max = 0` (zero
//! tube width, spec §13; see [`crate::trim`] and [`crate::wedge`] module docs), and the
//! straight-crease scope predicate is the population itself. With SEW in, `closure_valid`
//! certifies the full `CLOSURE_VALID(j)` — a watertight sewn shell.
//!
//! Nothing keys on the flank *type*: [`closure_valid`] threads two arbitrary
//! [`geom::chart::Chart`]s' fields through the same checkers, and every fault falls out of a
//! ring/Sturm comparison, never a Rust branch on cone-vs-cylinder.

use arrange2d::boolean::{CapOut, CapOutFault};
use certify_core::MarginSq;
use certify_core::Verdict;
use certify_core::cap_in::ValidatedD24;
use certify_core::certify1d::{ClipVerdict, RegFault, clip, trim_local};
use certify_core::miter::{CutEnds, MiterOutFault, MiterOutWitness, Occupancy, OrderSign};
use certify_core::sew::{
    EdgeRecord, FaceGermSpecies, SewCounts, SewEdgesFault, SewLinkFault, sew_edges, sew_link,
};
use certify_core::wedge::{WedgeFault, WedgeWitness, regularity};
use lattice::{Backend, Bignum, Interval, Rat};

use crate::ledge::{LedgeError, ledge_cap_certified};
use crate::miter::{MiterSearchError, clean_miter_cap};
use crate::trim::{clip_w_cert, crease_anchor, field_a, field_b, trim_local_cert};
use crate::wedge::wedge_cert;
use crate::{Joint, MuRange};

/// The searcher's built inputs for the clean-miter branch — the paired cut edges of the two
/// flanks, the shared crease direction, the per-pairing order-sign claims and transverse
/// occupancies, and the EDGE-REG separation margin. Consumed by
/// [`clean_miter_cap`](crate::miter::clean_miter_cap) inside [`closure_cap`].
pub struct MiterInput<'a, B: Backend = Bignum> {
    /// Flank A's cut edges, head to tail around the cap.
    pub a: &'a [CutEnds<B>],
    /// Flank B's cut edges, at matching cyclic positions.
    pub b: &'a [CutEnds<B>],
    /// The shared crease-line direction `m̂`.
    pub crease_dir: &'a (Rat<B>, Rat<B>),
    /// The searcher's order-sign claim per pairing.
    pub claimed: &'a [OrderSign],
    /// The transverse occupancy per pairing.
    pub occ: &'a [Occupancy],
    /// The EDGE-REG separation margin each edge must clear.
    pub margin: &'a Rat<B>,
}

/// Which disjunct of `CLOSURE-CAP(j)` (`MITER-BRANCH ∨ LEDGE-BRANCH`) certified the cap.
pub enum CapWitness<B: Backend = Bignum> {
    /// The clean-miter branch: the flanks' cut faces coincided (MITER-FIT ∧ MITER-EDGE-LEDGER ∧
    /// MITER-OUT), carrying the [`MiterOutWitness`].
    Miter(MiterOutWitness<B>),
    /// The forced-ledge branch: a certified planar cap region (CAP-IN-D24 ∧ LEDGE-DOM ∧
    /// CAP-OUT), carrying the [`CapOut`].
    Ledge(CapOut<B>),
}

/// How the MITER branch declined.
pub enum MiterMiss<B: Backend = Bignum> {
    /// The searcher refused the pairing — not a clean miter (MITER-FIT refuted, or ragged
    /// input). This is the canonical cue to fall back to LEDGE.
    NotClean(MiterSearchError<B>),
    /// MITER-FIT passed but MITER-OUT refuted the inventory — a genuine miter-branch failure.
    Out(MiterOutFault),
}

/// How the LEDGE branch declined.
pub enum LedgeMiss {
    /// The bridge declined a component (an arc carrier the M4 slice does not represent).
    Bridge(LedgeError),
    /// CAP-OUT refuted the licensed boundary (a constructor bug — licensed input is well-formed).
    Out(CapOutFault),
    /// CAP-OUT returned `Unresolved` (documented total; folded defensively).
    Inconclusive,
}

/// Why [`closure_cap`] could certify neither disjunct of CLOSURE-CAP.
pub enum CapFault<B: Backend = Bignum> {
    /// No branch was offered — the searcher supplied neither a miter attempt nor a ledge
    /// boundary.
    NoBranch,
    /// The only offered branch (MITER) failed.
    Miter(MiterMiss<B>),
    /// The only offered branch (LEDGE) failed.
    Ledge(LedgeMiss),
    /// Both branches were offered and both failed — the cap is genuinely uncertifiable.
    Both {
        /// The MITER refusal.
        miter: MiterMiss<B>,
        /// The LEDGE refusal.
        ledge: LedgeMiss,
    },
}

/// `CLOSURE-CAP(j)`, the `MITER-BRANCH ∨ LEDGE-BRANCH` disjunction (spec §8.5): certify the
/// closure cap by whichever disjunct succeeds, attempting the clean miter first. SEW — the shared
/// final conjunct of both branches — is applied once by [`closure_valid`], not here.
///
/// The MITER branch ([`clean_miter_cap`](crate::miter::clean_miter_cap)) is tried when a
/// [`MiterInput`] is offered; a refusal falls through to the LEDGE branch
/// ([`ledge_cap_certified`](crate::ledge::ledge_cap_certified)) when a licensed [`ValidatedD24`]
/// boundary is offered. Returns [`Verified`](Verdict::Verified) with the [`CapWitness`] of the
/// branch that certified, or [`Refuted`](Verdict::Refuted) with a [`CapFault`] recording what
/// each offered branch did. Offering neither is [`CapFault::NoBranch`].
///
/// ```
/// use certify_core::Verdict;
/// use certify_core::cap_in::{cap_in_d24, FlankId};
/// use closure::cap_in::segment_edge;
/// use closure::valid::{closure_cap, CapWitness};
/// use lattice::{Bignum, Rat};
///
/// let p = |x: i128, y: i128| (Rat::<Bignum>::from_i128(x), Rat::from_i128(y));
/// // A square cap spanning both flanks — the forced-ledge branch (no miter offered).
/// let sq = [
///     segment_edge(&p(0, 0), &p(2, 0), FlankId::Crease),
///     segment_edge(&p(2, 0), &p(2, 2), FlankId::A),
///     segment_edge(&p(2, 2), &p(0, 2), FlankId::A),
///     segment_edge(&p(0, 2), &p(0, 0), FlankId::B),
/// ];
/// let d24 = match cap_in_d24(&sq) {
///     Verdict::Verified(v) => v,
///     other => panic!("cap boundary must license: {other:?}"),
/// };
/// match closure_cap(None, Some(&d24)) {
///     Verdict::Verified(CapWitness::Ledge(cap)) => assert_eq!(cap.region().faces.len(), 1),
///     other => panic!("the forced ledge must certify: {}", matches!(other, Verdict::Verified(_))),
/// }
/// ```
pub fn closure_cap<B: Backend>(
    miter: Option<&MiterInput<'_, B>>,
    ledge: Option<&ValidatedD24<B>>,
) -> Verdict<CapWitness<B>, CapFault<B>, ()> {
    // MITER-BRANCH first: a clean miter caps with no ledge to arrange.
    let miter_miss = match miter {
        Some(mi) => match clean_miter_cap(mi.a, mi.b, mi.crease_dir, mi.claimed, mi.occ, mi.margin)
        {
            Ok(Verdict::Verified(w)) => return Verdict::Verified(CapWitness::Miter(w)),
            Ok(Verdict::Refuted(f)) => Some(MiterMiss::Out(f)),
            // MITER-OUT is total; a defensive Unresolved is treated as a non-clean miss.
            Ok(Verdict::Unresolved(())) => {
                Some(MiterMiss::NotClean(MiterSearchError::Inconclusive {
                    at: 0,
                }))
            }
            Err(e) => Some(MiterMiss::NotClean(e)),
        },
        None => None,
    };
    // LEDGE-BRANCH fallback.
    let ledge_miss = match ledge {
        Some(d24) => match ledge_cap_certified(d24) {
            Ok(Verdict::Verified(cap)) => return Verdict::Verified(CapWitness::Ledge(cap)),
            Ok(Verdict::Refuted(f)) => Some(LedgeMiss::Out(f)),
            Ok(Verdict::Unresolved(())) => Some(LedgeMiss::Inconclusive),
            Err(e) => Some(LedgeMiss::Bridge(e)),
        },
        None => None,
    };
    Verdict::Refuted(match (miter_miss, ledge_miss) {
        (None, None) => CapFault::NoBranch,
        (Some(m), None) => CapFault::Miter(m),
        (None, Some(l)) => CapFault::Ledge(l),
        (Some(m), Some(l)) => CapFault::Both { miter: m, ledge: l },
    })
}

/// One boundary vertex's **SEW-LINK** inputs: the embedded spherical link (its incident rays in
/// stored rotation order vs geometric azimuth order) and the FACE-GERM species cover of its
/// selected sectors. The searcher builds these from the sewn shell's arrangement
/// (`arrange2d::boolean::vertex_link` + the germ classification); [`sew_link`] audits them.
pub struct VertexLink {
    /// `Link_emitted` — the incident rays in stored rotation-walk order.
    pub emitted: Vec<usize>,
    /// `Link_geometric` — the same rays in geometric azimuth-sort order.
    pub geometric: Vec<usize>,
    /// The cyclic sector-selected mask, aligned to `geometric`.
    pub sectors: Vec<bool>,
    /// The FACE-GERM species of each *selected* sector, in azimuth order.
    pub species: Vec<FaceGermSpecies>,
}

/// The searcher's **SEW** inputs — the shared final conjunct of both CLOSURE-CAP branches
/// (`SEW = SEW-EDGES ∧ SEW-LINK`, spec §8.5). The EDGE-OCCUPANCY records + declared incidence
/// counts (SEW-EDGES) and every boundary vertex's embedded link (SEW-LINK). Built by the sewing
/// searcher (the `sew` crate) from the certified cap; audited here by the pure `certify_core::sew`
/// checkers. An empty/internal joint declares zero counts and zero records, and has no boundary
/// vertices — `SewInput::default`.
pub struct SewInput<B: Backend = Bignum> {
    /// The emitted EDGE-OCCUPANCY records of the sewn shell.
    pub records: Vec<EdgeRecord<B>>,
    /// The declared source-side incidence counts (both directions).
    pub counts: SewCounts,
    /// The boundary vertices' embedded links (the SEW-LINK domain is `V_∂`).
    pub links: Vec<VertexLink>,
}

impl<B: Backend> Default for SewInput<B> {
    fn default() -> Self {
        SewInput {
            records: Vec::new(),
            counts: SewCounts {
                cap_to_flank: 0,
                flank_to_flank: 0,
            },
            links: Vec::new(),
        }
    }
}

/// The authored **treatment parameters** of a closure joint — everything the searcher supplies
/// beyond the geometric [`Joint`] itself. Threaded into [`closure_valid`].
///
/// The trim boxes are **per flank**: each flank's retained σ-support sits near its own crease
/// station (they need not coincide), while the ruling range `mu`, the normal offset `w`, and
/// the margins are shared. `cap_miter` / `cap_ledge` are the two CLOSURE-CAP disjunct inputs
/// (offer either or both).
pub struct ClosureTreatment<'a, B: Backend = Bignum> {
    /// The bevel slope `s_bev ≥ 0`.
    pub s_bev: Rat<B>,
    /// The proposed REG-V squared margin `m > 0`.
    pub reg_v_margin: MarginSq<Rat<B>>,
    /// The retained ruling range `[μ⁻, μ⁺]` (shared).
    pub mu: MuRange<B>,
    /// The normal-offset box `w ∈ [w⁻, w⁺]` (shared).
    pub w: Interval<B>,
    /// Flank A's retained σ-support (near its crease station `σ_a`).
    pub sigma_a: Interval<B>,
    /// Flank B's retained σ-support (near its crease station `σ_b`).
    pub sigma_b: Interval<B>,
    /// Flank A's confinement fiber `(μ, w)` — the box-minimizing corner for TRIM-LOCAL.
    pub confine_a: (Rat<B>, Rat<B>),
    /// Flank B's confinement fiber `(μ, w)`.
    pub confine_b: (Rat<B>, Rat<B>),
    /// The TRIM-LOCAL interior-confinement squared margin.
    pub trim_margin: MarginSq<Rat<B>>,
    /// The CLIP-W transversality squared margin (`(∂_wG)² ≥ m`).
    pub clip_margin: MarginSq<Rat<B>>,
    /// The clean-miter disjunct input (if offered).
    pub cap_miter: Option<MiterInput<'a, B>>,
    /// The forced-ledge disjunct input (if offered).
    pub cap_ledge: Option<&'a ValidatedD24<B>>,
    /// The SEW inputs — the sewn shell's edge records + counts and boundary-vertex links.
    pub sew: SewInput<B>,
}

/// The evidence a [`closure_valid`] `Verified` carries: the regularity witness and which
/// CLOSURE-CAP disjunct certified. The trim/clip conjuncts leave no residual witness — reaching
/// `Verified` *is* their certification.
pub struct ClosureValid<B: Backend = Bignum> {
    /// The REG-V / WEDGE / EXT-WEDGE witness (`d = n_A·n_B` and the cleared REG-V margin).
    pub wedge: WedgeWitness<B>,
    /// The certified cap and its branch.
    pub cap: CapWitness<B>,
}

/// Which conjunct of `CLOSURE_VALID(j)` refused.
pub enum ClosureFault<B: Backend = Bignum> {
    /// A crease normal, bisector, or pedal was singular at a crease station — the searcher
    /// could not build the certificate.
    SingularCrease,
    /// The regularity bundle (REG-V ∧ WEDGE ∧ EXT-WEDGE, + crease-local SIDE/COLLAR) refused.
    Regularity(WedgeFault<B>),
    /// CLIP-DOM: the trim boundary is not transverse across a flank's support (the CLIP-W rung
    /// did not certify).
    ClipDom {
        /// `false` = flank A, `true` = flank B.
        flank_b: bool,
        /// The non-certifying CLIP verdict.
        verdict: ClipVerdict,
    },
    /// TRIM-LOCAL (SIDE's wrong-side test): a flank's retained side is not uniformly `G_i > 0`.
    TrimLocal {
        /// `false` = flank A, `true` = flank B.
        flank_b: bool,
        /// The checker's refutation.
        fault: RegFault<B>,
    },
    /// CLOSURE-CAP certified neither disjunct.
    Cap(CapFault<B>),
    /// SEW-EDGES refused the sewn shell's edge-occupancy ledger (a pinch, a bad identity, or a
    /// count mismatch).
    SewEdges(SewEdgesFault),
    /// SEW-LINK refused a boundary vertex's embedded spherical link.
    SewLink {
        /// The offending vertex's index into [`SewInput::links`].
        vertex: usize,
        /// The link refutation.
        fault: SewLinkFault,
    },
}

/// `CLOSURE_VALID(j)` (spec §8.5): the closure-level conjunction that certifies a joint's treatment
/// as a watertight sewn shell.
///
/// Assembles, short-circuiting to the first refutation: the regularity bundle, CLIP-DOM and
/// TRIM-LOCAL for both flanks' retained-side fields `G_A`/`G_B`, CLOSURE-CAP, and finally
/// **SEW = SEW-EDGES ∧ SEW-LINK** over the searcher's [`SewInput`]. The remaining spec conjuncts
/// (FLANK-FIT, TUBE-LOCAL/SELF, REMOTE, VERTEX) are vacuous on the straight-crease scope
/// (`κ_max = 0`; module docs). Returns [`Verified`](Verdict::Verified) with a [`ClosureValid`]
/// witness, or [`Refuted`](Verdict::Refuted) naming the failed conjunct.
pub fn closure_valid<B: Backend>(
    joint: &Joint<B>,
    t: &ClosureTreatment<'_, B>,
) -> Verdict<ClosureValid<B>, ClosureFault<B>, ()> {
    // REG-V ∧ WEDGE ∧ EXT-WEDGE (+ crease-local SIDE/COLLAR witness).
    let wcert = match wedge_cert(joint, t.s_bev.clone(), t.reg_v_margin.clone()) {
        Some(c) => c,
        None => return Verdict::Refuted(ClosureFault::SingularCrease),
    };
    let wedge = match regularity(&wcert) {
        Verdict::Verified(w) => w,
        Verdict::Refuted(f) => return Verdict::Refuted(ClosureFault::Regularity(f)),
        Verdict::Unresolved(()) => return Verdict::Unresolved(()),
    };

    // Build the two retained-side fields G_A, G_B against the trim plane through the crease.
    let x0 = match crease_anchor(joint) {
        Some(x) => x,
        None => return Verdict::Refuted(ClosureFault::SingularCrease),
    };
    let g_a = match field_a(joint, &x0) {
        Some(g) => g,
        None => return Verdict::Refuted(ClosureFault::SingularCrease),
    };
    let g_b = match field_b(joint, &x0) {
        Some(g) => g,
        None => return Verdict::Refuted(ClosureFault::SingularCrease),
    };

    // CLIP-DOM(G_A), CLIP-DOM(G_B): the trim boundary is w-transverse across each support.
    for (flank_b, g, sigma) in [(false, &g_a, &t.sigma_a), (true, &g_b, &t.sigma_b)] {
        let cw = clip_w_cert(g, t.clip_margin.clone(), sigma.clone());
        let verdict = clip(&cw, &[], &[], None);
        if verdict != ClipVerdict::Certified {
            return Verdict::Refuted(ClosureFault::ClipDom { flank_b, verdict });
        }
    }

    // SIDE(b_J) ∧ TRIM-LOCAL: each flank's retained side is uniformly G_i > 0 over its support.
    for (flank_b, g, sigma, confine) in [
        (false, &g_a, &t.sigma_a, &t.confine_a),
        (true, &g_b, &t.sigma_b, &t.confine_b),
    ] {
        let cert = match trim_local_cert(
            g,
            &t.mu,
            &t.w,
            sigma,
            &confine.0,
            &confine.1,
            t.trim_margin.clone(),
        ) {
            Some(c) => c,
            None => return Verdict::Refuted(ClosureFault::SingularCrease),
        };
        match trim_local(&cert) {
            Verdict::Verified(_) => {}
            Verdict::Refuted(fault) => {
                return Verdict::Refuted(ClosureFault::TrimLocal { flank_b, fault });
            }
            Verdict::Unresolved(()) => return Verdict::Unresolved(()),
        }
    }

    // CLOSURE-CAP: MITER ∨ LEDGE.
    let cap = match closure_cap(t.cap_miter.as_ref(), t.cap_ledge) {
        Verdict::Verified(c) => c,
        Verdict::Refuted(f) => return Verdict::Refuted(ClosureFault::Cap(f)),
        Verdict::Unresolved(()) => return Verdict::Unresolved(()),
    };

    // SEW = SEW-EDGES ∧ SEW-LINK — the shared final conjunct of both cap branches: audit the sewn
    // shell's edge-occupancy ledger, then every boundary vertex's embedded spherical link.
    match sew_edges(&t.sew.records, t.sew.counts) {
        Verdict::Verified(_) => {}
        Verdict::Refuted(f) => return Verdict::Refuted(ClosureFault::SewEdges(f)),
        Verdict::Unresolved(()) => return Verdict::Unresolved(()),
    }
    for (vertex, l) in t.sew.links.iter().enumerate() {
        match sew_link(&l.emitted, &l.geometric, &l.sectors, &l.species) {
            Verdict::Verified(_) => {}
            Verdict::Refuted(fault) => {
                return Verdict::Refuted(ClosureFault::SewLink { vertex, fault });
            }
            Verdict::Unresolved(()) => return Verdict::Unresolved(()),
        }
    }

    Verdict::Verified(ClosureValid { wedge, cap })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cap_in::segment_edge;
    use crate::miter::segment_cut_ends;
    use crate::{Crease, Flank, JointSign};
    use certify_core::cap_in::{FlankId, cap_in_d24};
    use certify_core::sew::{EdgeIdentity, EdgeProvenance};
    use geom::chart::Chart;
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
    /// The canonical cylinder about the x-axis (`q = 1 + σi`).
    fn cylinder() -> Chart<Bignum> {
        Chart::new(
            [poly(&[1]), poly(&[0, 1]), poly(&[0]), poly(&[0])],
            RatFunc::zero(),
        )
    }
    fn mu() -> MuRange<Bignum> {
        MuRange {
            lo: q(-1),
            hi: q(1),
        }
    }
    /// The canonical 90° cylinder self-fold: σ_a = 0 (normal ẑ), σ_b = 1 (normal −ŷ),
    /// s_J = +1 ⇒ b_J = (0, 1, 1). Flank A's retained support is σ ∈ [0, 1/4]; flank B's is
    /// σ ∈ [1/2, 1] (`g_B_w = −g_A_w`, positive past the root σ = −1 + √2 ≈ 0.414).
    fn fold() -> Joint<Bignum> {
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
    fn iv(lo: (i128, i128), hi: (i128, i128)) -> Interval<Bignum> {
        Interval {
            lo: Rat::new(lo.0, lo.1),
            hi: Rat::new(hi.0, hi.1),
        }
    }
    /// A SEW-passing packet for the sewn fold: one flank-to-flank clean-miter seam edge
    /// (opposite-side boundary-boundary occupancy, a coincident PAIR-IDENTICAL pair) and one
    /// boundary vertex whose link is a trivially-consistent one-arc boundary (`Link_emitted`
    /// identical to `Link_geometric`, its selected sectors covered by flank germs).
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

    /// The treatment scaffold shared by both cap-branch tests — regularity + trim params tuned
    /// to the 90° fold (from the C2/C3 known-passing boxes) and a SEW-passing packet. Caller
    /// supplies the cap disjunct.
    fn treatment<'a>(
        cap_miter: Option<MiterInput<'a, Bignum>>,
        cap_ledge: Option<&'a ValidatedD24<Bignum>>,
    ) -> ClosureTreatment<'a, Bignum> {
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
            cap_miter,
            cap_ledge,
            sew: sew_ok(),
        }
    }
    /// The MITER cap inputs a [`diamond`] hands to a [`MiterInput`]: the cut edges, the claimed
    /// per-edge order signs, the occupancy rows, the crease direction, and the margin.
    type DiamondCap = (
        Vec<CutEnds<Bignum>>,
        Vec<OrderSign>,
        Vec<Occupancy>,
        (Q, Q),
        Q,
    );

    /// A diamond clean-miter cap outline (from the C5 MITER test): four edges, each traced
    /// identically by both flanks, transverse to the crease direction ŷ.
    fn diamond() -> DiamondCap {
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
        let occ = vec![
            Occupancy {
                a_l: true,
                a_r: false,
                b_l: false,
                b_r: true,
                frame: false,
            };
            4
        ];
        let claimed = vec![OrderSign::Preserving; 4];
        (a, claimed, occ, p(0, 1), q(1))
    }
    /// A square ledge cap spanning both flanks (from the C4 LEDGE doctest).
    fn square_d24() -> ValidatedD24<Bignum> {
        let sq = [
            segment_edge(&p(0, 0), &p(2, 0), FlankId::Crease),
            segment_edge(&p(2, 0), &p(2, 2), FlankId::A),
            segment_edge(&p(2, 2), &p(0, 2), FlankId::A),
            segment_edge(&p(0, 2), &p(0, 0), FlankId::B),
        ];
        match cap_in_d24(&sq) {
            Verdict::Verified(v) => v,
            other => panic!("square cap must license: {other:?}"),
        }
    }

    #[test]
    fn the_disjunction_certifies_the_clean_miter() {
        let (a, claimed, occ, cd, margin) = diamond();
        let b = a.clone();
        let mi = MiterInput {
            a: &a,
            b: &b,
            crease_dir: &cd,
            claimed: &claimed,
            occ: &occ,
            margin: &margin,
        };
        match closure_cap(Some(&mi), None) {
            Verdict::Verified(CapWitness::Miter(w)) => assert_eq!(w.edge_margins.len(), 4),
            other => panic!(
                "the clean miter must certify via the MITER disjunct: {}",
                matches!(other, Verdict::Verified(_))
            ),
        }
    }

    #[test]
    fn the_disjunction_falls_back_to_the_ledge() {
        // A degenerate miter attempt (empty edge lists is Ragged → NotClean) with a valid ledge
        // boundary offered: the disjunction refuses the miter and certifies the ledge. This is
        // the "two abutting boxes / miter cap suppression" route — MITER declines, LEDGE caps.
        let d24 = square_d24();
        let empty: Vec<CutEnds<Bignum>> = Vec::new();
        let claimed: Vec<OrderSign> = Vec::new();
        let occ: Vec<Occupancy> = Vec::new();
        let cd = p(0, 1);
        let margin = q(1);
        let mi = MiterInput {
            a: &empty,
            b: &empty,
            crease_dir: &cd,
            claimed: &claimed,
            occ: &occ,
            margin: &margin,
        };
        // Empty pairing lists: clean_miter_cap succeeds vacuously? No — an empty cycle has no
        // edges, so MITER-OUT refutes (Empty). Either way MITER does not clean-certify a real
        // cap here; the fallback must reach the ledge.
        match closure_cap(Some(&mi), Some(&d24)) {
            Verdict::Verified(CapWitness::Ledge(cap)) => assert_eq!(cap.region().faces.len(), 1),
            Verdict::Verified(CapWitness::Miter(_)) => {
                panic!("an empty miter must not clean-certify a cap")
            }
            other => panic!(
                "the ledge fallback must certify: {}",
                matches!(other, Verdict::Verified(_))
            ),
        }
    }

    #[test]
    fn closure_valid_certifies_the_fold_with_a_miter_cap() {
        let (a, claimed, occ, cd, margin) = diamond();
        let b = a.clone();
        let mi = MiterInput {
            a: &a,
            b: &b,
            crease_dir: &cd,
            claimed: &claimed,
            occ: &occ,
            margin: &margin,
        };
        let t = treatment(Some(mi), None);
        match closure_valid(&fold(), &t) {
            Verdict::Verified(v) => {
                assert!(v.wedge.n_dot.is_zero()); // 90° fold: d = 0
                assert!(matches!(v.cap, CapWitness::Miter(_)));
            }
            Verdict::Refuted(_) => panic!("the 90° fold with a clean-miter cap is CLOSURE_VALID"),
            Verdict::Unresolved(()) => panic!("closure_valid was inconclusive"),
        }
    }

    #[test]
    fn closure_valid_certifies_the_fold_with_a_ledge_cap() {
        let d24 = square_d24();
        let t = treatment(None, Some(&d24));
        match closure_valid(&fold(), &t) {
            Verdict::Verified(v) => assert!(matches!(v.cap, CapWitness::Ledge(_))),
            Verdict::Refuted(_) => panic!("the 90° fold with a forced-ledge cap is CLOSURE_VALID"),
            Verdict::Unresolved(()) => panic!("closure_valid was inconclusive"),
        }
    }

    #[test]
    fn closure_valid_refuses_a_flat_joint() {
        // Both flanks meet at the same station ⇒ n_A = n_B ⇒ |V|² = 0: the regularity conjunct
        // deletes the record. The conjunction short-circuits at REG-V, never reaching the cap.
        let flat = Joint::new(
            Flank::new(cylinder(), mu()),
            Flank::new(cylinder(), mu()),
            Crease {
                sigma_a: q(0),
                sigma_b: q(0),
            },
            JointSign::Plus,
        );
        let d24 = square_d24();
        let t = treatment(None, Some(&d24));
        assert!(matches!(
            closure_valid(&flat, &t),
            Verdict::Refuted(ClosureFault::Regularity(_))
        ));
    }

    #[test]
    fn closure_valid_refuses_a_pinch_occupancy() {
        // A regular, cap-certified fold, but the sewn shell carries an opposite-quadrant pinch —
        // both sides of flank A occupied, neither of B — a non-manifold transverse link SEW-EDGES
        // must reject even though every geometric conjunct upstream passed.
        let d24 = square_d24();
        let mut t = treatment(None, Some(&d24));
        t.sew.records = vec![EdgeRecord {
            occupancy: Occupancy {
                a_l: true,
                a_r: true,
                b_l: false,
                b_r: false,
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
        }];
        t.sew.counts = SewCounts {
            cap_to_flank: 0,
            flank_to_flank: 1,
        };
        assert!(matches!(
            closure_valid(&fold(), &t),
            Verdict::Refuted(ClosureFault::SewEdges(SewEdgesFault::Pinch { .. }))
        ));
    }

    #[test]
    fn closure_valid_refuses_a_crossing_link() {
        // The shell's edges sew, but a boundary vertex's link is `a→c→b→d`: the same rays as the
        // emitted order, so a multiset/count test passes — yet not a cyclic rotation of it, so the
        // embedding crosses. SEW-LINK (via link_iso_ok) refuses what a count check would miss.
        let d24 = square_d24();
        let mut t = treatment(None, Some(&d24));
        t.sew.links = vec![VertexLink {
            emitted: vec![0, 1, 2, 3],
            geometric: vec![0, 2, 1, 3],
            sectors: vec![true, true, false, false],
            species: vec![FaceGermSpecies::Flank, FaceGermSpecies::Flank],
        }];
        assert!(matches!(
            closure_valid(&fold(), &t),
            Verdict::Refuted(ClosureFault::SewLink {
                fault: SewLinkFault::LinkMismatch,
                ..
            })
        ));
    }
}
