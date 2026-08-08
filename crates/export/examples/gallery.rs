//! Emit `gallery.html` — a page of certified boolean regions rendered as SVG.
//!
//! For every shape in [`fixtures::gallery`], this runs the certified boolean entry
//! [`ledge_dom_certified`] under `∪`/`∩`/`△`, flattens each `Verified` region through the
//! quarantined exact→`f64` bridge, and writes a self-contained gallery page.
//!
//! Run it with the `diagnostics` feature (the only place floats are allowed):
//!
//! ```text
//! cargo run --example gallery --features diagnostics
//! ```
//!
//! It writes `gallery.html` in the current directory — open it in a browser.

use arrange2d::boolean::{BoolOp, ledge_dom_certified};
use certify_core::Verdict;
use export::svg::{GalleryItem, GalleryView, bounds_of_edges, gallery_html, region_svg};
use fixtures::gallery;

/// The three ops, with the caption each view carries.
const OPS: [(BoolOp, &str); 3] = [
    (BoolOp::Or, "∪ union"),
    (BoolOp::And, "∩ intersect"),
    (BoolOp::Xor, "△ sym-diff"),
];

/// Pixel width of each rendered view.
const VIEW_PX: u32 = 240;

fn main() {
    let shapes = gallery::all();
    let mut items = Vec::with_capacity(shapes.len());

    for shape in &shapes {
        // One frame per shape (from both input operands) so its three views share a scale.
        let frame = bounds_of_edges(&shape.edges);
        let mut views = Vec::with_capacity(OPS.len());
        for (op, label) in OPS {
            let svg = match ledge_dom_certified(&shape.edges, &shape.operand_of, op) {
                Verdict::Verified(cap) => region_svg(cap.region(), &frame, VIEW_PX),
                _ => panic!(
                    "gallery shape `{}` did not certify under {op:?}",
                    shape.name
                ),
            };
            views.push(GalleryView {
                label: label.to_string(),
                svg,
            });
        }
        items.push(GalleryItem {
            name: shape.name.to_string(),
            blurb: shape.blurb.to_string(),
            views,
        });
    }

    let html = gallery_html("kirigami-kernel — certified boolean gallery", &items);
    let path = "gallery.html";
    std::fs::write(path, &html).expect("write gallery.html");
    println!(
        "wrote {path} — {} shapes × {} ops, {} bytes",
        shapes.len(),
        OPS.len(),
        html.len(),
    );
}
