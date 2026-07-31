//! The replayable `(claim, certificate)` a searcher emits (M3a Phase 4). Per
//! event: the branch taken, the vanishing minors / discriminant `Δ`, the
//! membership comparisons, and the tangency-identity value — everything the
//! future `certify_core::arrange` checker (M3e) needs to re-verify *without*
//! re-searching. Designed and populated now; validated by differential + property
//! + corpus until the checker lands.

use geom::content::{CurveId, Point2};
use lattice::{Backend, Bignum, Surd};

/// Which stratum of the spine a pair fell into (spec §6 most-degenerate-first).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpineBranch {
    /// Step 1: the carriers coincide (deferred to the stage-2 1D lattice, 3c).
    CarrierCoincident,
    /// Steps 2–3: carriers meet in no retained point (parallel/disjoint, or every
    /// candidate discarded by membership).
    NoIntersection,
    /// Step 4: at least one carrier point survived membership and was classified.
    Touches,
}

/// A retained touch, with the exact value the M3e checker re-tests: `det` is the
/// classification determinant `det(ċ_A, ċ_B)` — its sign is transversality, and
/// `det = 0` is the exact tangency identity. (`Δ` and the membership comparisons
/// are recomputable locally by the checker from the two carriers and this point,
/// so they are not re-stored here — the "stated once" discipline.)
#[derive(Debug)]
pub struct TouchWitness<B: Backend = Bignum> {
    pub point: Point2<B>,
    pub det: Surd<B>,
}

impl<B: Backend> Clone for TouchWitness<B> {
    fn clone(&self) -> Self {
        TouchWitness {
            point: self.point.clone(),
            det: self.det.clone(),
        }
    }
}

/// The stage-2 1D-coincidence outcome for a `CarrierCoincident` pair (slice 3c) —
/// the normative outcome lattice collapsed to the emitted-shape classes. `touches`
/// counts touch-at-point vertices; `merged`/`residuals` count the emitted
/// coincidence sub-edges. The checker re-derives the spans from the two carriers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CoincOutcome {
    /// Disjoint on the shared carrier — nothing emitted.
    Disjoint,
    /// Same-source pieces (one decomposed curve) — not a coincidence, skipped.
    SameSource,
    /// Touch(es) at a point / the shared extrema — vertices, no merged edge.
    Touch { touches: usize },
    /// Overlap — one merged edge (both operands) plus residual sub-edges.
    Overlap {
        touches: usize,
        merged: usize,
        residuals: usize,
    },
}

/// The decision trail for one processed edge pair — replayed as-is by the checker.
#[derive(Debug)]
pub struct PairWitness<B: Backend = Bignum> {
    pub sources: (CurveId, CurveId),
    pub branch: SpineBranch,
    pub touches: Vec<TouchWitness<B>>,
    /// The 1D-coincidence outcome — `Some` exactly on the `CarrierCoincident` branch.
    pub coincidence: Option<CoincOutcome>,
}

impl<B: Backend> Clone for PairWitness<B> {
    fn clone(&self) -> Self {
        PairWitness {
            sources: self.sources,
            branch: self.branch,
            touches: self.touches.clone(),
            coincidence: self.coincidence,
        }
    }
}

/// The full arrangement witness: one record per processed pair (including the
/// `CarrierCoincident` and `NoIntersection` branches, so the checker can replay
/// the entire decision tree, not only the emitted events).
#[derive(Debug)]
pub struct Witness<B: Backend = Bignum> {
    pub pairs: Vec<PairWitness<B>>,
}

impl<B: Backend> Clone for Witness<B> {
    fn clone(&self) -> Self {
        Witness {
            pairs: self.pairs.clone(),
        }
    }
}

impl<B: Backend> Default for Witness<B> {
    fn default() -> Self {
        Witness { pairs: Vec::new() }
    }
}

impl<B: Backend> Witness<B> {
    pub fn new() -> Self {
        Self::default()
    }
}
