//! **IO.1 acceptance (the part that does not need a customer file).**
//!
//! The milestone's real gate is a device outline from the user cutting a part end to end. Until
//! that file exists, this is the strongest statement available: the outline a **file** produces and
//! the outline the acceptance device is **already cut with** are the same profile — same edges,
//! same arcs, same `r²`, same fill — so everything already proved about `acceptance::contour_panel`
//! transfers to the imported route without re-running it.
//!
//! It also pins the distinction the two formats draw, which is easy to state backwards: **both
//! import at `δ = 0`, and they mean different things by it.**

use arrange2d::locate::winding_parity;
use geom::content::Edge;
use interchange::svg::{SvgOptions, read_svg};
use interchange::unit::Unit;
use lattice::{Bignum, Rat};

type Q = Rat<Bignum>;

fn decimal(q: &Q, places: usize) -> String {
    interchange::num::to_decimal(q, places)
}

/// The circle a profile's arc edges sit on, as the set of distinct `r²`.
fn arc_radii(edges: &[Edge<Bignum>]) -> Vec<Q> {
    let mut out: Vec<Q> = Vec::new();
    for e in edges {
        if let Edge::Arc(a) = e
            && !out.contains(&a.circle.r2)
        {
            out.push(a.circle.r2.clone());
        }
    }
    out
}

/// **The acceptance device's own outline, read out of a file.**
///
/// `acceptance::contour_outline_geometry` is the rounded rectangle `contour_panel` is cut with —
/// centre `(0, 11/5)`, half-extents `(1/4, 1/5)`, corner radius `1/10`. Written as an SVG `<rect>`
/// with `rx`, it must import to *that* outline: not to something within a tolerance of it, to it.
#[test]
fn a_file_produces_the_outline_the_acceptance_device_is_cut_with() {
    let (cx, cy, w, h, r) = acceptance::contour_outline_geometry();
    let authored =
        acceptance::rounded_outline(cx.clone(), cy.clone(), w.clone(), h.clone(), r.clone());

    // The same rectangle in SVG's y-down space. The viewBox is 5 tall, so the reader's flip sends
    // svg-y ↦ 5 − y; the rect therefore sits at 5 − (cy + h) with the same height.
    let x = cx.sub(&w);
    let top = Q::from_i128(5).sub(&cy.add(&h));
    let doc = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="2mm" height="5mm" viewBox="-1 0 2 5">
             <rect x="{x}" y="{top}" width="{ww}" height="{hh}" rx="{r}" ry="{r}"/>
           </svg>"#,
        x = decimal(&x, 6),
        top = decimal(&top, 6),
        ww = decimal(&w.mul(&Q::from_i128(2)), 6),
        hh = decimal(&h.mul(&Q::from_i128(2)), 6),
        r = decimal(&r, 6),
    );

    let read = read_svg::<Bignum>(&doc, &SvgOptions::default()).expect("the device outline");
    assert!(read.report.is_exact(), "{}", read.report.summary());
    assert_eq!(read.report.closure_gap, Q::from_i128(0));
    let imported = read.profile().into_edges();

    // Same decomposition, same circles.
    assert_eq!(
        imported.len(),
        authored.len(),
        "imported {} edges against the authored {}",
        imported.len(),
        authored.len()
    );
    let (ri, ra) = (arc_radii(&imported), arc_radii(&authored));
    assert_eq!(ri.len(), 1, "one corner radius");
    assert_eq!(ri, ra, "r² must be the authored 1/100 exactly");

    // Same *region*: sampled on a grid fine enough to straddle every corner arc, the two outlines
    // agree point for point under the arrangement's own fill rule. A corner silently squared off,
    // or a radius off by a hair, separates here.
    let mut probed = 0usize;
    for i in -30i128..=30 {
        for j in -25i128..=25 {
            let px = cx.add(&Q::new(i, 100));
            let py = cy.add(&Q::new(j, 100));
            assert_eq!(
                winding_parity(&px, &py, &imported),
                winding_parity(&px, &py, &authored),
                "fill differs at ({}, {})",
                decimal(&px, 4),
                decimal(&py, 4)
            );
            probed += 1;
        }
    }
    assert!(probed > 3000, "the probe grid must actually be dense");
}

/// **Both formats import at `δ = 0`, and that means two different things.**
///
/// An SVG `<rect rx>` states the *shape*: its corner endpoints are the axis-aligned tangent points,
/// so the arcs come out exactly tangent to the sides. A DXF bulge states the *curve*: `tan(Δθ/4)`
/// for a quarter turn is `√2 − 1`, which no file can write down, so a real file carries a decimal
/// near it and the import reproduces **that** curve exactly — faithful to the file, and not quite
/// the tangent quarter-circle its author had in mind.
///
/// `δ = 0` is a statement about the translator, never a promise that the file said what was meant.
#[test]
fn a_bulge_reproduces_the_file_s_curve_and_a_rect_reproduces_the_shape() {
    use interchange::dxf::{DxfOptions, read_dxf};
    use interchange::element::Element;

    // A 1 × 1 square with 1/4 corners, written the way a CAD tool writes one: the quarter-turn
    // bulge rounded to ten decimals.
    let mut e = String::from("0\nLWPOLYLINE\n8\n0\n90\n8\n70\n1\n");
    for (x, y, b) in [
        ("0.25", "0.0", "0.0"),
        ("0.75", "0.0", "0.4142135624"),
        ("1.0", "0.25", "0.0"),
        ("1.0", "0.75", "0.4142135624"),
        ("0.75", "1.0", "0.0"),
        ("0.25", "1.0", "0.4142135624"),
        ("0.0", "0.75", "0.0"),
        ("0.0", "0.25", "0.4142135624"),
    ] {
        e += &format!("10\n{x}\n20\n{y}\n42\n{b}\n");
    }
    let dxf = format!(
        "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n\
         0\nSECTION\n2\nENTITIES\n{e}0\nENDSEC\n0\nEOF\n"
    );
    let read = read_dxf::<Bignum>(&dxf, &DxfOptions::default()).expect("a rounded square");
    assert!(read.report.is_exact(), "the translator moved nothing");

    // The corner arcs came out on a circle of radius *very near* 1/4 — but not 1/4, because the
    // file's bulge is not exactly tan(22.5°). The difference is the FILE's rounding, and the
    // importer neither hides it nor charges itself for it.
    let mut off_by = Q::from_i128(0);
    let quarter_squared = Q::new(1, 16);
    for el in &read.loops[0] {
        if let Element::Arc(a) = el {
            assert!(a.is_consistent());
            let d = a.r2.sub(&quarter_squared);
            let d = if d.sign() < 0 { d.neg() } else { d };
            if d > off_by {
                off_by = d;
            }
        }
    }
    assert!(
        off_by.sign() > 0,
        "a rounded bulge is not exactly a quarter turn"
    );
    assert!(
        off_by < Q::new(1, 1_000_000_000),
        "…but it is the file's ten decimals, not a translator error: {}",
        decimal(&off_by, 15)
    );

    // The SVG rect route states the shape instead, so its r² is exactly the authored one.
    let doc = r#"<svg xmlns="http://www.w3.org/2000/svg" width="1mm" height="1mm"
                      viewBox="0 0 1 1">
                   <rect x="0" y="0" width="1" height="1" rx="0.25" ry="0.25"/>
                 </svg>"#;
    let svg = read_svg::<Bignum>(doc, &SvgOptions::default()).expect("a rounded square");
    for el in &svg.loops[0] {
        if let Element::Arc(a) = el {
            assert_eq!(a.r2, quarter_squared, "the tangent construction is exact");
        }
    }
}

/// A file in inches produces a part 25.4× the one in millimetres — exactly, vertex for vertex.
/// This is the control that makes the unit path a tested claim rather than a plausible one.
#[test]
fn the_unit_is_a_factor_and_not_an_approximation() {
    let body = r#"<rect x="0" y="0" width="1" height="1"/>"#;
    let mm = read_svg::<Bignum>(
        &format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="4mm" height="4mm"
                    viewBox="0 0 4 4">{body}</svg>"#
        ),
        &SvgOptions::default(),
    )
    .expect("mm");
    let inch = read_svg::<Bignum>(
        &format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="4in" height="4in"
                    viewBox="0 0 4 4">{body}</svg>"#
        ),
        &SvgOptions {
            target: Unit::Millimetre,
            ..SvgOptions::default()
        },
    )
    .expect("in");

    let corners = |r: &interchange::Imported<Bignum>| -> Vec<[Q; 2]> {
        let mut v: Vec<[Q; 2]> = r.loops[0].iter().filter_map(|e| e.start()).collect();
        v.sort_by(|a, b| a[0].cmp(&b[0]).then(a[1].cmp(&b[1])));
        v
    };
    let (a, b) = (corners(&mm), corners(&inch));
    assert_eq!(a.len(), b.len());
    for (p, q) in a.iter().zip(&b) {
        assert_eq!(q[0], p[0].mul(&Q::new(127, 5)), "x scaled exactly");
        assert_eq!(q[1], p[1].mul(&Q::new(127, 5)), "y scaled exactly");
    }
}
