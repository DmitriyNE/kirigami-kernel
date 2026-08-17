//! **IO.2 acceptance — the round trip, with its two legs kept apart.**
//!
//! `import(export(P))` against `P` is not one number. Writing and reading lose different things, in
//! different places, and a test that reports a single figure has averaged an exact leg with an
//! inexact one and said nothing about either.
//!
//! The asymmetry, measured here rather than asserted:
//!
//! | | inbound | outbound |
//! |---|---|---|
//! | DXF bulge | **exact** — the file's own rational | rounds `tan(Δθ/4)`, moving centre and radius |
//! | SVG `A` | rounds the **centre** onto the endpoints' bisector | rounds `√r²`, moving the radius |
//!
//! So a straight outline survives both round trips *identically*, and a curved one survives with a
//! cost that lands on a different datum each way.

use interchange::dxf::{DxfOptions, read_dxf, write_dxf};
use interchange::element::Element;
use interchange::svg::{SvgOptions, read_svg, write_svg};
use interchange::write::{Drawing, WriteOptions};
use lattice::{Bignum, Rat};

type Q = Rat<Bignum>;

fn p(x: i128, y: i128) -> [Q; 2] {
    [Q::from_i128(x), Q::from_i128(y)]
}

fn abs(q: &Q) -> Q {
    if q.sign() < 0 { q.neg() } else { q.clone() }
}

/// The vertices of every loop, sorted, so two readings can be compared without depending on
/// traversal order.
fn vertices(loops: &[Vec<Element<Bignum>>]) -> Vec<[Q; 2]> {
    let mut v: Vec<[Q; 2]> = loops.iter().flatten().filter_map(|e| e.start()).collect();
    v.sort_by(|a, b| a[0].cmp(&b[0]).then(a[1].cmp(&b[1])));
    v
}

/// An L-shaped outline of straight sides — the shape a board's cutout actually is.
fn ell() -> Vec<Element<Bignum>> {
    let pts = [p(0, 0), p(30, 0), p(30, 10), p(12, 10), p(12, 24), p(0, 24)];
    (0..pts.len())
        .map(|i| Element::Segment {
            start: pts[i].clone(),
            end: pts[(i + 1) % pts.len()].clone(),
        })
        .collect()
}

/// **A straight outline survives both round trips exactly**, in both formats and both directions.
/// No tolerance appears anywhere in this test, because none is warranted.
#[test]
fn a_straight_outline_round_trips_with_nothing_lost() {
    let drawing = Drawing::<Bignum>::new().layer("outline", vec![ell()]);
    let opts = WriteOptions::default();

    let (dxf_text, dxf_out) = write_dxf(&drawing, &opts);
    assert!(dxf_out.is_exact(), "DXF write: {}", dxf_out.summary());
    let dxf_back = read_dxf::<Bignum>(&dxf_text, &DxfOptions::default()).expect("DXF round trip");
    assert!(dxf_back.report.is_exact(), "{}", dxf_back.report.summary());
    assert_eq!(dxf_back.report.loops, 1);
    assert_eq!(dxf_back.report.closure_gap, Q::from_i128(0));
    assert_eq!(vertices(&dxf_back.loops), vertices(&[ell()]));

    let (svg_text, svg_out) = write_svg(&drawing, &opts);
    assert!(svg_out.is_exact(), "SVG write: {}", svg_out.summary());
    let svg_back = read_svg::<Bignum>(&svg_text, &SvgOptions::default()).expect("SVG round trip");
    assert!(svg_back.report.is_exact(), "{}", svg_back.report.summary());
    assert_eq!(svg_back.report.loops, 1);
    assert_eq!(vertices(&svg_back.loops), vertices(&[ell()]));

    // …and the two formats agree with each other, not merely each with the source.
    assert_eq!(vertices(&dxf_back.loops), vertices(&svg_back.loops));
}

/// **Awkward bounds round-trip as tightly as tidy ones.**
///
/// Every fixture above has integer extents, so its `viewBox` prints exactly and the writer's flip
/// axis and the reader's reconstruction of it agree by luck. A real flat pattern does not: the
/// acceptance panel spans `2.984615…` to `3.585454…`, and printing the frame at a *coarser*
/// precision than the coordinates — which reads better — shifted the whole drawing by a micron on
/// re-import. Found by looking at the demo's own output, not by a test, so here is the test.
#[test]
fn a_drawing_with_untidy_bounds_round_trips_to_the_coordinate_rounding() {
    let places = 9usize;
    let pts = [
        [
            Q::new(38_800_000, 13_000_000),
            Q::new(-3_254_321, 13_000_000),
        ],
        [
            Q::new(46_611_111, 13_000_000),
            Q::new(-3_254_321, 13_000_000),
        ],
        [
            Q::new(46_611_111, 13_000_000),
            Q::new(3_254_321, 13_000_000),
        ],
        [
            Q::new(38_800_000, 13_000_000),
            Q::new(3_254_321, 13_000_000),
        ],
    ];
    let loop_: Vec<Element<Bignum>> = (0..4)
        .map(|i| Element::Segment {
            start: pts[i].clone(),
            end: pts[(i + 1) % 4].clone(),
        })
        .collect();
    let drawing = Drawing::<Bignum>::new().layer("outline", vec![loop_]);
    let opts = WriteOptions {
        places,
        ..WriteOptions::default()
    };

    // Half a place is the *whole* budget: the frame must not add anything of its own.
    let budget = Q::new(1, 2).mul(&Q::new(1, 1_000_000_000));
    for (name, text) in [
        ("dxf", write_dxf(&drawing, &opts).0),
        ("svg", write_svg(&drawing, &opts).0),
    ] {
        let back = if name == "dxf" {
            read_dxf::<Bignum>(&text, &DxfOptions::default()).expect("dxf round trip")
        } else {
            read_svg::<Bignum>(&text, &SvgOptions::default()).expect("svg round trip")
        };
        let (there, here) = (
            vertices(&back.loops),
            vertices(&[(0..4)
                .map(|i| Element::Segment {
                    start: pts[i].clone(),
                    end: pts[(i + 1) % 4].clone(),
                })
                .collect::<Vec<_>>()]),
        );
        assert_eq!(there.len(), here.len(), "{name}");
        for (a, b) in there.iter().zip(&here) {
            for k in 0..2 {
                let moved = abs(&a[k].sub(&b[k]));
                assert!(
                    moved <= budget,
                    "{name}: coordinate {k} moved {moved:?}, over half a place"
                );
            }
        }
    }
}

/// **A curved outline costs something on the way out, nothing on the way in — through DXF.**
///
/// The bulge is where the format cannot hold the arc, so the write leg is nonzero; reading a bulge
/// is exact, so the read leg is zero. One number for the pair would report neither.
#[test]
fn a_dxf_arc_costs_the_write_leg_and_not_the_read_leg() {
    // A slot: two straight flanks closed by two semicircular ends. Semicircles are the case the
    // format *can* hold (bulge exactly 1), so the fixture uses a 3-4-5 quarter instead for one end
    // and a semicircle for the other — one lossy arc and one exact one, in the same loop.
    let arc = interchange::arc::from_bulge::<Bignum>(p(10, 0), p(10, 6), &Q::from_i128(1))
        .expect("a semicircular end");
    let round_end = interchange::arc::from_bulge::<Bignum>(
        p(0, 6),
        [Q::new(-18, 5), Q::new(6, 5)],
        &Q::new(1, 3),
    )
    .expect("a rounded corner");
    let loops = vec![vec![
        Element::Segment {
            start: p(0, 0),
            end: p(10, 0),
        },
        Element::Arc(arc),
        Element::Segment {
            start: p(10, 6),
            end: p(0, 6),
        },
        Element::Arc(round_end),
        Element::Segment {
            start: [Q::new(-18, 5), Q::new(6, 5)],
            end: p(0, 0),
        },
    ]];
    let drawing = Drawing::<Bignum>::new().layer("outline", loops);

    let (text, out) = write_dxf(&drawing, &WriteOptions::default());
    // The write leg is nonzero and small: the semicircle writes exactly, the other arc does not.
    assert!(out.curve.sign() > 0, "a general arc is not free outbound");
    assert!(
        out.curve < Q::new(1, 1_000_000_000),
        "…but it is a rounding, not a redesign: {}",
        out.summary()
    );

    let back = read_dxf::<Bignum>(&text, &DxfOptions::default()).expect("round trip");
    assert_eq!(
        back.report.delta,
        Q::from_i128(0),
        "reading a bulge costs nothing — the whole read leg is exact"
    );
    assert_eq!(back.report.loops, 1);
    assert_eq!(back.loops[0].iter().filter(|e| e.is_arc()).count(), 2);
    // The vertices are untouched by both legs; only the curves between them moved.
    for e in back.loops[0].iter() {
        if let Element::Arc(a) = e {
            assert!(
                a.is_consistent(),
                "and the arc that came back is consistent"
            );
        }
    }
}

/// **Each format rounds a different scalar, so each is exact on arcs the other is not.**
///
/// Outbound, both formats derive the centre and radius from one written number — DXF from the
/// bulge `tan(Δθ/4)`, SVG from the radius `√r²`. Since those are different numbers, they are
/// rational on different arcs, and the difference is a *two-sided* differential rather than a
/// matter of magnitudes:
///
/// * a **quarter turn of radius 5** writes exactly through SVG (`r = 5`) and inexactly through DXF
///   (`tan 22.5° = √2 − 1`);
/// * a **semicircle of radius √2** writes exactly through DXF (`tan 45° = 1`) and inexactly through
///   SVG (`√2`).
///
/// Each half on its own would read as "arcs cost a little"; together they say which carrier suits
/// which arc, which is the fact a caller can act on.
#[test]
fn each_format_is_exact_on_the_arcs_the_other_rounds() {
    use interchange::arc::{ExactArc, from_bulge};

    let write_costs = |loops: Vec<Vec<Element<Bignum>>>| {
        let d = Drawing::<Bignum>::new().layer("outline", loops);
        let o = WriteOptions::default();
        (write_dxf(&d, &o).1.curve, write_svg(&d, &o).1.curve)
    };

    // A quarter turn of radius 5, closed by its two radii. `√25` is rational; `tan(Δθ/4)` is not.
    let quarter = ExactArc::<Bignum>::exact(
        Q::from_i128(0),
        Q::from_i128(0),
        Q::from_i128(25),
        p(5, 0),
        p(0, 5),
        true,
    )
    .expect("exact by construction");
    let (dxf_q, svg_q) = write_costs(vec![vec![
        Element::Arc(quarter),
        Element::Segment {
            start: p(0, 5),
            end: p(0, 0),
        },
        Element::Segment {
            start: p(0, 0),
            end: p(5, 0),
        },
    ]]);
    assert_eq!(
        svg_q,
        Q::from_i128(0),
        "SVG writes a rational radius exactly"
    );
    assert!(dxf_q.sign() > 0, "DXF must round tan(22.5°) = √2 − 1");

    // A semicircle of radius √2 — `tan 45° = 1` is rational, `√2` is not.
    let semi = from_bulge::<Bignum>(p(1, 1), p(-1, -1), &Q::from_i128(1)).expect("exact");
    assert_eq!(
        semi.r2,
        Q::from_i128(2),
        "an irrational radius, exactly held"
    );
    let (dxf_s, svg_s) = write_costs(vec![vec![
        Element::Arc(semi),
        Element::Segment {
            start: p(-1, -1),
            end: p(1, 1),
        },
    ]]);
    assert_eq!(
        dxf_s,
        Q::from_i128(0),
        "DXF writes a semicircle's bulge exactly"
    );
    assert!(svg_s.sign() > 0, "SVG must round √2");

    // The differential is genuinely two-sided: neither format dominates.
    assert!(
        dxf_q.sign() > 0 && svg_s.sign() > 0 && svg_q.is_zero() && dxf_s.is_zero(),
        "dxf: {dxf_q:?}/{dxf_s:?}  svg: {svg_q:?}/{svg_s:?}"
    );
}

/// Both routes come back as **consistent** arcs within their reported budgets — the property that
/// makes a round-tripped file usable rather than merely close.
#[test]
fn a_round_tripped_arc_is_still_exactly_on_its_own_circle() {
    // A quarter turn of radius 5 about the origin, closed by two radii. Its bulge is tan(22.5°) =
    // √2 − 1 — irrational, so both formats have to give something up.
    let quarter = interchange::arc::from_centre_angles::<Bignum>(
        p(0, 0),
        &Q::from_i128(5),
        &Q::from_i128(30),
        &Q::from_i128(120),
        &interchange::arc::ArcTolerance::report_only(),
    )
    .expect("a certified quarter");
    let (s, e) = (quarter.start.clone(), quarter.end.clone());
    let loops = vec![vec![
        Element::Arc(quarter.clone()),
        Element::Segment {
            start: e,
            end: p(0, 0),
        },
        Element::Segment {
            start: p(0, 0),
            end: s,
        },
    ]];
    let drawing = Drawing::<Bignum>::new().layer("outline", loops);
    let opts = WriteOptions::default();

    let (dxf_text, _) = write_dxf(&drawing, &opts);
    let dxf_back = read_dxf::<Bignum>(&dxf_text, &DxfOptions::default()).expect("dxf");
    let (svg_text, _) = write_svg(&drawing, &opts);
    let svg_back = read_svg::<Bignum>(&svg_text, &SvgOptions::default()).expect("svg");

    let arc_of = |r: &interchange::Imported<Bignum>| {
        r.loops[0]
            .iter()
            .find_map(|e| match e {
                Element::Arc(a) => Some(a.clone()),
                _ => None,
            })
            .expect("an arc came back")
    };
    let (d, v) = (arc_of(&dxf_back), arc_of(&svg_back));

    // **The property that matters**: both come back *exactly* on their own circles. A round trip
    // that returned an arc whose endpoints merely sat near its circle would poison the arrangement
    // downstream rather than earn a refusal — which is the whole reason `is_consistent` exists.
    assert!(d.is_consistent(), "DXF round trip");
    assert!(v.is_consistent(), "SVG round trip");

    // And both land near the original — the drift is a rounding, not a redesign.
    let drift = |a: &interchange::arc::ExactArc<Bignum>| {
        abs(&a.cx.sub(&quarter.cx))
            .add(&abs(&a.cy.sub(&quarter.cy)))
            .add(&abs(&a.r2.sub(&quarter.r2)))
    };
    assert!(drift(&d) < Q::new(1, 1_000_000), "DXF drift");
    assert!(drift(&v) < Q::new(1, 1_000_000), "SVG drift");

    // Neither round trip is *free*, which is what makes the assertions above non-vacuous: an arc
    // whose written scalar is irrational costs something in both formats.
    assert!(drift(&d).sign() > 0 && drift(&v).sign() > 0);
}

/// The drawing a fab house opens: 1:1 physical units, no margin, layers separable — and the
/// pattern's own quality written into it rather than implied.
#[test]
fn the_svg_is_a_drawing_and_not_a_viewer() {
    let hole = vec![Element::Circle {
        cx: Q::from_i128(20),
        cy: Q::from_i128(5),
        r2: Q::from_i128(4),
    }];
    let drawing = Drawing::<Bignum>::new()
        .layer("outline", vec![ell()])
        .layer("holes", vec![hole]);
    let (text, report) = write_svg(
        &drawing,
        &WriteOptions {
            note: Some("flat pattern eps=4.1481e-1 worst box=2.3e-9".into()),
            ..WriteOptions::default()
        },
    );

    // 1:1 — the physical size and the viewBox agree, and there is no padding to un-scale.
    assert!(text.contains(r#"width="30mm""#), "{text}");
    assert!(text.contains(r#"height="24mm""#));
    assert!(text.contains(r#"viewBox="0 0 30 24""#));
    // Separable layers, and the note is in the file.
    assert!(text.contains(r#"<g id="outline""#));
    assert!(text.contains(r#"<g id="holes""#));
    assert!(text.contains("eps=4.1481e-1"));
    // Two entities: one path, one circle.
    assert_eq!(report.entities, 2);
    assert_eq!(text.matches("<path").count(), 1);
    assert_eq!(text.matches("<circle").count(), 1);

    // …and it still reads back as the same two loops.
    let back = read_svg::<Bignum>(&text, &SvgOptions::default()).expect("round trip");
    assert_eq!(back.report.loops, 2);
    assert_eq!(back.report.scale, Q::from_i128(1));
}
