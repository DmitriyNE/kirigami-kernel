//! **DXF (ASCII) reading** — group codes straight to exact rationals.
//!
//! A DXF ASCII file is pairs of lines: a group code, then its value. For the five entities a board
//! outline is made of — `LINE`, `ARC`, `CIRCLE`, `LWPOLYLINE`, `POLYLINE`/`VERTEX` — that is the
//! entire grammar, which is why this is read directly rather than through a crate.
//!
//! The decision is not about size. A DXF library hands coordinates over as `f64`, and that throws
//! away the file's own decimal text — the very thing [`crate::num::rat_from_decimal`] reads
//! exactly. Reading the text ourselves makes the transport error **exactly zero** rather than
//! ulp-bounded, so a DXF import is exact end to end whenever the geometry is (`LINE`, `CIRCLE`,
//! `LWPOLYLINE` with bulges). Only an `ARC` entity costs a certified `δ`, and only because it
//! over-states its own geometry (see [`crate::arc`]).
//!
//! What that buys, said plainly for the workflow: **export your outline as an `LWPOLYLINE` with
//! bulges and the whole import is exact.**

use crate::arc::{ArcFault, ArcTolerance, from_bulge, from_centre_angles};
use crate::element::{Element, assemble};
use crate::num::rat_from_decimal;
use crate::num::{sqrt_rational, to_decimal};
use crate::report::{ImportFault, ImportReport};
use crate::unit::Unit;
use crate::write::{Cost, Drawing, ExportReport, WriteOptions, abs, bulge};
use crate::{Imported, max_of};
use lattice::{Backend, Rat};

/// How to read a DXF: what unit to believe, what unit to produce, and how much slack to allow.
pub struct DxfOptions<B: Backend> {
    /// The unit to use when the file's `$INSUNITS` is absent or unitless. `None` means a file that
    /// does not declare a unit is **refused** — an inferred unit is a 25.4× part.
    pub assume_unit: Option<Unit>,
    /// The unit the geometry comes out in.
    pub target: Unit,
    /// The largest gap between adjacent entities that assembly may absorb, in `target` units.
    pub weld: Rat<B>,
    /// Arc construction budget and refinement.
    pub arc: ArcTolerance<B>,
}

impl<B: Backend> Default for DxfOptions<B> {
    /// Millimetres in, millimetres out, a 1 µm weld — the settings a flex outline actually wants.
    fn default() -> Self {
        DxfOptions {
            assume_unit: None,
            target: Unit::Millimetre,
            weld: Rat::new(1, 1000),
            arc: ArcTolerance::report_only(),
        }
    }
}

/// One `(group code, value)` pair of the file, value text kept verbatim.
struct Pair<'a> {
    code: i64,
    value: &'a str,
}

/// Split the file into group-code pairs. A line that is not an integer code is a malformed file.
fn pairs(text: &str) -> Result<Vec<Pair<'_>>, ImportFault> {
    let mut out = Vec::new();
    let mut lines = text.lines();
    while let Some(code_line) = lines.next() {
        let code_text = code_line.trim();
        if code_text.is_empty() {
            continue;
        }
        let Ok(code) = code_text.parse::<i64>() else {
            return Err(ImportFault::MalformedNumber {
                entity: "group code".into(),
                text: code_text.into(),
            });
        };
        let Some(value) = lines.next() else {
            return Err(ImportFault::MalformedNumber {
                entity: format!("group {code}"),
                text: "<missing value line>".into(),
            });
        };
        out.push(Pair {
            code,
            value: value.trim(),
        });
    }
    Ok(out)
}

/// The unit the file declares, if it declares one.
fn declared_unit(pairs: &[Pair<'_>]) -> Option<i64> {
    pairs
        .windows(2)
        .find(|w| w[0].code == 9 && w[0].value == "$INSUNITS" && w[1].code == 70)
        .and_then(|w| w[1].value.parse::<i64>().ok())
}

/// Read an ASCII DXF into closed loops of exact geometry.
///
/// ```
/// use interchange::dxf::{DxfOptions, read_dxf};
/// use interchange::unit::Unit;
/// use lattice::Bignum;
///
/// // A 2 mm square, as four LINE entities.
/// let mut dxf = String::from("0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n\
///                             0\nSECTION\n2\nENTITIES\n");
/// for (x0, y0, x1, y1) in [(0, 0, 2, 0), (2, 0, 2, 2), (2, 2, 0, 2), (0, 2, 0, 0)] {
///     dxf += &format!("0\nLINE\n10\n{x0}.0\n20\n{y0}.0\n11\n{x1}.0\n21\n{y1}.0\n");
/// }
/// dxf += "0\nENDSEC\n0\nEOF\n";
///
/// let read = read_dxf::<Bignum>(&dxf, &DxfOptions::default()).expect("a square");
/// assert_eq!(read.report.loops, 1);
/// assert_eq!(read.report.source_unit, Unit::Millimetre);
/// assert!(read.report.is_exact(), "straight geometry costs nothing");
/// ```
pub fn read_dxf<B: Backend>(text: &str, opts: &DxfOptions<B>) -> Result<Imported<B>, ImportFault> {
    if text.starts_with("AutoCAD Binary DXF") {
        return Err(ImportFault::BinaryDxf);
    }
    let pairs = pairs(text)?;

    let declared = declared_unit(&pairs);
    let source = match declared
        .and_then(Unit::from_dxf_insunits)
        .or(opts.assume_unit)
    {
        Some(u) => u,
        None => {
            return Err(ImportFault::UnknownUnit {
                declared: declared.map(|d| format!("$INSUNITS {d}")),
            });
        }
    };
    let scale = source.factor_to::<B>(opts.target);

    // Entities live between `2/ENTITIES` and the matching `0/ENDSEC`.
    let start = pairs.windows(2).position(|w| {
        w[0].code == 0 && w[0].value == "SECTION" && w[1].code == 2 && w[1].value == "ENTITIES"
    });
    let Some(start) = start else {
        return Err(ImportFault::Empty);
    };
    let body = &pairs[start + 2..];
    let end = body
        .iter()
        .position(|p| p.code == 0 && p.value == "ENDSEC")
        .unwrap_or(body.len());
    let body = &body[..end];

    // Split into entities on code 0.
    let mut starts: Vec<usize> = body
        .iter()
        .enumerate()
        .filter(|(_, p)| p.code == 0)
        .map(|(i, _)| i)
        .collect();
    starts.push(body.len());

    let mut elements: Vec<Element<B>> = Vec::new();
    let mut entities = 0usize;
    let mut delta = Rat::from_i128(0);
    let mut i = 0usize;
    while i + 1 < starts.len() {
        let kind = body[starts[i]].value;
        let mut span_end = starts[i + 1];
        let mut consumed = 1usize;
        // POLYLINE owns the VERTEX/SEQEND entities that follow it.
        if kind == "POLYLINE" {
            while i + consumed < starts.len() - 1
                && matches!(body[starts[i + consumed]].value, "VERTEX" | "SEQEND")
            {
                span_end = starts[i + consumed + 1];
                consumed += 1;
            }
        }
        let span = &body[starts[i]..span_end];
        i += consumed;
        entities += 1;

        let produced = match kind {
            "LINE" => line_entity(span, &scale)?,
            "CIRCLE" => circle_entity(span, &scale)?,
            "ARC" => arc_entity(span, &scale, &opts.arc)?,
            "LWPOLYLINE" | "POLYLINE" => polyline_entity(span, &scale, kind)?,
            "ENDSEC" | "SEQEND" | "VERTEX" => {
                entities -= 1;
                Vec::new()
            }
            other => {
                return Err(ImportFault::UnsupportedEntity {
                    kind: other.to_string(),
                });
            }
        };
        for e in &produced {
            delta = max_of(delta, e.delta());
        }
        elements.extend(produced);
    }

    if elements.is_empty() {
        return Err(ImportFault::Empty);
    }
    let assembled = assemble(elements, &opts.weld)?;
    Ok(Imported {
        report: ImportReport {
            source_unit: source,
            target_unit: opts.target,
            scale,
            entities,
            loops: assembled.loops.len(),
            delta,
            // The reader sees the file's own decimal text, so nothing was transported through an
            // `f64` on the way in. This is the whole reason it is hand-rolled.
            transport: Rat::from_i128(0),
            closure_gap: assembled.closure_gap,
        },
        loops: assembled.loops,
    })
}

/// The value of the first pair with this group code, as an exact rational scaled to target units.
fn coord<B: Backend>(
    span: &[Pair<'_>],
    code: i64,
    scale: &Rat<B>,
    entity: &str,
) -> Result<Rat<B>, ImportFault> {
    raw(span, code, entity).map(|q| q.mul(scale))
}

/// The value of the first pair with this group code, as an exact rational, **unscaled** — for the
/// quantities that are not lengths (angles in degrees, bulges, flags).
fn raw<B: Backend>(span: &[Pair<'_>], code: i64, entity: &str) -> Result<Rat<B>, ImportFault> {
    let p = span
        .iter()
        .find(|p| p.code == code)
        .ok_or_else(|| ImportFault::MalformedNumber {
            entity: entity.to_string(),
            text: format!("<no group {code}>"),
        })?;
    rat_from_decimal(p.value).ok_or_else(|| ImportFault::MalformedNumber {
        entity: entity.to_string(),
        text: p.value.to_string(),
    })
}

fn line_entity<B: Backend>(
    span: &[Pair<'_>],
    scale: &Rat<B>,
) -> Result<Vec<Element<B>>, ImportFault> {
    Ok(vec![Element::Segment {
        start: [
            coord(span, 10, scale, "LINE")?,
            coord(span, 20, scale, "LINE")?,
        ],
        end: [
            coord(span, 11, scale, "LINE")?,
            coord(span, 21, scale, "LINE")?,
        ],
    }])
}

fn circle_entity<B: Backend>(
    span: &[Pair<'_>],
    scale: &Rat<B>,
) -> Result<Vec<Element<B>>, ImportFault> {
    let r = coord(span, 40, scale, "CIRCLE")?;
    if r.sign() <= 0 {
        return Err(ImportFault::DegenerateArc {
            entity: "CIRCLE".into(),
            reason: "NonPositiveRadius".into(),
        });
    }
    Ok(vec![Element::Circle {
        cx: coord(span, 10, scale, "CIRCLE")?,
        cy: coord(span, 20, scale, "CIRCLE")?,
        r2: r.mul(&r),
    }])
}

fn arc_entity<B: Backend>(
    span: &[Pair<'_>],
    scale: &Rat<B>,
    tol: &ArcTolerance<B>,
) -> Result<Vec<Element<B>>, ImportFault> {
    let centre = [
        coord(span, 10, scale, "ARC")?,
        coord(span, 20, scale, "ARC")?,
    ];
    let r = coord(span, 40, scale, "ARC")?;
    // Angles are angles: they do not scale with the unit.
    let a0 = raw(span, 50, "ARC")?;
    let a1 = raw(span, 51, "ARC")?;
    let arc = from_centre_angles(centre, &r, &a0, &a1, tol).map_err(|f| arc_fault("ARC", &f))?;
    Ok(vec![Element::Arc(arc)])
}

/// `LWPOLYLINE` (`10`/`20` vertices, optional `42` bulge attached to the vertex it leaves) and the
/// older `POLYLINE`/`VERTEX` spelling, which carries the same three codes one entity deeper.
fn polyline_entity<B: Backend>(
    span: &[Pair<'_>],
    scale: &Rat<B>,
    kind: &str,
) -> Result<Vec<Element<B>>, ImportFault> {
    // Closed flag: group 70 bit 1. On a POLYLINE it sits before the vertices, on an LWPOLYLINE
    // among the header codes; either way it is the first 70 in the span.
    let closed = span
        .iter()
        .find(|p| p.code == 70)
        .and_then(|p| p.value.parse::<i64>().ok())
        .is_some_and(|f| f & 1 == 1);

    let mut verts: Vec<([Rat<B>; 2], Rat<B>)> = Vec::new();
    let mut pending_x: Option<Rat<B>> = None;
    // An `LWPOLYLINE` carries its vertices directly. The older `POLYLINE` carries a **dummy**
    // `10/20/30` of its own (the format requires it) and the real vertices one entity deeper, in
    // the `VERTEX` records — so reading every `10/20` in the span picks up a phantom vertex at the
    // origin and splits the loop. Found by round-tripping the writer's own output, which is the
    // only thing that exercises the R12 spelling.
    let mut in_vertex = kind == "LWPOLYLINE";
    for p in span {
        if p.code == 0 {
            in_vertex = kind == "LWPOLYLINE" || p.value == "VERTEX";
            continue;
        }
        if !in_vertex {
            continue;
        }
        match p.code {
            10 => {
                pending_x = Some(
                    rat_from_decimal::<B>(p.value)
                        .ok_or_else(|| ImportFault::MalformedNumber {
                            entity: kind.to_string(),
                            text: p.value.to_string(),
                        })?
                        .mul(scale),
                );
            }
            20 => {
                let Some(x) = pending_x.take() else { continue };
                let y = rat_from_decimal::<B>(p.value)
                    .ok_or_else(|| ImportFault::MalformedNumber {
                        entity: kind.to_string(),
                        text: p.value.to_string(),
                    })?
                    .mul(scale);
                verts.push(([x, y], Rat::from_i128(0)));
            }
            42 => {
                // The bulge belongs to the vertex it was written after.
                if let Some(last) = verts.last_mut() {
                    last.1 = rat_from_decimal::<B>(p.value).ok_or_else(|| {
                        ImportFault::MalformedNumber {
                            entity: kind.to_string(),
                            text: p.value.to_string(),
                        }
                    })?;
                }
            }
            _ => {}
        }
    }
    if verts.len() < 2 {
        return Err(ImportFault::Empty);
    }

    let n = verts.len();
    let last = if closed { n } else { n - 1 };
    let mut out = Vec::with_capacity(last);
    for k in 0..last {
        let (a, bulge) = (&verts[k].0, &verts[k].1);
        let b = &verts[(k + 1) % n].0;
        if bulge.is_zero() {
            out.push(Element::Segment {
                start: a.clone(),
                end: b.clone(),
            });
        } else {
            // The bulge path is exact: nothing here consults a tolerance.
            let arc = from_bulge(a.clone(), b.clone(), bulge).map_err(|f| arc_fault(kind, &f))?;
            out.push(Element::Arc(arc));
        }
    }
    Ok(out)
}

/// Name an arc refusal without leaking the generic fault type across the API boundary.
fn arc_fault<B: Backend>(entity: &str, f: &ArcFault<B>) -> ImportFault {
    use crate::num::to_decimal;
    match f {
        ArcFault::ToleranceExceeded { delta, budget } => ImportFault::ToleranceExceeded {
            entity: entity.to_string(),
            delta: to_decimal(delta, 12),
            budget: to_decimal(budget, 12),
        },
        other => ImportFault::DegenerateArc {
            entity: entity.to_string(),
            reason: other.name().to_string(),
        },
    }
}

// --- writing --------------------------------------------------------------------------------

/// Write a [`Drawing`] as an **R12 (AC1009) ASCII DXF**, and report what that cost.
///
/// R12 because it is the dialect every tool still reads: no handles, no classes, no object
/// section. Loops become one `POLYLINE` each — closed, with a `VERTEX` per element and a bulge
/// (group `42`) wherever that element is an arc — so a loop's closure is a property of the entity
/// rather than something the reader on the other end has to weld back together. A loop that is a
/// whole circle becomes a `CIRCLE`.
///
/// **The bulge is where a DXF loses an arc.** `tan(Δθ/4)` is irrational for almost every arc a
/// kernel produces, so the written decimal moves the arc's *centre and radius* while leaving its
/// two vertices exactly where they are — the mirror image of what reading a DXF `ARC` does. That
/// error is not bounded from the format, it is **measured**: the writer re-reads its own decimals
/// through [`from_bulge`] and reports how far the arc came back
/// ([`ExportReport::curve`](crate::write::ExportReport::curve)).
///
/// ```
/// use interchange::dxf::{DxfOptions, read_dxf, write_dxf};
/// use interchange::element::Element;
/// use interchange::write::{Drawing, WriteOptions};
/// use lattice::{Bignum, Rat};
/// type Q = Rat<Bignum>;
///
/// let square: Vec<Element<Bignum>> = (0..4)
///     .map(|i| {
///         let c = |k: i128| [Q::from_i128(k & 1), Q::from_i128((k >> 1) & 1)];
///         let order = [0, 1, 3, 2];
///         Element::Segment { start: c(order[i]), end: c(order[(i + 1) % 4]) }
///     })
///     .collect();
/// let drawing = Drawing::<Bignum>::new().layer("outline", vec![square]);
/// let (text, report) = write_dxf(&drawing, &WriteOptions::default());
/// assert!(report.is_exact(), "a square costs a DXF nothing");
///
/// // …and it reads back as itself.
/// let back = read_dxf::<Bignum>(&text, &DxfOptions::default()).expect("round trip");
/// assert_eq!(back.report.loops, 1);
/// assert!(back.report.is_exact());
/// ```
pub fn write_dxf<B: Backend>(
    drawing: &Drawing<B>,
    opts: &WriteOptions,
) -> (String, ExportReport<B>) {
    let mut cost = Cost::<B>::new(opts.places);
    let mut out = String::new();
    let pair = |code: i64, value: &str, out: &mut String| {
        out.push_str(&format!("{code}\n{value}\n"));
    };

    // — HEADER —
    pair(0, "SECTION", &mut out);
    pair(2, "HEADER", &mut out);
    pair(9, "$ACADVER", &mut out);
    pair(1, "AC1009", &mut out);
    pair(9, "$INSUNITS", &mut out);
    pair(70, &opts.unit.dxf_insunits().to_string(), &mut out);
    if let Some([lx, ly, hx, hy]) = drawing.bounds() {
        // The extents are a *frame*, so they are widened rather than rounded — a tool that zooms
        // to them must not crop the geometry by half a decimal place.
        for (name, x, y) in [("$EXTMIN", &lx, &ly), ("$EXTMAX", &hx, &hy)] {
            pair(9, name, &mut out);
            pair(10, &to_decimal(x, opts.places), &mut out);
            pair(20, &to_decimal(y, opts.places), &mut out);
            pair(30, "0.0", &mut out);
        }
    }
    pair(0, "ENDSEC", &mut out);

    // — TABLES: declare the layers, so a strict reader does not invent them —
    pair(0, "SECTION", &mut out);
    pair(2, "TABLES", &mut out);
    pair(0, "TABLE", &mut out);
    pair(2, "LAYER", &mut out);
    pair(70, &drawing.layers.len().to_string(), &mut out);
    for l in &drawing.layers {
        pair(0, "LAYER", &mut out);
        pair(2, &l.name, &mut out);
        pair(70, "0", &mut out);
        pair(62, "7", &mut out);
        pair(6, "CONTINUOUS", &mut out);
    }
    pair(0, "ENDTAB", &mut out);
    pair(0, "ENDSEC", &mut out);

    // — ENTITIES —
    pair(0, "SECTION", &mut out);
    pair(2, "ENTITIES", &mut out);
    for layer in &drawing.layers {
        for chain in &layer.loops {
            match chain.as_slice() {
                [Element::Circle { cx, cy, r2 }] => {
                    pair(0, "CIRCLE", &mut out);
                    pair(8, &layer.name, &mut out);
                    pair(10, &cost.text(cx), &mut out);
                    pair(20, &cost.text(cy), &mut out);
                    pair(30, "0.0", &mut out);
                    let r = sqrt_rational(r2, 32);
                    let written = cost.coord(&r);
                    cost.curve(abs(&written.sub(&r)));
                    pair(40, &to_decimal(&written, opts.places), &mut out);
                    cost.entities += 1;
                }
                elements => {
                    pair(0, "POLYLINE", &mut out);
                    pair(8, &layer.name, &mut out);
                    pair(66, "1", &mut out); // vertices follow (R12 requires it)
                    pair(70, "1", &mut out); // closed
                    pair(10, "0.0", &mut out);
                    pair(20, "0.0", &mut out);
                    pair(30, "0.0", &mut out);
                    for e in elements {
                        let Some(start) = e.start() else { continue };
                        pair(0, "VERTEX", &mut out);
                        pair(8, &layer.name, &mut out);
                        let vx = cost.coord(&start[0]);
                        let vy = cost.coord(&start[1]);
                        pair(10, &to_decimal(&vx, opts.places), &mut out);
                        pair(20, &to_decimal(&vy, opts.places), &mut out);
                        pair(30, "0.0", &mut out);
                        if let Element::Arc(a) = e {
                            let b = cost.coord(&bulge(a));
                            pair(42, &to_decimal(&b, opts.places), &mut out);
                            // Measure, do not bound: read the decimals actually written and see
                            // where the arc came back.
                            let end = e.end().expect("an arc has an end");
                            let ex = [cost.coord(&end[0]), cost.coord(&end[1])];
                            if let Ok(back) = from_bulge::<B>([vx, vy], ex, &b) {
                                let moved_c =
                                    abs(&back.cx.sub(&a.cx)).add(&abs(&back.cy.sub(&a.cy)));
                                let moved_r = abs(
                                    &sqrt_rational(&back.r2, 32).sub(&sqrt_rational(&a.r2, 32))
                                );
                                cost.curve(moved_c.add(&moved_r));
                            }
                        }
                    }
                    pair(0, "SEQEND", &mut out);
                    pair(8, &layer.name, &mut out);
                    cost.entities += 1;
                }
            }
        }
    }
    pair(0, "ENDSEC", &mut out);
    pair(0, "EOF", &mut out);

    (out, cost.into_report(opts.unit))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::Element;
    use lattice::Bignum;

    type Q = Rat<Bignum>;

    /// Wrap entity text in the minimum header/section scaffolding a DXF needs.
    fn file(insunits: Option<i64>, entities: &str) -> String {
        let header = match insunits {
            Some(u) => format!("0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n{u}\n0\nENDSEC\n"),
            None => String::new(),
        };
        format!("{header}0\nSECTION\n2\nENTITIES\n{entities}0\nENDSEC\n0\nEOF\n")
    }

    fn opts() -> DxfOptions<Bignum> {
        DxfOptions::default()
    }

    /// A rounded rectangle as an `LWPOLYLINE` with bulges — the outline form a flex fabricator
    /// emits — imports with **nothing moved at all**. Not "within tolerance": `δ = 0`, the
    /// closure gap is `0`, and the transport bound is `0` because the reader saw the text.
    #[test]
    fn a_bulge_polyline_imports_with_no_error_of_any_kind() {
        // A 20 × 10 rectangle with quarter-ish corners; bulge 1/2 is a legitimate rational arc.
        let mut e = String::from("0\nLWPOLYLINE\n8\n0\n90\n8\n70\n1\n");
        for (x, y, b) in [
            ("2.0", "0.0", "0.0"),
            ("18.0", "0.0", "0.5"),
            ("20.0", "2.0", "0.0"),
            ("20.0", "8.0", "0.5"),
            ("18.0", "10.0", "0.0"),
            ("2.0", "10.0", "0.5"),
            ("0.0", "8.0", "0.0"),
            ("0.0", "2.0", "0.5"),
        ] {
            e += &format!("10\n{x}\n20\n{y}\n42\n{b}\n");
        }
        let read = read_dxf::<Bignum>(&file(Some(4), &e), &opts()).expect("a rounded rectangle");
        assert_eq!(read.report.loops, 1);
        assert_eq!(read.loops[0].len(), 8, "four sides and four corner arcs");
        assert_eq!(read.report.delta, Q::from_i128(0), "bulges cost nothing");
        assert_eq!(read.report.transport, Q::from_i128(0), "the text was read");
        assert_eq!(read.report.closure_gap, Q::from_i128(0), "nothing welded");
        assert!(read.report.is_exact());
        // And the corners really are arcs, not chords.
        assert_eq!(read.loops[0].iter().filter(|e| e.is_arc()).count(), 4);
    }

    /// The exact decimal a file wrote is the exact rational that comes out — including a value
    /// binary floating point cannot hold.
    #[test]
    fn coordinates_are_the_file_s_own_decimals() {
        let e = "0\nLINE\n10\n0.1\n20\n0.2\n11\n0.3\n21\n0.0\n\
                 0\nLINE\n10\n0.3\n20\n0.0\n11\n0.1\n21\n0.2\n";
        let read = read_dxf::<Bignum>(&file(Some(4), e), &opts()).expect("two lines");
        let Element::Segment { start, .. } = &read.loops[0][0] else {
            panic!("expected a segment")
        };
        assert!(
            start == &[Q::new(1, 10), Q::new(2, 10)] || start == &[Q::new(3, 10), Q::from_i128(0)],
            "got {start:?}"
        );
    }

    /// Units convert exactly, and the *same* outline in inches is the millimetre one scaled by
    /// exactly `127/5` — the control that makes the unit path testable rather than plausible.
    #[test]
    fn the_same_outline_in_inches_is_the_millimetre_one_times_127_over_5() {
        let e = |s: &str| {
            format!(
                "0\nLINE\n10\n0.0\n20\n0.0\n11\n{s}\n21\n0.0\n\
                 0\nLINE\n10\n{s}\n20\n0.0\n11\n0.0\n21\n0.0\n"
            )
        };
        let mm = read_dxf::<Bignum>(&file(Some(4), &e("1.0")), &opts()).expect("mm");
        let inch = read_dxf::<Bignum>(&file(Some(1), &e("1.0")), &opts()).expect("in");
        assert_eq!(inch.report.scale, Q::new(127, 5));
        let end_of = |r: &Imported<Bignum>| match &r.loops[0][0] {
            Element::Segment { start, end } => {
                if start[0].is_zero() {
                    end[0].clone()
                } else {
                    start[0].clone()
                }
            }
            _ => panic!("segment"),
        };
        assert_eq!(end_of(&inch), end_of(&mm).mul(&Q::new(127, 5)));
    }

    /// A file that does not say what its numbers mean is refused, not guessed at — unless the
    /// caller takes responsibility for the assumption.
    #[test]
    fn an_undeclared_unit_refuses_until_the_caller_supplies_one() {
        let e = "0\nLINE\n10\n0.0\n20\n0.0\n11\n1.0\n21\n0.0\n\
                 0\nLINE\n10\n1.0\n20\n0.0\n11\n0.0\n21\n0.0\n";
        assert!(matches!(
            read_dxf::<Bignum>(&file(None, e), &opts()),
            Err(ImportFault::UnknownUnit { declared: None })
        ));
        // `$INSUNITS 0` is a declaration of *unitless*, which is still not a unit.
        assert!(matches!(
            read_dxf::<Bignum>(&file(Some(0), e), &opts()),
            Err(ImportFault::UnknownUnit { declared: Some(_) })
        ));
        let assumed = DxfOptions {
            assume_unit: Some(Unit::Millimetre),
            ..opts()
        };
        assert!(read_dxf::<Bignum>(&file(None, e), &assumed).is_ok());
    }

    /// An `ARC` entity is the one lossy form, and it is lossy in a *specific* way: the circle is
    /// the file's exactly, the endpoints are certified onto it, and `δ` is the endpoint distance.
    #[test]
    fn an_arc_entity_costs_a_certified_delta_and_keeps_its_circle() {
        // 30° → 90°: the start angle is deliberately *off* the quarter-turn grid, where the
        // reduction would have solved it exactly and this test would prove nothing. The closing
        // lines run to the nominal `(5 cos 30°, 5 sin 30°)`, which the weld absorbs.
        let e = "0\nARC\n10\n0.0\n20\n0.0\n40\n5.0\n50\n30.0\n51\n90.0\n\
                 0\nLINE\n10\n0.0\n20\n5.0\n11\n0.0\n21\n0.0\n\
                 0\nLINE\n10\n0.0\n20\n0.0\n11\n4.330127018922193\n21\n2.5\n";
        let read = read_dxf::<Bignum>(&file(Some(4), e), &opts()).expect("a sector");
        assert_eq!(read.report.loops, 1);
        assert!(!read.report.is_exact(), "an ARC entity is not free");
        assert!(read.report.delta < Q::new(1, 1_000_000_000_000_000i128));
        for el in &read.loops[0] {
            if let Element::Arc(a) = el {
                assert_eq!(a.r2, Q::from_i128(25), "the circle is the file's");
                assert!(a.is_consistent());
            }
        }
        // The welding the snapped endpoints needed is the *file's* gap, not our δ.
        assert!(read.report.closure_gap.sign() > 0);
        assert!(read.report.closure_gap != read.report.delta);
    }

    /// Refusals name the thing they refuse.
    #[test]
    fn unsupported_and_malformed_input_refuses_by_name() {
        let spline = "0\nSPLINE\n10\n0.0\n20\n0.0\n";
        assert!(matches!(
            read_dxf::<Bignum>(&file(Some(4), spline), &opts()),
            Err(ImportFault::UnsupportedEntity { kind }) if kind == "SPLINE"
        ));
        let bad = "0\nLINE\n10\n1,5\n20\n0.0\n11\n1.0\n21\n0.0\n";
        assert!(matches!(
            read_dxf::<Bignum>(&file(Some(4), bad), &opts()),
            Err(ImportFault::MalformedNumber { .. })
        ));
        assert!(matches!(
            read_dxf::<Bignum>("AutoCAD Binary DXF\r\n", &opts()),
            Err(ImportFault::BinaryDxf)
        ));
        let open = "0\nLINE\n10\n0.0\n20\n0.0\n11\n1.0\n21\n0.0\n\
                    0\nLINE\n10\n1.0\n20\n0.0\n11\n1.0\n21\n1.0\n";
        assert!(matches!(
            read_dxf::<Bignum>(&file(Some(4), open), &opts()),
            Err(ImportFault::OpenLoop { .. })
        ));
    }
}
