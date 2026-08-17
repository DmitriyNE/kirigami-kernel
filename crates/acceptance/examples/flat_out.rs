//! **IO.2 — the flat pattern leaves as a manufacturable drawing.**
//!
//! Direction ③ of the four: a certified `FlatPattern` out to DXF and SVG at 1:1 physical scale,
//! with its own quality written into the file rather than implied.
//!
//! Two things it is careful about, both of which a demo could easily get wrong:
//!
//! * **The flat boundary really is a chord polygon.** The development's vertices are interval
//!   *boxes*, and the pattern's own certified `ε` is the number that says how good they are. The
//!   drawing carries `ε` and the worst box in a comment, so a reader can never mistake "written to
//!   twelve decimals" for "accurate to twelve decimals".
//! * **The outline and the holes go on separate layers**, because they are different instructions
//!   to a fab house and a single-layer file has thrown that away.
//!
//! ```text
//! cargo run --release --example flat_out            # writes generated-demos/flat_pattern.{dxf,svg}
//! cargo run --release --example flat_out -- --segments 48
//! ```

use certify_core::Verdict;
use interchange::dxf::write_dxf;
use interchange::element::Element;
use interchange::num::to_decimal;
use interchange::svg::write_svg;
use interchange::unit::Unit;
use interchange::write::{Drawing, WriteOptions};
use lattice::{Bignum, Rat};

type Q = Rat<Bignum>;

/// A closed loop of straight segments through `pts` — how a chorded boundary becomes elements.
fn polygon_loop(pts: &[[Q; 2]]) -> Vec<Element<Bignum>> {
    (0..pts.len())
        .filter(|&i| pts[i] != pts[(i + 1) % pts.len()])
        .map(|i| Element::Segment {
            start: pts[i].clone(),
            end: pts[(i + 1) % pts.len()].clone(),
        })
        .collect()
}

fn main() {
    let mut out_dir = "generated-demos".to_string();
    let mut segments = 96usize;
    let argv: Vec<String> = std::env::args().collect();
    for i in 0..argv.len() {
        match argv[i].as_str() {
            "--out-dir" if i + 1 < argv.len() => out_dir = argv[i + 1].clone(),
            "--segments" if i + 1 < argv.len() => {
                segments = argv[i + 1].parse().expect("--segments takes a number");
            }
            _ => {}
        }
    }
    std::fs::create_dir_all(&out_dir).expect("create --out-dir");

    // The acceptance device: a panel whose boundary is an authored radiused outline (AUTH.3d).
    let part = acceptance::contour_panel(segments, None);
    let t = std::time::Instant::now();
    let flat = match part.develop() {
        Verdict::Verified(f) => f,
        Verdict::Refuted(fault) => {
            println!("[refused] develop: {fault:?}");
            return;
        }
        Verdict::Unresolved(_) => {
            println!("[unresolved] develop ran out of budget");
            return;
        }
    };
    let outline = flat.outline();
    let worst_box = outline
        .vertices
        .iter()
        .map(|b| b.backward_error())
        .fold(Q::from_i128(0), |a, b| if b > a { b } else { a });
    println!(
        "[develop] {:.2}s   {} outline points   ε = {}   worst vertex box = {}",
        t.elapsed().as_secs_f64(),
        outline.vertices.len(),
        to_decimal(flat.eps(), 9),
        to_decimal(&worst_box, 12),
    );

    // The boundary is a chord polygon — the development's vertices are boxes, and this takes their
    // centres. That is the honest shape of a developed flat pattern, and the note below says so.
    let boundary: Vec<[Q; 2]> = outline
        .vertices
        .iter()
        .map(|b| {
            let (x, y) = b.center();
            [x, y]
        })
        .collect();
    let holes: Vec<Vec<Element<Bignum>>> = flat
        .holes()
        .iter()
        .map(|h| {
            let pts: Vec<[Q; 2]> = h
                .vertices
                .iter()
                .map(|b| {
                    let (x, y) = b.center();
                    [x, y]
                })
                .collect();
            polygon_loop(&pts)
        })
        .chain(flat.flat_hole_polys().iter().map(|p| polygon_loop(p)))
        .collect();

    let drawing = Drawing::<Bignum>::new()
        .layer("outline", vec![polygon_loop(&boundary)])
        .layer("holes", holes);

    let opts = WriteOptions {
        unit: Unit::Millimetre,
        places: 9,
        note: Some(format!(
            "kirigami flat pattern | chorded boundary, {} points | certified eps={} | \
             worst vertex box={} | coordinates are exact rationals rounded to 9 places",
            outline.vertices.len(),
            to_decimal(flat.eps(), 9),
            to_decimal(&worst_box, 12),
        )),
    };

    let (dxf_text, dxf_report) = write_dxf(&drawing, &opts);
    let dxf_path = format!("{out_dir}/flat_pattern.dxf");
    std::fs::write(&dxf_path, dxf_text).expect("write dxf");
    println!("[dxf] {}\n  wrote {dxf_path}", dxf_report.summary());

    let (svg_text, svg_report) = write_svg(&drawing, &opts);
    let svg_path = format!("{out_dir}/flat_pattern.svg");
    std::fs::write(&svg_path, svg_text).expect("write svg");
    println!("[svg] {}\n  wrote {svg_path}", svg_report.summary());

    // A chorded boundary has no curves, so both writers should have cost the coordinate rounding
    // and nothing else. Said out loud, because "exact" is a claim worth checking in the open.
    println!(
        "[exact] dxf={}  svg={}   (a chord polygon has no curve for a format to round)",
        dxf_report.curve.is_zero(),
        svg_report.curve.is_zero(),
    );
}
