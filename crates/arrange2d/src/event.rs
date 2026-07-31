//! The `Event` / `EventSet` the spine emits (M3a Phase 4): a touch vertex with
//! its kind, sidedness bits, and provenance. The set is deduped by exact
//! `geom::content::Point2` equality — the ℓ=0 vertex identity (free,
//! classifier-internal); `0 < ℓ < q_sep` edges are never merged.

use geom::content::{CurveId, Edge, Point2};
use lattice::{Backend, Bignum};

/// How two carriers meet at a retained vertex.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TouchKind {
    /// Carriers cross: `det(ċ_A, ċ_B) ≠ 0`. `det_sign` is the raw sidedness datum
    /// (the ℤ₂² face-flip encoding is deferred to slice 3d).
    Transverse { det_sign: i8 },
    /// Carriers are tangent: `det(ċ_A, ċ_B) = 0` (the A-identity holds), reached
    /// only for non-coincident carriers (most-degenerate-first guard).
    Tangent,
    /// A touch on a **shared** carrier (slice 3c): two coincident edges meet at a
    /// point (a shared endpoint / extremum), decided by the 1D coincidence lattice.
    Coincident,
}

/// One incidence at a vertex: how a specific pair of source curves touches there.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Incidence {
    pub kind: TouchKind,
    pub sources: (CurveId, CurveId),
}

/// A touch vertex: the exact point and every pairwise incidence at it. No valence
/// is assumed — concurrency (three or more branches through a point) is just a
/// vertex with more incidences.
#[derive(Debug)]
pub struct Vertex<B: Backend = Bignum> {
    pub point: Point2<B>,
    pub incidences: Vec<Incidence>,
}

impl<B: Backend> Clone for Vertex<B> {
    fn clone(&self) -> Self {
        Vertex {
            point: self.point.clone(),
            incidences: self.incidences.clone(),
        }
    }
}

/// The set of touch vertices, deduped by exact [`Point2`] equality: equal points
/// are one vertex (concurrency / endpoint-on-curve — the ℓ=0 identity, free), and
/// `0 < ℓ < q_sep` points are never merged.
#[derive(Debug)]
pub struct EventSet<B: Backend = Bignum> {
    pub vertices: Vec<Vertex<B>>,
}

impl<B: Backend> Clone for EventSet<B> {
    fn clone(&self) -> Self {
        EventSet {
            vertices: self.vertices.clone(),
        }
    }
}

impl<B: Backend> Default for EventSet<B> {
    fn default() -> Self {
        EventSet {
            vertices: Vec::new(),
        }
    }
}

impl<B: Backend> EventSet<B> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an incidence at `point`, merging into the existing vertex when the
    /// point is exactly equal (the ℓ=0 identity).
    pub fn insert(&mut self, point: Point2<B>, inc: Incidence) {
        if let Some(v) = self.vertices.iter_mut().find(|v| v.point == point) {
            v.incidences.push(inc);
        } else {
            self.vertices.push(Vertex {
                point,
                incidences: vec![inc],
            });
        }
    }

    /// Number of distinct touch vertices.
    pub fn len(&self) -> usize {
        self.vertices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }
}

/// Which input curves cover a coincidence sub-edge (slice 3c).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Operand {
    /// The merged coincident sub-edge — both operands cover it (spec §6: 3d step-5
    /// attaches "both operands' signed incidence"; step-7 makes it a merge
    /// instruction consumed by the quotient).
    Both,
    /// A residual sub-edge covered only by the first curve.
    First,
    /// A residual sub-edge covered only by the second curve.
    Second,
}

/// A 1-D coincidence output edge on a shared carrier (slice 3c): the merged edge
/// (`Both`) where two coincident curves overlap, or a residual sub-edge (`First`/
/// `Second`) where only one covers. Fed to 3d's DCEL.
#[derive(Debug)]
pub struct CoincEdge<B: Backend = Bignum> {
    pub edge: Edge<B>,
    pub operand: Operand,
    pub sources: (CurveId, CurveId),
}

impl<B: Backend> Clone for CoincEdge<B> {
    fn clone(&self) -> Self {
        CoincEdge {
            edge: self.edge.clone(),
            operand: self.operand,
            sources: self.sources,
        }
    }
}
