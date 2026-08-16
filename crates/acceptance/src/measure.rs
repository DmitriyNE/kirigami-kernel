//! Measurements over an **emitted** flat pattern — the shared vocabulary the demos report and the
//! V&V pins assert.
//!
//! Every function here reads the same float polylines the SVG draws (`export::svg::region_to_polys`
//! over the assembled region) rather than an intermediate the exporter might not use, and goes
//! through the quarantined exact→`f64` bridge rather than converting coordinates by hand — a naive
//! conversion returns `NaN` on a large rational, which `min`/`max` then swallow, turning a real
//! measurement into a silent "could not measure".
//!
//! They live beside the device recipes for the reason the crate exists: a demo that reports one
//! number and a test that asserts a similarly-named other one looks green while guarding nothing.

use arrange2d::boolean::Region;
use export::approx::surd_to_f64;
use export::brep::Brep;
use export::svg::region_to_polys;
use lattice::Backend;

/// The vertex tolerance a CAD kernel reads the emitted shell with — OCCT's `Precision::Confusion`,
/// `10⁻⁷`.
///
/// Two points closer than this are the *same* point to the consumer, whatever the exact tier says.
/// An edge shorter than it is therefore a curve whose ends coincide while its two vertices do not,
/// and `BRepBuilderAPI_MakeEdge` refuses it (`DifferentPointsOnClosedCurve`) — with every
/// certificate still `Verified`, since the certificates are about the rails and say nothing about
/// what a floating-point consumer can represent.
pub const CAD_VERTEX_TOL: f64 = 1e-7;

/// The shortest emitted edge's 3-D chord — the number that decides whether a CAD kernel can
/// represent the shell at all, and the one no verdict reports.
///
/// Measured endpoint-to-endpoint because that is the comparison the consumer makes: it asks whether
/// an edge's two vertices are distinct, not how long the curve between them is. `+∞` for a shell
/// with no edges.
pub fn shortest_edge<B: Backend>(brep: &Brep<B>) -> f64 {
    let p = |i: usize| {
        let v = &brep.verts()[i];
        [surd_to_f64(&v[0]), surd_to_f64(&v[1]), surd_to_f64(&v[2])]
    };
    brep.edges()
        .iter()
        .map(|e| {
            let (a, b) = (p(e.start), p(e.end));
            ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
        })
        .fold(f64::INFINITY, f64::min)
}

/// The **hole** rings the SVG actually draws, per face: `rings[face][hole]`.
///
/// `region_to_polys` concatenates each face's outer cycles then its hole cycles, so the split is
/// positional. That is only valid while the outer boundary flattens to exactly one cycle — which
/// the assertion here checks rather than assumes, so a future multi-cycle outer fails loudly
/// instead of silently handing back the boundary as if it were a hole.
pub fn emitted_hole_rings<B: Backend>(region: &Region<B>) -> Vec<Vec<Vec<[f64; 2]>>> {
    let polys = region_to_polys(region);
    region
        .faces
        .iter()
        .zip(polys.faces)
        .map(|(face, fp)| {
            assert_eq!(
                fp.rings.len(),
                1 + face.holes.len(),
                "expected one outer cycle plus {} hole cycles, got {} rings — the positional \
                 outer/hole split in `emitted_hole_rings` no longer holds",
                face.holes.len(),
                fp.rings.len()
            );
            fp.rings.into_iter().skip(1).collect()
        })
        .collect()
}

/// The ring's **longest emitted edge as a fraction of its own size** — the VV.3 defect metric.
///
/// Size is the larger bounding-box side, so for a round hole this is the diameter and the fraction
/// reads directly as "how much of the hole does one straight edge span". A closed cut that a graph
/// model must split into two branches and bridge shows up here as a single edge spanning 30–48% of
/// the hole; a curve that tracks the cut to its tangent rulings shows up as ordinary chord spacing.
/// Ring order is the traversal order, and the closing edge is included.
///
/// Returns `0.0` for a degenerate (zero-extent or non-finite) ring rather than dividing by zero.
pub fn longest_edge_fraction(ring: &[[f64; 2]]) -> f64 {
    if ring.len() < 2 {
        return 0.0;
    }
    let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
    for p in ring {
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let extent = (hi[0] - lo[0]).max(hi[1] - lo[1]);
    if extent.is_nan() || extent <= 0.0 {
        return 0.0;
    }
    let mut longest = 0.0f64;
    for i in 0..ring.len() {
        let a = ring[i];
        let b = ring[(i + 1) % ring.len()];
        let d = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        longest = longest.max(d);
    }
    longest / extent
}

/// The ring's area, by the shoelace sum — orientation-free.
pub fn ring_area(ring: &[[f64; 2]]) -> f64 {
    let n = ring.len();
    let mut acc = 0.0f64;
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        acc += a[0] * b[1] - b[0] * a[1];
    }
    (acc / 2.0).abs()
}

/// The most times a straight ray from the flat apex `(0, 0)` crosses `ring`.
///
/// **This is the ruling-crossing signature, read off the artifact.** A cone develops by an isometry
/// that sends each ruling to a ray from the flat apex, so a ruling meeting the cutter twice is
/// exactly a ray meeting the developed hole in two intervals — four crossings. Two is what every
/// band footprint gives, however non-convex its flat shape; only a genuine multi-stretch footprint
/// gives four, which is the property AUTH.2 exists for and the one a size or reflex-corner check
/// cannot see (`docs/cutter-extrude-design.md` §11.6).
///
/// Sampled over the ring's own angular extent, so the count is of rays that actually meet it.
pub fn max_ray_crossings(ring: &[[f64; 2]]) -> usize {
    /// Rays sampled across the ring's angular extent.
    const RAYS: usize = 4001;
    let n = ring.len();
    if n < 3 {
        return 0;
    }
    let (lo, hi) = ring.iter().fold((f64::MAX, f64::MIN), |(a, b), p| {
        let t = p[1].atan2(p[0]);
        (a.min(t), b.max(t))
    });
    let mut best = 0;
    for k in 0..RAYS {
        let t = lo + (hi - lo) * (k as f64 + 0.5) / RAYS as f64;
        let (c, s) = (t.cos(), t.sin());
        let mut hits = 0;
        for i in 0..n {
            let (a, b) = (ring[i], ring[(i + 1) % n]);
            // Signed distances to the ray's line; opposite signs means the segment crosses it.
            let (ca, cb) = (a[0] * s - a[1] * c, b[0] * s - b[1] * c);
            if (ca > 0.0) == (cb > 0.0) {
                continue;
            }
            let u = ca / (ca - cb);
            let r = (a[0] + u * (b[0] - a[0])) * c + (a[1] + u * (b[1] - a[1])) * s;
            if r > 0.0 {
                hits += 1;
            }
        }
        best = best.max(hits);
    }
    best
}

/// The most times any of `rulings` crosses `ring`, each ruling given as the **flat images of two
/// domain points on it** (`µ̂ = 0` then `µ̂ = 1`, as [`Part::flat_rulings`] emits them).
///
/// [`max_ray_crossings`] is this for a chart whose support is constant: there `γ ≡ 0`, every ruling
/// image passes through the flat apex, and sampling rays from the origin *is* sampling the family.
/// The moment a region's support curves, the images stop being concurrent — each is offset by the
/// running directrix `γ(σ)` — and a ray from the origin is no longer a ruling, so the four-crossing
/// signature has to be read against the family the development actually produces.
///
/// Counted along the **whole line**, not a half-line: the two supplied points fix it, and the sheet
/// may lie on either side of `µ̂ = 0`. That is sound for the signature it measures — a ruling is a
/// full line in the domain, and the cutter's footprint sits on one side of the apex anyway.
///
/// [`Part::flat_rulings`]: author::part::Part::flat_rulings
pub fn max_ruling_crossings(ring: &[[f64; 2]], rulings: &[[[f64; 2]; 2]]) -> usize {
    let n = ring.len();
    if n < 3 {
        return 0;
    }
    let mut best = 0;
    for [o, p] in rulings {
        let (dx, dy) = (p[0] - o[0], p[1] - o[1]);
        if !(dx.is_finite() && dy.is_finite()) || (dx == 0.0 && dy == 0.0) {
            continue;
        }
        let mut hits = 0;
        for i in 0..n {
            let (a, b) = (ring[i], ring[(i + 1) % n]);
            // Signed areas against the ruling line; opposite signs means the edge crosses it.
            let side = |q: [f64; 2]| (q[0] - o[0]) * dy - (q[1] - o[1]) * dx;
            let (ca, cb) = (side(a), side(b));
            if (ca > 0.0) != (cb > 0.0) {
                hits += 1;
            }
        }
        best = best.max(hits);
    }
    best
}

/// Is the simple ring `inner` contained in the simple ring `outer`?
///
/// Containment, not vertex sampling: a vertex test alone passes on a ring that pokes out between
/// two of its own vertices. Two simple closed curves are nested exactly when they do not cross and
/// one point of the first is inside the second, so that is what is checked.
pub fn ring_inside(inner: &[[f64; 2]], outer: &[[f64; 2]]) -> bool {
    !rings_cross(inner, outer) && inner.first().is_some_and(|p| point_in_ring(*p, outer))
}

/// Are the two simple rings disjoint — neither crossing nor nested?
pub fn rings_disjoint(a: &[[f64; 2]], b: &[[f64; 2]]) -> bool {
    !rings_cross(a, b)
        && !a.first().is_some_and(|p| point_in_ring(*p, b))
        && !b.first().is_some_and(|p| point_in_ring(*p, a))
}

/// Does any edge of `a` properly cross an edge of `b`?
fn rings_cross(a: &[[f64; 2]], b: &[[f64; 2]]) -> bool {
    let side = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| {
        (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0])
    };
    for i in 0..a.len() {
        let (p, q) = (a[i], a[(i + 1) % a.len()]);
        for j in 0..b.len() {
            let (r, t) = (b[j], b[(j + 1) % b.len()]);
            if (side(p, q, r) > 0.0) != (side(p, q, t) > 0.0)
                && (side(r, t, p) > 0.0) != (side(r, t, q) > 0.0)
            {
                return true;
            }
        }
    }
    false
}

/// Even-odd point-in-ring, by a `+x` ray cast.
fn point_in_ring(p: [f64; 2], ring: &[[f64; 2]]) -> bool {
    let n = ring.len();
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1]) {
            let x = a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
            if x > p[0] {
                inside = !inside;
            }
        }
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(cx: f64, cy: f64, h: f64) -> Vec<[f64; 2]> {
        vec![
            [cx - h, cy - h],
            [cx + h, cy - h],
            [cx + h, cy + h],
            [cx - h, cy + h],
        ]
    }

    /// Nesting is decided by non-crossing plus one inside point, so a shape that pokes out between
    /// its own vertices is *not* reported as contained — the failure a vertex-only test misses.
    #[test]
    fn containment_sees_a_ring_that_pokes_out_between_its_vertices() {
        let outer = square(0.0, 0.0, 1.0);
        assert!(ring_inside(&square(0.0, 0.0, 0.5), &outer));
        // A star whose points reach past the square, but whose sampled vertices sit inside it.
        let spiky = vec![[0.0, 0.0], [0.9, 0.0], [1.5, 0.5], [0.0, 0.9]];
        assert!(!ring_inside(&spiky, &outer));
        assert!(rings_disjoint(
            &square(0.0, 0.0, 0.4),
            &square(3.0, 0.0, 0.4)
        ));
        assert!(!rings_disjoint(&square(0.0, 0.0, 0.4), &outer));
    }

    /// A convex ring is met twice by every ray from a point outside it; an L opening across the
    /// rays is met four times.
    #[test]
    fn ray_crossings_separate_a_band_from_a_two_stretch_footprint() {
        assert_eq!(max_ray_crossings(&square(5.0, 0.0, 1.0)), 2);
        // An L whose notch opens **across** the rays from the origin: a thin arm along `y ≈ 0`
        // from x = 4 to 6 and an upright arm at its far end. A shallow ray leaves the thin arm
        // through its top edge, crosses the notch, and re-enters the upright one.
        let ell = vec![
            [4.0, 0.0],
            [6.0, 0.0],
            [6.0, 2.0],
            [5.7, 2.0],
            [5.7, 0.3],
            [4.0, 0.3],
        ];
        assert_eq!(max_ray_crossings(&ell), 4);
    }
}
