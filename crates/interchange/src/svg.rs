//! **SVG reading** — geometry only, with the y-axis and the units resolved exactly.
//!
//! XML is a real grammar and the SVG path mini-language is genuinely fiddly (implicit segment
//! repeats, the flag packing in `A`), so these are read through `roxmltree` + `svgtypes` rather
//! than by hand. The cost is that coordinates arrive as `f64`: the file's decimal text is recovered
//! through [`crate::num::rat_from_f64`], which returns the literal itself for anything under 17
//! significant digits, and what cannot be seen from this side is reported as
//! [`ImportReport::transport`](crate::report::ImportReport::transport) rather than assumed away.
//! DXF, whose grammar is trivial, is read directly and has no transport error at all.
//!
//! Two things SVG makes exact that look like they should not be:
//!
//! * **Units.** Every CSS absolute unit is a rational fraction of an inch, so `width="120mm"` with
//!   a `viewBox` gives an exact rational user-unit scale. A file with no absolute size is
//!   **refused** unless the caller names a unit — see [`crate::unit`].
//! * **Transforms.** `matrix`/`translate`/`scale` are exact affine maps over ℚ. A general affine
//!   map takes a circle to an ellipse, so a transform whose linear part is not a **similarity** is
//!   refused when the geometry contains an arc, and accepted when it does not. `rotate` is exact
//!   only at multiples of 90°; anything else is refused rather than snapped.
//!
//! SVG's y-axis points **down**. The reader flips it about the `viewBox`, which is exact and
//! orientation-reversing — so every arc's sweep flips with it, and the report says the flip
//! happened.

use crate::arc::{ArcFault, ArcTolerance, ExactArc, from_endpoints_radius};
use crate::element::{Element, assemble};
use crate::num::{rat_from_f64, sqrt_rational, to_decimal, transport_bound};
use crate::report::{ImportFault, ImportReport};
use crate::unit::Unit;
use crate::{Imported, max_of};
use lattice::{Backend, Rat};
use svgtypes::{
    Length, LengthUnit, PathParser, PathSegment, TransformListParser, TransformListToken,
};

/// How to read an SVG.
pub struct SvgOptions<B: Backend> {
    /// What one *user unit* means when the file gives no absolute `width`/`height`. `None` refuses
    /// such a file rather than assuming pixels.
    pub assume_unit: Option<Unit>,
    /// The unit the geometry comes out in.
    pub target: Unit,
    /// The largest gap between adjacent entities that assembly may absorb, in `target` units.
    pub weld: Rat<B>,
    /// Arc construction budget and refinement.
    pub arc: ArcTolerance<B>,
}

impl<B: Backend> Default for SvgOptions<B> {
    fn default() -> Self {
        SvgOptions {
            assume_unit: None,
            target: Unit::Millimetre,
            weld: Rat::new(1, 1000),
            arc: ArcTolerance::report_only(),
        }
    }
}

/// An exact 2-D affine map `x' = a·x + c·y + e`, `y' = b·x + d·y + f` — SVG's own `matrix(a b c d e f)`.
struct Mat23<B: Backend> {
    a: Rat<B>,
    b: Rat<B>,
    c: Rat<B>,
    d: Rat<B>,
    e: Rat<B>,
    f: Rat<B>,
}

impl<B: Backend> Clone for Mat23<B> {
    fn clone(&self) -> Self {
        Mat23 {
            a: self.a.clone(),
            b: self.b.clone(),
            c: self.c.clone(),
            d: self.d.clone(),
            e: self.e.clone(),
            f: self.f.clone(),
        }
    }
}

impl<B: Backend> Mat23<B> {
    fn identity() -> Self {
        let (o, z) = (Rat::from_i128(1), Rat::from_i128(0));
        Mat23 {
            a: o.clone(),
            b: z.clone(),
            c: z.clone(),
            d: o,
            e: z.clone(),
            f: z,
        }
    }

    /// `self ∘ other` — apply `other` first.
    fn compose(&self, o: &Self) -> Self {
        Mat23 {
            a: self.a.mul(&o.a).add(&self.c.mul(&o.b)),
            b: self.b.mul(&o.a).add(&self.d.mul(&o.b)),
            c: self.a.mul(&o.c).add(&self.c.mul(&o.d)),
            d: self.b.mul(&o.c).add(&self.d.mul(&o.d)),
            e: self.a.mul(&o.e).add(&self.c.mul(&o.f)).add(&self.e),
            f: self.b.mul(&o.e).add(&self.d.mul(&o.f)).add(&self.f),
        }
    }

    fn apply(&self, p: &[Rat<B>; 2]) -> [Rat<B>; 2] {
        [
            self.a.mul(&p[0]).add(&self.c.mul(&p[1])).add(&self.e),
            self.b.mul(&p[0]).add(&self.d.mul(&p[1])).add(&self.f),
        ]
    }

    fn det(&self) -> Rat<B> {
        self.a.mul(&self.d).sub(&self.b.mul(&self.c))
    }

    /// The squared uniform scale, if the linear part is a similarity — a rotation-scale
    /// `[[a, −b], [b, a]]` or its reflection `[[a, b], [b, −a]]`. `None` for anything that would
    /// take a circle to an ellipse.
    fn similarity_scale2(&self) -> Option<Rat<B>> {
        let rotation = self.a == self.d && self.b == self.c.clone().neg();
        let reflection = self.a == self.d.clone().neg() && self.b == self.c;
        (rotation || reflection).then(|| self.a.mul(&self.a).add(&self.b.mul(&self.b)))
    }
}

/// Read an SVG document into closed loops of exact geometry.
///
/// ```
/// use interchange::svg::{SvgOptions, read_svg};
/// use lattice::{Bignum, Rat};
///
/// // A 40 × 20 mm board outline, stated in millimetres by width/viewBox.
/// let doc = r#"<svg xmlns="http://www.w3.org/2000/svg" width="40mm" height="20mm"
///                   viewBox="0 0 40 20">
///                <rect x="0" y="0" width="40" height="20"/>
///              </svg>"#;
/// let read = read_svg::<Bignum>(doc, &SvgOptions::default()).expect("a board outline");
/// assert_eq!(read.report.loops, 1);
/// assert_eq!(read.report.scale, Rat::from_i128(1)); // one user unit is one millimetre
/// assert!(read.report.is_exact());
/// ```
pub fn read_svg<B: Backend>(text: &str, opts: &SvgOptions<B>) -> Result<Imported<B>, ImportFault> {
    let doc = roxmltree::Document::parse(text).map_err(|e| ImportFault::NotSvg(e.to_string()))?;
    let root = doc.root_element();
    if root.tag_name().name() != "svg" {
        return Err(ImportFault::NotSvg(format!(
            "root element is <{}>, not <svg>",
            root.tag_name().name()
        )));
    }

    let (source, scale, flip_about) = resolve_frame(&root, opts)?;
    // User units → target units, with the y-axis flipped about the viewBox so the drawing lands
    // where a viewer shows it. Exact, and orientation-reversing: every arc's sweep flips with it.
    let global = Mat23 {
        a: scale.clone(),
        b: Rat::from_i128(0),
        c: Rat::from_i128(0),
        d: scale.clone().neg(),
        e: Rat::from_i128(0),
        f: scale.mul(&flip_about),
    };

    let mut ctx = Ctx {
        elements: Vec::new(),
        entities: 0,
        delta: Rat::from_i128(0),
        transport: Rat::from_i128(0),
        opts,
    };
    walk(&root, &global, &mut ctx)?;

    if ctx.elements.is_empty() {
        return Err(ImportFault::Empty);
    }
    let entities = ctx.entities;
    let (delta, transport) = (ctx.delta.clone(), ctx.transport.mul(&scale));
    let assembled = assemble(ctx.elements, &opts.weld)?;
    Ok(Imported {
        report: ImportReport {
            source_unit: source,
            target_unit: opts.target,
            scale,
            entities,
            loops: assembled.loops.len(),
            delta,
            transport,
            closure_gap: assembled.closure_gap,
        },
        loops: assembled.loops,
    })
}

/// What one user unit is worth, and the y about which to flip.
fn resolve_frame<B: Backend>(
    root: &roxmltree::Node<'_, '_>,
    opts: &SvgOptions<B>,
) -> Result<(Unit, Rat<B>, Rat<B>), ImportFault> {
    let view_box = root
        .attribute("viewBox")
        .and_then(|v| v.parse::<svgtypes::ViewBox>().ok());
    let width = root
        .attribute("width")
        .and_then(|v| v.parse::<Length>().ok());

    // The flip axis: the viewBox's own bottom edge, so the drawing keeps its place.
    let flip_about = match &view_box {
        Some(vb) => exact(vb.y)?.add(&exact(vb.h)?),
        None => Rat::from_i128(0),
    };

    // An absolute width plus a viewBox pins the physical size of a user unit, exactly.
    if let (Some(vb), Some(w)) = (&view_box, &width)
        && let Some(unit) = css_unit(w.unit)
        && vb.w != 0.0
    {
        let physical = exact(w.number)?.mul(&unit.factor_to::<B>(opts.target));
        return Ok((unit, physical.div(&exact(vb.w)?), flip_about));
    }

    match opts.assume_unit {
        Some(u) => Ok((u, u.factor_to::<B>(opts.target), flip_about)),
        None => Err(ImportFault::UnknownUnit {
            declared: width.map(|w| format!("width has no absolute unit ({:?})", w.unit)),
        }),
    }
}

fn css_unit(u: LengthUnit) -> Option<Unit> {
    match u {
        LengthUnit::Mm => Some(Unit::Millimetre),
        LengthUnit::Cm => Some(Unit::Centimetre),
        LengthUnit::In => Some(Unit::Inch),
        LengthUnit::Pt => Some(Unit::Point),
        LengthUnit::Pc => Some(Unit::Pica),
        LengthUnit::Px => Some(Unit::Pixel),
        // `None` is a user unit, `Em`/`Ex`/`Percent` have no absolute length.
        _ => None,
    }
}

/// An `f64` from the parser as the decimal literal behind it.
fn exact<B: Backend>(x: f64) -> Result<Rat<B>, ImportFault> {
    rat_from_f64(x).ok_or_else(|| ImportFault::MalformedNumber {
        entity: "svg".into(),
        text: format!("{x}"),
    })
}

/// Accumulator threaded through the element walk.
struct Ctx<'o, B: Backend> {
    elements: Vec<Element<B>>,
    entities: usize,
    delta: Rat<B>,
    transport: Rat<B>,
    opts: &'o SvgOptions<B>,
}

/// Elements that carry no geometry and are skipped rather than refused.
const IGNORED: &[&str] = &[
    "title",
    "desc",
    "metadata",
    "defs",
    "style",
    "script",
    "sodipodi:namedview",
    "namedview",
];

fn walk<B: Backend>(
    node: &roxmltree::Node<'_, '_>,
    parent: &Mat23<B>,
    ctx: &mut Ctx<'_, B>,
) -> Result<(), ImportFault> {
    for child in node.children().filter(|n| n.is_element()) {
        let name = child.tag_name().name();
        if IGNORED.contains(&name) {
            continue;
        }
        let local = match child.attribute("transform") {
            Some(t) => parse_transform(t, name)?,
            None => Mat23::identity(),
        };
        let m = parent.compose(&local);

        let produced: Vec<Element<B>> = match name {
            "g" | "svg" | "a" => {
                walk(&child, &m, ctx)?;
                continue;
            }
            "path" => path_element(&child, &m, ctx.opts)?,
            "rect" => rect_element(&child, &m)?,
            "circle" => circle_element(&child, &m)?,
            "line" => line_element(&child, &m)?,
            "polygon" => poly_element(&child, &m, true)?,
            "polyline" => poly_element(&child, &m, false)?,
            "ellipse" => {
                return Err(ImportFault::EllipticalArc {
                    entity: "ellipse".into(),
                });
            }
            other => {
                return Err(ImportFault::UnsupportedEntity {
                    kind: other.to_string(),
                });
            }
        };
        ctx.entities += 1;
        for e in &produced {
            ctx.delta = max_of(ctx.delta.clone(), e.delta());
        }
        ctx.transport = max_of(ctx.transport.clone(), node_transport(&child));
        ctx.elements.extend(produced);
    }
    Ok(())
}

/// An ulp bound over every number this element's attributes carry — what a parser could have cost
/// before we saw the text.
fn node_transport<B: Backend>(node: &roxmltree::Node<'_, '_>) -> Rat<B> {
    let mut worst = Rat::from_i128(0);
    for a in node.attributes() {
        for tok in a.value().split([' ', ',', '\t', '\n']) {
            if let Ok(x) = tok.trim().parse::<f64>() {
                worst = max_of(worst, transport_bound(x));
            }
        }
    }
    worst
}

fn parse_transform<B: Backend>(text: &str, entity: &str) -> Result<Mat23<B>, ImportFault> {
    let mut m = Mat23::identity();
    for token in TransformListParser::from(text) {
        let token = token.map_err(|e| ImportFault::MalformedNumber {
            entity: entity.to_string(),
            text: e.to_string(),
        })?;
        let step = match token {
            TransformListToken::Matrix { a, b, c, d, e, f } => Mat23 {
                a: exact(a)?,
                b: exact(b)?,
                c: exact(c)?,
                d: exact(d)?,
                e: exact(e)?,
                f: exact(f)?,
            },
            TransformListToken::Translate { tx, ty } => Mat23 {
                e: exact(tx)?,
                f: exact(ty)?,
                ..Mat23::identity()
            },
            TransformListToken::Scale { sx, sy } => Mat23 {
                a: exact(sx)?,
                d: exact(sy)?,
                ..Mat23::identity()
            },
            // A rotation is exact only where its sine and cosine are rational. Multiples of 90°
            // are; nothing else is, and snapping one silently would move the whole drawing.
            TransformListToken::Rotate { angle } => {
                let quarter = angle / 90.0;
                if quarter.fract() != 0.0 {
                    return Err(ImportFault::IrrationalTransform {
                        entity: entity.to_string(),
                    });
                }
                let (c, s) = match (quarter as i64).rem_euclid(4) {
                    0 => (1, 0),
                    1 => (0, 1),
                    2 => (-1, 0),
                    _ => (0, -1),
                };
                Mat23 {
                    a: Rat::from_i128(c),
                    b: Rat::from_i128(s),
                    c: Rat::from_i128(-s),
                    d: Rat::from_i128(c),
                    ..Mat23::identity()
                }
            }
            TransformListToken::SkewX { .. } | TransformListToken::SkewY { .. } => {
                return Err(ImportFault::IrrationalTransform {
                    entity: entity.to_string(),
                });
            }
        };
        m = m.compose(&step);
    }
    Ok(m)
}

/// An attribute as an exact rational, defaulting to zero when absent (SVG's own rule).
fn attr<B: Backend>(
    node: &roxmltree::Node<'_, '_>,
    name: &str,
    entity: &str,
) -> Result<Rat<B>, ImportFault> {
    let Some(text) = node.attribute(name) else {
        return Ok(Rat::from_i128(0));
    };
    let length = text
        .trim()
        .parse::<Length>()
        .map_err(|_| ImportFault::MalformedNumber {
            entity: entity.to_string(),
            text: text.to_string(),
        })?;
    if !matches!(length.unit, LengthUnit::None | LengthUnit::Px) {
        // A per-attribute unit would be a second scale on top of the document's; refuse rather
        // than silently applying one of them.
        return Err(ImportFault::UnsupportedEntity {
            kind: format!("{entity}/@{name} with unit {:?}", length.unit),
        });
    }
    exact(length.number)
}

/// Map an arc through an exact affine map — refusing one that would make it an ellipse.
fn map_arc<B: Backend>(
    a: ExactArc<B>,
    m: &Mat23<B>,
    entity: &str,
) -> Result<ExactArc<B>, ImportFault> {
    let Some(s2) = m.similarity_scale2() else {
        return Err(ImportFault::NonSimilarityTransform {
            entity: entity.to_string(),
        });
    };
    let centre = m.apply(&[a.cx.clone(), a.cy.clone()]);
    let flips = m.det().sign() < 0;
    ExactArc::exact(
        centre[0].clone(),
        centre[1].clone(),
        a.r2.mul(&s2),
        m.apply(&a.start),
        m.apply(&a.end),
        a.ccw != flips,
    )
    .map(|mut mapped| {
        // A length scales by √s², so the backward error does too.
        mapped.delta = a.delta.mul(&sqrt_rational(&s2, 32));
        mapped
    })
    .map_err(|f| arc_fault(entity, &f))
}

fn arc_fault<B: Backend>(entity: &str, f: &ArcFault<B>) -> ImportFault {
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

fn path_element<B: Backend>(
    node: &roxmltree::Node<'_, '_>,
    m: &Mat23<B>,
    opts: &SvgOptions<B>,
) -> Result<Vec<Element<B>>, ImportFault> {
    let Some(d) = node.attribute("d") else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    // Current point and subpath start, in **user** coordinates (the arc constructions need the
    // radius in the same space as the endpoints; the whole element is mapped as it is emitted).
    let mut cur = [Rat::from_i128(0), Rat::from_i128(0)];
    let mut sub_start = cur.clone();
    let zero = Rat::from_i128(0);

    let push_seg = |a: &[Rat<B>; 2], b: &[Rat<B>; 2], out: &mut Vec<Element<B>>| {
        if a != b {
            out.push(Element::Segment {
                start: m.apply(a),
                end: m.apply(b),
            });
        }
    };

    for seg in PathParser::from(d) {
        let seg = seg.map_err(|e| ImportFault::MalformedNumber {
            entity: "path".into(),
            text: e.to_string(),
        })?;
        let base = |abs: bool| {
            if abs {
                [zero.clone(), zero.clone()]
            } else {
                cur.clone()
            }
        };
        match seg {
            PathSegment::MoveTo { abs, x, y } => {
                let o = base(abs);
                cur = [o[0].add(&exact(x)?), o[1].add(&exact(y)?)];
                sub_start = cur.clone();
            }
            PathSegment::LineTo { abs, x, y } => {
                let o = base(abs);
                let next = [o[0].add(&exact(x)?), o[1].add(&exact(y)?)];
                push_seg(&cur, &next, &mut out);
                cur = next;
            }
            PathSegment::HorizontalLineTo { abs, x } => {
                let o = base(abs);
                let next = [o[0].add(&exact(x)?), cur[1].clone()];
                push_seg(&cur, &next, &mut out);
                cur = next;
            }
            PathSegment::VerticalLineTo { abs, y } => {
                let o = base(abs);
                let next = [cur[0].clone(), o[1].add(&exact(y)?)];
                push_seg(&cur, &next, &mut out);
                cur = next;
            }
            PathSegment::EllipticalArc {
                abs,
                rx,
                ry,
                x_axis_rotation,
                large_arc,
                sweep,
                x,
                y,
            } => {
                if rx != ry {
                    return Err(ImportFault::EllipticalArc {
                        entity: "path/A".into(),
                    });
                }
                if x_axis_rotation != 0.0 {
                    // Irrelevant for a circle, but a file that states it may not mean a circle.
                    return Err(ImportFault::EllipticalArc {
                        entity: "path/A (x-axis-rotation)".into(),
                    });
                }
                let o = base(abs);
                let next = [o[0].add(&exact(x)?), o[1].add(&exact(y)?)];
                let arc = from_endpoints_radius(
                    cur.clone(),
                    next.clone(),
                    &exact(rx)?,
                    large_arc,
                    sweep,
                    &opts.arc,
                )
                .map_err(|f| arc_fault("path/A", &f))?;
                out.push(Element::Arc(map_arc(arc, m, "path/A")?));
                cur = next;
            }
            PathSegment::ClosePath { .. } => {
                push_seg(&cur, &sub_start, &mut out);
                cur = sub_start.clone();
            }
            PathSegment::CurveTo { .. }
            | PathSegment::SmoothCurveTo { .. }
            | PathSegment::Quadratic { .. }
            | PathSegment::SmoothQuadratic { .. } => {
                return Err(ImportFault::BezierSegment {
                    entity: "path".into(),
                });
            }
        }
    }
    Ok(out)
}

fn rect_element<B: Backend>(
    node: &roxmltree::Node<'_, '_>,
    m: &Mat23<B>,
) -> Result<Vec<Element<B>>, ImportFault> {
    let (x, y) = (attr(node, "x", "rect")?, attr(node, "y", "rect")?);
    let (w, h) = (attr(node, "width", "rect")?, attr(node, "height", "rect")?);
    if w.sign() <= 0 || h.sign() <= 0 {
        return Ok(Vec::new()); // SVG: a zero dimension disables rendering.
    }
    let rx = match (node.attribute("rx"), node.attribute("ry")) {
        (None, None) => Rat::from_i128(0),
        _ => {
            let (rx, ry) = (attr(node, "rx", "rect")?, attr(node, "ry", "rect")?);
            if rx != ry {
                return Err(ImportFault::EllipticalArc {
                    entity: "rect (rx ≠ ry)".into(),
                });
            }
            rx
        }
    };

    let (x1, y1) = (x.add(&w), y.add(&h));
    if rx.is_zero() {
        let corners = [
            [x.clone(), y.clone()],
            [x1.clone(), y.clone()],
            [x1.clone(), y1.clone()],
            [x.clone(), y1.clone()],
        ];
        return Ok((0..4)
            .map(|i| Element::Segment {
                start: m.apply(&corners[i]),
                end: m.apply(&corners[(i + 1) % 4]),
            })
            .collect());
    }

    // A rounded rectangle: four sides, and four quarter arcs whose endpoints are the axis-aligned
    // tangent points — exactly rational, so `δ = 0` with no search at all.
    let r2 = rx.mul(&rx);
    let (xa, xb) = (x.add(&rx), x1.sub(&rx));
    let (ya, yb) = (y.add(&rx), y1.sub(&rx));
    let mut out: Vec<Element<B>> = Vec::new();
    let seg = |a: [Rat<B>; 2], b: [Rat<B>; 2], out: &mut Vec<Element<B>>| {
        if a != b {
            out.push(Element::Segment {
                start: m.apply(&a),
                end: m.apply(&b),
            });
        }
    };
    // Clockwise in SVG's y-down space, starting at the top edge.
    seg([xa.clone(), y.clone()], [xb.clone(), y.clone()], &mut out);
    let corner = |cx: Rat<B>,
                  cy: Rat<B>,
                  s: [Rat<B>; 2],
                  e: [Rat<B>; 2]|
     -> Result<Element<B>, ImportFault> {
        let a =
            ExactArc::exact(cx, cy, r2.clone(), s, e, false).map_err(|f| arc_fault("rect", &f))?;
        Ok(Element::Arc(map_arc(a, m, "rect")?))
    };
    out.push(corner(
        xb.clone(),
        ya.clone(),
        [xb.clone(), y.clone()],
        [x1.clone(), ya.clone()],
    )?);
    seg([x1.clone(), ya.clone()], [x1.clone(), yb.clone()], &mut out);
    out.push(corner(
        xb.clone(),
        yb.clone(),
        [x1.clone(), yb.clone()],
        [xb.clone(), y1.clone()],
    )?);
    seg([xb.clone(), y1.clone()], [xa.clone(), y1.clone()], &mut out);
    out.push(corner(
        xa.clone(),
        yb.clone(),
        [xa.clone(), y1.clone()],
        [x.clone(), yb.clone()],
    )?);
    seg([x.clone(), yb.clone()], [x.clone(), ya.clone()], &mut out);
    out.push(corner(
        xa.clone(),
        ya.clone(),
        [x.clone(), ya.clone()],
        [xa.clone(), y.clone()],
    )?);
    Ok(out)
}

fn circle_element<B: Backend>(
    node: &roxmltree::Node<'_, '_>,
    m: &Mat23<B>,
) -> Result<Vec<Element<B>>, ImportFault> {
    let r = attr(node, "r", "circle")?;
    if r.sign() <= 0 {
        return Ok(Vec::new());
    }
    let Some(s2) = m.similarity_scale2() else {
        return Err(ImportFault::NonSimilarityTransform {
            entity: "circle".into(),
        });
    };
    let c = m.apply(&[attr(node, "cx", "circle")?, attr(node, "cy", "circle")?]);
    Ok(vec![Element::Circle {
        cx: c[0].clone(),
        cy: c[1].clone(),
        r2: r.mul(&r).mul(&s2),
    }])
}

fn line_element<B: Backend>(
    node: &roxmltree::Node<'_, '_>,
    m: &Mat23<B>,
) -> Result<Vec<Element<B>>, ImportFault> {
    Ok(vec![Element::Segment {
        start: m.apply(&[attr(node, "x1", "line")?, attr(node, "y1", "line")?]),
        end: m.apply(&[attr(node, "x2", "line")?, attr(node, "y2", "line")?]),
    }])
}

fn poly_element<B: Backend>(
    node: &roxmltree::Node<'_, '_>,
    m: &Mat23<B>,
    closed: bool,
) -> Result<Vec<Element<B>>, ImportFault> {
    let entity = if closed { "polygon" } else { "polyline" };
    let Some(text) = node.attribute("points") else {
        return Ok(Vec::new());
    };
    let pts: Vec<[Rat<B>; 2]> = svgtypes::PointsParser::from(text)
        .map(|(x, y)| Ok([exact(x)?, exact(y)?]))
        .collect::<Result<_, ImportFault>>()?;
    if pts.len() < 2 {
        return Err(ImportFault::MalformedNumber {
            entity: entity.into(),
            text: text.to_string(),
        });
    }
    let n = pts.len();
    let last = if closed { n } else { n - 1 };
    Ok((0..last)
        .filter(|&i| pts[i] != pts[(i + 1) % n])
        .map(|i| Element::Segment {
            start: m.apply(&pts[i]),
            end: m.apply(&pts[(i + 1) % n]),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice::Bignum;

    type Q = Rat<Bignum>;

    fn opts() -> SvgOptions<Bignum> {
        SvgOptions::default()
    }

    fn doc(body: &str) -> String {
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="40mm" height="20mm"
                    viewBox="0 0 40 20">{body}</svg>"#
        )
    }

    /// **The y-axis flip is real and exact.** SVG's y points down; the model's points up. A rect
    /// at the top of the viewBox must come out at the *top* of the model, not the bottom.
    #[test]
    fn the_y_axis_is_flipped_about_the_view_box() {
        let read = read_svg::<Bignum>(
            &doc(r#"<rect x="0" y="0" width="10" height="4"/>"#),
            &opts(),
        )
        .expect("a rect");
        let ys: Vec<Q> = read.loops[0]
            .iter()
            .filter_map(|e| e.start().map(|p| p[1].clone()))
            .collect();
        // y ∈ {0, 4} in SVG becomes {20, 16} in the model — the viewBox is 20 tall.
        assert!(ys.contains(&Q::from_i128(20)), "{ys:?}");
        assert!(ys.contains(&Q::from_i128(16)), "{ys:?}");
        assert!(!ys.contains(&Q::from_i128(0)), "the flip did not happen");
    }

    /// A rounded rectangle — the flex outline shape — imports with `δ = 0`: its corner arcs meet
    /// the sides at the axis-aligned tangent points, which are exactly rational.
    #[test]
    fn a_rounded_rect_imports_exactly_with_its_corners_still_arcs() {
        let read = read_svg::<Bignum>(
            &doc(r#"<rect x="2" y="2" width="36" height="16" rx="4" ry="4"/>"#),
            &opts(),
        )
        .expect("a rounded rect");
        assert_eq!(read.report.loops, 1);
        assert_eq!(read.loops[0].len(), 8, "four sides and four corners");
        assert_eq!(read.loops[0].iter().filter(|e| e.is_arc()).count(), 4);
        assert_eq!(
            read.report.delta,
            Q::from_i128(0),
            "tangent points are exact"
        );
        assert_eq!(read.report.closure_gap, Q::from_i128(0));
        for e in &read.loops[0] {
            if let Element::Arc(a) = e {
                assert!(a.is_consistent());
                assert_eq!(a.r2, Q::from_i128(16), "r² survived the flip exactly");
            }
        }
    }

    /// Physical units resolve exactly, and the control is a document that says the same thing in
    /// inches: the geometry must be the millimetre one scaled by exactly `127/5`.
    #[test]
    fn the_same_drawing_in_inches_is_the_millimetre_one_times_127_over_5() {
        let mm = read_svg::<Bignum>(
            &doc(r#"<rect x="0" y="0" width="10" height="4"/>"#),
            &opts(),
        )
        .expect("mm");
        let inches = String::from(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="40in" height="20in"
                    viewBox="0 0 40 20"><rect x="0" y="0" width="10" height="4"/></svg>"#,
        );
        let inch = read_svg::<Bignum>(&inches, &opts()).expect("in");
        assert_eq!(mm.report.scale, Q::from_i128(1));
        assert_eq!(inch.report.scale, Q::new(127, 5));
        let widest = |r: &Imported<Bignum>| {
            r.loops[0]
                .iter()
                .filter_map(|e| e.start().map(|p| p[0].clone()))
                .fold(Q::from_i128(0), max_of)
        };
        assert_eq!(widest(&inch), widest(&mm).mul(&Q::new(127, 5)));
    }

    /// A document that never says how big it is is refused, not read as pixels.
    #[test]
    fn a_document_with_no_physical_size_refuses() {
        let d = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 20">
                     <rect x="0" y="0" width="10" height="4"/></svg>"#;
        assert!(matches!(
            read_svg::<Bignum>(d, &opts()),
            Err(ImportFault::UnknownUnit { .. })
        ));
        let assumed = SvgOptions {
            assume_unit: Some(Unit::Millimetre),
            ..opts()
        };
        assert!(read_svg::<Bignum>(d, &assumed).is_ok());
    }

    /// Refusals, by name — including the one a real workflow hits most: an exporter that turned
    /// every curve into a cubic.
    #[test]
    fn the_named_refusals_fire_on_the_things_they_name() {
        /// A predicate naming the fault a body must produce.
        type Expect = fn(&ImportFault) -> bool;
        let cases: Vec<(&str, Expect)> = vec![
            (r#"<path d="M0,0 C1,1 2,2 3,3 Z"/>"#, |f| {
                matches!(f, ImportFault::BezierSegment { .. })
            }),
            (r#"<ellipse cx="5" cy="5" rx="3" ry="2"/>"#, |f| {
                matches!(f, ImportFault::EllipticalArc { .. })
            }),
            (r#"<path d="M0,0 A3,2 0 0 1 4,0 Z"/>"#, |f| {
                matches!(f, ImportFault::EllipticalArc { .. })
            }),
            (r#"<text x="0" y="0">hi</text>"#, |f| {
                matches!(f, ImportFault::UnsupportedEntity { .. })
            }),
            (
                r#"<g transform="rotate(37)"><circle cx="5" cy="5" r="2"/></g>"#,
                |f| matches!(f, ImportFault::IrrationalTransform { .. }),
            ),
            (
                r#"<g transform="scale(2,3)"><circle cx="5" cy="5" r="2"/></g>"#,
                |f| matches!(f, ImportFault::NonSimilarityTransform { .. }),
            ),
        ];
        for (body, want) in cases {
            let got = read_svg::<Bignum>(&doc(body), &opts());
            match got {
                Err(ref f) if want(f) => {}
                other => panic!("{body} gave {other:?}"),
            }
        }
    }

    /// An exact transform is applied rather than refused, and a uniform scale keeps a circle a
    /// circle — `r²` picking up exactly the squared factor.
    #[test]
    fn exact_similarity_transforms_are_applied() {
        let read = read_svg::<Bignum>(
            &doc(r#"<g transform="translate(4,2) scale(3)"><circle cx="2" cy="2" r="1"/></g>"#),
            &opts(),
        )
        .expect("a scaled circle");
        let Element::Circle { cx, cy, r2 } = &read.loops[0][0] else {
            panic!("expected a circle")
        };
        assert_eq!(*r2, Q::from_i128(9), "r² × 3²");
        assert_eq!(*cx, Q::from_i128(10), "4 + 3·2");
        assert_eq!(*cy, Q::from_i128(12), "flip: 20 − (2 + 3·2)");
    }

    /// A path of lines and a circular arc, with the arc surviving as an arc.
    #[test]
    fn a_path_of_lines_and_one_arc_closes() {
        let read = read_svg::<Bignum>(
            &doc(r#"<path d="M4,10 L10,10 A3,3 0 0 1 4,10 Z"/>"#),
            &opts(),
        )
        .expect("a D-shape");
        assert_eq!(read.report.loops, 1);
        assert_eq!(read.loops[0].iter().filter(|e| e.is_arc()).count(), 1);
        // The chord is 6 and the radius 3, so the arc is a semicircle: exactly representable, and
        // the reader must find that rather than settling for a bound.
        assert_eq!(read.report.delta, Q::from_i128(0));
    }
}
