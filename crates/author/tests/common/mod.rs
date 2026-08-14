//! Shared measurements over an **emitted** flat pattern (VV.3).
//!
//! These read the same float polylines the SVG draws — `export::svg::region_to_polys` on the
//! assembled region — rather than an intermediate the exporter might not use. A quality check on
//! geometry that never reaches the artifact would not have caught the defect these exist for.

use arrange2d::boolean::Region;
use export::svg::region_to_polys;
use lattice::Backend;

/// The **hole** rings the SVG actually draws, per face: `rings[face][hole]`.
///
/// `region_to_polys` concatenates each face's outer cycles then its hole cycles, so the split is
/// positional. That is only valid while the outer boundary flattens to exactly one cycle — which
/// the assertion here checks rather than assumes, so a future multi-cycle outer fails loudly
/// instead of silently handing back the boundary as if it were a hole.
///
/// Only holes are returned because only holes have a meaningful chord metric: an outer boundary
/// legitimately contains long straight runs (rails, the notch), so the same measurement there
/// would be noise.
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

/// The ring's **longest emitted edge as a fraction of its own size** — the defect metric.
///
/// Size is the larger bounding-box side, so for a round hole this is the diameter and the
/// fraction reads directly as "how much of the hole does one straight edge span". A closed cut
/// that a graph model must split into two branches and bridge shows up here as a single edge
/// spanning 30–48% of the hole; a curve that tracks the cut to its tangent rulings shows up as
/// ordinary chord spacing. Ring order is the traversal order, and the closing edge is included.
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
