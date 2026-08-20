//! **IO.1 acceptance — the file boundary, against a real drawing.**
//!
//! The milestone's gate was always a device outline from the user. `data/inner-cut.dxf` is that
//! file — the Ø 8 bore with a 10° tab reaching in to Ø 4, eight `ARC`/`LINE` entities out of a real
//! CAD tool — and [`a_real_device_drawing_reads_as_one_exact_loop`] is what it earns. What it cost
//! is a construction the module had specified and left unbuilt: four of its junctions are **arc to
//! arc**, where neither endpoint is free to move.
//!
//! Alongside it, the statement that stood in for a customer file and is still worth making: the
//! outline a file produces and the outline the acceptance device is **already cut with** are the
//! same profile — same edges, same arcs, same `r²`, same fill.
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

/// **The device's other cut file — and the one place a drawing and a recipe meet at `δ = 0`.**
///
/// `data/outer-cut.dxf` is the rim: six entities forming the Ø 21.5 circle interrupted over 15°
/// about `+y` by a lug reaching out to Ø 27.5 — the gauge arc, two radial flanks, an R 1.5875 nose
/// and, since the drawings' G1 revision, an R 0.3 root fillet where each flank meets the rim. The
/// bore file has come to the same six: the two profiles are now the same shape problem entity for
/// entity. Two things about this one are load-bearing downstream and neither is a tolerance:
///
/// * **the rim arc carries the recipe's own `outer_r = 43/4`, to the importer's published δ.** So
///   swapping the authored disc for the file changes the *shape* and not the gauge, and
///   `lapped::normal_cut`'s cast is the same cast. A drawing that agreed only to nine decimals
///   would have been a different part with a plausible-looking diff.
///
///   It used to be `r² == 1849/16` on the nose, and the G1 revision is what ended that: with an
///   R 0.3 root fillet at each end of the rim its junctions became **arc-to-arc**, where both sides
///   own their endpoints and neither may be moved, so the rim is the side that gets re-gauged —
///   `r²` lands 1.59e-12 high, a radius moved by 7.4e-14 mm. Before the fillets it met `LINE`
///   endpoints, which the importer may slide for free, and kept its stated radius bit-for-bit.
///   The claim that survives is the stronger one to state anyway: the deviation is bounded by the
///   importer's **own** reported `δ`, so a gauge that drifted further would still be caught.
/// * **the flanks are radial** — each one's line passes within `4·10⁻¹⁴ mm` of the axis, which is
///   the file's float noise and not an exact zero. Cast from a point on the axis a radial line
///   sweeps a *plane through the axis*, and whether a ruling lies in that plane or crosses it is
///   the whole of `author/tests/rim_notch.rs`.
#[test]
fn the_devices_rim_file_states_the_recipes_own_radius_exactly() {
    use interchange::dxf::{DxfOptions, read_dxf};
    use interchange::element::Element;
    use interchange::report::ImportFault;

    assert!(matches!(
        read_dxf::<Bignum>(acceptance::OUTER_CUT_DXF, &DxfOptions::default()),
        Err(ImportFault::UnknownUnit { .. })
    ));
    let opts = DxfOptions::<Bignum> {
        assume_unit: Some(acceptance::OUTER_CUT_UNIT),
        ..Default::default()
    };
    let read = read_dxf::<Bignum>(acceptance::OUTER_CUT_DXF, &opts).expect("the device's rim");
    assert_eq!(read.report.entities, 6);
    assert_eq!(read.report.loops, 1, "{}", read.report.summary());

    // The two error numbers stay apart here too: ours is the arc re-gauge, the file's is its own
    // closure sloppiness, and it lands on the `LINE` endpoints where moving costs nothing.
    let (delta, gap) = (&read.report.delta, &read.report.closure_gap);
    assert!(
        *delta < Q::new(1, 1_000_000_000_000i128),
        "δ {}",
        decimal(delta, 18)
    );
    assert!(
        *gap > *delta && *gap < Q::new(1, 1_000_000_000i128),
        "gap {}",
        decimal(gap, 18)
    );

    let nominal = Q::new(1849, 16);
    // A radius that moved by at most δ moves `r²` by at most `2rδ + δ²`; `22 > 2·43/4` covers both
    // terms at this δ. Tied to the *reported* δ rather than to a constant, so a file whose gauge
    // really drifted cannot pass by the importer quietly admitting a larger error.
    let r2_slack = delta.mul(&Q::from_i128(22));
    let mut rim = 0usize;
    let mut flanks = 0usize;
    for el in &read.loops[0] {
        match el {
            Element::Arc(a) => {
                assert!(a.is_consistent(), "an assembled arc left its circle");
                let off = a.r2.sub(&nominal);
                let off = if off.sign() < 0 { off.neg() } else { off };
                if off < r2_slack {
                    rim += 1;
                }
            }
            Element::Segment { start, end } => {
                // The line through `start`/`end` misses the axis by |start × end| / |end − start|.
                let cross = start[0].mul(&end[1]).sub(&end[0].mul(&start[1]));
                let cross = if cross.sign() < 0 { cross.neg() } else { cross };
                // The flanks are ≈1.31 long, so bounding the numerator alone bounds the miss.
                assert!(
                    cross < Q::new(1, 10_000_000_000_000i128),
                    "a flank is not radial: |a × b| = {}",
                    decimal(&cross, 20)
                );
                flanks += 1;
            }
            other => panic!("unexpected element {other:?}"),
        }
    }
    assert_eq!(
        rim, 1,
        "one arc on the recipe's own Ø 21.5 circle, r² = 1849/16 to within the re-gauge"
    );
    assert_eq!(flanks, 2, "two radial flanks");

    // The shape, by the arrangement's own fill rule: the disc out to 10.75 everywhere, and out to
    // 13.75 only inside the lug's wedge.
    let edges = read.profile().into_edges();
    let at = |r: f64, deg: f64| -> bool {
        let (x, y) = (r * deg.to_radians().cos(), r * deg.to_radians().sin());
        let f = |v: f64| Q::new((v * 1_000_000.0) as i128, 1_000_000);
        winding_parity(&f(x), &f(y), &edges)
    };
    assert!(at(10.0, 0.0) && at(10.0, 210.0), "the rim disc is material");
    assert!(!at(11.0, 0.0) && !at(11.0, 270.0), "…and stops at 10.75");
    assert!(
        at(11.0, 90.0) && at(13.7, 90.0),
        "the lug reaches out to 13.75"
    );
    assert!(!at(13.9, 90.0), "…and no further");
    assert!(
        !at(11.0, 82.0) && !at(11.0, 98.0),
        "the lug is only 15° wide"
    );
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

/// **The device's own cut file, read end to end.**
///
/// `data/inner-cut.dxf` is what a CAD tool actually emits: eight entities, no `$INSUNITS`, and
/// coordinates carrying the tool's own float noise (its Ø 8 bore is stated as `r = 3.999999907`,
/// 93 pm short — which the importer carries rather than tidies, because a decimal literal *is* a
/// rational and rounding it to `4` would be the translator inventing geometry).
///
/// Three things it pins, each of which was a real edge of the design:
///
/// * **the unit is supplied, never inferred.** The file has `$MEASUREMENT 1`, which says *metric*
///   and not *millimetre* — those are different claims, and the reader refuses the difference
///   rather than guessing a 10× part.
/// * **four arc-to-arc junctions assemble.** Both sides of such a junction own their endpoints, so
///   neither may be moved; the follower is re-gauged onto the shared vertex instead
///   (`interchange::arc::ExactArc::regauged`), keeping its centre and its own sweep and paying the
///   move into `δ`. This is the construction `interchange::element` wrote down and left unbuilt
///   until a real file needed it.
/// * **the two error numbers stay apart.** `δ = 2.6e-14` is *ours* — how far the emitted arcs are
///   from the ones the file states. The closure gap `2.3e-10` is the *file's*, and it lands
///   entirely on the two `LINE` endpoints, where moving costs nothing. Reporting one number would
///   have averaged a translator's fidelity with a drawing's sloppiness.
#[test]
fn a_real_device_drawing_reads_as_one_exact_loop() {
    use interchange::dxf::{DxfOptions, read_dxf};
    use interchange::report::ImportFault;

    // Without a unit the read is a refusal, not a guess.
    assert!(matches!(
        read_dxf::<Bignum>(acceptance::INNER_CUT_DXF, &DxfOptions::default()),
        Err(ImportFault::UnknownUnit { .. })
    ));

    let opts = DxfOptions::<Bignum> {
        assume_unit: Some(acceptance::INNER_CUT_UNIT),
        ..Default::default()
    };
    let read = read_dxf::<Bignum>(acceptance::INNER_CUT_DXF, &opts).expect("the device's bore");
    assert_eq!(read.report.entities, 6);
    assert_eq!(read.report.loops, 1, "{}", read.report.summary());
    assert_eq!(read.loops[0].len(), 6, "every entity is in the loop");

    // The junctions really were arc-to-arc: a re-gauge is the only thing that can put δ above the
    // per-entity floor, and the two numbers differ by four orders.
    let (delta, gap) = (&read.report.delta, &read.report.closure_gap);
    assert!(
        delta.sign() > 0 && *delta < Q::new(1, 1_000_000_000_000i128),
        "δ {}",
        decimal(delta, 18)
    );
    assert!(
        *gap > *delta,
        "the file's gap {} exceeds ours {}",
        decimal(gap, 18),
        decimal(delta, 18)
    );
    assert!(
        *gap < Q::new(1, 1_000_000_000i128),
        "gap {}",
        decimal(gap, 18)
    );

    // Every arc came out exactly on its own circle — the runtime-checked hypothesis, after the
    // re-gauge moved two of the circles.
    for e in &read.loops[0] {
        if let interchange::element::Element::Arc(a) = e {
            assert!(a.is_consistent(), "an assembled arc left its circle");
        }
    }

    // The shape: a bore of radius ≈ 4 with a tab reaching in to ≈ 2, and nothing between 2 and 4
    // except the flanks. `winding_parity` is inside the *removed* region.
    let edges = read.profile().into_edges();
    let on_axis = |r: f64, deg: f64| -> bool {
        let (x, y) = (r * deg.to_radians().cos(), r * deg.to_radians().sin());
        let f = |v: f64| Q::new((v * 1_000_000.0) as i128, 1_000_000);
        winding_parity(&f(x), &f(y), &edges)
    };
    assert!(on_axis(3.5, 0.0), "the bore is removed away from the tab");
    assert!(on_axis(3.5, 180.0), "…on the far side too");
    assert!(
        !on_axis(3.5, 90.0),
        "the tab is material, at 3.5 up the middle"
    );
    assert!(!on_axis(2.5, 90.0), "…and still material at 2.5");
    assert!(
        on_axis(1.5, 90.0),
        "…but past its Ø 4 tip the bore is removed again"
    );
    assert!(on_axis(3.5, 75.0), "the tab is only ~10° wide");
}
