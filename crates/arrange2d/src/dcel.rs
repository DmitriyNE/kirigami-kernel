//! The half-edge arrangement (spec §6 step 1) — the DCEL the boolean runs on. Built
//! from the retained events: every input edge is split at the arrangement vertices
//! interior to it, coincident sub-edges (same carrier + same endpoints, distinct
//! sources) are merged with their source multiplicity, and each surviving sub-edge
//! becomes a twin half-edge pair. The vertex **rotation system** is the
//! [`super::tangent`] outgoing-tangent azimuth order; faces are traced from the
//! `next` cycles.
//!
//! This is an untrusted **searcher** (like the rest of `arrange2d`). The
//! substrate-link check here ([`Dcel::substrate_link_ok`]: twin pairing complete, no
//! dangling half-edge) is a kernel-defect detector, not the region certificate —
//! correctness is established by the verified checkers in
//! [`certify_core::arrange`], run over the emitted labeling by
//! [`crate::boolean::ledge_dom_certified`].

use certify_core::Verdict;
use core::cmp::Ordering;
use geom::content::{ArcPiece, Edge, Half, Orient, Point2, SegPiece};
use lattice::{Backend, Surd};

use crate::membership::on_edge;
use crate::spine::arrange_events;
use crate::tangent::{dir_cmp, outgoing_tangent};

/// An undirected arrangement edge: the x-monotone sub-edge geometry, its two
/// endpoint vertex ids (`va` = `edge.start`, `vb` = `edge.end`), and the set of
/// `(source, orientation)` that cover it — multiplicity ≥ 2 (distinct sources) is a
/// **coincident** edge (both operands; spec §6 step 5), the rest are ordinary.
pub struct SubEdge<B: Backend> {
    pub edge: Edge<B>,
    pub va: usize,
    pub vb: usize,
    pub sources: Vec<(geom::content::CurveId, Orient)>,
}

/// A directed half-edge. `edge` indexes [`Dcel::edges`]; `dir` is `true` when it
/// runs `start → end` (origin = `va`), `false` for `end → start` (origin = `vb`).
/// `twin` is `id ^ 1`. `next`/`prev` are the traced face cycle; `cycle` its id.
#[derive(Clone, Copy)]
pub struct HalfEdge {
    pub origin: usize,
    pub edge: usize,
    pub dir: bool,
    pub twin: usize,
    pub next: usize,
    pub prev: usize,
    pub cycle: usize,
}

/// The half-edge arrangement: vertices (by id), undirected sub-edges, the half-edge
/// arena (two per edge, `2k` = `start→end`, `2k+1` = `end→start`), and the number of
/// traced face cycles.
pub struct Dcel<B: Backend> {
    pub verts: Vec<Point2<B>>,
    pub edges: Vec<SubEdge<B>>,
    pub halfedges: Vec<HalfEdge>,
    pub n_cycles: usize,
}

const NONE: usize = usize::MAX;

/// The id of `p` in `verts`, appending it if new (exact `Point2` dedup — the ℓ=0
/// identity, matching [`crate::event::EventSet`]).
fn vid<B: Backend>(verts: &mut Vec<Point2<B>>, p: &Point2<B>) -> usize {
    if let Some(i) = verts.iter().position(|q| q == p) {
        i
    } else {
        verts.push(p.clone());
        verts.len() - 1
    }
}

/// The sub-segment of `parent` between `p` and `q` (both on its carrier).
fn sub_seg<B: Backend>(parent: &SegPiece<B>, p: &Point2<B>, q: &Point2<B>) -> Edge<B> {
    Edge::Seg(Box::new(SegPiece {
        line: parent.line.clone(),
        start: p.clone(),
        end: q.clone(),
        orient: parent.orient,
        source: parent.source,
    }))
}

/// The sub-arc of `parent` between `p` and `q` (both on its circle and half).
fn sub_arc<B: Backend>(parent: &ArcPiece<B>, p: &Point2<B>, q: &Point2<B>) -> Edge<B> {
    let (x_lo, x_hi) = if p.x.cmp(&q.x) == Ordering::Greater {
        (q.x.clone(), p.x.clone())
    } else {
        (p.x.clone(), q.x.clone())
    };
    Edge::Arc(Box::new(ArcPiece {
        circle: parent.circle.clone(),
        half: parent.half,
        x_lo,
        x_hi,
        start: p.clone(),
        end: q.clone(),
        winding: parent.winding.clone(),
        source: parent.source,
    }))
}

fn half_u8(h: Half) -> u8 {
    match h {
        Half::Upper => 0,
        Half::Lower => 1,
    }
}

/// Order two carriers so coincident edges (identical carrier) compare `Equal`: a
/// segment's carrier is fixed by its two endpoints, an arc's by its circle + half.
fn carrier_cmp<B: Backend>(a: &Edge<B>, b: &Edge<B>) -> Ordering {
    match (a, b) {
        (Edge::Seg(_), Edge::Seg(_)) => Ordering::Equal,
        (Edge::Seg(_), Edge::Arc(_)) => Ordering::Less,
        (Edge::Arc(_), Edge::Seg(_)) => Ordering::Greater,
        (Edge::Arc(x), Edge::Arc(y)) => x
            .circle
            .cx
            .cmp(&y.circle.cx)
            .then_with(|| x.circle.cy.cmp(&y.circle.cy))
            .then_with(|| x.circle.r2.cmp(&y.circle.r2))
            .then_with(|| half_u8(x.half).cmp(&half_u8(y.half))),
    }
}

/// The outgoing tangent of half-edge `h`.
fn tangent_of<B: Backend>(d: &Dcel<B>, h: usize) -> crate::tangent::Outgoing<B> {
    let he = &d.halfedges[h];
    outgoing_tangent(&d.edges[he.edge].edge, he.dir)
}

impl<B: Backend> Dcel<B> {
    /// Build the arrangement of `edges` (spec §6 step 1). Runs the event spine to
    /// find the vertices, splits every edge at its interior vertices, merges
    /// coincident sub-edges, links twins + the azimuth rotation, and traces faces.
    pub fn build(edges: &[Edge<B>]) -> Self {
        let ev = match arrange_events(edges) {
            Verdict::Verified((ev, _coinc, _wit)) => ev,
            _ => unreachable!("degree-≤2 arrangement is always Verified"),
        };

        // 1. Vertices = the event points + every input endpoint (deduped).
        let mut verts: Vec<Point2<B>> = Vec::new();
        for v in &ev.vertices {
            vid(&mut verts, &v.point);
        }
        for e in edges {
            let (s, t) = endpoints(e);
            vid(&mut verts, s);
            vid(&mut verts, t);
        }

        // 2. Split every edge at the vertices on its (bounded) extent → raw sub-edges.
        struct Raw<B: Backend> {
            va: usize,
            vb: usize,
            edge: Edge<B>,
            src: (geom::content::CurveId, Orient),
        }
        let mut raws: Vec<Raw<B>> = Vec::new();
        for e in edges {
            // vertices on this edge, ordered along it (lexicographic = along the
            // x-monotone piece; vertical segments fall to y — see `membership`).
            let mut on: Vec<Point2<B>> = verts
                .iter()
                .filter(|p| on_carrier(p, e) && on_edge(p, e))
                .cloned()
                .collect();
            on.sort();
            on.dedup();
            let (src, orient) = source_orient(e);
            for w in on.windows(2) {
                let (p, q) = (&w[0], &w[1]);
                let sub = match e {
                    Edge::Seg(s) => sub_seg(s, p, q),
                    Edge::Arc(a) => sub_arc(a, p, q),
                };
                raws.push(Raw {
                    va: verts.iter().position(|x| x == p).unwrap(),
                    vb: verts.iter().position(|x| x == q).unwrap(),
                    edge: sub,
                    src: (src, orient),
                });
            }
        }

        // 3. Merge coincident sub-edges (same endpoint pair + same carrier).
        raws.sort_by(|a, b| {
            a.va.cmp(&b.va)
                .then_with(|| a.vb.cmp(&b.vb))
                .then_with(|| carrier_cmp(&a.edge, &b.edge))
        });
        let mut sub_edges: Vec<SubEdge<B>> = Vec::new();
        for r in raws {
            if let Some(last) = sub_edges.last_mut() {
                if last.va == r.va
                    && last.vb == r.vb
                    && carrier_cmp(&last.edge, &r.edge) == Ordering::Equal
                {
                    last.sources.push(r.src);
                    continue;
                }
            }
            sub_edges.push(SubEdge {
                edge: r.edge,
                va: r.va,
                vb: r.vb,
                sources: vec![r.src],
            });
        }

        // 4. Two half-edges per sub-edge (`2k` = va→vb, `2k+1` = vb→va).
        let mut halfedges: Vec<HalfEdge> = Vec::with_capacity(2 * sub_edges.len());
        for (k, se) in sub_edges.iter().enumerate() {
            halfedges.push(HalfEdge {
                origin: se.va,
                edge: k,
                dir: true,
                twin: 2 * k + 1,
                next: NONE,
                prev: NONE,
                cycle: NONE,
            });
            halfedges.push(HalfEdge {
                origin: se.vb,
                edge: k,
                dir: false,
                twin: 2 * k,
                next: NONE,
                prev: NONE,
                cycle: NONE,
            });
        }

        let mut d = Dcel {
            verts,
            edges: sub_edges,
            halfedges,
            n_cycles: 0,
        };
        d.link_rotation();
        d.trace_faces();
        d
    }

    /// The vertex rotation system: at each vertex sort the outgoing half-edges by the
    /// azimuth order, then `next(twin(o_i)) = o_{i−1}` (the incoming half-edge on
    /// `o_i` continues onto the next outgoing clockwise) — faces to the left, bounded
    /// cycles CCW.
    fn link_rotation(&mut self) {
        let n = self.verts.len();
        let mut by_origin: Vec<Vec<usize>> = vec![Vec::new(); n];
        for (h, he) in self.halfedges.iter().enumerate() {
            by_origin[he.origin].push(h);
        }
        for outs in &mut by_origin {
            outs.sort_by(|&h1, &h2| dir_cmp(&tangent_of(self, h1), &tangent_of(self, h2)));
            let k = outs.len();
            for i in 0..k {
                let o_i = outs[i];
                let t_i = self.halfedges[o_i].twin; // incoming to this vertex
                let o_prev = outs[(i + k - 1) % k];
                self.halfedges[t_i].next = o_prev;
                self.halfedges[o_prev].prev = t_i;
            }
        }
    }

    /// Trace the `next` cycles, assigning each half-edge its face-cycle id.
    fn trace_faces(&mut self) {
        let n = self.halfedges.len();
        let mut n_cycles = 0;
        for start in 0..n {
            if self.halfedges[start].cycle != NONE {
                continue;
            }
            let c = n_cycles;
            n_cycles += 1;
            let mut cur = start;
            loop {
                self.halfedges[cur].cycle = c;
                cur = self.halfedges[cur].next;
                if cur == start {
                    break;
                }
            }
        }
        self.n_cycles = n_cycles;
    }

    /// The **substrate-link diagnostic** (spec §6): twin pairing is a complete
    /// involution, no half-edge is dangling, and `next`/`prev` are mutually inverse
    /// with `next(h)` originating where `h` ends. Validates the overlay's own
    /// integrity — not the region certificate ([`certify_core::arrange`] owns that).
    pub fn substrate_link_ok(&self) -> bool {
        let he = &self.halfedges;
        for (h, e) in he.iter().enumerate() {
            if e.next == NONE || e.prev == NONE || e.cycle == NONE {
                return false;
            }
            if he[e.twin].twin != h {
                return false;
            }
            if he[e.next].prev != h || he[e.prev].next != h {
                return false;
            }
            // next(h) must start where h ends (= where twin(h) starts).
            if he[e.next].origin != he[e.twin].origin {
                return false;
            }
        }
        true
    }

    pub fn n_verts(&self) -> usize {
        self.verts.len()
    }
    pub fn n_edges(&self) -> usize {
        self.edges.len()
    }
}

/// Is `p` **on the carrier** of `e` — the line `a·x+b·y+c = 0` or the circle
/// `(p−C)² = r²`? Exact. [`super::membership::on_edge`] presumes on-carrier (it
/// tests only the bounded extent), so a split must gate on this first: the global
/// vertex set contains points off this edge's carrier that share its x-extent.
fn on_carrier<B: Backend>(p: &Point2<B>, e: &Edge<B>) -> bool {
    match e {
        Edge::Seg(s) => {
            p.x.scale(&s.line.a)
                .add(&p.y.scale(&s.line.b))
                .unwrap_surd()
                .add(&Surd::from_rat(s.line.c.clone()))
                .sign()
                == 0
        }
        Edge::Arc(a) => {
            let nx = p.x.sub(&Surd::from_rat(a.circle.cx.clone())).unwrap_surd();
            let ny = p.y.sub(&Surd::from_rat(a.circle.cy.clone())).unwrap_surd();
            nx.square()
                .add(&ny.square())
                .unwrap_surd()
                .sub(&Surd::from_rat(a.circle.r2.clone()))
                .sign()
                == 0
        }
    }
}

/// The two endpoints of an edge (as `(start, end)` references).
fn endpoints<B: Backend>(e: &Edge<B>) -> (&Point2<B>, &Point2<B>) {
    match e {
        Edge::Seg(s) => (&s.start, &s.end),
        Edge::Arc(a) => (&a.start, &a.end),
    }
}

/// An edge's source curve id and orientation bit (the stored face-orientation bit).
fn source_orient<B: Backend>(e: &Edge<B>) -> (geom::content::CurveId, Orient) {
    match e {
        Edge::Seg(s) => (s.source, s.orient),
        Edge::Arc(a) => (a.source, a.winding.orient),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geom::content::{Circle, Curve, CurveId, Line};
    use lattice::{Bignum, Rat};

    type Q = Rat<Bignum>;
    type P = Point2<Bignum>;

    fn rp(x: i128, y: i128) -> P {
        Point2::from_rat(Q::from_i128(x), Q::from_i128(y))
    }
    /// A segment edge between two rational points, with the exact carrier line.
    fn seg(sx: i128, sy: i128, ex: i128, ey: i128) -> Edge<Bignum> {
        let (a, b) = (Q::from_i128(-(ey - sy)), Q::from_i128(ex - sx));
        let c = a
            .mul(&Q::from_i128(sx))
            .add(&b.mul(&Q::from_i128(sy)))
            .neg();
        Edge::Seg(Box::new(SegPiece {
            line: Line { a, b, c },
            start: rp(sx, sy),
            end: rp(ex, ey),
            orient: Orient::Ccw,
            source: CurveId(0),
        }))
    }
    fn circle_edges(cx: i128, cy: i128, r2: i128, src: u32) -> Vec<Edge<Bignum>> {
        crate::decompose::decompose(&Curve::Circle {
            circle: Circle {
                cx: Q::from_i128(cx),
                cy: Q::from_i128(cy),
                r2: Q::from_i128(r2),
            },
            orient: Orient::Ccw,
            source: CurveId(src),
        })
    }

    /// Euler check: for a connected planar arrangement V − E + F = 2, and every
    /// traced cycle is a face boundary, so `n_cycles = F`.
    #[test]
    fn square_two_faces() {
        // unit square (0,0)-(2,0)-(2,2)-(0,2).
        let sq = [
            seg(0, 0, 2, 0),
            seg(2, 0, 2, 2),
            seg(2, 2, 0, 2),
            seg(0, 2, 0, 0),
        ];
        let d = Dcel::build(&sq);
        assert!(d.substrate_link_ok());
        assert_eq!(d.n_verts(), 4);
        assert_eq!(d.n_edges(), 4);
        assert_eq!(d.halfedges.len(), 8);
        assert_eq!(d.n_cycles, 2, "one bounded + the unbounded face");
    }

    /// A star / X of two crossing segments: a tree with a degree-4 centre, no bounded
    /// face — one cycle (the unbounded face), traced through every half-edge.
    #[test]
    fn crossing_segments_one_cycle() {
        let x = [seg(-2, 0, 2, 0), seg(0, -2, 0, 2)];
        let d = Dcel::build(&x);
        assert!(d.substrate_link_ok());
        assert_eq!(d.n_verts(), 5, "4 leaves + the centre");
        assert_eq!(d.n_edges(), 4, "each segment split at the centre");
        assert_eq!(d.n_cycles, 1, "a tree encloses no bounded face");
    }

    /// The load-bearing corpus config: two overlapping disks (a lens). Circles
    /// (0,0,25) and (8,0,25) meet at the rational points (4,±3). V = 2 crossings +
    /// 4 extrema, E = 8, so F = 4 (two lunes + the lens + the unbounded face).
    #[test]
    fn two_overlapping_disks() {
        let mut edges = circle_edges(0, 0, 25, 0);
        edges.extend(circle_edges(8, 0, 25, 1));
        let d = Dcel::build(&edges);
        assert!(d.substrate_link_ok(), "substrate-link integrity");
        assert_eq!(d.n_verts(), 6);
        assert_eq!(d.n_edges(), 8);
        assert_eq!(d.n_cycles, 4, "two lunes + lens + unbounded (V−E+F=2)");
    }

    /// Coincident edges (same carrier, distinct sources) merge to one sub-edge with
    /// multiplicity 2 — the "both operands" edge (spec §6 step 5). Two identical
    /// segments on the same line.
    #[test]
    fn coincident_segments_merge_with_multiplicity() {
        let mut a = seg(0, 0, 4, 0);
        // second segment, same geometry, different source.
        let b = match &a {
            Edge::Seg(s) => Edge::Seg(Box::new(SegPiece {
                source: CurveId(1),
                ..(**s).clone()
            })),
            _ => unreachable!(),
        };
        if let Edge::Seg(s) = &mut a {
            s.source = CurveId(0);
        }
        let d = Dcel::build(&[a, b]);
        assert!(d.substrate_link_ok());
        assert_eq!(
            d.n_edges(),
            1,
            "the two coincident segments merge to one edge"
        );
        assert_eq!(d.edges[0].sources.len(), 2, "covered by both sources");
    }
}
