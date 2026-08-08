//! 2D SVG rendering of certified boolean regions — diagnostics only.
//!
//! A [`Region`] carries no floats; a browser needs pixels. This module flattens a
//! region's exact boundary loops to `f64` polylines through the quarantined
//! [`approx`](crate::approx) bridge, then serialises each face as one `<svg>` `<path>`
//! (all of its rings, `fill-rule="evenodd"`). Nothing here ever feeds a predicate:
//! floats appear at the last moment, for display, exactly as [`approx`](crate::approx)
//! prescribes.
//!
//! The one non-float decision is *edge orientation and grouping*: a boundary loop's edges
//! arrive as an unordered bag whose stored `start`/`end` may run either way, and a loop that
//! **pinches** (a tangency, a figure-eight) is emitted as several cycles sharing a vertex.
//! So each loop is decomposed into closed cycles by chaining on **exact** [`Point2`]
//! equality — never joining two edges that do not share a vertex — and every cycle is drawn
//! as an `evenodd` subpath, letting nesting and pinches resolve themselves. See
//! [`region_to_polys`].
//!
//! [`gallery_html`] assembles a page of these SVGs; the `gallery` example drives it.

use crate::approx::{rat_to_f64, surd_to_f64};
use arrange2d::boolean::Region;
use geom::content::{ArcPiece, Edge, Half, Point2};
use lattice::Backend;

/// Interior samples per arc edge when flattening (endpoints are added exactly on top).
const ARC_SAMPLES: usize = 24;

/// One face flattened to float polylines: all of its boundary rings, outer and hole alike.
///
/// Each ring is a closed polyline (the first vertex is not repeated at the end — closure
/// is implied). The whole set is drawn as a single `evenodd` path, so an interior ring is
/// cut out and a pinched loop's several cycles compose without a spurious joining chord —
/// the outer/hole distinction is topological and irrelevant to the even-odd fill.
#[derive(Clone, Debug)]
pub struct FacePolys {
    /// The face's boundary rings, as `[x, y]` float vertices — outer cycles and hole cycles.
    pub rings: Vec<Vec<[f64; 2]>>,
}

/// A whole region flattened to float polylines — one [`FacePolys`] per connected face.
#[derive(Clone, Debug)]
pub struct RegionPolys {
    /// One entry per face of the region, in the region's own order.
    pub faces: Vec<FacePolys>,
}

/// An axis-aligned float bounding box, used to fit an SVG `viewBox`.
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    /// Minimum x.
    pub minx: f64,
    /// Minimum y.
    pub miny: f64,
    /// Maximum x.
    pub maxx: f64,
    /// Maximum y.
    pub maxy: f64,
}

impl Bounds {
    /// The tightest box covering `points` (an empty iterator yields an inverted/empty box).
    pub fn of_points<I: IntoIterator<Item = [f64; 2]>>(points: I) -> Bounds {
        let mut b = Bounds {
            minx: f64::INFINITY,
            miny: f64::INFINITY,
            maxx: f64::NEG_INFINITY,
            maxy: f64::NEG_INFINITY,
        };
        for p in points {
            b.include(p);
        }
        b
    }

    fn include(&mut self, [x, y]: [f64; 2]) {
        if x < self.minx {
            self.minx = x;
        }
        if y < self.miny {
            self.miny = y;
        }
        if x > self.maxx {
            self.maxx = x;
        }
        if y > self.maxy {
            self.maxy = y;
        }
    }

    fn width(&self) -> f64 {
        self.maxx - self.minx
    }

    fn height(&self) -> f64 {
        self.maxy - self.miny
    }

    /// Grow the box outward by `frac` of each side (a display margin); degenerate sides
    /// are floored so a zero-extent region still yields a drawable frame.
    fn padded(&self, frac: f64) -> Bounds {
        let w = self.width().max(1e-9);
        let h = self.height().max(1e-9);
        Bounds {
            minx: self.minx - w * frac,
            miny: self.miny - h * frac,
            maxx: self.maxx + w * frac,
            maxy: self.maxy + h * frac,
        }
    }
}

/// The bounding box of a slice of input edges (both operands of a shape) — the stable
/// frame all of that shape's `△`/`∩`/`∪` outputs are drawn into.
pub fn bounds_of_edges<B: Backend>(edges: &[Edge<B>]) -> Bounds {
    Bounds::of_points(edges.iter().flat_map(|e| edge_points(e, true)))
}

/// Flatten a certified region to float polylines (diagnostics only).
///
/// Each face's boundary loops (its outer loop and any holes) are decomposed into closed
/// cycles by chaining edges on **exact** [`Point2`] equality (deciding each edge's direction
/// without a float comparison, and splitting a pinched loop at its shared vertex), then arcs
/// are sampled and endpoints read through the [`approx`](crate::approx) bridge. A face's
/// cycles — outer and hole alike — are pooled into one [`FacePolys`], drawn `evenodd`.
pub fn region_to_polys<B: Backend>(region: &Region<B>) -> RegionPolys {
    RegionPolys {
        faces: region
            .faces
            .iter()
            .map(|f| {
                let mut rings = flatten_cycles(&f.outer);
                for h in &f.holes {
                    rings.extend(flatten_cycles(h));
                }
                FacePolys { rings }
            })
            .collect(),
    }
}

/// The two stored endpoints of an edge, `(start, end)`.
fn endpoints<B: Backend>(e: &Edge<B>) -> (&Point2<B>, &Point2<B>) {
    match e {
        Edge::Seg(s) => (&s.start, &s.end),
        Edge::Arc(a) => (&a.start, &a.end),
    }
}

/// The endpoint reached traversing `e` in the given direction (`forward` ⇒ its `end`).
fn far_end<B: Backend>(e: &Edge<B>, forward: bool) -> &Point2<B> {
    let (s, t) = endpoints(e);
    if forward { t } else { s }
}

/// A `Point2` as a display `[x, y]` float pair.
fn pt<B: Backend>(p: &Point2<B>) -> [f64; 2] {
    [surd_to_f64(&p.x), surd_to_f64(&p.y)]
}

/// Float vertices of a single edge from its `start` to its `end` (arcs sampled), reversed
/// when `forward` is false so the ring chains head-to-tail.
fn edge_points<B: Backend>(e: &Edge<B>, forward: bool) -> Vec<[f64; 2]> {
    let mut p = match e {
        Edge::Seg(s) => vec![pt(&s.start), pt(&s.end)],
        Edge::Arc(a) => arc_points(a),
    };
    if !forward {
        p.reverse();
    }
    p
}

/// Sample an x-monotone arc piece from its `start` to its `end`, **uniformly in angle**
/// (not in x) so a circle's near-vertical left/right renders as smoothly as its flat
/// top/bottom. Endpoints are read exactly; interior points ride the circle itself,
/// `(cx + r·cos θ, cy + r·sin θ)`, interpolating θ between the two endpoint angles.
///
/// Within one [`Half`] the endpoint angles need no unwrapping — a `Lower` arc lives in
/// `θ ∈ [−π, 0]`, an `Upper` arc in `[0, π]` — except that `atan2` returns `+π` at a
/// `Lower` arc's left extreme, so it is pulled down to `−π` for the interpolation to trace
/// the lower arc rather than its reflection.
fn arc_points<B: Backend>(a: &ArcPiece<B>) -> Vec<[f64; 2]> {
    use core::f64::consts::PI;
    let cx = rat_to_f64(&a.circle.cx);
    let cy = rat_to_f64(&a.circle.cy);
    let r = rat_to_f64(&a.circle.r2).max(0.0).sqrt();
    let [sx, sy] = pt(&a.start);
    let [ex, ey] = pt(&a.end);
    let mut th0 = (sy - cy).atan2(sx - cx);
    let mut th1 = (ey - cy).atan2(ex - cx);
    if let Half::Lower = a.half {
        if th0 > 0.0 {
            th0 -= 2.0 * PI;
        }
        if th1 > 0.0 {
            th1 -= 2.0 * PI;
        }
    }
    let mut out = Vec::with_capacity(ARC_SAMPLES + 1);
    out.push([sx, sy]);
    for i in 1..ARC_SAMPLES {
        let t = i as f64 / ARC_SAMPLES as f64;
        let th = th0 + (th1 - th0) * t;
        out.push([cx + r * th.cos(), cy + r * th.sin()]);
    }
    out.push([ex, ey]);
    out
}

/// Decompose one boundary loop's edges into closed float rings.
///
/// The edges are treated as an unordered bag. Starting from each not-yet-used edge, the walk
/// greedily extends by picking any unused edge sharing the current endpoint (by **exact**
/// [`Point2`] equality) and orienting it to continue, until it returns to the cycle's start;
/// then a fresh cycle begins on the remaining edges. Two edges are joined only when they
/// truly meet, so a pinched loop (the kernel emits, e.g., internal tangency as circle-A's
/// arcs then circle-B's arcs — one boundary component, two cycles sharing the pinch vertex)
/// splits into its cycles instead of being bridged by a spurious chord across the gap.
///
/// A well-formed face boundary has every vertex at even degree, so this consumes every edge
/// into closed cycles; the `break` on no match is a guard against malformed input, not an
/// expected path.
fn flatten_cycles<B: Backend>(edges: &[Edge<B>]) -> Vec<Vec<[f64; 2]>> {
    let n = edges.len();
    let mut used = vec![false; n];
    let mut cycles = Vec::new();

    for start in 0..n {
        if used[start] {
            continue;
        }
        // Open a cycle on `edges[start]`, oriented start→end (direction is arbitrary — a
        // cycle is symmetric, so chaining from either end returns to the other).
        used[start] = true;
        let (cycle_start, _) = endpoints(&edges[start]);
        let mut ring = edge_points(&edges[start], true);
        let mut cur_end = far_end(&edges[start], true);

        while *cur_end != *cycle_start {
            // Find any unused edge incident to `cur_end`.
            let next = (0..n).find(|&j| {
                if used[j] {
                    return false;
                }
                let (a, b) = endpoints(&edges[j]);
                *a == *cur_end || *b == *cur_end
            });
            let Some(j) = next else { break };
            used[j] = true;
            let (a, _) = endpoints(&edges[j]);
            let forward = *a == *cur_end;
            let pts = edge_points(&edges[j], forward);
            // pts[0] duplicates the running end — skip it.
            ring.extend_from_slice(&pts[1..]);
            cur_end = far_end(&edges[j], forward);
        }
        cycles.push(ring);
    }
    cycles
}

// --- SVG serialisation ------------------------------------------------------------------

/// Format a display coordinate — three decimals is ample for a viewer, and keeps the
/// emitted path strings compact.
fn fmt(x: f64) -> String {
    format!("{x:.3}")
}

/// One ring as an SVG path data fragment `M … L … Z` (empty for an empty ring).
fn ring_path(ring: &[[f64; 2]]) -> String {
    if ring.is_empty() {
        return String::new();
    }
    let mut d = String::new();
    for (i, p) in ring.iter().enumerate() {
        d.push_str(if i == 0 { "M " } else { "L " });
        d.push_str(&fmt(p[0]));
        d.push(' ');
        d.push_str(&fmt(p[1]));
        d.push(' ');
    }
    d.push('Z');
    d
}

/// Render already-extracted [`RegionPolys`] as a self-contained `<svg>` fitted to `frame`
/// (plus a small margin), at `px` pixels wide. Each face is one `evenodd` path so holes
/// are cut out. The `viewBox` is in model coordinates with a y-flip, so math-up is
/// visually up.
///
/// ```
/// use export::svg::{polys_svg, Bounds, FacePolys, RegionPolys};
/// let tri = vec![[0.0, 0.0], [2.0, 0.0], [1.0, 2.0]];
/// let polys = RegionPolys { faces: vec![FacePolys { rings: vec![tri.clone()] }] };
/// let svg = polys_svg(&polys, &Bounds::of_points(tri), 120);
/// assert!(svg.starts_with("<svg"));
/// assert!(svg.contains("<path"));
/// assert!(svg.contains("fill-rule=\"evenodd\""));
/// ```
pub fn polys_svg(polys: &RegionPolys, frame: &Bounds, px: u32) -> String {
    let fr = frame.padded(0.08);
    let w = fr.width();
    let h = fr.height();
    let wpx = f64::from(px);
    let hpx = (wpx * (h / w)).round().max(1.0);
    // matrix(1 0 0 -1 0 flip) maps y ↦ (miny+maxy) − y: a vertical flip that stays in-box.
    let flip = fr.miny + fr.maxy;
    let sw = fmt(w * 0.006);

    let mut paths = String::new();
    for face in &polys.faces {
        let mut d = String::new();
        for (i, ring) in face.rings.iter().enumerate() {
            if i > 0 {
                d.push(' ');
            }
            d.push_str(&ring_path(ring));
        }
        paths.push_str(&format!(
            "<path d=\"{d}\" fill=\"#5b8def\" fill-opacity=\"0.35\" \
             fill-rule=\"evenodd\" stroke=\"#1f3b8c\" stroke-width=\"{sw}\" \
             stroke-linejoin=\"round\"/>"
        ));
    }

    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{wpx}\" height=\"{hpx}\" \
         viewBox=\"{minx} {miny} {w} {h}\">\
         <g transform=\"matrix(1 0 0 -1 0 {flip})\">{paths}</g></svg>",
        wpx = fmt(wpx),
        hpx = fmt(hpx),
        minx = fmt(fr.minx),
        miny = fmt(fr.miny),
        w = fmt(w),
        h = fmt(h),
    )
}

/// Render a certified region to an `<svg>` fitted to `frame` at `px` pixels wide —
/// [`region_to_polys`] then [`polys_svg`].
pub fn region_svg<B: Backend>(region: &Region<B>, frame: &Bounds, px: u32) -> String {
    polys_svg(&region_to_polys(region), frame, px)
}

// --- gallery page ------------------------------------------------------------------------

/// One rendered view of a shape (e.g. its `∩` result) with a caption.
#[derive(Clone, Debug)]
pub struct GalleryView {
    /// Caption shown under the SVG (e.g. `"∩ intersect"`).
    pub label: String,
    /// The `<svg>` markup, as produced by [`region_svg`].
    pub svg: String,
}

/// One gallery card: a named shape, its blurb, and its rendered views.
#[derive(Clone, Debug)]
pub struct GalleryItem {
    /// The shape's stable name (card heading).
    pub name: String,
    /// One-line description of the configuration.
    pub blurb: String,
    /// The rendered views (typically `∪`/`∩`/`△`).
    pub views: Vec<GalleryView>,
}

/// Escape a string for HTML text/attribute context.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Minimal self-contained page CSS — a responsive grid of cards, each a row of views.
const CSS: &str = "\
:root{color-scheme:light}\
body{font:15px/1.5 system-ui,sans-serif;margin:2rem;color:#1a1a1a;background:#fafafc}\
h1{font-size:1.4rem}\
.lede{max-width:60rem;color:#444}\
main{display:grid;gap:1.5rem;grid-template-columns:repeat(auto-fill,minmax(20rem,1fr))}\
.card{border:1px solid #e2e2ea;border-radius:10px;padding:1rem;background:#fff}\
.card h2{font-size:1.05rem;margin:0 0 .25rem}\
.blurb{color:#555;margin:0 0 .75rem;font-size:.9rem}\
.views{display:flex;gap:.5rem;flex-wrap:wrap}\
.view{margin:0;text-align:center}\
.svgwrap{border:1px solid #eee;border-radius:6px;background:#fff}\
figcaption{font-size:.8rem;color:#666;margin-top:.25rem}\
code{background:#eef;padding:0 .2em;border-radius:3px}";

/// Assemble a full HTML page from gallery items — a grid of cards, one per shape, each
/// showing its captioned views. Returns a complete, self-contained document.
pub fn gallery_html(title: &str, items: &[GalleryItem]) -> String {
    let mut cards = String::new();
    for it in items {
        let mut views = String::new();
        for v in &it.views {
            views.push_str(&format!(
                "<figure class=\"view\"><div class=\"svgwrap\">{svg}</div>\
                 <figcaption>{label}</figcaption></figure>",
                svg = v.svg,
                label = esc(&v.label),
            ));
        }
        cards.push_str(&format!(
            "<section class=\"card\"><h2>{name}</h2><p class=\"blurb\">{blurb}</p>\
             <div class=\"views\">{views}</div></section>",
            name = esc(&it.name),
            blurb = esc(&it.blurb),
        ));
    }
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>{title}</title><style>{CSS}</style></head><body>\
         <h1>{title}</h1><p class=\"lede\">Every region below is a <strong>certified</strong> \
         boolean output (CAP-OUT <code>Verified</code>), flattened to pixels through the \
         quarantined exact→<code>f64</code> bridge — floats touch the display only, never a \
         predicate.</p><main>{cards}</main></body></html>",
        title = esc(title),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrange2d::boolean::{BoolOp, ledge_dom_certified};
    use certify_core::Verdict;
    use fixtures::gallery;

    /// Extract the certified region for one shape+op, or panic with the shape name.
    fn polys_for(shape: &gallery::Shape, op: BoolOp) -> RegionPolys {
        match ledge_dom_certified(&shape.edges, &shape.operand_of, op) {
            Verdict::Verified(cap) => region_to_polys(cap.region()),
            _ => panic!(
                "gallery shape `{}` did not certify under {op:?}",
                shape.name
            ),
        }
    }

    /// Longest chord between consecutive vertices of a ring (its closing edge included).
    /// Finely-sampled arcs give short chords; a spurious joining chord shows up as a big one.
    fn max_chord(ring: &[[f64; 2]]) -> f64 {
        let mut m = 0.0f64;
        for i in 0..ring.len() {
            let a = ring[i];
            let b = ring[(i + 1) % ring.len()];
            m = m.max(((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt());
        }
        m
    }

    /// The annulus `△` is one face carrying two rings (outer disk + inner hole); its `∪` is
    /// one face with a single ring (a filled disk).
    #[test]
    fn annulus_xor_has_ring_and_hole() {
        let a = gallery::annulus();
        let xor = polys_for(&a, BoolOp::Xor);
        assert_eq!(xor.faces.len(), 1, "annulus △ is one face");
        assert_eq!(
            xor.faces[0].rings.len(),
            2,
            "annulus △ is an outer ring plus one hole ring"
        );

        let or = polys_for(&a, BoolOp::Or);
        assert_eq!(or.faces.len(), 1);
        assert_eq!(
            or.faces[0].rings.len(),
            1,
            "annulus ∪ is a single filled disk"
        );
    }

    /// Internal tangency `△` (`A∖B`, `B` tangent inside `A` at `(2,0)`) is a crescent that
    /// pinches to a point at the tangency. The kernel emits its boundary as one face whose
    /// loop is really two cycles — circle `A` then circle `B` — sharing the pinch vertex.
    /// The flattener must split them into two rings, *not* bridge `A`'s far side to `B` with
    /// a diameter-spanning chord (the old spike). Guard all three facts: two rings, no long
    /// chord (finely-sampled arcs only), and every vertex inside the outer disk `A`.
    #[test]
    fn internal_tangency_xor_is_two_clean_cycles() {
        let it = gallery::internal_tangency();
        let xor = polys_for(&it, BoolOp::Xor);
        assert_eq!(xor.faces.len(), 1, "internal-tangency △ is one face");
        let face = &xor.faces[0];
        assert_eq!(
            face.rings.len(),
            2,
            "crescent is circle A plus circle B as two cycles, not one bridged loop"
        );
        for ring in &face.rings {
            // Circle A has radius 2; its finest-sampled chord is well under 1. A spike back
            // across the disk would be ~4. This is the direct regression guard.
            assert!(
                max_chord(ring) < 1.0,
                "internal-tangency △ ring has a spurious long chord (spike): {}",
                max_chord(ring)
            );
            for &[x, y] in ring {
                assert!(
                    x * x + y * y <= 4.0 + 1e-6,
                    "internal-tangency △ vertex ({x}, {y}) escapes outer disk A"
                );
            }
        }
    }

    /// Across every gallery shape and op, each face has at least one ring and every ring is a
    /// non-degenerate closed polyline (≥3 vertices).
    #[test]
    fn every_ring_is_nonempty() {
        for shape in gallery::all() {
            for op in [BoolOp::Xor, BoolOp::And, BoolOp::Or] {
                let polys = polys_for(&shape, op);
                for face in &polys.faces {
                    assert!(
                        !face.rings.is_empty(),
                        "shape `{}` {op:?}: face has no rings",
                        shape.name
                    );
                    for ring in &face.rings {
                        assert!(
                            ring.len() >= 3,
                            "shape `{}` {op:?}: degenerate ring",
                            shape.name
                        );
                    }
                }
            }
        }
    }

    /// A rendered shape produces well-formed SVG with a `<path>` per face.
    #[test]
    fn svg_is_well_formed() {
        let sq = gallery::two_squares();
        let frame = bounds_of_edges(&sq.edges);
        let and = polys_for(&sq, BoolOp::And);
        let svg = polys_svg(&and, &frame, 200);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert_eq!(svg.matches("<path").count(), and.faces.len());
    }

    /// The page assembler emits a complete document embedding each view's SVG and caption.
    #[test]
    fn gallery_html_embeds_views() {
        let items = vec![GalleryItem {
            name: "two-squares".into(),
            blurb: "demo".into(),
            views: vec![GalleryView {
                label: "∩ intersect".into(),
                svg: "<svg id=\"probe\"></svg>".into(),
            }],
        }];
        let html = gallery_html("gallery", &items);
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("two-squares"));
        assert!(html.contains("∩ intersect"));
        assert!(html.contains("<svg id=\"probe\">"));
    }
}
